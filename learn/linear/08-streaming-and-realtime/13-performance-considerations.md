---
title: Performance Considerations
description: Profiling and optimizing streaming pipelines for throughput, latency, and memory usage with attention to allocation patterns, syscall frequency, and async overhead.
---

# Performance Considerations

> **What you'll learn:**
> - How to measure time-to-first-token, inter-token latency, and total throughput in a streaming pipeline
> - Reducing memory allocations in the hot path by reusing buffers, using bytes::Bytes, and minimizing copies
> - Profiling async task scheduling overhead and minimizing context switches between the network and render tasks

You have built a streaming pipeline that is correct -- it parses SSE, handles partial JSON, manages backpressure, and supports cancellation. But is it fast? For most LLM streaming scenarios, the answer is "fast enough" because the bottleneck is the LLM's token generation speed (30-80 tokens/second), not your client's processing speed. But there are situations where client-side performance matters: multiple concurrent streams, expensive rendering (syntax highlighting, markdown), constrained environments (CI runners, embedded devices), and the ever-important time-to-first-token metric. This subchapter covers how to measure and optimize your pipeline's performance.

## Measuring What Matters

Before optimizing, you need to measure. Here are the three metrics that matter for a streaming agent:

```rust
use std::time::{Duration, Instant};

pub struct StreamMetrics {
    stream_start: Instant,
    first_token_time: Option<Duration>,
    last_token_time: Option<Instant>,
    token_count: u64,
    total_bytes: u64,

    /// Inter-token latencies for jitter analysis
    token_latencies: Vec<Duration>,
}

impl StreamMetrics {
    pub fn new() -> Self {
        Self {
            stream_start: Instant::now(),
            first_token_time: None,
            last_token_time: None,
            token_count: 0,
            total_bytes: 0,
            token_latencies: Vec::new(),
        }
    }

    pub fn record_token(&mut self, bytes: usize) {
        let now = Instant::now();

        if self.first_token_time.is_none() {
            self.first_token_time = Some(now.duration_since(self.stream_start));
        }

        if let Some(last) = self.last_token_time {
            self.token_latencies.push(now.duration_since(last));
        }

        self.last_token_time = Some(now);
        self.token_count += 1;
        self.total_bytes += bytes as u64;
    }

    pub fn report(&self) {
        let total_duration = self.stream_start.elapsed();

        println!("=== Stream Performance ===");

        if let Some(ttft) = self.first_token_time {
            println!("Time to first token: {:?}", ttft);
        }

        println!("Total duration: {:?}", total_duration);
        println!("Tokens: {}", self.token_count);
        println!("Bytes: {}", self.total_bytes);

        if self.token_count > 0 {
            let tokens_per_sec = self.token_count as f64 / total_duration.as_secs_f64();
            println!("Throughput: {:.1} tokens/sec", tokens_per_sec);
        }

        if !self.token_latencies.is_empty() {
            let avg_latency: Duration = self.token_latencies.iter().sum::<Duration>()
                / self.token_latencies.len() as u32;
            let max_latency = self.token_latencies.iter().max().unwrap();
            let min_latency = self.token_latencies.iter().min().unwrap();

            println!("Inter-token latency:");
            println!("  avg: {:?}", avg_latency);
            println!("  min: {:?}", min_latency);
            println!("  max: {:?}", max_latency);

            // Calculate p95 latency
            let mut sorted = self.token_latencies.clone();
            sorted.sort();
            let p95_idx = (sorted.len() as f64 * 0.95) as usize;
            if p95_idx < sorted.len() {
                println!("  p95: {:?}", sorted[p95_idx]);
            }
        }
    }
}
```

Run this instrumentation on a real LLM stream to establish your baseline. Typical numbers for a well-implemented client:

| Metric | Target | Concern threshold |
|--------|--------|-------------------|
| Time to first token | 200-500ms | >1s |
| Inter-token avg latency | 15-30ms | >50ms |
| Inter-token p95 latency | <60ms | >100ms |
| Memory per stream | <1MB | >10MB |

If your numbers are within these ranges, your pipeline is not the bottleneck and further optimization has diminishing returns.

## Reducing Allocations in the Hot Path

