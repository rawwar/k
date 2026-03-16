---
title: Buffering Strategies
description: Explore different buffering approaches for streamed content including line buffering, word buffering, and adaptive strategies.
---

# Buffering Strategies

> **What you'll learn:**
> - How to choose between character, word, and line-level buffering for display
> - How adaptive buffering adjusts granularity based on output speed and terminal capabilities
> - How to implement a ring buffer for keeping recent context while discarding old streamed data

Printing every single token the instant it arrives is the simplest approach, and for most cases it works fine. But there are scenarios where flushing on every token is wasteful or even harmful. If the model generates 80 tokens per second, that is 80 system calls to `flush()`. If the terminal is slow (think: SSH over a high-latency connection), those flushes can bottleneck your entire pipeline. This subchapter explores buffering strategies that balance responsiveness with efficiency.

## The buffering spectrum

Buffering exists on a spectrum between two extremes:

| Strategy          | Latency         | Throughput    | System calls |
|-------------------|-----------------|---------------|--------------|
| Unbuffered        | Lowest (each token) | Low        | Very high    |
| Character buffer  | Very low        | Medium        | High         |
| Word buffer       | Low             | Good          | Medium       |
| Line buffer       | Medium          | High          | Low          |
| Block buffer      | High            | Highest       | Very low     |

For a coding agent, you want the leftmost viable option. Users expect to see tokens appear individually -- it is part of the "AI is thinking" experience. But you should be prepared to move right on the spectrum when conditions demand it.

## Flush-per-token: the default

Your current renderer calls `flush()` after every token. Let's measure the actual cost:

```rust
use std::io::{self, Write};
use std::time::Instant;

fn benchmark_flush_per_token(tokens: &[&str]) -> io::Result<std::time::Duration> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    let start = Instant::now();
    for token in tokens {
        write!(handle, "{}", token)?;
        handle.flush()?;
    }
    Ok(start.elapsed())
}

fn benchmark_batch_flush(tokens: &[&str]) -> io::Result<std::time::Duration> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    let start = Instant::now();
    for token in tokens {
        write!(handle, "{}", token)?;
    }
    handle.flush()?;
    Ok(start.elapsed())
}
```

On a local terminal, the difference is typically negligible -- maybe 50 microseconds versus 5 microseconds for 100 tokens. Over SSH with 100ms latency, the difference can be dramatic: 10 seconds versus 100 milliseconds. The key insight is that **buffering strategy should adapt to the environment**.

## Word-level buffering

Word buffering waits until a complete word (delimited by whitespace) is available before flushing. This reduces flush calls while still providing a smooth reading experience:

```rust
use std::io::{self, Write};

/// Buffers tokens and flushes at word boundaries.
pub struct WordBuffer {
    buffer: String,
    stdout: io::Stdout,
}

impl WordBuffer {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            stdout: io::stdout(),
        }
    }

    /// Add a token to the buffer. Flushes complete words to the terminal.
    pub fn push(&mut self, token: &str) -> io::Result<()> {
        self.buffer.push_str(token);

        // Look for the last whitespace boundary
        if let Some(last_space) = self.buffer.rfind(|c: char| c.is_whitespace()) {
            // Flush everything up to and including the last whitespace
            let to_flush = &self.buffer[..=last_space];
            write!(self.stdout, "{}", to_flush)?;
            self.stdout.flush()?;

            // Keep the remaining partial word in the buffer
            self.buffer = self.buffer[last_space + 1..].to_string();
        }

        Ok(())
    }

    /// Flush any remaining buffered content. Call when the stream ends.
    pub fn finish(&mut self) -> io::Result<()> {
        if !self.buffer.is_empty() {
            write!(self.stdout, "{}", self.buffer)?;
            self.stdout.flush()?;
            self.buffer.clear();
        }
        Ok(())
    }
}
```

Word buffering is particularly effective for prose output, where the model generates tokens like `"The"`, `" quick"`, `" brown"`, `" fox"`. Each space triggers a flush of the preceding word. For code output, it is less ideal because long lines without spaces might stay buffered for too many tokens.

## Line-level buffering

Line buffering waits for a newline character before flushing. This is the default behavior of stdout when connected to a terminal, but you can implement it explicitly to handle code output better:

```rust
use std::io::{self, Write};

/// Buffers tokens and flushes at line boundaries.
/// Falls back to flushing after a timeout to prevent long lines from stalling.
pub struct LineBuffer {
    buffer: String,
    stdout: io::Stdout,
    max_pending_chars: usize,
}

impl LineBuffer {
    pub fn new(max_pending_chars: usize) -> Self {
        Self {
            buffer: String::new(),
            stdout: io::stdout(),
            max_pending_chars,
        }
    }

    pub fn push(&mut self, token: &str) -> io::Result<()> {
        self.buffer.push_str(token);

        // Flush on newline or if the buffer exceeds the maximum
        if self.buffer.contains('\n') || self.buffer.len() >= self.max_pending_chars {
            write!(self.stdout, "{}", self.buffer)?;
            self.stdout.flush()?;
            self.buffer.clear();
        }

        Ok(())
    }

    pub fn finish(&mut self) -> io::Result<()> {
        if !self.buffer.is_empty() {
            write!(self.stdout, "{}", self.buffer)?;
            self.stdout.flush()?;
            self.buffer.clear();
        }
        Ok(())
    }
}
```

