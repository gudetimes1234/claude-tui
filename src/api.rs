use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::conversation::{Message, Role};

#[derive(Serialize)]
struct ApiMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    _type: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct ApiResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct StreamDelta {
    #[serde(rename = "type")]
    delta_type: Option<String>,
    text: Option<String>,
}

#[derive(Deserialize)]
struct StreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<StreamDelta>,
}

pub enum StreamChunk {
    Text(String),
    Done,
    Error(String),
}

pub struct ApiClient {
    client: reqwest::Client,
    api_key: String,
    pub model: String,
}

const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";

impl ApiClient {
    pub fn new() -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow!("ANTHROPIC_API_KEY not set"))?;

        let model = std::env::var("CLAUDE_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

        Ok(Self {
            client: reqwest::Client::new(),
            api_key,
            model,
        })
    }

    pub fn set_model(&mut self, model: String) {
        self.model = model;
    }

    pub fn get_model(&self) -> &str {
        &self.model
    }

    fn build_request(&self, messages: &[Message], system_prompt: Option<&str>, stream: bool, model_override: Option<&str>) -> ApiRequest {
        let api_messages: Vec<ApiMessage> = messages
            .iter()
            .map(|m| ApiMessage {
                role: match m.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                },
                content: m.content.clone(),
            })
            .collect();

        ApiRequest {
            model: model_override.unwrap_or(&self.model).to_string(),
            max_tokens: 4096,
            system: system_prompt.map(|s| s.to_string()),
            messages: api_messages,
            stream,
        }
    }

    pub async fn send_message(
        &self,
        messages: &[Message],
        system_prompt: Option<&str>,
        model_override: Option<&str>,
    ) -> Result<String> {
        let request = self.build_request(messages, system_prompt, false, model_override);

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("API error {}: {}", status, text));
        }

        let api_response: ApiResponse = response.json().await?;

        api_response
            .content
            .first()
            .and_then(|block| block.text.clone())
            .ok_or_else(|| anyhow!("No text in response"))
    }

    pub async fn send_message_streaming(
        &self,
        messages: &[Message],
        system_prompt: Option<&str>,
        model_override: Option<&str>,
        tx: mpsc::Sender<StreamChunk>,
    ) -> Result<()> {
        let request = self.build_request(messages, system_prompt, true, model_override);

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            let _ = tx.send(StreamChunk::Error(format!("API error {}: {}", status, text))).await;
            return Ok(());
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(bytes) => {
                    buffer.push_str(&String::from_utf8_lossy(&bytes));

                    // Process complete lines
                    while let Some(newline_pos) = buffer.find('\n') {
                        let line = buffer[..newline_pos].to_string();
                        buffer = buffer[newline_pos + 1..].to_string();

                        if line.starts_with("data: ") {
                            let json_str = &line[6..];
                            if let Ok(event) = serde_json::from_str::<StreamEvent>(json_str) {
                                match event.event_type.as_str() {
                                    "content_block_delta" => {
                                        if let Some(delta) = event.delta {
                                            if delta.delta_type.as_deref() == Some("text_delta") {
                                                if let Some(text) = delta.text {
                                                    let _ = tx.send(StreamChunk::Text(text)).await;
                                                }
                                            }
                                        }
                                    }
                                    "message_stop" => {
                                        let _ = tx.send(StreamChunk::Done).await;
                                        return Ok(());
                                    }
                                    "error" => {
                                        let _ = tx.send(StreamChunk::Error("Stream error".to_string())).await;
                                        return Ok(());
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(StreamChunk::Error(e.to_string())).await;
                    return Ok(());
                }
            }
        }

        let _ = tx.send(StreamChunk::Done).await;
        Ok(())
    }
}
