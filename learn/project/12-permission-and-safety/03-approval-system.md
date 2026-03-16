---
title: Approval System
description: Building an interactive approval workflow that pauses agent execution for human review before dangerous operations, with support for accept-once and accept-always patterns.
---

# Approval System

> **What you'll learn:**
> - How to implement an approval checkpoint that halts the agent loop pending human input
> - How to support accept-once, accept-always, and deny responses for granular control
> - Patterns for presenting operation details clearly so humans can make informed approval decisions

The permission system from the previous subchapter decides whether an operation needs approval. Now you need the actual mechanism that pauses the agent, presents the operation to the user, collects their decision, and resumes or aborts accordingly.

A good approval system balances safety with usability. If every operation triggers a prompt, users will either switch to FullAuto mode (defeating the purpose) or start rubber-stamping approvals without reading them (equally dangerous). The goal is to ask only when it matters and to make the prompt informative enough that users can make a genuine decision.

## The Approval Request

First, let's define what information the approval prompt needs to convey. The user must understand *what* the agent wants to do, *why* it matters, and *what the consequences* might be:

```rust
use std::fmt;

/// A request for user approval before executing an operation.
#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    /// Human-readable description of the operation.
    pub operation: String,
    /// The tool being invoked.
    pub tool_name: String,
    /// Key parameters the user should review.
    pub parameters: Vec<(String, String)>,
    /// Why this operation was flagged for approval.
    pub reason: String,
    /// Risk level hint for UI formatting.
    pub risk: RiskLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    Moderate,
    High,
    Critical,
}

impl fmt::Display for ApprovalRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let risk_indicator = match self.risk {
            RiskLevel::Moderate => "[MODERATE]",
            RiskLevel::High => "[HIGH RISK]",
            RiskLevel::Critical => "[CRITICAL]",
        };

        writeln!(f, "\n{} Approval required: {}", risk_indicator, self.operation)?;
        writeln!(f, "  Tool: {}", self.tool_name)?;
        for (key, value) in &self.parameters {
            // Truncate long values for readability
            let display_value = if value.len() > 200 {
                format!("{}... ({} chars total)", &value[..200], value.len())
            } else {
                value.clone()
            };
            writeln!(f, "  {}: {}", key, display_value)?;
        }
        writeln!(f, "  Reason: {}", self.reason)?;
        write!(f, "\n  [y] approve  [n] deny  [a] always approve this tool  [q] abort session")
    }
}
```

## The Approval Response

The user's response is more than a yes/no. Supporting "always approve" for a specific tool dramatically reduces prompt fatigue for repeated safe operations:

```rust
/// User's response to an approval request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalResponse {
    /// Approve this specific operation.
    ApproveOnce,
    /// Approve and remember: do not ask for this tool again during the session.
    ApproveAlways,
    /// Deny this specific operation. The agent should try an alternative.
    Deny,
    /// Abort the entire session. Used when the user loses trust in the agent.
    AbortSession,
}
```

::: python Coming from Python
In Python, you would typically implement approval with a simple `input()` call:
```python
response = input("Approve? [y/n/a/q]: ").strip().lower()
if response == "y":
    proceed()
```
In Rust, reading from stdin is slightly more involved because `std::io::stdin().read_line()` returns a `Result`. But the bigger difference is structural: in an async agent loop, you need the approval system to work with your async runtime rather than blocking the entire thread.
:::

## The Approval Manager

The `ApprovalManager` tracks which tools have been permanently approved and handles the prompt/response cycle. It also records every decision for audit purposes:

