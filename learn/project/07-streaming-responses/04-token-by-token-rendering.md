---
title: Token By Token Rendering
description: Display LLM output incrementally as each token arrives, providing immediate visual feedback to the user in the terminal.
---

# Token By Token Rendering

> **What you'll learn:**
> - How to extract text deltas from content_block_delta events
> - How to append tokens to the terminal display without flickering or corruption
> - How to handle special characters, Unicode, and partial UTF-8 sequences in token streams

You have an SSE parser that produces typed `StreamEvent` values, and a chunked transfer layer that feeds it complete lines. Now it is time to do something useful with those events: print text to the terminal as it arrives. This is where your agent stops feeling like a batch processor and starts feeling like a real-time assistant.

## Extracting text deltas

The stream event you care about most for rendering is `ContentBlockDelta` with a `TextDelta` variant. Let's build a function that processes a stream of events and renders text in real time:

```rust
use std::io::{self, Write};

use crate::sse::{Delta, StreamEvent};

/// Processes a single stream event for display purposes.
/// Returns the text content if the event contained a text delta.
pub fn render_event(event: &StreamEvent) -> Option<&str> {
    match event {
        StreamEvent::ContentBlockDelta {
            delta: Delta::TextDelta { text },
            ..
        } => Some(text.as_str()),
        _ => None,
    }
}

/// Writes a text delta directly to stdout without a trailing newline.
/// Flushes immediately so the user sees each token as it arrives.
pub fn print_token(text: &str) {
    print!("{}", text);
    // Flush is critical -- without it, stdout line-buffers and the user
    // sees nothing until a newline character arrives
    io::stdout().flush().expect("failed to flush stdout");
}
```

The `flush()` call is essential. By default, Rust's `stdout` is line-buffered when connected to a terminal, which means output only appears when a `\n` is written. Since LLM tokens rarely end with newlines, you would see nothing until the model produces one. Flushing after every token ensures immediate display.

## The streaming render loop

Let's integrate rendering into the stream processing pipeline from the previous subchapter. Instead of collecting events into a `Vec`, you print text as it arrives:

```rust
use bytes::Bytes;
use futures::StreamExt;
use std::io::{self, Write};

use crate::chunked::LineSplitter;
use crate::sse::{Delta, SseParser, StreamEvent};

/// The accumulated result of processing a complete stream.
pub struct StreamResult {
    pub text_content: String,
    pub stop_reason: Option<String>,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

pub async fn stream_and_render(
    mut byte_stream: impl futures::Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
) -> Result<StreamResult, Box<dyn std::error::Error>> {
    let mut splitter = LineSplitter::new();
    let mut parser = SseParser::new();

    let mut full_text = String::new();
    let mut stop_reason = None;
    let mut input_tokens = 0u32;
    let mut output_tokens = 0u32;

    while let Some(chunk_result) = byte_stream.next().await {
        let chunk = chunk_result?;
        let lines = splitter.feed(&chunk);

        for line in lines {
            if let Some(sse_event) = parser.feed_line(&line) {
                if sse_event.event_type == "ping" {
                    continue;
                }

                let stream_event: StreamEvent = serde_json::from_str(&sse_event.data)?;

                match &stream_event {
                    StreamEvent::MessageStart { message } => {
                        input_tokens = message.usage.input_tokens;
                    }
                    StreamEvent::ContentBlockDelta {
                        delta: Delta::TextDelta { text },
                        ..
                    } => {
                        // Print the token immediately
                        print!("{}", text);
                        io::stdout().flush()?;
                        // Also accumulate for later use
                        full_text.push_str(text);
                    }
                    StreamEvent::MessageDelta { delta, usage } => {
                        stop_reason = delta.stop_reason.clone();
                        output_tokens = usage.output_tokens;
                    }
                    StreamEvent::MessageStop => {
                        // Print a final newline after the complete response
                        println!();
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(StreamResult {
        text_content: full_text,
        stop_reason,
        input_tokens,
        output_tokens,
    })
}
```

Notice that you accumulate the full text in `full_text` alongside printing it. You need the complete text for the conversation history -- after streaming finishes, you build the same `Message` struct that the non-streaming API would have returned and append it to the conversation.

::: python Coming from Python
In Python, rendering a stream is as simple as:
```python
with client.messages.stream(model="claude-sonnet-4-20250514", messages=messages, max_tokens=1024) as stream:
    for text in stream.text_stream:
        print(text, end="", flush=True)
```
The Rust version is more explicit about flushing and error handling, but the core pattern is identical: iterate over text chunks and print each one without a trailing newline. The big difference is that Rust gives you access to every layer of the pipeline, so you can optimize at any point.
:::

## Handling special characters and Unicode

LLM output contains all sorts of characters that need careful handling: code blocks with backticks, ANSI-like sequences, emoji, CJK characters, and more. Here are the cases you need to handle:

### Newlines and carriage returns

Tokens frequently contain newline characters, especially in code output. These work naturally with `print!()` -- the terminal handles `\n` correctly. But watch out for `\r` (carriage return), which moves the cursor to the beginning of the line. If a token contains `\r\n`, the terminal handles it fine. An isolated `\r` could overwrite already-displayed text.

### Multi-byte UTF-8 characters

A single emoji like a checkmark might be encoded as 3 bytes in UTF-8. In theory, a chunk boundary could split those bytes. However, the Anthropic API sends tokens as complete Unicode strings in JSON, so you will not encounter partial UTF-8 within a single `text_delta`. The risk is at the HTTP chunk level, but your `LineSplitter` handles this by operating on complete lines.

