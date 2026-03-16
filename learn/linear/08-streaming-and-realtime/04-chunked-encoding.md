---
title: Chunked Encoding
description: How HTTP chunked transfer encoding works at the byte level, including chunk size parsing, trailer headers, and interaction with compression.
---

# Chunked Encoding

> **What you'll learn:**
> - The chunked transfer encoding format with hex size prefixes, CRLF delimiters, and the zero-length terminator
> - How chunked encoding interacts with content encoding (gzip/br) and why decompression must happen after dechunking
> - Common pitfalls when parsing chunked streams including incomplete chunks and buffer boundary alignment

SSE provides the application-level framing for your streaming events, but underneath it, the raw bytes travel over HTTP using **chunked transfer encoding**. This is the mechanism that allows the server to send a response without knowing its total size in advance -- essential for streaming, where the response grows as the LLM generates more tokens. You typically do not parse chunked encoding yourself (libraries like `reqwest` and `hyper` handle it), but understanding how it works helps you debug streaming issues, interpret network traces, and understand performance characteristics.

## Why Chunked Encoding Exists

In traditional HTTP, the server includes a `Content-Length` header telling the client exactly how many bytes to expect:

```
HTTP/1.1 200 OK
Content-Length: 42
Content-Type: application/json

{"message": "This response is 42 bytes."}
```

But when streaming LLM responses, the server does not know how many tokens the model will generate. The response might be 500 bytes or 50,000 bytes. Chunked transfer encoding solves this by letting the server send the response in pieces, each prefixed with its size:

```
HTTP/1.1 200 OK
Transfer-Encoding: chunked
Content-Type: text/event-stream

1a\r\n
event: content_block_delta\r\n
\r\n
2f\r\n
data: {"type":"text_delta","text":"Hello"}\r\n
\r\n
0\r\n
\r\n
```

The server sends each chunk with a hex size prefix, and the client reconstructs the full response by concatenating the chunk bodies. A zero-length chunk signals the end of the response.

## The Chunk Format

Each chunk follows this exact byte-level format:

```
<chunk-size-in-hex>\r\n
<chunk-data>\r\n
```

And the stream ends with:

```
0\r\n
\r\n
```

Let's break down a real chunked response byte by byte. Consider a server streaming two SSE events:

```
HTTP/1.1 200 OK
Transfer-Encoding: chunked
Content-Type: text/event-stream

4f\r\n
event: message_start\r\ndata: {"type":"message_start","message":{"id":"msg_01"}}\r\n\r\n\r\n
3a\r\n
event: content_block_delta\r\ndata: {"text":"Hello"}\r\n\r\n\r\n
0\r\n
\r\n
```

The hex value `4f` is 79 in decimal -- that is how many bytes are in the first chunk body. The hex value `3a` is 58 in decimal. The parser reads the hex size, reads exactly that many bytes, skips the trailing `\r\n`, and repeats.

Here is a minimal parser that demonstrates the logic, even though you would never write this in production (your HTTP library does it for you):

```rust
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

struct ChunkedReader<R> {
    inner: BufReader<R>,
    remaining_in_chunk: usize,
    done: bool,
}

impl<R: tokio::io::AsyncRead + Unpin> ChunkedReader<R> {
    fn new(reader: R) -> Self {
        Self {
            inner: BufReader::new(reader),
            remaining_in_chunk: 0,
            done: false,
        }
    }

    async fn read_next_chunk(&mut self) -> Option<Vec<u8>> {
        if self.done {
            return None;
        }

        // Read the chunk size line (hex digits followed by \r\n)
        let mut size_line = String::new();
        self.inner.read_line(&mut size_line).await.ok()?;
        let size_str = size_line.trim();

        // Parse hex size
        let size = usize::from_str_radix(size_str, 16).ok()?;

        if size == 0 {
            self.done = true;
            // Read the trailing \r\n after the zero chunk
            let mut trailer = String::new();
            let _ = self.inner.read_line(&mut trailer).await;
            return None;
        }

        // Read exactly `size` bytes of chunk data
        let mut data = vec![0u8; size];
        self.inner.read_exact(&mut data).await.ok()?;

        // Skip the trailing \r\n after the chunk data
        let mut crlf = [0u8; 2];
        self.inner.read_exact(&mut crlf).await.ok()?;

        Some(data)
    }
}
```

::: python Coming from Python
Python's `httpx` and `requests` libraries handle chunked decoding transparently:
```python
import httpx

with httpx.stream("POST", url, json=payload) as response:
    for chunk in response.iter_bytes():
        # chunk is already dechunked -- you never see the hex size prefixes
        process(chunk)
```
Rust's `reqwest` does the same -- when you call `response.bytes_stream()`, the chunked encoding has already been decoded by `hyper` underneath. The chunks you receive are application-level data, not HTTP-level chunks. Understanding the encoding helps you debug with tools like `tcpdump` or Wireshark, but your application code never touches it directly.
:::

## Chunk Boundaries vs. Event Boundaries

Here is a critical concept: **HTTP chunk boundaries do not align with SSE event boundaries.** A single HTTP chunk might contain multiple SSE events, or a single SSE event might span multiple HTTP chunks.

Consider an SSE event that is 200 bytes. If the server's TCP send buffer happens to flush at 128 bytes, you will receive:

- HTTP Chunk 1: the first 128 bytes (contains the start of the event)
- HTTP Chunk 2: the remaining 72 bytes (contains the end of the event)

Or the server might batch several small events into one chunk:

- HTTP Chunk 1: three complete events totaling 500 bytes

This misalignment is why your SSE parser must operate on a **line-by-line basis**, accumulating a buffer and scanning for complete lines and blank-line delimiters, rather than assuming each chunk contains exactly one event:

```rust
struct SseLineBuffer {
    buffer: String,
}

impl SseLineBuffer {
    fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    /// Feed raw bytes from the HTTP stream and extract complete lines.
    fn feed(&mut self, data: &[u8]) -> Vec<String> {
        let text = String::from_utf8_lossy(data);
        self.buffer.push_str(&text);

        let mut lines = Vec::new();

        // Extract complete lines (terminated by \n)
        while let Some(newline_pos) = self.buffer.find('\n') {
            let line = self.buffer[..newline_pos].trim_end_matches('\r').to_string();
            lines.push(line);
            self.buffer = self.buffer[newline_pos + 1..].to_string();
        }

        lines
    }
}
```

This line buffer sits between the HTTP layer (which gives you arbitrary byte chunks) and the SSE parser (which needs complete lines). It handles the case where a line is split across two HTTP chunks by accumulating partial data.

## Chunked Encoding and Compression

Many servers compress their responses with gzip or Brotli for bandwidth efficiency. When a response is both chunked and compressed, the **compression is applied to the message body before chunking**. The decompression order is:

1. Dechunk: remove the hex size prefixes and concatenate chunk bodies
2. Decompress: apply gzip/brotli decompression to the concatenated data
3. Parse: interpret the decompressed text as SSE events

This is specified by the HTTP `Content-Encoding` header:

```
HTTP/1.1 200 OK
Transfer-Encoding: chunked
Content-Encoding: gzip
Content-Type: text/event-stream
```

`reqwest` handles this automatically when you enable the `gzip` or `brotli` features:

```rust
let client = reqwest::Client::builder()
    .gzip(true)
    .brotli(true)
    .build()?;
```

With these features enabled, `response.bytes_stream()` returns decompressed, dechunked data. You work with plain text SSE events.

However, compression can interact poorly with streaming. A compressor works best with large blocks of data -- it needs to see repeated patterns to compress effectively. With streaming, each chunk might be tiny (a single SSE event of 50-100 bytes), and the compressor cannot compress efficiently at that granularity. Some servers disable compression for SSE streams, or use flush-friendly compression settings that sacrifice compression ratio for lower latency.

## Trailer Headers

The chunked encoding specification allows **trailer headers** after the zero-length terminator chunk:

```
0\r\n
X-Stream-Duration: 4523\r\n
X-Token-Count: 847\r\n
\r\n
```

Trailer headers are declared in a `Trailer` response header at the start:

```
HTTP/1.1 200 OK
Transfer-Encoding: chunked
Trailer: X-Stream-Duration, X-Token-Count
```

LLM APIs could theoretically use trailers to send final usage statistics after the stream completes. In practice, most APIs include this information in the final SSE event (like `message_delta` with usage data) rather than as HTTP trailers, because trailer support across HTTP clients and proxies is inconsistent.

## Debugging Chunked Streams

When you need to see the raw chunked encoding on the wire, `curl` with verbose mode is your friend:

```bash
curl -v --no-buffer \
  -H "Content-Type: application/json" \
  -H "Accept: text/event-stream" \
  -d '{"model":"claude-sonnet-4-20250514","stream":true,"max_tokens":100,"messages":[{"role":"user","content":"Hi"}]}' \
  https://api.anthropic.com/v1/messages
```

The `--no-buffer` flag tells curl not to buffer the output, so you see chunks as they arrive. The `-v` flag shows the HTTP headers, including `Transfer-Encoding: chunked`.

For deeper inspection, Wireshark or `tcpdump` with a display filter for HTTP will show you the actual chunk boundaries:

```bash
tcpdump -i any -A 'host api.anthropic.com and port 443' -w stream.pcap
```

These tools are invaluable when debugging timing issues -- for example, when you suspect the server is batching multiple events into one chunk, causing bursty rendering.

::: wild In the Wild
Most production coding agents do not deal with chunked encoding directly -- they rely on their HTTP library to handle it. However, OpenCode's Go implementation and Claude Code both had to handle edge cases around chunked encoding when operating behind certain corporate proxies that rebuffer the chunked stream, introducing artificial latency. If your agent will run in corporate environments, be aware that proxies can silently alter the chunking behavior of HTTP responses.
:::

## Key Takeaways

- Chunked transfer encoding lets the server stream a response without knowing its total size, using hex-prefixed chunks terminated by a zero-length chunk.
- **HTTP chunk boundaries do not align with SSE event boundaries** -- your SSE parser must buffer and split on line boundaries, not chunk boundaries.
- Compression (gzip/brotli) is applied before chunking, so decompression must happen after dechunking. `reqwest` handles both transparently.
- Trailer headers are part of the chunked encoding spec but rarely used by LLM APIs, which prefer to include final metadata in the last SSE event.
- You rarely parse chunked encoding yourself, but understanding the format helps you debug streaming issues with tools like `curl -v`, `tcpdump`, and Wireshark.
