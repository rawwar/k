---
title: SSE Protocol
description: Deep dive into the Server-Sent Events protocol format, event types, and how the Anthropic API structures its streaming responses.
---

# SSE Protocol

> **What you'll learn:**
> - How the SSE wire format works with event, data, id, and retry fields
> - What event types the Anthropic streaming API emits and their payload schemas
> - How to parse raw SSE text lines into structured event objects in Rust

Server-Sent Events (SSE) is the protocol that carries streaming responses from the Anthropic API to your agent. Before you write any streaming code, you need to understand what the wire format looks like, what events the API sends, and how to parse them. SSE is deliberately simple -- it is just structured text over HTTP -- but the details matter when you are building a parser from scratch.

## The SSE wire format

SSE is defined in the [HTML Living Standard](https://html.spec.whatwg.org/multipage/server-sent-events.html). The format is plain text, with events separated by blank lines. Each event consists of one or more fields, each on its own line:

```
event: message_start
data: {"type":"message_start","message":{"id":"msg_01X","type":"message","role":"assistant","content":[],"model":"claude-sonnet-4-20250514","stop_reason":null,"usage":{"input_tokens":25,"output_tokens":1}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" world"}}

```

The four SSE field types are:

| Field   | Purpose                                           | Example                    |
|---------|---------------------------------------------------|----------------------------|
| `event` | Names the event type (defaults to "message")      | `event: content_block_delta` |
| `data`  | The payload -- can span multiple lines             | `data: {"type":"text"}`    |
| `id`    | Event ID for reconnection (see Chapter 11)        | `id: evt_123`             |
| `retry` | Server-suggested reconnection delay in ms         | `retry: 5000`             |

Key rules of the SSE format:

1. **Lines starting with `:` are comments** and should be ignored. Servers often send `: ping` as keepalives.
2. **A blank line terminates an event.** Two consecutive newlines (`\n\n`) signal the end of one event.
3. **Multi-line data** uses multiple `data:` lines -- the parser concatenates them with newlines between.
4. **Unknown fields are ignored.** This makes SSE forward-compatible.

## Anthropic streaming event types

The Anthropic Messages API emits a specific sequence of SSE events. Understanding their order and payloads is critical for building your parser. Here is the complete lifecycle of a streamed response:

```
message_start          -- contains the Message shell (id, model, role, usage)
  content_block_start  -- starts a text or tool_use content block
    content_block_delta  -- one or more deltas with text or JSON fragments
    content_block_delta
    ...
  content_block_stop   -- ends the current content block
  content_block_start  -- a second content block (e.g., a tool call)
    content_block_delta
    ...
  content_block_stop
message_delta          -- final update with stop_reason and output usage
message_stop           -- stream is complete
```

Let's look at the important event types in detail:

**`message_start`** -- Arrives first. Its `data` payload contains a `message` object with the message ID, model name, role (`assistant`), an empty `content` array, and initial `usage` with `input_tokens` counted.

**`content_block_start`** -- Signals the beginning of a new content block. The payload includes an `index` (0-based position in the content array) and a `content_block` stub. For text, it looks like `{"type":"text","text":""}`. For tool use, it includes the tool `name` and `id`.

**`content_block_delta`** -- The workhorse event. For text blocks, the delta contains `{"type":"text_delta","text":"..."}` with one or more tokens. For tool_use blocks, it contains `{"type":"input_json_delta","partial_json":"..."}` with a fragment of the tool's JSON arguments.

**`content_block_stop`** -- Marks the end of a content block. The payload just contains the `index`.

**`message_delta`** -- Arrives near the end. Contains the `stop_reason` (e.g., `"end_turn"`, `"tool_use"`) and `usage` with `output_tokens`.

**`message_stop`** -- The final event. Signals that the stream is complete.

**`ping`** -- Sent periodically as keepalives. Your parser should ignore these.

**`error`** -- Indicates a server-side error. Contains an error object with `type` and `message` fields.

## Building an SSE parser in Rust

Let's build a parser that converts raw SSE text into typed event structures. Start with the data types:

```rust
use serde::Deserialize;

/// A single parsed SSE event with its event type and data payload.
#[derive(Debug, Clone)]
pub struct SseEvent {
    pub event_type: String,
    pub data: String,
    pub id: Option<String>,
}

/// All possible streaming event types from the Anthropic API.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: MessageShell },

    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: usize,
        content_block: ContentBlockStub,
    },

    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: usize, delta: Delta },

    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: usize },

    #[serde(rename = "message_delta")]
    MessageDelta { delta: MessageDeltaBody, usage: OutputUsage },

    #[serde(rename = "message_stop")]
    MessageStop,

    #[serde(rename = "ping")]
    Ping,

    #[serde(rename = "error")]
    Error { error: ApiError },
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageShell {
    pub id: String,
    pub model: String,
    pub role: String,
    pub usage: InputUsage,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlockStub {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum Delta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageDeltaBody {
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InputUsage {
    pub input_tokens: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OutputUsage {
    pub output_tokens: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}
```

Now the parser itself. It processes lines one at a time, accumulating fields until it hits a blank line:

```rust
/// Parses raw SSE lines into SseEvent structs.
pub struct SseParser {
    event_type: Option<String>,
    data_lines: Vec<String>,
    last_event_id: Option<String>,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            event_type: None,
            data_lines: Vec::new(),
            last_event_id: None,
        }
    }

    /// Feed a single line to the parser.
    /// Returns Some(SseEvent) when a complete event is ready.
    pub fn feed_line(&mut self, line: &str) -> Option<SseEvent> {
        // Blank line means "dispatch the event"
        if line.is_empty() {
            return self.dispatch();
        }

        // Comment lines start with ':'
        if line.starts_with(':') {
            return None;
        }

        // Split on the first ':' to get field name and value
        let (field, value) = if let Some(colon_pos) = line.find(':') {
            let field = &line[..colon_pos];
            // Value starts after colon, stripping one optional leading space
            let value = &line[colon_pos + 1..];
            let value = value.strip_prefix(' ').unwrap_or(value);
            (field, value)
        } else {
            // Line with no colon: field name is the entire line, value is empty
            (line.as_ref(), "")
        };

        match field {
            "event" => self.event_type = Some(value.to_string()),
            "data" => self.data_lines.push(value.to_string()),
            "id" => self.last_event_id = Some(value.to_string()),
            "retry" => { /* We handle retry in the reconnection layer */ }
            _ => { /* Unknown fields are ignored per spec */ }
        }

        None
    }

    fn dispatch(&mut self) -> Option<SseEvent> {
        if self.data_lines.is_empty() && self.event_type.is_none() {
            return None; // Empty event, nothing to dispatch
        }

        let event = SseEvent {
            event_type: self.event_type.take().unwrap_or_else(|| "message".to_string()),
            data: self.data_lines.join("\n"),
            id: self.last_event_id.clone(),
        };

        self.data_lines.clear();

        Some(event)
    }
}
```

Finally, convert raw `SseEvent` structs into typed `StreamEvent` values:

```rust
impl SseEvent {
    /// Parse the SSE event's data field into a typed StreamEvent.
    pub fn into_stream_event(self) -> Result<StreamEvent, serde_json::Error> {
        serde_json::from_str(&self.data)
    }
}
```

::: python Coming from Python
In Python, you typically never write an SSE parser. Libraries like `httpx-sse` or the Anthropic SDK handle it internally:
```python
# The SDK hides all SSE parsing
with client.messages.stream(...) as stream:
    for event in stream.events():
        print(event.type, event.data)
```
In Rust, the `eventsource-stream` crate can handle SSE parsing for you, but building your own teaches you exactly what is happening on the wire. Production agents like Claude Code use custom SSE parsers for maximum control over error handling and event routing.
:::

## Testing the parser

You can verify your parser handles the SSE format correctly with a unit test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_text_delta() {
        let mut parser = SseParser::new();

        // Feed lines from a real SSE event
        assert!(parser.feed_line("event: content_block_delta").is_none());
        assert!(parser
            .feed_line(r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#)
            .is_none());

        // Blank line dispatches the event
        let event = parser.feed_line("").expect("should dispatch event");
        assert_eq!(event.event_type, "content_block_delta");

        let stream_event: StreamEvent =
            serde_json::from_str(&event.data).expect("should parse JSON");

        match stream_event {
            StreamEvent::ContentBlockDelta {
                index,
                delta: Delta::TextDelta { text },
            } => {
                assert_eq!(index, 0);
                assert_eq!(text, "Hello");
            }
            other => panic!("unexpected event: {:?}", other),
        }
    }

    #[test]
    fn test_comment_lines_ignored() {
        let mut parser = SseParser::new();
        assert!(parser.feed_line(": this is a keepalive comment").is_none());
        assert!(parser.feed_line("").is_none()); // No event to dispatch
    }

    #[test]
    fn test_multi_line_data() {
        let mut parser = SseParser::new();
        parser.feed_line("data: line one");
        parser.feed_line("data: line two");
        let event = parser.feed_line("").expect("should dispatch");
        assert_eq!(event.data, "line one\nline two");
    }
}
```

::: wild In the Wild
Claude Code implements a custom SSE parser that is tightly coupled with its event routing system. Rather than parsing all events into a generic struct and then dispatching, it routes events to specialized handlers as soon as the event type line is parsed -- before the data line even arrives. This lets it show "tool call starting" indicators with zero additional latency. OpenCode uses Go's `bufio.Scanner` to read SSE lines and a similar two-phase parse approach.
:::

## Key Takeaways

- SSE is a simple text-based protocol: fields are `event`, `data`, `id`, and `retry`, with blank lines separating events.
- The Anthropic API emits a structured sequence of events: `message_start`, `content_block_start`, `content_block_delta` (the main payload carrier), `content_block_stop`, `message_delta`, and `message_stop`.
- Serde's `#[serde(tag = "type")]` attribute maps directly to the Anthropic event format, giving you type-safe deserialization of each event variant.
- Building your own SSE parser (rather than using a crate) gives you full control over error handling and lets you route events incrementally.
- Always test edge cases: comment lines, multi-line data fields, and empty events.
