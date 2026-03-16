---
title: Parallel Execution
description: Running multiple child processes concurrently, managing process groups, collecting results, and handling partial failures in parallel tool invocations.
---

# Parallel Execution

> **What you'll learn:**
> - How to spawn and manage multiple child processes concurrently using tokio::JoinSet
> - Strategies for collecting and aggregating results from parallel process executions
> - Handling partial failures when some processes succeed and others fail in a concurrent batch

A coding agent often needs to run multiple commands at the same time. The LLM might request running a linter and a formatter simultaneously. Your agent might want to check types and run tests in parallel to give faster feedback. Or the agent might need to execute several file searches concurrently. Running these sequentially wastes time -- if each takes 5 seconds, running three sequentially takes 15 seconds, but running them in parallel takes roughly 5. This subchapter covers the patterns for parallel process execution in an async Rust agent.

## Spawning Multiple Processes with JoinSet

Tokio's `JoinSet` is the primary tool for managing a dynamic set of concurrent tasks. You spawn tasks into the set and then collect their results as they complete:

```rust
use tokio::process::Command;
use tokio::task::JoinSet;
use std::process::Stdio;

#[derive(Debug)]
struct CommandResult {
    command: String,
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
}

async fn run_command(program: &str, args: &[&str]) -> CommandResult {
    let cmd_str = format!("{} {}", program, args.join(" "));

    let output = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    match output {
        Ok(output) => CommandResult {
            command: cmd_str,
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
        },
        Err(e) => CommandResult {
            command: cmd_str,
            stdout: String::new(),
            stderr: format!("Failed to spawn: {}", e),
            exit_code: None,
        },
    }
}

#[tokio::main]
async fn main() {
    let mut set = JoinSet::new();

    // Spawn multiple commands concurrently
    set.spawn(run_command("echo", &["task one"]));
    set.spawn(run_command("echo", &["task two"]));
    set.spawn(run_command("echo", &["task three"]));

    // Collect results as they complete (order is not guaranteed)
    while let Some(result) = set.join_next().await {
        match result {
            Ok(cmd_result) => {
                println!(
                    "[{}] exit={:?} stdout={}",
                    cmd_result.command,
                    cmd_result.exit_code,
                    cmd_result.stdout.trim()
                );
            }
            Err(e) => eprintln!("Task panicked: {}", e),
        }
    }
}
```

`JoinSet::join_next()` returns results in completion order, not spawn order. This is usually what you want -- process whichever result finishes first.

::: python Coming from Python
Python's `asyncio.gather` or `asyncio.TaskGroup` serves the same purpose:
```python
import asyncio

async def run(cmd):
    proc = await asyncio.create_subprocess_exec(
        *cmd, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE
    )
    stdout, stderr = await proc.communicate()
    return cmd, proc.returncode, stdout.decode()

async def main():
    results = await asyncio.gather(
        run(["echo", "one"]),
        run(["echo", "two"]),
        run(["echo", "three"]),
    )
    for cmd, code, out in results:
        print(f"{cmd}: {out.strip()}")

asyncio.run(main())
```
Rust's `JoinSet` is more flexible than `gather` because you can add tasks dynamically and process results as they complete, rather than waiting for all of them.
:::

## Parallel Execution with Shared Timeout

When running multiple commands in parallel, you often want a shared timeout that applies to the entire batch. If the batch does not complete within the timeout, kill all remaining processes:

```rust
use tokio::process::Command;
use tokio::task::JoinSet;
use tokio::time::{timeout, Duration};
use std::process::Stdio;

#[derive(Debug)]
struct TaskResult {
    name: String,
    output: String,
    success: bool,
}

async fn run_task(name: String, program: &str, args: Vec<String>) -> TaskResult {
    let output = Command::new(program)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    match output {
        Ok(out) => TaskResult {
            name,
            output: String::from_utf8_lossy(&out.stdout).to_string(),
            success: out.status.success(),
        },
        Err(e) => TaskResult {
            name,
            output: format!("Error: {}", e),
            success: false,
        },
    }
}

#[tokio::main]
async fn main() {
    let mut set = JoinSet::new();

    set.spawn(run_task(
        "lint".into(), "echo", vec!["no warnings".into()],
    ));
    set.spawn(run_task(
        "test".into(), "echo", vec!["3 tests passed".into()],
    ));
    set.spawn(run_task(
        "format_check".into(), "echo", vec!["all formatted".into()],
    ));

    let batch_timeout = Duration::from_secs(60);
    let mut results = Vec::new();

    match timeout(batch_timeout, async {
        while let Some(result) = set.join_next().await {
            if let Ok(task_result) = result {
                results.push(task_result);
            }
        }
    })
    .await
    {
        Ok(()) => {
            println!("All tasks completed:");
            for r in &results {
                println!("  {} - success={}: {}", r.name, r.success, r.output.trim());
            }
        }
        Err(_) => {
            // Timeout: abort remaining tasks
            set.abort_all();
            println!("Batch timed out. Completed {} of 3 tasks:", results.len());
            for r in &results {
                println!("  {} - {}", r.name, r.output.trim());
            }
        }
    }
}
```

The `set.abort_all()` call cancels all remaining tasks. For process-backed tasks, the Tokio runtime drops the `Child` handles, which sends SIGKILL to the child processes.

## Handling Partial Failures

In many agent scenarios, some commands succeeding and others failing is a normal outcome -- not an error. You need a strategy for aggregating mixed results:

```rust
use tokio::process::Command;
use tokio::task::JoinSet;
use std::process::Stdio;

#[derive(Debug)]
enum TaskOutcome {
    Success { name: String, output: String },
    Failure { name: String, error: String, exit_code: Option<i32> },
    SpawnError { name: String, error: String },
}

async fn run_checked(name: String, program: String, args: Vec<String>) -> TaskOutcome {
    let output = Command::new(&program)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => TaskOutcome::Success {
            name,
            output: String::from_utf8_lossy(&out.stdout).to_string(),
        },
        Ok(out) => TaskOutcome::Failure {
            name,
            error: String::from_utf8_lossy(&out.stderr).to_string(),
            exit_code: out.status.code(),
        },
        Err(e) => TaskOutcome::SpawnError {
            name,
            error: e.to_string(),
        },
    }
}

#[tokio::main]
async fn main() {
    let mut set = JoinSet::new();

    // Mix of commands -- some will succeed, some may fail
    set.spawn(run_checked("echo_test".into(), "echo".into(), vec!["ok".into()]));
    set.spawn(run_checked("false_test".into(), "false".into(), vec![]));
    set.spawn(run_checked("missing_cmd".into(), "nonexistent_program".into(), vec![]));

    let mut successes = Vec::new();
    let mut failures = Vec::new();

    while let Some(result) = set.join_next().await {
        match result {
            Ok(TaskOutcome::Success { name, output }) => {
                successes.push((name, output));
            }
            Ok(TaskOutcome::Failure { name, error, exit_code }) => {
                failures.push((name, format!("exit {:?}: {}", exit_code, error)));
            }
            Ok(TaskOutcome::SpawnError { name, error }) => {
                failures.push((name, format!("spawn error: {}", error)));
            }
            Err(e) => {
                failures.push(("unknown".into(), format!("task panic: {}", e)));
            }
        }
    }

    println!("Successes ({}):", successes.len());
    for (name, output) in &successes {
        println!("  {} -> {}", name, output.trim());
    }
    println!("Failures ({}):", failures.len());
    for (name, error) in &failures {
        println!("  {} -> {}", name, error.trim());
    }
}
```

This three-variant enum (`Success`, `Failure`, `SpawnError`) gives the agent rich information to report back to the LLM. The LLM can then decide how to proceed -- retry the failed commands, work around the failures, or ask the user for help.

## Controlling Concurrency

Running too many processes simultaneously can overwhelm the system. Use a semaphore to limit the number of concurrent processes:

```rust
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use std::process::Stdio;
use std::sync::Arc;

const MAX_CONCURRENT: usize = 4;

async fn run_limited(
    semaphore: Arc<Semaphore>,
    name: String,
    program: String,
    args: Vec<String>,
) -> (String, bool) {
    // Acquire a permit before spawning the process
    let _permit = semaphore.acquire().await.expect("semaphore closed");

    let output = Command::new(&program)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    match output {
        Ok(out) => (name, out.status.success()),
        Err(_) => (name, false),
    }
    // Permit is dropped here, allowing another task to proceed
}

#[tokio::main]
async fn main() {
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));
    let mut set = JoinSet::new();

    // Spawn 10 tasks, but only 4 will run at a time
    for i in 0..10 {
        let sem = semaphore.clone();
        set.spawn(run_limited(
            sem,
            format!("task-{}", i),
            "echo".into(),
            vec![format!("output-{}", i)],
        ));
    }

    while let Some(result) = set.join_next().await {
        if let Ok((name, success)) = result {
            println!("{}: success={}", name, success);
        }
    }
}
```

The semaphore ensures that no more than `MAX_CONCURRENT` processes run at any time. Each task acquires a permit before spawning its process and automatically releases it when the task completes (when the permit is dropped).

::: wild In the Wild
Production agents limit concurrency to avoid overwhelming the system. Claude Code typically runs one command at a time (serialized), but when parallel tool execution is enabled, it uses a bounded concurrency model to prevent resource exhaustion. The concurrency limit is often configurable, allowing users with more powerful machines to run more commands in parallel.
:::

## Collecting Results in Order

Sometimes you need results in the order they were submitted, not completion order. Collect results into an indexed structure:

```rust
use tokio::process::Command;
use std::process::Stdio;
use std::collections::HashMap;

async fn run_indexed(index: usize, program: &str, args: &[&str]) -> (usize, String, bool) {
    let output = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .output()
        .await;

    match output {
        Ok(out) => (index, String::from_utf8_lossy(&out.stdout).to_string(), out.status.success()),
        Err(e) => (index, format!("Error: {}", e), false),
    }
}

#[tokio::main]
async fn main() {
    let commands = vec![
        ("echo", vec!["first"]),
        ("echo", vec!["second"]),
        ("echo", vec!["third"]),
    ];

    let mut handles = Vec::new();
    for (i, (prog, args)) in commands.iter().enumerate() {
        let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        handles.push(tokio::spawn(run_indexed(i, prog, &args)));
    }

    let mut results: HashMap<usize, (String, bool)> = HashMap::new();
    for handle in handles {
        let (index, output, success) = handle.await.expect("task panicked");
        results.insert(index, (output, success));
    }

    // Print in order
    for i in 0..commands.len() {
        if let Some((output, success)) = results.get(&i) {
            println!("[{}] success={}: {}", i, success, output.trim());
        }
    }
}
```

## Key Takeaways

- Use `tokio::task::JoinSet` to manage a dynamic set of concurrent process executions. Results arrive in completion order, not spawn order.
- Apply a shared timeout to the entire batch of parallel commands with `tokio::time::timeout`, and use `set.abort_all()` to cancel remaining tasks when the timeout expires.
- Model partial failures explicitly with an enum that distinguishes success, process failure, and spawn errors. This gives the LLM rich information to decide how to proceed.
- Limit concurrency with `tokio::sync::Semaphore` to prevent spawning more processes than the system can handle.
- When order matters, index results by their position and reassemble them after all tasks complete.
