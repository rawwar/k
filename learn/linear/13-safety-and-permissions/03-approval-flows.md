---
title: Approval Flows
description: Implement human-in-the-loop approval mechanisms that pause agent execution for confirmation on high-risk operations.
---

# Approval Flows

> **What you'll learn:**
> - How to classify tool invocations by risk level and route dangerous operations through user approval
> - Techniques for presenting clear, actionable approval prompts that help users make informed accept/reject decisions
> - How to implement approval state machines that handle timeouts, batch approvals, and session-level trust escalation

Permission checks tell you whether an operation is structurally allowed. Approval flows add a human judgment layer: even if the permission system says the agent *could* do something, the approval flow asks whether it *should* do it right now. This is the critical last line of defense before an action takes effect on your codebase.

## Risk Classification

Before building the approval mechanism, you need a system for classifying how risky each tool invocation is. Not every file write needs approval -- writing a new test file is lower risk than overwriting `Cargo.toml`. Not every shell command needs approval -- `cargo check` is safe, but `cargo publish` has irreversible consequences.

```rust
use std::path::Path;

/// Risk levels that determine whether an action needs approval.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
enum RiskLevel {
    /// No approval needed -- operation is inherently safe
    Safe,
    /// First-time approval, then auto-approved for the session
    SessionApprove,
    /// Always requires explicit approval
    AlwaysApprove,
    /// Blocked entirely -- no amount of approval can enable this
    Blocked,
}

/// Classify a file write operation by risk level.
fn classify_file_write(path: &str) -> RiskLevel {
    let path = Path::new(path);
    let filename = path.file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
    let extension = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    // Critical system/config files -- always require approval
    let always_approve_files = [
        "Cargo.toml", "Cargo.lock", ".gitignore", "Dockerfile",
        ".github", "Makefile", "build.rs",
    ];
    if always_approve_files.iter().any(|f| filename == *f) {
        return RiskLevel::AlwaysApprove;
    }

    // Sensitive files -- block entirely
    let blocked_patterns = [".env", "credentials", "secrets", ".ssh", ".aws"];
    let path_str = path.to_string_lossy().to_lowercase();
    if blocked_patterns.iter().any(|p| path_str.contains(p)) {
        return RiskLevel::Blocked;
    }

    // Source code files -- approve once per session
    if ["rs", "toml", "yaml", "json", "ts", "py"].contains(&extension) {
        return RiskLevel::SessionApprove;
    }

    // Test files -- generally safe
    if path_str.contains("test") || path_str.contains("spec") {
        return RiskLevel::Safe;
    }

    // Unknown file types -- require approval
    RiskLevel::AlwaysApprove
}

/// Classify a shell command by risk level.
fn classify_shell_command(command: &str) -> RiskLevel {
    let cmd_lower = command.to_lowercase();

    // Safe commands that never need approval
    let safe_commands = [
        "cargo check", "cargo test", "cargo clippy", "cargo fmt",
        "git status", "git diff", "git log", "ls", "cat", "head",
        "wc", "find", "grep",
    ];
    if safe_commands.iter().any(|c| cmd_lower.starts_with(c)) {
        return RiskLevel::Safe;
    }

    // Dangerous commands that are always blocked
    let blocked_commands = [
        "rm -rf /", "sudo", "chmod 777", "> /dev/",
        "mkfs", "dd if=", ":(){ :|:",
    ];
    if blocked_commands.iter().any(|c| cmd_lower.contains(c)) {
        return RiskLevel::Blocked;
    }

    // Irreversible commands -- always need approval
    let always_approve = [
        "cargo publish", "git push", "docker push",
        "npm publish", "rm -rf",
    ];
    if always_approve.iter().any(|c| cmd_lower.contains(c)) {
        return RiskLevel::AlwaysApprove;
    }

    // Everything else -- session approve
    RiskLevel::SessionApprove
}

fn main() {
    // File write examples
    println!("Write src/main.rs: {:?}", classify_file_write("src/main.rs"));
    println!("Write Cargo.toml: {:?}", classify_file_write("Cargo.toml"));
    println!("Write .env: {:?}", classify_file_write(".env"));
    println!("Write tests/test_foo.rs: {:?}", classify_file_write("tests/test_foo.rs"));

    // Shell command examples
    println!("\ncargo test: {:?}", classify_shell_command("cargo test"));
    println!("cargo publish: {:?}", classify_shell_command("cargo publish"));
    println!("git push origin main: {:?}", classify_shell_command("git push origin main"));
    println!("rm -rf /: {:?}", classify_shell_command("rm -rf /"));
}
```

## The Approval State Machine

An approval flow is more than a yes/no prompt. It needs to handle timeouts, remember past decisions, and support different approval modes. Let's model this as a state machine:

