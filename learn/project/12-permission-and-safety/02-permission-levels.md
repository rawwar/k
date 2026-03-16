---
title: Permission Levels
description: Designing a tiered permission system that classifies agent operations as read-only, write, or destructive, and enforces different approval requirements for each tier.
---

# Permission Levels

> **What you'll learn:**
> - How to classify agent operations into permission tiers based on risk and reversibility
> - How to implement a permission registry that maps tools and subcommands to required levels
> - Patterns for escalating and de-escalating permissions during a session

The threat model identified what can go wrong. Now you need a structure for controlling *what the agent is allowed to do at any given moment*. Permission levels are the foundation of that control — they classify every agent operation into a risk tier and enforce different requirements for each tier.

Think of permission levels as a dial, not a switch. At one end, the agent can only read files and report information. At the other end, it can execute any command without asking. Most users want something in between: the agent should write code freely but ask before running destructive commands. The permission system makes this preference explicit and enforceable.

## Designing the Permission Tiers

A good permission system needs enough tiers to be useful but few enough to be understandable. Three tiers strike the right balance for a coding agent:

```rust
/// Permission level that controls what operations the agent can perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PermissionLevel {
    /// Agent can only read files, list directories, search code.
    /// No side effects whatsoever.
    ReadOnly,
    /// Agent can read and write files, run safe commands.
    /// Destructive operations require approval.
    Standard,
    /// Agent can perform any operation without approval.
    /// Use with caution — intended for trusted, well-tested tasks.
    FullAuto,
}

impl PermissionLevel {
    /// Check whether this level permits the given operation.
    pub fn permits(&self, required: PermissionLevel) -> bool {
        *self >= required
    }
}

impl std::fmt::Display for PermissionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionLevel::ReadOnly => write!(f, "read-only"),
            PermissionLevel::Standard => write!(f, "standard"),
            PermissionLevel::FullAuto => write!(f, "full-auto"),
        }
    }
}
```

The `PartialOrd` and `Ord` derives are deliberate — they create a natural ordering where `ReadOnly < Standard < FullAuto`. This means checking "does the current level allow this operation?" reduces to a simple comparison.

::: python Coming from Python
In Python, you might model permission levels with an `IntEnum` so you can compare them with `<` and `>=`:
```python
from enum import IntEnum

class PermissionLevel(IntEnum):
    READ_ONLY = 1
    STANDARD = 2
    FULL_AUTO = 3
```
Rust's `derive(Ord)` on a fieldless enum gives you the same comparison capability, but it is based on the declaration order of variants rather than explicit integer values. The first variant is the smallest.
:::

## Classifying Operations

Every tool call the agent makes needs a required permission level. Let's define an `OperationClass` that maps tool operations to their minimum permission:

```rust
/// Classification of an operation's risk and required permission level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationClass {
    /// Pure read: file reads, directory listings, search, git status.
    /// Never modifies state.
    Read,
    /// Creates or modifies files within the project directory.
    /// Reversible through checkpoints.
    Write,
    /// Executes a shell command that has been classified as safe.
    SafeExec,
    /// Executes a shell command that could have side effects.
    UnsafeExec,
    /// Operations that are destructive or irreversible:
    /// deleting files, force push, modifying git history.
    Destructive,
}

impl OperationClass {
    /// The minimum permission level required for this operation class.
    pub fn required_permission(&self) -> PermissionLevel {
        match self {
            OperationClass::Read => PermissionLevel::ReadOnly,
            OperationClass::Write => PermissionLevel::Standard,
            OperationClass::SafeExec => PermissionLevel::Standard,
            OperationClass::UnsafeExec => PermissionLevel::Standard,
            OperationClass::Destructive => PermissionLevel::FullAuto,
        }
    }

    /// Whether this operation requires explicit user approval
    /// even when the permission level is sufficient.
    pub fn requires_approval(&self, current_level: PermissionLevel) -> bool {
        match self {
            OperationClass::Read => false,
            OperationClass::Write => current_level == PermissionLevel::Standard,
            OperationClass::SafeExec => false,
            OperationClass::UnsafeExec => current_level != PermissionLevel::FullAuto,
            OperationClass::Destructive => current_level != PermissionLevel::FullAuto,
        }
    }
}
```