The `max_pending_chars` safety valve prevents pathological cases where the model generates a very long line without any newlines. Setting it to 80 or 120 characters (one terminal width) is a reasonable default.

## Adaptive buffering

The best strategy is to adapt based on measured conditions. Here is a buffer that starts with per-token flushing and switches to word-level buffering if the token rate exceeds a threshold:

```rust
use std::io::{self, Write};
use std::time::Instant;

/// Automatically adjusts buffering granularity based on token arrival rate.
pub struct AdaptiveBuffer {
    buffer: String,
    stdout: io::Stdout,
    token_count: u32,
    window_start: Instant,
    /// Tokens per second above which we switch to word buffering.
    rate_threshold: f64,
    /// Whether we have switched to word-buffering mode.
    word_mode: bool,
}

impl AdaptiveBuffer {
    pub fn new(rate_threshold: f64) -> Self {
        Self {
            buffer: String::new(),
            stdout: io::stdout(),
            token_count: 0,
            window_start: Instant::now(),
            rate_threshold,
            word_mode: false,
        }
    }

    pub fn push(&mut self, token: &str) -> io::Result<()> {
        self.token_count += 1;
        self.buffer.push_str(token);

        // Recalculate rate every 10 tokens
        if self.token_count % 10 == 0 {
            let elapsed = self.window_start.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                let rate = self.token_count as f64 / elapsed;
                self.word_mode = rate > self.rate_threshold;
            }
        }

        if self.word_mode {
            // Word-level: flush at whitespace boundaries
            if let Some(last_space) = self.buffer.rfind(|c: char| c.is_whitespace()) {
                let to_flush = &self.buffer[..=last_space];
                write!(self.stdout, "{}", to_flush)?;
                self.stdout.flush()?;
                self.buffer = self.buffer[last_space + 1..].to_string();
            }
        } else {
            // Character-level: flush immediately
            write!(self.stdout, "{}", self.buffer)?;
            self.stdout.flush()?;
            self.buffer.clear();
        }

        Ok(())
    }

    pub fn finish(&mut self) -> io::Result<()> {
        if !self.buffer.is_empty() {
            write!(self.stdout, "{}", self.buffer)?;
            self.stdout.flush()?;
            self.buffer.clear();
        }
        Ok(())
    }
}
```

A rate threshold of 60.0 tokens per second is a reasonable starting point. Below that rate, the user perceives individual tokens appearing. Above it, the output flows so fast that batching by word looks equally smooth.

::: python Coming from Python
Python's `print()` function handles buffering automatically based on whether stdout is a terminal (line-buffered) or a pipe (block-buffered). The `flush=True` parameter overrides this. In Rust, you have explicit control over every layer:
```python
# Python: one option
print(token, end="", flush=True)
```
```rust
// Rust: you choose the strategy
adaptive_buffer.push(token)?;
```
This explicit control matters when your agent runs over SSH, in a Docker container, or piped to another process -- each environment benefits from a different buffering strategy.
:::

## Ring buffer for context retention

In long streaming sessions, you might not want to keep every token in memory. A ring buffer retains the last N characters while discarding older content:

```rust
use std::collections::VecDeque;

/// A fixed-capacity ring buffer that keeps the most recent characters.
pub struct RingBuffer {
    buffer: VecDeque<char>,
    capacity: usize,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push_str(&mut self, text: &str) {
        for ch in text.chars() {
            if self.buffer.len() >= self.capacity {
                self.buffer.pop_front();
            }
            self.buffer.push_back(ch);
        }
    }

    pub fn contents(&self) -> String {
        self.buffer.iter().collect()
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }
}
```

This is useful for displaying a "last N characters" preview in a status bar, or for maintaining a context window that you can search through for tool call results without keeping the entire conversation in the display buffer.

::: wild In the Wild
Claude Code uses an adaptive approach to rendering: it buffers tokens when generating code blocks (where line-at-a-time display looks better) and switches to immediate rendering for conversational text. OpenCode's Bubble Tea TUI batches all updates to a 16ms frame cycle regardless of token arrival rate, which naturally provides adaptive buffering at the UI layer.
:::

## Key Takeaways

- Flush-per-token is correct for most local terminal use but can bottleneck over high-latency connections like SSH.
- Word-level buffering reduces system calls while preserving a smooth reading experience for prose output.
- Line-level buffering with a `max_pending_chars` safety valve works well for code output where lines are the natural unit.
- Adaptive buffering measures the token arrival rate and switches strategies automatically, giving you the best of both worlds.
- A ring buffer is useful for keeping recent context in memory without unbounded growth during long streaming sessions.