```rust
use std::collections::HashSet;
use std::io::{self, BufRead, Write as IoWrite};

/// Records a single approval decision for audit.
#[derive(Debug, Clone)]
pub struct ApprovalRecord {
    pub tool_name: String,
    pub operation: String,
    pub response: ApprovalResponse,
    pub timestamp: std::time::Instant,
}

/// Manages the approval workflow for the agent session.
pub struct ApprovalManager {
    /// Tools that the user has permanently approved for this session.
    always_approved: HashSet<String>,
    /// Full history of approval decisions.
    history: Vec<ApprovalRecord>,
}

impl ApprovalManager {
    pub fn new() -> Self {
        Self {
            always_approved: HashSet::new(),
            history: Vec::new(),
        }
    }

    /// Check if a tool has been permanently approved.
    pub fn is_always_approved(&self, tool_name: &str) -> bool {
        self.always_approved.contains(tool_name)
    }

    /// Request approval from the user. Returns the decision.
    ///
    /// If the tool has been marked "always approve", this returns
    /// `ApproveOnce` immediately without prompting.
    pub fn request_approval(
        &mut self,
        request: &ApprovalRequest,
    ) -> io::Result<ApprovalResponse> {
        // Check if this tool is already approved for the session
        if self.always_approved.contains(&request.tool_name) {
            let record = ApprovalRecord {
                tool_name: request.tool_name.clone(),
                operation: request.operation.clone(),
                response: ApprovalResponse::ApproveOnce,
                timestamp: std::time::Instant::now(),
            };
            self.history.push(record);
            return Ok(ApprovalResponse::ApproveOnce);
        }

        // Display the approval request
        println!("{}", request);

        // Read user input
        let response = Self::read_response()?;

        // If the user chose "always approve", remember it
        if response == ApprovalResponse::ApproveAlways {
            self.always_approved.insert(request.tool_name.clone());
        }

        // Record the decision
        let record = ApprovalRecord {
            tool_name: request.tool_name.clone(),
            operation: request.operation.clone(),
            response: response.clone(),
            timestamp: std::time::Instant::now(),
        };
        self.history.push(record);

        Ok(response)
    }

    fn read_response() -> io::Result<ApprovalResponse> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        loop {
            write!(stdout, "\n> ")?;
            stdout.flush()?;

            let mut input = String::new();
            stdin.lock().read_line(&mut input)?;

            match input.trim().to_lowercase().as_str() {
                "y" | "yes" => return Ok(ApprovalResponse::ApproveOnce),
                "a" | "always" => return Ok(ApprovalResponse::ApproveAlways),
                "n" | "no" => return Ok(ApprovalResponse::Deny),
                "q" | "quit" | "abort" => return Ok(ApprovalResponse::AbortSession),
                _ => {
                    println!("  Please enter: y (approve), n (deny), a (always), q (abort)");
                }
            }
        }
    }

    /// Get all approval records for audit purposes.
    pub fn audit_history(&self) -> &[ApprovalRecord] {
        &self.history
    }

    /// Get the set of tools that have been permanently approved.
    pub fn always_approved_tools(&self) -> &HashSet<String> {
        &self.always_approved
    }

    /// Revoke a previously granted "always approve" permission.
    pub fn revoke_always_approve(&mut self, tool_name: &str) -> bool {
        self.always_approved.remove(tool_name)
    }
}
```

## Building Informative Approval Prompts

The quality of the approval prompt directly affects safety. A vague "approve this operation?" teaches the user nothing. Let's build helper functions that create rich approval requests for common operations:

```rust
impl ApprovalRequest {
    /// Create an approval request for a file write operation.
    pub fn for_file_write(path: &str, content_preview: &str) -> Self {
        let preview = if content_preview.len() > 500 {
            format!("{}...\n({} bytes total)", &content_preview[..500], content_preview.len())
        } else {
            content_preview.to_string()
        };

        Self {
            operation: format!("Write to file: {}", path),
            tool_name: "write_file".to_string(),
            parameters: vec![
                ("path".to_string(), path.to_string()),
                ("content_preview".to_string(), preview),
            ],
            reason: "File modification requires approval in standard mode".to_string(),
            risk: RiskLevel::Moderate,
        }
    }

    /// Create an approval request for a shell command.
    pub fn for_shell_command(command: &str, working_dir: &str) -> Self {
        Self {
            operation: format!("Execute command: {}", command),
            tool_name: "shell".to_string(),
            parameters: vec![
                ("command".to_string(), command.to_string()),
                ("working_directory".to_string(), working_dir.to_string()),
            ],
            reason: "Shell commands may have side effects".to_string(),
            risk: RiskLevel::High,
        }
    }

    /// Create an approval request for a destructive git operation.
    pub fn for_destructive_git(subcommand: &str, details: &str) -> Self {
        Self {
            operation: format!("Destructive git operation: {}", subcommand),
            tool_name: "git".to_string(),
            parameters: vec![
                ("subcommand".to_string(), subcommand.to_string()),
                ("details".to_string(), details.to_string()),
            ],
            reason: "This operation may be irreversible".to_string(),
            risk: RiskLevel::Critical,
        }
    }
}
```

## Integrating Approval into the Agent Loop

Here is how the approval system fits into the agent loop. The key is that a denial does not crash the agent — it returns a message to the LLM explaining that the operation was rejected, so the model can try an alternative approach:

