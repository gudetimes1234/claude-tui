# claude-tui Specification

A terminal-based chat client for the Anthropic API, written in Rust.

## Stack
- ratatui 0.29 - TUI framework
- crossterm 0.28 - Terminal backend
- tokio 1 - Async runtime
- reqwest 0.12 - HTTP client (with `json` and `stream` features)
- serde/serde_json - JSON serialization
- anyhow - Error handling
- chrono - Timestamps
- dirs - Config paths
- uuid - Conversation IDs

## Architecture

```
src/
├── main.rs          # Entry, terminal setup, event loop
├── app.rs           # App state, mode handling
├── ui.rs            # All rendering logic
├── api.rs           # Anthropic API client
├── conversation.rs  # Message and conversation types
```

## Core Types

```rust
pub enum Mode {
    Normal,
    Insert,
    Help,
}

pub enum Role {
    User,
    Assistant,
}

pub struct Message {
    pub role: Role,
    pub content: String,
    pub timestamp: DateTime<Local>,
}

pub struct Conversation {
    pub id: Uuid,
    pub title: Option<String>,
    pub messages: Vec<Message>,
    pub system_prompt: Option<String>,
    pub scroll_offset: usize,
}

pub struct App {
    pub conversations: Vec<Conversation>,
    pub active_tab: usize,
    pub input: String,
    pub cursor_position: usize,
    pub mode: Mode,
    pub is_loading: bool,
    pub should_quit: bool,
}
```

## UI Layout

```
╭─ Tabs ──────────────────────────────────────────────────────╮
│ [Chat 1] [Chat 2] [+ New]                                   │
├─ Messages ──────────────────────────────────────────────────┤
│                                                             │
│   ╭─────────────────────────────────────────╮               │
│   │ Assistant message, left-aligned         │               │
│   ╰─────────────────────────────────────────╯               │
│   10:30 AM                                                  │
│                                                             │
│                   ╭─────────────────────────────────────╮   │
│                   │ User message, right-aligned         │   │
│                   ╰─────────────────────────────────────╯   │
│                                              10:31 AM       │
│                                                             │
├─ Input ─────────────────────────────────────────────────────┤
│ > _                                                         │
├─ Status ────────────────────────────────────────────────────┤
│ NORMAL | Chat 1 | 3 msgs | ^n new  ^w close  ? help        │
╰─────────────────────────────────────────────────────────────╯
```

## Keybindings

### Normal Mode
- `i` or `Enter` → Insert mode
- `q` → Quit
- `j/k` or `↑/↓` → Scroll messages
- `g` → Top of conversation
- `G` → Bottom of conversation
- `Ctrl+n` → New tab
- `Ctrl+w` → Close tab
- `Ctrl+h` or `Ctrl+←` → Previous tab
- `Ctrl+l` or `Ctrl+→` → Next tab
- `Ctrl+s` → Save conversation
- `?` → Toggle help overlay

### Insert Mode
- `Escape` → Normal mode
- `Enter` → Send message
- `Backspace` → Delete char
- `←/→` → Move cursor
- Printable chars → Insert

## Visual Style
- Rounded borders everywhere
- User bubbles: Blue border, right-aligned
- Assistant bubbles: Green border, left-aligned
- Timestamps: Dark gray, below bubbles
- Active tab: Highlighted
- Insert mode: Blue input border
- Normal mode: Gray input border
- Loading: Show spinner or "..." indicator
- Max bubble width: 70% of screen width
