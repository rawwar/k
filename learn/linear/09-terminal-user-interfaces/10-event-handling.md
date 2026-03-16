---
title: Event Handling
description: Processing keyboard, mouse, and resize events in a TUI application with crossterm's event stream, key mapping, and async event loops.
---

# Event Handling

> **What you'll learn:**
> - How crossterm's event stream delivers keyboard, mouse, paste, and resize events as an async stream
> - Building a key mapping layer that translates raw key events into application-level actions and commands
> - Integrating event handling with the Elm architecture update function for a clean event processing pipeline

Event handling is the input side of the TUI equation. While the view function determines what users see, event handling determines how they interact. In a coding agent, you need to handle keyboard input for typing messages, key combinations for navigation and commands, mouse events for scrolling and clicking, and resize events to reflow layouts -- all while the agent might be streaming a response in the background.

## The crossterm Event System

crossterm provides a unified event system that works across platforms. Events are delivered through a blocking `event::read()` call or a polling `event::poll()` check:

```rust
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // In a real app, you would be in raw mode here.
    // This example shows the event types without raw mode setup.

    println!("crossterm event types:");

    // The Event enum covers all input types:
    let _examples: Vec<&str> = vec![
        "Event::Key(KeyEvent)       - keyboard input",
        "Event::Mouse(MouseEvent)   - mouse clicks, movement, scroll",
        "Event::Resize(cols, rows)  - terminal resize",
        "Event::Paste(String)       - bracketed paste content",
        "Event::FocusGained         - terminal window focused",
        "Event::FocusLost           - terminal window unfocused",
    ];

    for desc in &_examples {
        println!("  {}", desc);
    }

    // event::poll() checks if an event is available (non-blocking)
    // Returns true if an event is ready within the timeout
    let has_event = event::poll(Duration::from_millis(0))?;
    println!("\nEvent ready: {}", has_event);

    Ok(())
}
```

The `KeyEvent` struct contains detailed information about each keystroke:

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

fn describe_key(key: &KeyEvent) {
    println!("Key event:");
    println!("  code: {:?}", key.code);
    println!("  modifiers: {:?}", key.modifiers);
    println!("  kind: {:?}", key.kind);

    // KeyCode variants include:
    // Char('a')..Char('z')  - regular characters
    // Enter, Backspace, Tab, Esc
    // Left, Right, Up, Down - arrow keys
    // Home, End, PageUp, PageDown
    // Insert, Delete
    // F(1)..F(12) - function keys

    // KeyModifiers is a bitflag:
    // SHIFT, CONTROL, ALT, SUPER, HYPER, META

    // KeyEventKind distinguishes press, repeat, release
    // (on terminals that support it)
}

fn main() {
    // Example key events
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let shift_tab = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
    let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);

    describe_key(&ctrl_c);
    println!();
    describe_key(&enter);
    println!();
    describe_key(&shift_tab);
    println!();
    describe_key(&f5);
}
```

## The Event Loop

The event loop is where events are read and dispatched to the update function. There are several strategies for structuring it:

### Blocking Event Loop

The simplest approach blocks on `event::read()` and redraws after each event:

```rust
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

struct App {
    should_quit: bool,
    counter: u32,
    last_event: String,
}

enum Message {
    Increment,
    Decrement,
    Quit,
    Unknown(String),
}

fn event_to_message(event: &Event) -> Option<Message> {
    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            ..
        }) => Some(Message::Quit),

        Event::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }) => Some(Message::Quit),

        Event::Key(KeyEvent {
            code: KeyCode::Up,
            ..
        }) => Some(Message::Increment),

        Event::Key(KeyEvent {
            code: KeyCode::Down,
            ..
        }) => Some(Message::Decrement),

        other => Some(Message::Unknown(format!("{:?}", other))),
    }
}

