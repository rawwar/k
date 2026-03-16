---
title: Progress Display
description: Build progress indicators that show streaming status, token counts, and elapsed time to keep users informed during long responses.
---

# Progress Display

> **What you'll learn:**
> - How to display a spinner or progress bar while waiting for the first token
> - How to show live token count and throughput metrics during streaming
> - How to render tool execution progress inline with the streaming response

A user staring at a blank terminal after sending a prompt does not know if their agent is connecting to the API, waiting for the first token, or frozen. Progress indicators fill this gap. In this subchapter you will build three types of feedback: a connection spinner, live streaming metrics, and tool execution indicators.

## The connection spinner

The gap between sending the request and receiving the first token is the most anxious moment for the user. A spinner shows that something is happening:

```rust
use std::io::{self, Write};
use std::time::Duration;
use tokio::sync::watch;

/// A simple terminal spinner that runs in the background.
pub struct Spinner {
    /// Send `true` to stop the spinner.
    stop_tx: watch::Sender<bool>,
    /// Handle to the background task.
    handle: tokio::task::JoinHandle<()>,
}

impl Spinner {
    /// Start a spinner with the given message (e.g., "Thinking...").
    pub fn start(message: &str) -> Self {
        let (stop_tx, mut stop_rx) = watch::channel(false);
        let message = message.to_string();

        let handle = tokio::spawn(async move {
            let frames = ['|', '/', '-', '\\'];
            let mut i = 0;

            loop {
                // Check if we should stop
                if *stop_rx.borrow() {
                    // Clear the spinner line
                    eprint!("\r{}\r", " ".repeat(message.len() + 4));
                    io::stderr().flush().ok();
                    return;
                }

                eprint!("\r{} {}", frames[i % frames.len()], message);
                io::stderr().flush().ok();
                i += 1;

                // Wait 100ms or until stop signal
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                    _ = stop_rx.changed() => {
                        eprint!("\r{}\r", " ".repeat(message.len() + 4));
                        io::stderr().flush().ok();
                        return;
                    }
                }
            }
        });

        Self { stop_tx, handle }
    }

    /// Stop the spinner and clean up the display line.
    pub async fn stop(self) {
        self.stop_tx.send(true).ok();
        self.handle.await.ok();
    }
}
```

Use it in the streaming flow:

```rust
// Start the spinner before connecting
let spinner = Spinner::start("Thinking...");

// Establish the streaming connection
let byte_stream = start_streaming_request(&client, &api_key, &messages).await?;

// Process events -- stop the spinner when the first token arrives
let mut first_token = true;
// ... in the event loop:
if let StreamEvent::ContentBlockDelta { delta: Delta::TextDelta { .. }, .. } = &event {
    if first_token {
        spinner.stop().await;
        first_token = false;
    }
}
```

The spinner writes to stderr using `eprint!()` so it does not interfere with the actual response text going to stdout. The `\r` carriage return moves the cursor to the beginning of the line, so the spinner characters overwrite each other. When stopped, the spinner clears its line entirely.

::: python Coming from Python
In Python, you might use the `rich` library for spinners:
```python
from rich.console import Console
from rich.spinner import Spinner

console = Console(stderr=True)
with console.status("Thinking..."):
    response = await get_first_token()
```
The Rust version is more verbose but achieves the same effect. The key pattern is the same: start the spinner before the network call, stop it when data arrives, and use stderr to avoid mixing with stdout content.
:::

## Live streaming metrics

During a long response, showing token count and throughput helps the user gauge progress:

```rust
use std::io::{self, Write};
use std::time::Instant;

/// Tracks and displays streaming progress metrics.
pub struct StreamProgress {
    /// When streaming started (first token received).
    started_at: Option<Instant>,
    /// Total text tokens received.
    token_count: u32,
    /// Total tool calls completed.
    tool_calls_completed: u32,
    /// Whether to show the status bar.
    show_metrics: bool,
    /// Last time we updated the status bar.
    last_update: Instant,
    /// Minimum interval between status bar updates.
    update_interval: Duration,
}

impl StreamProgress {
    pub fn new(show_metrics: bool) -> Self {
        Self {
            started_at: None,
            token_count: 0,
            tool_calls_completed: 0,
            show_metrics,
            last_update: Instant::now(),
            update_interval: Duration::from_millis(500),
        }
    }

    /// Record a text token arrival.
    pub fn record_token(&mut self) {
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }
        self.token_count += 1;
        self.maybe_update_display();
    }

    /// Record a tool call completion.
    pub fn record_tool_call(&mut self) {
        self.tool_calls_completed += 1;
    }

    /// Calculate current tokens per second.
    pub fn tokens_per_second(&self) -> f64 {
        match self.started_at {
            Some(start) => {
                let elapsed = start.elapsed().as_secs_f64();
                if elapsed > 0.0 {
                    self.token_count as f64 / elapsed
                } else {
                    0.0
                }
            }
            None => 0.0,
        }
    }

    /// Update the status bar if enough time has passed.
    fn maybe_update_display(&mut self) {
        if !self.show_metrics {
            return;
        }

        if self.last_update.elapsed() < self.update_interval {
            return;
        }

        self.last_update = Instant::now();
        self.render_status_bar();
    }

    fn render_status_bar(&self) {
        let elapsed = self.started_at
            .map(|s| s.elapsed())
            .unwrap_or_default();

        let tps = self.tokens_per_second();

        // Write status to stderr so it doesn't mix with response text
        eprint!(
            "\r\x1b[90m[{} tokens | {:.1} tok/s | {:.1}s]\x1b[0m",
            self.token_count,
            tps,
            elapsed.as_secs_f64()
        );
        io::stderr().flush().ok();
    }

    /// Print final summary when streaming is complete.
    pub fn print_summary(&self) {
        if !self.show_metrics {
            return;
        }

        let elapsed = self.started_at
            .map(|s| s.elapsed())
            .unwrap_or_default();

        let tps = self.tokens_per_second();

        // Clear the status bar line and print the summary
        eprintln!(
            "\r\x1b[90m[{} tokens | {:.1} tok/s | {:.1}s | {} tool calls]\x1b[0m",
            self.token_count,
            tps,
            elapsed.as_secs_f64(),
            self.tool_calls_completed
        );
    }
}
```

The status bar uses ANSI escape codes: `\x1b[90m` sets the text to dark gray (so it is visually distinct from the response), and `\x1b[0m` resets colors. The `\r` carriage return overwrites the previous status bar on each update.

The `update_interval` of 500ms prevents excessive stderr writes. Updating the status bar on every single token (50+ times/second) would cause visible flicker. Updating twice per second is smooth enough while keeping overhead negligible.

## Tool execution progress

When the model calls a tool, the user should see what is happening. Here is a tool progress indicator that integrates with the streaming display:

```rust
use std::io::{self, Write};

/// Displays inline tool execution progress.
pub struct ToolProgressDisplay;

impl ToolProgressDisplay {
    /// Show that a tool call is starting.
    pub fn tool_started(name: &str) {
        eprintln!("\n\x1b[36m> Running: {}\x1b[0m", name);
    }

    /// Show that a tool call completed with a brief result summary.
    pub fn tool_completed(name: &str, duration: Duration, success: bool) {
        let status = if success {
            "\x1b[32m+\x1b[0m" // Green checkmark
        } else {
            "\x1b[31m!\x1b[0m" // Red exclamation
        };

        eprintln!(
            "\x1b[36m  {} {} ({:.1}s)\x1b[0m",
            status,
            name,
            duration.as_secs_f64()
        );
    }

    /// Show tool output preview (first N characters).
    pub fn tool_output_preview(output: &str, max_chars: usize) {
        let preview: String = output.chars().take(max_chars).collect();
        let truncated = output.len() > max_chars;
        eprint!("\x1b[90m  | {}", preview);
        if truncated {
            eprint!("...");
        }
        eprintln!("\x1b[0m");
    }
}
```

