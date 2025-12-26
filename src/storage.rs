use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::conversation::{Conversation, Message, Role};

#[derive(Serialize, Deserialize)]
struct SavedMessage {
    role: String,
    content: String,
    timestamp: String,
}

#[derive(Serialize, Deserialize)]
struct SavedConversation {
    id: String,
    title: Option<String>,
    system_prompt: Option<String>,
    messages: Vec<SavedMessage>,
}

pub fn get_storage_dir() -> PathBuf {
    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("claude-tui")
        .join("conversations");

    // Create directory if it doesn't exist
    let _ = fs::create_dir_all(&data_dir);

    data_dir
}

pub fn save_conversation(conv: &Conversation) -> Result<PathBuf> {
    let saved = SavedConversation {
        id: conv.id.to_string(),
        title: conv.title.clone(),
        system_prompt: conv.system_prompt.clone(),
        messages: conv
            .messages
            .iter()
            .map(|m| SavedMessage {
                role: match m.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                },
                content: m.content.clone(),
                timestamp: m.timestamp.to_rfc3339(),
            })
            .collect(),
    };

    let path = get_storage_dir().join(format!("{}.json", conv.id));
    let json = serde_json::to_string_pretty(&saved)?;
    fs::write(&path, json)?;

    Ok(path)
}

pub fn list_saved_conversations() -> Result<Vec<(PathBuf, String)>> {
    let dir = get_storage_dir();
    let mut results = Vec::new();

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(saved) = serde_json::from_str::<SavedConversation>(&content) {
                        let title = saved.title.unwrap_or_else(|| "Untitled".to_string());
                        results.push((path, title));
                    }
                }
            }
        }
    }

    Ok(results)
}
