use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Tabs, Wrap},
    Frame,
};

use crate::app::{App, Mode};
use crate::conversation::Role;

pub fn render(app: &App, frame: &mut Frame) {
    let area = frame.area();

    // Outer border
    let outer_block = Block::default()
        .title(" claude-tui ")
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .style(Style::default().fg(Color::Cyan));

    let inner_area = outer_block.inner(area);
    frame.render_widget(outer_block, area);

    // Split inner area: tabs, messages, input, status
    let chunks = Layout::vertical([
        Constraint::Length(1), // Tab bar
        Constraint::Min(1),    // Messages area
        Constraint::Length(3), // Input area
        Constraint::Length(1), // Status bar
    ])
    .split(inner_area);

    // Render tabs
    render_tabs(app, frame, chunks[0]);

    // Render messages
    render_messages(app, frame, chunks[1]);

    // Input area
    let input_border_color = match app.mode {
        Mode::Insert => Color::Blue,
        Mode::Normal | Mode::Help => Color::Gray,
    };

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(input_border_color));

    let input_text = format!("> {}", app.input);
    let input_paragraph = Paragraph::new(input_text).block(input_block);
    frame.render_widget(input_paragraph, chunks[2]);

    // Show cursor in insert mode
    if app.mode == Mode::Insert {
        let cursor_x = chunks[2].x + 3 + app.cursor_position as u16;
        let cursor_y = chunks[2].y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }

    // Status bar
    render_status_bar(app, frame, chunks[3]);

    // Help overlay
    if app.mode == Mode::Help {
        render_help_overlay(frame, area);
    }
}

fn render_status_bar(app: &App, frame: &mut Frame, area: Rect) {
    let msg_count = app.current_conversation().messages.len();
    let loading_indicator = if app.is_loading { " (thinking...)" } else { "" };
    let api_warning = if !app.has_api_key() {
        " ⚠ ANTHROPIC_API_KEY not set"
    } else {
        ""
    };

    let status_text = match app.mode {
        Mode::Normal => format!(
            "NORMAL | {} msgs | i insert  j/k scroll  ^n new  ^w close  ^s save  ? help  q quit{}{}",
            msg_count, loading_indicator, api_warning
        ),
        Mode::Insert => format!(
            "INSERT | Esc → normal  Enter → send{}{}",
            loading_indicator, api_warning
        ),
        Mode::Help => "HELP | Press any key to close".to_string(),
    };

    // Show status message, error, or default
    let (display_text, status_color) = if let Some(ref error) = app.error_message {
        (format!("Error: {}", error), Color::Red)
    } else if let Some(ref status) = app.status_message {
        (status.clone(), Color::Green)
    } else {
        (status_text, Color::DarkGray)
    };

    let status = Paragraph::new(Line::from(vec![Span::styled(
        display_text,
        Style::default().fg(status_color),
    )]));
    frame.render_widget(status, area);
}

fn render_help_overlay(frame: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(""),
        Line::from(Span::styled("  Normal Mode", Style::default().add_modifier(Modifier::BOLD))),
        Line::from("  ───────────"),
        Line::from("  i, Enter       Insert mode"),
        Line::from("  q              Quit"),
        Line::from("  j, k, ↑, ↓     Scroll messages"),
        Line::from("  g, G           Top/bottom of chat"),
        Line::from("  Ctrl+n         New conversation"),
        Line::from("  Ctrl+w         Close conversation"),
        Line::from("  Ctrl+h/l       Previous/next tab"),
        Line::from("  Ctrl+s         Save conversation"),
        Line::from("  ?              Toggle this help"),
        Line::from(""),
        Line::from(Span::styled("  Insert Mode", Style::default().add_modifier(Modifier::BOLD))),
        Line::from("  ───────────"),
        Line::from("  Escape         Normal mode"),
        Line::from("  Enter          Send message"),
        Line::from("  ←/→            Move cursor"),
        Line::from("  Backspace      Delete character"),
        Line::from(""),
        Line::from(Span::styled("  Commands", Style::default().add_modifier(Modifier::BOLD))),
        Line::from("  ────────"),
        Line::from("  /model         Show current model"),
        Line::from("  /model <name>  Switch model"),
        Line::from("  /help          Show this help"),
        Line::from(""),
        Line::from(Span::styled("        Press any key to close", Style::default().fg(Color::DarkGray))),
        Line::from(""),
    ];

    let help_height = help_text.len() as u16 + 2;
    let help_width = 50;

    let popup_area = centered_rect(help_width, help_height, area);

    let block = Block::default()
        .title(" Keybindings ")
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let help = Paragraph::new(help_text).block(block);

    frame.render_widget(Clear, popup_area);
    frame.render_widget(help, popup_area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let [area] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    area
}

