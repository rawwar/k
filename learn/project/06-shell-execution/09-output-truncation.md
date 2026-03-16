---
title: Output Truncation
description: Implement strategies for truncating large command output to fit within context window limits while preserving the most useful information.
---

# Output Truncation

> **What you'll learn:**
> - How to set byte and line limits on captured process output
> - How to implement head, tail, and middle-truncation strategies for long output
> - How to communicate truncation metadata back to the LLM so it can request more if needed

A single `find / -name "*.rs"` can produce megabytes of output. Running `cat` on a large log file might return hundreds of thousands of lines. If you pass all of that to the LLM, you waste context window tokens, increase latency, and potentially exceed the model's input limit. Output truncation is not optional -- it is essential for a working agent.

## Why Truncation Matters

LLMs have finite context windows. Even with models that support 200K tokens, each token of command output displaces a token that could be used for reasoning, conversation history, or code. A few untruncated `grep -r` results can easily consume half the available context.

The goal of truncation is to keep the most **useful** portion of the output while staying within size limits. "Useful" depends on the command:

- For `cargo test`, the most useful part is the end (which tests passed/failed).
- For `ls -la`, the beginning is usually most important.
- For `grep -r`, both the beginning and end matter.

## Basic Byte-Limited Capture

The simplest approach is to stop reading after a certain number of bytes. Here is a function that reads from an async reader with a byte limit:

```rust
use tokio::io::{AsyncRead, AsyncReadExt};

/// Read up to `max_bytes` from an async reader.
/// Returns the bytes read and whether the output was truncated.
async fn read_limited(
    reader: &mut (impl AsyncRead + Unpin),
    max_bytes: usize,
) -> std::io::Result<(Vec<u8>, bool)> {
    let mut buf = Vec::with_capacity(max_bytes.min(65536));
    let mut total = 0;
    let mut temp = [0u8; 8192];

    loop {
        let remaining = max_bytes - total;
        if remaining == 0 {
            // Check if there is more data we are not reading
            let extra = reader.read(&mut [0u8; 1]).await?;
            return Ok((buf, extra > 0));
        }

        let to_read = remaining.min(temp.len());
        let n = reader.read(&mut temp[..to_read]).await?;

        if n == 0 {
            // EOF -- all output was captured
            return Ok((buf, false));
        }

        buf.extend_from_slice(&temp[..n]);
        total += n;
    }
}
```

This function reads up to `max_bytes` from the stream, then checks if there is any remaining data. It returns a `(data, truncated)` tuple so you can inform the LLM when output was cut short.

## Three Truncation Strategies

Different commands benefit from different truncation approaches. Let's implement all three:

### Head Truncation (Keep the Beginning)

Keep the first N bytes/lines and discard the rest. Best for commands like `ls`, `cat`, `head`, and `find`:

```rust
/// Truncate output to the first `max_lines` lines.
pub fn truncate_head(output: &str, max_lines: usize) -> (String, bool) {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= max_lines {
        return (output.to_string(), false);
    }

    let kept: String = lines[..max_lines].join("\n");
    let omitted = lines.len() - max_lines;
    let result = format!(
        "{}\n\n[... {} more lines truncated ...]",
        kept, omitted
    );
    (result, true)
}
```

### Tail Truncation (Keep the End)

Keep the last N lines and discard the beginning. Best for `cargo test`, `cargo build`, and other commands where the summary is at the end:

```rust
/// Truncate output to the last `max_lines` lines.
pub fn truncate_tail(output: &str, max_lines: usize) -> (String, bool) {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= max_lines {
        return (output.to_string(), false);
    }

    let omitted = lines.len() - max_lines;
    let kept: String = lines[lines.len() - max_lines..].join("\n");
    let result = format!(
        "[... {} lines truncated ...]\n\n{}",
        omitted, kept
    );
    (result, true)
}
```

### Middle Truncation (Keep Head and Tail)

Keep the first and last N lines, dropping the middle. Best for `grep -r` and other search commands where both the beginning and the summary at the end are valuable:

```rust
/// Keep the first `head_lines` and last `tail_lines`, truncating the middle.
pub fn truncate_middle(
    output: &str,
    head_lines: usize,
    tail_lines: usize,
) -> (String, bool) {
    let lines: Vec<&str> = output.lines().collect();
    let total_keep = head_lines + tail_lines;

    if lines.len() <= total_keep {
        return (output.to_string(), false);
    }

    let head: String = lines[..head_lines].join("\n");
    let tail: String = lines[lines.len() - tail_lines..].join("\n");
    let omitted = lines.len() - total_keep;
    let result = format!(
        "{}\n\n[... {} lines omitted ...]\n\n{}",
        head, omitted, tail
    );
    (result, true)
}
```

## Putting It Together: A Smart Truncator

Let's build a truncator that chooses the right strategy automatically based on configurable limits:

```rust
/// Configuration for output truncation.
#[derive(Debug, Clone)]
pub struct TruncationConfig {
    /// Maximum number of bytes to capture from the process.
    pub max_bytes: usize,
    /// Maximum number of lines to include in the result.
    pub max_lines: usize,
    /// How many lines to keep from the head when using middle truncation.
    pub head_lines: usize,
    /// How many lines to keep from the tail when using middle truncation.
    pub tail_lines: usize,
    /// The truncation strategy to use.
    pub strategy: TruncationStrategy,
}

#[derive(Debug, Clone)]
pub enum TruncationStrategy {
    /// Keep the first N lines.
    Head,
    /// Keep the last N lines.
    Tail,
    /// Keep the first and last N lines.
    Middle,
    /// Automatically choose based on output characteristics.
    Auto,
}

impl Default for TruncationConfig {
    fn default() -> Self {
        Self {
            max_bytes: 512 * 1024,  // 512 KB
            max_lines: 500,
            head_lines: 200,
            tail_lines: 200,
            strategy: TruncationStrategy::Middle,
        }
    }
}

impl TruncationConfig {
    /// Apply truncation to the given output string.
    pub fn truncate(&self, output: &str) -> (String, bool) {
        // First, apply byte limit
        let byte_limited = if output.len() > self.max_bytes {
            // Find the last newline within the byte limit to avoid
            // cutting in the middle of a UTF-8 character or line
            let truncated = &output[..self.max_bytes];
            match truncated.rfind('\n') {
                Some(pos) => &output[..pos],
                None => truncated,
            }
        } else {
            output
        };

        // Then, apply line limit using the configured strategy
        let lines: Vec<&str> = byte_limited.lines().collect();
        if lines.len() <= self.max_lines {
            return (byte_limited.to_string(), output.len() > self.max_bytes);
        }

        match self.strategy {
            TruncationStrategy::Head => truncate_head(byte_limited, self.max_lines),
            TruncationStrategy::Tail => truncate_tail(byte_limited, self.max_lines),
            TruncationStrategy::Middle => {
                truncate_middle(byte_limited, self.head_lines, self.tail_lines)
            }
            TruncationStrategy::Auto => {
                // Default to middle truncation for most commands
                truncate_middle(byte_limited, self.head_lines, self.tail_lines)
            }
        }
    }
}
```

::: tip Coming from Python
Python does not have built-in output truncation for subprocesses. You would typically do it manually:
```python
result = subprocess.run(["find", "/"], capture_output=True, text=True)
lines = result.stdout.splitlines()
if len(lines) > 500:
    truncated = "\n".join(lines[:200]) + f"\n\n[... {len(lines)-400} lines omitted ...]\n\n" + "\n".join(lines[-200:])
else:
    truncated = result.stdout
```
Rust's ownership model makes truncation cleaner because you can take ownership of the output string and process it in place without worrying about aliasing or mutation bugs.
:::

## Integrating Truncation into ShellOutput

Update the `ShellOutput` type to track truncation metadata:

```rust
#[derive(Debug, Clone)]
pub struct ShellOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
    pub timed_out: bool,
    /// Whether stdout was truncated.
    pub stdout_truncated: bool,
    /// Whether stderr was truncated.
    pub stderr_truncated: bool,
    /// Original byte count before truncation.
    pub original_stdout_bytes: usize,
}

impl ShellOutput {
    /// Format the output for the LLM, including truncation notices.
    pub fn to_tool_result(&self) -> String {
        let mut result = String::new();

        if !self.stdout.is_empty() {
            result.push_str(&self.stdout);
        }

        if self.stdout_truncated {
            result.push_str(&format!(
                "\n\n[Output truncated. Original size: {} bytes. \
                 Use head/tail/grep for targeted output.]",
                self.original_stdout_bytes
            ));
        }

        if !self.stderr.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str("[stderr]\n");
            result.push_str(&self.stderr);
        }

        if !self.success {
            result.push_str(&format!("\n[exit code: {}]", self.exit_code));
        }

        if self.timed_out {
            result.push_str("\n[command timed out]");
        }

        if result.is_empty() {
            result.push_str("[no output]");
        }

        result
    }
}
```

The truncation notice tells the LLM exactly what happened and suggests a more targeted approach. A smart LLM will respond by running `head -50 file.log` or `grep "error" file.log` instead of trying to read the entire file again.

::: info In the Wild
Claude Code uses middle truncation by default, keeping the first and last portions of command output. The truncation message includes the original byte count and a hint that the LLM can use more targeted commands to see specific parts. This creates a feedback loop where the LLM learns to use `grep`, `head`, `tail`, and other filtering commands to get precisely the output it needs.
:::

## Key Takeaways

- Always enforce output size limits. A 512 KB / 500 line default is a reasonable starting point for most coding agent use cases.
- Use **middle truncation** (keep head + tail) as the default strategy. It preserves both the beginning (headers, file listings) and the end (summaries, error messages).
- Track truncation metadata (`truncated: bool`, `original_bytes: usize`) and include it in the tool result so the LLM knows output was cut and can request targeted output.
- Apply byte limits first (to bound memory usage), then line limits (to bound context window consumption).
- The truncation message should suggest alternative commands (`head`, `tail`, `grep`) to guide the LLM toward more efficient output retrieval.
