---
title: Stdin Stdout Stderr
description: Deep dive into the three standard file descriptors, how they connect processes via pipes, and patterns for bidirectional communication with child processes.
---

# Stdin Stdout Stderr

> **What you'll learn:**
> - The role of file descriptors 0, 1, and 2 in Unix and how they map to stdin, stdout, and stderr
> - How to pipe data into a child process's stdin and read from its stdout simultaneously without deadlocking
> - Patterns for redirecting, merging, and separating standard streams for different use cases

Every Unix process starts with three open file descriptors: standard input (fd 0), standard output (fd 1), and standard error (fd 2). These three channels form the communication backbone between your agent and the commands it runs. Understanding how to wire them up correctly is the difference between an agent that reliably captures tool output and one that deadlocks mysteriously or loses error messages.

## The Three Standard Streams

Each stream has a distinct role by convention:

| File Descriptor | Name | Purpose |
|----------------|------|---------|
| 0 | stdin | Input data fed to the process |
| 1 | stdout | Primary output (results, data) |
| 2 | stderr | Diagnostic output (errors, warnings, progress) |

When you type a command in a terminal, your keyboard input flows to the process through stdin, the process writes its results to stdout, and error messages go to stderr. The terminal displays both stdout and stderr -- they appear interleaved on your screen -- but they are separate streams that can be redirected independently.

For a coding agent, the typical pattern is:

- **stdin**: Usually empty (most tools read from files, not stdin). Occasionally used to feed data to tools like `python -c` or `jq`.
- **stdout**: Captured and parsed to extract tool results.
- **stderr**: Captured to detect errors and warnings. Some tools (like `cargo`) write progress information to stderr.

## Stdio Configuration in Rust

Rust's `Command` builder lets you configure each stream independently using the `Stdio` type:

```rust
use std::process::{Command, Stdio};

fn main() {
    let child = Command::new("grep")
        .args(["pattern", "-"])
        .stdin(Stdio::piped())    // We will write to it
        .stdout(Stdio::piped())   // We will read from it
        .stderr(Stdio::inherit()) // Goes to our terminal
        .spawn()
        .expect("failed to spawn grep");
}
```

The three `Stdio` options are:

- **`Stdio::inherit()`** -- the child shares the parent's stream. Output goes directly to the terminal (or wherever the parent's stream points). This is the default for all three streams when you use `status()`.
- **`Stdio::piped()`** -- creates a pipe between parent and child. The parent gets a handle to read from (for stdout/stderr) or write to (for stdin).
- **`Stdio::null()`** -- connects the stream to `/dev/null`. Output is silently discarded; reads return EOF immediately.

::: tip Coming from Python
Python's `subprocess.Popen` offers the same three options with different names: `subprocess.PIPE`, `subprocess.DEVNULL`, and `None` (inherit). Rust's `Stdio::piped()` is `subprocess.PIPE`, `Stdio::null()` is `subprocess.DEVNULL`, and `Stdio::inherit()` is the default `None`. The concepts are identical -- only the spelling differs.
:::

## Writing to a Child's Stdin

Some tools expect input on stdin. For example, you might pipe JSON to `jq` for formatting, or feed source code to a linter. Here is how to write to a child's stdin:

```rust
use std::process::{Command, Stdio};
use std::io::Write;

fn main() {
    let mut child = Command::new("wc")
        .arg("-l")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn wc");

    // Write data to the child's stdin
    let stdin = child.stdin.as_mut().expect("no stdin handle");
    stdin.write_all(b"line one\nline two\nline three\n").expect("write failed");

    // IMPORTANT: drop stdin to close the pipe, signaling EOF to the child
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("wait failed");
    let line_count = String::from_utf8_lossy(&output.stdout);
    println!("Line count: {}", line_count.trim());
}
```

The critical step is **dropping the stdin handle**. When you drop (close) the write end of the pipe, the child receives EOF on its stdin. Many tools wait for EOF before processing input and producing output. If you forget to close stdin, the child waits forever for more input, and your agent waits forever for output -- a classic deadlock.

### Async Stdin Writing

With Tokio, the stdin handle implements `AsyncWrite`:

```rust
use tokio::process::Command;
use tokio::io::AsyncWriteExt;
use std::process::Stdio;

#[tokio::main]
async fn main() {
    let mut child = Command::new("python3")
        .args(["-c", "import sys; data = sys.stdin.read(); print(f'Got {len(data)} bytes')"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn python");

    let mut stdin = child.stdin.take().expect("no stdin");
    stdin.write_all(b"hello from the agent").await.expect("write failed");
    drop(stdin); // Signal EOF

    let output = child.wait_with_output().await.expect("wait failed");
    println!("{}", String::from_utf8_lossy(&output.stdout));
}
```

## Bidirectional Communication: Avoiding Deadlocks

The trickiest scenario is reading stdout while writing to stdin. If the child's output pipe buffer fills up before you finish writing to stdin, the child blocks on its write, your agent blocks on its write, and everything deadlocks.

The solution is to handle reading and writing concurrently. With Tokio, you can spawn separate tasks:

```rust
use tokio::process::Command;
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
use std::process::Stdio;

#[tokio::main]
async fn main() {
    let mut child = Command::new("sort")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn sort");

    let mut stdin = child.stdin.take().expect("no stdin");
    let stdout = child.stdout.take().expect("no stdout");

    // Write to stdin in one task
    let write_task = tokio::spawn(async move {
        let data = "banana\napple\ncherry\ndate\n";
        stdin.write_all(data.as_bytes()).await.expect("write failed");
        // stdin is dropped here, closing the pipe
    });

    // Read from stdout in another task
    let read_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        let mut result = Vec::new();
        while let Some(line) = lines.next_line().await.expect("read failed") {
            result.push(line);
        }
        result
    });

    // Wait for both tasks to complete
    write_task.await.expect("write task panicked");
    let sorted_lines = read_task.await.expect("read task panicked");

    let status = child.wait().await.expect("wait failed");

    println!("Sorted output:");
    for line in &sorted_lines {
        println!("  {}", line);
    }
    println!("Exit: {}", status);
}
```

The write task and read task run concurrently. The write task feeds data into stdin and then drops the handle (signaling EOF). The read task drains stdout. Because both tasks make progress simultaneously, neither pipe buffer fills up, and there is no deadlock.

## Merging Stdout and Stderr

Sometimes you want stdout and stderr combined into a single stream, preserving the original interleaving order. This is useful when the interleaving of normal output and error messages matters for understanding what happened.

Rust does not have a built-in "merge" option, but you can redirect stderr to stdout by using the `Stdio::from` on a `ChildStdout`:

```rust
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};

fn main() {
    let mut child = Command::new("sh")
        .args(["-c", "echo stdout-line; echo stderr-line >&2; echo another-stdout"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn");

    // Read both streams with separate threads to preserve ordering as much as possible
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let stdout_thread = std::thread::spawn(move || {
        BufReader::new(stdout)
            .lines()
            .filter_map(|l| l.ok())
            .map(|l| format!("[out] {}", l))
            .collect::<Vec<_>>()
    });

    let stderr_thread = std::thread::spawn(move || {
        BufReader::new(stderr)
            .lines()
            .filter_map(|l| l.ok())
            .map(|l| format!("[err] {}", l))
            .collect::<Vec<_>>()
    });

    let stdout_lines = stdout_thread.join().expect("stdout thread panicked");
    let stderr_lines = stderr_thread.join().expect("stderr thread panicked");

    let status = child.wait().expect("wait failed");

    for line in &stdout_lines {
        println!("{}", line);
    }
    for line in &stderr_lines {
        println!("{}", line);
    }
    println!("Exit: {}", status);
}
```

Note that when reading stdout and stderr separately, you lose the exact interleaving order. The OS pipes buffer data independently, so there is no reliable way to reconstruct the original order. If exact ordering matters, you can redirect stderr to stdout at the shell level with `2>&1`, which merges them into a single stream before they reach your agent:

```rust
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};

fn main() {
    let mut child = Command::new("sh")
        .args(["-c", "echo stdout; echo stderr >&2; echo more-stdout"])
        .arg("2>&1") // This won't work here -- see below
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn");

    // The correct way is to put the redirect inside the shell command:
    let mut child = Command::new("sh")
        .args(["-c", "echo stdout; echo stderr >&2; echo more-stdout 2>&1"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn");

    let stdout = child.stdout.take().unwrap();
    for line in BufReader::new(stdout).lines() {
        let line = line.expect("read error");
        println!("[merged] {}", line);
    }

    let status = child.wait().expect("wait failed");
    println!("Exit: {}", status);
}
```

## Suppressing Output with Stdio::null()

When you don't care about a stream, redirect it to null to avoid buffering overhead:

```rust
use std::process::{Command, Stdio};

fn main() {
    let status = Command::new("cargo")
        .arg("check")
        .stdout(Stdio::null()) // Suppress normal output
        .stderr(Stdio::null()) // Suppress error output too
        .status()
        .expect("failed to run cargo");

    // We only care about success/failure, not the output
    if status.success() {
        println!("Code compiles!");
    } else {
        println!("Compilation failed");
    }
}
```

This is equivalent to `> /dev/null 2>&1` in shell. Use it when you only need the exit code.

::: info In the Wild
Production agents typically capture both stdout and stderr separately, tag each line with its source stream, and combine them when reporting to the LLM. Claude Code, for example, captures stderr to detect compilation warnings and errors even when the command exits successfully. This dual-stream capture lets the agent provide richer context to the LLM about what happened during execution.
:::

## Key Takeaways

- Every process has three standard streams: stdin (fd 0), stdout (fd 1), and stderr (fd 2). Rust's `Stdio` type configures each independently as `piped()`, `inherit()`, or `null()`.
- Always close (drop) the stdin handle after writing to signal EOF; failing to do so causes the child to wait forever for more input.
- Read stdout and stderr concurrently using separate tasks to avoid pipe buffer deadlocks -- this is the single most common process communication bug.
- Exact interleaving order between stdout and stderr is lost when they are read from separate pipes; merge at the shell level with `2>&1` if ordering matters.
- Use `Stdio::null()` to suppress output you do not need, reducing memory usage and avoiding unnecessary buffering.
