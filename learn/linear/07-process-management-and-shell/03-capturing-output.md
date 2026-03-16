---
title: Capturing Output
description: Techniques for capturing, buffering, and processing stdout and stderr output from child processes in both blocking and streaming modes.
---

# Capturing Output

> **What you'll learn:**
> - How to capture the complete output of a child process into memory for parsing and validation
> - Streaming output line-by-line from a long-running process to provide real-time feedback
> - Handling mixed stdout/stderr output and preserving interleaved ordering

A coding agent lives and dies by its ability to read what commands print. When the agent runs `cargo test`, it needs to parse the output to determine which tests failed. When it runs a linter, it extracts file paths and line numbers from the output. When it executes a user's script, it captures both stdout and stderr to report back to the LLM. This subchapter covers the two fundamental approaches -- bulk capture and line-by-line streaming -- and the tradeoffs between them.

## Bulk Capture with output()

The simplest approach captures everything at once. The `output()` method waits for the child to finish and returns all of stdout and stderr as byte vectors:

```rust
use std::process::Command;

fn main() {
    let output = Command::new("cargo")
        .args(["test", "--no-run"])
        .output()
        .expect("failed to run cargo");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("=== STDOUT ===\n{}", stdout);
    println!("=== STDERR ===\n{}", stderr);
    println!("Exit code: {:?}", output.status.code());
}
```

`String::from_utf8_lossy` converts the raw bytes to a string, replacing any invalid UTF-8 sequences with the replacement character. This is important because some commands produce binary output or output in non-UTF-8 encodings. For a coding agent, lossy conversion is usually acceptable -- you want to see the text even if a few bytes are garbled.

### When Bulk Capture Works Well

Bulk capture is the right choice when:

- The command finishes quickly (under a few seconds).
- The output is small enough to fit comfortably in memory (under a few megabytes).
- You need all the output before you can do anything useful with it (e.g., parsing JSON output from `cargo test --message-format=json`).

### When Bulk Capture Falls Short

Bulk capture is problematic when:

- The command runs for minutes (e.g., `cargo build` on a large project). The user sees nothing until it finishes.
- The output is very large. A test suite with verbose logging might produce hundreds of megabytes.
- You want to kill the command early based on output content (e.g., stop after the first test failure).

::: tip Coming from Python
Python's `subprocess.run(capture_output=True)` behaves identically -- it waits for the process to complete, then gives you `result.stdout` and `result.stderr` as bytes. The async equivalent is `asyncio.create_subprocess_exec()` with `await process.communicate()`. Rust's `output()` is the direct analog of both.
:::

## Streaming Output Line by Line

For long-running commands, streaming gives the user real-time feedback and lets the agent make decisions as output arrives. You use `spawn()` with `Stdio::piped()`, then read from the child's stdout handle:

```rust
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};

fn main() {
    let mut child = Command::new("cargo")
        .args(["build", "--release"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn cargo build");

    // Stream stderr line by line (cargo writes progress to stderr)
    let stderr = child.stderr.take().expect("no stderr handle");
    let reader = BufReader::new(stderr);

    for line in reader.lines() {
        match line {
            Ok(line) => println!("[cargo] {}", line),
            Err(e) => {
                eprintln!("Error reading line: {}", e);
                break;
            }
        }
    }

    let status = child.wait().expect("failed to wait on cargo");
    println!("Build finished with: {}", status);
}
```

`BufReader` wraps the raw pipe handle and provides `lines()`, which yields one `String` per line. The loop runs concurrently with the child process -- as the child writes output, the parent reads it.

### Async Streaming with Tokio

For an async agent, use `tokio::process::Command` and `tokio::io::BufReader`:

```rust
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use std::process::Stdio;

async fn run_and_stream(program: &str, args: &[&str]) -> Result<i32, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn failed: {}", e))?;

    let stdout = child.stdout.take().expect("no stdout");
    let mut lines = BufReader::new(stdout).lines();

    while let Some(line) = lines.next_line().await.map_err(|e| format!("read error: {}", e))? {
        println!("[output] {}", line);
    }

    let status = child.wait().await.map_err(|e| format!("wait failed: {}", e))?;
    Ok(status.code().unwrap_or(-1))
}

#[tokio::main]
async fn main() {
    match run_and_stream("cargo", &["check"]).await {
        Ok(code) => println!("Exited with code: {}", code),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

The `next_line().await` call yields control back to Tokio whenever there is no output ready to read, allowing other tasks to make progress.

## Capturing Both Streams Simultaneously

A common mistake is reading stdout to completion and then reading stderr (or vice versa). This can deadlock if the child fills one pipe's buffer while the parent is blocked reading the other. The OS pipe buffer is typically 64 KB -- once full, the child blocks on its write call, and if the parent is blocked reading the other pipe, both processes wait forever.

The correct approach reads both streams concurrently. With Tokio, spawn separate tasks:

```rust
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use std::process::Stdio;