```rust
use std::collections::HashMap;
use std::io::{self, Write};
use std::time::{Duration, Instant};

/// Tracks approval state across a session.
struct ApprovalManager {
    /// Approvals granted for the remainder of this session.
    /// Key is a "signature" of the operation type.
    session_approvals: HashMap<String, Instant>,
    /// How long session approvals remain valid
    session_timeout: Duration,
    /// Maximum time to wait for user input before auto-denying
    prompt_timeout: Duration,
}

#[derive(Debug, Clone)]
enum ApprovalDecision {
    Approved,
    Denied,
    ApprovedForSession,
    TimedOut,
}

/// What we show the user when requesting approval.
struct ApprovalRequest {
    tool_name: String,
    description: String,
    risk_level: String,
    details: Vec<String>,
}

impl ApprovalManager {
    fn new() -> Self {
        Self {
            session_approvals: HashMap::new(),
            session_timeout: Duration::from_secs(3600), // 1 hour
            prompt_timeout: Duration::from_secs(120),    // 2 minutes
        }
    }

    /// Check if this operation was already approved for the session.
    fn has_session_approval(&self, signature: &str) -> bool {
        if let Some(granted_at) = self.session_approvals.get(signature) {
            if granted_at.elapsed() < self.session_timeout {
                return true;
            }
        }
        false
    }

    /// Record a session-level approval.
    fn grant_session_approval(&mut self, signature: String) {
        self.session_approvals.insert(signature, Instant::now());
    }

    /// Format and display an approval prompt, then read the user's decision.
    fn prompt_user(&mut self, request: &ApprovalRequest) -> ApprovalDecision {
        println!("\n{}", "=".repeat(60));
        println!("APPROVAL REQUIRED: {}", request.tool_name);
        println!("{}", "-".repeat(60));
        println!("Risk Level: {}", request.risk_level);
        println!("Description: {}", request.description);
        if !request.details.is_empty() {
            println!("\nDetails:");
            for detail in &request.details {
                println!("  - {}", detail);
            }
        }
        println!("{}", "-".repeat(60));
        println!("Options:");
        println!("  [y] Yes, approve this action");
        println!("  [n] No, deny this action");
        println!("  [s] Yes, and approve similar actions for this session");
        println!("{}", "=".repeat(60));
        print!("Your choice: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        match input.trim().to_lowercase().as_str() {
            "y" | "yes" => ApprovalDecision::Approved,
            "s" | "session" => ApprovalDecision::ApprovedForSession,
            _ => ApprovalDecision::Denied,
        }
    }

    /// The main entry point: decide whether to approve a tool invocation.
    fn evaluate(
        &mut self,
        tool_name: &str,
        risk_level: RiskLevel,
        description: &str,
        details: Vec<String>,
    ) -> ApprovalDecision {
        match risk_level {
            RiskLevel::Safe => ApprovalDecision::Approved,
            RiskLevel::Blocked => {
                println!("BLOCKED: {} - operation is not allowed", tool_name);
                ApprovalDecision::Denied
            }
            RiskLevel::SessionApprove => {
                let signature = format!("{}:{}", tool_name, "session");
                if self.has_session_approval(&signature) {
                    println!("Auto-approved (session): {}", tool_name);
                    return ApprovalDecision::Approved;
                }
                let decision = self.prompt_user(&ApprovalRequest {
                    tool_name: tool_name.to_string(),
                    description: description.to_string(),
                    risk_level: "Session Approve".to_string(),
                    details,
                });
                if matches!(decision, ApprovalDecision::ApprovedForSession) {
                    self.grant_session_approval(signature);
                }
                decision
            }
            RiskLevel::AlwaysApprove => {
                self.prompt_user(&ApprovalRequest {
                    tool_name: tool_name.to_string(),
                    description: description.to_string(),
                    risk_level: "ALWAYS APPROVE (high risk)".to_string(),
                    details,
                })
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
enum RiskLevel {
    Safe,
    SessionApprove,
    AlwaysApprove,
    Blocked,
}

fn main() {
    let mut manager = ApprovalManager::new();

    // Safe operation -- auto-approved
    let decision = manager.evaluate(
        "shell",
        RiskLevel::Safe,
        "Run cargo test",
        vec!["Command: cargo test".into()],
    );
    println!("Decision: {:?}\n", decision);

    // Blocked operation -- auto-denied
    let decision = manager.evaluate(
        "shell",
        RiskLevel::Blocked,
        "Run rm -rf /",
        vec!["Command: rm -rf /".into()],
    );
    println!("Decision: {:?}\n", decision);

    // In a real agent, SessionApprove and AlwaysApprove operations
    // would pause here waiting for user input via prompt_user().
    // For this example, we demonstrate the classification logic.
    println!("Session approve would prompt user for: file_write to Cargo.toml");
    println!("Always approve would prompt user for: git push origin main");
}
```

