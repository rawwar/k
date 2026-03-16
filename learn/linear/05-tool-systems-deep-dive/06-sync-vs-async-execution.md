---
title: Sync vs Async Execution
description: Choosing between synchronous and asynchronous tool execution based on latency requirements, concurrency needs, and tool behavior.
---

# Sync vs Async Execution

> **What you'll learn:**
> - When synchronous execution is appropriate (fast tools, sequential dependencies) versus asynchronous (I/O-bound, parallel-safe)
> - How to implement async tool execution in Rust using tokio tasks and channels
> - Patterns for handling parallel tool calls where the model requests multiple tools simultaneously

The previous subchapter explored *where* tools execute. This one explores *how* they execute from a concurrency perspective. The distinction between synchronous and asynchronous execution affects your agent's throughput, responsiveness, and architecture.

## Synchronous Execution

In synchronous execution, the agent calls a tool and waits for it to finish before doing anything else. The current thread blocks until the tool returns its result.

```rust
use std::fs;

pub fn read_file_sync(path: &str) -> Result<String, String> {
    fs::read_to_string(path)
        .map_err(|e| format!("Failed to read '{}': {}", path, e))
}

// In the agent loop:
fn handle_tool_call(name: &str, input: &serde_json::Value) -> String {
    match name {
        "read_file" => {
            let path = input["path"].as_str().unwrap();
            match read_file_sync(path) {
                Ok(content) => content,
                Err(e) => format!("Error: {}", e),
            }
        }
        _ => format!("Unknown tool: {}", name),
    }
}
```

Synchronous execution is straightforward: call the function, get the result, move on. There is no concurrency to reason about, no futures to manage, and no task scheduling overhead.

**When sync is the right choice:**
- The tool completes in milliseconds (reading a small file, checking if a path exists)
- The tool has side effects that must complete before the next step (writing a file that will be read immediately after)
- You are building a simple prototype and want minimal complexity

**When sync is the wrong choice:**
- The tool takes seconds to complete (running tests, compiling code, searching a large codebase)
- The model requests multiple tool calls in parallel
- The agent needs to remain responsive (showing a spinner, handling user cancellation)

## Asynchronous Execution

Asynchronous execution lets the agent start a tool and continue doing other work while waiting for the result. In Rust, this means using `async`/`await` with tokio:

```rust
use tokio::fs;

pub async fn read_file_async(path: &str) -> Result<String, String> {
    fs::read_to_string(path)
        .await
        .map_err(|e| format!("Failed to read '{}': {}", path, e))
}

// In the agent loop:
async fn handle_tool_call(name: &str, input: &serde_json::Value) -> String {
    match name {
        "read_file" => {
            let path = input["path"].as_str().unwrap();
            match read_file_async(path).await {
                Ok(content) => content,
                Err(e) => format!("Error: {}", e),
            }
        }
        _ => format!("Unknown tool: {}", name),
    }
}
```

At first glance, this looks almost identical to the sync version -- just with `async` and `.await` sprinkled in. The difference becomes apparent when you handle multiple tool calls.

::: python Coming from Python
If you have used `asyncio` in Python, Rust's async model will feel somewhat familiar:
```python
import asyncio

async def read_file_async(path: str) -> str:
    # In Python, you'd use aiofiles for async file I/O
    import aiofiles
    async with aiofiles.open(path) as f:
        return await f.read()
```
The key difference is that Python's async is cooperative (you must `await` or the task never yields), and Rust's is also cooperative but enforced at compile time. You cannot accidentally forget to `await` a future in Rust -- the compiler complains because an un-awaited future is an unused value of type `impl Future`.
:::

## Parallel Tool Calls

Modern LLM APIs support parallel tool calls, where the model requests multiple tools in a single response. For example, the model might want to read three files simultaneously:

```json
[
  {"type": "tool_use", "name": "read_file", "input": {"path": "/project/src/main.rs"}},
  {"type": "tool_use", "name": "read_file", "input": {"path": "/project/Cargo.toml"}},
  {"type": "tool_use", "name": "read_file", "input": {"path": "/project/src/lib.rs"}}
]
```

With synchronous execution, you process these one at a time:

```rust
// Sequential: ~300ms if each read takes ~100ms
let mut results = Vec::new();
for tool_call in &tool_calls {
    let result = handle_tool_call(&tool_call.name, &tool_call.input);
    results.push(result);
}
```

With asynchronous execution, you can process them all concurrently:

```rust
use futures::future::join_all;

// Parallel: ~100ms if each read takes ~100ms
let futures: Vec<_> = tool_calls
    .iter()
    .map(|tc| handle_tool_call(&tc.name, &tc.input))
    .collect();

let results = join_all(futures).await;
```

The `join_all` function runs all futures concurrently. If each file read takes 100ms, three sequential reads take 300ms while three parallel reads take about 100ms. For I/O-bound tools (file reads, network requests, subprocess execution), this parallelism is significant.

## The Hybrid Approach

