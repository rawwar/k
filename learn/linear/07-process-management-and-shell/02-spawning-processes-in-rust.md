---
title: Spawning Processes in Rust
description: Using Rust's std::process::Command and tokio::process::Command to create and configure child processes with type-safe APIs.
---

# Spawning Processes in Rust

> **What you'll learn:**
> - How to use std::process::Command to spawn synchronous child processes with argument and environment configuration
> - How tokio::process::Command extends the API for async process management in an event-driven agent
> - The difference between spawn(), output(), and status() and when to use each method

Now that you understand the Unix process model, let's put it into practice with Rust. The standard library provides `std::process::Command`, a builder that configures and launches child processes. For async applications -- which includes nearly every coding agent -- Tokio provides `tokio::process::Command` with an almost identical API that integrates with the async runtime. In this subchapter you will learn both APIs, understand their differences, and know when to reach for each.

## The Command Builder Pattern

Rust uses the builder pattern for process configuration. You create a `Command`, chain configuration methods, and then call one of three execution methods. Here is the simplest possible example:

```rust
use std::process::Command;

fn main() {
    let status = Command::new("echo")
        .arg("hello")
        .arg("world")
        .status()
        .expect("failed to execute echo");

    println!("Exited with: {}", status);
}
```

`Command::new("echo")` specifies the program to run. The `.arg()` calls append arguments. Finally, `.status()` spawns the process, waits for it to finish, and returns an `ExitStatus`.

Notice that arguments are added one at a time. This is deliberate -- it avoids shell injection vulnerabilities because each argument is passed directly to the OS as a separate string. No shell interprets these arguments, so metacharacters like `;`, `|`, and `$` are treated as literal text.

### Adding Multiple Arguments at Once

If you have a vector of arguments, use `.args()` (plural):

```rust
use std::process::Command;

fn main() {
    let args = vec!["test", "--release", "--", "--nocapture"];

    let status = Command::new("cargo")
        .args(&args)
        .status()
        .expect("failed to run cargo test");

    if status.success() {
        println!("All tests passed!");
    } else {
        println!("Tests failed with code: {:?}", status.code());
    }
}
```

::: python Coming from Python
In Python, `subprocess.run(["cargo", "test", "--release"])` takes a list of arguments in the constructor. Rust's builder pattern achieves the same thing but separates the program name (`Command::new`) from the arguments (`.arg()` / `.args()`). Both languages strongly recommend the list form over passing a single string to a shell -- Rust makes the safe choice the default, while Python requires you to avoid `shell=True` by discipline.
:::

## Three Ways to Execute

`Command` offers three execution methods. Choosing the right one depends on what you need from the child process.

### status() -- Just the Exit Code

Use `status()` when you only care whether the command succeeded. The child's stdout and stderr go directly to the parent's terminal (they are inherited):

```rust
use std::process::Command;

fn main() {
    let status = Command::new("cargo")
        .arg("check")
        .status()
        .expect("failed to start cargo");

    if !status.success() {
        eprintln!("cargo check failed");
        std::process::exit(1);
    }
}
```

### output() -- Exit Code + Captured Output

Use `output()` when you need to read what the command printed. This captures both stdout and stderr into `Vec<u8>` buffers:

```rust
use std::process::Command;

fn main() {
    let output = Command::new("rustc")
        .arg("--version")
        .output()
        .expect("failed to run rustc");

    let version = String::from_utf8_lossy(&output.stdout);
    println!("Rust compiler version: {}", version.trim());

    if !output.stderr.is_empty() {
        let errors = String::from_utf8_lossy(&output.stderr);
        eprintln!("Stderr: {}", errors);
    }
}
```

The `output()` method internally sets stdout and stderr to `Stdio::piped()`, spawns the child, reads all output into memory, waits for exit, and returns an `Output` struct containing `status`, `stdout`, and `stderr`.

### spawn() -- Full Control

Use `spawn()` when you need more control: streaming output line by line, writing to stdin, or running the process in the background. It returns a `Child` handle immediately, without waiting:

```rust
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};

fn main() {
    let mut child = Command::new("ping")
        .args(["-c", "3", "localhost"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start ping");

    // Read stdout line by line while the process runs
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let line = line.expect("failed to read line");
            println!("[ping] {}", line);
        }
    }

    let status = child.wait().expect("failed to wait on child");
    println!("ping exited with: {}", status);
}
```

