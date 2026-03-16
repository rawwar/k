---
title: HTTP Streaming Protocols
description: Overview of HTTP/1.1 and HTTP/2 streaming mechanisms including chunked transfer encoding, long polling, WebSockets, and server-sent events.
---

# HTTP Streaming Protocols

> **What you'll learn:**
> - The landscape of HTTP streaming options and how they differ in capability, complexity, and browser/client support
> - How HTTP/1.1 chunked transfer encoding enables streaming without knowing the total response size in advance
> - Why server-sent events emerged as the dominant protocol for LLM API streaming and its advantages over WebSockets

Before you can stream LLM responses in Rust, you need to understand what is happening at the protocol level. HTTP was originally designed for request-response interactions: the client sends a request, the server sends back a complete response, and the connection closes. Streaming data over HTTP requires bending this model, and there are several ways to do it. Each makes different trade-offs between simplicity, capability, and compatibility. Let's walk through them and see why the LLM ecosystem converged on the choices it did.

## Long Polling: The Simplest Hack

Long polling is the oldest trick for simulating real-time communication over HTTP. The client sends a request, and the server holds the connection open until it has data to send. When data arrives, the server responds, and the client immediately sends a new request to wait for more data.

```rust
use reqwest::Client;

async fn long_poll_loop(client: &Client, url: &str) {
    loop {
        // Each request blocks until the server has new data
        let response = client
            .get(url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await;

        match response {
            Ok(resp) => {
                let body = resp.text().await.unwrap();
                println!("Received: {}", body);
                // Immediately start the next poll
            }
            Err(e) if e.is_timeout() => {
                // Timeout is normal -- server had nothing to send
                continue;
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }
}
```

Long polling works, but it has significant drawbacks for LLM streaming. Each "poll" is a complete HTTP request-response cycle, which adds overhead for headers, TCP round-trips, and connection setup. For token-by-token streaming where you might receive 50+ events per second, long polling is absurdly wasteful. No production LLM API uses it.

## WebSockets: Full-Duplex Communication

WebSockets upgrade an HTTP connection to a full-duplex communication channel. Once the upgrade handshake completes, both client and server can send messages at any time without the overhead of HTTP headers on each message.

```rust
use tokio_tungstenite::connect_async;
use futures::{StreamExt, SinkExt};

async fn websocket_stream(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let (mut ws_stream, _) = connect_async(url).await?;

    // Send a message to the server
    ws_stream
        .send(tokio_tungstenite::tungstenite::Message::Text(
            r#"{"prompt": "Explain Rust ownership"}"#.into(),
        ))
        .await?;

    // Receive messages as they arrive
    while let Some(msg) = ws_stream.next().await {
        match msg? {
            tokio_tungstenite::tungstenite::Message::Text(text) => {
                println!("Token: {}", text);
            }
            tokio_tungstenite::tungstenite::Message::Close(_) => break,
            _ => {}
        }
    }

    Ok(())
}
```

WebSockets are powerful -- they support bidirectional communication, binary frames, and low per-message overhead. So why don't most LLM APIs use them? Several reasons:

**Complexity.** WebSockets require connection upgrade negotiation, frame parsing, ping/pong heartbeats, and custom reconnection logic. For a unidirectional stream (server sends tokens, client just listens), this is overkill.

**Proxy compatibility.** Many corporate proxies, load balancers, and CDNs struggle with WebSocket connections. They expect HTTP request-response patterns and may drop or buffer WebSocket frames unpredictably.

**No automatic reconnection.** When a WebSocket connection drops, you must reestablish it from scratch. The protocol provides no built-in mechanism for resuming from where you left off.

**Stateful servers.** WebSocket connections are inherently stateful on the server side. The server must maintain the connection context in memory for the duration. This makes horizontal scaling and load balancing more complicated than stateless HTTP.

Some LLM providers do offer WebSocket endpoints -- notably for real-time voice APIs where bidirectional streaming is essential. But for text generation, simpler solutions dominate.

::: python Coming from Python
In Python, you might use `websockets` or `aiohttp` for WebSocket connections:
```python
import websockets

async with websockets.connect(url) as ws:
    await ws.send('{"prompt": "Hello"}')
    async for message in ws:
        print(message)
```
The Rust `tokio-tungstenite` crate serves the same role, but you interact with the WebSocket through `futures::Stream` and `futures::Sink` traits instead of Python's async iterator protocol. The pattern is similar -- send a message, then iterate over incoming messages -- but Rust's ownership model ensures that only one task can write to the socket at a time.
:::

## Server-Sent Events: The Sweet Spot

Server-Sent Events (SSE) hit the sweet spot for LLM streaming. SSE is a simple protocol built on plain HTTP: the server sends a stream of text events over a standard HTTP response. The client opens a regular HTTP connection, and the server writes events to it as they become available.

Here is what an SSE stream looks like on the wire:

```
HTTP/1.1 200 OK
Content-Type: text/event-stream
Cache-Control: no-cache

event: message_start
data: {"type":"message_start","message":{"id":"msg_01","model":"claude-sonnet-4-20250514"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Rust"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"'s ownership"}}

event: message_stop
data: {"type":"message_stop"}

```

SSE wins for LLM streaming because of several properties:

**Plain HTTP.** SSE uses standard HTTP responses. Every proxy, load balancer, and CDN understands HTTP. There is no upgrade handshake, no frame encoding, no binary protocol.

