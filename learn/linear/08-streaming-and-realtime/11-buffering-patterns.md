---
title: Buffering Patterns
description: Buffer design strategies for streaming pipelines including line buffering, time-based flushing, size-based batching, and adaptive buffering for variable network conditions.
---

# Buffering Patterns

> **What you'll learn:**
> - The tradeoff between latency and throughput when choosing buffer sizes and flush strategies
> - Implementing time-based flush intervals that ensure content appears within a maximum delay regardless of chunk size
> - Adaptive buffering techniques that adjust batch sizes based on network throughput and rendering speed

Buffering is the unsung hero of streaming performance. Every layer of your pipeline has buffers -- the TCP receive buffer, the HTTP library's internal buffer, your SSE line buffer, the channel buffer between stages, and the terminal's output buffer. Each one affects the user experience. Too much buffering and the output feels sluggish, lagging behind the LLM's generation. Too little and you waste CPU cycles on frequent tiny flushes, or the output appears jittery. This subchapter explores the buffering strategies that give your agent the smoothest possible output.

## The Latency-Throughput Tradeoff

Buffering trades latency for throughput. Sending data one byte at a time minimizes latency (each byte appears immediately) but maximizes overhead (each send is a syscall). Sending data in large batches maximizes throughput (fewer syscalls, better compression) but increases latency (data waits in the buffer until the batch is full).

For LLM streaming, latency is almost always more important than throughput. The user is reading the output in real-time, and any perceptible delay feels like the agent is hesitating. But you cannot ignore throughput entirely -- if your rendering loop spends all its time on syscalls, it will fall behind the token stream.

Here is the spectrum of buffering strategies:

```
No buffering         Line buffering        Time-based          Size-based
(per-byte flush)     (flush on \n)         (flush every Xms)   (flush every N bytes)
   |                    |                     |                    |
   v                    v                     v                    v
Minimum latency    Good balance          Configurable         Maximum throughput
Maximum overhead   Natural boundaries    Smooth output        Maximum latency
```

## Line Buffering

Line buffering flushes the output buffer whenever a newline character arrives. This is a natural boundary for text content -- users read line by line, so showing complete lines feels natural:

```rust
use std::io::{self, BufWriter, Write};

pub struct LineBufferedWriter {
    writer: BufWriter<io::Stdout>,
}

impl LineBufferedWriter {
    pub fn new() -> Self {
        // BufWriter batches writes, reducing syscall overhead
        Self {
            writer: BufWriter::new(io::stdout()),
        }
    }

    pub fn write_token(&mut self, token: &str) {
        self.writer.write_all(token.as_bytes()).unwrap();

        // Flush if the token contains a newline
        if token.contains('\n') {
            self.writer.flush().unwrap();
        }
    }

    pub fn flush(&mut self) {
        self.writer.flush().unwrap();
    }
}
```

Line buffering works well for prose and code, where lines are typically 40-120 characters. The delay between the start of a line and its display is the time it takes the LLM to generate that line -- usually less than a second. The user does not notice because they are still reading the previous line.

The downside: if the LLM generates a very long line (a minified JSON blob, a long URL), the user waits until the entire line is generated. And for single-line responses, the user sees nothing until the LLM generates the first newline.

## Time-Based Flushing

Time-based flushing ensures that content appears within a maximum delay, regardless of how many characters have accumulated:

```rust
use std::io::{self, Write};
use std::time::{Duration, Instant};

pub struct TimedBuffer {
    buffer: String,
    last_flush: Instant,
    max_delay: Duration,
}

impl TimedBuffer {
    pub fn new(max_delay: Duration) -> Self {
        Self {
            buffer: String::new(),
            last_flush: Instant::now(),
            max_delay,
        }
    }

    /// Add a token to the buffer. Returns true if the buffer was flushed.
    pub fn push(&mut self, token: &str) -> bool {
        self.buffer.push_str(token);

        let should_flush = self.last_flush.elapsed() >= self.max_delay
            || token.contains('\n');

        if should_flush {
            self.flush();
            true
        } else {
            false
        }
    }

    pub fn flush(&mut self) {
        if !self.buffer.is_empty() {
            print!("{}", self.buffer);
            io::stdout().flush().unwrap();
            self.buffer.clear();
            self.last_flush = Instant::now();
        }
    }
}
```

A max delay of 50ms is a good starting point. At this interval, the user sees at most 50ms of buffering delay -- well below the threshold of human perception (about 100ms for text). The buffer accumulates tokens within each 50ms window and flushes them as a batch, reducing syscall overhead while maintaining perceived immediacy.

