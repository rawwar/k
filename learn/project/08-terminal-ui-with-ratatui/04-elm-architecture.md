---
title: Elm Architecture
description: Apply the Elm architecture pattern (Model-Update-View) to structure your terminal application with predictable state management.
---

# Elm Architecture

> **What you'll learn:**
> - How the Model-Update-View pattern separates state, logic, and rendering concerns
> - How to define a message enum that represents all possible user and system actions
> - How to implement the update function as a pure state transition for testability

The Elm architecture is a pattern for structuring interactive applications that originated in the Elm programming language and has since been adopted by frameworks across many ecosystems. It divides your application into three parts: a **Model** (your state), a **View** function (renders state to UI), and an **Update** function (transforms state in response to messages). This pattern is the backbone of your agent's terminal UI.

## The Three Parts

### Model: Your Application State

The Model is a single struct that holds every piece of state your application needs. For a coding agent, that includes the conversation history, the current input text, scroll positions, which pane is focused, and whether the agent is currently streaming a response.

```rust
/// The complete state of our TUI application.
pub struct App {
    /// The conversation messages (user and assistant turns)
    pub messages: Vec<ChatMessage>,
    /// The current text in the input box
    pub input: String,
    /// Cursor position within the input string
    pub cursor_position: usize,
    /// Scroll offset for the conversation pane
    pub scroll_offset: u16,
    /// Which pane currently has focus
    pub focus: Pane,
    /// Whether the agent is currently generating a response
    pub is_streaming: bool,
    /// Whether the application should exit
    pub should_quit: bool,
    /// Token usage for the current session
    pub token_count: usize,
    /// The name of the active model
    pub model_name: String,
}

pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

pub enum Role {
    User,
    Assistant,
}

pub enum Pane {
    Conversation,
    Input,
}
```

The critical design rule: **all mutable state lives in the Model**. If a value can change during the application's lifetime, it belongs in this struct. This makes your state easy to inspect, serialize, and test.

### View: Rendering State to UI

The View is a function that takes an immutable reference to the Model and draws the UI. It does not modify state -- it only reads it. In Ratatui terms, this is the closure you pass to `terminal.draw()`:

```rust
fn view(frame: &mut Frame, app: &App) {
    // Split the screen into conversation and input areas
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),      // conversation takes remaining space
            Constraint::Length(3),    // input box is 3 rows tall
            Constraint::Length(1),    // status bar is 1 row
        ])
        .split(frame.area());

    // Render conversation
    let conversation_text: Vec<Line> = app.messages.iter().map(|msg| {
        let prefix = match msg.role {
            Role::User => "You: ",
            Role::Assistant => "Agent: ",
        };
        Line::from(format!("{}{}", prefix, msg.content))
    }).collect();

    let conversation = Paragraph::new(conversation_text)
        .block(Block::default().title(" Conversation ").borders(Borders::ALL))
        .scroll((app.scroll_offset, 0));

    frame.render_widget(conversation, chunks[0]);

    // Render input box
    let input = Paragraph::new(app.input.as_str())
        .block(Block::default().title(" Input ").borders(Borders::ALL));

    frame.render_widget(input, chunks[1]);

    // Render status bar
    let status = Paragraph::new(format!(
        " {} | Tokens: {} | {}",
        app.model_name,
        app.token_count,
        if app.is_streaming { "Streaming..." } else { "Ready" }
    ));

    frame.render_widget(status, chunks[2]);
}
```

Because the view function is pure (no side effects, no state mutation), you can call it with any `App` state and get a predictable result. This makes your UI deterministic.

### Update: Processing Messages

The Update function takes the current state and a message, then returns the new state (or modifies the state in place). Messages represent everything that can happen in your application:

```rust
/// Every possible event that can change application state.
pub enum Message {
    // Input events
    KeyPressed(char),
    Backspace,
    Submit,
    Quit,

    // Navigation events
    ScrollUp,
    ScrollDown,
    SwitchFocus,

    // Agent events
    StreamingStarted,
    TokenReceived(String),
    StreamingCompleted(String),

    // System events
    Resize(u16, u16),
    Tick,
}
```

The update function matches on each message variant and modifies the state accordingly:

```rust
impl App {
    pub fn update(&mut self, msg: Message) {
        match msg {
            Message::KeyPressed(c) => {
                self.input.insert(self.cursor_position, c);
                self.cursor_position += 1;
            }
            Message::Backspace => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                    self.input.remove(self.cursor_position);
                }
            }
            Message::Submit => {
                if !self.input.is_empty() {
                    let user_msg = ChatMessage {
                        role: Role::User,
                        content: self.input.clone(),
                    };
                    self.messages.push(user_msg);
                    self.input.clear();
                    self.cursor_position = 0;
                    // In a real app, this would trigger an API call
                }
            }
            Message::Quit => {
                self.should_quit = true;
            }
            Message::ScrollUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            Message::ScrollDown => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
            }
            Message::SwitchFocus => {
                self.focus = match self.focus {
                    Pane::Conversation => Pane::Input,
                    Pane::Input => Pane::Conversation,
                };
            }
            Message::StreamingStarted => {
                self.is_streaming = true;
            }
            Message::TokenReceived(token) => {
                // Append token to the last assistant message
                if let Some(last) = self.messages.last_mut() {
                    if matches!(last.role, Role::Assistant) {
                        last.content.push_str(&token);
                    }
                }
            }
            Message::StreamingCompleted(full_response) => {
                self.is_streaming = false;
                self.token_count += full_response.len() / 4; // rough estimate
            }
            Message::Resize(_, _) => {
                // Ratatui handles resize automatically in the next draw call.
                // You might want to reset scroll offsets here.
            }
            Message::Tick => {
                // Used for animations like spinners -- update animation state
            }
        }
    }
}
```