In practice, you will want a hybrid approach: an async runtime that can handle both sync and async tools. The pattern is to run everything in an async context and use `tokio::task::spawn_blocking` for tools that do synchronous, CPU-bound work:

```rust
use tokio::task;

pub async fn execute_tool(
    name: &str,
    input: serde_json::Value,
) -> Result<String, String> {
    match name {
        // Async tool: file reading with tokio::fs
        "read_file" => {
            let path = input["path"].as_str().unwrap().to_string();
            tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| format!("Failed to read '{}': {}", path, e))
        }

        // Sync tool wrapped in spawn_blocking: CPU-intensive search
        "search_files" => {
            let pattern = input["pattern"].as_str().unwrap().to_string();
            let dir = input["directory"].as_str().unwrap().to_string();

            task::spawn_blocking(move || {
                search_files_sync(&pattern, &dir)
            })
            .await
            .map_err(|e| format!("Task panicked: {}", e))?
        }

        // Async subprocess: shell command
        "shell" => {
            let command = input["command"].as_str().unwrap().to_string();
            execute_shell_async(&command).await
        }

        _ => Err(format!("Unknown tool: {}", name)),
    }
}

fn search_files_sync(pattern: &str, dir: &str) -> Result<String, String> {
    // CPU-bound grep-like operation
    // This runs on a blocking thread pool, not the async executor
    todo!("Implement grep-like search")
}

async fn execute_shell_async(command: &str) -> Result<String, String> {
    let output = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .await
        .map_err(|e| format!("Failed to execute: {}", e))?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
```

The key insight here is `spawn_blocking`. Tokio's async runtime uses a small number of threads to drive many concurrent tasks. If a task blocks one of these threads (by doing synchronous I/O or heavy computation), it starves other tasks. `spawn_blocking` moves the blocking work onto a separate thread pool, keeping the async runtime responsive.

## Timeouts and Cancellation

Async execution also gives you clean timeout and cancellation semantics:

```rust
use tokio::time::{timeout, Duration};

pub async fn execute_with_timeout(
    name: &str,
    input: serde_json::Value,
    timeout_ms: u64,
) -> Result<String, String> {
    let duration = Duration::from_millis(timeout_ms);

    match timeout(duration, execute_tool(name, input)).await {
        Ok(result) => result,
        Err(_) => Err(format!(
            "Tool '{}' timed out after {}ms. \
             The operation took too long and was cancelled.",
            name, timeout_ms
        )),
    }
}
```

This is particularly important for shell command execution. A model might accidentally run an infinite loop, or a compilation might take minutes on a large project. Timeouts ensure the agent does not hang indefinitely.

::: wild In the Wild
Claude Code uses an async architecture where each tool call runs as an async operation with configurable timeouts. Shell commands have a default timeout (often around 120 seconds) that prevents runaway processes. OpenCode uses Go's goroutines and context-based cancellation for a similar effect. Both agents support parallel tool execution -- when the model requests multiple reads in a single response, they run concurrently rather than sequentially.
:::

## Ordering and Dependencies

One subtlety with parallel tool calls is dependencies. If the model requests:

1. Write a file to `/project/src/new.rs`
2. Read the file at `/project/src/new.rs`

These have a dependency -- the read must happen after the write. In practice, most LLM APIs handle this by sending dependent tool calls in separate response turns. Parallel tool calls within a single turn are intended to be independent.

However, you should still be defensive. If you detect that two parallel tool calls have a dependency (like writing and reading the same file), serialize them:

```rust
pub async fn execute_tool_calls(
    calls: Vec<ToolCall>,
) -> Vec<ToolResult> {
    // Group by affected file path to detect conflicts
    let mut write_paths: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut sequential = Vec::new();
    let mut parallel = Vec::new();

    for call in calls {
        let path = call.input.get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let is_mutating = matches!(call.name.as_str(), "write_file" | "edit_file");

        if is_mutating {
            write_paths.insert(path.to_string());
            sequential.push(call);
        } else if write_paths.contains(path) {
            // This read depends on a write to the same path
            sequential.push(call);
        } else {
            parallel.push(call);
        }
    }

    // Execute parallel calls concurrently
    let parallel_futures: Vec<_> = parallel
        .into_iter()
        .map(|tc| execute_tool(&tc.name, tc.input))
        .collect();
    let mut results: Vec<ToolResult> = join_all(parallel_futures)
        .await
        .into_iter()
        .map(|r| ToolResult::from(r))
        .collect();

    // Execute sequential calls in order
    for call in sequential {
        let result = execute_tool(&call.name, call.input).await;
        results.push(ToolResult::from(result));
    }

    results
}
```

## Key Takeaways

- Synchronous execution is simplest and fine for fast, independent tools -- async adds complexity that you should justify with concrete benefits
- Async execution enables parallel tool calls, which significantly reduces latency when the model requests multiple tools at once
- Use `tokio::task::spawn_blocking` for CPU-bound or synchronously blocking tools within an async runtime
- Always implement timeouts on tool execution, especially for shell commands and external processes
- Be aware of dependency ordering in parallel tool calls -- serialize mutations and their dependent reads
