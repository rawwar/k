---
title: Signal Handling
description: Send Unix signals to child processes for graceful shutdown, forced termination, and proper process group management.
---

# Signal Handling

> **What you'll learn:**
> - How to send SIGTERM and SIGKILL signals to child processes in Rust
> - How to manage process groups to ensure all descendant processes are cleaned up
> - How to implement a graceful shutdown sequence with escalating signal severity

In the previous subchapter, you used `child.kill()` to terminate timed-out processes. That sends SIGKILL, which is the nuclear option -- the process is terminated immediately with no chance to clean up. Many programs (like build tools and test runners) expect to receive SIGTERM first so they can save state, close files, and exit gracefully. In this subchapter, you will implement a proper signal escalation strategy and handle process groups so that child processes and their descendants are all cleaned up correctly.

## Unix Signals Primer

Signals are the Unix mechanism for inter-process communication. The signals relevant to process management are:

| Signal | Number | Default Action | Can Be Caught? |
|---|---|---|---|
| SIGTERM | 15 | Terminate gracefully | Yes |
| SIGKILL | 9 | Terminate immediately | No |
| SIGINT | 2 | Interrupt (like Ctrl+C) | Yes |

**SIGTERM** is the polite way to ask a process to stop. Well-behaved programs catch SIGTERM and perform cleanup before exiting. **SIGKILL** is the forceful way -- the kernel terminates the process immediately, bypassing all signal handlers. The process cannot catch, block, or ignore SIGKILL.

For your agent, the ideal strategy is: send SIGTERM, wait a few seconds for graceful shutdown, then send SIGKILL if the process is still running.

## Sending Signals in Rust

On Unix systems, you send signals using the `nix` crate or the `libc` crate. Here is how to send SIGTERM to a child process using `nix`:

```rust
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use std::process::Stdio;
use tokio::process::Command as TokioCommand;
use tokio::time::{self, Duration};

/// Send SIGTERM to a child process.
fn send_sigterm(pid: u32) -> Result<(), nix::Error> {
    signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM)
}

/// Send SIGKILL to a child process.
fn send_sigkill(pid: u32) -> Result<(), nix::Error> {
    signal::kill(Pid::from_raw(pid as i32), Signal::SIGKILL)
}

#[tokio::main]
async fn main() {
    let mut child = TokioCommand::new("sleep")
        .arg("60")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .expect("failed to spawn");

    let pid = child.id().expect("no pid");
    println!("Started child with PID {}", pid);

    // Send SIGTERM
    send_sigterm(pid).expect("failed to send SIGTERM");
    println!("Sent SIGTERM to PID {}", pid);

    // Wait for the process to exit
    let status = child.wait().await.expect("failed to wait");
    println!("Child exited with: {}", status);
}
```

::: python Coming from Python
Python's `subprocess.Popen` provides `terminate()` for SIGTERM and `kill()` for SIGKILL:
```python
import subprocess, signal
proc = subprocess.Popen(["sleep", "60"])
proc.terminate()  # Sends SIGTERM
proc.wait(timeout=5)  # Wait for graceful exit
proc.kill()  # Sends SIGKILL if still alive
```
Rust's `child.kill()` only sends SIGKILL. For SIGTERM, you need to use the `nix` crate and the process ID, or use the standard library's `Command` on Unix with the `id()` method. This gives you more explicit control but requires more code.
:::

## Graceful Shutdown with Escalation

The best practice for terminating a process is a three-step escalation:

1. **SIGTERM**: Ask nicely. The process should clean up and exit.
2. **Wait**: Give it a few seconds to comply.
3. **SIGKILL**: Force termination if it did not exit.

Here is a reusable function that implements this pattern:

```rust
use anyhow::Result;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use tokio::process::Child;
use tokio::time::{self, Duration};

/// Gracefully terminate a child process with SIGTERM -> wait -> SIGKILL escalation.
///
/// Returns Ok(()) if the process was successfully terminated.
pub async fn graceful_kill(child: &mut Child, grace_period: Duration) -> Result<()> {
    let pid = match child.id() {
        Some(pid) => pid,
        None => {
            // Process already exited
            return Ok(());
        }
    };

    // Step 1: Send SIGTERM
    let _ = signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM);

    // Step 2: Wait for the grace period
    match time::timeout(grace_period, child.wait()).await {
        Ok(Ok(_status)) => {
            // Process exited gracefully after SIGTERM
            return Ok(());
        }
        Ok(Err(e)) => {
            // Error waiting -- try SIGKILL anyway
            eprintln!("Error waiting for process {}: {}", pid, e);
        }
        Err(_) => {
            // Grace period expired -- process did not exit
        }
    }

    // Step 3: Send SIGKILL
    let _ = child.kill().await;
    let _ = child.wait().await;

    Ok(())
}
```

