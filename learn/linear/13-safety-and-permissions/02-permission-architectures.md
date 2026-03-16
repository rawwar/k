---
title: Permission Architectures
description: Design layered permission systems that govern what actions a coding agent is allowed to perform at each scope level.
---

# Permission Architectures

> **What you'll learn:**
> - How to design capability-based permission models that scope agent access by tool, directory, and operation type
> - The tradeoffs between coarse-grained and fine-grained permission systems for developer experience and safety
> - How to implement permission checks as middleware that intercepts every tool invocation before execution

With your threat model in hand, the next step is building the system that enforces boundaries. A permission architecture defines what an agent is allowed to do, at what scope, and under what conditions. Get this wrong and you either have an agent that is too dangerous to use, or one that is too restricted to be useful. The art is finding the right balance.

## The Permission Design Space

Permission systems for coding agents exist on a spectrum. On one end, you have simple boolean flags: "can the agent run shell commands?" On the other end, you have fine-grained policies: "the agent can run `cargo test` in `/home/user/project/` but not `cargo publish`, and it can read any `.rs` file but only write to files under `src/`."

Let's start by defining the core types that model this spectrum:

```rust
use std::path::PathBuf;

/// The broad categories of operations an agent can perform.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ToolCategory {
    FileRead,
    FileWrite,
    ShellExecute,
    NetworkAccess,
    GitOperation,
}

/// A specific permission rule governing one category of operation.
#[derive(Debug, Clone)]
enum PermissionRule {
    /// Allow everything in this category
    AllowAll,
    /// Deny everything in this category
    DenyAll,
    /// Allow only within specified paths
    ScopedPaths(Vec<PathBuf>),
    /// Allow only specific commands/patterns
    AllowPatterns(Vec<String>),
    /// Require user approval before each invocation
    RequireApproval,
}

/// A complete permission policy combining rules for all categories.
#[derive(Debug, Clone)]
struct PermissionPolicy {
    rules: Vec<(ToolCategory, PermissionRule)>,
    /// The root directory the agent is allowed to operate in
    project_root: PathBuf,
    /// Whether to default to deny when no rule matches
    default_deny: bool,
}
```

## Capability-Based Design

The most robust approach to agent permissions is capability-based security. Instead of maintaining a list of things the agent *cannot* do (which you will inevitably miss something on), you define exactly what the agent *can* do, and everything else is denied by default.

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A capability represents a specific, bounded permission granted to the agent.
#[derive(Debug, Clone)]
struct Capability {
    /// Human-readable name for this capability
    name: String,
    /// What tool category this applies to
    category: ToolCategory,
    /// Constraints that narrow the scope of this capability
    constraints: Vec<Constraint>,
}

#[derive(Debug, Clone)]
enum Constraint {
    /// Only allow operations within these directories
    PathPrefix(PathBuf),
    /// Only allow these specific commands
    CommandWhitelist(Vec<String>),
    /// Only allow files matching these glob patterns
    FileGlob(String),
    /// Maximum file size for write operations (in bytes)
    MaxFileSize(u64),
    /// Operations are read-only
    ReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ToolCategory {
    FileRead,
    FileWrite,
    ShellExecute,
    NetworkAccess,
    GitOperation,
}

/// The capability store holds all granted capabilities and checks requests.
struct CapabilityStore {
    capabilities: Vec<Capability>,
}

impl CapabilityStore {
    fn new() -> Self {
        Self {
            capabilities: Vec::new(),
        }
    }

    fn grant(&mut self, capability: Capability) {
        self.capabilities.push(capability);
    }