```rust
/// The result of attempting to execute a tool after permission and approval checks.
#[derive(Debug)]
pub enum ToolExecutionResult {
    /// Tool executed successfully with this output.
    Success(String),
    /// Tool was denied by permission check or user approval.
    Denied(String),
    /// User requested session abort.
    SessionAborted,
}

/// Orchestrates permission checking, approval, and tool execution.
pub fn execute_with_safety(
    gate: &PermissionGate,
    approval_mgr: &mut ApprovalManager,
    tool_name: &str,
    subcommand: Option<&str>,
    create_request: impl FnOnce() -> ApprovalRequest,
    execute: impl FnOnce() -> Result<String, String>,
) -> ToolExecutionResult {
    // Step 1: Permission check
    let decision = gate.check(tool_name, subcommand);

    match decision {
        PermissionDecision::Allowed => {
            // Execute directly
            match execute() {
                Ok(output) => ToolExecutionResult::Success(output),
                Err(e) => ToolExecutionResult::Denied(format!("Execution error: {}", e)),
            }
        }
        PermissionDecision::NeedsApproval { reason: _ } => {
            // Build and present the approval request
            let request = create_request();

            match approval_mgr.request_approval(&request) {
                Ok(ApprovalResponse::ApproveOnce | ApprovalResponse::ApproveAlways) => {
                    match execute() {
                        Ok(output) => ToolExecutionResult::Success(output),
                        Err(e) => {
                            ToolExecutionResult::Denied(format!("Execution error: {}", e))
                        }
                    }
                }
                Ok(ApprovalResponse::Deny) => ToolExecutionResult::Denied(
                    "User denied the operation. Try an alternative approach.".to_string(),
                ),
                Ok(ApprovalResponse::AbortSession) => ToolExecutionResult::SessionAborted,
                Err(e) => ToolExecutionResult::Denied(format!("Approval error: {}", e)),
            }
        }
        PermissionDecision::Denied { reason } => {
            ToolExecutionResult::Denied(format!("Permission denied: {}", reason))
        }
    }
}
```

::: wild In the Wild
Claude Code's approval UX shows the full tool call parameters inline in the conversation. When the agent wants to write a file, you see the complete file content with syntax highlighting and a diff view showing what changed. This makes it easy to spot errors before they happen. OpenCode takes a similar approach, showing a side-by-side diff for file modifications. Both agents support a "trust this tool" option that auto-approves subsequent calls to the same tool.
:::

## Handling Approval in Async Contexts

If your agent loop is async (using Tokio), you need the approval system to work without blocking the runtime. The simplest approach is to use `tokio::task::spawn_blocking` for the stdin read:

```rust
use tokio::task;

/// Async wrapper around the approval manager for use in async agent loops.
pub async fn request_approval_async(
    approval_mgr: &mut ApprovalManager,
    request: ApprovalRequest,
) -> Result<ApprovalResponse, String> {
    // Clone what we need for the blocking task
    let tool_name = request.tool_name.clone();
    let is_already_approved = approval_mgr.is_always_approved(&tool_name);

    if is_already_approved {
        return Ok(ApprovalResponse::ApproveOnce);
    }

    // Print the request on the current task (so output ordering is correct)
    println!("{}", request);

    // Read input on a blocking thread to avoid starving the async runtime
    let response = task::spawn_blocking(move || {
        ApprovalManager::read_response()
    })
    .await
    .map_err(|e| format!("Approval task failed: {}", e))?
    .map_err(|e| format!("IO error during approval: {}", e))?;

    Ok(response)
}
```

## Testing the Approval System

You can test the approval logic without actual user input by extracting the decision logic from the IO. Here is a test that verifies "always approve" behavior:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_always_approve_skips_prompt() {
        let mut mgr = ApprovalManager::new();

        // Manually mark a tool as always approved
        mgr.always_approved.insert("write_file".to_string());

        let request = ApprovalRequest::for_file_write(
            "src/main.rs",
            "fn main() { println!(\"hello\"); }",
        );

        // This should return immediately without prompting
        let result = mgr.request_approval(&request).unwrap();
        assert_eq!(result, ApprovalResponse::ApproveOnce);

        // Verify it was recorded
        assert_eq!(mgr.audit_history().len(), 1);
    }

    #[test]
    fn test_revoke_always_approve() {
        let mut mgr = ApprovalManager::new();
        mgr.always_approved.insert("shell".to_string());

        assert!(mgr.is_always_approved("shell"));
        mgr.revoke_always_approve("shell");
        assert!(!mgr.is_always_approved("shell"));
    }
}
```

## Key Takeaways

- The approval system sits between the permission check and tool execution, providing a human checkpoint for operations that need review.
- Supporting four response types — ApproveOnce, ApproveAlways, Deny, and AbortSession — balances safety with usability by reducing prompt fatigue for trusted tools.
- Rich approval prompts that show tool name, parameters, and risk level help users make informed decisions instead of rubber-stamping approvals.
- When the user denies an operation, return a descriptive message to the LLM so it can adjust its strategy rather than crashing the agent loop.
- In async contexts, use `spawn_blocking` for stdin reads to avoid blocking the Tokio runtime.
