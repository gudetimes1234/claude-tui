use chrono::{DateTime, Local};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq)]
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
    pub id: Uuid,
    pub title: Option<String>,
    pub messages: Vec<Message>,
    pub system_prompt: Option<String>,
    pub scroll_offset: usize,
}

impl Conversation {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            title: None,
            messages: Vec::new(),
            system_prompt: None,
            scroll_offset: 0,
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        self.generate_title();
    }

    pub fn generate_title(&mut self) {
        if self.title.is_none() {
            if let Some(msg) = self.messages.iter().find(|m| matches!(m.role, Role::User)) {
                let title: String = msg.content.chars().take(30).collect();
                self.title = Some(if msg.content.len() > 30 {
                    format!("{}...", title)
                } else {
                    title
                });
            }
        }
    }

    pub fn display_title(&self) -> &str {
        self.title.as_deref().unwrap_or("New Chat")
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    pub fn scroll_down(&mut self, max_visible: usize) {
        let max_scroll = self.messages.len().saturating_sub(max_visible);
        if self.scroll_offset < max_scroll {
            self.scroll_offset += 1;
        }
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_to_bottom(&mut self, max_visible: usize) {
        self.scroll_offset = self.messages.len().saturating_sub(max_visible);
    }
}

impl Default for Conversation {
    fn default() -> Self {
        Self::new()
    }
}
