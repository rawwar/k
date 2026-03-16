---
title: Server Sent Events
description: The SSE protocol specification in detail — event types, data fields, ID tracking, retry directives, and the text/event-stream content type.
---

# Server Sent Events

> **What you'll learn:**
> - The SSE wire format including event, data, id, and retry fields and how they combine into discrete events
> - How the Last-Event-ID header enables resumable streams after disconnections
> - The text/event-stream content type negotiation and how SSE fits into the HTTP request/response model

Now that you know *why* SSE is the dominant streaming protocol for LLM APIs, let's dig into *how* it works at the wire level. SSE is deceptively simple -- the specification fits on a few pages -- but the details matter. Incorrect parsing of multi-line data fields, missing BOM handling, or ignored retry directives will cause subtle bugs that only surface under production load. This subchapter gives you the specification knowledge you need to build a correct SSE parser in the next section.

## The Wire Format

An SSE stream is a series of **events** separated by blank lines (two consecutive newline characters). Each event consists of one or more **fields**, where each field is a line in the format `field_name: value`. The specification defines exactly four field names:

| Field | Purpose | Example |
|-------|---------|---------|
| `data` | The event payload | `data: {"text": "hello"}` |
| `event` | The event type name | `event: content_block_delta` |
| `id` | The event identifier for resumption | `id: evt_123` |
| `retry` | Reconnection delay in milliseconds | `retry: 3000` |

Here is a complete SSE stream showing all field types:

```
retry: 3000

event: message_start
id: evt_001
data: {"type":"message_start","message":{"id":"msg_abc","model":"claude-sonnet-4-20250514"}}

event: content_block_start
id: evt_002
data: {"type":"content_block_start","index":0,"content_block":{"type":"text"}}

event: content_block_delta
id: evt_003
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: content_block_delta
id: evt_004
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":", world!"}}

event: content_block_stop
id: evt_005
data: {"type":"content_block_stop","index":0}

event: message_stop
id: evt_006
data: {"type":"message_stop"}

```

A few rules govern how these fields combine into events:

**Blank lines dispatch events.** When the parser encounters a blank line (a line with zero characters before the newline), it dispatches the current event to the application and resets its internal buffers. If no `data` field has been set, the event is silently dropped.

**Multiple `data` fields concatenate with newlines.** If an event contains multiple `data:` lines, their values are joined with `\n` characters. This allows multi-line payloads:

```
data: first line
data: second line
data: third line

```

This produces a single event with data `"first line\nsecond line\nthird line"`.

**The `event` field sets the type.** If no `event:` field is present, the event type defaults to `"message"`. LLM APIs typically use named event types like `content_block_delta`, `message_start`, and `message_stop`.

**The `id` field sets the last event ID.** The client should store this value. If the connection drops, the client includes it in the `Last-Event-ID` header when reconnecting. The server can use this to resume the stream from the correct position.

**The `retry` field sets the reconnection delay.** The value is a number of milliseconds. When the connection drops, the client should wait at least this long before attempting to reconnect.

## Field Parsing Rules

The specification has precise rules for how lines are parsed into fields:

1. If a line starts with a colon (`:`), it is a **comment** and must be ignored. Comments are often used as heartbeat keepalives:

```
: heartbeat

```

2. If a line contains a colon, everything before the first colon is the field name, and everything after it (with a single leading space stripped if present) is the value:

```
data: hello world    -> field="data", value="hello world"
data:hello world     -> field="data", value="hello world"
data:  hello world   -> field="data", value=" hello world" (only ONE space stripped)
```

3. If a line does not contain a colon, the entire line is treated as the field name with an empty string value:

```
data                 -> field="data", value=""
```

4. Unknown field names are ignored. This allows the protocol to be extended without breaking existing parsers.

Let's encode these rules in Rust:

```rust
#[derive(Debug, Clone, Default)]
struct SseEvent {
    event_type: Option<String>,
    data: String,
    id: Option<String>,
    retry: Option<u64>,
}

fn parse_field(line: &str) -> Option<(&str, &str)> {
    // Comment lines start with ':'
    if line.starts_with(':') {
        return None;
    }

    if let Some(colon_pos) = line.find(':') {
        let field_name = &line[..colon_pos];
        let mut value = &line[colon_pos + 1..];
        // Strip a single leading space if present
        if value.starts_with(' ') {
            value = &value[1..];
        }
        Some((field_name, value))
    } else {
        // No colon: entire line is the field name, value is empty
        Some((line, ""))
    }
}
```

::: python Coming from Python
Python's `sseclient` library or `httpx-sse` handles SSE parsing for you:
```python
import httpx
from httpx_sse import connect_sse

with httpx.Client() as client:
    with connect_sse(client, "POST", url, json=payload) as event_source:
        for event in event_source.iter_sse():
            print(f"type={event.event}, data={event.data}")
```
In Rust, there is no single dominant SSE library with the same level of maturity, so you will often write your own parser. This gives you fine-grained control over memory allocation, error handling, and integration with your async runtime -- things that matter when you are processing thousands of events per conversation.
:::

