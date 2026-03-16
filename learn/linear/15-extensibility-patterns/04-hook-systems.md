---
title: Hook Systems
description: Design a hook system that lets extensions intercept, modify, or replace agent behavior at well-defined extension points.
---

# Hook Systems

> **What you'll learn:**
> - How hooks differ from events -- hooks allow modification of data flowing through the agent while events are purely observational
> - How to implement pre/post hooks for key agent operations (tool execution, message sending, file writes) with priority ordering
> - Techniques for designing hook APIs that are powerful enough for real extensions but constrained enough to prevent plugins from breaking the agent

The event bus you built in the previous subchapter lets plugins observe what the agent is doing. But observation is not enough for many real extensions. A content filter needs to modify messages before they reach the LLM. A security plugin needs to block dangerous tool invocations. A rate limiter needs to delay requests. These use cases require hooks: extension points where plugins can intercept and modify the data flowing through the agent.

The key distinction is simple. Events are notifications: "this happened." Hooks are interceptions: "this is about to happen -- do you want to change it?"

## Events vs. Hooks

| Aspect | Events | Hooks |
|--------|--------|-------|
| Direction | Outward (notify observers) | Inline (intercept data flow) |
| Modification | Read-only | Can modify, replace, or cancel |
| Timing | After the fact | Before and after the action |
| Failure impact | Handler errors are logged and ignored | Hook errors can block the action |
| Use case | Logging, metrics, UI updates | Content filtering, security, rate limiting |

## The Hook Point Abstraction

A hook point is a well-defined location in the agent's execution where plugins can intercept data. Let's define the core types:

```rust
use std::sync::Arc;
use async_trait::async_trait;

/// The result of a hook handler invocation.
/// This controls whether the operation proceeds.
#[derive(Debug)]
pub enum HookAction<T> {
    /// Continue with potentially modified data.
    Continue(T),
    /// Skip this operation entirely, providing a reason.
    Skip { reason: String },
    /// Replace the operation's result without executing it.
    Replace { result: T, reason: String },
}

/// A hook handler that can intercept and modify operations.
#[async_trait]
pub trait HookHandler<T: Send + Sync>: Send + Sync {
    /// Called before the operation executes.
    /// The handler receives the operation's input and can modify it,
    /// skip the operation, or let it proceed unchanged.
    async fn pre_hook(&self, data: T) -> Result<HookAction<T>, anyhow::Error> {
        Ok(HookAction::Continue(data))
    }

    /// Called after the operation executes.
    /// The handler receives the operation's result and can modify it.
    async fn post_hook(&self, data: T) -> Result<T, anyhow::Error> {
        Ok(data)
    }

    /// Priority determines execution order (lower numbers run first).
    fn priority(&self) -> i32 { 0 }

    /// A human-readable name for debugging and logging.
    fn name(&self) -> &str;
}
```

## Defining Hook Points for an Agent

Your agent has a few critical paths where hooks are most valuable. Let's define the data types that flow through each hook point:

```rust
/// Data passed through the tool execution hook.
#[derive(Debug, Clone)]
pub struct ToolExecutionHookData {
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub invocation_id: String,
}

/// Data passed through the tool result hook.
#[derive(Debug, Clone)]
pub struct ToolResultHookData {
    pub tool_name: String,
    pub invocation_id: String,
    pub result: String,
    pub was_error: bool,
}

/// Data passed through the message hook (before sending to LLM).
#[derive(Debug, Clone)]
pub struct MessageHookData {
    pub role: String,
    pub content: String,
    pub metadata: serde_json::Value,
}

/// Data passed through the file write hook.
#[derive(Debug, Clone)]
pub struct FileWriteHookData {
    pub path: String,
    pub content: String,
    pub is_new_file: bool,
}
```

## The Hook Registry

The hook registry manages handlers for each hook point and runs them in priority order:

```rust
use std::collections::HashMap;
use tokio::sync::RwLock;

pub struct HookRegistry {
    tool_execution_hooks: RwLock<Vec<Arc<dyn HookHandler<ToolExecutionHookData>>>>,
    tool_result_hooks: RwLock<Vec<Arc<dyn HookHandler<ToolResultHookData>>>>,
    message_hooks: RwLock<Vec<Arc<dyn HookHandler<MessageHookData>>>>,
    file_write_hooks: RwLock<Vec<Arc<dyn HookHandler<FileWriteHookData>>>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self {
            tool_execution_hooks: RwLock::new(Vec::new()),
            tool_result_hooks: RwLock::new(Vec::new()),
            message_hooks: RwLock::new(Vec::new()),
            file_write_hooks: RwLock::new(Vec::new()),
        }
    }

    pub async fn register_tool_execution_hook(
        &self,
        handler: Arc<dyn HookHandler<ToolExecutionHookData>>,
    ) {
        let mut hooks = self.tool_execution_hooks.write().await;
        hooks.push(handler);
        hooks.sort_by_key(|h| h.priority());
    }

    /// Run all pre-hooks for tool execution.
    /// Returns the (potentially modified) data or a skip/replace action.
    pub async fn run_pre_tool_execution(
        &self,
        mut data: ToolExecutionHookData,
    ) -> Result<HookAction<ToolExecutionHookData>, anyhow::Error> {
        let hooks = self.tool_execution_hooks.read().await;
        for hook in hooks.iter() {
            match hook.pre_hook(data).await? {
                HookAction::Continue(modified) => {
                    data = modified;
                }
                action @ HookAction::Skip { .. } => {
                    return Ok(action);
                }
                action @ HookAction::Replace { .. } => {
                    return Ok(action);
                }
            }
        }
        Ok(HookAction::Continue(data))
    }

    /// Run all post-hooks for tool results.
    pub async fn run_post_tool_result(
        &self,
        mut data: ToolResultHookData,
    ) -> Result<ToolResultHookData, anyhow::Error> {
        let hooks = self.tool_result_hooks.read().await;
        for hook in hooks.iter() {
            data = hook.post_hook(data).await?;
        }
        Ok(data)
    }
}
```

## Example: A Security Hook

Let's implement a practical hook that blocks dangerous shell commands. This is the kind of hook that production agents use to prevent the LLM from running destructive operations:

```rust
/// A security hook that blocks dangerous shell commands.
pub struct DangerousCommandBlocker {
    blocked_patterns: Vec<String>,
}

impl DangerousCommandBlocker {
    pub fn new() -> Self {
        Self {
            blocked_patterns: vec![
                "rm -rf /".to_string(),
                "rm -rf ~".to_string(),
                "mkfs".to_string(),
                "dd if=".to_string(),
                "> /dev/sda".to_string(),
                "chmod -R 777 /".to_string(),
            ],
        }
    }
}

#[async_trait]
impl HookHandler<ToolExecutionHookData> for DangerousCommandBlocker {
    async fn pre_hook(
        &self,
        data: ToolExecutionHookData,
    ) -> Result<HookAction<ToolExecutionHookData>, anyhow::Error> {
        if data.tool_name == "shell" {
            if let Some(command) = data.arguments.get("command").and_then(|c| c.as_str()) {
                for pattern in &self.blocked_patterns {
                    if command.contains(pattern) {
                        return Ok(HookAction::Skip {
                            reason: format!(
                                "Blocked dangerous command containing '{pattern}'"
                            ),
                        });
                    }
                }
            }
        }
        Ok(HookAction::Continue(data))
    }

    fn priority(&self) -> i32 {
        -100 // Run before everything else
    }

    fn name(&self) -> &str {
        "dangerous_command_blocker"
    }
}
```

::: wild In the Wild
Claude Code implements a hook system for its "hooks" feature, which lets users define pre and post hooks for tool execution in their project configuration. For example, you can define a hook that runs a linter after every file write, or a hook that asks for confirmation before shell commands matching certain patterns. The hooks are defined in `.claude/hooks.json` and execute as shell commands, giving users extensibility without writing Rust code. This config-driven approach complements the programmatic hook API we are building here.
:::

## Example: A Content Sanitization Hook

Here is a post-hook that sanitizes tool results before they are sent back to the LLM, redacting sensitive information:

```rust
/// Redacts sensitive patterns from tool output before the LLM sees it.
pub struct SensitiveDataRedactor {
    patterns: Vec<(regex::Regex, &'static str)>,
}

impl SensitiveDataRedactor {
    pub fn new() -> Result<Self, regex::Error> {
        Ok(Self {
            patterns: vec![
                (
                    regex::Regex::new(r"(?i)(api[_-]?key|secret|token|password)\s*[:=]\s*\S+")?,
                    "[REDACTED_CREDENTIAL]",
                ),
                (
                    regex::Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b")?,
                    "[REDACTED_EMAIL]",
                ),
            ],
        })
    }
}

#[async_trait]
impl HookHandler<ToolResultHookData> for SensitiveDataRedactor {
    async fn post_hook(
        &self,
        mut data: ToolResultHookData,
    ) -> Result<ToolResultHookData, anyhow::Error> {
        for (pattern, replacement) in &self.patterns {
            data.result = pattern.replace_all(&data.result, *replacement).to_string();
        }
        Ok(data)
    }

    fn priority(&self) -> i32 {
        100 // Run after other hooks so we redact final output
    }

    fn name(&self) -> &str {
        "sensitive_data_redactor"
    }
}
```