In the agentic loop, wire these displays together:

```rust
// In the main streaming + tool execution loop:
loop {
    let spinner = Spinner::start("Thinking...");
    let mut progress = StreamProgress::new(true);
    let mut first_token = true;

    let byte_stream = start_streaming_request(&client, &api_key, &messages).await?;
    // ... process stream events ...

    // When a text delta arrives:
    // if first_token { spinner.stop().await; first_token = false; }
    // progress.record_token();

    // When streaming finishes:
    progress.print_summary();

    // When executing tool calls:
    for tool_call in &output.tool_calls {
        ToolProgressDisplay::tool_started(&tool_call.name);
        let start = Instant::now();
        let result = execute_tool(tool_call).await;
        let success = result.is_ok();
        ToolProgressDisplay::tool_completed(&tool_call.name, start.elapsed(), success);
        if let Ok(output_text) = &result {
            ToolProgressDisplay::tool_output_preview(output_text, 100);
        }
    }
}
```

## A complete progress-aware stream function

Let's put the spinner, metrics, and tool display together:

```rust
pub async fn stream_with_progress(
    client: &reqwest::Client,
    api_key: &str,
    messages: &[serde_json::Value],
    cancel_token: CancellationToken,
    show_metrics: bool,
) -> Result<StreamOutput, Box<dyn std::error::Error>> {
    let spinner = Spinner::start("Thinking...");
    let mut progress = StreamProgress::new(show_metrics);
    let mut spinner_stopped = false;

    let byte_stream = start_streaming_request(client, api_key, messages).await?;
    let mut splitter = LineSplitter::new();
    let mut parser = SseParser::new();
    let mut sm = StreamStateMachine::new();

    loop {
        let chunk = tokio::select! {
            chunk = futures::StreamExt::next(&mut byte_stream) => {
                match chunk {
                    Some(Ok(bytes)) => bytes,
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
                    if !spinner_stopped {
                        spinner.stop().await;
                        spinner_stopped = true;
                    }
                    print!("{}", text);
                    io::stdout().flush()?;
                    progress.record_token();
                }
                StreamAction::ShowToolProgress { name } => {
                    if !spinner_stopped {
                        spinner.stop().await;
                        spinner_stopped = true;
                    }
                    ToolProgressDisplay::tool_started(&name);
                }
                StreamAction::Finished { .. } => break,
                StreamAction::ReportError(err) => {
                    eprintln!("\n[Error: {:?}]", err);
                    break;
                }
                _ => {}
            }
        }

        if matches!(sm.state(), StreamState::Complete { .. } | StreamState::Errored { .. }) {
            break;
        }
    }

    if !spinner_stopped {
        spinner.stop().await;
    }

    println!();
    progress.print_summary();

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

::: wild In the Wild
Claude Code shows a "Thinking..." spinner before the first token, then switches to flowing text. During tool execution, it displays the tool name with a colored indicator and shows a brief preview of the tool output. Token counts and timing are shown in a subtle status area. OpenCode's Bubble Tea TUI has a dedicated status bar at the bottom of the screen that continuously updates with token count, elapsed time, and the current model name -- all rendered as part of the TUI frame rather than inline with the response text.
:::

## Key Takeaways

- A spinner during the connection phase ("Thinking...") reassures the user that the agent is working, covering the 200-500ms gap before the first token.
- Write progress indicators to stderr so they do not mix with the response text on stdout.
- Update the status bar at a fixed interval (500ms) rather than on every token to avoid flicker and excessive system calls.
- Tool execution progress (started, completed, output preview) keeps the user informed during the silent gaps in an agentic loop when tools are running.
- Use ANSI color codes to visually distinguish progress indicators from response content: gray for metrics, cyan for tool indicators.
