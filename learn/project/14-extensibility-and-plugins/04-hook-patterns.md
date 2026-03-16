---
title: Hook Patterns
description: Implementing pre and post execution hooks that let plugins intercept, modify, or veto tool calls and message handling at defined interception points.
---

# Hook Patterns

> **What you'll learn:**
> - How hooks differ from events and when to use each pattern for extensibility
> - How to implement before/after hooks that can modify tool inputs and outputs
> - Patterns for hook ordering, short-circuiting, and error handling in hook chains

Events tell plugins what happened. Hooks let plugins change what happens. While events are fire-and-forget notifications, hooks are synchronous interception points where a plugin can inspect, modify, or even veto an operation before it proceeds. This distinction is fundamental to building a flexible extension system.

## Events vs. Hooks

Before writing code, let's be precise about the difference:

| Aspect | Events | Hooks |
|--------|--------|-------|
| **Direction** | Agent notifies plugins | Plugin modifies agent behavior |
| **Timing** | After the fact (or fire-and-forget) | Before and after, blocking |
| **Return value** | None (handlers return `()`) | Modified data or a veto signal |
| **Failure mode** | Handler errors are logged, agent continues | Hook errors can abort the operation |
| **Concurrency** | Handlers run in parallel | Hooks run sequentially in priority order |

Use events when plugins just need to observe. Use hooks when plugins need to participate in the decision.

## Defining Hook Points

Identify the places in your agent where external code should be able to intervene:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The lifecycle points where hooks can intercept behavior.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookPoint {
    /// Before a tool call is executed. Can modify arguments or veto the call.
    PreToolUse,
    /// After a tool call completes. Can modify the result.
    PostToolUse,
    /// Before the user's message is sent to the LLM. Can modify or filter it.
    PreMessage,
    /// After the LLM's response is received. Can modify it before display.
    PostMessage,
    /// Before a slash command is dispatched. Can redirect or block it.
    PreCommand,
    /// Before the session ends. Can perform cleanup or save state.
    PreSessionEnd,
    /// When a notification would be shown to the user. Can suppress or modify it.
    Notification,
}

/// The data flowing through a hook chain. Each hook point has its own context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookContext {
    pub hook_point: HookPoint,
    pub tool_name: Option<String>,
    pub tool_input: Option<Value>,
    pub tool_output: Option<Value>,
    pub message: Option<String>,
    pub metadata: Value,
}

/// What a hook handler returns to control the pipeline.
#[derive(Debug)]
pub enum HookAction {
    /// Continue with potentially modified context.
    Continue(HookContext),
    /// Skip the operation entirely, using this as the result.
    Skip(String),
    /// Abort with an error.
    Abort(String),
}
```

The `HookAction` enum is the key design element. `Continue` passes (possibly modified) data to the next hook in the chain. `Skip` short-circuits the entire operation -- useful for caching or blocking. `Abort` stops execution with an error.

::: python Coming from Python
Python frameworks often implement hooks as middleware chains. In Django, for example:
```python
class SecurityMiddleware:
    def __init__(self, get_response):
        self.get_response = get_response

    def __call__(self, request):
        # Pre-processing (like PreToolUse)
        if self.is_dangerous(request):
            return HttpResponseForbidden("Blocked")

        response = self.get_response(request)  # Next in chain

        # Post-processing (like PostToolUse)
        response["X-Security-Check"] = "passed"
        return response
```
Rust's hook system works the same way, but uses explicit `HookAction` returns instead of implicitly calling the next middleware. This makes the control flow visible -- you can see exactly where a hook continues, skips, or aborts.
:::

## Building the Hook Registry

The hook registry stores ordered lists of handlers for each hook point:

```rust
use std::sync::Arc;
use std::pin::Pin;
use std::future::Future;
use std::collections::HashMap;

/// A hook handler function.
pub type HookHandler = Arc<
    dyn Fn(HookContext) -> Pin<Box<dyn Future<Output = HookAction> + Send>>
        + Send
        + Sync,
>;

/// A registered hook with its priority and owner.
struct RegisteredHook {
    id: u64,
    owner: String,
    priority: i32, // Lower number = runs first
    handler: HookHandler,
}

