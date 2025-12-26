# Step 1: Skeleton

Reference: See `prompts/00_SPEC.md` for overall design.

## Goal
Create the project with a basic TUI that displays and exits cleanly.

## Cargo.toml
```toml
[package]
name = "claude-tui"
version = "0.1.0"
edition = "2021"

[dependencies]
ratatui = "0.29"
crossterm = "0.28"
tokio = { version = "1", features = ["full"] }
anyhow = "1"
```

## main.rs

1. Initialize crossterm: raw mode, alternate screen
2. Set up panic hook to restore terminal on crash
3. Create ratatui Terminal with CrosstermBackend
4. Render loop:
   - Draw a centered box (60x20)
   - Rounded corners
   - Title: " claude-tui "
   - Centered text inside: "Press 'q' to quit"
   - Border color: Cyan
5. Event loop: poll for keys, quit on 'q'
6. Restore terminal on exit

## Test
```bash
cargo run
```

Should see a nice box, 'q' exits, terminal is normal after.
