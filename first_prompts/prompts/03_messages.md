# Step 3: Message Display

Reference: See `prompts/00_SPEC.md` for overall design.

## Goal
Render messages as styled bubbles with scrolling.

## Create `src/conversation.rs`

Implement `Role`, `Message`, `Conversation` as defined in SPEC.

```rust
use chrono::{DateTime, Local};

#[derive(Clone)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub timestamp: DateTime<Local>,
}

impl Message {
    pub fn new(role: Role, content: String) -> Self {
        Self {
            role,
            content,
            timestamp: Local::now(),
        }
    }
}

pub struct Conversation {
    pub messages: Vec<Message>,
    pub scroll_offset: usize,
}
```

## Update `src/app.rs`

Add `conversation: Conversation` to App.

Update `submit()`:
1. Create User message from input
2. Push to conversation
3. Clear input
4. Push fake Assistant message: "This is a placeholder. API coming in step 4!"

## Update `src/ui.rs`

Render messages in the messages area:

User messages:
- Right-aligned
- Blue border, rounded corners
- Timestamp below in dark gray

Assistant messages:
- Left-aligned  
- Green border, rounded corners
- Timestamp below in dark gray

Max bubble width: 70% of available width.
Wrap text inside bubbles.

## Add Scrolling

Add `scroll_offset: usize` to Conversation (or App).

Normal mode keys:
- `j` / `↓` → Scroll down
- `k` / `↑` → Scroll up
- `g` → Top
- `G` → Bottom

## Test
- Type message, Enter, see blue bubble on right
- Fake response appears as green bubble on left
- Add several messages, scroll with j/k
- g goes to top, G to bottom
