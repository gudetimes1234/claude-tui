# Step 2: Input Field

Reference: See `prompts/00_SPEC.md` for overall design.

## Goal
Add text input with Normal/Insert modes.

## Add Dependencies
```toml
chrono = "0.4"
```

## Create `src/app.rs`

```rust
pub enum Mode {
    Normal,
    Insert,
}

pub struct App {
    pub input: String,
    pub cursor_position: usize,
    pub mode: Mode,
    pub should_quit: bool,
}
```

Methods:
- `new() -> Self`
- `move_cursor_left(&mut self)`
- `move_cursor_right(&mut self)`
- `insert_char(&mut self, c: char)`
- `delete_char(&mut self)`
- `submit(&mut self) -> Option<String>`

## Create `src/ui.rs`

`render(app: &App, frame: &mut Frame)` function.

Layout:
```
╭─ claude-tui ────────────────────────────────────────────────╮
│                     (messages area - empty)                 │
├─────────────────────────────────────────────────────────────┤
│ > input here_                                               │
├─────────────────────────────────────────────────────────────┤
│ NORMAL | Press 'i' to type, 'q' to quit                     │
╰─────────────────────────────────────────────────────────────╯
```

- Input border: Blue in Insert mode, Gray in Normal
- Show cursor only in Insert mode
- Status bar shows current mode

## Update main.rs

Handle keybindings per mode as specified in SPEC.

## Test
- 'i' enters Insert mode (border turns blue)
- Type text, arrow keys move cursor, backspace deletes
- Enter clears input
- Escape returns to Normal mode
- 'q' in Normal mode quits