Notice that `requires_approval` adds a second dimension beyond simple permission checking. Even in `Standard` mode, safe shell commands (like `cargo check` or `ls`) do not need approval, but unsafe commands always do. Only `FullAuto` mode skips approval entirely.

## The Permission Registry

Now you need a registry that maps specific tool invocations to their operation class. This is where you decide, at a granular level, what each tool is allowed to do:

```rust
use std::collections::HashMap;

/// Maps tool names and optional subcommands to operation classes.
pub struct PermissionRegistry {
    /// Map of "tool_name" or "tool_name:subcommand" to operation class.
    rules: HashMap<String, OperationClass>,
    /// Default class for unregistered operations.
    default_class: OperationClass,
}

impl PermissionRegistry {
    pub fn new() -> Self {
        let mut rules = HashMap::new();

        // File operations
        rules.insert("read_file".to_string(), OperationClass::Read);
        rules.insert("list_directory".to_string(), OperationClass::Read);
        rules.insert("search_files".to_string(), OperationClass::Read);
        rules.insert("write_file".to_string(), OperationClass::Write);

        // Git operations
        rules.insert("git:status".to_string(), OperationClass::Read);
        rules.insert("git:diff".to_string(), OperationClass::Read);
        rules.insert("git:log".to_string(), OperationClass::Read);
        rules.insert("git:add".to_string(), OperationClass::Write);
        rules.insert("git:commit".to_string(), OperationClass::Write);
        rules.insert("git:push".to_string(), OperationClass::UnsafeExec);
        rules.insert("git:push --force".to_string(), OperationClass::Destructive);
        rules.insert("git:reset --hard".to_string(), OperationClass::Destructive);

        // Shell execution
        rules.insert("shell:safe".to_string(), OperationClass::SafeExec);
        rules.insert("shell:unsafe".to_string(), OperationClass::UnsafeExec);

        Self {
            rules,
            default_class: OperationClass::UnsafeExec,
        }
    }

    /// Look up the operation class for a tool call.
    /// Tries "tool:subcommand" first, then "tool" alone,
    /// then falls back to the default.
    pub fn classify(&self, tool: &str, subcommand: Option<&str>) -> &OperationClass {
        if let Some(sub) = subcommand {
            let key = format!("{}:{}", tool, sub);
            if let Some(class) = self.rules.get(&key) {
                return class;
            }
        }
        self.rules
            .get(tool)
            .unwrap_or(&self.default_class)
    }
}
```

The fallback to `UnsafeExec` as the default is a safety-first choice: any operation you forget to register will require approval in Standard mode. It is far better to over-prompt than to silently allow an unclassified dangerous operation.

## The Permission Gate

Let's tie it all together with a `PermissionGate` that checks whether an operation is allowed and routes it to the approval system if needed:

```rust
/// Result of a permission check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecision {
    /// Operation is allowed without further checks.
    Allowed,
    /// Operation requires user approval before proceeding.
    NeedsApproval { reason: String },
    /// Operation is denied at the current permission level.
    Denied { reason: String },
}

pub struct PermissionGate {
    current_level: PermissionLevel,
    registry: PermissionRegistry,
}

impl PermissionGate {
    pub fn new(level: PermissionLevel) -> Self {
        Self {
            current_level: level,
            registry: PermissionRegistry::new(),
        }
    }

    /// Check whether an operation is permitted, needs approval, or is denied.
    pub fn check(&self, tool: &str, subcommand: Option<&str>) -> PermissionDecision {
        let op_class = self.registry.classify(tool, subcommand);
        let required = op_class.required_permission();

        // First check: does the current level meet the minimum requirement?
        if !self.current_level.permits(required) {
            return PermissionDecision::Denied {
                reason: format!(
                    "Operation requires {} permission, current level is {}",
                    required, self.current_level
                ),
            };
        }

        // Second check: does the operation need explicit approval?
        if op_class.requires_approval(self.current_level) {
            let label = subcommand
                .map(|s| format!("{}:{}", tool, s))
                .unwrap_or_else(|| tool.to_string());
            return PermissionDecision::NeedsApproval {
                reason: format!(
                    "Operation '{}' classified as {:?} — requires approval",
                    label, op_class
                ),
            };
        }

        PermissionDecision::Allowed
    }

    /// Change the current permission level (e.g., user escalates to FullAuto).
    pub fn set_level(&mut self, level: PermissionLevel) {
        self.current_level = level;
    }

    pub fn current_level(&self) -> PermissionLevel {
        self.current_level
    }
}

fn main() {
    let gate = PermissionGate::new(PermissionLevel::Standard);

    // Reading is always fine
    let decision = gate.check("read_file", None);
    println!("read_file: {:?}", decision);

    // Writing needs approval in Standard mode
    let decision = gate.check("write_file", None);
    println!("write_file: {:?}", decision);

    // Safe shell commands are fine
    let decision = gate.check("shell", Some("safe"));
    println!("shell:safe: {:?}", decision);

    // Force push is denied below FullAuto
    let decision = gate.check("git", Some("push --force"));
    println!("git push --force: {:?}", decision);

    // In ReadOnly mode, writes are denied entirely
    let readonly_gate = PermissionGate::new(PermissionLevel::ReadOnly);
    let decision = readonly_gate.check("write_file", None);
    println!("write_file (read-only): {:?}", decision);
}
```

