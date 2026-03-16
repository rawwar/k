// Chapter 8: Terminal UI with Ratatui — Code snapshot
//
// Demonstrates the Elm architecture (Model-Update-View) applied to a terminal
// UI built with Ratatui and crossterm.  The application has:
//   - A scrollable conversation pane (message history)
//   - A text input area at the bottom
//   - A status bar showing model name, token count, and streaming state
//   - Keyboard handling: text input, Enter to send, Ctrl+C to quit, scrolling

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

// ---------------------------------------------------------------------------
// Model — all mutable application state lives here
// ---------------------------------------------------------------------------

/// Roles for chat messages.
#[derive(Debug, Clone, PartialEq)]
enum Role {
    User,
    Assistant,
}

/// A single message in the conversation.
#[derive(Debug, Clone)]
struct ChatMessage {
    role: Role,
    content: String,
}

/// Every possible event that can change application state.
enum Message {
    // Text editing
    KeyPressed(char),
    Backspace,
    Submit,

    // Navigation / scrolling
    ScrollUp,
    ScrollDown,

    // Application lifecycle
    Quit,
    Tick,
}

/// Complete application state (the Elm "Model").
struct App {
    /// Whether the application should exit on the next loop iteration.
    should_quit: bool,
    /// Conversation history (user and assistant turns).
    messages: Vec<ChatMessage>,
    /// Current text in the input box.
    input: String,
    /// Cursor byte-offset within `input`.
    cursor_position: usize,
    /// Vertical scroll offset for the conversation pane.
    scroll_offset: u16,
    /// Whether the agent is currently streaming a response.
    is_streaming: bool,
    /// Cumulative token count for the session.
    token_count: usize,
    /// Name of the active model.
    model_name: String,
}

impl App {
    fn new() -> Self {
        Self {
            should_quit: false,
            messages: vec![ChatMessage {
                role: Role::Assistant,
                content: String::from(
                    "Hello! I'm your coding agent. How can I help?",
                ),
            }],
            input: String::new(),
            cursor_position: 0,
            scroll_offset: 0,
            is_streaming: false,
            token_count: 0,
            model_name: String::from("claude-sonnet-4"),
        }
    }

    // --------------------------------------------------------------------
    // Update — process a single message, mutating the model
    // --------------------------------------------------------------------

    fn update(&mut self, msg: Message) {
        match msg {
            Message::KeyPressed(c) => {
                self.input.insert(self.cursor_position, c);
                self.cursor_position += c.len_utf8();
            }
            Message::Backspace => {
                if self.cursor_position > 0 {
                    // Walk back to the previous character boundary.
                    let prev = self.input[..self.cursor_position]
                        .char_indices()
                        .last()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.input.drain(prev..self.cursor_position);
                    self.cursor_position = prev;
                }
            }
            Message::Submit => {
                let text = self.input.trim().to_string();
                if !text.is_empty() {
                    // Record the user message.
                    self.messages.push(ChatMessage {
                        role: Role::User,
                        content: text.clone(),
                    });

                    // Simulate an assistant reply (a real agent would call the
                    // API here and stream tokens back via a channel).
                    self.messages.push(ChatMessage {
                        role: Role::Assistant,
                        content: format!(
                            "I received your message: \"{}\". \
                             (This is a placeholder — in a full agent the \
                             response would stream from the API.)",
                            text
                        ),
                    });

                    // Rough token estimate for the status bar.
                    self.token_count += text.len() / 4 + 1;

                    // Clear input.
                    self.input.clear();
                    self.cursor_position = 0;

                    // Auto-scroll to the bottom so the new messages are visible.
                    self.scroll_offset = u16::MAX;
                }
            }
            Message::ScrollUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            Message::ScrollDown => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
            }
            Message::Quit => {
                self.should_quit = true;
            }
            Message::Tick => {
                // Reserved for animations (spinners, streaming dots).
            }
        }
    }
}

// ---------------------------------------------------------------------------
// View — render the current state into a Frame (pure, no side-effects)
// ---------------------------------------------------------------------------

fn view(frame: &mut Frame, app: &App) {
    // Three-region vertical layout: conversation | input box | status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // conversation pane — takes remaining space
            Constraint::Length(3), // input box — fixed height
            Constraint::Length(1), // status bar — single row
        ])
        .split(frame.area());

    render_conversation(frame, app, chunks[0]);
    render_input(frame, app, chunks[1]);
    render_status_bar(frame, app, chunks[2]);
}