fn render_tabs(app: &App, frame: &mut Frame, area: Rect) {
    let titles: Vec<Line> = app
        .conversations
        .iter()
        .enumerate()
        .map(|(i, conv)| {
            let title = conv.display_title();
            let style = if i == app.active_tab {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Line::from(Span::styled(title, style))
        })
        .collect();

    let tabs = Tabs::new(titles)
        .select(app.active_tab)
        .divider(Span::raw(" │ "));

    frame.render_widget(tabs, area);
}

fn render_messages(app: &App, frame: &mut Frame, area: Rect) {
    let conversation = app.current_conversation();

    if conversation.messages.is_empty() {
        let hint = Paragraph::new("Start typing to begin a conversation.\nPress 'i' to enter insert mode, '?' for help.")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, area);
        return;
    }

    let max_bubble_width = (area.width as f32 * 0.7) as u16;
    let mut y_offset = area.y;

    // Calculate visible messages based on scroll offset
    let visible_messages = conversation
        .messages
        .iter()
        .skip(conversation.scroll_offset);

    for message in visible_messages {
        if y_offset >= area.y + area.height {
            break;
        }

        let (border_color, alignment) = match message.role {
            Role::User => (Color::Blue, Alignment::Right),
            Role::Assistant => (Color::Green, Alignment::Left),
        };

        // Wrap text for bubble
        let content_lines = wrap_text(&message.content, max_bubble_width.saturating_sub(4) as usize);
        let bubble_height = content_lines.len() as u16 + 2; // +2 for borders

        if y_offset + bubble_height + 1 > area.y + area.height {
            break;
        }

        // Calculate bubble position
        let bubble_width = content_lines
            .iter()
            .map(|l| l.len())
            .max()
            .unwrap_or(0)
            .min(max_bubble_width as usize - 2) as u16
            + 4; // padding

        let bubble_x = match alignment {
            Alignment::Right => area.x + area.width - bubble_width - 1,
            Alignment::Left => area.x + 1,
            _ => area.x,
        };

        let bubble_rect = Rect::new(bubble_x, y_offset, bubble_width, bubble_height);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(border_color));

        let text_lines: Vec<Line> = content_lines.into_iter().map(Line::from).collect();
        let paragraph = Paragraph::new(text_lines).block(block).wrap(Wrap { trim: false });

        frame.render_widget(paragraph, bubble_rect);

        // Timestamp below bubble
        let timestamp = message.timestamp.format("%H:%M").to_string();
        let timestamp_x = match alignment {
            Alignment::Right => bubble_x + bubble_width - timestamp.len() as u16,
            _ => bubble_x,
        };

        let timestamp_span = Span::styled(timestamp, Style::default().fg(Color::DarkGray).dim());
        let timestamp_rect = Rect::new(timestamp_x, y_offset + bubble_height, timestamp_span.width() as u16, 1);
        frame.render_widget(Paragraph::new(Line::from(timestamp_span)), timestamp_rect);

        y_offset += bubble_height + 2; // bubble + timestamp + spacing
    }
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current_line = String::new();
        for word in line.split_whitespace() {
            if current_line.is_empty() {
                if word.len() > max_width {
                    // Word is too long, split it
                    let mut remaining = word;
                    while remaining.len() > max_width {
                        lines.push(remaining[..max_width].to_string());
                        remaining = &remaining[max_width..];
                    }
                    current_line = remaining.to_string();
                } else {
                    current_line = word.to_string();
                }
            } else if current_line.len() + 1 + word.len() <= max_width {
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
