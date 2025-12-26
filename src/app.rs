use crate::api::ApiClient;
use crate::conversation::{Conversation, Message, Role};
use crate::storage;

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Normal,
    Insert,
    Help,
}

pub struct App {
    pub input: String,
    pub cursor_position: usize,
    pub mode: Mode,
    pub should_quit: bool,
    pub conversations: Vec<Conversation>,
    pub active_tab: usize,
    pub api_client: Option<ApiClient>,
    pub is_loading: bool,
    pub error_message: Option<String>,
    pub status_message: Option<String>,
    pub current_model: Option<String>,
    pub pending_model_change: Option<String>,
}

impl App {
    pub fn new() -> Self {
        let (api_client, current_model) = match ApiClient::new() {
            Ok(client) => {
                let model = client.get_model().to_string();
                (Some(client), Some(model))
            }
            Err(e) => {
                eprintln!("Warning: {}", e);
                (None, None)
            }
        };

        Self {
            input: String::new(),
            cursor_position: 0,
            mode: Mode::Normal,
            should_quit: false,
            conversations: vec![Conversation::new()],
            active_tab: 0,
            api_client,
            is_loading: false,
            error_message: None,
            status_message: None,
            current_model,
            pending_model_change: None,
        }
    }

    pub fn current_conversation(&self) -> &Conversation {
        &self.conversations[self.active_tab]
    }

    pub fn current_conversation_mut(&mut self) -> &mut Conversation {
        &mut self.conversations[self.active_tab]
    }

    pub fn new_conversation(&mut self) {
        self.conversations.push(Conversation::new());
        self.active_tab = self.conversations.len() - 1;
    }

    pub fn close_current_conversation(&mut self) {
        if self.conversations.len() > 1 {
            self.conversations.remove(self.active_tab);
            if self.active_tab >= self.conversations.len() {
                self.active_tab = self.conversations.len() - 1;
            }
        }
    }

    pub fn next_tab(&mut self) {
        if self.active_tab < self.conversations.len() - 1 {
            self.active_tab += 1;
        }
    }

    pub fn prev_tab(&mut self) {
        if self.active_tab > 0 {
            self.active_tab -= 1;
        }
    }

    pub fn save_current_conversation(&mut self) {
        match storage::save_conversation(self.current_conversation()) {
            Ok(_) => {
                self.status_message = Some("Conversation saved ✓".to_string());
            }
            Err(e) => {
                self.set_error(format!("Failed to save: {}", e));
            }
        }
    }

    pub fn toggle_help(&mut self) {
        self.mode = if self.mode == Mode::Help {
            Mode::Normal
        } else {
            Mode::Help
        };
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_position < self.input.len() {
            self.cursor_position += 1;
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor_position, c);
        self.cursor_position += 1;
    }

    pub fn delete_char(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            self.input.remove(self.cursor_position);
        }
    }

    pub fn submit(&mut self) -> Option<String> {
        if self.input.is_empty() {
            return None;
        }
        let input = std::mem::take(&mut self.input);
        self.cursor_position = 0;

        // Check for commands
        if input.starts_with('/') {
            self.handle_command(&input);
            return None;
        }

        // Add user message
        self.current_conversation_mut()
            .add_message(Message::new(Role::User, input.clone()));

        Some(input)
    }

    fn handle_command(&mut self, input: &str) {
        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        match parts[0] {
            "/model" => {
                if parts.len() > 1 {
                    let new_model = parts[1].trim().to_string();
                    self.pending_model_change = Some(new_model.clone());
                    self.status_message = Some(format!("Model set to: {}", new_model));
                } else {
                    // Show current model
                    let current = self.current_model.as_deref().unwrap_or("unknown");
                    self.status_message = Some(format!("Current model: {}", current));
                }
            }
            "/help" => {
                self.mode = Mode::Help;
            }
            _ => {
                self.set_error(format!("Unknown command: {}", parts[0]));
            }
        }
    }

    pub fn start_assistant_message(&mut self) {
        self.current_conversation_mut()
            .add_message(Message::new(Role::Assistant, String::new()));
    }

    pub fn append_to_last_message(&mut self, text: &str) {
        if let Some(last) = self.current_conversation_mut().messages.last_mut() {
            last.content.push_str(text);
        }
    }

    pub fn finish_streaming(&mut self) {
        self.is_loading = false;
    }

    pub fn set_error(&mut self, error: String) {
        self.error_message = Some(error);
        self.is_loading = false;
    }

    pub fn clear_error(&mut self) {
        self.error_message = None;
        self.status_message = None;
    }

    pub fn has_api_key(&self) -> bool {
        self.api_client.is_some()
    }
}
