use std::fs;
use std::io::{self, stdout, BufRead, BufReader};
use std::panic;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Tabs, Wrap},
    Frame, Terminal,
};
use serde::{Deserialize, Serialize};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const MODEL: &str = "claude-sonnet-4-20250514";
const SAVE_DIR: &str = ".claude-tui";

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    Normal,
    Insert,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    role: Role,
    content: String,
}

impl Message {
    fn new(role: Role, content: String) -> Self {
        Self { role, content }
    }
}

#[derive(Debug, Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum StreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: StreamMessage },
    #[serde(rename = "content_block_start")]
    ContentBlockStart { index: usize, content_block: ContentBlock },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: usize, delta: Delta },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: usize },
    #[serde(rename = "message_delta")]
    MessageDelta { delta: MessageDeltaContent, usage: Option<Usage> },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "error")]
    Error { error: StreamError },
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct StreamMessage {
    id: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ContentBlock {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Delta {
    #[serde(rename = "type")]
    delta_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct MessageDeltaContent {
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Usage {
    output_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct StreamError {
    message: String,
}

enum AppEvent {
    StreamStart(usize),
    StreamDelta(usize, String),
    StreamEnd(usize),
    StreamError(usize, String),
}

struct InputField {
    content: String,
    cursor: usize,
}

impl InputField {
    fn new() -> Self {
        Self {
            content: String::new(),
            cursor: 0,
        }
    }

    fn insert(&mut self, c: char) {
        self.content.insert(self.cursor, c);
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.content.remove(self.cursor);
        }
    }

    fn delete(&mut self) {
        if self.cursor < self.content.len() {
            self.content.remove(self.cursor);
        }
    }

    fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    fn move_right(&mut self) {
        if self.cursor < self.content.len() {
            self.cursor += 1;
        }
    }

    fn move_start(&mut self) {
        self.cursor = 0;
    }

    fn move_end(&mut self) {
        self.cursor = self.content.len();
    }

    fn clear(&mut self) -> String {
        let content = std::mem::take(&mut self.content);
        self.cursor = 0;
        content
    }

    fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

/// Saved conversation format
#[derive(Debug, Serialize, Deserialize)]
struct SavedConversation {
    name: String,
    messages: Vec<Message>,
}

/// A single conversation tab
struct Conversation {
    id: usize,
    name: String,
    messages: Vec<Message>,
    scroll_offset: usize,
    is_loading: bool,
    streaming_content: String,
    input: InputField,
}

impl Conversation {
    fn new(id: usize) -> Self {
        Self {
            id,
            name: format!("Chat {}", id + 1),
            messages: Vec::new(),
            scroll_offset: 0,
            is_loading: false,
            streaming_content: String::new(),
            input: InputField::new(),
        }
    }

    fn from_saved(id: usize, saved: SavedConversation) -> Self {
        Self {
            id,
            name: saved.name,
            messages: saved.messages,
            scroll_offset: 0,
            is_loading: false,
            streaming_content: String::new(),
            input: InputField::new(),
        }
    }

    fn to_saved(&self) -> SavedConversation {
        SavedConversation {
            name: self.name.clone(),
            messages: self.messages.clone(),
        }
    }

    fn add_message(&mut self, role: Role, content: String) {
        self.messages.push(Message::new(role, content));
        self.scroll_to_bottom();
    }

    fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    fn scroll_down(&mut self, visible_lines: usize, total_lines: usize) {
        if total_lines > visible_lines && self.scroll_offset < total_lines - visible_lines {
            self.scroll_offset += 1;
        }
    }

    fn scroll_to_bottom(&mut self) {
        self.scroll_offset = usize::MAX;
    }
}

struct App {
    mode: Mode,
    should_quit: bool,
    conversations: Vec<Conversation>,
    active_tab: usize,
    next_id: usize,
    api_key: Option<String>,
    error_message: Option<String>,
    status_message: Option<String>,
    event_rx: mpsc::Receiver<AppEvent>,
    event_tx: mpsc::Sender<AppEvent>,
    show_help: bool,
}

impl App {
    fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel();
        let api_key = std::env::var("ANTHROPIC_API_KEY").ok();

        let mut app = Self {
            mode: Mode::Normal,
            should_quit: false,
            conversations: Vec::new(),
            active_tab: 0,
            next_id: 0,
            api_key,
            error_message: None,
            status_message: None,
            event_rx,
            event_tx,
            show_help: false,
        };

        // Try to load saved conversations
        if let Err(_) = app.load_conversations() {
            // If loading fails, create a fresh tab
            app.new_tab();
        }

        if app.conversations.is_empty() {
            app.new_tab();
        }

        app
    }

    fn save_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(SAVE_DIR)
    }

    fn save_conversations(&self) -> Result<()> {
        let save_dir = Self::save_dir();
        fs::create_dir_all(&save_dir)?;

        let conversations: Vec<SavedConversation> = self
            .conversations
            .iter()
            .filter(|c| !c.messages.is_empty()) // Only save non-empty conversations
            .map(|c| c.to_saved())
            .collect();

        let json = serde_json::to_string_pretty(&conversations)?;
        fs::write(save_dir.join("conversations.json"), json)?;

        Ok(())
    }

    fn load_conversations(&mut self) -> Result<()> {
        let save_path = Self::save_dir().join("conversations.json");

        if !save_path.exists() {
            return Ok(());
        }

        let json = fs::read_to_string(save_path)?;
        let saved: Vec<SavedConversation> = serde_json::from_str(&json)?;

        for saved_conv in saved {
            let conv = Conversation::from_saved(self.next_id, saved_conv);
            self.next_id += 1;
            self.conversations.push(conv);
        }

        Ok(())
    }

    fn current_conversation(&self) -> &Conversation {
        &self.conversations[self.active_tab]
    }

    fn current_conversation_mut(&mut self) -> &mut Conversation {
        &mut self.conversations[self.active_tab]
    }

    fn new_tab(&mut self) {
        let conv = Conversation::new(self.next_id);
        self.next_id += 1;
        self.conversations.push(conv);
        self.active_tab = self.conversations.len() - 1;
    }

    fn close_tab(&mut self) {
        if self.conversations.len() > 1 {
            self.conversations.remove(self.active_tab);
            if self.active_tab >= self.conversations.len() {
                self.active_tab = self.conversations.len() - 1;
            }
        }
    }

    fn next_tab(&mut self) {
        if !self.conversations.is_empty() {
            self.active_tab = (self.active_tab + 1) % self.conversations.len();
        }
    }

    fn prev_tab(&mut self) {
        if !self.conversations.is_empty() {
            self.active_tab = if self.active_tab == 0 {
                self.conversations.len() - 1
            } else {
                self.active_tab - 1
            };
        }
    }

    fn send_message(&mut self) {
        let conv = self.current_conversation_mut();
        if conv.input.is_empty() || conv.is_loading {
            return;
        }

        let content = conv.input.clear();
        conv.add_message(Role::User, content);
        self.error_message = None;

        let api_key = match &self.api_key {
            Some(key) => key.clone(),
            None => {
                self.error_message = Some("ANTHROPIC_API_KEY not set. Export it and restart.".to_string());
                return;
            }
        };

        let conv = self.current_conversation_mut();
        conv.is_loading = true;
        conv.streaming_content.clear();

        let messages = conv.messages.clone();
        let tab_id = conv.id;
        let tx = self.event_tx.clone();

        thread::spawn(move || {
            stream_api_call(&api_key, &messages, tab_id, tx);
        });
    }

    fn process_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                AppEvent::StreamStart(tab_id) => {
                    if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == tab_id) {
                        conv.streaming_content.clear();
                    }
                }
                AppEvent::StreamDelta(tab_id, text) => {
                    if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == tab_id) {
                        conv.streaming_content.push_str(&text);
                        conv.scroll_to_bottom();
                    }
                }
                AppEvent::StreamEnd(tab_id) => {
                    if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == tab_id) {
                        conv.is_loading = false;
                        if !conv.streaming_content.is_empty() {
                            let content = std::mem::take(&mut conv.streaming_content);
                            conv.add_message(Role::Assistant, content);
                        }
                    }
                    // Auto-save after receiving a response
                    let _ = self.save_conversations();
                }
                AppEvent::StreamError(tab_id, e) => {
                    if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == tab_id) {
                        conv.is_loading = false;
                        conv.streaming_content.clear();
                    }
                    if self.conversations.get(self.active_tab).map(|c| c.id) == Some(tab_id) {
                        self.error_message = Some(e);
                    }
                }
            }
        }
    }

    fn clear_current_conversation(&mut self) {
        let conv = self.current_conversation_mut();
        conv.messages.clear();
        conv.scroll_offset = 0;
        self.status_message = Some("Conversation cleared".to_string());
    }
}

