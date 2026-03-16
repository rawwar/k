---
title: Process Spawning
description: Learn how to spawn child processes in Rust using std::process::Command and Tokio's async process API for non-blocking execution.
---

# Process Spawning

> **What you'll learn:**
> - How to use `std::process::Command` to spawn synchronous child processes
> - How to use `tokio::process::Command` for async, non-blocking process execution
> - How to handle exit statuses and propagate errors from spawned processes

Your coding agent needs to run commands like `cargo test`, `git status`, or `ls -la` and report the results back to the LLM. In this subchapter, you will learn the two fundamental approaches Rust provides for spawning child processes: the synchronous `std::process::Command` from the standard library, and the asynchronous `tokio::process::Command` that integrates with Tokio's event loop.

## The Unix Process Model in 30 Seconds

When you run a command in your terminal, the operating system creates a new **child process**. This child inherits the parent's environment variables and working directory, runs independently, and eventually exits with a numeric **status code** (0 for success, non-zero for failure). Rust gives you fine-grained control over every step of this lifecycle.

## Synchronous Process Spawning with `std::process::Command`

The simplest way to run a command in Rust is with `std::process::Command`. Here is a complete program that runs `echo hello` and prints the output:

```rust
use std::process::Command;

fn main() {
    let output = Command::new("echo")
        .arg("hello from the agent")
        .output()
        .expect("failed to execute process");

    println!("Status: {}", output.status);
    println!("Stdout: {}", String::from_utf8_lossy(&output.stdout));
    println!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
}
```

Let's break down what happens here:

1. **`Command::new("echo")`** creates a new command builder targeting the `echo` binary. This does not run anything yet -- it just prepares the configuration.
2. **`.arg("hello from the agent")`** adds a single argument. Each call to `.arg()` adds one argument to the argument list. You can also use `.args(&["arg1", "arg2"])` to add multiple at once.
3. **`.output()`** spawns the child process, waits for it to finish, and collects all of its stdout and stderr into memory. This call **blocks** the current thread until the process exits.
4. **`output.status`** is an `ExitStatus` that tells you whether the command succeeded.
5. **`output.stdout`** and **`output.stderr`** are `Vec<u8>` byte buffers containing the raw output.

`String::from_utf8_lossy` converts bytes to a string, replacing any invalid UTF-8 sequences with the Unicode replacement character. This is safer than `String::from_utf8()` for commands that might emit binary data.

### Three Ways to Run a Command

`std::process::Command` offers three methods for execution, each suited to different situations:

```rust
use std::process::Command;

fn main() {
    // 1. output() - capture everything, wait for completion
    let output = Command::new("ls")
        .arg("-la")
        .output()
        .expect("failed to run ls");
    println!("Captured {} bytes of stdout", output.stdout.len());

    // 2. status() - just get the exit code, inherit parent's stdio
    let status = Command::new("echo")
        .arg("this prints directly to YOUR terminal")
        .status()
        .expect("failed to run echo");
    println!("Exit code: {}", status.code().unwrap_or(-1));

    // 3. spawn() - start the process and get a handle back immediately
    let mut child = Command::new("sleep")
        .arg("1")
        .spawn()
        .expect("failed to start sleep");
    println!("Child PID: {}", child.id());
    let exit = child.wait().expect("failed to wait on child");
    println!("Sleep finished with: {}", exit);
}
```

- **`output()`** is what you want for the agent's shell tool -- you need to capture stdout and stderr to send them back to the LLM.
- **`status()`** runs the command with inherited stdio (output goes directly to the terminal). Useful for interactive commands but not for agent tools.
- **`spawn()`** returns a `Child` handle immediately without waiting. This is the foundation for implementing timeouts, which we will cover in a later subchapter.

::: tip Coming from Python
In Python you would run a subprocess with `subprocess.run()`:
```python
import subprocess
result = subprocess.run(["echo", "hello"], capture_output=True, text=True)
print(result.stdout)
print(result.returncode)
```
Rust's `Command::new("echo").arg("hello").output()` serves the same purpose. The key difference: Python's `subprocess.run()` returns a `CompletedProcess` with string fields if you pass `text=True`, while Rust gives you raw `Vec<u8>` bytes that you must explicitly convert. Rust also returns a `Result`, forcing you to handle the case where the command could not even be started (for example, if the binary does not exist).
:::

## Async Process Spawning with `tokio::process::Command`

Your coding agent is built on Tokio, an async runtime. The synchronous `Command::output()` blocks the entire thread while the process runs. This is fine for quick commands, but a `cargo build` in a large project could take minutes, blocking your agent from doing anything else.

