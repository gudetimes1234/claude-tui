mod api;
mod app;
mod conversation;
mod storage;
mod ui;

use std::io::{self, stdout};
use std::panic;
use std::sync::Arc;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;

use api::{ApiClient, StreamChunk};
use app::{App, Mode};
use conversation::Message;

enum AppEvent {
    Key(crossterm::event::KeyEvent),
    StreamChunk(String),
    StreamDone,
    StreamError(String),
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set up panic hook to restore terminal on panic
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal();
        original_hook(panic_info);
    }));

    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the app
    let mut app = App::new();
    let result = run(&mut terminal, &mut app).await;

    // Restore terminal
    restore_terminal()?;

    result
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

async fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    let (tx, mut rx) = mpsc::channel::<AppEvent>(32);

    // Clone API client for async tasks
    let api_client: Option<Arc<ApiClient>> = app.api_client.take().map(Arc::new);

    // Spawn keyboard event reader
    let tx_keys = tx.clone();
    tokio::spawn(async move {
        loop {
            if event::poll(std::time::Duration::from_millis(50)).unwrap_or(false) {
                if let Ok(Event::Key(key)) = event::read() {
                    if tx_keys.send(AppEvent::Key(key)).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    loop {
        terminal.draw(|frame| {
            ui::render(app, frame);
        })?;

        if app.should_quit {
            return Ok(());
        }

        // Wait for events with timeout for responsive UI
        match tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv()).await {
            Ok(Some(event)) => match event {
                AppEvent::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    match app.mode {
                        Mode::Help => {
                            // Any key closes help
                            app.mode = Mode::Normal;
                        }
                        Mode::Normal => handle_normal_mode(app, key.code, key.modifiers),
                        Mode::Insert => {
                            if let Some(should_send) =
                                handle_insert_mode(app, key.code, &api_client)
                            {
                                if should_send {
                                    // Send API request with streaming
                                    if let Some(ref client) = api_client {
                                        // Apply pending model change
                                        if let Some(new_model) = app.pending_model_change.take() {
                                            app.current_model = Some(new_model);
                                        }

                                        app.is_loading = true;
                                        app.start_assistant_message();

                                        let client = Arc::clone(client);
                                        let conv = app.current_conversation();
                                        let messages: Vec<Message> =
                                            conv.messages[..conv.messages.len() - 1].to_vec();
                                        let tx = tx.clone();
                                        let model = app.current_model.clone();

                                        // Create a channel for stream chunks
                                        let (stream_tx, mut stream_rx) = mpsc::channel::<StreamChunk>(32);

                                        // Spawn the streaming request
                                        tokio::spawn(async move {
                                            let _ = client.send_message_streaming(&messages, None, model.as_deref(), stream_tx).await;
                                        });

                                        // Spawn a task to forward stream chunks to main event loop
                                        let tx_stream = tx.clone();
                                        tokio::spawn(async move {
                                            while let Some(chunk) = stream_rx.recv().await {
                                                let event = match chunk {
                                                    StreamChunk::Text(text) => AppEvent::StreamChunk(text),
                                                    StreamChunk::Done => AppEvent::StreamDone,
                                                    StreamChunk::Error(e) => AppEvent::StreamError(e),
                                                };
                                                if tx_stream.send(event).await.is_err() {
                                                    break;
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                AppEvent::StreamChunk(text) => {
                    app.append_to_last_message(&text);
                }
                AppEvent::StreamDone => {
                    app.finish_streaming();
                }
                AppEvent::StreamError(error) => {
                    app.set_error(error);
                }
            },
            Ok(None) => break, // Channel closed
            Err(_) => {}       // Timeout, continue
        }
    }

    Ok(())
}

fn handle_normal_mode(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    app.clear_error();

    // Handle Ctrl+ keybindings
    if modifiers.contains(KeyModifiers::CONTROL) {
        match code {
            KeyCode::Char('n') => app.new_conversation(),
            KeyCode::Char('w') => app.close_current_conversation(),
            KeyCode::Char('h') | KeyCode::Left => app.prev_tab(),
            KeyCode::Char('l') | KeyCode::Right => app.next_tab(),
            KeyCode::Char('s') => app.save_current_conversation(),
            _ => {}
        }
        return;
    }

    match code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('i') | KeyCode::Enter => app.mode = Mode::Insert,
        KeyCode::Char('j') | KeyCode::Down => app.current_conversation_mut().scroll_down(10),
        KeyCode::Char('k') | KeyCode::Up => app.current_conversation_mut().scroll_up(),
        KeyCode::Char('g') => app.current_conversation_mut().scroll_to_top(),
        KeyCode::Char('G') => app.current_conversation_mut().scroll_to_bottom(10),
        KeyCode::Char('?') => app.toggle_help(),
        _ => {}
    }
}

fn handle_insert_mode(
    app: &mut App,
    code: KeyCode,
    api_client: &Option<Arc<ApiClient>>,
) -> Option<bool> {
    match code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            None
        }
        KeyCode::Enter => {
            if app.is_loading {
                return None;
            }

            if api_client.is_none() {
                app.set_error("ANTHROPIC_API_KEY not set".to_string());
                return None;
            }

            if app.submit().is_some() {
                Some(true) // Signal to send API request
            } else {
                None
            }
        }
        KeyCode::Backspace => {
            app.delete_char();
            None
        }
        KeyCode::Left => {
            app.move_cursor_left();
            None
        }
        KeyCode::Right => {
            app.move_cursor_right();
            None
        }
        KeyCode::Char(c) => {
            app.insert_char(c);
            None
        }
        _ => None,
    }
}