fn stream_api_call(api_key: &str, messages: &[Message], tab_id: usize, tx: mpsc::Sender<AppEvent>) {
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(AppEvent::StreamError(tab_id, format!("Client error: {}", e)));
            return;
        }
    };

    let request = ApiRequest {
        model: MODEL.to_string(),
        max_tokens: 4096,
        messages: messages.to_vec(),
        stream: true,
    };

    let response = match client
        .post(API_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
    {
        Ok(resp) => resp,
        Err(e) => {
            let msg = if e.is_timeout() {
                "Request timed out. Please try again.".to_string()
            } else if e.is_connect() {
                "Connection failed. Check your internet connection.".to_string()
            } else {
                format!("Request failed: {}", e)
            };
            let _ = tx.send(AppEvent::StreamError(tab_id, msg));
            return;
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();

        let msg = match status.as_u16() {
            401 => "Invalid API key. Check your ANTHROPIC_API_KEY.".to_string(),
            429 => "Rate limited. Please wait and try again.".to_string(),
            500..=599 => "API server error. Please try again later.".to_string(),
            _ => format!("API error ({}): {}", status, body),
        };

        let _ = tx.send(AppEvent::StreamError(tab_id, msg));
        return;
    }

    let _ = tx.send(AppEvent::StreamStart(tab_id));

    let reader = BufReader::new(response);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                let _ = tx.send(AppEvent::StreamError(tab_id, format!("Read error: {}", e)));
                return;
            }
        };

        if !line.starts_with("data: ") {
            continue;
        }

        let json_str = &line[6..];

        if json_str == "[DONE]" {
            break;
        }

        let event: StreamEvent = match serde_json::from_str(json_str) {
            Ok(e) => e,
            Err(_) => continue,
        };

        match event {
            StreamEvent::ContentBlockDelta { delta, .. } => {
                if let Some(text) = delta.text {
                    if tx.send(AppEvent::StreamDelta(tab_id, text)).is_err() {
                        return;
                    }
                }
            }
            StreamEvent::Error { error } => {
                let _ = tx.send(AppEvent::StreamError(tab_id, error.message));
                return;
            }
            StreamEvent::MessageStop => {
                break;
            }
            _ => {}
        }
    }

    let _ = tx.send(AppEvent::StreamEnd(tab_id));
}

