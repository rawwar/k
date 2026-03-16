---
title: Plan Mode Design
description: Implement a plan-only execution mode where the agent proposes actions without executing them, enabling review before commitment.
---

# Plan Mode Design

> **What you'll learn:**
> - How to architect a dual-mode system where agents can operate in plan mode (propose only) or execute mode (act on proposals)
> - Techniques for generating detailed, reviewable execution plans that include diffs, command previews, and risk annotations
> - How to implement plan-to-execute transitions that let users selectively approve, modify, or reject individual planned actions

Every safety mechanism we have built so far tries to prevent bad actions. Plan mode takes a different approach: it prevents all actions until a human reviews and approves them. Instead of executing tool calls directly, the agent generates a plan -- a detailed description of everything it intends to do -- and presents it for review. Only after the user approves (in whole or in part) does execution begin.

This is the coding agent equivalent of a dry-run flag, and it is one of the most effective safety features you can build because it gives the user full visibility before anything happens.

## Dual-Mode Architecture

The key architectural insight is that plan mode and execute mode should share the same code paths. The agent reasons identically in both modes -- it reads files, decides what to change, and constructs tool calls. The only difference is what happens when a tool call is dispatched: in execute mode, the tool runs; in plan mode, the tool call is recorded without running.

```rust
/// The two execution modes the agent can operate in.
#[derive(Debug, Clone, PartialEq)]
enum ExecutionMode {
    /// Agent proposes actions but does not execute them
    Plan,
    /// Agent executes actions (possibly after plan review)
    Execute,
}

/// A planned action that the agent intends to take.
#[derive(Debug, Clone)]
struct PlannedAction {
    /// Unique identifier for this action
    id: u32,
    /// The tool that would be invoked
    tool_name: String,
    /// Human-readable description of what this action does
    description: String,
    /// The full details needed to execute this action
    details: ActionDetails,
    /// Risk assessment for this action
    risk: RiskAnnotation,
}

#[derive(Debug, Clone)]
enum ActionDetails {
    FileWrite {
        path: String,
        content: String,
        /// Diff against the current file content (if file exists)
        diff_preview: Option<String>,
    },
    FileCreate {
        path: String,
        content: String,
    },
    FileDelete {
        path: String,
    },
    ShellCommand {
        command: String,
        working_dir: String,
    },
    GitOperation {
        operation: String,
        args: Vec<String>,
    },
}

#[derive(Debug, Clone)]
struct RiskAnnotation {
    level: RiskLevel,
    explanation: String,
    reversible: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// The execution plan generated during plan mode.
struct ExecutionPlan {
    actions: Vec<PlannedAction>,
    next_id: u32,
}

impl ExecutionPlan {
    fn new() -> Self {
        Self {
            actions: Vec::new(),
            next_id: 1,
        }
    }

    /// Add an action to the plan.
    fn add_action(
        &mut self,
        tool_name: &str,
        description: &str,
        details: ActionDetails,
        risk: RiskAnnotation,
    ) -> u32 {
        let id = self.next_id;
        self.next_id += 1;

        self.actions.push(PlannedAction {
            id,
            tool_name: tool_name.to_string(),
            description: description.to_string(),
            details,
            risk,
        });

        id
    }

    /// Display the plan in a human-readable format.
    fn display(&self) {
        println!("=== Execution Plan ({} actions) ===\n", self.actions.len());

        for action in &self.actions {
            let risk_marker = match action.risk.level {
                RiskLevel::Low => "[LOW]",
                RiskLevel::Medium => "[MED]",
                RiskLevel::High => "[HIGH]",
                RiskLevel::Critical => "[CRITICAL]",
            };

            let reversible = if action.risk.reversible { "reversible" } else { "IRREVERSIBLE" };

            println!(
                "  {}. {} {} ({}) - {}",
                action.id, risk_marker, action.tool_name,
                reversible, action.description
            );

            match &action.details {
                ActionDetails::FileWrite { path, diff_preview, .. } => {
                    println!("     Path: {}", path);
                    if let Some(diff) = diff_preview {
                        println!("     Diff:\n{}", indent_lines(diff, "       "));
                    }
                }
                ActionDetails::FileCreate { path, content } => {
                    let preview = if content.len() > 100 {
                        format!("{}...", &content[..100])
                    } else {
                        content.clone()
                    };
                    println!("     Create: {} ({} bytes)", path, content.len());
                    println!("     Preview: {}", preview);
                }
                ActionDetails::ShellCommand { command, working_dir } => {
                    println!("     Command: {}", command);
                    println!("     Dir: {}", working_dir);
                }
                ActionDetails::FileDelete { path } => {
                    println!("     Delete: {}", path);
                }
                ActionDetails::GitOperation { operation, args } => {
                    println!("     Git: {} {}", operation, args.join(" "));
                }
            }
            println!("     Risk: {}", action.risk.explanation);
            println!();
        }
    }

    /// Get a summary of the plan's risk profile.
    fn risk_summary(&self) -> String {
        let critical = self.actions.iter().filter(|a| a.risk.level == RiskLevel::Critical).count();
        let high = self.actions.iter().filter(|a| a.risk.level == RiskLevel::High).count();
        let irreversible = self.actions.iter().filter(|a| !a.risk.reversible).count();

        format!(
            "{} actions total: {} critical, {} high risk, {} irreversible",
            self.actions.len(), critical, high, irreversible
        )
    }
}

fn indent_lines(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|line| format!("{}{}", prefix, line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn main() {
    let mut plan = ExecutionPlan::new();

    plan.add_action(
        "file_write",
        "Add error handling to main function",
        ActionDetails::FileWrite {
            path: "src/main.rs".into(),
            content: "// updated content...".into(),
            diff_preview: Some(
                "- fn main() {\n+ fn main() -> Result<(), Box<dyn std::error::Error>> {"
                    .into(),
            ),
        },
        RiskAnnotation {
            level: RiskLevel::Low,
            explanation: "Modifying existing source file within project".into(),
            reversible: true,
        },
    );

    plan.add_action(
        "file_create",
        "Create new error types module",
        ActionDetails::FileCreate {
            path: "src/errors.rs".into(),
            content: "use std::fmt;\n\n#[derive(Debug)]\npub enum AgentError { ... }".into(),
        },
        RiskAnnotation {
            level: RiskLevel::Low,
            explanation: "Creating new file within project src/".into(),
            reversible: true,
        },
    );

    plan.add_action(
        "shell",
        "Run tests to verify changes",
        ActionDetails::ShellCommand {
            command: "cargo test".into(),
            working_dir: "/home/user/myproject".into(),
        },
        RiskAnnotation {
            level: RiskLevel::Low,
            explanation: "Running test suite -- read-only operation".into(),
            reversible: true,
        },
    );

    plan.add_action(
        "shell",
        "Publish updated crate",
        ActionDetails::ShellCommand {
            command: "cargo publish".into(),
            working_dir: "/home/user/myproject".into(),
        },
        RiskAnnotation {
            level: RiskLevel::Critical,
            explanation: "Publishing to crates.io is irreversible".into(),
            reversible: false,
        },
    );

    plan.display();
    println!("Risk summary: {}", plan.risk_summary());
}
```

