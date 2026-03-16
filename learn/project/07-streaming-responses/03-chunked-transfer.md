---
title: Chunked Transfer
description: Handle HTTP chunked transfer encoding to receive streaming data incrementally from the API without buffering the entire response.
---

# Chunked Transfer

> **What you'll learn:**
> - How HTTP chunked transfer encoding delivers data in variable-size pieces
> - How to use reqwest's streaming body to read chunks as they arrive
> - How to handle chunk boundaries that split SSE events mid-line

In the previous subchapter you built an SSE parser that processes complete lines. But HTTP does not deliver data line-by-line -- it delivers it in chunks. A single HTTP chunk might contain three complete SSE events, half an event, or even split a UTF-8 character across two chunks. This subchapter bridges the gap between raw HTTP bytes and the clean lines your SSE parser expects.

## How chunked transfer encoding works

When a server does not know the total response size in advance (because it is generating tokens on the fly), it uses HTTP chunked transfer encoding. Instead of a `Content-Length` header, the response includes `Transfer-Encoding: chunked`. The body arrives as a series of chunks, each prefixed with its size in hexadecimal:

```
HTTP/1.1 200 OK
Content-Type: text/event-stream
Transfer-Encoding: chunked

1a\r\n
event: message_start\n\r\n
2f\r\n
data: {"type":"message_start"}\n\n\r\n
0\r\n
\r\n
```

The good news is that `reqwest` handles the chunk framing for you -- you never see the hex size prefixes. But you do receive data in arbitrary-sized pieces that do not align with logical boundaries. A single `chunk()` call might return:

```
"event: content_block_del"
```

And the next call returns:

```
"ta\ndata: {\"type\":\"content_block_delta\"}\n\n"
```

Your job is to stitch these chunks back into complete lines before feeding them to the SSE parser.

## Streaming with reqwest

Let's set up the streaming HTTP request. You need to change your API call to request streaming and read the response body incrementally:

```rust
use bytes::Bytes;
use futures::StreamExt;
use reqwest::Client;

pub async fn start_streaming_request(
    client: &Client,
    api_key: &str,
    messages: &[Message],
) -> Result<impl futures::Stream<Item = Result<Bytes, reqwest::Error>>, reqwest::Error> {
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 4096,
            "stream": true,
            "messages": messages,
        }))
        .send()
        .await?
        .error_for_status()?;

    Ok(response.bytes_stream())
}
```

The key difference from your Chapter 2 code is `"stream": true` in the request body and calling `.bytes_stream()` instead of `.json()` on the response. The `.bytes_stream()` method returns a `Stream` of `Bytes` chunks -- Rust's async equivalent of an iterator that yields data as it arrives from the network.

::: python Coming from Python
In Python with `httpx`, streaming looks like this:
```python
with httpx.stream("POST", url, json=body, headers=headers) as response:
    for chunk in response.iter_bytes():
        process(chunk)
```
The Rust version uses `futures::Stream` instead of a Python iterator. The key difference is that Rust's stream is lazy and async -- it only polls the network when you call `.next().await`, giving you precise control over when data is consumed.
:::

## The line splitter

Between raw HTTP chunks and the SSE parser, you need a component that buffers partial data and emits complete lines. Here is a `LineSplitter` that handles this:

```rust
/// Buffers incoming byte chunks and yields complete lines.
/// Handles the case where a line is split across multiple chunks.
pub struct LineSplitter {
    buffer: String,
}

impl LineSplitter {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    /// Feed a chunk of bytes into the splitter.
    /// Returns a Vec of complete lines (without trailing newlines).
    /// Incomplete lines remain buffered until the next chunk arrives.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<String> {
        // Convert bytes to string, handling potential UTF-8 issues
        let text = match std::str::from_utf8(chunk) {
            Ok(s) => s.to_string(),
            Err(_) => String::from_utf8_lossy(chunk).to_string(),
        };

        self.buffer.push_str(&text);

        let mut lines = Vec::new();

        // Process all complete lines in the buffer
        while let Some(newline_pos) = self.buffer.find('\n') {
            let line = self.buffer[..newline_pos].to_string();
            // Remove the line and the newline character from the buffer
            self.buffer = self.buffer[newline_pos + 1..].to_string();
            // Strip trailing \r if present (handles \r\n line endings)
            let line = line.strip_suffix('\r').unwrap_or(&line).to_string();
            lines.push(line);
        }

        lines
    }

    /// Check if there is any remaining data in the buffer.
    /// Call this when the stream ends to handle the last line
    /// if it was not terminated by a newline.
    pub fn finish(&mut self) -> Option<String> {
        if self.buffer.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.buffer))
        }
    }
}
```

This handles three important edge cases:

