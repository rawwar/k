---
title: Parsing SSE in Rust
description: Building a robust SSE parser in Rust that handles multi-line data fields, BOM stripping, comment lines, and incomplete events at buffer boundaries.
---

# Parsing SSE in Rust

> **What you'll learn:**
> - How to implement a streaming SSE parser using Rust's async byte stream traits and line-based state machine
> - Handling edge cases in SSE parsing: multi-line data concatenation, UTF-8 BOM, comment filtering, and field name normalization
> - Integrating an SSE parser with reqwest's streaming response body to process LLM API responses

You understand the SSE specification. You understand chunked encoding. Now it is time to build a real SSE parser in Rust that takes raw bytes from an HTTP response and produces structured events your agent can act on. This is one of the most critical pieces of infrastructure in your streaming pipeline -- every token your agent displays passes through this parser.

## The Event Struct

First, let's define what the parser produces. An SSE event has an optional type, a data payload, an optional ID, and an optional retry value:

```rust
#[derive(Debug, Clone)]
pub struct SseEvent {
    /// The event type (from the "event:" field). None means "message" (the default).
    pub event_type: Option<String>,
    /// The event data (from one or more "data:" fields, joined by newlines).
    pub data: String,
    /// The event ID (from the "id:" field).
    pub id: Option<String>,
    /// The reconnection time in milliseconds (from the "retry:" field).
    pub retry: Option<u64>,
}

impl SseEvent {
    /// Returns the event type, defaulting to "message" if none was specified.
    pub fn event_type(&self) -> &str {
        self.event_type.as_deref().unwrap_or("message")
    }
}
```

## The Line-Based State Machine

SSE parsing is fundamentally a line-based state machine. You accumulate fields until a blank line triggers event dispatch. Here is the full parser:

```rust
#[derive(Debug, Default)]
struct EventBuilder {
    event_type: Option<String>,
    data_lines: Vec<String>,
    id: Option<String>,
    retry: Option<u64>,
    has_data: bool,
}

impl EventBuilder {
    /// Process a single line from the SSE stream.
    /// Returns Some(SseEvent) when a blank line triggers dispatch.
    fn process_line(&mut self, line: &str) -> Option<SseEvent> {
        // Blank line: dispatch the event if we have data
        if line.is_empty() {
            return self.dispatch();
        }

        // Comment lines start with ':'
        if line.starts_with(':') {
            return None; // Ignore comments
        }

        // Parse field name and value
        let (field, value) = if let Some(colon_pos) = line.find(':') {
            let field = &line[..colon_pos];
            let mut value = &line[colon_pos + 1..];
            // Strip one leading space
            if value.starts_with(' ') {
                value = &value[1..];
            }
            (field, value)
        } else {
            // No colon: entire line is the field name, value is empty
            (line, "")
        };

        match field {
            "data" => {
                self.data_lines.push(value.to_string());
                self.has_data = true;
            }
            "event" => {
                self.event_type = Some(value.to_string());
            }
            "id" => {
                // Per spec: ignore id fields that contain null characters
                if !value.contains('\0') {
                    self.id = Some(value.to_string());
                }
            }
            "retry" => {
                if let Ok(ms) = value.parse::<u64>() {
                    self.retry = Some(ms);
                }
                // Non-numeric retry values are silently ignored per spec
            }
            _ => {
                // Unknown field names are ignored per spec
            }
        }

        None
    }

    /// Dispatch the current event and reset the builder.
    fn dispatch(&mut self) -> Option<SseEvent> {
        if !self.has_data {
            // No data fields were set -- drop the event
            self.reset();
            return None;
        }

        let event = SseEvent {
            event_type: self.event_type.take(),
            data: self.data_lines.join("\n"),
            id: self.id.take(),
            retry: self.retry.take(),
        };

        self.reset();
        Some(event)
    }

    fn reset(&mut self) {
        self.event_type = None;
        self.data_lines.clear();
        self.id = None;
        self.retry = None;
        self.has_data = false;
    }
}
```

The key design decisions in this parser:

- **`data_lines` is a Vec**, not a String. Multiple `data:` lines are accumulated and joined with `\n` at dispatch time. This avoids repeatedly reallocating a string as data lines arrive.
- **`has_data` tracks whether any data field was set.** If an event has only `event:` and `id:` fields but no `data:`, it is silently dropped per the specification.
- **`dispatch()` consumes and resets the builder.** After dispatching an event, the builder is ready for the next one. The `event_type` resets to None (unlike `id`, which the spec says persists across events in browser implementations -- but for LLM API usage, we reset it for safety).

## Integrating with reqwest

Now let's connect this parser to a real HTTP stream. The `reqwest` response gives you a `bytes_stream()` that yields chunks of bytes. You need to feed these into a line buffer, then feed lines into the event builder:

```rust
use futures::StreamExt;
use reqwest::Response;
use tokio::sync::mpsc;

pub struct SseStream {
    line_buffer: String,
    event_builder: EventBuilder,
    bom_stripped: bool,
}

impl SseStream {
    pub fn new() -> Self {
        Self {
            line_buffer: String::new(),
            event_builder: EventBuilder::default(),
            bom_stripped: false,
        }
    }

    /// Process raw bytes from the HTTP stream and return any complete events.
    pub fn feed(&mut self, data: &[u8]) -> Vec<SseEvent> {
        let mut bytes = data;

        // Strip BOM at the start of the stream
        if !self.bom_stripped {
            if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
                bytes = &bytes[3..];
            }
            self.bom_stripped = true;
        }

        let text = String::from_utf8_lossy(bytes);
        self.line_buffer.push_str(&text);

        let mut events = Vec::new();

        // Extract complete lines and process them
        while let Some(newline_pos) = self.line_buffer.find('\n') {
            let line = self.line_buffer[..newline_pos]
                .trim_end_matches('\r')
                .to_string();

            self.line_buffer = self.line_buffer[newline_pos + 1..].to_string();

            if let Some(event) = self.event_builder.process_line(&line) {
                events.push(event);
            }
        }

        events
    }
}

/// Stream SSE events from a reqwest response.
pub async fn stream_sse_events(
    response: Response,
    tx: mpsc::Sender<SseEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut parser = SseStream::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        let events = parser.feed(&bytes);

        for event in events {
            tx.send(event).await?;
        }
    }

    Ok(())
}
```