## Plan-to-Execute Transitions

After the user reviews a plan, they should be able to approve it in several ways: approve everything, approve selected actions, modify actions before executing, or reject the entire plan:

```rust
/// User decisions on a plan.
#[derive(Debug)]
enum PlanDecision {
    /// Execute all actions in the plan
    ApproveAll,
    /// Execute only the specified action IDs
    ApproveSelected(Vec<u32>),
    /// Reject the entire plan
    RejectAll,
    /// Approve all but skip the specified action IDs
    ApproveExcept(Vec<u32>),
}

/// Manages the transition from plan mode to execution.
struct PlanExecutor {
    plan: ExecutionPlan,
}

/// The result of applying a plan decision.
#[derive(Debug, Clone, PartialEq)]
enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone)]
struct PlannedAction {
    id: u32,
    tool_name: String,
    description: String,
    risk_level: RiskLevel,
}

struct ExecutionPlan {
    actions: Vec<PlannedAction>,
}

impl PlanExecutor {
    fn new(plan: ExecutionPlan) -> Self {
        Self { plan }
    }

    /// Apply the user's decision and return the actions to execute.
    fn apply_decision(&self, decision: &PlanDecision) -> Vec<&PlannedAction> {
        match decision {
            PlanDecision::ApproveAll => {
                self.plan.actions.iter().collect()
            }
            PlanDecision::ApproveSelected(ids) => {
                self.plan.actions
                    .iter()
                    .filter(|a| ids.contains(&a.id))
                    .collect()
            }
            PlanDecision::RejectAll => {
                vec![]
            }
            PlanDecision::ApproveExcept(excluded_ids) => {
                self.plan.actions
                    .iter()
                    .filter(|a| !excluded_ids.contains(&a.id))
                    .collect()
            }
        }
    }

    /// Execute the approved actions in order.
    fn execute(&self, decision: &PlanDecision) {
        let actions = self.apply_decision(decision);

        if actions.is_empty() {
            println!("No actions to execute.");
            return;
        }

        println!("Executing {} of {} planned actions:\n",
            actions.len(), self.plan.actions.len());

        for action in actions {
            println!("  Executing #{}: {} - {}",
                action.id, action.tool_name, action.description);
            // In production, this would dispatch to the actual tool
        }
    }
}

fn main() {
    let plan = ExecutionPlan {
        actions: vec![
            PlannedAction {
                id: 1,
                tool_name: "file_write".into(),
                description: "Update main.rs".into(),
                risk_level: RiskLevel::Low,
            },
            PlannedAction {
                id: 2,
                tool_name: "file_create".into(),
                description: "Create errors.rs".into(),
                risk_level: RiskLevel::Low,
            },
            PlannedAction {
                id: 3,
                tool_name: "shell".into(),
                description: "Run cargo test".into(),
                risk_level: RiskLevel::Low,
            },
            PlannedAction {
                id: 4,
                tool_name: "shell".into(),
                description: "Publish crate".into(),
                risk_level: RiskLevel::Critical,
            },
        ],
    };

    let executor = PlanExecutor::new(plan);

    // User approves everything except the publish step
    println!("--- Approve except #4 (publish) ---");
    executor.execute(&PlanDecision::ApproveExcept(vec![4]));

    println!("\n--- Approve only #1 and #3 ---");
    executor.execute(&PlanDecision::ApproveSelected(vec![1, 3]));

    println!("\n--- Reject all ---");
    executor.execute(&PlanDecision::RejectAll);
}
```