## Designing Effective Approval Prompts

A good approval prompt gives the user enough information to make an informed decision without overwhelming them. The worst approval UX is a generic "Allow this action? [y/n]" with no context. The best shows exactly what will happen and why it matters.

Key elements of an effective approval prompt:

1. **What**: The exact operation (not just "shell command" but `cargo publish --registry crates-io`)
2. **Where**: The path or scope (`/home/user/myproject/Cargo.toml`)
3. **Why it matters**: The risk classification and specific concern ("This will publish your crate publicly and cannot be undone")
4. **What changes**: For file writes, show a diff preview. For commands, show the full command with arguments.

::: wild In the Wild
Claude Code presents approval prompts with full context: for file edits, it shows a syntax-highlighted diff of the proposed changes. For shell commands, it displays the complete command string. Users can type "y" to approve once, or the agent remembers the tool pattern for the session. Claude Code also supports a `--dangerously-skip-permissions` flag for CI/CD environments where a human cannot be in the loop, but this requires explicit opt-in. Codex takes a similar approach, showing diffs for file changes and full commands for shell operations, with the ability to approve or reject each action.
:::

::: python Coming from Python
Python developers are familiar with interactive prompts from tools like `pip install` ("Proceed (Y/n)?") or `git` ("Are you sure you want to...?"). The Rust implementation follows the same pattern but adds type safety -- the `ApprovalDecision` enum makes it impossible to forget handling a case, while Python's string-based approach ("y"/"n") is prone to unchecked branches. Rust's exhaustive match forces you to handle `Approved`, `Denied`, `ApprovedForSession`, and `TimedOut` explicitly.
:::

## Batch Approvals and Auto-Approve Patterns

For workflows where stopping for approval every few seconds would be impractical, you can implement batch approval patterns:

```rust
/// An auto-approve rule that grants blanket approval for matching operations.
#[derive(Debug, Clone)]
struct AutoApproveRule {
    /// Description of what this rule covers
    description: String,
    /// Tool name pattern (supports wildcards)
    tool_pattern: String,
    /// Optional path pattern
    path_pattern: Option<String>,
    /// Optional command pattern
    command_pattern: Option<String>,
}

impl AutoApproveRule {
    fn matches(&self, tool_name: &str, path: Option<&str>, command: Option<&str>) -> bool {
        let tool_match = if self.tool_pattern == "*" {
            true
        } else {
            tool_name == self.tool_pattern
        };

        let path_match = match (&self.path_pattern, path) {
            (Some(pattern), Some(p)) => p.starts_with(pattern.as_str()),
            (Some(_), None) => false,
            (None, _) => true,
        };

        let cmd_match = match (&self.command_pattern, command) {
            (Some(pattern), Some(c)) => c.starts_with(pattern.as_str()),
            (Some(_), None) => false,
            (None, _) => true,
        };

        tool_match && path_match && cmd_match
    }
}

fn main() {
    let auto_rules = vec![
        AutoApproveRule {
            description: "Allow all cargo commands".into(),
            tool_pattern: "shell".into(),
            path_pattern: None,
            command_pattern: Some("cargo".into()),
        },
        AutoApproveRule {
            description: "Allow writing to src/".into(),
            tool_pattern: "file_write".into(),
            path_pattern: Some("src/".into()),
            command_pattern: None,
        },
    ];

    let test_cases = vec![
        ("shell", None, Some("cargo test")),
        ("shell", None, Some("rm -rf /")),
        ("file_write", Some("src/main.rs"), None),
        ("file_write", Some("Cargo.toml"), None),
    ];

    for (tool, path, cmd) in test_cases {
        let approved = auto_rules.iter().any(|r| r.matches(tool, path, cmd));
        println!(
            "Tool: {}, Path: {:?}, Cmd: {:?} => Auto-approve: {}",
            tool, path, cmd, approved
        );
    }
}
```

## Key Takeaways

- Risk classification should categorize every tool invocation into Safe (auto-approve), SessionApprove (approve once), AlwaysApprove (approve every time), or Blocked (never allowed)
- Effective approval prompts show the exact operation, its scope, and why it is risky -- not just a generic "allow this?"
- Session-level approvals prevent prompt fatigue by remembering past decisions, but they should expire after a configurable timeout
- Auto-approve rules enable CI/CD and batch workflows where human-in-the-loop is impractical, but they require explicit configuration to prevent accidental over-permissioning
- The approval state machine must handle edge cases like timeouts, user cancellation, and the distinction between "deny this once" and "deny all similar actions"