## Session-Level Permission Escalation

Users often start a session in Standard mode and escalate to FullAuto after verifying the agent understands their intent. Your permission system should support this, but with guardrards:

```rust
/// Tracks permission changes within a session for audit purposes.
#[derive(Debug, Clone)]
pub struct PermissionChange {
    pub from: PermissionLevel,
    pub to: PermissionLevel,
    pub timestamp: std::time::Instant,
    pub reason: String,
}

pub struct SessionPermissions {
    current: PermissionLevel,
    initial: PermissionLevel,
    history: Vec<PermissionChange>,
}

impl SessionPermissions {
    pub fn new(initial: PermissionLevel) -> Self {
        Self {
            current: initial,
            initial,
            history: Vec::new(),
        }
    }

    /// Escalate permissions. Records the change for audit.
    pub fn escalate(&mut self, to: PermissionLevel, reason: String) {
        let change = PermissionChange {
            from: self.current,
            to,
            timestamp: std::time::Instant::now(),
            reason,
        };
        self.history.push(change);
        self.current = to;
    }

    /// Reset to the initial permission level.
    pub fn reset(&mut self) {
        if self.current != self.initial {
            self.escalate(
                self.initial,
                "Reset to initial permission level".to_string(),
            );
        }
    }

    pub fn current(&self) -> PermissionLevel {
        self.current
    }

    pub fn change_count(&self) -> usize {
        self.history.len()
    }
}
```

::: wild In the Wild
Claude Code uses three permission modes: "plan" (read-only, the agent can only describe what it would do), "normal" (the agent can read and write but prompts for approval on shell commands), and "full auto" (the agent can do anything without prompting). Users can toggle between modes during a session. OpenCode has a similar two-tier system with an explicit "auto-approve" toggle for tool calls.
:::

## Integrating with the Agent Loop

The permission gate slots into the agent loop right before tool execution. Here is how the check flows:

1. The LLM requests a tool call (e.g., `write_file` with a path and content).
2. The agent loop calls `permission_gate.check("write_file", None)`.
3. If the result is `Allowed`, execute the tool immediately.
4. If the result is `NeedsApproval`, show the operation details and wait for user input (covered in the next subchapter).
5. If the result is `Denied`, return an error to the LLM explaining the operation is not permitted at the current permission level.

This is a clean separation of concerns: the permission system decides *whether* to proceed, the approval system handles *how* to ask the user, and the tool system handles *what* to execute. Each can be tested and reasoned about independently.

## Key Takeaways

- Three permission tiers — ReadOnly, Standard, and FullAuto — provide enough granularity without overwhelming users with configuration.
- The `PermissionRegistry` maps every tool invocation to an `OperationClass`, with a safe default of `UnsafeExec` for unregistered operations so you fail toward caution.
- The `PermissionGate` makes a three-way decision — Allowed, NeedsApproval, or Denied — that cleanly separates permission logic from approval UX and tool execution.
- Permission escalation within a session should be tracked for audit purposes, and resetting to the initial level should always be possible.
- Rust's `derive(Ord)` on fieldless enums gives you natural ordering for permission comparisons, avoiding the need for manual integer assignments.