This function takes a `reqwest::Response` and a channel sender, then pushes parsed SSE events into the channel. The channel decouples the network reader from the event consumer, which is essential for backpressure (covered in a [later subchapter](/linear/08-streaming-and-realtime/08-backpressure-and-flow-control)).

::: python Coming from Python
In Python, you might use `aiohttp` for streaming:
```python
async with session.post(url, json=payload) as resp:
    async for line in resp.content:
        line = line.decode('utf-8').strip()
        if line.startswith('data: '):
            data = line[6:]
            yield json.loads(data)
```
The Rust version is more explicit about buffer management and line splitting. Where Python's `async for line in resp.content` gives you lines magically, in Rust you manage the buffer yourself, which gives you control over allocation patterns and lets you handle partial lines that span chunk boundaries correctly. The trade-off is more code, but no hidden allocations or unexpected buffering behavior.
:::

## Putting It All Together

Here is a complete example that connects to an LLM API, parses the SSE stream, and prints each event:

```rust
use futures::StreamExt;
use reqwest::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let api_key = std::env::var("ANTHROPIC_API_KEY")?;

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 256,
            "stream": true,
            "messages": [{"role": "user", "content": "Say hello in three languages"}]
        }).to_string())
        .send()
        .await?;

    // Validate content type
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !content_type.starts_with("text/event-stream") {
        let body = response.text().await?;
        return Err(format!("Expected SSE stream, got {}: {}", content_type, body).into());
    }

    let mut parser = SseStream::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        let events = parser.feed(&bytes);

        for event in events {
            match event.event_type() {
                "content_block_delta" => {
                    // Parse the delta to extract text
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&event.data) {
                        if let Some(text) = value
                            .get("delta")
                            .and_then(|d| d.get("text"))
                            .and_then(|t| t.as_str())
                        {
                            print!("{}", text);
                        }
                    }
                }
                "message_stop" => {
                    println!(); // Newline after the message
                }
                _ => {
                    // message_start, content_block_start, etc.
                }
            }
        }
    }

    Ok(())
}
```

This is a functional streaming client. It handles all the SSE parsing edge cases (BOM, comments, multi-line data, partial lines), validates the content type, and extracts text deltas from the Anthropic event format.

## Testing Your Parser

SSE parsers are excellent candidates for unit testing because the input and output are well-defined:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_event() {
        let mut parser = SseStream::new();
        let events = parser.feed(b"data: hello\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
        assert_eq!(events[0].event_type(), "message");
    }

    #[test]
    fn test_named_event() {
        let mut parser = SseStream::new();
        let events = parser.feed(b"event: delta\ndata: {\"text\":\"hi\"}\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), "delta");
    }

    #[test]
    fn test_multiline_data() {
        let mut parser = SseStream::new();
        let events = parser.feed(b"data: line1\ndata: line2\ndata: line3\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "line1\nline2\nline3");
    }

    #[test]
    fn test_split_across_chunks() {
        let mut parser = SseStream::new();
        // First chunk: partial line
        let events1 = parser.feed(b"data: hel");
        assert_eq!(events1.len(), 0);
        // Second chunk: rest of line + blank line
        let events2 = parser.feed(b"lo world\n\n");
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0].data, "hello world");
    }

    #[test]
    fn test_comment_ignored() {
        let mut parser = SseStream::new();
        let events = parser.feed(b": this is a comment\ndata: actual data\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "actual data");
    }

    #[test]
    fn test_bom_stripped() {
        let mut parser = SseStream::new();
        let mut input = vec![0xEF, 0xBB, 0xBF]; // BOM
        input.extend_from_slice(b"data: after bom\n\n");
        let events = parser.feed(&input);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "after bom");
    }

    #[test]
    fn test_event_without_data_is_dropped() {
        let mut parser = SseStream::new();
        let events = parser.feed(b"event: ping\nid: 123\n\n");
        assert_eq!(events.len(), 0); // No data field, event is dropped
    }
}
```

These tests cover the most important edge cases. The `test_split_across_chunks` test is particularly important -- it verifies that your parser correctly handles data that arrives in arbitrary-sized pieces, which is the norm in production.

::: wild In the Wild
Claude Code implements its SSE parser as a streaming transform that sits between the HTTP response stream and the event processing layer. The parser is designed to be allocation-efficient: it reuses internal buffers across events and avoids cloning strings when possible. For a coding agent processing thousands of streaming events per conversation, these small optimizations add up to measurable improvements in memory usage and GC pressure (or in Rust's case, allocation overhead).
:::

## Key Takeaways

- An SSE parser is a **line-based state machine** that accumulates fields until a blank line triggers event dispatch. The implementation fits in roughly 100 lines of Rust.
- The parser must handle **partial lines that span HTTP chunks** by maintaining a line buffer that accumulates data between `feed()` calls.
- **Multi-line data fields** are joined with newline characters, events without data are silently dropped, and comment lines (starting with `:`) are ignored.
- Integrating with `reqwest` is straightforward: iterate over `response.bytes_stream()`, feed each chunk into the parser, and collect emitted events.
- **Unit testing SSE parsers is easy and high-value** because the input/output contract is well-defined. Always test the split-across-chunks case.