#[tokio::main]
async fn main() {
    let mut child = Command::new("cargo")
        .args(["test"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn");

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // Read both streams concurrently
    let stdout_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        let mut output = Vec::new();
        while let Some(line) = lines.next_line().await.expect("stdout read error") {
            output.push(line);
        }
        output
    });

    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        let mut output = Vec::new();
        while let Some(line) = lines.next_line().await.expect("stderr read error") {
            output.push(line);
        }
        output
    });

    let stdout_lines = stdout_task.await.expect("stdout task panicked");
    let stderr_lines = stderr_task.await.expect("stderr task panicked");

    let status = child.wait().await.expect("wait failed");

    println!("--- STDOUT ({} lines) ---", stdout_lines.len());
    for line in &stdout_lines {
        println!("  {}", line);
    }

    println!("--- STDERR ({} lines) ---", stderr_lines.len());
    for line in &stderr_lines {
        println!("  {}", line);
    }

    println!("Exit: {}", status);
}
```

By spawning separate Tokio tasks for stdout and stderr, both pipes drain simultaneously and neither can deadlock.

## Truncating Large Output

An agent that sends process output to an LLM must respect token limits. If `cargo test --nocapture` produces 50,000 lines of debug output, sending all of it wastes tokens and may exceed the context window. A practical pattern is to capture output with a size limit:

```rust
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use std::process::Stdio;

const MAX_LINES: usize = 200;
const MAX_BYTES: usize = 50_000;

async fn capture_bounded(program: &str, args: &[&str]) -> (Vec<String>, bool) {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn failed");

    let stdout = child.stdout.take().unwrap();
    let mut lines_reader = BufReader::new(stdout).lines();
    let mut lines = Vec::new();
    let mut total_bytes = 0;
    let mut truncated = false;

    while let Some(line) = lines_reader.next_line().await.expect("read error") {
        total_bytes += line.len();
        lines.push(line);

        if lines.len() >= MAX_LINES || total_bytes >= MAX_BYTES {
            truncated = true;
            break;
        }
    }

    // Still need to wait for the child even if we stopped reading
    let _ = child.wait().await;
    (lines, truncated)
}

#[tokio::main]
async fn main() {
    let (lines, truncated) = capture_bounded("find", &["/usr", "-type", "f"]).await;
    println!("Captured {} lines", lines.len());
    if truncated {
        println!("(output was truncated)");
    }
}
```

When the agent reports results to the LLM, it includes a note like "Output truncated after 200 lines" so the LLM knows the output is incomplete.

::: info In the Wild
Claude Code captures command output and applies intelligent truncation before feeding it back to the model. It preserves the first and last sections of output (head + tail) because error messages often appear at the end while context appears at the beginning. This "head-and-tail" strategy retains the most useful information when output exceeds token budgets.
:::

## Parsing Structured Output

Some tools emit structured output that is easier to parse than free-form text. Cargo, for example, supports JSON output:

```rust
use tokio::process::Command;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct CargoMessage {
    reason: String,
    #[serde(default)]
    message: Option<CompilerMessage>,
}

#[derive(Debug, Deserialize)]
struct CompilerMessage {
    message: String,
    level: String,
}

#[tokio::main]
async fn main() {
    let output = Command::new("cargo")
        .args(["check", "--message-format=json"])
        .output()
        .await
        .expect("failed to run cargo");

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        if let Ok(msg) = serde_json::from_str::<CargoMessage>(line) {
            if let Some(compiler_msg) = msg.message {
                println!("[{}] {}", compiler_msg.level, compiler_msg.message);
            }
        }
    }
}
```

When a tool supports structured output, always prefer it. Parsing JSON is more reliable than extracting information from human-readable text with regex.

## Key Takeaways

- Use `output()` for quick commands where you need all the output at once; use `spawn()` with piped handles for long-running commands that benefit from streaming.
- Always read stdout and stderr concurrently (e.g., via separate Tokio tasks) to avoid pipe deadlocks.
- Apply output truncation with line and byte limits to prevent large outputs from overwhelming the LLM's context window.
- Prefer structured output formats (JSON, machine-readable flags) over parsing human-readable text when tools support them.
- Use `String::from_utf8_lossy` to safely convert command output bytes to strings, tolerating non-UTF-8 content.
