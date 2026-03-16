---
title: Stdout Stderr Capture
description: Capture and separate standard output and standard error streams from child processes to provide structured execution results.
---

# Stdout Stderr Capture

> **What you'll learn:**
> - How to configure piped stdout and stderr on a child process
> - How to read streams asynchronously without deadlocking on buffer limits
> - How to structure captured output into a unified result type for the tool system

In the previous subchapter you used `.output()` to capture everything at once. That works for simple cases, but production agents need finer control. What if a command produces megabytes of output that could fill up memory? What if you want to stream output back to the user in real-time while also collecting it for the LLM? In this subchapter, you will learn how to pipe stdout and stderr separately, read them asynchronously, and structure the results for your tool system.

## Understanding Unix Streams

Every Unix process has three standard streams:

- **stdin** (file descriptor 0) -- input to the process
- **stdout** (file descriptor 1) -- normal output
- **stderr** (file descriptor 2) -- error and diagnostic output

When your agent runs `cargo test`, the test results go to stdout and compiler warnings go to stderr. The LLM needs both streams to understand what happened, so you need to capture them separately.

## Configuring Piped Streams

By default, a child process inherits the parent's stdio -- its output goes directly to your terminal. To capture it programmatically, you configure the streams as **piped**:

```rust
use std::process::Stdio;
use tokio::process::Command;

#[tokio::main]
async fn main() {
    let mut child = Command::new("ls")
        .arg("-la")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .expect("failed to spawn ls");

    // Take ownership of the stdout/stderr handles
    let stdout = child.stdout.take().expect("stdout was not piped");
    let stderr = child.stderr.take().expect("stderr was not piped");

    // Read both streams (we'll improve this next)
    use tokio::io::AsyncReadExt;
    let mut stdout_buf = Vec::new();
    let mut stderr_buf = Vec::new();

    let mut stdout = stdout;
    let mut stderr = stderr;

    stdout.read_to_end(&mut stdout_buf).await.expect("failed to read stdout");
    stderr.read_to_end(&mut stderr_buf).await.expect("failed to read stderr");

    let status = child.wait().await.expect("failed to wait on child");

    println!("Status: {}", status);
    println!("Stdout ({} bytes): {}", stdout_buf.len(), String::from_utf8_lossy(&stdout_buf));
    println!("Stderr ({} bytes): {}", stderr_buf.len(), String::from_utf8_lossy(&stderr_buf));
}
```

Three important details here:

- **`Stdio::piped()`** creates an OS pipe connecting the child's stream to a handle you can read from.
- **`Stdio::null()`** for stdin means the child gets no input. Without this, the child might try to read from your terminal.
- **`.take()`** moves the stream handle out of the `Child` struct. You must take ownership before reading.

## The Deadlock Trap

The code above has a subtle bug that is safe for small outputs but dangerous for large ones. Can you spot it?

We read stdout completely, *then* read stderr. If the child writes a lot to stderr while its stdout pipe buffer is full, the child blocks waiting for someone to read stdout, but we are waiting for stderr to finish -- **deadlock**.

The OS pipe buffer is typically 64 KB. If a command produces more than 64 KB on stdout and any output on stderr, the sequential read can hang forever.

The fix is to read both streams **concurrently**:

```rust
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

#[tokio::main]
async fn main() {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg("echo 'stdout line'; echo 'stderr line' >&2; echo 'more stdout'")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .expect("failed to spawn");

    let mut stdout = child.stdout.take().expect("no stdout");
    let mut stderr = child.stderr.take().expect("no stderr");

    // Read both streams concurrently using tokio::join!
    let (stdout_result, stderr_result) = tokio::join!(
        async {
            let mut buf = Vec::new();
            stdout.read_to_end(&mut buf).await.map(|_| buf)
        },
        async {
            let mut buf = Vec::new();
            stderr.read_to_end(&mut buf).await.map(|_| buf)
        }
    );

    let stdout_bytes = stdout_result.expect("failed to read stdout");
    let stderr_bytes = stderr_result.expect("failed to read stderr");

    let status = child.wait().await.expect("failed to wait");

    println!("Exit: {}", status);
    println!("Stdout: {}", String::from_utf8_lossy(&stdout_bytes));
    println!("Stderr: {}", String::from_utf8_lossy(&stderr_bytes));
}
```

`tokio::join!` runs both read futures concurrently on the same task. As the child writes to either stream, Tokio reads from whichever pipe has data available. No deadlock possible.

::: tip Coming from Python
In Python, `subprocess.run(capture_output=True)` handles this concurrency for you internally -- it uses threads to drain both pipes simultaneously. You never see the deadlock risk. In Rust, `.output()` also handles it internally (it uses `tokio::join!` under the hood in the async version), but when you use `spawn()` and read pipes manually, the concurrency is your responsibility.
```python
# Python hides the complexity -- two threads drain stdout and stderr
result = subprocess.run(["cmd"], capture_output=True, text=True)
# In Rust with spawn(), you must use tokio::join! or similar
```
:::

