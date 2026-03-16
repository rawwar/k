---
title: Real Time UI Updates
description: Integrate streaming data with the terminal UI layer to provide smooth, flicker-free real-time content updates.
---

# Real Time UI Updates

> **What you'll learn:**
> - How to decouple the stream processing thread from the UI rendering thread
> - How to batch UI updates at the display refresh rate to avoid excessive redraws
> - How to handle scroll position and viewport management as new content streams in

So far you have been printing tokens directly to stdout as they arrive. That works for a simple CLI, but as your agent grows -- with progress bars, tool indicators, status lines, and eventually a full TUI (Chapter 8) -- you need a proper architecture for real-time UI updates. This subchapter introduces the patterns that bridge streaming data and terminal rendering.

## The problem with direct printing

When you call `print!("{}", token)` from the stream processing loop, you are coupling three concerns:

1. **Stream processing** -- parsing SSE events, assembling tool calls.
2. **Data accumulation** -- building the conversation history.
3. **Rendering** -- writing to the terminal.

This tight coupling causes problems as complexity grows:

- **Thread safety:** If you want to show a spinner while also rendering tokens, two tasks need to write to the terminal. Without coordination, their output interleaves.
- **Redrawing:** When a tool call completes and you want to update a status line, you cannot easily "go back" and modify already-printed text.
- **Testing:** You cannot unit-test your stream processing logic without it writing to a real terminal.

## The event bus pattern

The solution is to separate stream processing from rendering with an event bus. The stream processor emits structured events; a dedicated renderer consumes them:

```rust
use tokio::sync::mpsc;
use std::time::Duration;

/// UI events that the renderer understands.
#[derive(Debug, Clone)]
pub enum UiEvent {
    /// Append text to the response area.
    AppendText(String),
    /// Show a spinner with the given message.
    ShowSpinner(String),
    /// Hide the spinner.
    HideSpinner,
    /// Update the status bar with streaming metrics.
    UpdateStatus {
        tokens: u32,
        tokens_per_second: f64,
        elapsed: Duration,
    },
    /// Show that a tool call is in progress.
    ToolStarted { name: String },
    /// Show that a tool call completed.
    ToolCompleted {
        name: String,
        success: bool,
        duration: Duration,
    },
    /// A new assistant turn is starting.
    TurnStarted,
    /// The current turn is complete.
    TurnComplete,
    /// An error occurred.
    ShowError(String),
}

/// Creates the UI event channel.
pub fn create_ui_channel() -> (mpsc::UnboundedSender<UiEvent>, mpsc::UnboundedReceiver<UiEvent>) {
    mpsc::unbounded_channel()
}
```

Note the use of an unbounded channel here. For UI events, backpressure is handled differently than for stream data -- you never want the stream processor to block on sending a UI event. Instead, the renderer coalesces events, as you will see next.

## The frame-based renderer

Instead of rendering every event immediately, batch them into frames. A frame is a collection of UI updates applied at once at a fixed rate (typically 30-60 FPS for a terminal):

```rust
use std::io::{self, Write};
use std::time::{Duration, Instant};

/// Renders UI events to the terminal at a fixed frame rate.
pub struct FrameRenderer {
    /// Pending text to append (accumulated between frames).
    pending_text: String,
    /// Current status bar content.
    status_bar: Option<String>,
    /// Current spinner state.
    spinner_message: Option<String>,
    spinner_frame: usize,
    /// Minimum time between renders.
    frame_interval: Duration,
    /// When the last frame was rendered.
    last_frame: Instant,
    /// Total lines of response text rendered.
    lines_rendered: usize,
}

impl FrameRenderer {
    pub fn new(fps: u32) -> Self {
        Self {
            pending_text: String::new(),
            status_bar: None,
            spinner_message: None,
            spinner_frame: 0,
            frame_interval: Duration::from_millis(1000 / fps as u64),
            last_frame: Instant::now(),
            lines_rendered: 0,
        }
    }

    /// Process a UI event. Does not render immediately.
    pub fn handle_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::AppendText(text) => {
                self.pending_text.push_str(&text);
            }
            UiEvent::ShowSpinner(message) => {
                self.spinner_message = Some(message);
            }
            UiEvent::HideSpinner => {
                self.spinner_message = None;
            }
            UiEvent::UpdateStatus {
                tokens,
                tokens_per_second,
                elapsed,
            } => {
                self.status_bar = Some(format!(
                    "{} tokens | {:.1} tok/s | {:.1}s",
                    tokens,
                    tokens_per_second,
                    elapsed.as_secs_f64()
                ));
            }
            UiEvent::ToolStarted { name } => {
                self.pending_text
                    .push_str(&format!("\n\x1b[36m> Running: {}\x1b[0m\n", name));
            }
            UiEvent::ToolCompleted {
                name,
                success,
                duration,
            } => {
                let icon = if success { "+" } else { "!" };
                self.pending_text.push_str(&format!(
                    "\x1b[36m  {} {} ({:.1}s)\x1b[0m\n",
                    icon,
                    name,
                    duration.as_secs_f64()
                ));
            }
            UiEvent::TurnStarted | UiEvent::TurnComplete => {}
            UiEvent::ShowError(msg) => {
                self.pending_text
                    .push_str(&format!("\n\x1b[31m[Error: {}]\x1b[0m\n", msg));
            }
        }
    }

    /// Check if it is time to render a frame.
    pub fn should_render(&self) -> bool {
        self.last_frame.elapsed() >= self.frame_interval
            || (!self.pending_text.is_empty()
                && self.last_frame.elapsed() >= Duration::from_millis(16))
    }

    /// Render the current frame to the terminal.
    pub fn render_frame(&mut self) -> io::Result<()> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        // Render any pending text
        if !self.pending_text.is_empty() {
            let newlines = self.pending_text.chars().filter(|c| *c == '\n').count();
            self.lines_rendered += newlines;

            write!(handle, "{}", self.pending_text)?;
            self.pending_text.clear();
        }

        // Render spinner if active (on stderr to avoid mixing)
        if let Some(message) = &self.spinner_message {
            let frames = ['|', '/', '-', '\\'];
            let frame_char = frames[self.spinner_frame % frames.len()];
            self.spinner_frame += 1;

            eprint!("\r{} {}", frame_char, message);
            io::stderr().flush()?;
        }

        handle.flush()?;
        self.last_frame = Instant::now();

        Ok(())
    }
}
```