fn update(app: &mut App, msg: Message) {
    match msg {
        Message::Increment => {
            app.counter += 1;
            app.last_event = "Incremented".to_string();
        }
        Message::Decrement => {
            app.counter = app.counter.saturating_sub(1);
            app.last_event = "Decremented".to_string();
        }
        Message::Quit => {
            app.should_quit = true;
        }
        Message::Unknown(desc) => {
            app.last_event = format!("Unhandled: {}", desc);
        }
    }
}

fn main() {
    let mut app = App {
        should_quit: false,
        counter: 0,
        last_event: "None".to_string(),
    };

    println!("Blocking event loop structure:");
    println!("  loop {{");
    println!("      terminal.draw(|f| view(f, &app))?;");
    println!("      let event = event::read()?;");
    println!("      if let Some(msg) = event_to_message(&event) {{");
    println!("          update(&mut app, msg);");
    println!("      }}");
    println!("      if app.should_quit {{ break; }}");
    println!("  }}");

    // Simulate some events
    update(&mut app, Message::Increment);
    update(&mut app, Message::Increment);
    update(&mut app, Message::Decrement);
    println!("\nCounter after simulated events: {}", app.counter);
}
```

### Polling Event Loop with Tick Rate

For applications that need to update the screen even without user input (like a streaming agent), use `event::poll()` with a timeout:

```rust
use crossterm::event::{self, Event};
use std::time::{Duration, Instant};

struct App {
    should_quit: bool,
    tick_count: u64,
    frame_count: u64,
}

