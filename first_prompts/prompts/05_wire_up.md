# Step 5: Wire It Up

Reference: See `prompts/00_SPEC.md` for overall design.

## Goal
Connect input → API → display. Full working chat (no streaming yet).

## Update App

Add:
```rust
pub api_client: Option<ApiClient>,  // Option in case key is missing
pub is_loading: bool,
```

Initialize `api_client` in `App::new()`, handle missing key gracefully.

## Async Architecture

The TUI event loop is synchronous, but API calls are async. Use channels:

```rust
use tokio::sync::mpsc;

enum AppEvent {
    Key(KeyEvent),
    ApiResponse(String),
    ApiError(String),
}
```

In main:
1. Create `mpsc::channel` for events
2. Spawn a task that listens for keyboard events and sends them
3. Main loop receives from channel, handles both keys and API responses

When user submits a message:
1. Set `is_loading = true`
2. Clone the channel sender
3. Spawn async task:
   ```rust
   tokio::spawn(async move {
       match api_client.send_message(&messages, system).await {
           Ok(response) => tx.send(AppEvent::ApiResponse(response)).await,
           Err(e) => tx.send(AppEvent::ApiError(e.to_string())).await,
       }
   });
   ```
4. When `ApiResponse` received:
   - Push Assistant message to conversation
   - Set `is_loading = false`
   - Scroll to bottom

## Update UI

When `is_loading`:
- Show "Claude is thinking..." or animated dots after the user's message
- Show "(thinking...)" in status bar
- Optionally dim the input area

## Handle Errors

When `ApiError` received:
- Set `is_loading = false`
- Show error in a system message (gray/italic) or in status bar

## Test
1. Set ANTHROPIC_API_KEY
2. `cargo run`
3. Type a question, press Enter
4. See your message appear in blue bubble
5. See loading indicator
6. See Claude's actual response appear in green bubble
7. Continue conversation - context should work