## Generating Meaningful Diffs

For file modifications, the plan should include a diff so the user can see exactly what will change. Here is a simple unified diff generator:

```rust
/// Generate a simple unified diff between two strings.
fn generate_diff(original: &str, modified: &str, filename: &str) -> String {
    let original_lines: Vec<&str> = original.lines().collect();
    let modified_lines: Vec<&str> = modified.lines().collect();

    let mut diff = format!("--- a/{}\n+++ b/{}\n", filename, filename);

    let max_lines = original_lines.len().max(modified_lines.len());

    // Simple line-by-line comparison (a real implementation would use
    // the Myers diff algorithm via the `similar` crate)
    let mut i = 0;
    while i < max_lines {
        let orig = original_lines.get(i).copied();
        let modi = modified_lines.get(i).copied();

        match (orig, modi) {
            (Some(o), Some(m)) if o == m => {
                diff.push_str(&format!(" {}\n", o));
            }
            (Some(o), Some(m)) => {
                diff.push_str(&format!("-{}\n", o));
                diff.push_str(&format!("+{}\n", m));
            }
            (Some(o), None) => {
                diff.push_str(&format!("-{}\n", o));
            }
            (None, Some(m)) => {
                diff.push_str(&format!("+{}\n", m));
            }
            (None, None) => break,
        }
        i += 1;
    }

    diff
}

fn main() {
    let original = r#"fn main() {
    println!("Hello, world!");
}"#;

    let modified = r#"fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Hello, world!");
    Ok(())
}"#;

    let diff = generate_diff(original, modified, "src/main.rs");
    println!("Diff preview:\n{}", diff);
}
```

