---
title: Event Loop
description: Build the core event loop that polls for input events, updates application state, and triggers UI re-renders at the right cadence.
---

# Event Loop

> **What you'll learn:**
> - How to structure an async event loop using crossterm's event polling with Tokio
> - How to handle tick-based rendering versus event-driven rendering trade-offs
> - How to integrate external async events like streaming tokens into the event loop

The event loop is the heartbeat of your TUI application. It is the `loop` that runs from the moment your application starts until the user quits. Every iteration, it checks for input events, updates state, and redraws the screen. Getting this loop right determines whether your agent feels responsive or sluggish.

## The Basic Synchronous Loop

The simplest event loop blocks on input, updates state, and redraws:

```rust
use crossterm::event::{self, Event, KeyCode};
use ratatui::prelude::*;

fn run_sync(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        // 1. Draw the current state
        terminal.draw(|frame| view(frame, app))?;

        // 2. Block until an event arrives
        let event = event::read()?;

        // 3. Convert the event into a message
        if let Some(msg) = event_to_message(event) {
            // 4. Update state
            app.update(msg);
        }

        // 5. Check exit condition
        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn event_to_message(event: Event) -> Option<Message> {
    match event {
        Event::Key(key) => match key.code {
            KeyCode::Char('q') => Some(Message::Quit),
            KeyCode::Char(c) => Some(Message::KeyPressed(c)),
            KeyCode::Enter => Some(Message::Submit),
            _ => None,
        },
        Event::Resize(w, h) => Some(Message::Resize(w, h)),
        _ => None,
    }
}
```

This works, but it has a critical flaw: **it blocks**. While waiting for a keypress, the screen cannot update. If your agent is streaming a response, the tokens arrive but do not render until the user presses a key. That is unacceptable for an interactive agent.

## Polling with Timeout

The first improvement is to use `event::poll()` with a timeout instead of the blocking `event::read()`:

```rust
use std::time::Duration;
use crossterm::event::{self, Event, KeyCode};
use ratatui::prelude::*;

fn run_with_polling(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    let tick_rate = Duration::from_millis(50); // 20 FPS

    loop {
        // 1. Draw
        terminal.draw(|frame| view(frame, app))?;

        // 2. Poll for events with a timeout
        if event::poll(tick_rate)? {
            // An event is available -- read it
            let event = event::read()?;
            if let Some(msg) = event_to_message(event) {
                app.update(msg);
            }
        } else {
            // No event within the timeout -- send a Tick
            app.update(Message::Tick);
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
```

Now the loop runs at approximately 20 frames per second (every 50ms), even when no input events arrive. The `Tick` message gives your application a chance to update animations (like streaming dots or spinners) and redraw.

::: tip Coming from Python
This is similar to how `curses` applications use `timeout()` or `nodelay()`:
```python
import curses

def main(stdscr):
    stdscr.timeout(50)  # non-blocking getch with 50ms timeout
    while True:
        ch = stdscr.getch()
        if ch == -1:
            pass  # timeout, no input
        elif ch == ord('q'):
            break
        stdscr.refresh()
```
In Python's `textual`, the event loop is hidden inside the framework and you just write handlers. Ratatui gives you full control over the loop, which means more boilerplate but also more flexibility.
:::

## The Async Event Loop

For a coding agent, you need to handle events from multiple sources simultaneously:

- **Keyboard input** from the user
- **Streaming tokens** from the LLM API
- **Tool execution results** from background tasks
- **Timer ticks** for animations and UI updates

Tokio's `select!` macro is the tool for this. It waits on multiple async operations and executes whichever one completes first:

```rust
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use crossterm::event::{self, Event as CrosstermEvent, EventStream};
use futures::StreamExt;

/// Events from all sources, unified into a single enum.
pub enum AppEvent {
    /// A terminal event (key press, mouse, resize)
    Terminal(CrosstermEvent),
    /// A tick for animations and periodic updates
    Tick,
    /// A token arrived from the streaming API
    StreamToken(String),
    /// The streaming response is complete
    StreamDone(String),
    /// An error occurred in a background task
    Error(String),
}

pub async fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    mut stream_rx: mpsc::UnboundedReceiver<StreamEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut ticker = interval(Duration::from_millis(50));
    let mut reader = EventStream::new();

    loop {
        // Draw the current state
        terminal.draw(|frame| view(frame, app))?;

        if app.should_quit {
            break;
        }

        // Wait for the first event from any source
        let app_event = tokio::select! {
            // Terminal events (keyboard, mouse, resize)
            Some(Ok(event)) = reader.next() => {
                AppEvent::Terminal(event)
            }
            // Timer tick for animations
            _ = ticker.tick() => {
                AppEvent::Tick
            }
            // Streaming tokens from the LLM
            Some(stream_event) = stream_rx.recv() => {
                match stream_event {
                    StreamEvent::Token(t) => AppEvent::StreamToken(t),
                    StreamEvent::Done(full) => AppEvent::StreamDone(full),
                    StreamEvent::Error(e) => AppEvent::Error(e),
                }
            }
        };

        // Convert to a Message and update state
        let message = match app_event {
            AppEvent::Terminal(CrosstermEvent::Key(key)) => {
                event_to_message(CrosstermEvent::Key(key))
            }
            AppEvent::Terminal(CrosstermEvent::Resize(w, h)) => {
                Some(Message::Resize(w, h))
            }
            AppEvent::Terminal(_) => None,
            AppEvent::Tick => Some(Message::Tick),
            AppEvent::StreamToken(token) => Some(Message::TokenReceived(token)),
            AppEvent::StreamDone(full) => Some(Message::StreamingCompleted(full)),
            AppEvent::Error(e) => Some(Message::ErrorOccurred(e)),
        };

        if let Some(msg) = message {
            app.update(msg);
        }
    }

    Ok(())
}

pub enum StreamEvent {
    Token(String),
    Done(String),
    Error(String),
}
```

