---
title: Plan Mode
description: Implementing a plan-only mode where the agent describes intended changes without executing them, allowing human review of the full plan before any modifications occur.
---

# Plan Mode

> **What you'll learn:**
> - How to intercept tool calls and render them as planned actions instead of executing them
> - How to present a coherent plan summary that humans can review and approve or reject
> - Patterns for transitioning from plan mode to execution mode while preserving the plan

Plan mode is a safety feature that lets the user say "show me what you would do, but do not actually do it." The agent runs through its full reasoning process, generates tool calls, and presents them as a plan — but nothing touches the filesystem, nothing runs in the shell, nothing changes. The user reviews the plan and either approves it for execution or asks for modifications.

This is the ultimate defense against the confused deputy problem. Instead of approving each operation individually (which leads to prompt fatigue), the user sees the *entire* plan and can evaluate whether it matches their intent before any action is taken.

## The Plan Data Model

A plan is a list of intended actions, each corresponding to a tool call that the agent would normally execute:

```rust
use std::fmt;

/// A single action the agent intends to take.
#[derive(Debug, Clone)]
pub struct PlannedAction {
    /// Sequential index within the plan.
    pub index: usize,
    /// The tool that would be invoked.
    pub tool_name: String,
    /// Parameters that would be passed to the tool.
    pub parameters: Vec<(String, String)>,
    /// Human-readable description of what this action does.
    pub description: String,
    /// Risk level of the action.
    pub risk: ActionRisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionRisk {
    Safe,
    Moderate,
    High,
    Destructive,
}

impl fmt::Display for PlannedAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let risk_marker = match self.risk {
            ActionRisk::Safe => " ",
            ActionRisk::Moderate => "*",
            ActionRisk::High => "!",
            ActionRisk::Destructive => "X",
        };

        writeln!(f, "  [{}] {} {}", risk_marker, self.index + 1, self.description)?;
        writeln!(f, "      Tool: {}", self.tool_name)?;
        for (key, value) in &self.parameters {
            let display_val = if value.len() > 100 {
                format!("{}... ({} chars)", &value[..100], value.len())
            } else {
                value.clone()
            };
            writeln!(f, "      {}: {}", key, display_val)?;
        }
        Ok(())
    }
}

/// A complete plan comprising multiple actions.
#[derive(Debug, Clone)]
pub struct Plan {
    /// Description of the overall goal.
    pub goal: String,
    /// Ordered list of actions.
    pub actions: Vec<PlannedAction>,
}

impl Plan {
    pub fn new(goal: &str) -> Self {
        Self {
            goal: goal.to_string(),
            actions: Vec::new(),
        }
    }

    pub fn add_action(&mut self, action: PlannedAction) {
        self.actions.push(action);
    }

    /// Count actions by risk level.
    pub fn risk_summary(&self) -> (usize, usize, usize, usize) {
        let safe = self.actions.iter().filter(|a| a.risk == ActionRisk::Safe).count();
        let moderate = self.actions.iter().filter(|a| a.risk == ActionRisk::Moderate).count();
        let high = self.actions.iter().filter(|a| a.risk == ActionRisk::High).count();
        let destructive = self.actions.iter().filter(|a| a.risk == ActionRisk::Destructive).count();
        (safe, moderate, high, destructive)
    }
}

impl fmt::Display for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== Plan: {} ===\n", self.goal)?;

        if self.actions.is_empty() {
            writeln!(f, "  (no actions planned)")?;
            return Ok(());
        }

        for action in &self.actions {
            write!(f, "{}", action)?;
        }

        let (safe, moderate, high, destructive) = self.risk_summary();
        writeln!(f, "\nRisk summary: {} safe, {} moderate, {} high, {} destructive",
            safe, moderate, high, destructive)?;
        writeln!(f, "Legend: [ ] safe  [*] moderate  [!] high  [X] destructive")?;
        writeln!(f, "\n[e] execute plan  [m] modify  [c] cancel")
    }
}
```

## The Plan Mode Interceptor

The key mechanism is intercepting tool calls in the agent loop. When plan mode is active, tool calls are captured into the plan instead of being executed:

```rust
/// Tracks whether the agent is in plan mode or execution mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentMode {
    /// Normal execution: tool calls are executed immediately
    /// (subject to permission and approval checks).
    Execute,
    /// Plan mode: tool calls are captured into a plan.
    Plan,
    /// Executing a previously approved plan.
    ExecutingPlan(Plan),
}

/// Intercepts tool calls based on the current agent mode.
pub struct ModeInterceptor {
    mode: AgentMode,
    current_plan: Option<Plan>,
}

impl ModeInterceptor {
    pub fn new(mode: AgentMode) -> Self {
        let current_plan = if mode == AgentMode::Plan {
            Some(Plan::new("Pending plan"))
        } else {
            None
        };
        Self {
            mode,
            current_plan,
        }
    }

    /// Called when the LLM requests a tool call.
    /// Returns `Execute` with the tool name if in execute mode,
    /// or `Planned` with the captured action if in plan mode.
    pub fn intercept_tool_call(
        &mut self,
        tool_name: &str,
        parameters: Vec<(String, String)>,
        description: &str,
        risk: ActionRisk,
    ) -> InterceptResult {
        match &self.mode {
            AgentMode::Execute => {
                InterceptResult::Execute
            }
            AgentMode::Plan => {
                let index = self
                    .current_plan
                    .as_ref()
                    .map(|p| p.actions.len())
                    .unwrap_or(0);

                let action = PlannedAction {
                    index,
                    tool_name: tool_name.to_string(),
                    parameters,
                    description: description.to_string(),
                    risk,
                };

                if let Some(plan) = &mut self.current_plan {
                    plan.add_action(action.clone());
                }

                InterceptResult::Planned(action)
            }
            AgentMode::ExecutingPlan(_) => {
                // When executing a plan, tool calls proceed normally
                InterceptResult::Execute
            }
        }
    }

    /// Get the current plan (only available in plan mode).
    pub fn current_plan(&self) -> Option<&Plan> {
        self.current_plan.as_ref()
    }

    /// Finalize the plan and set its goal description.
    pub fn finalize_plan(&mut self, goal: &str) -> Option<Plan> {
        if let Some(plan) = &mut self.current_plan {
            plan.goal = goal.to_string();
        }
        self.current_plan.clone()
    }

    /// Transition from plan mode to executing the approved plan.
    pub fn approve_plan(&mut self) -> Option<Plan> {
        if let Some(plan) = self.current_plan.take() {
            self.mode = AgentMode::ExecutingPlan(plan.clone());
            Some(plan)
        } else {
            None
        }
    }

    /// Cancel the current plan and return to plan mode.
    pub fn cancel_plan(&mut self) {
        self.mode = AgentMode::Plan;
        self.current_plan = Some(Plan::new("Pending plan"));
    }

    /// Switch between modes.
    pub fn set_mode(&mut self, mode: AgentMode) {
        self.mode = mode;
        if mode == AgentMode::Plan {
            self.current_plan = Some(Plan::new("Pending plan"));
        }
    }

    pub fn current_mode(&self) -> &AgentMode {
        &self.mode
    }
}

/// Result of intercepting a tool call.
#[derive(Debug, Clone)]
pub enum InterceptResult {
    /// Proceed with execution.
    Execute,
    /// Tool call was captured into the plan.
    Planned(PlannedAction),
}
```

::: python Coming from Python
In Python, you might implement plan mode with a simple flag and a list:
```python
class Agent:
    def __init__(self):
        self.plan_mode = False
        self.plan = []

    def call_tool(self, tool_name, **params):
        if self.plan_mode:
            self.plan.append({"tool": tool_name, "params": params})
            return {"status": "planned"}
        else:
            return self.execute_tool(tool_name, **params)
```
The Rust version uses an enum for the mode instead of a boolean, which prevents invalid states like "executing a plan while in plan mode." The type system makes it impossible to be in two modes simultaneously.
:::

## Generating Simulated Tool Results

When the agent is in plan mode, the LLM still expects tool results so it can continue reasoning. You need to return *simulated* results that are plausible enough for the model to plan its next steps:

```rust
/// Generate a simulated tool result for plan mode.
/// The result should be plausible enough for the LLM to continue planning
/// without being mistaken for actual execution.
pub fn simulate_tool_result(tool_name: &str, parameters: &[(String, String)]) -> String {
    match tool_name {
        "read_file" => {
            let path = parameters
                .iter()
                .find(|(k, _)| k == "path")
                .map(|(_, v)| v.as_str())
                .unwrap_or("unknown");
            format!(
                "[PLAN MODE] Would read file: {}. \
                 Actual content not available in plan mode. \
                 Assume the file exists and contains the expected content.",
                path
            )
        }
        "write_file" => {
            let path = parameters
                .iter()
                .find(|(k, _)| k == "path")
                .map(|(_, v)| v.as_str())
                .unwrap_or("unknown");
            format!(
                "[PLAN MODE] Would write to file: {}. \
                 File not actually modified.",
                path
            )
        }
        "shell" => {
            let cmd = parameters
                .iter()
                .find(|(k, _)| k == "command")
                .map(|(_, v)| v.as_str())
                .unwrap_or("unknown");
            format!(
                "[PLAN MODE] Would execute: {}. \
                 Command not actually run. \
                 Assume it succeeds with expected output.",
                cmd
            )
        }
        _ => {
            format!(
                "[PLAN MODE] Would invoke tool '{}'. \
                 Not actually executed.",
                tool_name
            )
        }
    }
}
```

The "[PLAN MODE]" prefix is important — it prevents the LLM from confusing simulated results with real ones if the conversation history is ever replayed.

## Plan Execution