fn main() {
    let tick_rate = Duration::from_millis(100); // 10 ticks per second

    let mut app = App {
        should_quit: false,
        tick_count: 0,
        frame_count: 0,
    };

    println!("Polling event loop with tick rate:");
    println!();
    println!("  let tick_rate = Duration::from_millis(100);");
    println!("  let mut last_tick = Instant::now();");
    println!();
    println!("  loop {{");
    println!("      // Draw the frame");
    println!("      terminal.draw(|f| view(f, &app))?;");
    println!("      app.frame_count += 1;");
    println!();
    println!("      // Calculate remaining time until next tick");
    println!("      let timeout = tick_rate");
    println!("          .checked_sub(last_tick.elapsed())");
    println!("          .unwrap_or(Duration::ZERO);");
    println!();
    println!("      // Poll for events with the remaining timeout");
    println!("      if event::poll(timeout)? {{");
    println!("          let event = event::read()?;");
    println!("          handle_event(&mut app, event);");
    println!("      }}");
    println!();
    println!("      // Process tick if enough time has passed");
    println!("      if last_tick.elapsed() >= tick_rate {{");
    println!("          app.tick_count += 1;");
    println!("          last_tick = Instant::now();");
    println!("          // Update animations, check for new stream data, etc.");
    println!("      }}");
    println!("  }}");
}
```

::: python Coming from Python
Python's Textual uses an async event loop based on `asyncio`. Events are dispatched to handler methods like `on_key`, `on_click`, and `on_mount`. If you have used Textual, Ratatui's approach is more explicit -- you write the event loop yourself and decide how to dispatch events. This gives you more control but requires more boilerplate. The `ratatui` crate does not include an event loop; you build your own using `crossterm::event`. Some Ratatui companion crates like `tui-framework` provide opinionated event loop scaffolding.
:::

## Key Mapping Layer

Raw key events are too low-level for your application logic. A key mapping layer translates them into application-level actions:

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;

/// Application-level actions
#[derive(Debug, Clone, PartialEq)]
enum Action {
    // Navigation
    ScrollUp,
    ScrollDown,
    ScrollPageUp,
    ScrollPageDown,
    ScrollToTop,
    ScrollToBottom,

    // Panel management
    NextPanel,
    PreviousPanel,
    ToggleToolPanel,

    // Input
    Submit,
    Cancel,
    NewLine,

    // Agent control
    Interrupt,
    Retry,

    // Application
    Quit,
    Help,
}

struct KeyMap {
    bindings: HashMap<(KeyCode, KeyModifiers), Action>,
}

impl KeyMap {
    fn default_bindings() -> Self {
        let mut bindings = HashMap::new();

        // Navigation
        bindings.insert(
            (KeyCode::Up, KeyModifiers::NONE),
            Action::ScrollUp,
        );
        bindings.insert(
            (KeyCode::Down, KeyModifiers::NONE),
            Action::ScrollDown,
        );
        bindings.insert(
            (KeyCode::PageUp, KeyModifiers::NONE),
            Action::ScrollPageUp,
        );
        bindings.insert(
            (KeyCode::PageDown, KeyModifiers::NONE),
            Action::ScrollPageDown,
        );
        bindings.insert(
            (KeyCode::Home, KeyModifiers::NONE),
            Action::ScrollToTop,
        );
        bindings.insert(
            (KeyCode::End, KeyModifiers::NONE),
            Action::ScrollToBottom,
        );

        // Panel management
        bindings.insert(
            (KeyCode::Tab, KeyModifiers::NONE),
            Action::NextPanel,
        );
        bindings.insert(
            (KeyCode::BackTab, KeyModifiers::SHIFT),
            Action::PreviousPanel,
        );
        bindings.insert(
            (KeyCode::Char('t'), KeyModifiers::CONTROL),
            Action::ToggleToolPanel,
        );

        // Input
        bindings.insert(
            (KeyCode::Enter, KeyModifiers::NONE),
            Action::Submit,
        );
        bindings.insert(
            (KeyCode::Esc, KeyModifiers::NONE),
            Action::Cancel,
        );
        bindings.insert(
            (KeyCode::Enter, KeyModifiers::SHIFT),
            Action::NewLine,
        );

        // Agent control
        bindings.insert(
            (KeyCode::Char('c'), KeyModifiers::CONTROL),
            Action::Interrupt,
        );
        bindings.insert(
            (KeyCode::Char('r'), KeyModifiers::CONTROL),
            Action::Retry,
        );

        // Application
        bindings.insert(
            (KeyCode::Char('q'), KeyModifiers::CONTROL),
            Action::Quit,
        );
        bindings.insert(
            (KeyCode::Char('?'), KeyModifiers::NONE),
            Action::Help,
        );

        KeyMap { bindings }
    }

    fn resolve(&self, key: &KeyEvent) -> Option<&Action> {
        self.bindings.get(&(key.code, key.modifiers))
    }
}

fn main() {
    let keymap = KeyMap::default_bindings();

    // Test some key lookups
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);

    println!("Ctrl+C -> {:?}", keymap.resolve(&ctrl_c));
    println!("Enter  -> {:?}", keymap.resolve(&enter));
    println!("Tab    -> {:?}", keymap.resolve(&tab));

    // Unknown key returns None (passed to text input handling)
    let letter = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    println!("'a'    -> {:?}", keymap.resolve(&letter));
}
```

## Input Focus and Modal Event Handling

In a multi-panel UI, the same key might mean different things depending on which panel has focus. For example, arrow keys scroll the chat when the chat panel is focused, but navigate the file list when the explorer is focused:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
enum FocusedPanel {
    Chat,
    Input,
    ToolOutput,
    FileExplorer,
}

struct App {
    focused: FocusedPanel,
    // ... other fields
}

enum AppMessage {
    ChatScroll(i32),
    ToolScroll(i32),
    FileNavigate(i32),
    TypeChar(char),
    Submit,
    SwitchFocus(FocusedPanel),
}

