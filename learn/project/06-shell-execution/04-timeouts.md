---
title: Timeouts
description: Enforce execution time limits on spawned processes using Tokio's timeout mechanisms to prevent runaway commands from blocking the agent.
---

# Timeouts

> **What you'll learn:**
> - How to use `tokio::time::timeout` to wrap async process execution
> - How to gracefully terminate a process that exceeds its time limit
> - How to configure per-command and global default timeout values

A coding agent without timeouts is a ticking time bomb. Imagine the LLM generates `find / -name "*.rs"` -- a command that could scan the entire filesystem for minutes. Or it runs `cargo build` on a project with a pathological dependency graph that takes forever. Without a timeout, your agent hangs indefinitely, consuming resources and blocking the user.

In this subchapter, you will implement timeout enforcement using `tokio::time::timeout`, building on the `ShellCommand` builder from the previous subchapter.

## The Timeout Wrapper Pattern

Tokio provides `tokio::time::timeout`, which wraps any future with a deadline. If the future does not complete within the specified duration, it returns `Err(Elapsed)` and the inner future is dropped:

```rust
use std::time::Duration;
use tokio::time;

#[tokio::main]
async fn main() {
    let result = time::timeout(
        Duration::from_secs(2),
        tokio::time::sleep(Duration::from_secs(10)),
    )
    .await;

    match result {
        Ok(()) => println!("Completed in time"),
        Err(_) => println!("Timed out!"),
    }
}
```

This pattern applies directly to process execution. You wrap the `.output()` call (or the `child.wait()` call) with a timeout, and if the process does not finish in time, you get an error instead of waiting forever.

## Applying Timeouts to Process Execution

Here is how to combine the timeout with your shell command execution:

```rust
use anyhow::{anyhow, Result};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command as TokioCommand;
use tokio::time;

#[derive(Debug, Clone)]
pub struct ShellOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
    pub timed_out: bool,
}

pub async fn execute_with_timeout(
    command: &str,
    timeout_duration: Duration,
) -> Result<ShellOutput> {
    let mut child = TokioCommand::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn command: {}", e))?;

    // Wrap the wait-and-capture in a timeout
    let result = time::timeout(timeout_duration, child.wait_with_output()).await;

    match result {
        Ok(Ok(output)) => {
            // Process completed within the timeout
            Ok(ShellOutput {
                exit_code: output.status.code().unwrap_or(-1),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                success: output.status.success(),
                timed_out: false,
            })
        }
        Ok(Err(e)) => {
            // Process failed to complete (IO error)
            Err(anyhow!("Process execution error: {}", e))
        }
        Err(_elapsed) => {
            // Timeout expired -- kill the process
            let _ = child.kill().await;
            Ok(ShellOutput {
                exit_code: -1,
                stdout: String::new(),
                stderr: format!(
                    "Command timed out after {} seconds",
                    timeout_duration.as_secs()
                ),
                success: false,
                timed_out: true,
            })
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // This completes quickly
    let result = execute_with_timeout("echo 'fast command'", Duration::from_secs(5)).await?;
    println!("Fast: {} (timed_out={})", result.stdout.trim(), result.timed_out);

    // This will time out
    let result = execute_with_timeout("sleep 60", Duration::from_secs(2)).await?;
    println!("Slow: {} (timed_out={})", result.stderr.trim(), result.timed_out);

    Ok(())
}
```

The `match` on the timeout result handles three cases:

1. **`Ok(Ok(output))`** -- the process finished within the timeout and returned its output.
2. **`Ok(Err(e))`** -- the process encountered an I/O error (rare, but possible).
3. **`Err(_elapsed)`** -- the timeout expired. You must kill the process explicitly.

::: tip Coming from Python
Python's `subprocess.run()` has a built-in `timeout` parameter:
```python
try:
    result = subprocess.run(["sleep", "60"], timeout=2, capture_output=True)
except subprocess.TimeoutExpired as e:
    print(f"Timed out after {e.timeout} seconds")
    print(f"Partial output: {e.stdout}")
```
Rust does not have a built-in timeout on `Command` -- you compose it yourself with `tokio::time::timeout`. This gives you more control: you can choose whether to SIGTERM or SIGKILL the process, capture partial output, or implement a graceful shutdown sequence. The Python version always sends SIGKILL on timeout.
:::