## Content Type Negotiation

The SSE protocol requires the server to respond with `Content-Type: text/event-stream`. The client signals its willingness to accept an SSE stream by including this in the `Accept` header:

```
POST /v1/messages HTTP/1.1
Accept: text/event-stream
Content-Type: application/json

{"model":"claude-sonnet-4-20250514","stream":true,...}
```

If the server returns a different content type, the client should treat the response as an error, not an SSE stream. This is an important validation step:

```rust
async fn validate_sse_response(
    response: &reqwest::Response,
) -> Result<(), String> {
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !content_type.starts_with("text/event-stream") {
        return Err(format!(
            "Expected text/event-stream, got: {}",
            content_type
        ));
    }

    Ok(())
}
```

In practice, LLM APIs return `text/event-stream` when you set `"stream": true` in the request body. If you forget to set that flag, you get a regular JSON response with `Content-Type: application/json` instead.

## The BOM Issue

The SSE specification requires that if the stream begins with a UTF-8 Byte Order Mark (BOM, the bytes `0xEF 0xBB 0xBF`), the parser must strip it. While modern APIs rarely include a BOM, a robust parser must handle it:

```rust
fn strip_bom(data: &[u8]) -> &[u8] {
    if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &data[3..]
    } else {
        data
    }
}
```

This is a one-time operation at the start of the stream. After stripping the BOM, the parser processes lines normally.

## Event ID and Stream Resumption

The `id` field is the mechanism that makes SSE streams resumable. Here is the flow:

1. The server sends events with `id` fields: `id: evt_004`
2. The client stores the most recent `id` value as its "last event ID."
3. If the connection drops, the client reconnects and includes a `Last-Event-ID: evt_004` header.
4. The server uses this ID to resume the stream from the event after `evt_004`.

```rust
struct SseClient {
    last_event_id: Option<String>,
    retry_ms: u64,
}

impl SseClient {
    fn new() -> Self {
        Self {
            last_event_id: None,
            retry_ms: 3000, // default
        }
    }

    fn process_event(&mut self, event: &SseEvent) {
        // Update last event ID if present
        if let Some(ref id) = event.id {
            self.last_event_id = Some(id.clone());
        }

        // Update retry interval if present
        if let Some(retry) = event.retry {
            self.retry_ms = retry;
        }
    }

    fn reconnect_headers(&self) -> Vec<(&str, String)> {
        let mut headers = vec![];
        if let Some(ref id) = self.last_event_id {
            headers.push(("Last-Event-ID", id.clone()));
        }
        headers
    }
}
```

Not all LLM APIs support stream resumption via `Last-Event-ID`. Anthropic's API uses event IDs for tracking but does not currently support mid-stream resumption. However, implementing ID tracking costs nothing and prepares your parser for servers that do support it.

## LLM-Specific Event Patterns

Different LLM providers structure their SSE events differently, but the pattern is consistent. Here is what a typical Anthropic streaming response looks like:

```
event: message_start
data: {"type":"message_start","message":{"id":"msg_01XFDUDYJgAACzvnptvVoYEL","type":"message","role":"assistant","content":[],"model":"claude-sonnet-4-20250514","stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":25,"output_tokens":1}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"!"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":3}}

event: message_stop
data: {"type":"message_stop"}

```

The lifecycle is: `message_start` -> one or more content blocks (each with `start`, `delta`s, `stop`) -> `message_delta` (with final usage) -> `message_stop`. Tool use blocks follow the same pattern but with `type: "tool_use"` instead of `type: "text"`.

::: wild In the Wild
Claude Code uses the streaming event types to drive its UI state machine. When a `content_block_start` with `type: "tool_use"` arrives, it transitions the display from text rendering to showing a tool call in progress. The `content_block_delta` events for tool calls carry partial JSON arguments, which Claude Code accumulates until the `content_block_stop` signals that the complete tool call is available for execution. This event-driven approach keeps the UI responsive and informative throughout the streaming process.
:::

## Key Takeaways

- SSE events consist of four fields -- `data`, `event`, `id`, and `retry` -- separated by blank lines. Only `data` is required for an event to be dispatched.
- Multiple `data:` lines within a single event are concatenated with newline characters, enabling multi-line payloads.
- Comment lines (starting with `:`) are silently ignored and commonly used as heartbeat keepalives to prevent connection timeouts.
- The `id` field enables stream resumption via the `Last-Event-ID` header on reconnection, though not all LLM APIs support mid-stream resumption.
- LLM APIs use named event types (`message_start`, `content_block_delta`, etc.) to signal lifecycle transitions that drive the agent's UI state machine.