Tokio provides `tokio::process::Command`, which has an almost identical API but returns futures instead of blocking:

```rust
use tokio::process::Command;

#[tokio::main]
async fn main() {
    let output = Command::new("echo")
        .arg("hello from async Rust")
        .output()
        .await
        .expect("failed to execute process");

    println!("Status: {}", output.status);
    println!("Stdout: {}", String::from_utf8_lossy(&output.stdout));
}
```

The only visible difference is the `.await` after `.output()`. Under the hood, Tokio registers the child process's file descriptors with its event loop (using `epoll` on Linux or `kqueue` on macOS). When the process writes to stdout or exits, the Tokio reactor wakes the task -- no thread is blocked in the meantime.

This matters because your agent's agentic loop is async. While a shell command runs, the loop can still handle other tasks like processing user input or managing concurrent tool calls.

### Spawning with `spawn()` for More Control

Just like the synchronous version, `tokio::process::Command` has a `spawn()` method that returns immediately with a `Child` handle:

```rust
use tokio::process::Command;

#[tokio::main]
async fn main() {
    let mut child = Command::new("sleep")
        .arg("2")
        .spawn()
        .expect("failed to start sleep");

    println!("Child is running with PID: {:?}", child.id());

    // Do other work while the child runs...
    println!("Doing other async work...");

    // Now wait for the child to finish
    let status = child.wait().await.expect("child process failed");
    println!("Child exited with: {}", status);
}
```

The `spawn()` + `wait()` pattern is the building block for timeouts. In a later subchapter, you will wrap the `child.wait()` future with `tokio::time::timeout` to kill processes that run too long.

## Handling Exit Status

A command's exit status tells you whether it succeeded. By convention, exit code 0 means success, and any non-zero code means failure. Here is how you check it:

```rust
use tokio::process::Command;

#[tokio::main]
async fn main() {
    let output = Command::new("ls")
        .arg("/nonexistent/path")
        .output()
        .await
        .expect("failed to execute ls");

    if output.status.success() {
        println!("Command succeeded!");
        println!("{}", String::from_utf8_lossy(&output.stdout));
    } else {
        let code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Command failed with exit code {}: {}", code, stderr);
    }
}
```

Note the distinction between two kinds of failure:

1. **The command could not start** -- `Command::output()` returns `Err`. This happens when the binary does not exist, you lack execute permission, or the OS refuses to create the process.
2. **The command ran but exited with a non-zero status** -- `Command::output()` returns `Ok(output)` where `output.status.success()` is false. This is the normal case for a command like `grep` that found no matches, or `cargo test` with failing tests.

For the agent's shell tool, both cases need to be reported back to the LLM so it can decide what to do next.

## Putting It Together: A Basic Shell Executor

Let's write the function that will serve as the foundation for our shell tool. In `src/tools/shell.rs`, start with this:

```rust
use anyhow::Result;
use tokio::process::Command as TokioCommand;

/// The result of executing a shell command.
#[derive(Debug)]
pub struct ShellOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Execute a shell command and capture its output.
pub async fn execute_command(program: &str, args: &[&str]) -> Result<ShellOutput> {
    let output = TokioCommand::new(program)
        .args(args)
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to execute '{}': {}", program, e))?;

    Ok(ShellOutput {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let result = execute_command("echo", &["hello", "world"]).await?;
    println!("Exit code: {}", result.exit_code);
    println!("Stdout: {}", result.stdout);
    println!("Stderr: {}", result.stderr);
    Ok(())
}
```

This `execute_command` function will grow throughout this chapter as you add timeouts, environment control, output truncation, and safety checks. But even in this minimal form, it handles both failure modes: it propagates OS-level errors via `?` and captures non-zero exit codes in the `ShellOutput` struct.

::: info In the Wild
Claude Code spawns all shell commands through `tokio::process::Command` wrapped in a timeout. The async approach is essential because Claude Code can handle multiple tool calls in parallel -- while one command runs, another can be starting. OpenCode takes the same approach in Go, using `exec.CommandContext` which is Go's equivalent of combining `Command` with a timeout context.
:::

## Key Takeaways

- Use `std::process::Command` for synchronous process execution and `tokio::process::Command` for async execution that integrates with your agent's event loop.
- The `.output()` method captures stdout/stderr into memory; `.spawn()` gives you a `Child` handle for more control over the process lifecycle.
- Always check both `Result` (could the process start?) and `ExitStatus` (did the process succeed?) -- they represent different failure modes.
- Convert raw byte output with `String::from_utf8_lossy` to handle commands that might emit non-UTF-8 data.
- The async `spawn()` + `wait()` pattern is the foundation for timeout enforcement, which you will implement in a later subchapter.