pub struct HookRegistry {
    hooks: HashMap<HookPoint, Vec<RegisteredHook>>,
    next_id: u64,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self {
            hooks: HashMap::new(),
            next_id: 0,
        }
    }

    /// Register a hook handler at a specific hook point with a priority.
    pub fn register(
        &mut self,
        owner: &str,
        point: HookPoint,
        priority: i32,
        handler: HookHandler,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let hooks = self.hooks.entry(point.clone()).or_insert_with(Vec::new);
        hooks.push(RegisteredHook {
            id,
            owner: owner.to_string(),
            priority,
            handler,
        });

        // Keep hooks sorted by priority (lowest first)
        hooks.sort_by_key(|h| h.priority);

        println!(
            "[hooks] Registered hook at {:?} by '{}' (priority={}, id={})",
            point, owner, priority, id
        );
        id
    }

    /// Remove a specific hook by ID.
    pub fn deregister(&mut self, hook_id: u64) {
        for hooks in self.hooks.values_mut() {
            hooks.retain(|h| h.id != hook_id);
        }
    }

    /// Remove all hooks registered by a specific plugin.
    pub fn deregister_all_by_owner(&mut self, owner: &str) {
        for hooks in self.hooks.values_mut() {
            hooks.retain(|h| h.owner != owner);
        }
    }

    /// Execute the hook chain for a given hook point.
    /// Returns the final context after all hooks have processed it,
    /// or a Skip/Abort action if any hook short-circuits.
    pub async fn execute(
        &self,
        point: &HookPoint,
        mut context: HookContext,
    ) -> HookAction {
        let hooks = match self.hooks.get(point) {
            Some(hooks) => hooks,
            None => return HookAction::Continue(context),
        };

        for hook in hooks {
            match (hook.handler)(context.clone()).await {
                HookAction::Continue(modified_ctx) => {
                    // Pass modified context to the next hook
                    context = modified_ctx;
                }
                HookAction::Skip(reason) => {
                    println!(
                        "[hooks] Hook '{}' (owner='{}') skipped at {:?}: {}",
                        hook.id, hook.owner, point, reason
                    );
                    return HookAction::Skip(reason);
                }
                HookAction::Abort(error) => {
                    println!(
                        "[hooks] Hook '{}' (owner='{}') aborted at {:?}: {}",
                        hook.id, hook.owner, point, error
                    );
                    return HookAction::Abort(error);
                }
            }
        }

        HookAction::Continue(context)
    }
}
```

The `execute` method is the pipeline. It iterates through hooks in priority order, threading the modified context through each one. Any hook can short-circuit with `Skip` or `Abort`.

## Integrating Hooks into the Agent

Now modify the agent's tool execution to pass through the hook chain:

```rust
impl Agent {
    pub async fn execute_tool_with_hooks(
        &self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<Value> {
        // Build the pre-hook context
        let pre_context = HookContext {
            hook_point: HookPoint::PreToolUse,
            tool_name: Some(tool_name.to_string()),
            tool_input: Some(arguments.clone()),
            tool_output: None,
            message: None,
            metadata: serde_json::json!({}),
        };

        // Run pre-tool hooks
        let hook_registry = self.hook_registry.read().await;
        let pre_result = hook_registry
            .execute(&HookPoint::PreToolUse, pre_context)
            .await;

        let final_args = match pre_result {
            HookAction::Continue(ctx) => {
                // Hooks may have modified the arguments
                ctx.tool_input.unwrap_or(arguments)
            }
            HookAction::Skip(reason) => {
                // A hook vetoed this tool call
                return Ok(serde_json::json!({
                    "skipped": true,
                    "reason": reason
                }));
            }
            HookAction::Abort(error) => {
                return Err(anyhow::anyhow!("Hook aborted tool call: {}", error));
            }
        };

        // Execute the actual tool
        let tool_registry = self.tool_registry.read().await;
        let result = tool_registry.invoke(tool_name, final_args).await?;

        // Build the post-hook context
        let post_context = HookContext {
            hook_point: HookPoint::PostToolUse,
            tool_name: Some(tool_name.to_string()),
            tool_input: None,
            tool_output: Some(result.clone()),
            message: None,
            metadata: serde_json::json!({}),
        };

        // Run post-tool hooks
        let post_result = hook_registry
            .execute(&HookPoint::PostToolUse, post_context)
            .await;

        match post_result {
            HookAction::Continue(ctx) => {
                Ok(ctx.tool_output.unwrap_or(result))
            }
            HookAction::Skip(reason) => {
                Ok(serde_json::json!({
                    "modified": true,
                    "reason": reason
                }))
            }
            HookAction::Abort(error) => {
                Err(anyhow::anyhow!("Post-hook aborted: {}", error))
            }
        }
    }
}
```

## Practical Hook Examples

### A Security Hook That Blocks Dangerous Commands

```rust
fn create_security_hook() -> HookHandler {
    Arc::new(|context: HookContext| {
        Box::pin(async move {
            // Only check shell tool calls
            if context.tool_name.as_deref() != Some("shell") {
                return HookAction::Continue(context);
            }

            let blocked_patterns = ["rm -rf /", "mkfs", "dd if=", ":(){ :|:& };:"];

            if let Some(input) = &context.tool_input {
                if let Some(command) = input.get("command").and_then(|c| c.as_str()) {
                    for pattern in &blocked_patterns {
                        if command.contains(pattern) {
                            return HookAction::Abort(format!(
                                "Blocked dangerous command pattern: '{}'",
                                pattern
                            ));
                        }
                    }
                }
            }

            HookAction::Continue(context)
        })
    })
}

// Register it during plugin activation:
// hook_registry.register("security", HookPoint::PreToolUse, 0, create_security_hook());
// Priority 0 ensures this runs before any other pre-tool hooks.
```

### An Audit Hook That Logs All Tool Calls

```rust
fn create_audit_hook(log_path: String) -> HookHandler {
    Arc::new(move |context: HookContext| {
        let log_path = log_path.clone();
        Box::pin(async move {
            if let (Some(tool), Some(input)) = (&context.tool_name, &context.tool_input) {
                let entry = serde_json::json!({
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "tool": tool,
                    "input": input,
                });

                // Append to audit log -- fire and forget
                if let Ok(mut file) = tokio::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_path)
                    .await
                {
                    use tokio::io::AsyncWriteExt;
                    let line = format!("{}\n", entry);
                    let _ = file.write_all(line.as_bytes()).await;
                }
            }

            // Always continue -- audit hooks should never block operations
            HookAction::Continue(context)
        })
    })
}
```

### A Caching Hook That Skips Redundant Tool Calls

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

fn create_cache_hook(
    cache: Arc<Mutex<HashMap<String, Value>>>,
) -> HookHandler {
    Arc::new(move |context: HookContext| {
        let cache = cache.clone();
        Box::pin(async move {
            if let (Some(tool), Some(input)) = (&context.tool_name, &context.tool_input) {
                let cache_key = format!("{}:{}", tool, input);
                let cache = cache.lock().await;

                if let Some(cached_result) = cache.get(&cache_key) {
                    // Return the cached result instead of calling the tool
                    return HookAction::Skip(
                        serde_json::to_string(cached_result)
                            .unwrap_or_default(),
                    );
                }
            }

            HookAction::Continue(context)
        })
    })
}
```