::: python Coming from Python
Python's `sys.stdout` is line-buffered by default when writing to a terminal, and fully buffered when writing to a pipe. You can control this with `flush=True`:
```python
import sys
import time

class TimedBuffer:
    def __init__(self, max_delay=0.05):
        self.buffer = ""
        self.last_flush = time.monotonic()
        self.max_delay = max_delay

    def write(self, token):
        self.buffer += token
        if time.monotonic() - self.last_flush >= self.max_delay or '\n' in token:
            sys.stdout.write(self.buffer)
            sys.stdout.flush()
            self.buffer = ""
            self.last_flush = time.monotonic()
```
The Rust version uses `Instant::now()` instead of `time.monotonic()` and `std::io::stdout()` instead of `sys.stdout`, but the logic is identical. The key difference is that Rust's `BufWriter` gives you explicit control over the buffer size and flush behavior, whereas Python's stdio buffering is configured at the interpreter level.
:::

## Size-Based Batching

Size-based batching flushes when the buffer exceeds a certain number of bytes. This is useful when you want to control the granularity of terminal updates:

```rust
pub struct SizedBuffer {
    buffer: String,
    max_size: usize,
}

impl SizedBuffer {
    pub fn new(max_size: usize) -> Self {
        Self {
            buffer: String::new(),
            max_size,
        }
    }

    pub fn push(&mut self, token: &str) -> bool {
        self.buffer.push_str(token);

        if self.buffer.len() >= self.max_size {
            self.flush();
            true
        } else {
            false
        }
    }

    pub fn flush(&mut self) {
        if !self.buffer.is_empty() {
            print!("{}", self.buffer);
            std::io::stdout().flush().unwrap();
            self.buffer.clear();
        }
    }
}
```

A common size threshold is 256-512 bytes -- roughly one to four lines of text. This reduces syscalls by an order of magnitude compared to per-token flushing while keeping the perceived delay short.

## Combining Strategies

The most effective approach combines multiple flush triggers:

```rust
use std::io::{self, Write};
use std::time::{Duration, Instant};

pub struct SmartBuffer {
    buffer: String,
    last_flush: Instant,
    /// Maximum time between flushes
    max_delay: Duration,
    /// Maximum buffer size before forced flush
    max_size: usize,
    /// Track total bytes written for statistics
    total_bytes: usize,
    total_flushes: usize,
}

impl SmartBuffer {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            last_flush: Instant::now(),
            max_delay: Duration::from_millis(50),
            max_size: 512,
            total_bytes: 0,
            total_flushes: 0,
        }
    }

    pub fn push(&mut self, token: &str) {
        self.buffer.push_str(token);
        self.maybe_flush();
    }

    fn maybe_flush(&mut self) {
        let should_flush =
            // Time-based: don't let content sit too long
            self.last_flush.elapsed() >= self.max_delay
            // Size-based: don't accumulate too much
            || self.buffer.len() >= self.max_size
            // Content-based: flush on paragraph breaks for natural pacing
            || self.buffer.ends_with("\n\n")
            // Content-based: flush on code block boundaries
            || self.buffer.trim_end().ends_with("```");

        if should_flush {
            self.flush();
        }
    }

    pub fn flush(&mut self) {
        if !self.buffer.is_empty() {
            print!("{}", self.buffer);
            io::stdout().flush().unwrap();
            self.total_bytes += self.buffer.len();
            self.total_flushes += 1;
            self.buffer.clear();
            self.last_flush = Instant::now();
        }
    }

    pub fn stats(&self) -> (usize, usize) {
        (self.total_bytes, self.total_flushes)
    }
}
```

This `SmartBuffer` flushes on whichever trigger fires first: time, size, paragraph break, or code block boundary. The content-based triggers (paragraph breaks and code block markers) create natural visual pauses that align with the structure of the text.

## Adaptive Buffering

The optimal buffer settings depend on conditions that change over time: network speed, LLM generation speed, and terminal rendering speed. Adaptive buffering adjusts its parameters based on observed conditions:

```rust
use std::time::{Duration, Instant};

pub struct AdaptiveBuffer {
    buffer: String,
    max_delay: Duration,

    /// Track token arrival rate
    tokens_received: u32,
    window_start: Instant,
    tokens_per_second: f64,
}