The "hot path" of your streaming pipeline is the code that runs for every token: reading bytes from the network, parsing the SSE line, extracting the JSON delta, and rendering the text. Every allocation in this path adds latency and GC pressure (or in Rust's case, allocator overhead).

### Reusing Buffers

The biggest allocation win is reusing your SSE line buffer and JSON parsing buffer instead of creating new `String`s for each event:

```rust
pub struct OptimizedSseStream {
    /// Reusable line buffer -- cleared but not deallocated between events
    line_buffer: String,
    /// Reusable event data buffer
    data_buffer: String,
    event_type: Option<String>,
    bom_stripped: bool,
}

impl OptimizedSseStream {
    pub fn new() -> Self {
        Self {
            // Pre-allocate reasonable capacity
            line_buffer: String::with_capacity(4096),
            data_buffer: String::with_capacity(1024),
            event_type: None,
            bom_stripped: false,
        }
    }

    pub fn feed(&mut self, data: &[u8]) -> Vec<SseEvent> {
        let mut bytes = data;
        if !self.bom_stripped {
            if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
                bytes = &bytes[3..];
            }
            self.bom_stripped = true;
        }

        let text = String::from_utf8_lossy(bytes);
        self.line_buffer.push_str(&text);

        let mut events = Vec::new();

        loop {
            let newline_pos = match self.line_buffer.find('\n') {
                Some(pos) => pos,
                None => break,
            };

            // Extract the line without allocating a new String
            let line_end = if newline_pos > 0
                && self.line_buffer.as_bytes()[newline_pos - 1] == b'\r'
            {
                newline_pos - 1
            } else {
                newline_pos
            };

            // Process the line in-place
            let is_blank = line_end == 0
                || (line_end == 0 && newline_pos == 0);

            if self.line_buffer[..line_end].is_empty() {
                // Blank line: dispatch event
                if !self.data_buffer.is_empty() {
                    events.push(SseEvent {
                        event_type: self.event_type.take(),
                        data: self.data_buffer.clone(),
                        id: None,
                        retry: None,
                    });
                    self.data_buffer.clear(); // Clear but keep allocation
                }
            } else if self.line_buffer[..line_end].starts_with(':') {
                // Comment, ignore
            } else if let Some(colon) = self.line_buffer[..line_end].find(':') {
                let field = &self.line_buffer[..colon];
                let mut value_start = colon + 1;
                if value_start < line_end
                    && self.line_buffer.as_bytes()[value_start] == b' '
                {
                    value_start += 1;
                }
                let value = &self.line_buffer[value_start..line_end];

                match field {
                    "data" => {
                        if !self.data_buffer.is_empty() {
                            self.data_buffer.push('\n');
                        }
                        self.data_buffer.push_str(value);
                    }
                    "event" => {
                        self.event_type = Some(value.to_string());
                    }
                    _ => {}
                }
            }

            // Remove the processed line from the buffer
            // This is the one unavoidable allocation: shifting the buffer contents
            self.line_buffer = self.line_buffer[newline_pos + 1..].to_string();
        }

        events
    }
}

#[derive(Debug, Clone)]
struct SseEvent {
    event_type: Option<String>,
    data: String,
    id: Option<String>,
    retry: Option<u64>,
}
```

The key optimizations:

- **`String::with_capacity`** pre-allocates buffer space. The buffers grow to their maximum size once and then get reused via `.clear()`, which resets the length without freeing the allocation.
- **In-place line processing** avoids creating temporary `String`s for each line. We work with slices of `line_buffer` directly.

### Using bytes::Bytes for Zero-Copy

The `bytes` crate provides `Bytes`, a reference-counted byte buffer that supports zero-copy slicing. `reqwest` already returns `Bytes` from its stream, so you can avoid copying data until you actually need to modify it:

```rust
use bytes::{Bytes, BytesMut};

pub struct ZeroCopyBuffer {
    /// Accumulated bytes that haven't been processed into complete lines
    pending: BytesMut,
}

impl ZeroCopyBuffer {
    pub fn new() -> Self {
        Self {
            pending: BytesMut::with_capacity(4096),
        }
    }

    /// Feed raw bytes and extract complete lines as byte slices.
    /// Returns slices that reference the internal buffer -- no copies.
    pub fn feed(&mut self, data: Bytes) -> Vec<Bytes> {
        self.pending.extend_from_slice(&data);

        let mut lines = Vec::new();
        let mut start = 0;

        for i in 0..self.pending.len() {
            if self.pending[i] == b'\n' {
                let end = if i > start && self.pending[i - 1] == b'\r' {
                    i - 1
                } else {
                    i
                };
                // Split off a frozen Bytes reference -- this is O(1)
                let line = Bytes::copy_from_slice(&self.pending[start..end]);
                lines.push(line);
                start = i + 1;
            }
        }

        // Remove processed bytes from the buffer
        if start > 0 {
            let _ = self.pending.split_to(start);
        }

        lines
    }
}
```

`BytesMut::split_to` is an O(1) operation that adjusts pointers without copying data. For high-throughput scenarios, this eliminates the string copying that dominates naive implementations.

::: python Coming from Python
Python does not have an equivalent to Rust's zero-copy buffer management. Every `str` concatenation in Python creates a new string object:
```python
buffer = ""
for chunk in stream:
    buffer += chunk  # This creates a new string every time!
```
Using `io.BytesIO` or `bytearray` is more efficient:
```python
buffer = bytearray()
buffer.extend(chunk)  # Amortized O(1) append
```
Rust's `BytesMut` combines the efficiency of `bytearray` with zero-copy slicing that has no Python equivalent. When you call `split_to()`, no data is copied -- you get a new view into the same memory. This is possible because Rust's ownership system guarantees that the original buffer and the slice cannot be used simultaneously in conflicting ways.
:::

## Reducing Syscalls

Each `stdout().flush()` is a `write()` syscall, and each syscall has overhead (context switch to kernel, ~1-5 microseconds). At 60 tokens/second with per-token flushing, that is 60 syscalls/second -- negligible. But if you are rendering with ANSI escape codes (color, cursor movement), each token might generate multiple writes, and the overhead adds up.

Use `BufWriter` to batch writes:

```rust
use std::io::{BufWriter, Write};

fn efficient_render(tokens: &[&str]) {
    let stdout = std::io::stdout();
    let mut writer = BufWriter::with_capacity(8192, stdout.lock());

    for token in tokens {
        // These writes go to the in-memory buffer, not the kernel
        write!(writer, "{}", token).unwrap();
    }

    // Single syscall to flush everything
    writer.flush().unwrap();
}
```

`BufWriter::with_capacity(8192, ...)` creates an 8KB buffer. Writes accumulate in this buffer, and only when you call `flush()` (or the buffer fills up) does an actual `write()` syscall happen.

## Async Task Overhead

Each `tokio::spawn` creates a new async task that the runtime must schedule. For your streaming pipeline, you might have 3-5 tasks: network reader, SSE parser, event processor, renderer, and signal handler. This is fine -- Tokio can handle millions of tasks.

But be aware of how `tokio::select!` interacts with task scheduling. Each branch in a `select!` creates a future that must be polled:

```rust
// Each iteration polls all three futures
loop {
    tokio::select! {
        chunk = stream.next() => { /* ... */ }
        _ = cancel.cancelled() => { break; }
        _ = tick.tick() => { /* flush */ }
    }
}
```

If the `stream.next()` future involves multiple layers of async indirection (HTTP/2 frame decoding, TLS, decompression), each poll touches several layers of state. For the typical LLM streaming case (50-80 events/second), this overhead is negligible. But if you are ever profiling and see unexpected CPU usage in your streaming loop, check how many futures are being polled in your `select!` and whether any of them are doing expensive work on each poll.

## Profiling Tools

When you need to dig deeper, here are the tools to reach for:

**`tokio-console`** visualizes async task scheduling in real time. Install it and instrument your app to see which tasks are running, which are waiting, and how long each poll takes:

```rust
// Add to Cargo.toml: tokio = { features = ["tracing"] }
// Add to Cargo.toml: console-subscriber = "0.4"

#[tokio::main]
async fn main() {
    console_subscriber::init(); // Enable tokio-console
    // ... your agent code
}
```

**`DHAT`** (part of Valgrind) profiles heap allocations, showing you where allocations happen and how much memory each site uses. This is invaluable for finding unexpected allocations in the hot path.

**`criterion`** benchmarks individual functions with statistical rigor. Use it to benchmark your SSE parser in isolation:

```rust
// benches/sse_parsing.rs
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_sse_parsing(c: &mut Criterion) {
    let data = b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n";

    c.bench_function("parse_sse_event", |b| {
        b.iter(|| {
            let mut parser = OptimizedSseStream::new();
            let events = parser.feed(data);
            assert_eq!(events.len(), 1);
        })
    });
}

criterion_group!(benches, bench_sse_parsing);
criterion_main!(benches);
```

## When to Optimize

A practical checklist before spending time on performance:

1. **Measure first.** Use `StreamMetrics` to establish your baseline. If TTFT is under 500ms and inter-token p95 is under 60ms, you are fine.
2. **Profile before guessing.** Use `tokio-console` or `perf` to find the actual bottleneck. It is rarely where you expect.
3. **Optimize the bottleneck.** If the bottleneck is network latency, no amount of buffer optimization will help. If it is rendering, focus on the render path.
4. **Retest after each change.** Optimizations can interact in unexpected ways. Always measure after making a change.

::: wild In the Wild
Most production coding agents do not heavily optimize their streaming client performance because the LLM generation speed is the dominant bottleneck. Claude Code's Rust-optimized paths focus on areas where the client does significant work: parsing large tool call results, rendering complex markdown, and managing conversation context. OpenCode's Go implementation similarly focuses optimization effort on the TUI rendering path, where frame budget constraints (16ms for 60fps) create a real performance requirement.
:::

## Key Takeaways

- **Measure before optimizing:** TTFT, inter-token latency (p95), and memory per stream are the metrics that matter. If they are within targets, your pipeline is fast enough.
- **Reuse buffers** in the hot path by using `String::with_capacity` and `.clear()` instead of allocating new strings per event. Pre-allocation eliminates the most common source of per-token overhead.
- **`bytes::Bytes` and `BytesMut`** enable zero-copy buffer management with O(1) split operations, eliminating data copies in the parsing path.
- **Batch terminal writes** with `BufWriter` to reduce syscall frequency. A single 8KB flush is far cheaper than 100 individual small writes.
- The LLM's generation speed is almost always the bottleneck. **Optimize your client only when profiling shows it is the limiting factor** -- usually in rendering, not parsing.