## Why Not Just Use `.output()`?

If `.output()` handles the concurrent reading internally, why bother with manual pipe reading? Several reasons matter for a production agent:

1. **Streaming**: You might want to display output to the user as it arrives, not wait for the command to finish.
2. **Size limits**: You might want to stop reading after collecting a certain number of bytes (output truncation, covered later in this chapter).
3. **Interleaving**: Some tools need to see stdout and stderr in the order they were produced, which requires reading from both in an event-driven fashion.

For the basic shell tool, `.output()` is perfectly fine. But understanding pipes is essential for the advanced features you will add later.

## Building a Structured Output Type

The LLM needs structured information about command results. Let's define a proper output type and a capture function:

```rust
use anyhow::Result;
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::Command as TokioCommand;

/// Structured result from executing a shell command.
#[derive(Debug, Clone)]
pub struct ShellOutput {
    /// The command's exit code. -1 if the process was killed by a signal.
    pub exit_code: i32,
    /// Captured standard output as a UTF-8 string.
    pub stdout: String,
    /// Captured standard error as a UTF-8 string.
    pub stderr: String,
    /// Whether the command completed successfully (exit code 0).
    pub success: bool,
}

impl ShellOutput {
    /// Format the output for inclusion in an LLM message.
    pub fn to_tool_result(&self) -> String {
        let mut result = String::new();

        if !self.stdout.is_empty() {
            result.push_str(&self.stdout);
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

        if result.is_empty() {
            result.push_str("[no output]");
        }

        result
    }
}

/// Execute a command with piped stdout and stderr, capturing both concurrently.
pub async fn execute_and_capture(
    program: &str,
    args: &[&str],
) -> Result<ShellOutput> {
    let mut child = TokioCommand::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn '{}': {}", program, e))?;

    let mut stdout_handle = child.stdout.take().expect("stdout not piped");
    let mut stderr_handle = child.stderr.take().expect("stderr not piped");

    let (stdout_result, stderr_result) = tokio::join!(
        async {
            let mut buf = Vec::new();
            stdout_handle.read_to_end(&mut buf).await.map(|_| buf)
        },
        async {
            let mut buf = Vec::new();
            stderr_handle.read_to_end(&mut buf).await.map(|_| buf)
        }
    );

    let stdout_bytes = stdout_result
        .map_err(|e| anyhow::anyhow!("Failed to read stdout: {}", e))?;
    let stderr_bytes = stderr_result
        .map_err(|e| anyhow::anyhow!("Failed to read stderr: {}", e))?;

    let status = child.wait().await
        .map_err(|e| anyhow::anyhow!("Failed to wait for process: {}", e))?;

    let exit_code = status.code().unwrap_or(-1);

    Ok(ShellOutput {
        exit_code,
        stdout: String::from_utf8_lossy(&stdout_bytes).into_owned(),
        stderr: String::from_utf8_lossy(&stderr_bytes).into_owned(),
        success: status.success(),
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    // Successful command
    let result = execute_and_capture("echo", &["hello world"]).await?;
    println!("=== Tool result ===\n{}", result.to_tool_result());

    // Failing command
    let result = execute_and_capture("ls", &["/nonexistent"]).await?;
    println!("\n=== Tool result ===\n{}", result.to_tool_result());

    Ok(())
}
```

The `to_tool_result()` method formats the output in a way that is informative for the LLM. It includes stderr only when present (prefixed with `[stderr]` so the LLM knows it is error output), appends the exit code only on failure, and returns `[no output]` for commands that produce nothing.

## Handling Binary Output

Not every command produces text. If a command emits binary data (like `xxd` or `cat` on a binary file), `String::from_utf8_lossy` replaces invalid bytes with the Unicode replacement character. For an agent shell tool, this is the right behavior -- the LLM can only process text, so binary output needs to be represented safely.

If you ever need to detect binary output, check for null bytes:

```rust
fn is_likely_binary(data: &[u8]) -> bool {
    data.iter().take(8192).any(|&b| b == 0)
}
```

You could use this to provide a helpful message like `[binary output, 45,231 bytes]` instead of sending garbled replacement characters to the LLM.

::: info In the Wild
Claude Code formats tool results with clear labels for stdout and stderr. When a command produces both streams, they are sent separately so the LLM can distinguish between normal output and error messages. Codex takes a similar approach, prefixing stderr with a marker to avoid confusion in the LLM's context.
:::

## Key Takeaways

- Use `Stdio::piped()` to capture stdout and stderr, and `Stdio::null()` for stdin to prevent the child from reading terminal input.
- Always read stdout and stderr **concurrently** (via `tokio::join!`) to avoid deadlocks when pipe buffers fill up.
- The `.output()` method handles concurrent reading internally and is sufficient for most agent shell tool needs.
- Structure your output into a typed `ShellOutput` with `exit_code`, `stdout`, `stderr`, and `success` fields for clean integration with the tool system.
- Use `String::from_utf8_lossy` for safe conversion of potentially non-UTF-8 command output.
