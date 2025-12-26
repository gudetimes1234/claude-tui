# Step 6: Streaming

Reference: See `prompts/00_SPEC.md` for overall design.

## Goal
Stream responses token-by-token for better UX.

## Add Dependencies
```toml
reqwest = { version = "0.12", features = ["json", "stream"] }
futures-util = "0.3"
```

## Update `src/api.rs`

Add streaming method:
```rust
use futures_util::Stream;

pub async fn send_message_streaming(
    &self,
    messages: &[Message],
    system_prompt: Option<&str>,
) -> Result<impl Stream<Item = Result<String>>> {
    // POST with "stream": true in the body
    // Return a stream that yields text chunks
}
```

### Streaming Request
Same as before, but add `"stream": true` to the JSON body.

### SSE Response Format
The response is Server-Sent Events. Relevant events:

```
event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: message_stop
data: {"type":"message_stop"}
```

Parse logic:
1. Read lines from response body
2. When you see `event: content_block_delta`, the next `data:` line has the payload
3. Parse JSON, extract `delta.text`
4. Yield that text
5. Stop when you see `message_stop`

Use `reqwest`'s `.bytes_stream()` and parse SSE manually, or line-by-line.

## Update Event Loop

New event type:
```rust
enum AppEvent {
    Key(KeyEvent),
    ApiResponse(String),      // Complete response (keep for non-streaming fallback)
    ApiChunk(String),         // Streaming chunk
    ApiStreamDone,            // Stream finished
    ApiError(String),
}
```

When streaming starts:
1. Set `is_loading = true`
2. Create empty Assistant message, push to conversation
3. Spawn task that streams chunks

For each `ApiChunk`:
1. Append text to the last Assistant message's content
2. Re-render (UI updates live)

On `ApiStreamDone`:
1. Set `is_loading = false`

## Test
- Send a message
- Watch response appear word-by-word
- Much more responsive feel
- Long responses stream smoothly
