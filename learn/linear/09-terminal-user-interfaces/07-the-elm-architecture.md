---
title: The Elm Architecture
description: Applying the Elm architecture (Model-View-Update) pattern to TUI applications for predictable state management, testability, and clean separation of concerns.
---

# The Elm Architecture

> **What you'll learn:**
> - The three components of TEA -- Model (application state), View (render function), Update (state transition on messages) -- and how they compose
> - Why the unidirectional data flow of TEA eliminates entire categories of UI bugs related to inconsistent state
> - How to implement TEA in Rust with enums for messages, a struct for model, and a pure render function

The Elm Architecture (TEA) is a pattern for structuring interactive applications that originated in the Elm programming language. It has since been adopted across the UI world -- React/Redux, Bubbletea, and the Ratatui community all use variations of it. For TUI applications, TEA provides a clarity of structure that is hard to achieve with ad-hoc event handling.

## The Three Components

TEA divides your application into three strictly separated concerns:

1. **Model** -- a struct that holds the entire application state. Nothing about the UI is stored anywhere else.
2. **Update** -- a function that takes the current model and a message (event) and returns a new model. This is the only way state changes.
3. **View** -- a function that takes the current model and renders the UI. It has no side effects -- it only reads state and produces output.

The data flows in one direction: events produce messages, messages flow into `update`, `update` produces a new model, and the new model flows into `view` to render the screen.

```
Events  -->  Messages  -->  update(model, msg)  -->  new model  -->  view(model)
  ^                                                                        |
  |                                                                        |
  +-----------------------------  screen output  <-------------------------+
```

This unidirectional flow means you never have to wonder "where was this state changed?" The answer is always: in the `update` function.

## Model: Your Application State

In Rust, the Model is a struct (or a set of structs) that contains everything your application needs to render and respond to events:

```rust
use std::time::Instant;

/// The complete state of our coding agent TUI
struct Model {
    // Conversation state
    messages: Vec<ChatMessage>,
    input_buffer: String,
    cursor_position: usize,

    // UI state
    scroll_offset: usize,
    active_panel: Panel,
    show_tool_output: bool,

    // Agent state
    is_streaming: bool,
    current_tool: Option<String>,
    token_count: usize,

    // Application state
    should_quit: bool,
    last_render: Instant,
}

struct ChatMessage {
    role: Role,
    content: String,
    timestamp: Instant,
}

enum Role {
    User,
    Assistant,
    System,
}

enum Panel {
    Chat,
    ToolOutput,
    FileExplorer,
}

fn main() {
    let model = Model {
        messages: Vec::new(),
        input_buffer: String::new(),
        cursor_position: 0,
        scroll_offset: 0,
        active_panel: Panel::Chat,
        show_tool_output: false,
        is_streaming: false,
        current_tool: None,
        token_count: 0,
        should_quit: false,
        last_render: Instant::now(),
    };

    println!("Model contains {} messages", model.messages.len());
    println!("Active panel: Chat");
    println!("All state lives in one place -- the Model struct.");
}
```

The key discipline: every piece of state that affects rendering or behavior lives in the Model. If you find yourself storing state in a widget or a global variable, move it to the Model.

::: tip Coming from Python
In Python Textual apps, state often lives in individual widget objects -- a `TextInput` stores its text, a `ListView` stores its items. This distributed state can lead to synchronization bugs when multiple widgets need to agree on the same data. TEA's answer is radical centralization: one struct holds everything, and widgets are stateless views of that struct. This is more like Redux in the JavaScript world than typical Python GUI patterns.
:::

## Messages: What Can Happen

Messages represent everything that can happen in your application. In Rust, an enum is the perfect fit:

```rust
use crossterm::event::{KeyEvent, MouseEvent};
use std::time::Duration;

/// Every possible event or action in the application
enum Message {
    // Input events
    KeyPress(KeyEvent),
    MouseEvent(MouseEvent),
    Resize(u16, u16),
    Paste(String),

    // Agent events
    StreamToken(String),
    StreamComplete,
    ToolStarted { name: String, id: String },
    ToolOutput { id: String, chunk: String },
    ToolFinished { id: String, exit_code: i32 },

    // UI commands
    ScrollUp(usize),
    ScrollDown(usize),
    SwitchPanel(Panel),
    ToggleToolPanel,
    SubmitInput,

    // System
    Tick(Duration),
    Quit,
}

enum Panel {
    Chat,
    ToolOutput,
}

fn main() {
    // Rust's enum exhaustiveness checking ensures you handle every
    // message type. If you add a new variant, the compiler tells you
    // every place that needs updating.

    let msg = Message::StreamToken("Hello".to_string());

    match msg {
        Message::KeyPress(_) => println!("Handle key"),
        Message::MouseEvent(_) => println!("Handle mouse"),
        Message::Resize(w, h) => println!("Resize to {}x{}", w, h),
        Message::Paste(text) => println!("Pasted: {}", text),
        Message::StreamToken(token) => println!("Token: {}", token),
        Message::StreamComplete => println!("Stream done"),
        Message::ToolStarted { name, .. } => println!("Tool: {}", name),
        Message::ToolOutput { chunk, .. } => println!("Output: {}", chunk),
        Message::ToolFinished { exit_code, .. } => println!("Exit: {}", exit_code),
        Message::ScrollUp(n) => println!("Scroll up {}", n),
        Message::ScrollDown(n) => println!("Scroll down {}", n),
        Message::SwitchPanel(p) => println!("Switch panel"),
        Message::ToggleToolPanel => println!("Toggle tool panel"),
        Message::SubmitInput => println!("Submit"),
        Message::Tick(_) => println!("Tick"),
        Message::Quit => println!("Quit"),
    }
}
```

The enum approach gives you **exhaustiveness checking** -- the compiler ensures you handle every possible message. When you add a new event type (say, `Message::FileChanged`), the compiler flags every `match` statement that needs updating.

## Update: The State Machine

The `update` function is the heart of TEA. It takes the current model and a message, and returns the updated model. Optionally, it can also return a **command** -- an action to perform that will produce future messages (like making an API call or reading a file):

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

struct Model {
    input_buffer: String,
    cursor_position: usize,
    messages: Vec<String>,
    scroll_offset: usize,
    should_quit: bool,
}

enum Message {
    KeyPress(KeyEvent),
    ScrollUp(usize),
    ScrollDown(usize),
    SubmitInput,
    Quit,
}

/// Optional side-effect to perform after update
enum Command {
    None,
    SendToLLM(String),
    Quit,
}

fn update(model: &mut Model, msg: Message) -> Command {
    match msg {
        Message::KeyPress(key) => {
            match key.code {
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    model.should_quit = true;
                    Command::Quit
                }
                KeyCode::Char(c) => {
                    model.input_buffer.insert(model.cursor_position, c);
                    model.cursor_position += 1;
                    Command::None
                }
                KeyCode::Backspace => {
                    if model.cursor_position > 0 {
                        model.cursor_position -= 1;
                        model.input_buffer.remove(model.cursor_position);
                    }
                    Command::None
                }
                KeyCode::Enter => {
                    update(model, Message::SubmitInput)
                }
                KeyCode::Left => {
                    model.cursor_position = model.cursor_position.saturating_sub(1);
                    Command::None
                }
                KeyCode::Right => {
                    if model.cursor_position < model.input_buffer.len() {
                        model.cursor_position += 1;
                    }
                    Command::None
                }
                _ => Command::None,
            }
        }

        Message::SubmitInput => {
            if !model.input_buffer.is_empty() {
                let input = model.input_buffer.clone();
                model.messages.push(format!("You: {}", input));
                model.input_buffer.clear();
                model.cursor_position = 0;
                // Scroll to bottom to show the new message
                model.scroll_offset = model.messages.len().saturating_sub(1);
                Command::SendToLLM(input)
            } else {
                Command::None
            }
        }

        Message::ScrollUp(n) => {
            model.scroll_offset = model.scroll_offset.saturating_sub(n);
            Command::None
        }

        Message::ScrollDown(n) => {
            model.scroll_offset = (model.scroll_offset + n)
                .min(model.messages.len().saturating_sub(1));
            Command::None
        }

        Message::Quit => {
            model.should_quit = true;
            Command::Quit
        }
    }
}

fn main() {
    let mut model = Model {
        input_buffer: String::new(),
        cursor_position: 0,
        messages: vec!["System: Agent ready.".to_string()],
        scroll_offset: 0,
        should_quit: false,
    };

    // Simulate some messages
    let key_h = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
    let key_i = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE);
    update(&mut model, Message::KeyPress(key_h));
    update(&mut model, Message::KeyPress(key_i));
    println!("Input buffer: '{}'", model.input_buffer);

    let cmd = update(&mut model, Message::SubmitInput);
    println!("Messages: {:?}", model.messages);
    match cmd {
        Command::SendToLLM(text) => println!("Would send to LLM: '{}'", text),
        _ => {}
    }
}
```

Notice how the `update` function is **testable in isolation**. You do not need a terminal, a backend, or any I/O to test it. You construct a Model, send it a Message, and assert on the resulting state.

## View: Rendering the State

The `view` function reads the Model and renders it to the screen. In Ratatui, this is the closure you pass to `terminal.draw()`:

```rust
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