fn main() -> Result<()> {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal();
        original_hook(panic_info);
    }));

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let result = run(&mut terminal, &mut app);

    // Save before exiting
    let _ = app.save_conversations();

    restore_terminal()?;

    result
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        app.process_events();

        // Clear temporary status messages
        if app.status_message.is_some() {
            app.status_message = None;
        }

        terminal.draw(|frame| ui(frame, app))?;

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key(app, key.code, key.modifiers);
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn handle_key(app: &mut App, key: KeyCode, modifiers: KeyModifiers) {
    // Handle help overlay
    if app.show_help {
        app.show_help = false;
        return;
    }

    if app.error_message.is_some() && key != KeyCode::Esc {
        app.error_message = None;
    }

    // Help key
    if key == KeyCode::Char('?') && app.mode == Mode::Normal {
        app.show_help = true;
        return;
    }

    // Global keybindings
    if modifiers.contains(KeyModifiers::CONTROL) {
        match key {
            KeyCode::Char('t') => {
                app.new_tab();
                return;
            }
            KeyCode::Char('w') => {
                app.close_tab();
                return;
            }
            KeyCode::Char('n') => {
                app.next_tab();
                return;
            }
            KeyCode::Char('p') => {
                app.prev_tab();
                return;
            }
            KeyCode::Char('s') => {
                if let Err(e) = app.save_conversations() {
                    app.error_message = Some(format!("Save failed: {}", e));
                } else {
                    app.status_message = Some("Saved!".to_string());
                }
                return;
            }
            KeyCode::Char('l') => {
                app.clear_current_conversation();
                return;
            }
            _ => {}
        }
    }

    // Tab key for switching tabs
    if key == KeyCode::Tab && modifiers.is_empty() && app.mode == Mode::Normal {
        app.next_tab();
        return;
    }
    if key == KeyCode::BackTab {
        app.prev_tab();
        return;
    }

    match app.mode {
        Mode::Normal => handle_normal_mode(app, key),
        Mode::Insert => handle_insert_mode(app, key, modifiers),
    }
}