## The render loop

Run the renderer as an async task that processes events and renders frames:

```rust
pub async fn run_render_loop(mut rx: mpsc::UnboundedReceiver<UiEvent>) {
    let mut renderer = FrameRenderer::new(30); // 30 FPS

    loop {
        // Drain all available events without blocking
        loop {
            match rx.try_recv() {
                Ok(event) => renderer.handle_event(event),
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Channel closed, render final frame and exit
                    renderer.render_frame().ok();
                    return;
                }
            }
        }

        // Render a frame if it is time
        if renderer.should_render() {
            renderer.render_frame().ok();
        }

        // Wait a short interval before checking for more events.
        // This is the frame rate limiter.
        tokio::time::sleep(Duration::from_millis(8)).await;

        // Also check for events that arrived while we were sleeping
        // (with a brief non-blocking drain)
        while let Ok(event) = rx.try_recv() {
            renderer.handle_event(event);
        }
    }
}
```

The render loop follows a standard game-loop pattern:

1. **Drain events** -- process all queued UI events without blocking.
2. **Render** -- if enough time has passed since the last frame, render.
3. **Sleep** -- yield to the runtime for a short interval.

This batching means that if 10 tokens arrive in a single 33ms frame window, they are all rendered in one `write!()` call instead of 10 separate calls. This eliminates flicker and reduces system call overhead.

::: python Coming from Python
Python's `rich` library uses a similar frame-based approach internally. Its `Live` display updates at a configurable refresh rate:
```python
from rich.live import Live
from rich.text import Text

with Live(refresh_per_second=30) as live:
    accumulated = ""
    for token in stream.text_stream:
        accumulated += token
        live.update(Text(accumulated))
```
The Rust version gives you explicit control over the render loop, which becomes important in Chapter 8 when you build a full TUI with Ratatui. The frame-based pattern you learn here translates directly to Ratatui's event loop.
:::

## Integrating with the stream processor

Wire the UI event bus into the streaming pipeline:

```rust
pub async fn stream_with_ui(
    client: &reqwest::Client,
    api_key: &str,
    messages: &[serde_json::Value],
    cancel_token: CancellationToken,
) -> Result<StreamOutput, Box<dyn std::error::Error>> {
    let (ui_tx, ui_rx) = create_ui_channel();

    // Spawn the render loop
    let render_handle = tokio::spawn(run_render_loop(ui_rx));

    // Signal the UI to show a spinner
    ui_tx.send(UiEvent::ShowSpinner("Thinking...".to_string())).ok();

    let byte_stream = start_streaming_request(client, api_key, messages).await?;
    let mut splitter = LineSplitter::new();
    let mut parser = SseParser::new();
    let mut sm = StreamStateMachine::new();
    let mut first_token = true;
    let start = Instant::now();
    let mut token_count = 0u32;

    loop {
        let chunk = tokio::select! {
            chunk = futures::StreamExt::next(&mut byte_stream) => {
                match chunk {
                    Some(Ok(b)) => b,
                    Some(Err(e)) => { sm.network_error(e.to_string()); break; }
                    None => break,
                }
            }
            _ = cancel_token.cancelled() => { sm.interrupt(); break; }
        };

        for line in splitter.feed(&chunk) {
            let Some(sse_event) = parser.feed_line(&line) else { continue };
            if sse_event.event_type == "ping" { continue; }
            let stream_event: StreamEvent = serde_json::from_str(&sse_event.data)?;
            let action = sm.handle_event(stream_event);

            match action {
                StreamAction::RenderToken(text) => {
                    if first_token {
                        ui_tx.send(UiEvent::HideSpinner).ok();
                        first_token = false;
                    }
                    ui_tx.send(UiEvent::AppendText(text)).ok();
                    token_count += 1;

                    // Update status every 10 tokens
                    if token_count % 10 == 0 {
                        let elapsed = start.elapsed();
                        let tps = token_count as f64 / elapsed.as_secs_f64();
                        ui_tx.send(UiEvent::UpdateStatus {
                            tokens: token_count,
                            tokens_per_second: tps,
                            elapsed,
                        }).ok();
                    }
                }
                StreamAction::ShowToolProgress { name } => {
                    ui_tx.send(UiEvent::ToolStarted { name }).ok();
                }
                StreamAction::Finished { .. } => {
                    ui_tx.send(UiEvent::TurnComplete).ok();
                    break;
                }
                StreamAction::ReportError(err) => {
                    ui_tx.send(UiEvent::ShowError(format!("{:?}", err))).ok();
                    break;
                }
                _ => {}
            }
        }

        if matches!(sm.state(), StreamState::Complete { .. } | StreamState::Errored { .. }) {
            break;
        }
    }

    // Drop the sender to signal the render loop to finish
    drop(ui_tx);
    render_handle.await.ok();

    Ok(StreamOutput {
        text: sm.text().to_string(),
        tool_calls: sm.take_tool_calls(),
        stop_reason: match sm.state() {
            StreamState::Complete { stop_reason, .. } => Some(stop_reason.clone()),
            StreamState::Interrupted { .. } => Some("user_interrupt".to_string()),
            _ => None,
        },
    })
}
```

The stream processor never touches stdout directly. It sends `UiEvent` messages through the channel, and the render loop decides when and how to display them. This separation makes it straightforward to swap in a full TUI renderer in Chapter 8 without changing any of the streaming logic.

## Viewport management

As content streams in, you need to decide what happens when it overflows the terminal height. For a simple CLI, the terminal scrolls automatically. But if you have a status bar at the bottom, new content can push it off-screen. Here is a basic viewport tracker:

```rust
/// Tracks the visible area of the terminal and the content position.
pub struct Viewport {
    /// Terminal height in rows.
    terminal_rows: u16,
    /// Lines reserved for UI chrome (status bar, etc.).
    reserved_rows: u16,
    /// Total lines of content generated.
    total_content_lines: usize,
    /// Whether auto-scroll is enabled.
    auto_scroll: bool,
}

impl Viewport {
    pub fn new(terminal_rows: u16, reserved_rows: u16) -> Self {
        Self {
            terminal_rows,
            reserved_rows,
            total_content_lines: 0,
            auto_scroll: true,
        }
    }

    /// The number of rows available for content.
    pub fn content_rows(&self) -> u16 {
        self.terminal_rows.saturating_sub(self.reserved_rows)
    }

    /// Record that new content lines were added.
    pub fn add_lines(&mut self, count: usize) {
        self.total_content_lines += count;
    }

    /// Whether the viewport needs to scroll to show the latest content.
    pub fn needs_scroll(&self) -> bool {
        self.auto_scroll && self.total_content_lines > self.content_rows() as usize
    }
}
```

This becomes more sophisticated when you build the Ratatui-based TUI in Chapter 8. For now, the important principle is that viewport management is separate from content generation.

::: wild In the Wild
Claude Code manages its viewport with a sophisticated layout system. The response content occupies most of the terminal, with a thin status bar at the bottom showing model name, token count, and key bindings. When streaming, new content auto-scrolls into view. The user can scroll back through the response using arrow keys, which temporarily disables auto-scroll until they return to the bottom. OpenCode's Bubble Tea TUI takes a similar approach with its viewport component, which wraps content in a scrollable area with automatic scroll-to-bottom during streaming.
:::

## Key Takeaways

- Decouple stream processing from rendering using an event bus (channel of `UiEvent` messages) so the two concerns can evolve independently.
- Batch UI updates into frames at a fixed rate (30 FPS) to eliminate flicker and reduce system call overhead -- multiple tokens arriving in one frame window are rendered in a single write.
- Use an unbounded channel for UI events to prevent the stream processor from blocking on rendering, and let the frame renderer coalesce events naturally.
- The frame-based render loop pattern (drain events, render frame, sleep) is the same pattern used by game engines and TUI frameworks like Ratatui, which you will adopt in Chapter 8.
- Viewport management becomes important when you have persistent UI elements (status bars, tool indicators) that must not be pushed off-screen by streaming content.