1. **Multiple lines in one chunk** -- the `while` loop extracts all complete lines.
2. **Lines split across chunks** -- the buffer retains the partial line until the next chunk completes it.
3. **Windows-style line endings** -- stripping `\r` handles both `\n` and `\r\n`.

## Connecting chunks to SSE events

Now let's wire the `LineSplitter` and `SseParser` together to process a streaming response end-to-end:

```rust
use crate::sse::{SseParser, StreamEvent};
use futures::StreamExt;

pub async fn process_stream(
    mut byte_stream: impl futures::Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
) -> Result<Vec<StreamEvent>, Box<dyn std::error::Error>> {
    let mut splitter = LineSplitter::new();
    let mut parser = SseParser::new();
    let mut events = Vec::new();

    while let Some(chunk_result) = byte_stream.next().await {
        let chunk = chunk_result?;
        let lines = splitter.feed(&chunk);

        for line in lines {
            if let Some(sse_event) = parser.feed_line(&line) {
                // Skip ping events
                if sse_event.event_type == "ping" {
                    continue;
                }

                let stream_event = sse_event.into_stream_event()?;
                events.push(stream_event);
            }
        }
    }

    // Handle any remaining buffered data
    if let Some(last_line) = splitter.finish() {
        if let Some(sse_event) = parser.feed_line(&last_line) {
            if sse_event.event_type != "ping" {
                let stream_event = sse_event.into_stream_event()?;
                events.push(stream_event);
            }
        }
    }

    Ok(events)
}
```

This collects all events into a `Vec`, which you will replace with real-time processing in the next subchapter. But it demonstrates the three-layer pipeline:

```
HTTP bytes ──> LineSplitter ──> SseParser ──> StreamEvent
   (chunks)      (lines)        (events)     (typed data)
```

## Handling tricky chunk boundaries

Let's look at a realistic scenario where chunk boundaries cause problems. Imagine the server sends this SSE data in two chunks:

**Chunk 1:**
```
event: content_block_delta\ndata: {"type":"content_block_delt
```

**Chunk 2:**
```
a","index":0,"delta":{"type":"text_delta","text":"Hi"}}\n\n
```

The `LineSplitter` handles this correctly:

1. **Chunk 1 arrives:** The splitter finds one `\n` and emits the line `"event: content_block_delta"`. The remaining text `"data: {\"type\":\"content_block_delt"` stays in the buffer.
2. **Chunk 2 arrives:** The splitter prepends the buffer to get the complete data line, finds `\n`, emits it, then finds the blank line `""` (from the double newline).
3. **The SSE parser** receives three lines: the event line, the data line, and the blank line. It dispatches a complete event.

Let's verify this with a test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_across_chunks() {
        let mut splitter = LineSplitter::new();

        // First chunk: one complete line, one partial
        let lines1 = splitter.feed(
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delt",
        );
        assert_eq!(lines1, vec!["event: content_block_delta"]);

        // Second chunk: completes the partial line plus a blank line
        let lines2 = splitter.feed(
            b"a\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\n\n",
        );
        assert_eq!(lines2.len(), 2);
        assert!(lines2[0].starts_with("data: "));
        assert_eq!(lines2[1], ""); // blank line triggers event dispatch
    }

    #[test]
    fn test_multiple_events_in_one_chunk() {
        let mut splitter = LineSplitter::new();
        let mut parser = SseParser::new();

        // One chunk containing two complete events
        let chunk = b"event: ping\ndata: {\"type\":\"ping\"}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
        let lines = splitter.feed(chunk);

        let mut events = Vec::new();
        for line in lines {
            if let Some(event) = parser.feed_line(&line) {
                events.push(event);
            }
        }

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "ping");
        assert_eq!(events[1].event_type, "message_stop");
    }
}
```

::: wild In the Wild
Production agents handle chunk boundaries carefully because they affect perceived latency. If a token arrives at the end of a chunk but the newline that completes the SSE event is in the next chunk, the token display is delayed until that next chunk arrives. Claude Code optimizes for this by processing partial SSE data eagerly -- it can detect that a data line is accumulating and pre-parse the JSON to extract the text delta before the event is formally complete. This shaves a few milliseconds off each token's display time.
:::

## Key Takeaways

- HTTP chunked transfer encoding delivers data in arbitrary-size pieces that do not align with SSE event boundaries.
- A `LineSplitter` buffers partial data between chunks and emits complete lines, bridging raw bytes and the SSE parser.
- The three-layer pipeline -- `HTTP chunks -> LineSplitter -> SseParser -> StreamEvent` -- cleanly separates concerns and makes each layer independently testable.
- Always handle the edge case where lines, events, or even UTF-8 characters are split across chunk boundaries.
- Using `reqwest`'s `.bytes_stream()` gives you a `futures::Stream` that yields chunks as they arrive, without buffering the entire response body.
