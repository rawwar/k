---
title: Backpressure
description: Implement backpressure mechanisms to prevent memory exhaustion when the terminal renders slower than tokens arrive.
---

# Backpressure

> **What you'll learn:**
> - How producer-consumer speed mismatches lead to unbounded memory growth
> - How to implement bounded channels between the stream reader and the UI renderer
> - How to apply backpressure without dropping tokens or corrupting the response

Your streaming pipeline has a producer (the network, delivering tokens) and a consumer (the terminal, rendering them). What happens when the producer is faster than the consumer? Without backpressure, unrendered tokens accumulate in memory without bound. On a fast API connection feeding a slow terminal (SSH over cellular, a resource-constrained container), this can exhaust memory within seconds. Backpressure is the mechanism that tells the producer to slow down.

## Understanding the mismatch

Consider the numbers. The Anthropic API can deliver tokens at 50-80 tokens per second. Each `content_block_delta` event, fully parsed, might be 200 bytes of overhead for a 5-character token. At 80 tokens/s, that is 16 KB/s of raw data -- trivial for a local terminal. But rendering involves:

1. Writing to stdout (a system call).
2. The terminal emulator processing the character.
3. The display refreshing.

Over SSH with 200ms round-trip latency and TCP Nagle buffering, each `flush()` can take 50-200ms. If you flush per token at 80 tokens/s, you need 80 flushes per second, each potentially blocking for 50ms. You can only flush 20 times per second. The remaining 60 tokens per second pile up.

Without backpressure, after 60 seconds of streaming you would have ~3600 tokens waiting in a buffer. For a very long response, this grows without bound.

## Bounded channels

The standard solution is to put a bounded channel between the producer and consumer. When the channel is full, the producer blocks until the consumer catches up. Here is the pattern using Tokio's `mpsc` channel:

```rust
use tokio::sync::mpsc;

/// Events that flow from the stream reader to the renderer.
#[derive(Debug)]
pub enum RenderEvent {
    /// A text token to display.
    TextDelta(String),
    /// A tool call started (show indicator).
    ToolCallStarted { name: String },
    /// The stream is complete.
    Done { stop_reason: Option<String> },
    /// An error occurred.
    Error(String),
}

/// Creates a bounded channel pair for stream-to-renderer communication.
/// `buffer_size` controls how many events can queue before backpressure kicks in.
pub fn create_render_channel(buffer_size: usize) -> (mpsc::Sender<RenderEvent>, mpsc::Receiver<RenderEvent>) {
    mpsc::channel(buffer_size)
}
```

Now split your pipeline into two tasks: one reads from the network and sends events, the other receives events and renders:

```rust
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// The producer: reads SSE events and sends render events through the channel.
pub async fn stream_reader_task(
    mut byte_stream: impl futures::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
    tx: mpsc::Sender<RenderEvent>,
    cancel_token: CancellationToken,
) {
    let mut splitter = LineSplitter::new();
    let mut parser = SseParser::new();

    loop {
        let chunk = tokio::select! {
            chunk = futures::StreamExt::next(&mut byte_stream) => {
                match chunk {
                    Some(Ok(bytes)) => bytes,
                    Some(Err(e)) => {
                        let _ = tx.send(RenderEvent::Error(e.to_string())).await;
                        return;
                    }
                    None => break,
                }
            }
            _ = cancel_token.cancelled() => break,
        };

        for line in splitter.feed(&chunk) {
            let Some(sse_event) = parser.feed_line(&line) else { continue };
            if sse_event.event_type == "ping" { continue; }

            let stream_event: StreamEvent = match serde_json::from_str(&sse_event.data) {
                Ok(e) => e,
                Err(e) => {
                    let _ = tx.send(RenderEvent::Error(e.to_string())).await;
                    return;
                }
            };

            match stream_event {
                StreamEvent::ContentBlockDelta {
                    delta: Delta::TextDelta { text }, ..
                } => {
                    // This `.send().await` blocks if the channel is full (backpressure!)
                    if tx.send(RenderEvent::TextDelta(text)).await.is_err() {
                        return; // Receiver dropped, stop reading
                    }
                }
                StreamEvent::ContentBlockStart {
                    content_block: ContentBlockStub::ToolUse { name, .. }, ..
                } => {
                    let _ = tx.send(RenderEvent::ToolCallStarted { name }).await;
                }
                StreamEvent::MessageDelta { delta, .. } => {
                    let _ = tx.send(RenderEvent::Done {
                        stop_reason: delta.stop_reason,
                    }).await;
                }
                StreamEvent::MessageStop => {
                    let _ = tx.send(RenderEvent::Done { stop_reason: None }).await;
                    return;
                }
                _ => {}
            }
        }
    }
}

/// The consumer: receives render events and writes to the terminal.
pub async fn renderer_task(
    mut rx: mpsc::Receiver<RenderEvent>,
) -> RendererOutput {
    let mut full_text = String::new();
    let mut stop_reason = None;

    while let Some(event) = rx.recv().await {
        match event {
            RenderEvent::TextDelta(text) => {
                print!("{}", text);
                std::io::Write::flush(&mut std::io::stdout()).ok();
                full_text.push_str(&text);
            }
            RenderEvent::ToolCallStarted { name } => {
                eprintln!("\n[Calling tool: {}]", name);
            }
            RenderEvent::Done { stop_reason: sr } => {
                stop_reason = sr;
                println!();
                break;
            }
            RenderEvent::Error(msg) => {
                eprintln!("\n[Stream error: {}]", msg);
                break;
            }
        }
    }

    RendererOutput {
        text: full_text,
        stop_reason,
    }
}

pub struct RendererOutput {
    pub text: String,
    pub stop_reason: Option<String>,
}
```