## Integrating Hooks into the Agent Core

The agent core calls hooks at the appropriate points. Here is how tool execution integrates with the hook system:

```rust
pub async fn execute_tool(
    tool_registry: &ToolRegistry,
    hook_registry: &HookRegistry,
    event_bus: &EventBus,
    tool_name: &str,
    arguments: serde_json::Value,
) -> Result<String> {
    let invocation_id = uuid::Uuid::new_v4().to_string();

    // Build the hook data
    let hook_data = ToolExecutionHookData {
        tool_name: tool_name.to_string(),
        arguments: arguments.clone(),
        invocation_id: invocation_id.clone(),
    };

    // Run pre-hooks
    let hook_data = match hook_registry.run_pre_tool_execution(hook_data).await? {
        HookAction::Continue(data) => data,
        HookAction::Skip { reason } => {
            return Err(anyhow::anyhow!("Tool execution blocked: {reason}"));
        }
        HookAction::Replace { result, reason } => {
            // A hook provided a replacement result. Return it directly.
            let _ = reason;
            return Ok(result.arguments.to_string());
        }
    };

    // Emit the "started" event
    event_bus.emit(&AgentEvent::ToolInvocationStarted {
        tool_name: hook_data.tool_name.clone(),
        args: hook_data.arguments.clone(),
        invocation_id: invocation_id.clone(),
    }).await;

    // Execute the actual tool (using potentially modified arguments)
    let start = std::time::Instant::now();
    let result = tool_registry
        .execute(&hook_data.tool_name, hook_data.arguments)
        .await;
    let duration = start.elapsed();

    // Build post-hook data
    let result_data = ToolResultHookData {
        tool_name: tool_name.to_string(),
        invocation_id: invocation_id.clone(),
        result: result.as_ref().map(|r| r.clone()).unwrap_or_default(),
        was_error: result.is_err(),
    };

    // Run post-hooks (may modify the result)
    let result_data = hook_registry.run_post_tool_result(result_data).await?;

    // Emit the "completed" event
    event_bus.emit(&AgentEvent::ToolInvocationCompleted {
        tool_name: tool_name.to_string(),
        invocation_id,
        result: Ok(result_data.result.clone()),
        duration,
    }).await;

    Ok(result_data.result)
}
```

::: python Coming from Python
Python hook systems often use decorators:
```python
@hooks.before("tool.execute")
def check_dangerous(tool_name, args):
    if tool_name == "shell" and "rm -rf" in args["command"]:
        raise HookVeto("Blocked dangerous command")

@hooks.after("tool.execute")
def redact_secrets(tool_name, result):
    return re.sub(r'api_key=\S+', 'api_key=[REDACTED]', result)
```
The Rust approach uses traits and structs instead of decorators, which gives you stronger typing (the compiler verifies the hook data types) and explicit priority ordering. The tradeoff is more boilerplate, but the type safety prevents the class of bug where a hook handler assumes the wrong data shape.
:::

## Design Principles for Hook APIs

When deciding where to add hook points, follow these guidelines:

1. **Hook at boundaries, not internals.** Hook the interface between the agent and external systems (LLM calls, tool execution, file I/O), not internal implementation details. Internal hooks create brittle APIs that break with refactors.

2. **Make hook data cloneable.** Hooks need to pass data through a chain of handlers. If the data is not `Clone`, you cannot recover from a handler failure.

3. **Set timeouts on hooks.** A misbehaving hook handler should not stall the entire agent. Give each handler a deadline and skip it if it times out.

4. **Log every skip and replace.** When a hook cancels or replaces an operation, log the reason clearly. Silent modifications make debugging impossible.

5. **Limit the number of hook points.** Each hook point is an API commitment. Start with 3-5 critical hook points and add more based on actual extension needs.

## Key Takeaways

- **Hooks intercept and modify** data flowing through the agent, while events merely notify observers -- this distinction determines when to use each pattern.
- The **pre/post hook pattern** with a `HookAction` enum (Continue, Skip, Replace) gives handlers fine-grained control over whether and how an operation proceeds.
- **Priority ordering** ensures security hooks run before everything else and sanitization hooks run last -- the order matters for correctness.
- Hooks integrate into the agent core at **boundary points** (tool execution, LLM calls, file I/O) where extension behavior is most valuable and least likely to break internals.
- Production agents like Claude Code combine **programmatic hooks** (for plugin authors) with **config-driven hooks** (for users who define shell commands in configuration files).