The `EventStream` type from crossterm provides an async interface to terminal events. Combined with `tokio::select!`, it lets you wait on terminal input, timer ticks, and streaming API responses simultaneously.

## Integrating with the Agent's Streaming

When the user submits a prompt, your agent spawns an async task that calls the LLM API and sends tokens through a channel. The event loop receives those tokens and feeds them into the update function:

```rust
use tokio::sync::mpsc;

impl App {
    /// Called when the user submits a prompt.
    pub fn submit_prompt(&mut self, tx: mpsc::UnboundedSender<StreamEvent>) {
        let prompt = self.input.clone();
        self.input.clear();
        self.cursor_position = 0;
        self.is_streaming = true;

        // Add the user message
        self.messages.push(ChatMessage {
            role: Role::User,
            content: prompt.clone(),
        });

        // Add a placeholder for the assistant response
        self.messages.push(ChatMessage {
            role: Role::Assistant,
            content: String::new(),
        });

        // Spawn the streaming task
        tokio::spawn(async move {
            match call_llm_streaming(&prompt).await {
                Ok(mut stream) => {
                    let mut full_response = String::new();
                    while let Some(token) = stream.next().await {
                        full_response.push_str(&token);
                        let _ = tx.send(StreamEvent::Token(token));
                    }
                    let _ = tx.send(StreamEvent::Done(full_response));
                }
                Err(e) => {
                    let _ = tx.send(StreamEvent::Error(e.to_string()));
                }
            }
        });
    }
}
```

## Rendering Cadence: When to Redraw

Drawing every 50ms (20 FPS) is fine for most interactions, but you might want to adjust the cadence based on what is happening:

```rust
use std::time::{Duration, Instant};

pub async fn adaptive_event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut last_draw = Instant::now();
    let idle_rate = Duration::from_millis(100);    // 10 FPS when idle
    let active_rate = Duration::from_millis(16);   // ~60 FPS when streaming

    loop {
        let frame_rate = if app.is_streaming {
            active_rate
        } else {
            idle_rate
        };

        // Only redraw if enough time has passed
        if last_draw.elapsed() >= frame_rate {
            terminal.draw(|frame| view(frame, app))?;
            last_draw = Instant::now();
        }

        if app.should_quit {
            break;
        }

        // Poll with the remaining time until next frame
        let remaining = frame_rate.saturating_sub(last_draw.elapsed());

        if event::poll(remaining)? {
            let event = event::read()?;
            if let Some(msg) = event_to_message(event) {
                app.update(msg);
            }
        }
    }

    Ok(())
}
```

This adaptive approach saves CPU when the agent is idle but provides smooth updates during streaming.

::: tip In the Wild
OpenCode uses Bubble Tea's built-in tick-based event loop, which fires at a fixed rate and batches events between ticks. Claude Code's Ink-based renderer similarly debounces updates to avoid overwhelming the terminal with too many redraws during fast token streaming. The pattern of reducing frame rate when idle and increasing it during active streaming is common across production agents.
:::

## Key Takeaways

- **The basic event loop** is draw-read-update-repeat, but blocking on input prevents the UI from updating during streaming -- always use polling with a timeout or async event handling.
- **`tokio::select!`** lets you wait on multiple async event sources simultaneously: keyboard input, timer ticks, and streaming API tokens.
- **`EventStream`** from crossterm provides an async interface to terminal events that integrates cleanly with Tokio's async runtime.
- **Channel-based communication** between the streaming task and the event loop (via `mpsc::UnboundedSender`) decouples the LLM API calls from the UI update cycle.
- **Adaptive frame rates** (faster during streaming, slower when idle) balance responsiveness with CPU efficiency.