    /// Check whether a tool invocation is allowed by any granted capability.
    fn check(&self, category: &ToolCategory, path: Option<&Path>, command: Option<&str>) -> PermissionResult {
        for cap in &self.capabilities {
            if &cap.category != category {
                continue;
            }

            let all_constraints_pass = cap.constraints.iter().all(|constraint| {
                match constraint {
                    Constraint::PathPrefix(prefix) => {
                        path.map_or(true, |p| p.starts_with(prefix))
                    }
                    Constraint::CommandWhitelist(allowed) => {
                        command.map_or(true, |cmd| {
                            allowed.iter().any(|a| cmd.starts_with(a))
                        })
                    }
                    Constraint::FileGlob(pattern) => {
                        // Simplified glob check -- in production, use the `glob` crate
                        path.map_or(true, |p| {
                            p.to_string_lossy().contains(
                                pattern.trim_start_matches('*').trim_end_matches('*')
                            )
                        })
                    }
                    Constraint::MaxFileSize(_) => true, // checked at execution time
                    Constraint::ReadOnly => category != &ToolCategory::FileWrite,
                }
            });

            if all_constraints_pass {
                return PermissionResult::Allowed(cap.name.clone());
            }
        }

        PermissionResult::Denied("No matching capability found".into())
    }
}

#[derive(Debug)]
enum PermissionResult {
    Allowed(String),      // name of the capability that granted access
    Denied(String),       // reason for denial
    NeedsApproval(String), // needs human confirmation
}

fn main() {
    let mut store = CapabilityStore::new();

    // Grant: read any file under the project directory
    store.grant(Capability {
        name: "project-read".into(),
        category: ToolCategory::FileRead,
        constraints: vec![
            Constraint::PathPrefix(PathBuf::from("/home/user/myproject")),
        ],
    });

    // Grant: write only Rust source files under src/
    store.grant(Capability {
        name: "source-write".into(),
        category: ToolCategory::FileWrite,
        constraints: vec![
            Constraint::PathPrefix(PathBuf::from("/home/user/myproject/src")),
            Constraint::FileGlob("*.rs".into()),
        ],
    });

    // Grant: run only cargo and git commands
    store.grant(Capability {
        name: "safe-commands".into(),
        category: ToolCategory::ShellExecute,
        constraints: vec![
            Constraint::CommandWhitelist(vec![
                "cargo".into(),
                "git status".into(),
                "git diff".into(),
            ]),
        ],
    });

    // Test: reading a project file -- should be allowed
    let result = store.check(
        &ToolCategory::FileRead,
        Some(Path::new("/home/user/myproject/src/main.rs")),
        None,
    );
    println!("Read project file: {:?}", result);

    // Test: reading outside project -- should be denied
    let result = store.check(
        &ToolCategory::FileRead,
        Some(Path::new("/etc/passwd")),
        None,
    );
    println!("Read /etc/passwd: {:?}", result);

    // Test: running cargo test -- should be allowed
    let result = store.check(
        &ToolCategory::ShellExecute,
        None,
        Some("cargo test"),
    );
    println!("Run cargo test: {:?}", result);

    // Test: running rm -rf -- should be denied
    let result = store.check(
        &ToolCategory::ShellExecute,
        None,
        Some("rm -rf /"),
    );
    println!("Run rm -rf /: {:?}", result);
}
```

## Permission Middleware Pattern

In a well-designed agent, permission checks do not live inside each tool implementation. Instead, they run as middleware that intercepts every tool call before it reaches the tool. This ensures no tool can accidentally skip its permission check:

```rust
use std::path::Path;

/// A tool invocation request, before any permission checks.
#[derive(Debug, Clone)]
struct ToolRequest {
    tool_name: String,
    category: ToolCategory,
    path: Option<String>,
    command: Option<String>,
    args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ToolCategory {
    FileRead,
    FileWrite,
    ShellExecute,
    NetworkAccess,
    GitOperation,
}

/// The result of executing a tool.
#[derive(Debug)]
struct ToolResponse {
    success: bool,
    output: String,
}

/// Trait for permission middleware layers.
trait PermissionMiddleware {
    /// Check a request and return Ok to proceed, Err to block.
    fn check(&self, request: &ToolRequest) -> Result<(), String>;
}

/// Checks that file operations stay within the project root.
struct PathBoundaryCheck {
    project_root: String,
}

impl PermissionMiddleware for PathBoundaryCheck {
    fn check(&self, request: &ToolRequest) -> Result<(), String> {
        if let Some(path) = &request.path {
            // Resolve the path to catch traversal attempts like "../../../etc/passwd"
            let resolved = std::fs::canonicalize(path)
                .unwrap_or_else(|_| std::path::PathBuf::from(path));
            if !resolved.starts_with(&self.project_root) {
                return Err(format!(
                    "Path '{}' is outside project root '{}'",
                    path, self.project_root
                ));
            }
        }
        Ok(())
    }
}

/// Checks commands against a denylist of dangerous patterns.
struct CommandDenylistCheck {
    denied_patterns: Vec<String>,
}

impl PermissionMiddleware for CommandDenylistCheck {
    fn check(&self, request: &ToolRequest) -> Result<(), String> {
        if let Some(cmd) = &request.command {
            let cmd_lower = cmd.to_lowercase();
            for pattern in &self.denied_patterns {
                if cmd_lower.contains(&pattern.to_lowercase()) {
                    return Err(format!(
                        "Command contains denied pattern: '{}'",
                        pattern
                    ));
                }
            }
        }
        Ok(())
    }
}

/// The permission pipeline runs all middleware checks in order.
struct PermissionPipeline {
    checks: Vec<Box<dyn PermissionMiddleware>>,
}

impl PermissionPipeline {
    fn new() -> Self {
        Self { checks: Vec::new() }
    }

    fn add_check(&mut self, check: Box<dyn PermissionMiddleware>) {
        self.checks.push(check);
    }