::: tip Coming from Python
Python's `textual` framework uses a similar message-passing architecture. In textual, you define `on_button_pressed` or `on_key` handlers that receive message objects. The difference is that textual binds handlers to widget classes in a retained-mode DOM, while the Elm architecture routes all messages through a single update function with a single state struct. The Elm approach is simpler to reason about because there is exactly one place where state changes happen.

If you have used Redux in JavaScript, the Elm architecture is the same pattern: a store (Model), actions (Messages), and a reducer (Update).
:::

## Putting It All Together

Here is how the three parts connect in the main loop:

```rust
use crossterm::event::{self, Event, KeyCode, KeyModifiers};

fn run(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App {
        messages: vec![ChatMessage {
            role: Role::Assistant,
            content: String::from("Hello! I'm your coding agent. How can I help?"),
        }],
        input: String::new(),
        cursor_position: 0,
        scroll_offset: 0,
        focus: Pane::Input,
        is_streaming: false,
        should_quit: false,
        token_count: 0,
        model_name: String::from("claude-sonnet"),
    };

    loop {
        // VIEW: render the current state
        terminal.draw(|frame| view(frame, &app))?;

        // Check if we should exit
        if app.should_quit {
            break;
        }

        // READ: convert raw events into messages
        if event::poll(std::time::Duration::from_millis(50))? {
            let message = match event::read()? {
                Event::Key(key) => match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(Message::Quit)
                    }
                    KeyCode::Char(c) => Some(Message::KeyPressed(c)),
                    KeyCode::Backspace => Some(Message::Backspace),
                    KeyCode::Enter => Some(Message::Submit),
                    KeyCode::Up => Some(Message::ScrollUp),
                    KeyCode::Down => Some(Message::ScrollDown),
                    KeyCode::Tab => Some(Message::SwitchFocus),
                    _ => None,
                },
                Event::Resize(w, h) => Some(Message::Resize(w, h)),
                _ => None,
            };

            // UPDATE: apply the message to the state
            if let Some(msg) = message {
                app.update(msg);
            }
        }
    }

    Ok(())
}
```

Notice the clean separation. The event-reading code does not know about the UI. The view function does not know about events. The update function does not know about rendering. Each part has a single responsibility.

## Why This Pattern Matters for a Coding Agent

A coding agent is more complex than a typical TUI application because it has **external async events** -- streaming tokens from the LLM, file system changes, subprocess output. The Elm architecture handles this gracefully because everything is a message:

```rust
// In your async event loop, you might have a channel receiver
// for tokens arriving from the streaming API:
pub enum Message {
    // ... keyboard events ...

    // These arrive from a background task via a channel
    TokenReceived(String),
    StreamingCompleted(String),
    ToolCallStarted(String),
    ToolCallCompleted(String, String),
    ErrorOccurred(String),
}
```

All events -- keyboard, streaming, tool execution -- flow through the same update function. This makes your state transitions predictable and debuggable. You can even log every message for replay debugging.

## Testing the Update Function

Because the update function is a pure state transition, testing it is straightforward:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_app() -> App {
        App {
            messages: vec![],
            input: String::new(),
            cursor_position: 0,
            scroll_offset: 0,
            focus: Pane::Input,
            is_streaming: false,
            should_quit: false,
            token_count: 0,
            model_name: String::from("test-model"),
        }
    }

    #[test]
    fn test_typing_updates_input() {
        let mut app = make_app();
        app.update(Message::KeyPressed('h'));
        app.update(Message::KeyPressed('i'));
        assert_eq!(app.input, "hi");
        assert_eq!(app.cursor_position, 2);
    }

    #[test]
    fn test_submit_creates_message_and_clears_input() {
        let mut app = make_app();
        app.update(Message::KeyPressed('h'));
        app.update(Message::KeyPressed('i'));
        app.update(Message::Submit);

        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.messages[0].content, "hi");
        assert!(app.input.is_empty());
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    fn test_quit_sets_flag() {
        let mut app = make_app();
        app.update(Message::Quit);
        assert!(app.should_quit);
    }

    #[test]
    fn test_scroll_does_not_underflow() {
        let mut app = make_app();
        assert_eq!(app.scroll_offset, 0);
        app.update(Message::ScrollUp);
        assert_eq!(app.scroll_offset, 0); // saturating_sub prevents underflow
    }
}
```

No terminal needed. No UI setup. Just construct a state, send messages, and assert on the result.

::: tip In the Wild
The Go TUI framework Bubble Tea, used by OpenCode, is built entirely around the Elm architecture. Every Bubble Tea component implements `Init()`, `Update(msg)`, and `View()` methods. Ratatui does not enforce this pattern at the framework level -- it gives you a raw `draw()` closure -- but adopting it voluntarily gives you the same architectural benefits. The pattern scales well: OpenCode has dozens of components all following the same Model-Update-View cycle.
:::

## Key Takeaways

- **The Elm architecture** divides your application into Model (state), View (rendering), and Update (state transitions), keeping each concern isolated and testable.
- **All mutable state lives in the Model** -- a single struct that represents the complete application state at any point in time.
- **Messages are an enum** representing every possible event, from keypresses to streaming tokens to system notifications.
- **The update function is the single source of state changes**, making it easy to reason about, debug, and test without any UI dependencies.
- **External async events** (streaming tokens, tool execution) fit naturally into the message-passing model, making the pattern ideal for coding agents.