The `Child` struct gives you access to the child's PID, its stdin/stdout/stderr handles (if piped), and methods to wait or kill the process. This is the method you will use most in a coding agent, because agents typically need to stream output in real time.

## Error Handling: Two Failure Points

Process spawning has two distinct failure points, and Rust's type system makes you handle both:

1. **Spawn failure** -- the OS could not create the process (program not found, permission denied). The `spawn()`, `status()`, and `output()` methods all return `Result<_, io::Error>`.

2. **Non-zero exit** -- the process ran but returned a failure code. The `ExitStatus` type has a `.success()` method and a `.code()` method.

```rust
use std::process::Command;

fn run_tool(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to spawn {}: {}", program, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{} exited with code {:?}: {}",
            program,
            output.status.code(),
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.trim().to_string())
}

fn main() {
    match run_tool("rustc", &["--version"]) {
        Ok(version) => println!("Found: {}", version),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Async Processes with tokio::process::Command

A coding agent is fundamentally async -- it handles user input, LLM API calls, and tool execution concurrently. Blocking the async runtime with a synchronous `Command::new("cargo").output()` stalls everything. Tokio provides `tokio::process::Command` with the same builder API but async execution methods.

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
```

Now use `tokio::process::Command`:

```rust
use tokio::process::Command;

#[tokio::main]
async fn main() {
    let output = Command::new("cargo")
        .args(["check", "--message-format=json"])
        .output()
        .await
        .expect("failed to run cargo check");

    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("Cargo output:\n{}", stdout);
}
```

The key difference: `output()` returns a `Future` that you `.await`. While this future is pending, Tokio can run other tasks -- handling user input, polling the LLM, or running other tools. The API is otherwise identical to the synchronous version.

### Async spawn() for Streaming

The async `spawn()` is where things get especially useful. The `Child` handle it returns has async methods for waiting, and its stdout/stderr handles implement `tokio::io::AsyncRead`:

```rust
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use std::process::Stdio;

#[tokio::main]
async fn main() {
    let mut child = Command::new("cargo")
        .args(["test", "--", "--nocapture"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn cargo test");

    let stdout = child.stdout.take().expect("no stdout handle");
    let mut reader = BufReader::new(stdout).lines();

    while let Some(line) = reader.next_line().await.expect("failed to read line") {
        println!("[test] {}", line);
    }

    let status = child.wait().await.expect("failed to wait");
    println!("Tests exited with: {}", status);
}
```

This is the pattern your agent will use to execute tools and stream results back to the user in real time. The `await` points let Tokio interleave this work with other async tasks.

::: wild In the Wild
Production coding agents like Claude Code and OpenCode use async process spawning exclusively. The agent needs to remain responsive to user cancellation, LLM streaming updates, and multiple concurrent tool executions. Blocking on a synchronous `output()` call would freeze the entire agent until the command finishes -- unacceptable for a tool that might run `cargo build` for minutes.
:::

## Choosing Between sync and async

| Scenario | Use |
|----------|-----|
| Quick one-shot tool (agent startup, version check) | `std::process::Command` is fine |
| Any command during agent operation | `tokio::process::Command` |
| Need to stream output to user | `tokio::process::Command` with `spawn()` |
| Need to enforce timeouts | `tokio::process::Command` with `tokio::time::timeout` |

As a rule of thumb: if you are inside an `async fn`, use `tokio::process::Command`. If you are in synchronous initialization code that runs before the Tokio runtime starts, use `std::process::Command`.

## Key Takeaways

- Rust's `Command` builder configures the program name, arguments, environment, and stdio before spawning -- matching the fork/exec configuration gap described in the previous subchapter.
- `status()` gives you just the exit code, `output()` captures stdout/stderr into memory, and `spawn()` gives you a `Child` handle for streaming and fine-grained control.
- Process spawning has two error paths: spawn failure (OS error) and non-zero exit (application error). Rust's `Result` and `ExitStatus` types make you handle both explicitly.
- For async agents, `tokio::process::Command` provides the same API with `.await`-based execution, allowing the agent to remain responsive during long-running commands.
- Always prefer the async variant inside your agent's event loop to avoid blocking the Tokio runtime.