::: wild In the Wild
Claude Code does not have a formal "plan mode" toggle, but its approval flow serves a similar purpose -- for every file write or command execution, the user sees exactly what will happen and can approve or reject it. Codex offers an explicit "Suggest" mode that is a pure plan mode: the agent analyzes the codebase and proposes changes, but does not execute any of them until the user explicitly applies the suggestions. This maps directly to the dual-mode architecture described above.
:::

::: python Coming from Python
Python developers will recognize this pattern from tools like `terraform plan` or `ansible --check` -- you review the proposed changes before applying them. The Rust implementation benefits from enums to model the execution mode exhaustively. When you match on `ExecutionMode::Plan` vs `ExecutionMode::Execute`, the compiler ensures you handle both cases. In Python, you might use a string or boolean flag, and it is easy to forget a branch, leading to actions accidentally executing in plan mode.
:::

## Integrating Plan Mode with the Agent Loop

The cleanest integration point is in the tool dispatcher. When the agent loop processes an LLM response containing tool calls, the dispatcher checks the execution mode:

```rust
/// A simplified tool dispatcher that respects execution mode.
struct ToolDispatcher {
    mode: ExecutionMode,
    plan: Vec<(String, String)>, // (tool_name, description)
}

#[derive(Debug, Clone, PartialEq)]
enum ExecutionMode {
    Plan,
    Execute,
}

impl ToolDispatcher {
    fn new(mode: ExecutionMode) -> Self {
        Self {
            mode,
            plan: Vec::new(),
        }
    }

    /// Dispatch a tool call, respecting the current execution mode.
    fn dispatch(&mut self, tool_name: &str, args: &str) -> String {
        match self.mode {
            ExecutionMode::Plan => {
                // Record the action without executing
                let description = format!("{}({})", tool_name, args);
                self.plan.push((tool_name.to_string(), description.clone()));
                format!("[PLAN MODE] Would execute: {}", description)
            }
            ExecutionMode::Execute => {
                // Actually execute the tool
                format!("[EXECUTED] {}({})", tool_name, args)
            }
        }
    }

    fn get_plan(&self) -> &[(String, String)] {
        &self.plan
    }
}

fn main() {
    // Simulate an agent session in plan mode
    let mut dispatcher = ToolDispatcher::new(ExecutionMode::Plan);

    let result1 = dispatcher.dispatch("read_file", "src/main.rs");
    let result2 = dispatcher.dispatch("write_file", "src/main.rs, <new content>");
    let result3 = dispatcher.dispatch("shell", "cargo test");

    println!("{}", result1);
    println!("{}", result2);
    println!("{}", result3);

    println!("\nCollected plan ({} actions):", dispatcher.get_plan().len());
    for (i, (tool, desc)) in dispatcher.get_plan().iter().enumerate() {
        println!("  {}. {} - {}", i + 1, tool, desc);
    }
}
```

## Key Takeaways

- Plan mode and execute mode should share the same reasoning logic -- the only difference is whether tool calls are recorded or executed
- Execution plans should include diff previews for file modifications, full command strings for shell commands, and risk annotations for every action
- Users should be able to approve plans granularly: approve all, approve selected actions, exclude specific actions, or reject everything
- Plan mode is the most user-friendly safety mechanism because it gives complete visibility into what the agent intends to do before anything happens
- Integrating plan mode at the tool dispatcher level keeps the implementation clean and ensures no tool can bypass plan-mode recording