fn handle_normal_mode(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('i') => app.mode = Mode::Insert,
        KeyCode::Char('a') => {
            app.mode = Mode::Insert;
            app.current_conversation_mut().input.move_right();
        }
        KeyCode::Char('A') => {
            app.mode = Mode::Insert;
            app.current_conversation_mut().input.move_end();
        }
        KeyCode::Char('I') => {
            app.mode = Mode::Insert;
            app.current_conversation_mut().input.move_start();
        }
        KeyCode::Char('h') | KeyCode::Left => app.current_conversation_mut().input.move_left(),
        KeyCode::Char('l') | KeyCode::Right => app.current_conversation_mut().input.move_right(),
        KeyCode::Char('0') | KeyCode::Home => app.current_conversation_mut().input.move_start(),
        KeyCode::Char('$') | KeyCode::End => app.current_conversation_mut().input.move_end(),
        KeyCode::Char('x') => app.current_conversation_mut().input.delete(),
        KeyCode::Char('d') => {
            app.current_conversation_mut().input.clear();
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.current_conversation_mut().scroll_down(20, 100);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.current_conversation_mut().scroll_up();
        }
        KeyCode::Char('G') => {
            app.current_conversation_mut().scroll_to_bottom();
        }
        KeyCode::Char('g') => {
            app.current_conversation_mut().scroll_offset = 0;
        }
        // Number keys 1-9 to switch tabs
        KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
            let idx = (c as usize) - ('1' as usize);
            if idx < app.conversations.len() {
                app.active_tab = idx;
            }
        }
        KeyCode::Esc => app.should_quit = true,
        _ => {}
    }
}

fn handle_insert_mode(app: &mut App, key: KeyCode, modifiers: KeyModifiers) {
    if modifiers.contains(KeyModifiers::CONTROL) {
        match key {
            KeyCode::Char('a') => {
                app.current_conversation_mut().input.move_start();
                return;
            }
            KeyCode::Char('e') => {
                app.current_conversation_mut().input.move_end();
                return;
            }
            KeyCode::Char('u') => {
                app.current_conversation_mut().input.clear();
                return;
            }
            _ => {}
        }
    }

    match key {
        KeyCode::Esc => app.mode = Mode::Normal,
        KeyCode::Char(c) => {
            app.current_conversation_mut().input.insert(c);
        }
        KeyCode::Backspace => app.current_conversation_mut().input.backspace(),
        KeyCode::Delete => app.current_conversation_mut().input.delete(),
        KeyCode::Left => app.current_conversation_mut().input.move_left(),
        KeyCode::Right => app.current_conversation_mut().input.move_right(),
        KeyCode::Home => app.current_conversation_mut().input.move_start(),
        KeyCode::End => app.current_conversation_mut().input.move_end(),
        KeyCode::Enter => {
            app.send_message();
        }
        KeyCode::Up => app.current_conversation_mut().scroll_up(),
        KeyCode::Down => app.current_conversation_mut().scroll_down(20, 100),
        _ => {}
    }
}

