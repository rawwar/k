---
title: Backpressure and Flow Control
description: Managing the flow of data between network, parsing, and rendering layers to prevent buffer bloat, dropped frames, and memory exhaustion.
---

# Backpressure and Flow Control

> **What you'll learn:**
> - What backpressure means in a streaming pipeline and why unbounded buffering leads to memory exhaustion and latency spikes
> - How to implement bounded channels between the network reader, parser, and renderer using tokio::sync::mpsc
> - Strategies for dropping or coalescing events when the consumer cannot keep up with the producer

Your streaming agent is a pipeline: bytes flow in from the network, get parsed into SSE events, get transformed into application events, and finally get rendered to the terminal. What happens when one stage of this pipeline is faster than the next? Without flow control, the fast producer fills memory with unprocessed data while the slow consumer falls further behind. This is the backpressure problem, and solving it correctly is the difference between an agent that stays responsive under load and one that consumes gigabytes of memory and then crashes.

## What Is Backpressure?

Backpressure is a mechanism that lets a slow consumer signal to a fast producer: "slow down, I can't keep up." The term comes from fluid dynamics -- when you put your thumb over a garden hose, the pressure *backs* up through the system, slowing the flow at the source.

In a streaming pipeline without backpressure, the producer runs at full speed regardless of how fast the consumer processes data. The excess data piles up in buffers. These buffers grow without bound, consuming memory until the system runs out:

```
Network (fast)  -->  [unbounded buffer grows forever]  -->  Renderer (slow)
```

With backpressure, the buffer has a fixed capacity. When it fills up, the producer is forced to wait until the consumer drains some items:

```
Network (fast)  -->  [bounded buffer, capacity 32]  -->  Renderer (slow)
                      ^-- producer blocks when full
```

## Why This Matters for LLM Streaming

You might think: "An LLM generates tokens at 30-80 per second. My terminal can easily keep up with that." And you would be right for the common case. But several scenarios create backpressure situations:

**Network bursts.** If the connection to the LLM API has variable latency, tokens might arrive in bursts. A one-second network stall followed by a burst of 80 tokens creates a spike that the renderer must handle smoothly.

**Expensive rendering.** If your TUI does syntax highlighting, markdown rendering, or layout computation on each token, the rendering step might occasionally take longer than the inter-token interval, especially during code blocks.

**Multiple concurrent streams.** If your agent runs multiple LLM calls in parallel (for example, generating code while simultaneously summarizing a file), each stream competes for rendering time.

**Tool call processing.** When the LLM issues a tool call, the agent must execute it before continuing. During execution, tokens from other streams might queue up.

## Bounded Channels with tokio::sync::mpsc

Tokio's `mpsc` (multi-producer, single-consumer) channel is the primary tool for backpressure in async Rust. When you create a bounded channel, the sender blocks (asynchronously) when the buffer is full:

```rust
use tokio::sync::mpsc;

#[derive(Debug)]
enum StreamEvent {
    TextDelta(String),
    ToolCallStart { id: String, name: String },
    ToolCallDelta { id: String, json_fragment: String },
    ToolCallEnd { id: String },
    MessageComplete,
    Error(String),
}

async fn create_streaming_pipeline() {
    // Bounded channel: capacity of 32 events
    let (tx, mut rx) = mpsc::channel::<StreamEvent>(32);

    // Producer: reads from network and sends events
    let producer = tokio::spawn(async move {
        // Simulate SSE events arriving from the network
        for i in 0..100 {
            let event = StreamEvent::TextDelta(format!("token_{} ", i));

            // This will await (not block the thread) if the channel is full.
            // Backpressure propagates: if the renderer is slow, the network
            // reader pauses, which causes TCP flow control to slow the server.
            if tx.send(event).await.is_err() {
                break; // Receiver dropped, stream is cancelled
            }
        }
    });

    // Consumer: renders events to the terminal
    let consumer = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                StreamEvent::TextDelta(text) => {
                    print!("{}", text);
                    // Simulate slow rendering
                    tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
                }
                StreamEvent::MessageComplete => {
                    println!("\n[Message complete]");
                    break;
                }
                _ => {}
            }
        }
    });

    let _ = tokio::join!(producer, consumer);
}
```

The magic here is that `tx.send(event).await` pauses the producer when the channel is full. Because the producer is an async task reading from the network, this pause propagates to the TCP layer: the producer stops reading from the socket, the TCP receive buffer fills, and TCP flow control tells the server to slow down. Backpressure flows all the way from your terminal to the LLM API server.

::: python Coming from Python
Python's `asyncio.Queue` serves the same purpose:
```python
import asyncio

queue = asyncio.Queue(maxsize=32)  # Bounded queue

async def producer():
    for i in range(100):
        await queue.put(f"token_{i}")  # Blocks if queue is full

async def consumer():
    while True:
        token = await queue.get()
        print(token, end="")
```
The semantics are identical: `await queue.put()` blocks when the queue is full, just like `tx.send().await` in Rust. The key difference is that Rust's `mpsc::Sender` is `Send + Sync`, so you can safely share it across threads (useful when the producer runs on a blocking thread pool), whereas Python's `asyncio.Queue` is single-threaded by design.
:::

## Choosing the Channel Capacity

The channel capacity is a tuning parameter that trades latency for throughput:

**Too small (1-4):** The producer frequently blocks, which means the network read stalls often. This adds jitter to token delivery because each read-parse-send cycle becomes serialized.

**Too large (1000+):** The buffer absorbs bursts but can accumulate significant latency. If 500 tokens are buffered, the rendered output is 500 tokens behind real-time.