fn handle_key_with_focus(app: &App, code: crossterm::event::KeyCode) -> Option<AppMessage> {
    use crossterm::event::KeyCode;

    match (app.focused, code) {
        // When chat is focused, arrows scroll
        (FocusedPanel::Chat, KeyCode::Up) => Some(AppMessage::ChatScroll(-1)),
        (FocusedPanel::Chat, KeyCode::Down) => Some(AppMessage::ChatScroll(1)),

        // When input is focused, enter submits
        (FocusedPanel::Input, KeyCode::Enter) => Some(AppMessage::Submit),
        (FocusedPanel::Input, KeyCode::Char(c)) => Some(AppMessage::TypeChar(c)),

        // When tool output is focused, arrows scroll
        (FocusedPanel::ToolOutput, KeyCode::Up) => Some(AppMessage::ToolScroll(-1)),
        (FocusedPanel::ToolOutput, KeyCode::Down) => Some(AppMessage::ToolScroll(1)),

        // When file explorer is focused, arrows navigate files
        (FocusedPanel::FileExplorer, KeyCode::Up) => Some(AppMessage::FileNavigate(-1)),
        (FocusedPanel::FileExplorer, KeyCode::Down) => Some(AppMessage::FileNavigate(1)),

        // Tab always switches focus regardless of current panel
        (_, KeyCode::Tab) => {
            let next = match app.focused {
                FocusedPanel::Chat => FocusedPanel::Input,
                FocusedPanel::Input => FocusedPanel::ToolOutput,
                FocusedPanel::ToolOutput => FocusedPanel::FileExplorer,
                FocusedPanel::FileExplorer => FocusedPanel::Chat,
            };
            Some(AppMessage::SwitchFocus(next))
        }

        _ => None,
    }
}

fn main() {
    let app = App {
        focused: FocusedPanel::Input,
    };

    println!("Focused panel: {:?}", app.focused);
    println!("Same key, different context -> different action");
}
```

::: wild In the Wild
Claude Code handles input focus carefully. When the user is typing a message, keystrokes go to the input buffer. When viewing a long response, Vim-style navigation keys (j/k for scrolling, q for quit) become active. This context-dependent key handling is essential for a TUI that does not feel clunky. The key mapping layer provides the clean abstraction point between raw terminal events and application semantics.
:::

## Handling Resize Events

When the terminal is resized, crossterm delivers a `Resize` event. Since Ratatui recalculates layout every frame based on `frame.area()`, you typically do not need special resize handling -- just redraw:

```rust
use crossterm::event::{Event, KeyCode};

struct App {
    terminal_width: u16,
    terminal_height: u16,
    should_quit: bool,
}

fn handle_event(app: &mut App, event: Event) {
    match event {
        Event::Resize(width, height) => {
            app.terminal_width = width;
            app.terminal_height = height;
            // No other action needed -- the next draw() call will
            // use frame.area() which reflects the new size.
            // Layout constraints adapt automatically.
        }
        Event::Key(key) if key.code == KeyCode::Char('q') => {
            app.should_quit = true;
        }
        _ => {}
    }
}

fn main() {
    let mut app = App {
        terminal_width: 80,
        terminal_height: 24,
        should_quit: false,
    };

    handle_event(&mut app, Event::Resize(120, 40));
    println!("Terminal resized to {}x{}", app.terminal_width, app.terminal_height);
    println!("Ratatui handles resize automatically via frame.area().");
    println!("Store the size if your app logic depends on it.");
}
```

## Key Takeaways

- crossterm delivers keyboard, mouse, paste, resize, and focus events through `event::read()` (blocking) or `event::poll()` (non-blocking with timeout) in a unified, cross-platform API.
- The event loop structure -- blocking for simple apps, polling with tick rate for streaming content -- determines how responsive your TUI feels and whether background updates (like streaming) can trigger redraws.
- A key mapping layer translates raw `KeyEvent` values into application-level `Action` enum variants, decoupling terminal input from application logic and enabling configurable keybindings.
- Focus-based event routing lets the same key produce different actions depending on which panel is active, which is essential for multi-panel agent interfaces.
- Resize events require no special handling in Ratatui because `frame.area()` always reflects the current terminal size and layout constraints adapt automatically on every draw call.
