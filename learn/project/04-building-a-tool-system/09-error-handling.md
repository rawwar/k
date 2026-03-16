---
title: Error Handling
description: Distinguish between tool execution errors and system-level failures and handle each appropriately.
---

# Error Handling

> **What you'll learn:**
> - The difference between a tool error (the tool ran but failed) and a system error (the tool could not run at all)
> - How to set the `is_error` flag on tool_result content blocks so the model knows something went wrong
> - How to design error messages that give the model enough context to retry or choose a different approach

Error handling in a tool system is different from error handling in a typical application. In a normal program, errors are for the developer -- they read the log, fix the bug, redeploy. In a tool system, errors are for the *model*. The model reads the error observation and decides what to do next: retry with different arguments, try a different tool, or explain to the user what went wrong. This means your error messages need to be informative, actionable, and structured.

## The Three Error Categories

In subchapter 2 you defined `ToolError` with three variants. Let's now examine when each one fires and what the model sees.

### InvalidInput

This error means the model sent arguments that do not match the tool's expectations. The validation layer (subchapter 7) catches most of these, but the tool's `execute` method may catch additional semantic errors that JSON Schema cannot express.

```rust
use serde_json::{json, Value};
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

impl std::error::Error for ToolError {}

// Schema validation catches: missing fields, wrong types
// But the tool catches semantic errors:
fn validate_line_range(input: &Value) -> Result<(), ToolError> {
    if let Some(offset) = input.get("offset").and_then(|v| v.as_i64()) {
        if offset < 1 {
            return Err(ToolError::InvalidInput(
                "offset must be >= 1 (line numbers are 1-based)".to_string()
            ));
        }
    }
    if let Some(limit) = input.get("limit").and_then(|v| v.as_i64()) {
        if limit < 1 {
            return Err(ToolError::InvalidInput(
                "limit must be >= 1".to_string()
            ));
        }
    }
    Ok(())
}
```

JSON Schema can express `"minimum": 1` constraints, but more complex validations -- "offset + limit must not exceed 10,000", "path must not contain `..`" -- need to live in the tool code. When these checks fail, return `InvalidInput` with a message that tells the model exactly what constraint was violated.

### ExecutionFailed

This error means the tool ran but the operation failed. The tool tried to read a file that does not exist. The shell command returned a non-zero exit code. The network request timed out. These are *expected* failures -- they happen in normal usage.

```rust
fn read_file(path: &str) -> Result<String, ToolError> {
    std::fs::read_to_string(path).map_err(|e| {
        ToolError::ExecutionFailed(format!(
            "Cannot read '{}': {}",
            path, e
        ))
    })
}
```

The error message includes both the specific file path and the OS error. This gives the model enough context to try something different: maybe the path was wrong, maybe the file does not exist yet and needs to be created, maybe the user needs to be asked about the correct location.

### SystemError

This error means something unexpected happened in the infrastructure. A panic was caught, a timeout fired, or a resource was exhausted. System errors are not the model's fault and are often not recoverable by the model.

```rust
// Created by the execution wrapper when a panic is caught:
// ToolError::SystemError("Tool 'read_file' panicked during execution")

// Created when a timeout fires:
// ToolError::SystemError("Tool 'run_shell' exceeded timeout of 30.0s")
```

System errors should be logged for the developer and reported to the model. The model cannot fix a panic, but it can tell the user something went wrong rather than silently retrying forever.

## Mapping Errors to the API

The Anthropic API's `tool_result` content block has an `is_error` field. When you set it to `true`, the model knows the tool call failed. All three error categories set `is_error: true`, but they produce different observation text:

```rust
use serde_json::Value;

pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
}

fn error_to_tool_result(tool_use_id: &str, error: &ToolError) -> ToolResult {
    let (content, is_error) = match error {
        ToolError::InvalidInput(msg) => {
            // Tell the model exactly what was wrong with its input
            (format!("Invalid input: {}\nPlease check the tool's schema and retry.", msg), true)
        }
        ToolError::ExecutionFailed(msg) => {
            // Tell the model what operation failed and why
            (format!("Tool execution failed: {}", msg), true)
        }
        ToolError::SystemError(msg) => {
            // Tell the model about the infrastructure failure
            (format!("System error: {}. This may be a temporary issue.", msg), true)
        }
    };

    ToolResult {
        tool_use_id: tool_use_id.to_string(),
        content,
        is_error,
    }
}
```

Notice the suffix "Please check the tool's schema and retry" on `InvalidInput`. This gentle nudge helps the model understand that it should look at the schema again and correct its arguments. For `SystemError`, the phrase "This may be a temporary issue" suggests the model might succeed if it retries (which is sometimes true for timeouts or resource contention).

::: python Coming from Python
In Python, you might map exceptions to error responses:

```python
try:
    result = tool.execute(input)
    return {"content": result, "is_error": False}
except ValueError as e:
    return {"content": f"Invalid input: {e}", "is_error": True}
except FileNotFoundError as e:
    return {"content": f"Execution failed: {e}", "is_error": True}
except Exception as e:
    return {"content": f"System error: {e}", "is_error": True}
```

The Rust approach with `ToolError` variants is more explicit -- you cannot accidentally catch the wrong exception type because `match` requires you to handle every variant. Python's exception hierarchy is flexible but easy to misuse (catching `Exception` swallows everything, including bugs).
:::

## Writing Good Error Messages

The quality of your error messages directly affects the model's ability to recover. Here are guidelines:

**Include the specific value that failed.** Not "invalid path" but "Cannot read 'src/main.rx': No such file or directory." The model can see the typo (`main.rx` instead of `main.rs`) and correct it.

**Suggest what to do next.** Not "permission denied" but "Permission denied for '/etc/shadow'. This file requires root access. Try a different file or ask the user for help."

**Be precise about constraints.** Not "invalid offset" but "offset must be >= 1 (line numbers are 1-based), but got 0."

**Include available alternatives when possible.** Not "unknown tool" but "Unknown tool 'read'. Available tools: read_file, write_file, echo."

Here is a helper that applies these principles to file-related errors:

```rust
use std::io;

fn file_error_message(path: &str, error: &io::Error) -> String {
    match error.kind() {
        io::ErrorKind::NotFound => {
            format!(
                "File '{}' not found. Check that the path is correct \
                 and the file exists.",
                path
            )
        }
        io::ErrorKind::PermissionDenied => {
            format!(
                "Permission denied for '{}'. The agent does not have \
                 read access to this file.",
                path
            )
        }
        io::ErrorKind::IsADirectory => {
            format!(
                "'{}' is a directory, not a file. Use a file path instead.",
                path
            )
        }
        _ => {
            format!("Cannot access '{}': {}", path, error)
        }
    }
}

fn main() {
    // Simulate a file not found error
    let err = io::Error::new(io::ErrorKind::NotFound, "file not found");
    println!("{}", file_error_message("src/main.rx", &err));

    // Simulate a permission error
    let err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
    println!("{}", file_error_message("/etc/shadow", &err));
}
```

Output:

```
File 'src/main.rx' not found. Check that the path is correct and the file exists.
Permission denied for '/etc/shadow'. The agent does not have read access to this file.
```

These messages are clear enough for the model to take corrective action.

## Error Recovery Patterns

The model has several strategies when it receives an error observation:

1. **Retry with corrected arguments.** The model fixes the mistake (typo in path, wrong parameter type) and calls the tool again.
2. **Try a different approach.** If reading a file fails, the model might search for the correct file name first.
3. **Ask the user.** If the model cannot recover, it produces a text response explaining what went wrong and asking for guidance.
4. **Give up.** If repeated retries fail, the model stops calling tools and reports the issue.

Your tool system does not control which strategy the model chooses -- that is the model's reasoning capability. But the quality of your error messages influences the choice. Vague errors lead to blind retries. Specific errors lead to targeted corrections.

::: wild In the Wild
Claude Code includes the full error context in tool result observations, including stack traces for unexpected errors. It also includes a structured "retry hint" in some error messages. OpenCode takes a similar approach, formatting errors with the failed command, the error output, and suggestions for common fixes. Both agents have found that verbose error messages improve model self-correction rates.
:::

## A Complete Error Handling Example

Here is a complete example showing how different error types flow through the system:

```rust
use serde_json::{json, Value};
use std::fmt;

#[derive(Debug)]
enum ToolError {
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

fn execute_read_file(input: &Value) -> Result<String, ToolError> {
    let path = input.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::InvalidInput(
            "Missing required field 'path' (string)".to_string()
        ))?;

    // Semantic validation
    if path.contains("..") {
        return Err(ToolError::InvalidInput(
            format!("Path '{}' contains '..', which is not allowed for security.", path)
        ));
    }

    // Actual file read
    std::fs::read_to_string(path).map_err(|e| {
        ToolError::ExecutionFailed(format!("Cannot read '{}': {}", path, e))
    })
}

fn main() {
    // Test 1: Valid call
    let input = json!({"path": "Cargo.toml"});
    match execute_read_file(&input) {
        Ok(content) => println!("Success: {} bytes", content.len()),
        Err(e) => println!("Error: {}", e),
    }

    // Test 2: Missing required field
    let input = json!({});
    match execute_read_file(&input) {
        Ok(_) => println!("Unexpected success"),
        Err(e) => println!("Error: {}", e),
    }

    // Test 3: Path traversal attempt
    let input = json!({"path": "../../etc/passwd"});
    match execute_read_file(&input) {
        Ok(_) => println!("Unexpected success"),
        Err(e) => println!("Error: {}", e),
    }

    // Test 4: File does not exist
    let input = json!({"path": "nonexistent.txt"});
    match execute_read_file(&input) {
        Ok(_) => println!("Unexpected success"),
        Err(e) => println!("Error: {}", e),
    }
}
```

Output:

```
Success: 189 bytes
Error: Invalid input: Missing required field 'path' (string)
Error: Invalid input: Path '../../etc/passwd' contains '..', which is not allowed for security.
Error: Execution failed: Cannot read 'nonexistent.txt': No such file or directory (os error 2)
```

Each error message is specific, includes the problematic value, and suggests what went wrong. The model can read any of these and take appropriate action.

## Key Takeaways

- `InvalidInput` is for argument errors (model's fault), `ExecutionFailed` is for operation failures (expected), and `SystemError` is for infrastructure problems (unexpected).
- All three error types set `is_error: true` on the `tool_result`, but with different observation text that guides the model's recovery strategy.
- Good error messages include the specific failing value, the constraint that was violated, and a suggestion for what to do next.
- The model uses error observations to decide between retrying, trying a different approach, asking the user, or giving up. Specific errors lead to better recovery.
- Map OS-level errors (like `io::ErrorKind`) to tool-specific messages that are meaningful in the context of the agent's task.