::: wild In the Wild
Claude Code defines hooks in settings files with a straightforward structure:
```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "shell",
        "command": "python3 /path/to/security_check.py"
      }
    ]
  }
}
```
Each hook has a `matcher` (which tools it applies to) and a `command` (the shell command to run). The exit code determines the action: 0 means continue, non-zero means block. This file-based approach makes hooks accessible to anyone who can write a shell script, without needing to write a Rust plugin. The input context is passed to the hook command via stdin, and the hook can write modified context to stdout.
:::

## Hook Priority Best Practices

When multiple plugins register hooks at the same point, priority determines execution order. Establish conventions:

| Priority Range | Purpose | Example |
|---------------|---------|---------|
| 0-99 | Security and safety checks | Block dangerous commands |
| 100-199 | Input transformation | Expand aliases, add defaults |
| 200-299 | Caching and optimization | Return cached results |
| 300-399 | Auditing and logging | Write to audit log |
| 400-499 | Analytics and metrics | Track usage statistics |

Security hooks run first because they should veto before any other processing occurs. Audit hooks run last so they see the final form of the data. Document these conventions so plugin authors know where their hooks should sit.

## Key Takeaways

- Hooks differ from events in a fundamental way: events are fire-and-forget notifications, while hooks are synchronous interception points that can modify or veto operations
- The `HookAction` enum (`Continue`, `Skip`, `Abort`) gives hook handlers explicit control over the pipeline -- they can pass through, short-circuit, or halt with an error
- Hooks execute in priority order, with lower numbers running first, so security checks (priority 0) always run before caching (priority 200) or logging (priority 300)
- The `PreToolUse` hook point is the most powerful -- it can modify tool arguments, block dangerous operations, or return cached results before the tool even runs
- Production agents like Claude Code use a file-based hook configuration that maps hook points to shell commands, making hooks accessible without compiled plugins
