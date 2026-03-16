---
title: Error Propagation
description: How tool execution errors are captured, formatted, and propagated back to the LLM so it can reason about failures and retry.
---

# Error Propagation

> **What you'll learn:**
> - The difference between tool errors (reported to the model) and system errors (handled by the agent)
> - How to format error messages that give the model enough context to self-correct without overwhelming it
> - Patterns for retry budgets, error categorization, and escalation to the user when self-correction fails

When a tool fails -- and tools will fail -- the agent faces a critical decision: who should handle this? Should the error go back to the language model so it can try a different approach? Or is this a system-level failure that the agent itself must handle? Getting this distinction right is the foundation of robust error propagation.

## Two Kinds of Errors

Every tool failure falls into one of two categories:

### Tool Errors: The Model's Problem

A tool error means the tool was called with inputs that did not work, but the system is functioning correctly. The model asked for something that cannot be done, and it needs to know so it can adjust.

Examples:
- File not found (the model guessed the wrong path)
- Permission denied (the model tried to write outside the project)
- Compilation error (the model's code has a bug)
- Search returned no results (the model's pattern was wrong)

Tool errors should go back to the model as a tool result with an error flag. The model sees the error, understands what went wrong, and tries a different approach.

### System Errors: The Agent's Problem

A system error means something is broken at the infrastructure level. The tool could not run at all, not because of bad inputs, but because of a system-level failure.

Examples:
- Disk full (cannot write any files)
- Network unreachable (cannot call external services)
- Process spawn failure (OS cannot create child processes)
- Out of memory (the agent process is in trouble)

System errors should generally *not* go back to the model. The model cannot fix a full disk or a network outage. Instead, the agent should handle these errors by retrying (for transient failures), reporting to the user, or shutting down gracefully.

## Representing Errors in Rust

In Rust, you can model this distinction with an enum:

```rust
#[derive(Debug)]
pub enum ToolError {
    /// Error caused by invalid tool inputs or expected failures.
    /// Send this back to the model as a tool result.
    ToolFailure {
        message: String,
        /// Hint for the model on how to fix the issue
        suggestion: Option<String>,
    },

    /// Error caused by system-level failures.
    /// Handle this in the agent, not the model.
    SystemError {
        message: String,
        /// Whether the operation might succeed if retried
        retryable: bool,
    },
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolError::ToolFailure { message, suggestion } => {
                write!(f, "{}", message)?;
                if let Some(hint) = suggestion {
                    write!(f, "\nSuggestion: {}", hint)?;
                }
                Ok(())
            }
            ToolError::SystemError { message, .. } => {
                write!(f, "System error: {}", message)
            }
        }
    }
}
```

Your tool implementations return `Result<String, ToolError>`, and the agent loop handles each variant differently:

```rust
pub async fn process_tool_call(
    name: &str,
    input: serde_json::Value,
) -> ToolCallOutcome {
    match execute_tool(name, input).await {
        Ok(output) => ToolCallOutcome::Success(output),

        Err(ToolError::ToolFailure { message, suggestion }) => {
            // Send back to the model as an error result
            let error_text = match suggestion {
                Some(hint) => format!("{}\n\nSuggestion: {}", message, hint),
                None => message,
            };
            ToolCallOutcome::ToolError(error_text)
        }

        Err(ToolError::SystemError { message, retryable }) => {
            if retryable {
                // Retry once before escalating
                eprintln!("Retrying after system error: {}", message);
                // ... retry logic ...
                ToolCallOutcome::SystemFailure(message)
            } else {
                // Cannot recover -- report to user
                ToolCallOutcome::SystemFailure(message)
            }
        }
    }
}

pub enum ToolCallOutcome {
    Success(String),
    ToolError(String),       // Goes back to the model
    SystemFailure(String),   // Reported to the user
}
```

::: python Coming from Python
In Python, you might model this with exception classes:
```python
class ToolFailure(Exception):
    """Error the model should see and correct."""
    def __init__(self, message: str, suggestion: str | None = None):
        self.message = message
        self.suggestion = suggestion

class SystemError(Exception):
    """Error the agent should handle, not the model."""
    def __init__(self, message: str, retryable: bool = False):
        self.message = message
        self.retryable = retryable
```
The difference is that Python uses exceptions (which propagate up the call stack until caught), while Rust uses `Result` (which must be explicitly handled at each level). Rust's approach makes it impossible to accidentally ignore an error.
:::

## Formatting Errors for the Model

When you send a tool error back to the model, the format of the error message directly affects whether the model can self-correct. Here is a template that works well:

```
[What failed]: The file '/project/src/mian.rs' was not found.
[Why it failed]: No file exists at that path.
[What to do]: Check the file path for typos. Did you mean '/project/src/main.rs'?
Use the list_files tool to see available files.
```

Let's implement this pattern:

```rust
pub fn format_file_not_found(path: &str, project_root: &str) -> ToolError {
    // Try to find similar file names for suggestions
    let suggestions = find_similar_files(path, project_root);

    let suggestion = if suggestions.is_empty() {
        Some("Use the list_files tool to see available files in the project.".to_string())
    } else {
        Some(format!(
            "Did you mean one of these?\n{}",
            suggestions
                .iter()
                .map(|s| format!("  - {}", s))
                .collect::<Vec<_>>()
                .join("\n")
        ))
    };

    ToolError::ToolFailure {
        message: format!("File not found: '{}'", path),
        suggestion,
    }
}

fn find_similar_files(target: &str, root: &str) -> Vec<String> {
    // Simple fuzzy matching: find files with similar names
    let target_name = std::path::Path::new(target)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    let mut matches = Vec::new();

    if let Ok(entries) = std::fs::read_dir(
        std::path::Path::new(target).parent().unwrap_or(std::path::Path::new(root))
    ) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            // Simple Levenshtein-like check: same first character and similar length
            if name_str.len().abs_diff(target_name.len()) <= 2
                && name_str.chars().next() == target_name.chars().next()
            {
                matches.push(entry.path().to_string_lossy().to_string());
            }
        }
    }

    matches
}
```

This approach is more work than returning a bare "File not found" string, but it dramatically improves the model's ability to recover. Instead of blindly guessing a new path, the model gets a list of similar files to choose from.

## Retry Budgets

Sometimes the model will keep retrying a tool call that cannot succeed. Perhaps it is stuck on a wrong path, or it keeps generating invalid regex patterns. You need a retry budget to prevent infinite loops:

```rust
pub struct RetryTracker {
    counts: std::collections::HashMap<String, u32>,
    max_retries: u32,
}

impl RetryTracker {
    pub fn new(max_retries: u32) -> Self {
        Self {
            counts: std::collections::HashMap::new(),
            max_retries,
        }
    }

    /// Returns true if the tool call should be allowed, false if budget exhausted.
    pub fn should_allow(&mut self, tool_name: &str, error_key: &str) -> bool {
        let key = format!("{}:{}", tool_name, error_key);
        let count = self.counts.entry(key).or_insert(0);
        *count += 1;
        *count <= self.max_retries
    }

    /// Reset the tracker (e.g., at the start of a new user turn)
    pub fn reset(&mut self) {
        self.counts.clear();
    }
}
```

In the agent loop, you check the retry budget before sending the error back to the model:

```rust
pub async fn handle_tool_error(
    error: &str,
    tool_name: &str,
    retries: &mut RetryTracker,
) -> ErrorAction {
    // Create a key from the error message (simplified - in practice, normalize this)
    let error_key = &error[..error.len().min(100)];

    if retries.should_allow(tool_name, error_key) {
        // Let the model try again
        ErrorAction::ReturnToModel(error.to_string())
    } else {
        // Budget exhausted - escalate to the user
        ErrorAction::EscalateToUser(format!(
            "The agent has tried '{}' {} times with the same error: {}",
            tool_name,
            retries.max_retries,
            error
        ))
    }
}

pub enum ErrorAction {
    ReturnToModel(String),
    EscalateToUser(String),
}
```

::: wild In the Wild
Claude Code implements a form of retry awareness by tracking tool call patterns. If the model makes the same tool call with the same parameters multiple times, the system recognizes the loop and can intervene. OpenCode includes a maximum iterations limit on the agentic loop itself, which provides a coarser but effective backstop against infinite error-retry loops. Both approaches reflect the same principle: the model's ability to self-correct is valuable but bounded, and the agent must have a fallback when self-correction fails.
:::

## Error Categorization

For better error handling, categorize tool errors by their cause. This helps you route errors to the right handling strategy:

```rust
pub enum ErrorCategory {
    /// Input problem - model should fix the inputs
    InvalidInput,
    /// Resource not found - model should look elsewhere
    NotFound,
    /// Permission denied - model should try a different approach
    PermissionDenied,
    /// External failure - model may want to retry
    ExternalFailure,
    /// Timeout - model should try a smaller operation
    Timeout,
}

impl ErrorCategory {
    pub fn default_suggestion(&self) -> &str {
        match self {
            Self::InvalidInput => "Check the parameters and try again with corrected values.",
            Self::NotFound => "Verify the path or identifier exists using a search or list tool.",
            Self::PermissionDenied => "This operation is not allowed. Try a different approach.",
            Self::ExternalFailure => "This may be a transient failure. You can retry.",
            Self::Timeout => "The operation took too long. Try a smaller scope or simpler command.",
        }
    }
}
```

Categorization also helps with metrics and debugging. You can track how often each error category occurs, which tools fail most, and whether the model is getting better or worse at using tools over time.

## Key Takeaways

- Tool errors (bad inputs, missing files) go back to the model; system errors (disk full, network down) are handled by the agent
- Error messages should state what failed, why it failed, and what to do next -- this three-part structure maximizes model self-correction
- Implement retry budgets to prevent the model from looping on the same failing tool call indefinitely
- Categorize errors (invalid input, not found, permission denied, timeout) to route them to appropriate handling strategies
- Proactive suggestions in error messages -- like offering similar file names when a file is not found -- dramatically improve recovery rates