    /// Run all checks. Returns the first error encountered, or Ok if all pass.
    fn evaluate(&self, request: &ToolRequest) -> Result<(), String> {
        for check in &self.checks {
            check.check(request)?;
        }
        Ok(())
    }
}

fn main() {
    let mut pipeline = PermissionPipeline::new();

    pipeline.add_check(Box::new(PathBoundaryCheck {
        project_root: "/home/user/myproject".into(),
    }));

    pipeline.add_check(Box::new(CommandDenylistCheck {
        denied_patterns: vec![
            "rm -rf".into(),
            "sudo".into(),
            "chmod 777".into(),
            "> /dev/".into(),
            "curl".into(),
        ],
    }));

    // Safe request: read a project file
    let safe_request = ToolRequest {
        tool_name: "read_file".into(),
        category: ToolCategory::FileRead,
        path: Some("/home/user/myproject/src/main.rs".into()),
        command: None,
        args: vec![],
    };

    match pipeline.evaluate(&safe_request) {
        Ok(()) => println!("ALLOWED: {:?}", safe_request.tool_name),
        Err(reason) => println!("DENIED: {} - {}", safe_request.tool_name, reason),
    }

    // Dangerous request: command with denied pattern
    let dangerous_request = ToolRequest {
        tool_name: "shell".into(),
        category: ToolCategory::ShellExecute,
        path: None,
        command: Some("sudo rm -rf /tmp/important".into()),
        args: vec![],
    };

    match pipeline.evaluate(&dangerous_request) {
        Ok(()) => println!("ALLOWED: {:?}", dangerous_request.tool_name),
        Err(reason) => println!("DENIED: {} - {}", dangerous_request.tool_name, reason),
    }
}
```

## Tiered Permission Levels

Most production agents define two or three permission tiers that users can choose from, balancing convenience against safety:

| Tier | File Read | File Write | Shell | Network | Use Case |
|------|-----------|------------|-------|---------|----------|
| **Read-Only** | Project only | Denied | `cargo check`, `git status` only | Denied | Code review, analysis |
| **Standard** | Project only | `src/` only | Allowlisted commands | LLM API only | Normal development |
| **Full Access** | Anywhere | Anywhere + approval | Any + approval for dangerous | Open | Advanced users, CI/CD |

::: wild In the Wild
Claude Code implements a tiered system with three levels: operations that are always allowed (reading files in the project), operations that require one-time approval per session (writing files, running safe commands), and operations that always require approval (running potentially dangerous commands). Users can also configure "allowlisted" commands and paths that permanently skip approval. Codex uses a different model with three tiers: Suggest (plan only, no execution), Auto Edit (can write files but not run commands), and Full Auto (can write files and run commands with network disabled).
:::

::: python Coming from Python
Python's permission model is essentially nonexistent at the language level -- any Python script can read any file, execute any command, and open any network connection. Rust does not add restrictions at the language level either, but its type system makes it natural to enforce permissions at compile time. By defining `ToolRequest` types that *must* pass through a `PermissionPipeline`, you make it structurally difficult to skip the check. In Python, you would typically enforce this through runtime decorators, which are easier to forget or bypass.
:::

## Scoping Permissions by Context

Permissions do not have to be static. You can adjust them based on context -- what the agent is doing right now and what it has done so far:

```rust
/// Dynamic permission context that adjusts based on agent state.
struct PermissionContext {
    /// Current trust level, which can escalate during a session
    trust_level: TrustLevel,
    /// How many tool calls have been made this session
    tool_call_count: u32,
    /// How many errors have occurred (high error count may reduce trust)
    error_count: u32,
    /// Files the agent has already read (can inform write permissions)
    files_read: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum TrustLevel {
    /// Agent just started, minimal trust
    Initial,
    /// User has approved some operations, moderate trust
    Elevated,
    /// User explicitly enabled full access for this session
    Full,
}

impl PermissionContext {
    fn should_auto_approve(&self, category: &str) -> bool {
        match self.trust_level {
            TrustLevel::Full => true,
            TrustLevel::Elevated => {
                category == "FileRead" || category == "GitOperation"
            }
            TrustLevel::Initial => category == "FileRead",
        }
    }

    fn should_restrict(&self) -> bool {
        // If the agent is making lots of errors, restrict permissions
        self.error_count > 3 && self.tool_call_count > 0
            && (self.error_count as f32 / self.tool_call_count as f32) > 0.5
    }
}

fn main() {
    let ctx = PermissionContext {
        trust_level: TrustLevel::Elevated,
        tool_call_count: 10,
        error_count: 1,
        files_read: vec!["src/main.rs".into()],
    };

    println!("Auto-approve FileRead: {}", ctx.should_auto_approve("FileRead"));
    println!("Auto-approve ShellExecute: {}", ctx.should_auto_approve("ShellExecute"));
    println!("Should restrict: {}", ctx.should_restrict());
}
```

## Key Takeaways

- Capability-based security (default-deny with explicit grants) is safer than denylist-based security (default-allow with explicit blocks) because you cannot forget to block something you never allowed
- Permission middleware intercepts every tool call before execution, ensuring consistent enforcement regardless of which tool is invoked
- Tiered permission levels let users choose their safety/convenience tradeoff, with read-only mode for code review and full-access mode for advanced workflows
- Dynamic permission contexts allow the system to adjust trust levels based on session history, reducing permissions when error rates suggest the agent is struggling
- Path boundary checks must resolve symbolic links and canonicalize paths to prevent traversal attacks like `../../etc/passwd`