Let's integrate this into the `execute` method from the timeouts subchapter:

```rust
impl ShellCommand {
    pub async fn execute(&self) -> Result<ShellOutput> {
        let mut cmd = self.build();
        let mut child = cmd.spawn()
            .map_err(|e| anyhow::anyhow!(
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
            Ok(Err(e)) => Err(anyhow::anyhow!("Process error: {}", e)),
            Err(_) => {
                // Graceful shutdown: SIGTERM -> 5s wait -> SIGKILL
                graceful_kill(&mut child, Duration::from_secs(5)).await?;

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

## Process Groups: Catching Escaped Children

When your shell command is `sh -c "cargo build"`, the `sh` process spawns `cargo`, which spawns `rustc`, which might spawn `cc`. If you send SIGTERM only to the `sh` process, all those child processes can become orphans.

The solution is **process groups**. On Unix, you can put a child process into its own process group, and then send signals to the entire group at once:

```rust
use std::os::unix::process::CommandExt;
use std::process::Stdio;
use tokio::process::Command as TokioCommand;

/// Create a command that spawns in its own process group.
fn build_command_with_process_group(command: &str) -> TokioCommand {
    let mut cmd = TokioCommand::new("sh");
    cmd.arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    // Set the child to be its own process group leader
    unsafe {
        cmd.pre_exec(|| {
            // setsid() creates a new session and process group
            libc::setsid();
            Ok(())
        });
    }

    cmd
}
```

The `pre_exec` closure runs in the child process after `fork()` but before `exec()`. Calling `setsid()` makes the child the leader of a new process group. Now you can send signals to the entire group using a negative PID:

```rust
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;

/// Send a signal to an entire process group.
fn kill_process_group(pid: u32, sig: Signal) -> Result<(), nix::Error> {
    // Negative PID means "send to the entire process group"
    signal::kill(Pid::from_raw(-(pid as i32)), sig)
}
```

This ensures that when you terminate a timed-out `cargo build`, all the compiler processes it spawned are also terminated.

::: wild In the Wild
Claude Code creates a new process group for every shell command execution. When a timeout fires, it sends SIGTERM to the entire process group, waits briefly, then escalates to SIGKILL. This prevents orphaned compiler or test processes from consuming system resources after the agent moves on. Codex CLI takes a similar approach, using process groups and a SIGTERM-then-SIGKILL escalation pattern.
:::

## Handling SIGINT for User Interruption

When the user presses Ctrl+C in the terminal, your agent receives SIGINT. You need to forward this to running child processes so they can shut down:

```rust
use tokio::signal;

/// Set up a SIGINT handler that forwards the signal to the active child process.
pub async fn setup_interrupt_handler(child_pid: Option<u32>) {
    signal::ctrl_c().await.expect("failed to listen for ctrl+c");

    if let Some(pid) = child_pid {
        // Forward SIGINT to the child process group
        let _ = nix::sys::signal::kill(
            Pid::from_raw(-(pid as i32)),
            Signal::SIGINT,
        );
    }
}
```

In practice, you would use `tokio::select!` to race the interrupt handler against the process wait, ensuring that a Ctrl+C during command execution terminates the child and returns control to the agent's main loop.

## Key Takeaways

- Use SIGTERM first for graceful shutdown, then escalate to SIGKILL after a configurable grace period (5 seconds is typical).
- Put child processes in their own process group using `setsid()` in `pre_exec`, then signal the entire group with a negative PID to catch all descendant processes.
- Rust's `child.kill()` sends SIGKILL only. For SIGTERM, use the `nix` crate's `signal::kill()` with the process ID.
- Always `wait()` on a child process after killing it to prevent zombie processes (processes that have exited but whose exit status has not been collected).
- Forward user interrupts (SIGINT/Ctrl+C) to running child processes so they can perform their own cleanup.
