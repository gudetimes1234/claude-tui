# Step 8: Polish

Reference: See `prompts/00_SPEC.md` for overall design.

## Goal
Save/load, help overlay, error handling, final polish.

## Add Dependencies
```toml
dirs = "5"
```

## Save/Load Conversations

### Directory
`~/.local/share/claude-tui/conversations/`

Create it if it doesn't exist.

### File Format
JSON files named `{uuid}.json`:

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "Help with Rust lifetimes",
  "system_prompt": null,
  "messages": [
    {
      "role": "user",
      "content": "Explain lifetimes",
      "timestamp": "2025-01-15T10:30:00Z"
    },
    {
      "role": "assistant", 
      "content": "Lifetimes are...",
      "timestamp": "2025-01-15T10:30:05Z"
    }
  ]
}
```

### Functions
Add to `conversation.rs` or new `storage.rs`:

```rust
pub fn save_conversation(conv: &Conversation) -> anyhow::Result<PathBuf>
pub fn load_conversation(path: &Path) -> anyhow::Result<Conversation>
pub fn list_saved_conversations() -> anyhow::Result<Vec<(PathBuf, String)>> // path, title
pub fn get_storage_dir() -> PathBuf
```

### Keybinding
- `Ctrl+s` → Save current conversation, show confirmation in status bar

### Optional: Auto-save
Save after each new message. Or save on quit.

---

## Help Overlay

Add `Mode::Help` to the Mode enum.

When `?` is pressed in Normal mode, set `mode = Mode::Help`.

### Render Help

Draw a centered overlay on top of everything:

```
╭─ Keybindings ───────────────────────────────────────╮
│                                                     │
│  Normal Mode                                        │
│  ───────────                                        │
│  i, Enter       Insert mode                         │
│  q              Quit                                │
│  j, k           Scroll up/down                      │
│  g, G           Top/bottom of chat                  │
│  Ctrl+n         New conversation                    │
│  Ctrl+w         Close conversation                  │
│  Ctrl+h/l       Previous/next tab                   │
│  Ctrl+s         Save conversation                   │
│  ?              Toggle this help                    │
│                                                     │
│  Insert Mode                                        │
│  ───────────                                        │
│  Escape         Normal mode                         │
│  Enter          Send message                        │
│  ←/→            Move cursor                         │
│  Backspace      Delete character                    │
│                                                     │
│            Press any key to close                   │
╰─────────────────────────────────────────────────────╯
```

Use `Clear` widget to draw a background, then the popup on top.

Any keypress in Help mode → return to Normal mode.

---

## Error Handling

### Missing API Key
- Don't crash on startup
- Show in status bar: "⚠ ANTHROPIC_API_KEY not set"
- If user tries to send, show error message in chat

### API Errors
- Network failure, rate limit, etc.
- Show as a system message in the conversation:
  ```
  ╭─ Error ─────────────────────────────────────────────╮
  │ API Error: Connection refused                       │
  ╰─────────────────────────────────────────────────────╯
  ```
- Use gray/red border, italic text

### Graceful Degradation
- App should always be usable even if API is down
- Can still create tabs, type, scroll, etc.

---

## Final Polish

### Empty State
When a conversation has no messages, show centered hint text:
```
Start typing to begin a conversation with Claude.
Press 'i' to enter insert mode, then type your message.
```

### Timestamps
Format nicely:
- Today: "10:30 AM"
- Yesterday: "Yesterday 10:30 AM"  
- This week: "Mon 10:30 AM"
- Older: "Jan 15, 10:30 AM"

### Text Wrapping
Ensure long messages wrap properly inside bubbles.
Test with very long single words and URLs.

### Terminal Resize
Handle `Event::Resize` - just re-render, layout should adapt.

### Status Bar Polish
Show useful info:
```
NORMAL | Chat: Rust help | 5 msgs | Saved ✓ | ^n new  ^w close  ? help
```

---

## Test Checklist
- [ ] `Ctrl+s` saves conversation to disk
- [ ] Saved files appear in `~/.local/share/claude-tui/conversations/`
- [ ] `?` shows help overlay, any key dismisses
- [ ] Unset API key - app still runs, shows warning
- [ ] Simulate network error - shows error message, doesn't crash
- [ ] Resize terminal while running - layout adapts
- [ ] Empty conversation shows hint text
- [ ] Timestamps look correct
- [ ] Long messages wrap properly