impl AdaptiveBuffer {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            max_delay: Duration::from_millis(50),
            tokens_received: 0,
            window_start: Instant::now(),
            tokens_per_second: 0.0,
        }
    }

    pub fn push(&mut self, token: &str) {
        self.buffer.push_str(token);
        self.tokens_received += 1;

        // Recalculate rate every second
        let elapsed = self.window_start.elapsed();
        if elapsed >= Duration::from_secs(1) {
            self.tokens_per_second =
                self.tokens_received as f64 / elapsed.as_secs_f64();
            self.tokens_received = 0;
            self.window_start = Instant::now();

            // Adapt buffer delay based on token rate
            self.adapt_delay();
        }

        self.maybe_flush();
    }

    fn adapt_delay(&mut self) {
        // High token rate (>60/s): buffer more to reduce flush overhead
        // Low token rate (<10/s): flush faster to minimize perceived latency
        self.max_delay = if self.tokens_per_second > 60.0 {
            Duration::from_millis(100) // Tokens are flowing fast, batch more
        } else if self.tokens_per_second > 30.0 {
            Duration::from_millis(50)  // Normal rate, standard buffering
        } else if self.tokens_per_second > 10.0 {
            Duration::from_millis(30)  // Slower rate, flush sooner
        } else {
            Duration::from_millis(10)  // Very slow, nearly immediate flush
        };
    }

    fn maybe_flush(&mut self) {
        if self.buffer.is_empty() {
            return;
        }

        let elapsed = Instant::now()
            .duration_since(self.window_start.checked_sub(
                Duration::from_millis(self.tokens_received as u64 * 20)
            ).unwrap_or(self.window_start));

        // Simple check: flush if buffer has been waiting too long
        if !self.buffer.is_empty()
            && (self.buffer.len() > 256 || self.buffer.contains('\n'))
        {
            self.flush();
        }
    }

    pub fn flush(&mut self) {
        if !self.buffer.is_empty() {
            print!("{}", self.buffer);
            std::io::stdout().flush().unwrap();
            self.buffer.clear();
        }
    }
}
```

The adaptive strategy is most useful when your agent operates in diverse network conditions -- fast office connections versus slow cellular networks. On a fast connection, tokens arrive rapidly and batching makes sense. On a slow connection, every token is precious and should be displayed immediately.

## The TCP Buffer Layer

One buffer you cannot directly control is the TCP receive buffer. The operating system buffers incoming TCP data before your application reads it. This buffer is typically 64KB-256KB and is managed by the kernel.

For LLM streaming, the TCP buffer rarely causes issues because the data rate is low (a few KB/s of SSE text). But on very slow clients (like a CI runner with constrained resources), the TCP buffer can fill up, triggering TCP flow control, which tells the server to slow down. This is actually desirable -- it is the same backpressure mechanism from the previous subchapter, operating at the transport layer.

You can observe TCP buffer behavior with:

```rust
// Get the TCP receive buffer size (for debugging)
fn print_tcp_buffer_info() {
    #[cfg(target_os = "linux")]
    {
        if let Ok(contents) = std::fs::read_to_string("/proc/sys/net/core/rmem_default") {
            println!("Default TCP receive buffer: {} bytes", contents.trim());
        }
    }
    // On macOS: sysctl net.inet.tcp.recvspace
}
```

::: wild In the Wild
Claude Code uses a relatively simple buffering strategy: tokens are written to stdout as they arrive, with the terminal's built-in line buffering providing the primary batching. For most terminal emulators, this results in smooth output because the terminal itself buffers rapid writes and renders them in its display refresh cycle (typically 60Hz). OpenCode, with its TUI, has a different challenge: it re-renders the entire visible area on each frame, so buffering tokens between frames is essential to avoid rendering overhead from exceeding the frame budget.
:::

## Key Takeaways

- **Line buffering** is a natural default for text streaming -- it aligns with how users read and produces smooth output for typical line lengths.
- **Time-based flushing** (50ms intervals) guarantees a maximum display delay regardless of content structure, which is important for long single-line outputs.
- **Combining triggers** (time, size, and content boundaries like paragraph breaks) gives the best overall experience, flushing at whichever natural boundary comes first.
- **Adaptive buffering** adjusts flush intervals based on observed token rate: flush aggressively when tokens are slow (maximize perceived responsiveness) and batch more when tokens are fast (reduce overhead).
- The **TCP receive buffer** provides an additional layer of buffering and backpressure that operates transparently at the transport level -- you rarely need to tune it directly.