**Sweet spot (16-64):** Large enough to absorb short bursts and smooth out scheduling jitter, but small enough that the buffer drain time is imperceptible (at 50 tokens/second, 32 buffered tokens drain in ~640ms).

```rust
// Reasonable defaults for an LLM streaming pipeline
const NETWORK_TO_PARSER_CAPACITY: usize = 64;   // Raw byte chunks
const PARSER_TO_APP_CAPACITY: usize = 32;        // Parsed SSE events
const APP_TO_RENDERER_CAPACITY: usize = 16;       // Render commands
```

Each stage can have a different capacity based on the expected processing speed of the consumer. The renderer channel is smallest because rendering is typically the slowest stage.

## Dropping and Coalescing Events

Sometimes you want to handle backpressure by dropping or merging events rather than slowing the producer. This makes sense when some events are "replaceable" -- for example, progress updates where only the latest value matters:

```rust
use tokio::sync::watch;

// watch channel: always contains the latest value, old values are overwritten
async fn progress_reporting() {
    let (tx, mut rx) = watch::channel(0u64); // Initial value: 0 tokens

    // Producer: updates token count rapidly
    let producer = tokio::spawn(async move {
        for count in 1..=1000 {
            // This never blocks -- it just overwrites the previous value
            let _ = tx.send(count);
        }
    });

    // Consumer: reads the latest count at its own pace
    let consumer = tokio::spawn(async move {
        loop {
            // Wait for the value to change
            if rx.changed().await.is_err() {
                break;
            }
            let count = *rx.borrow();
            println!("Tokens received: {}", count);
            // Consumer might skip intermediate values -- that's fine
            // for a progress counter
        }
    });

    let _ = tokio::join!(producer, consumer);
}
```

Tokio's `watch` channel is perfect for "latest value" semantics. The producer never blocks, and the consumer always sees the most recent value. Intermediate values are silently overwritten.

For text deltas, dropping is generally not acceptable -- you would miss tokens and the output would be garbled. But for metadata like token counts, timing information, or progress percentages, `watch` channels or periodic sampling work well.

## Multi-Stage Pipelines

A production streaming pipeline has multiple stages, each connected by a bounded channel:

```rust
use tokio::sync::mpsc;

/// Stage 1: Network reader -> raw byte chunks
async fn network_stage(
    response: reqwest::Response,
    tx: mpsc::Sender<bytes::Bytes>,
) {
    use futures::StreamExt;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => {
                if tx.send(bytes).await.is_err() {
                    break; // Downstream cancelled
                }
            }
            Err(e) => {
                eprintln!("Network error: {}", e);
                break;
            }
        }
    }
}

/// Stage 2: Raw bytes -> parsed SSE events
async fn parser_stage(
    mut rx: mpsc::Receiver<bytes::Bytes>,
    tx: mpsc::Sender<SseEvent>,
) {
    let mut parser = SseStream::new();

    while let Some(bytes) = rx.recv().await {
        let events = parser.feed(&bytes);
        for event in events {
            if tx.send(event).await.is_err() {
                return; // Downstream cancelled
            }
        }
    }
}

/// Stage 3: SSE events -> rendered output
async fn render_stage(mut rx: mpsc::Receiver<SseEvent>) {
    while let Some(event) = rx.recv().await {
        match event.event_type() {
            "content_block_delta" => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&event.data) {
                    if let Some(text) = v.get("delta")
                        .and_then(|d| d.get("text"))
                        .and_then(|t| t.as_str())
                    {
                        print!("{}", text);
                        std::io::stdout().flush().ok();
                    }
                }
            }
            _ => {}
        }
    }
    println!();
}

// SseEvent and SseStream types referenced from previous subchapter
use std::io::Write;

// Wire it all together:
async fn run_pipeline(response: reqwest::Response) {
    let (net_tx, net_rx) = mpsc::channel(64);
    let (parse_tx, parse_rx) = mpsc::channel(32);

    tokio::spawn(network_stage(response, net_tx));
    tokio::spawn(parser_stage(net_rx, parse_tx));
    render_stage(parse_rx).await;
}
```

Each stage runs as an independent async task. Backpressure propagates automatically: if the renderer is slow, the parser channel fills up, which causes the parser task to block on `tx.send()`, which causes it to stop reading from its input channel, which causes the network channel to fill up, which causes the network reader to stop calling `stream.next()`, which causes TCP flow control to kick in.

::: wild In the Wild
OpenCode structures its streaming pipeline as a series of Go channels connecting the HTTP reader, SSE parser, and TUI renderer. Each channel has a fixed buffer size, and backpressure propagates naturally through the Go runtime's channel semantics. Claude Code takes a slightly different approach, processing events synchronously within a single async context rather than splitting into separate tasks, which simplifies the flow control logic at the cost of less parallelism between parsing and rendering.
:::

## Key Takeaways

- **Backpressure** is the mechanism by which a slow consumer signals a fast producer to slow down. Without it, unbounded buffers grow until memory is exhausted.
- **Bounded `mpsc` channels** are the primary backpressure tool in async Rust. When the channel is full, `tx.send().await` pauses the producer, propagating pressure upstream to the network layer.
- **Channel capacity** is a tuning parameter: 16-64 events is a reasonable default for LLM streaming, balancing burst absorption against latency.
- Use **`watch` channels** for "latest value" semantics where intermediate values can be safely dropped (progress counters, token counts).
- A **multi-stage pipeline** (network -> parser -> renderer) with bounded channels between each stage provides clean separation of concerns with automatic backpressure propagation.