### Code blocks and indentation

When the model outputs a code block, it often sends tokens like:

```
"```" → "rust" → "\n" → "fn " → "main" → "()" → " {" → "\n" → "    " → "println!" → ...
```

Each token displays correctly because you are using `print!()` without any formatting -- the raw text flows through exactly as the model generated it.

## A robust token renderer

Let's build a more complete renderer that tracks state and handles edge cases:

```rust
use std::io::{self, Write};
use std::time::Instant;

/// Tracks rendering state for a streaming response.
pub struct TokenRenderer {
    /// All text received so far, for conversation history.
    accumulated_text: String,
    /// Number of tokens (deltas) received.
    token_count: u32,
    /// When rendering started, for throughput calculation.
    start_time: Option<Instant>,
    /// Whether we are inside a code block (between ``` markers).
    in_code_block: bool,
    /// The stdout handle, locked once for efficiency.
    stdout: io::Stdout,
}

impl TokenRenderer {
    pub fn new() -> Self {
        Self {
            accumulated_text: String::new(),
            token_count: 0,
            start_time: None,
            in_code_block: false,
            stdout: io::stdout(),
        }
    }

    /// Render a single text delta to the terminal.
    pub fn render_delta(&mut self, text: &str) -> io::Result<()> {
        if self.start_time.is_none() {
            self.start_time = Some(Instant::now());
        }

        self.token_count += 1;
        self.accumulated_text.push_str(text);

        // Track code block state for potential future formatting
        let backtick_count = text.matches("```").count();
        if backtick_count % 2 == 1 {
            self.in_code_block = !self.in_code_block;
        }

        // Write and flush immediately
        write!(self.stdout, "{}", text)?;
        self.stdout.flush()?;

        Ok(())
    }

    /// Call when the stream is complete. Prints a final newline
    /// and returns the accumulated text.
    pub fn finish(mut self) -> io::Result<RenderResult> {
        writeln!(self.stdout)?;
        self.stdout.flush()?;

        let elapsed = self.start_time.map(|t| t.elapsed());
        let tokens_per_second = elapsed.map(|d| {
            if d.as_secs_f64() > 0.0 {
                self.token_count as f64 / d.as_secs_f64()
            } else {
                0.0
            }
        });

        Ok(RenderResult {
            text: self.accumulated_text,
            token_count: self.token_count,
            tokens_per_second,
        })
    }

    /// Get the accumulated text so far (useful for partial results on interrupt).
    pub fn text_so_far(&self) -> &str {
        &self.accumulated_text
    }

    /// Check if we are currently inside a code block.
    pub fn is_in_code_block(&self) -> bool {
        self.in_code_block
    }
}

pub struct RenderResult {
    pub text: String,
    pub token_count: u32,
    pub tokens_per_second: Option<f64>,
}
```

This renderer gives you several advantages over raw `print!()`:

- **Text accumulation** -- you always have the full text available for the conversation history.
- **Token counting** -- track how many deltas you have received.
- **Throughput measurement** -- calculate tokens per second for the progress display you will build later.
- **Code block tracking** -- know whether you are inside a code fence, which is useful for formatting decisions.

## Integrating the renderer into the stream loop

Here is how the renderer fits into the processing pipeline:

```rust
pub async fn stream_with_renderer(
    mut byte_stream: impl futures::Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
) -> Result<RenderResult, Box<dyn std::error::Error>> {
    let mut splitter = LineSplitter::new();
    let mut parser = SseParser::new();
    let mut renderer = TokenRenderer::new();

    while let Some(chunk_result) = byte_stream.next().await {
        let chunk = chunk_result?;
        let lines = splitter.feed(&chunk);

        for line in lines {
            if let Some(sse_event) = parser.feed_line(&line) {
                if sse_event.event_type == "ping" {
                    continue;
                }

                let stream_event: StreamEvent = serde_json::from_str(&sse_event.data)?;

                match &stream_event {
                    StreamEvent::ContentBlockDelta {
                        delta: Delta::TextDelta { text },
                        ..
                    } => {
                        renderer.render_delta(text)?;
                    }
                    StreamEvent::MessageStop => {
                        return Ok(renderer.finish()?);
                    }
                    _ => {}
                }
            }
        }
    }

    // Stream ended without message_stop (unexpected)
    Ok(renderer.finish()?)
}
```

## Locked stdout for performance

When rendering many tokens per second (Claude can generate 50+ tokens/s), the overhead of acquiring the stdout lock on every `flush()` adds up. You can optimize by locking stdout once:

```rust
use std::io::{self, BufWriter, Write};

/// High-performance token writer that locks stdout once.
pub fn render_tokens_fast(tokens: &[String]) -> io::Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    for token in tokens {
        write!(handle, "{}", token)?;
    }

    handle.flush()?;
    Ok(())
}
```

In practice, for a streaming agent, the per-token lock overhead is negligible compared to the network latency between tokens. But if you batch multiple tokens (as you will explore in the buffering chapter), locking once is a measurable improvement.

## Key Takeaways

- Always call `io::stdout().flush()` after printing a token -- without it, stdout line-buffers and the user sees nothing until a newline.
- Accumulate the full text alongside printing it, because you need the complete text for the conversation history after streaming finishes.
- The `TokenRenderer` struct encapsulates rendering state: accumulated text, token count, throughput timing, and code block tracking.
- Unicode and special characters work naturally because the Anthropic API sends complete Unicode strings in each `text_delta` -- you never receive partial UTF-8 within a single token.
- For high-throughput rendering, lock stdout once rather than acquiring the lock on every write.
