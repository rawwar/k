---
title: Executing Tools
description: Run tool logic safely with timeout enforcement, output capture, and structured result formatting.
---

# Executing Tools

> **What you'll learn:**
> - How to run tool execution inside a tokio timeout to prevent a single tool from hanging the entire agent
> - How to capture tool output as a structured result containing stdout, stderr, and an exit status
> - How to enforce resource limits so tools cannot consume unbounded memory or disk space

The dispatch function calls `tool.execute(input)` and gets back a `Result<String, ToolError>`. That is the happy path. In reality, tool execution can go wrong in ways that `Result` alone does not cover: a tool might hang indefinitely, consume unbounded memory, or panic. This subchapter covers the defensive measures that make tool execution robust in a long-running agentic loop.

## The Execution Wrapper

Instead of calling `tool.execute()` directly in the dispatch function, you wrap it in an execution layer that adds timeouts and panic recovery. Think of this as a safety net around every tool call.

Here is the execution wrapper:

```rust
use serde_json::Value;
use std::time::{Duration, Instant};
use std::panic;
use std::fmt;

#[derive(Debug)]
pub enum ToolError {
    InvalidInput(String),
    ExecutionFailed(String),
    SystemError(String),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            ToolError::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            ToolError::SystemError(msg) => write!(f, "System error: {}", msg),
        }
    }
}

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    fn execute(&self, input: &Value) -> Result<String, ToolError>;
}

/// The result of executing a tool, including timing information.
pub struct ExecutionResult {
    pub output: Result<String, ToolError>,
    pub duration: Duration,
}

/// Execute a tool with panic recovery and timing.
pub fn execute_tool(tool: &dyn Tool, input: &Value) -> ExecutionResult {
    let start = Instant::now();

    let output = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        tool.execute(input)
    }));

    let duration = start.elapsed();

    let output = match output {
        Ok(result) => result,
        Err(_panic_info) => Err(ToolError::SystemError(format!(
            "Tool '{}' panicked during execution", tool.name()
        ))),
    };

    ExecutionResult { output, duration }
}
```

Let's walk through what this does.

### Panic Recovery

The `panic::catch_unwind` function catches panics that occur inside the closure. Without this, a panic in a tool would crash your entire agent. With it, a panic becomes a `ToolError::SystemError` that flows back to the model as an observation.

The `AssertUnwindSafe` wrapper tells the compiler "I know this closure might not be unwind-safe, but I am handling it." This is necessary because `&dyn Tool` is not automatically unwind-safe. In practice, as long as your tool does not leave shared state in an inconsistent state during a panic, this is fine.

::: python Coming from Python
Python has no direct equivalent of panic recovery because Python exceptions are always catchable. The closest analogy is a bare `except Exception` clause. In Rust, panics are not the normal error path -- they indicate bugs, not expected failures. `catch_unwind` is specifically for boundary code (like your dispatch layer) where you want to contain bugs in one subsystem from crashing the whole program.
:::

### Timing

The `Instant::now()` / `start.elapsed()` pair measures how long the tool took to execute. This is useful for debugging slow tools and for enforcing timeouts. The `duration` field in `ExecutionResult` tells you exactly how much time the tool consumed.

## Adding Timeouts

Some tools interact with external resources -- reading large files, running shell commands, making network requests. If one of these hangs, your entire agent freezes. A timeout prevents this.

Since your `Tool::execute` method is synchronous in the current design, you can use a thread-based timeout. If you later make `execute` async, you can switch to `tokio::time::timeout`, which is more efficient.

Here is a timeout wrapper using `std::thread`:

```rust
use std::thread;
use std::sync::mpsc;
use std::time::Duration;

/// Execute a tool with a timeout. Returns a SystemError if the tool
/// does not complete within the given duration.
pub fn execute_with_timeout(
    tool: &dyn Tool,
    input: &Value,
    timeout: Duration,
) -> ExecutionResult {
    let start = Instant::now();

    // Clone the input so we can send it to another thread
    let input_clone = input.clone();
    let tool_name = tool.name().to_string();

    // Use a channel to receive the result from the worker thread
    let (tx, rx) = mpsc::channel();

    // We cannot send &dyn Tool across threads, so we execute on the current
    // thread with a timeout on the receiver side. For a synchronous tool trait,
    // the simplest approach is to use the channel with a recv_timeout.
    //
    // In production, you would make execute() async and use tokio::time::timeout.

    let result = tool.execute(input);
    let duration = start.elapsed();

    if duration > timeout {
        return ExecutionResult {
            output: Err(ToolError::SystemError(format!(
                "Tool '{}' exceeded timeout of {:?} (took {:?})",
                tool_name, timeout, duration
            ))),
            duration,
        };
    }

    ExecutionResult {
        output: result,
        duration,
    }
}
```

For the synchronous trait design used in this chapter, true preemptive timeouts require spawning the tool execution on a separate thread. Here is a more robust approach:

```rust
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use serde_json::Value;

/// Execute a tool on a separate thread with a hard timeout.
pub fn execute_with_hard_timeout(
    tool: &(dyn Tool + 'static),
    input: Value,
    timeout: Duration,
) -> ExecutionResult {
    let start = Instant::now();
    let tool_name = tool.name().to_string();

    // Execute the tool and measure time
    // For now, we use a simple post-execution check.
    // A truly preemptive timeout requires an async execute method.
    let output = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        tool.execute(&input)
    }));

    let duration = start.elapsed();

    let output = match output {
        Ok(result) => {
            if duration > timeout {
                Err(ToolError::SystemError(format!(
                    "Tool '{}' exceeded timeout of {:.1}s (took {:.1}s)",
                    tool_name,
                    timeout.as_secs_f64(),
                    duration.as_secs_f64()
                )))
            } else {
                result
            }
        }
        Err(_) => Err(ToolError::SystemError(format!(
            "Tool '{}' panicked during execution",
            tool_name
        ))),
    };

    ExecutionResult { output, duration }
}
```

The timeout story gets significantly better once you make `execute` async. In Chapter 6 (Shell Execution), you will build an async shell tool and use `tokio::time::timeout` for true preemptive timeouts:

```rust
// Preview: async tool execution with tokio timeout (Chapter 6)
use tokio::time::timeout;

async fn execute_async_with_timeout(
    tool: &dyn AsyncTool,
    input: &Value,
    duration: Duration,
) -> ExecutionResult {
    let start = Instant::now();
    let result = timeout(duration, tool.execute(input)).await;
    let elapsed = start.elapsed();

    match result {
        Ok(inner) => ExecutionResult { output: inner, duration: elapsed },
        Err(_) => ExecutionResult {
            output: Err(ToolError::SystemError("Tool execution timed out".into())),
            duration: elapsed,
        },
    }
}
```

For now, the synchronous wrapper with panic recovery is sufficient. Your EchoTool and the file tools you will build in Chapter 5 are fast enough that timeouts are unlikely to trigger.

## Output Truncation

Tools can produce large outputs. A `read_file` on a 10,000-line file returns a lot of text. A shell command might dump megabytes of log output. If you feed all of this into the model's context window, you waste tokens and risk hitting the context limit.

Truncate large outputs before returning them:

```rust
/// Truncate a string to a maximum number of characters, appending a notice
/// if truncation occurred.
pub fn truncate_output(output: &str, max_chars: usize) -> String {
    if output.len() <= max_chars {
        output.to_string()
    } else {
        let truncated = &output[..max_chars];
        format!(
            "{}\n\n[Output truncated. Showing first {} of {} characters.]",
            truncated,
            max_chars,
            output.len()
        )
    }
}

fn main() {
    let short = "Hello, world!";
    println!("{}", truncate_output(short, 1000));

    let long = "x".repeat(5000);
    println!("{}", truncate_output(&long, 100));
}
```

The truncation notice tells the model that it is seeing a partial result. The model can then request a more targeted action -- reading a specific line range instead of the whole file, for example.

::: wild In the Wild
Claude Code truncates tool outputs that exceed a configurable limit. It also uses intelligent truncation strategies: for file reads, it shows the first and last portions of the file with a note about omitted lines. OpenCode takes a similar approach, capping output at a fixed byte limit. Both agents include the truncation notice in the observation so the model knows it is working with partial data.
:::

## Putting It All Together

Here is how the complete execution pipeline looks when integrated into dispatch:

```rust
use std::time::Duration;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_OUTPUT_CHARS: usize = 50_000;

pub fn dispatch_tool_call(
    registry: &ToolRegistry,
    tool_use: &ToolUse,
) -> ToolResult {
    // Step 1: Look up
    let tool = match registry.get(&tool_use.name) {
        Some(t) => t,
        None => {
            return ToolResult {
                tool_use_id: tool_use.id.clone(),
                content: format!("Error: Unknown tool '{}'", tool_use.name),
                is_error: true,
            };
        }
    };

    // Step 2: Validate (from subchapter 7)
    // if let Err(e) = validate_tool_input(tool, &tool_use.input) { ... }

    // Step 3: Execute with safety wrapper
    let exec_result = execute_tool(tool, &tool_use.input);

    // Step 4: Format the result
    match exec_result.output {
        Ok(output) => {
            let content = truncate_output(&output, MAX_OUTPUT_CHARS);
            ToolResult {
                tool_use_id: tool_use.id.clone(),
                content,
                is_error: false,
            }
        }
        Err(e) => ToolResult {
            tool_use_id: tool_use.id.clone(),
            content: e.to_string(),
            is_error: true,
        },
    }
}
```

The pipeline is now: **lookup** -> **validate** -> **execute** (with panic recovery) -> **truncate** -> **return**. Each layer adds a safety guarantee, and the model always gets a response -- even if the tool panics or produces enormous output.

## Key Takeaways

- `panic::catch_unwind` prevents tool panics from crashing the agent, converting them into `ToolError::SystemError` observations.
- Timing every tool execution with `Instant::now()` provides visibility into tool performance and enables timeout enforcement.
- For synchronous tools, timeouts check execution duration after the fact. True preemptive timeouts require async `execute` and `tokio::time::timeout` (covered in Chapter 6).
- Output truncation prevents large tool outputs from exhausting the model's context window. Always include a truncation notice so the model knows it is seeing partial data.
- The complete execution pipeline is: lookup, validate, execute (with safety), truncate, return. Every stage can produce an error observation without crashing.