**Automatic reconnection.** The SSE specification includes `retry` and `id` fields that allow the client to automatically reconnect and resume from the last received event. We will explore this in [Reconnection Strategies](/linear/08-streaming-and-realtime/10-reconnection-strategies).

**Text-based and debuggable.** You can observe an SSE stream with `curl` and read it with your eyes. Try that with a WebSocket binary frame.

**Unidirectional by design.** For LLM APIs, the client sends one request (the prompt) and receives a stream of response tokens. SSE maps perfectly to this pattern -- it is server-to-client only, which matches the LLM generation flow exactly.

**Simple parsing.** SSE events are delimited by blank lines, with fields separated by newlines. You can parse SSE with a state machine that fits in a few dozen lines of code.

## How SSE Rides on HTTP

SSE is not a separate protocol -- it is a convention for how the server formats the body of an HTTP response. The client makes a standard HTTP request:

```
POST /v1/messages HTTP/1.1
Host: api.anthropic.com
Content-Type: application/json
Accept: text/event-stream

{"model":"claude-sonnet-4-20250514","stream":true,"messages":[...]}
```

The server responds with `Content-Type: text/event-stream` and keeps the connection open. The response body is a series of text events, each separated by a blank line. Under the hood, the response body is delivered using HTTP chunked transfer encoding (which we will cover in detail in the [next subchapter](/linear/08-streaming-and-realtime/04-chunked-encoding)).

Here is how this looks in Rust with `reqwest`:

```rust
use reqwest::Client;
use futures::StreamExt;

async fn sse_stream(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("content-type", "application/json")
        .header("accept", "text/event-stream")
        .header("x-api-key", "your-key")
        .header("anthropic-version", "2023-06-01")
        .body(r#"{"model":"claude-sonnet-4-20250514","stream":true,"max_tokens":1024,"messages":[{"role":"user","content":"Hello"}]}"#)
        .send()
        .await?;

    // reqwest gives us a byte stream
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        let text = String::from_utf8_lossy(&bytes);
        // Raw SSE data -- we will parse this properly in chapter 8.5
        print!("{}", text);
    }

    Ok(())
}
```

## Protocol Comparison Table

| Feature | Long Polling | WebSockets | SSE |
|---------|-------------|------------|-----|
| Direction | Client-initiated | Bidirectional | Server-to-client |
| Transport | HTTP (repeated) | Upgraded TCP | HTTP (single) |
| Reconnection | Manual | Manual | Built-in (retry/id) |
| Proxy support | Excellent | Poor | Excellent |
| Per-message overhead | Full HTTP headers | 2-14 bytes | ~20 bytes |
| Parsing complexity | Low | Medium | Low |
| LLM API adoption | None | Rare (voice) | Universal (text) |

::: wild In the Wild
Anthropic's Messages API, OpenAI's Chat Completions API, and Google's Gemini API all use SSE for streaming text responses. The convergence is not accidental -- SSE's simplicity and HTTP compatibility make it the natural choice for a unidirectional token stream. When you see `"stream": true` in an LLM API request, the response will almost certainly be `text/event-stream`. Claude Code, OpenCode, and Codex all implement SSE parsers as a core part of their streaming infrastructure.
:::

## HTTP/2 and Multiplexing

HTTP/2 adds an important capability: stream multiplexing. Multiple logical streams share a single TCP connection, each with its own flow control. This means you can have multiple SSE streams open simultaneously without consuming multiple TCP connections.

For a coding agent, this matters when you have concurrent operations -- for example, streaming an LLM response while simultaneously making a tool call that also streams its output. With HTTP/1.1, each SSE stream requires its own TCP connection. With HTTP/2, they share one.

`reqwest` supports HTTP/2 out of the box. When the server supports it, the connection is automatically upgraded:

```rust
let client = reqwest::Client::builder()
    .http2_prior_knowledge() // Force HTTP/2 if you know the server supports it
    .build()?;
```

In practice, most LLM APIs support HTTP/2, and `reqwest` negotiates it automatically via ALPN during the TLS handshake. You do not need to do anything special -- the multiplexing happens transparently at the connection level.

## Choosing Your Protocol

For a CLI coding agent that communicates with LLM APIs, the choice is clear: **SSE over HTTP**. It is what every major LLM provider supports, it is simple to parse, it handles reconnection gracefully, and it works through any network infrastructure.

The rest of this chapter focuses on SSE as the streaming protocol. You will learn the specification in detail, build a parser in Rust, and layer application logic on top. But understanding the alternatives helps you appreciate *why* SSE was chosen, and it prepares you for edge cases like real-time voice (WebSockets) or legacy integrations (long polling) that you might encounter outside the LLM domain.

## Key Takeaways

- **Long polling** simulates streaming with repeated HTTP requests but adds unacceptable overhead for high-frequency token streams.
- **WebSockets** provide full-duplex communication but are overly complex for unidirectional LLM streaming and suffer from proxy compatibility issues.
- **Server-Sent Events (SSE)** are the universal standard for LLM API streaming because they use plain HTTP, support automatic reconnection, and are trivial to parse and debug.
- **HTTP/2 multiplexing** allows multiple SSE streams over a single TCP connection, which matters when your agent runs concurrent operations.
- Every major LLM provider (Anthropic, OpenAI, Google) uses SSE for text streaming -- building a solid SSE implementation is the single most important streaming investment for your agent.