## The Kill-After-Timeout Problem

When `tokio::time::timeout` returns `Err`, the inner future (the process wait) is dropped. But **dropping the future does not kill the child process**. The child keeps running as an orphan. You must explicitly call `child.kill()` to terminate it:

```rust
Err(_elapsed) => {
    // CRITICAL: kill the process, or it becomes an orphan
    let _ = child.kill().await;
    // ...
}
```

The `child.kill()` call sends SIGKILL on Unix, which immediately terminates the process. In the next subchapter on signal handling, you will implement a more graceful approach: send SIGTERM first, wait briefly, then escalate to SIGKILL.

## Integrating Timeouts into the ShellCommand Builder

Let's integrate timeout support into the builder from the previous subchapter. Add an `execute` method that handles the full lifecycle:

```rust
use anyhow::{anyhow, Result};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command as TokioCommand;
use tokio::time;

// Assuming ShellCommand and ShellOutput are defined as before

impl ShellCommand {
    /// Execute the command with the configured timeout.
    pub async fn execute(&self) -> Result<ShellOutput> {
        let mut cmd = self.build();
        let mut child = cmd.spawn()
            .map_err(|e| anyhow!(
                "Failed to spawn '{}': {}", self.display_command(), e
            ))?;

        let timeout_duration = self.get_timeout();

        let result = time::timeout(
            timeout_duration,
            child.wait_with_output(),
        )
        .await;

        match result {
            Ok(Ok(output)) => Ok(ShellOutput {
                exit_code: output.status.code().unwrap_or(-1),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                success: output.status.success(),
                timed_out: false,
            }),
            Ok(Err(e)) => Err(anyhow!("Process error: {}", e)),
            Err(_) => {
                // Kill the timed-out process
                let _ = child.kill().await;
                Ok(ShellOutput {
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: format!(
                        "Command '{}' timed out after {}s",
                        self.display_command(),
                        timeout_duration.as_secs()
                    ),
                    success: false,
                    timed_out: true,
                })
            }
        }
    }
}
```

Now callers can set the timeout through the builder and execute in one fluent chain:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let result = ShellCommand::new("cargo test")
        .working_dir("/path/to/project")
        .timeout(Duration::from_secs(120))
        .execute()
        .await?;

    println!("{}", result.to_tool_result());
    Ok(())
}
```

## Choosing the Right Timeout

Different commands need different timeouts. Here are sensible defaults for a coding agent:

| Command Type | Suggested Timeout | Reason |
|---|---|---|
| Quick queries (`ls`, `echo`, `cat`) | 10 seconds | These should complete instantly |
| Build tools (`cargo build`, `npm install`) | 120 seconds | Large projects take time |
| Test suites (`cargo test`, `pytest`) | 300 seconds | Test suites can be slow |
| Search tools (`grep -r`, `find`) | 30 seconds | Filesystem traversal varies |
| Default | 30 seconds | Safe middle ground |

In practice, you will use a single default timeout (30 seconds is common) and let the LLM or user override it for specific commands. The builder's `.timeout()` method makes this easy.

::: details How does tokio::time::timeout work under the hood?
`tokio::time::timeout` creates a future that races your inner future against a timer. Tokio maintains a hierarchical timing wheel that efficiently tracks thousands of timers. When you await the combined future, Tokio's executor polls both the inner future and the timer. Whichever completes first determines the result. The timer does not spawn a separate thread -- it is integrated into Tokio's cooperative scheduling. This makes timeouts very lightweight; you can have thousands of concurrent timeouts without performance concerns.
:::

## Key Takeaways

- Always enforce a timeout on shell commands to prevent runaway processes from blocking the agent indefinitely.
- Use `tokio::time::timeout` to wrap `child.wait_with_output()` -- it returns `Err(Elapsed)` if the process does not finish in time.
- When a timeout fires, you **must** explicitly kill the child process with `child.kill()` or it will continue running as an orphan.
- Add a `timed_out` field to your `ShellOutput` so the LLM knows a command was interrupted and can decide whether to retry with a longer timeout.
- Provide sensible default timeouts (30 seconds) but allow per-command overrides for long-running operations like builds and test suites.