/// Render the scrollable conversation pane.
fn render_conversation(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let mut lines: Vec<Line> = Vec::new();

    for msg in &app.messages {
        let (prefix, color) = match msg.role {
            Role::User => ("You", Color::Blue),
            Role::Assistant => ("Agent", Color::Green),
        };

        // Role label on its own line.
        lines.push(Line::from(Span::styled(
            format!("{}: ", prefix),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )));

        // Message body — each line indented by two spaces.
        for line in msg.content.lines() {
            lines.push(Line::from(format!("  {}", line)));
        }

        // Blank separator between messages.
        lines.push(Line::from(""));
    }

    // If the agent is streaming, show a placeholder indicator.
    if app.is_streaming {
        lines.push(Line::from(Span::styled(
            "  ...",
            Style::default().fg(Color::DarkGray),
        )));
    }

    // Clamp the scroll offset so it does not exceed the content length.
    let inner_height = area.height.saturating_sub(2); // account for borders
    let max_scroll = (lines.len() as u16).saturating_sub(inner_height);
    let offset = app.scroll_offset.min(max_scroll);

    let block = Block::default()
        .title(" Conversation ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((offset, 0));

    frame.render_widget(paragraph, area);
}

/// Render the text input area with a visible cursor.
fn render_input(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let display_text = if app.input.is_empty() {
        "Type a message and press Enter to send..."
    } else {
        app.input.as_str()
    };

    let style = if app.input.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };

    let block = Block::default()
        .title(" Input (Enter to send) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let paragraph = Paragraph::new(display_text).block(block).style(style);

    frame.render_widget(paragraph, area);

    // Position the blinking cursor inside the input box.
    // +1 for the left border on each axis.
    frame.set_cursor_position((
        area.x + app.cursor_position as u16 + 1,
        area.y + 1,
    ));
}

/// Render the single-row status bar.
fn render_status_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let sections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .split(area);

    let bg = Style::default().bg(Color::DarkGray).fg(Color::White);

    // Left: model name
    let left = Paragraph::new(Line::from(vec![
        Span::styled(" ", bg),
        Span::styled(
            app.model_name.as_str(),
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .style(bg);
    frame.render_widget(left, sections[0]);

    // Center: streaming status
    let status_text = if app.is_streaming {
        "Streaming..."
    } else {
        "Ready"
    };
    let center = Paragraph::new(Span::styled(status_text, bg))
        .style(bg)
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(center, sections[1]);

    // Right: token count
    let token_display = if app.token_count >= 1000 {
        format!("tokens: {:.1}K ", app.token_count as f64 / 1000.0)
    } else {
        format!("tokens: {} ", app.token_count)
    };
    let right = Paragraph::new(Span::styled(token_display, bg))
        .style(bg)
        .alignment(ratatui::layout::Alignment::Right);
    frame.render_widget(right, sections[2]);
}

// ---------------------------------------------------------------------------
// Event translation — convert raw crossterm events into our Message enum
// ---------------------------------------------------------------------------

fn event_to_message(event: Event) -> Option<Message> {
    match event {
        Event::Key(key) => {
            // Only handle key-press events (not release / repeat).
            if key.kind != KeyEventKind::Press {
                return None;
            }

            // Global shortcuts first.
            if key.code == KeyCode::Char('c')
                && key.modifiers.contains(KeyModifiers::CONTROL)
            {
                return Some(Message::Quit);
            }

            match key.code {
                KeyCode::Char(c) => Some(Message::KeyPressed(c)),
                KeyCode::Backspace => Some(Message::Backspace),
                KeyCode::Enter => Some(Message::Submit),
                KeyCode::Up => Some(Message::ScrollUp),
                KeyCode::Down => Some(Message::ScrollDown),
                KeyCode::Esc => Some(Message::Quit),
                _ => None,
            }
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Main — setup, event loop, teardown
// ---------------------------------------------------------------------------

fn main() -> io::Result<()> {
    // --- Terminal setup ---
    enable_raw_mode()?;
    let mut terminal = ratatui::init();
    let mut app = App::new();

    // --- Event loop (poll with timeout for tick-based refresh) ---
    let tick_rate = Duration::from_millis(50); // ~20 FPS

    while !app.should_quit {
        // VIEW: draw the current state.
        terminal.draw(|frame| view(frame, &app))?;

        // READ: poll for events (non-blocking with timeout).
        if event::poll(tick_rate)? {
            let ev = event::read()?;
            if let Some(msg) = event_to_message(ev) {
                // UPDATE: apply the message to the model.
                app.update(msg);
            }
        } else {
            // No event within the tick window — send a Tick for animations.
            app.update(Message::Tick);
        }
    }

    // --- Terminal teardown ---
    disable_raw_mode()?;
    ratatui::restore();
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests — the Elm architecture makes state logic trivially testable
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_app() -> App {
        App {
            should_quit: false,
            messages: vec![],
            input: String::new(),
            cursor_position: 0,
            scroll_offset: 0,
            is_streaming: false,
            token_count: 0,
            model_name: String::from("test-model"),
        }
    }

    #[test]
    fn typing_updates_input_buffer() {
        let mut app = make_app();
        app.update(Message::KeyPressed('h'));
        app.update(Message::KeyPressed('i'));
        assert_eq!(app.input, "hi");
        assert_eq!(app.cursor_position, 2);
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut app = make_app();
        app.update(Message::KeyPressed('a'));
        app.update(Message::KeyPressed('b'));
        app.update(Message::Backspace);
        assert_eq!(app.input, "a");
        assert_eq!(app.cursor_position, 1);
    }

    #[test]
    fn backspace_on_empty_input_is_safe() {
        let mut app = make_app();
        app.update(Message::Backspace);
        assert_eq!(app.input, "");
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    fn submit_creates_user_and_assistant_messages() {
        let mut app = make_app();
        app.update(Message::KeyPressed('h'));
        app.update(Message::KeyPressed('i'));
        app.update(Message::Submit);

        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages[0].role, Role::User);
        assert_eq!(app.messages[0].content, "hi");
        assert_eq!(app.messages[1].role, Role::Assistant);
        assert!(app.input.is_empty());
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    fn submit_empty_input_does_nothing() {
        let mut app = make_app();
        app.update(Message::Submit);
        assert!(app.messages.is_empty());
    }

    #[test]
    fn quit_sets_flag() {
        let mut app = make_app();
        app.update(Message::Quit);
        assert!(app.should_quit);
    }

    #[test]
    fn scroll_up_does_not_underflow() {
        let mut app = make_app();
        assert_eq!(app.scroll_offset, 0);
        app.update(Message::ScrollUp);
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn scroll_down_increments_offset() {
        let mut app = make_app();
        app.update(Message::ScrollDown);
        assert_eq!(app.scroll_offset, 1);
    }

    #[test]
    fn token_count_increases_on_submit() {
        let mut app = make_app();
        app.update(Message::KeyPressed('x'));
        app.update(Message::Submit);
        assert!(app.token_count > 0);
    }
}