struct Model {
    messages: Vec<String>,
    input_buffer: String,
    cursor_position: usize,
    scroll_offset: usize,
}

fn view(frame: &mut Frame, model: &Model) {
    // Split the screen: messages on top, input on bottom
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),      // Messages take remaining space
            Constraint::Length(3),    // Input is exactly 3 rows
        ])
        .split(frame.area());

    // Render message list
    let items: Vec<ListItem> = model.messages
        .iter()
        .map(|m| ListItem::new(m.as_str()))
        .collect();

    let messages_widget = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Messages"));

    frame.render_widget(messages_widget, chunks[0]);

    // Render input field
    let input_widget = Paragraph::new(model.input_buffer.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).title("Input"));

    frame.render_widget(input_widget, chunks[1]);

    // Position the cursor in the input field
    // +1 for the border on each side
    frame.set_cursor_position((
        chunks[1].x + model.cursor_position as u16 + 1,
        chunks[1].y + 1,
    ));
}

fn main() {
    let model = Model {
        messages: vec!["System: Ready.".into(), "You: Hello".into()],
        input_buffer: "typing here".into(),
        cursor_position: 11,
        scroll_offset: 0,
    };

    // In a real application, you would call:
    // terminal.draw(|frame| view(frame, &model))?;
    //
    // The view function is pure: it reads the model and renders.
    // No state mutation happens during rendering.

    println!("View function renders {} messages", model.messages.len());
    println!("Cursor at position {}", model.cursor_position);
}
```

The view function is **pure** in the sense that it does not modify the Model. It reads state and produces visual output. This makes the rendering predictable: given the same Model, you always get the same screen.

::: wild In the Wild
Go's Bubbletea enforces TEA more strictly -- the `Update` method returns a *new* model rather than mutating the existing one. In Rust, we typically mutate the model in place (`&mut Model`) because cloning large state structs every frame would be wasteful. The important discipline is the same: only `update` changes state, and `view` only reads it. OpenCode's Bubbletea-based TUI follows this pattern rigorously, with separate update and view methods on each component.
:::

## The Main Loop

TEA's three components connect in the main loop:

```rust
// The complete TEA loop (pseudocode matching our types above):

// fn main_loop() {
//     let mut model = Model::new();
//
//     loop {
//         // 1. VIEW: render current state
//         terminal.draw(|frame| view(frame, &model))?;
//
//         // 2. WAIT: for an event (key press, mouse, timer, async result)
//         let event = wait_for_event()?;
//
//         // 3. CONVERT: event to message
//         let msg = event_to_message(event);
//
//         // 4. UPDATE: produce new state and optional command
//         let cmd = update(&mut model, msg);
//
//         // 5. EXECUTE: command (may produce future messages)
//         match cmd {
//             Command::SendToLLM(text) => spawn_llm_request(text),
//             Command::Quit => break,
//             Command::None => {}
//         }
//
//         if model.should_quit {
//             break;
//         }
//     }
// }

fn main() {
    println!("TEA main loop: view -> wait -> convert -> update -> execute -> repeat");
    println!("State flows in one direction. View never mutates the model.");
    println!("Update is the single source of truth for all state changes.");
}
```

## Benefits for a Coding Agent

TEA is particularly well-suited for a coding agent TUI because:

1. **Streaming updates are just messages.** Each token from the LLM becomes a `StreamToken` message, processed by `update`, reflected by `view`. No special streaming logic in the renderer.

2. **Concurrent tool execution maps to messages.** When a tool runs in the background, its output arrives as `ToolOutput` messages. The update function processes them in order, maintaining consistency.

3. **Testability.** You can unit test the entire application logic by constructing a Model, sending Messages, and asserting on the resulting state -- no terminal needed.

4. **Debugging.** If the UI shows wrong content, you inspect the Model. If the Model is wrong, you trace which Message produced the bad state. The unidirectional flow makes the chain of causation clear.

## Key Takeaways

- The Elm Architecture separates your application into Model (state struct), Update (state transition function on messages), and View (pure rendering function), with unidirectional data flow.
- In Rust, the Model is a struct, Messages are an enum (with exhaustiveness checking), and the View is the closure passed to `terminal.draw()`.
- Only the `update` function modifies state; the `view` function only reads it. This separation eliminates inconsistent-state bugs and makes the application testable without a terminal.
- Commands returned from `update` represent side effects (API calls, file reads) that will produce future messages, keeping the update function itself free of I/O.
- TEA maps naturally to a coding agent: streaming tokens, tool output, and user input are all messages processed by the same predictable pipeline.
