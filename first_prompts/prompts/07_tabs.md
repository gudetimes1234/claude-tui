# Step 7: Tabs

Reference: See `prompts/00_SPEC.md` for overall design.

## Goal
Multiple conversations in tabs.

## Add Dependencies
```toml
uuid = { version = "1", features = ["v4"] }
```

## Update `src/conversation.rs`

```rust
use uuid::Uuid;

pub struct Conversation {
    pub id: Uuid,
    pub title: Option<String>,
    pub messages: Vec<Message>,
    pub system_prompt: Option<String>,
    pub scroll_offset: usize,
}

impl Conversation {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            title: None,
            messages: Vec::new(),
            system_prompt: None,
            scroll_offset: 0,
        }
    }
    
    /// Generate title from first user message
    pub fn generate_title(&mut self) {
        if self.title.is_none() {
            if let Some(msg) = self.messages.iter().find(|m| matches!(m.role, Role::User)) {
                // Take first 30 chars of first user message
                let title: String = msg.content.chars().take(30).collect();
                self.title = Some(if msg.content.len() > 30 {
                    format!("{}...", title)
                } else {
                    title
                });
            }
        }
    }
}
```

## Update `src/app.rs`

Change from single Conversation to:
```rust
pub struct App {
    pub conversations: Vec<Conversation>,
    pub active_tab: usize,
    // ... rest of fields
}
```

Add methods:
```rust
impl App {
    pub fn new_conversation(&mut self) {
        self.conversations.push(Conversation::new());
        self.active_tab = self.conversations.len() - 1;
    }
    
    pub fn close_current_conversation(&mut self) {
        if self.conversations.len() > 1 {
            self.conversations.remove(self.active_tab);
            if self.active_tab >= self.conversations.len() {
                self.active_tab = self.conversations.len() - 1;
            }
        }
    }
    
    pub fn next_tab(&mut self) {
        if self.active_tab < self.conversations.len() - 1 {
            self.active_tab += 1;
        }
    }
    
    pub fn prev_tab(&mut self) {
        if self.active_tab > 0 {
            self.active_tab -= 1;
        }
    }
    
    pub fn current_conversation(&self) -> &Conversation {
        &self.conversations[self.active_tab]
    }
    
    pub fn current_conversation_mut(&mut self) -> &mut Conversation {
        &mut self.conversations[self.active_tab]
    }
}
```

Initialize with one conversation in `App::new()`.

## Update `src/ui.rs`

Add tab bar at top of the layout:

```
╭─────────────────────────────────────────────────────────────╮
│ [Chat 1] [Chat 2] [New Chat]                                │
├─────────────────────────────────────────────────────────────┤
│                    (messages area)                          │
```

Use `ratatui::widgets::Tabs` or render manually with `Span`s.

Styling:
- Active tab: Bold, highlighted background or different color
- Inactive tabs: Normal/dimmed
- Show title or "New Chat" if no title yet

## Keybindings

Add to Normal mode handling:
- `Ctrl+n` → `app.new_conversation()`
- `Ctrl+w` → `app.close_current_conversation()`
- `Ctrl+h` or `Ctrl+Left` → `app.prev_tab()`
- `Ctrl+l` or `Ctrl+Right` → `app.next_tab()`

Note: In crossterm, Ctrl+arrow might come through as different key codes. Test and adjust.

## Test
- App starts with one tab
- `Ctrl+n` creates new tab, switches to it
- Type messages in different tabs - they're independent
- `Ctrl+h` / `Ctrl+l` switches between tabs
- Tab titles update after first message
- `Ctrl+w` closes tab (except the last one)