::: python Coming from Python
Python's `asyncio.Queue` serves a similar purpose:
```python
queue = asyncio.Queue(maxsize=64)

async def producer():
    async for chunk in stream:
        await queue.put(chunk)  # blocks if queue is full

async def consumer():
    while True:
        chunk = await queue.get()
        print(chunk, end="", flush=True)
```
Rust's `mpsc::channel` works the same way, but with one important addition: when the `Sender` is dropped, the `Receiver` immediately returns `None`, cleanly ending the consumer. In Python, you need to send a sentinel value or use `queue.join()`.
:::

## Choosing the buffer size

The channel buffer size controls the tradeoff between latency and throughput:

| Buffer size | Behavior                                   |
|-------------|--------------------------------------------|
| 1           | Maximum backpressure, adds latency per token |
| 16-32       | Good default, absorbs short bursts          |
| 64-128      | High throughput, tolerates slow consumers    |
| 1024+       | Minimal backpressure, risks memory growth    |

A buffer of **32** is a good starting point. At 80 tokens/s, a full buffer represents 400ms of tokens -- enough to absorb a slow `flush()` without the reader stalling, but small enough that memory stays bounded at a few kilobytes.

```rust
// In your main streaming function:
let (tx, rx) = create_render_channel(32);
```

## Running producer and consumer concurrently

Wire the two tasks together with `tokio::join!`:

```rust
pub async fn stream_with_backpressure(
    byte_stream: impl futures::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin + Send + 'static,
    cancel_token: CancellationToken,
) -> Result<RendererOutput, Box<dyn std::error::Error>> {
    let (tx, rx) = create_render_channel(32);

    let reader_cancel = cancel_token.clone();

    // Spawn the reader as a separate task
    let reader_handle = tokio::spawn(async move {
        stream_reader_task(byte_stream, tx, reader_cancel).await;
    });

    // Run the renderer on the current task
    let output = renderer_task(rx).await;

    // Wait for the reader to finish (it should already be done)
    reader_handle.await.ok();

    Ok(output)
}
```

The reader runs in a spawned task so it can be truly concurrent with the renderer. The renderer runs on the current task. When the channel's buffer is full, the reader's `tx.send().await` pauses until the renderer processes an event. This is backpressure in action -- the slow consumer naturally throttles the fast producer.

## Avoiding token loss

Backpressure must never cause token loss. The bounded channel guarantees this: `send().await` blocks rather than dropping the message. But there are two edge cases to watch:

1. **Renderer panics.** If the renderer task panics, the `Receiver` is dropped, and `tx.send()` returns `Err`. The reader should detect this and stop cleanly, as shown above.

2. **Reader drops `tx` before sending `Done`.** If the reader encounters a network error and returns early, it drops `tx`. The renderer's `rx.recv()` then returns `None`, ending the loop. No tokens are lost, but the renderer needs to handle the "stream ended without Done event" case.

```rust
// In renderer_task, after the while loop:
// If we exited because the channel closed (rx.recv() returned None)
// without receiving a Done event, the stream was interrupted
if stop_reason.is_none() {
    eprintln!("[Stream ended unexpectedly]");
}
```

## Key Takeaways

- Without backpressure, a fast API connection feeding a slow terminal causes unbounded memory growth as unrendered tokens accumulate.
- Tokio's bounded `mpsc::channel` provides natural backpressure: when the channel is full, the sender blocks until the receiver catches up.
- Split the streaming pipeline into a reader task (producer) and a renderer task (consumer) connected by a bounded channel.
- A buffer size of 32 is a good default -- it absorbs burst without risking meaningful memory growth.
- Bounded channels never drop messages, so you are guaranteed not to lose tokens. Handle the edge cases of dropped senders and receivers gracefully.
