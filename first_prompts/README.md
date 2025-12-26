# claude-tui Prompts

Prompts for building a terminal-based Claude chat client in Rust.

## How to Use

Feed these to Claude Code one at a time, in order:

1. `00_SPEC.md` - Reference document (don't need to "run" this, just keep it available)
2. `01_skeleton.md` - Basic TUI
3. `02_input_field.md` - Text input with modes
4. `03_messages.md` - Message bubbles and scrolling
5. `04_api_client.md` - Anthropic API client
6. `05_wire_up.md` - Connect everything
7. `06_streaming.md` - Stream responses
8. `07_tabs.md` - Multiple conversations
9. `08_polish.md` - Save/load, help, error handling

## Usage with Claude Code

For each step:
```bash
claude "$(cat prompts/01_skeleton.md)"
```

Or just paste the content into Claude Code.

Test after each step before moving to the next.

## Requirements

- Rust toolchain
- `ANTHROPIC_API_KEY` environment variable (needed from step 4 onwards)