fn ui(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(3),
        Constraint::Length(1),
    ])
    .split(area);

    render_header(frame, chunks[0], app);
    render_messages(frame, chunks[1], app);
    render_input(frame, chunks[2], app);
    render_status(frame, chunks[3], app);

    // Render help overlay if active
    if app.show_help {
        render_help(frame);
    }
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let tab_titles: Vec<Line> = app
        .conversations
        .iter()
        .enumerate()
        .map(|(i, conv)| {
            let style = if conv.is_loading {
                Style::default().fg(Color::Yellow)
            } else if i == app.active_tab {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Line::from(Span::styled(format!(" {} ", conv.name), style))
        })
        .collect();

    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title(" claude-tui ")
                .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .style(Style::default().fg(Color::DarkGray)),
        )
        .select(app.active_tab)
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .divider(Span::raw("|"));

    frame.render_widget(tabs, area);
}

fn render_messages(frame: &mut Frame, area: Rect, app: &mut App) {
    let conv = app.current_conversation_mut();
    let inner_width = area.width.saturating_sub(4) as usize;
    let inner_height = area.height.saturating_sub(2) as usize;

    let mut lines: Vec<Line> = Vec::new();

    if conv.messages.is_empty() && conv.streaming_content.is_empty() {
        lines.push(Line::from(Span::styled(
            "No messages yet. Press 'i' to enter insert mode and type a message.",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Press '?' for help.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for msg in &conv.messages {
            let (role_label, role_style) = match msg.role {
                Role::User => (
                    "You",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Role::Assistant => (
                    "Claude",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ),
            };

            lines.push(Line::from(Span::styled(role_label, role_style)));

            let content_style = match msg.role {
                Role::User => Style::default().fg(Color::White),
                Role::Assistant => Style::default().fg(Color::Cyan),
            };

            for line in wrap_text(&msg.content, inner_width) {
                lines.push(Line::from(Span::styled(line, content_style)));
            }

            lines.push(Line::from(""));
        }

        if !conv.streaming_content.is_empty() {
            lines.push(Line::from(Span::styled(
                "Claude",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )));

            for line in wrap_text(&conv.streaming_content, inner_width) {
                lines.push(Line::from(Span::styled(
                    line,
                    Style::default().fg(Color::Cyan),
                )));
            }

            lines.push(Line::from(Span::styled(
                "▌",
                Style::default().fg(Color::Yellow),
            )));
        }
    }

    if conv.is_loading && conv.streaming_content.is_empty() {
        lines.push(Line::from(Span::styled(
            "Claude is thinking...",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::ITALIC),
        )));
    }

    let total_lines = lines.len();
    if conv.scroll_offset == usize::MAX {
        conv.scroll_offset = total_lines.saturating_sub(inner_height);
    } else if total_lines > inner_height && conv.scroll_offset > total_lines - inner_height {
        conv.scroll_offset = total_lines.saturating_sub(inner_height);
    }

    let messages = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title(format!(" {} ", conv.name))
                .style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(Wrap { trim: false })
        .scroll((conv.scroll_offset as u16, 0));

    frame.render_widget(messages, area);
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();

    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        let words: Vec<&str> = paragraph.split_whitespace().collect();
        if words.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current_line = String::new();

        for word in words {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

fn render_input(frame: &mut Frame, area: Rect, app: &App) {
    let conv = app.current_conversation();
    let border_color = if conv.is_loading {
        Color::Yellow
    } else {
        match app.mode {
            Mode::Insert => Color::Green,
            Mode::Normal => Color::DarkGray,
        }
    };

    let title = if conv.is_loading {
        " Input (streaming...) "
    } else {
        " Input "
    };

    let input = Paragraph::new(conv.input.content.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title(title)
                .style(Style::default().fg(border_color)),
        );

    frame.render_widget(input, area);

    if !conv.is_loading {
        let cursor_x = area.x + 1 + conv.input.cursor as u16;
        let cursor_y = area.y + 1;
        if cursor_x < area.x + area.width - 1 {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }
}

fn render_status(frame: &mut Frame, area: Rect, app: &App) {
    let mode_str = match app.mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
    };

    let mode_color = match app.mode {
        Mode::Normal => Color::Blue,
        Mode::Insert => Color::Green,
    };

    let api_status = if app.api_key.is_some() {
        Span::styled("[API]", Style::default().fg(Color::Green))
    } else {
        Span::styled("[No Key]", Style::default().fg(Color::Red))
    };

    let help_text = match app.mode {
        Mode::Normal => "q:quit i:insert ?:help Ctrl+S:save Ctrl+L:clear",
        Mode::Insert => "Esc:normal Enter:send Ctrl+T:new Ctrl+N/P:tabs",
    };

    let mut spans = vec![
        Span::styled(
            format!(" {} ", mode_str),
            Style::default()
                .fg(Color::Black)
                .bg(mode_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        api_status,
        Span::raw(" "),
    ];

    if let Some(ref error) = app.error_message {
        spans.push(Span::styled(
            format!("Error: {} ", error),
            Style::default().fg(Color::Red),
        ));
    } else if let Some(ref status) = app.status_message {
        spans.push(Span::styled(
            format!("{} ", status),
            Style::default().fg(Color::Green),
        ));
    } else {
        spans.push(Span::styled(help_text, Style::default().fg(Color::DarkGray)));
    }

    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        format!("[{}/{}]", app.active_tab + 1, app.conversations.len()),
        Style::default().fg(Color::Yellow),
    ));

    let status = Line::from(spans);
    let status_bar = Paragraph::new(status);
    frame.render_widget(status_bar, area);
}

fn render_help(frame: &mut Frame) {
    let area = frame.area();

    // Create a centered popup
    let popup_width = 60.min(area.width - 4);
    let popup_height = 20.min(area.height - 4);
    let popup_x = (area.width - popup_width) / 2;
    let popup_y = (area.height - popup_height) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    let help_text = vec![
        Line::from(Span::styled("Keyboard Shortcuts", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(Span::styled("-- Normal Mode --", Style::default().fg(Color::Yellow))),
        Line::from("  i, a, A, I    Enter insert mode"),
        Line::from("  q, Esc        Quit"),
        Line::from("  j, k          Scroll messages up/down"),
        Line::from("  g, G          Go to top/bottom"),
        Line::from("  h, l          Move cursor in input"),
        Line::from("  x, d          Delete char / clear input"),
        Line::from("  1-9           Switch to tab N"),
        Line::from("  Tab           Next tab"),
        Line::from("  ?             Show this help"),
        Line::from(""),
        Line::from(Span::styled("-- Insert Mode --", Style::default().fg(Color::Yellow))),
        Line::from("  Esc           Return to normal mode"),
        Line::from("  Enter         Send message"),
        Line::from("  Ctrl+A/E      Go to start/end of line"),
        Line::from("  Ctrl+U        Clear input"),
        Line::from(""),
        Line::from(Span::styled("-- Global --", Style::default().fg(Color::Yellow))),
        Line::from("  Ctrl+T        New tab"),
        Line::from("  Ctrl+W        Close tab"),
        Line::from("  Ctrl+N/P      Next/prev tab"),
        Line::from("  Ctrl+S        Save conversations"),
        Line::from("  Ctrl+L        Clear current conversation"),
        Line::from(""),
        Line::from(Span::styled("Press any key to close", Style::default().fg(Color::DarkGray))),
    ];

    frame.render_widget(Clear, popup_area);

    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title(" Help ")
                .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .style(Style::default().fg(Color::White).bg(Color::Black)),
        );

    frame.render_widget(help, popup_area);
}
