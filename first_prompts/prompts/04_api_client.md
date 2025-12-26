# Step 4: API Client

Reference: See `prompts/00_SPEC.md` for overall design.

## Goal
Create an Anthropic API client (no streaming yet).

## Add Dependencies
```toml
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

## Create `src/api.rs`

```rust
use crate::conversation::{Message, Role};
use anyhow::Result;

pub struct ApiClient {
    client: reqwest::Client,
    api_key: String,
}

impl ApiClient {
    pub fn new() -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;
        
        Ok(Self {
            client: reqwest::Client::new(),
            api_key,
        })
    }

    pub async fn send_message(
        &self,
        messages: &[Message],
        system_prompt: Option<&str>,
    ) -> Result<String> {
        // POST to https://api.anthropic.com/v1/messages
        // Model: claude-sonnet-4-20250514
        // Max tokens: 4096
        // Return the assistant's response text
        todo!()
    }
}
```

### Request Format
```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 4096,
  "system": "optional system prompt",
  "messages": [
    {"role": "user", "content": "..."},
    {"role": "assistant", "content": "..."}
  ]
}
```

### Headers
- `x-api-key: {key}`
- `anthropic-version: 2023-06-01`
- `content-type: application/json`

### Response Format
```json
{
  "content": [
    {
      "type": "text",
      "text": "The response text here"
    }
  ]
}
```

Extract the text from `content[0].text`.

## Test
Create a simple test in main or a separate bin:
```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = ApiClient::new()?;
    let messages = vec![Message::new(Role::User, "Say hello in exactly 5 words.".into())];
    let response = client.send_message(&messages, None).await?;
    println!("{}", response);
    Ok(())
}
```

Should print Claude's response.
