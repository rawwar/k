---
title: Signals and Timeouts
description: Implementing signal delivery, graceful shutdown sequences, and process timeouts to prevent runaway commands from blocking the agent.
---

# Signals and Timeouts

> **What you'll learn:**
> - How Unix signals (SIGTERM, SIGKILL, SIGINT) work and how to send them to child processes from Rust
> - Implementing a graceful shutdown pattern: SIGTERM followed by a grace period then SIGKILL
> - Building configurable timeout wrappers that kill long-running processes and return partial output

A coding agent that runs arbitrary user commands must handle a fundamental problem: what if a command never finishes? A process caught in an infinite loop, a test suite that hangs on a network call, a build that takes far longer than expected -- all of these will freeze your agent unless you enforce timeouts. Signals are the Unix mechanism for interrupting processes, and combining them with timeouts gives you robust control over runaway commands.

## Unix Signals Primer

Signals are asynchronous notifications sent to a process by the kernel or another process. There are dozens of signals, but these are the ones you will use in a coding agent:

| Signal | Number | Default Action | Can Be Caught? | Use Case |
|--------|--------|---------------|----------------|----------|
| SIGTERM | 15 | Terminate | Yes | Polite "please shut down" |
| SIGKILL | 9 | Terminate | No | Forceful kill (last resort) |
| SIGINT | 2 | Terminate | Yes | User pressed Ctrl+C |
| SIGSTOP | 19 | Stop | No | Pause the process |
| SIGCONT | 18 | Continue | Yes | Resume a stopped process |

The critical distinction is between SIGTERM and SIGKILL:

- **SIGTERM** asks the process to shut down gracefully. The process can catch this signal, clean up resources (close files, flush buffers, remove temp files), and exit.
- **SIGKILL** terminates the process immediately. The kernel removes it from the process table. No cleanup, no signal handler, no negotiation. The process simply ceases to exist.

## Sending Signals from Rust

The `Child` handle's `kill()` method sends SIGKILL on Unix. For other signals, you need the `nix` crate or the `libc` crate:

```rust
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

fn main() {
    let mut child = Command::new("sleep")
        .arg("60")
        .stdout(Stdio::null())
        .spawn()
        .expect("failed to spawn sleep");

    println!("Spawned child with PID: {}", child.id());

    // Wait a moment, then kill it
    thread::sleep(Duration::from_secs(2));

    // child.kill() sends SIGKILL
    child.kill().expect("failed to kill child");
    let status = child.wait().expect("failed to wait");

    println!("Child exited with: {}", status);
    // On Unix, this prints something like "signal: 9"
}
```

For SIGTERM (the graceful option), use the `nix` crate:

```rust
use std::process::{Command, Stdio};
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;

fn main() {
    let mut child = Command::new("sleep")
        .arg("60")
        .stdout(Stdio::null())
        .spawn()
        .expect("failed to spawn");

    let pid = Pid::from_raw(child.id() as i32);

    // Send SIGTERM (graceful shutdown request)
    kill(pid, Signal::SIGTERM).expect("failed to send SIGTERM");

    let status = child.wait().expect("failed to wait");
    println!("Child exited with: {}", status);
}
```

Add `nix` to your `Cargo.toml`:

```toml
[dependencies]
nix = { version = "0.29", features = ["signal"] }
```

## The Graceful Shutdown Pattern

Production agents use a two-phase shutdown: send SIGTERM, wait a grace period, then escalate to SIGKILL if the process has not exited. This gives well-behaved processes a chance to clean up while guaranteeing that misbehaving ones do not hang the agent.

```rust
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use std::process::Stdio;

async fn graceful_kill(child: &mut tokio::process::Child, grace_period: Duration) {
    let pid = match child.id() {
        Some(id) => Pid::from_raw(id as i32),
        None => return, // Process already exited
    };

    // Phase 1: Send SIGTERM
    let _ = kill(pid, Signal::SIGTERM);

    // Phase 2: Wait up to grace_period for the process to exit
    match timeout(grace_period, child.wait()).await {
        Ok(_) => {
            // Process exited within the grace period
        }
        Err(_) => {
            // Grace period expired -- escalate to SIGKILL
            let _ = child.kill().await;
        }
    }
}

#[tokio::main]
async fn main() {
    let mut child = Command::new("sleep")
        .arg("300")
        .stdout(Stdio::null())
        .spawn()
        .expect("failed to spawn");

    println!("Sending graceful shutdown...");
    graceful_kill(&mut child, Duration::from_secs(5)).await;
    println!("Process terminated");
}
```

::: python Coming from Python
Python's `subprocess.Popen` has `terminate()` (sends SIGTERM) and `kill()` (sends SIGKILL). A common Python pattern is:
```python
import subprocess, signal
proc = subprocess.Popen(["sleep", "300"])
proc.terminate()  # SIGTERM
try:
    proc.wait(timeout=5)  # Grace period
except subprocess.TimeoutExpired:
    proc.kill()  # SIGKILL
```
Rust's pattern is structurally identical, but using async/await with `tokio::time::timeout` makes the grace period non-blocking.
:::

## Implementing Timeouts with tokio::time::timeout

The most common use of signals in a coding agent is enforcing command timeouts. Here is a complete timeout wrapper:

```rust
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use std::process::Stdio;

#[derive(Debug)]
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
}

pub async fn run_with_timeout(
    program: &str,
    args: &[&str],
    timeout_duration: Duration,
) -> Result<CommandResult, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn {}: {}", program, e))?;

    // Wait for the process to complete, or timeout
    match timeout(timeout_duration, child.wait_with_output()).await {
        Ok(Ok(output)) => {
            // Process finished within the timeout
            Ok(CommandResult {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code(),
                timed_out: false,
            })
        }
        Ok(Err(e)) => {
            // Process errored
            Err(format!("Process error: {}", e))
        }
        Err(_) => {
            // Timeout expired -- kill the process
            let _ = child.kill().await;
            Ok(CommandResult {
                stdout: String::new(),
                stderr: format!("Command timed out after {:?}", timeout_duration),
                exit_code: None,
                timed_out: true,
            })
        }
    }
}

#[tokio::main]
async fn main() {
    // This will complete normally
    let result = run_with_timeout("echo", &["hello"], Duration::from_secs(5))
        .await
        .expect("command failed");
    println!("Result: {:?}", result);

    // This will timeout
    let result = run_with_timeout("sleep", &["60"], Duration::from_secs(2))
        .await
        .expect("command failed");
    println!("Timed out: {}", result.timed_out);
}
```

### Timeout with Streaming Output

When you stream output and enforce a timeout, you need to timeout the entire operation -- both the streaming and the final wait:

```rust
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::{timeout, Duration};
use std::process::Stdio;

pub async fn stream_with_timeout(
    program: &str,
    args: &[&str],
    timeout_duration: Duration,
) -> Result<(Vec<String>, bool), String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Spawn error: {}", e))?;

    let stdout = child.stdout.take().expect("no stdout");
    let mut lines_reader = BufReader::new(stdout).lines();
    let mut collected_lines = Vec::new();

    let result = timeout(timeout_duration, async {
        while let Some(line) = lines_reader
            .next_line()
            .await
            .map_err(|e| format!("Read error: {}", e))?
        {
            collected_lines.push(line);
        }
        Ok::<_, String>(())
    })
    .await;

    let timed_out = result.is_err();
    if timed_out {
        let _ = child.kill().await;
    }
    let _ = child.wait().await;

    Ok((collected_lines, timed_out))
}

#[tokio::main]
async fn main() {
    let (lines, timed_out) = stream_with_timeout("ping", &["-c", "100", "localhost"], Duration::from_secs(3))
        .await
        .expect("failed");

    println!("Collected {} lines, timed_out: {}", lines.len(), timed_out);
    for line in &lines {
        println!("  {}", line);
    }
}
```

## Killing Process Groups

A common pitfall: you spawn `sh -c "make all"` and kill the shell process when the timeout expires. But `make` spawned `gcc`, which spawned `cc1`, and those descendants are still running because you only killed the shell. The solution is to kill the entire process group.

```rust
use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

pub async fn run_with_group_timeout(
    program: &str,
    args: &[&str],
    timeout_duration: Duration,
) -> Result<(String, bool), String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Create a new process group so we can kill all descendants
        .process_group(0)
        .spawn()
        .map_err(|e| format!("Spawn error: {}", e))?;

    let pid = child.id().expect("no pid") as i32;

    match timeout(timeout_duration, child.wait_with_output()).await {
        Ok(Ok(output)) => {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            Ok((combined, false))
        }
        Ok(Err(e)) => Err(format!("Process error: {}", e)),
        Err(_) => {
            // Kill the entire process group
            let _ = killpg(Pid::from_raw(pid), Signal::SIGKILL);
            let _ = child.wait().await;
            Ok(("Command timed out".to_string(), true))
        }
    }
}

#[tokio::main]
async fn main() {
    let (output, timed_out) = run_with_group_timeout(
        "sh",
        &["-c", "echo start; sleep 60; echo done"],
        Duration::from_secs(2),
    )
    .await
    .expect("failed");

    println!("Output: {}", output.trim());
    println!("Timed out: {}", timed_out);
}
```

The `.process_group(0)` call tells the OS to place the child in a new process group (with the child as group leader). Then `killpg` sends SIGKILL to every process in that group -- the shell, `sleep`, and any other descendants.

::: wild In the Wild
Claude Code creates a new process group for every shell command it executes. When a command exceeds its timeout, the agent kills the entire group to ensure no orphan processes linger. This is especially important during long agent sessions where dozens of commands may be executed -- leaked processes would accumulate and consume system resources.
:::

## Key Takeaways

- SIGTERM requests graceful shutdown (can be caught and handled); SIGKILL forces immediate termination (cannot be caught). Always try SIGTERM first, escalate to SIGKILL after a grace period.
- `tokio::time::timeout` wraps any future with a deadline -- use it with `child.wait_with_output()` to enforce command timeouts without blocking the async runtime.
- Kill process groups (not just the top-level child) to clean up all descendant processes. Use `.process_group(0)` when spawning and `killpg` when killing.
- A timeout wrapper should return partial output and a timeout flag so the agent can report what happened to the user and the LLM.
- The graceful shutdown pattern (SIGTERM, grace period, SIGKILL) is the industry standard for production agent systems.