When the user approves a plan, you execute its actions in order. Each action goes through the normal permission and approval checks — plan approval does not bypass the safety system:

```rust
/// Execute an approved plan, running each action in sequence.
/// Each action still goes through permission and safety checks.
pub fn execute_plan(
    plan: &Plan,
    safety_filter: &SafetyFilter,
) -> Vec<PlanStepResult> {
    let mut results = Vec::new();

    for action in &plan.actions {
        // Safety check each action even though the plan was approved
        let safety_ok = match action.tool_name.as_str() {
            "shell" => {
                let cmd = action
                    .parameters
                    .iter()
                    .find(|(k, _)| k == "command")
                    .map(|(_, v)| v.as_str())
                    .unwrap_or("");
                safety_filter.check_command(cmd)
            }
            "write_file" | "read_file" => {
                let path_str = action
                    .parameters
                    .iter()
                    .find(|(k, _)| k == "path")
                    .map(|(_, v)| v.as_str())
                    .unwrap_or("");
                safety_filter.check_path(std::path::Path::new(path_str))
            }
            _ => FilterVerdict::Allowed,
        };

        match safety_ok {
            FilterVerdict::Allowed => {
                // In a real implementation, dispatch to the actual tool executor
                results.push(PlanStepResult {
                    action_index: action.index,
                    status: StepStatus::Success("Executed successfully".to_string()),
                });
            }
            FilterVerdict::Blocked(reason) => {
                results.push(PlanStepResult {
                    action_index: action.index,
                    status: StepStatus::Blocked(reason.clone()),
                });
                // Stop execution on first blocked action
                println!(
                    "Plan execution halted at step {}: {}",
                    action.index + 1,
                    reason
                );
                break;
            }
        }
    }

    results
}

#[derive(Debug)]
pub struct PlanStepResult {
    pub action_index: usize,
    pub status: StepStatus,
}

#[derive(Debug)]
pub enum StepStatus {
    Success(String),
    Blocked(String),
    Failed(String),
}

fn main() {
    let mut interceptor = ModeInterceptor::new(AgentMode::Plan);

    // Simulate the LLM generating tool calls in plan mode
    interceptor.intercept_tool_call(
        "read_file",
        vec![("path".to_string(), "src/main.rs".to_string())],
        "Read the main source file",
        ActionRisk::Safe,
    );

    interceptor.intercept_tool_call(
        "write_file",
        vec![
            ("path".to_string(), "src/main.rs".to_string()),
            ("content".to_string(), "fn main() { /* updated */ }".to_string()),
        ],
        "Update main.rs with new implementation",
        ActionRisk::Moderate,
    );

    interceptor.intercept_tool_call(
        "shell",
        vec![("command".to_string(), "cargo test".to_string())],
        "Run tests to verify changes",
        ActionRisk::Safe,
    );

    // Finalize and display the plan
    let plan = interceptor.finalize_plan("Refactor main.rs and verify with tests");

    if let Some(plan) = &plan {
        println!("{}", plan);
    }
}
```

::: wild In the Wild
Claude Code's plan mode (called "plan" permission level) allows the agent to read files but prevents any writes or shell execution. The agent describes what it would do in natural language, and the user can review the full plan before switching to a mode that allows execution. This is implemented by intercepting tool calls at the permission layer — the same infrastructure that handles Standard vs. FullAuto modes.
:::

## User Commands for Plan Mode

The user controls plan mode through REPL commands:

```rust
/// Parse plan mode commands from user input.
pub fn parse_plan_command(input: &str) -> Option<PlanCommand> {
    let trimmed = input.trim();

    match trimmed {
        "/plan" => Some(PlanCommand::EnterPlanMode),
        "/plan show" => Some(PlanCommand::ShowPlan),
        "/plan execute" | "/plan run" => Some(PlanCommand::ExecutePlan),
        "/plan cancel" => Some(PlanCommand::CancelPlan),
        "/execute" => Some(PlanCommand::ExitPlanMode),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub enum PlanCommand {
    /// Switch to plan mode.
    EnterPlanMode,
    /// Show the current plan.
    ShowPlan,
    /// Approve and execute the current plan.
    ExecutePlan,
    /// Discard the current plan.
    CancelPlan,
    /// Switch back to execution mode.
    ExitPlanMode,
}
```

## Key Takeaways

- Plan mode intercepts tool calls and captures them as planned actions instead of executing them, giving the user a complete preview of what the agent intends to do.
- Simulated tool results (prefixed with "[PLAN MODE]") let the LLM continue reasoning about multi-step plans without actually modifying anything.
- Even when executing an approved plan, each action still goes through the normal safety checks — plan approval does not bypass the permission system or allowlists.
- Using an enum for `AgentMode` (Execute, Plan, ExecutingPlan) prevents invalid state combinations that a boolean `plan_mode` flag would allow.
- Plan mode is the strongest defense against the confused deputy problem because the user sees the full intent before any action is taken.
