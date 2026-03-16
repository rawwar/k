---
title: Plugin Isolation
description: Ensuring that misbehaving plugins cannot crash the agent or interfere with other plugins through process isolation, resource limits, and fault boundaries.
---

# Plugin Isolation

> **What you'll learn:**
> - How to isolate plugin execution so panics and errors do not propagate to the core agent
> - Techniques for resource limiting (CPU, memory, time) to prevent runaway plugins
> - Patterns for fault boundaries that degrade gracefully when a plugin fails

Plugins run third-party code. Even well-intentioned plugins can have bugs -- infinite loops, memory leaks, panics, or unexpected interactions with other plugins. Without isolation, a single misbehaving plugin can crash the entire agent. This section covers techniques for containing failures so the agent stays stable even when plugins do not.

## The Problem: Shared Process Space

In the plugin architecture you built earlier, plugins run inside the same process as the agent. This means:

- A plugin panic (via `unwrap()` on a `None`) crashes the whole agent
- A plugin that allocates unbounded memory starves the agent
- A plugin that enters an infinite loop blocks the executor
- A plugin that corrupts shared state (through `unsafe` or logic bugs) causes unpredictable behavior

You cannot eliminate all risk, but you can contain it. The strategy is defense in depth: multiple layers of protection that each catch a different class of failure.

## Layer 1: Panic Catching with catch_unwind

Rust's `std::panic::catch_unwind` catches panics before they propagate. Wrap every plugin invocation in a panic boundary:

```rust
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Execute a plugin handler with panic protection.
pub async fn safe_plugin_call<F, Fut, T>(
    plugin_name: &str,
    f: F,
) -> Result<T, PluginError>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T, PluginError>> + Send + 'static,
    T: Send + 'static,
{
    // Spawn the handler in its own task for isolation
    let name = plugin_name.to_string();
    let handle = tokio::task::spawn(async move {
        // catch_unwind only catches synchronous panics.
        // For async code, we need to wrap the future.
        AssertUnwindSafe(f())
            .catch_unwind()
            .await
    });

    match handle.await {
        Ok(Ok(Ok(result))) => Ok(result),
        Ok(Ok(Err(plugin_err))) => Err(plugin_err),
        Ok(Err(panic_info)) => {
            let msg = if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "Unknown panic".to_string()
            };
            eprintln!(
                "[isolation] Plugin '{}' panicked: {}",
                plugin_name, msg
            );
            Err(PluginError::InitFailed(format!(
                "Plugin '{}' panicked: {}",
                plugin_name, msg
            )))
        }
        Err(join_err) => {
            eprintln!(
                "[isolation] Plugin '{}' task failed: {}",
                plugin_name, join_err
            );
            Err(PluginError::InitFailed(format!(
                "Plugin '{}' task error: {}",
                plugin_name, join_err
            )))
        }
    }
}
```

::: tip Coming from Python
Python has a similar mechanism with `try/except`:
```python
def safe_plugin_call(plugin_name: str, func, *args):
    try:
        return func(*args)
    except Exception as e:
        print(f"Plugin '{plugin_name}' failed: {e}")
        return None
```
The difference is that Python's `except Exception` catches virtually everything, while Rust's `catch_unwind` only catches panics -- not all errors. Rust forces you to handle `Result` errors separately through the type system. This is actually safer: regular errors flow through `Result<T, E>` with compile-time checking, and `catch_unwind` is a safety net for the truly unexpected.
:::

## Layer 2: Timeout Protection

A plugin that hangs should not block the agent forever. Wrap plugin calls in timeouts:

```rust
use std::time::Duration;

/// Execute a plugin operation with a timeout.
pub async fn with_timeout<T>(
    plugin_name: &str,
    timeout: Duration,
    future: impl std::future::Future<Output = Result<T, PluginError>> + Send,
) -> Result<T, PluginError> {
    match tokio::time::timeout(timeout, future).await {
        Ok(result) => result,
        Err(_) => {
            eprintln!(
                "[isolation] Plugin '{}' timed out after {:?}",
                plugin_name, timeout
            );
            Err(PluginError::InitFailed(format!(
                "Plugin '{}' exceeded timeout of {:?}",
                plugin_name, timeout
            )))
        }
    }
}

/// Configurable timeout tiers based on the operation type.
pub struct TimeoutPolicy {
    pub init_timeout: Duration,
    pub tool_call_timeout: Duration,
    pub hook_timeout: Duration,
    pub event_handler_timeout: Duration,
}

impl Default for TimeoutPolicy {
    fn default() -> Self {
        Self {
            init_timeout: Duration::from_secs(10),
            tool_call_timeout: Duration::from_secs(120),
            hook_timeout: Duration::from_secs(5),
            event_handler_timeout: Duration::from_secs(2),
        }
    }
}
```

## Layer 3: Resource Tracking

Track resource usage per plugin to detect misbehavior:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Track resource usage per plugin.
pub struct ResourceTracker {
    usage: Arc<Mutex<HashMap<String, PluginResourceUsage>>>,
    limits: ResourceLimits,
}

#[derive(Debug, Clone, Default)]
pub struct PluginResourceUsage {
    pub total_cpu_ms: u64,
    pub call_count: u64,
    pub error_count: u64,
    pub timeout_count: u64,
    pub last_call_duration_ms: u64,
    pub registered_tools: usize,
    pub registered_hooks: usize,
    pub registered_events: usize,
}

#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum tools a single plugin can register.
    pub max_tools_per_plugin: usize,
    /// Maximum hooks a single plugin can register.
    pub max_hooks_per_plugin: usize,
    /// Maximum event subscriptions per plugin.
    pub max_subscriptions_per_plugin: usize,
    /// Maximum consecutive errors before disabling a plugin.
    pub max_consecutive_errors: u64,
    /// Maximum total CPU time (ms) per conversation turn.
    pub max_cpu_ms_per_turn: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_tools_per_plugin: 50,
            max_hooks_per_plugin: 20,
            max_subscriptions_per_plugin: 30,
            max_consecutive_errors: 5,
            max_cpu_ms_per_turn: 10_000,
        }
    }
}

impl ResourceTracker {
    pub fn new(limits: ResourceLimits) -> Self {
        Self {
            usage: Arc::new(Mutex::new(HashMap::new())),
            limits,
        }
    }

    /// Record a plugin call and its duration.
    pub async fn record_call(
        &self,
        plugin_name: &str,
        duration_ms: u64,
        success: bool,
    ) {
        let mut usage = self.usage.lock().await;
        let entry = usage
            .entry(plugin_name.to_string())
            .or_insert_with(PluginResourceUsage::default);

        entry.call_count += 1;
        entry.total_cpu_ms += duration_ms;
        entry.last_call_duration_ms = duration_ms;

        if !success {
            entry.error_count += 1;
        }
    }

    /// Record a timeout for a plugin.
    pub async fn record_timeout(&self, plugin_name: &str) {
        let mut usage = self.usage.lock().await;
        let entry = usage
            .entry(plugin_name.to_string())
            .or_insert_with(PluginResourceUsage::default);

        entry.timeout_count += 1;
    }

    /// Check if a plugin has exceeded its error budget.
    pub async fn should_disable(&self, plugin_name: &str) -> bool {
        let usage = self.usage.lock().await;
        if let Some(entry) = usage.get(plugin_name) {
            entry.error_count >= self.limits.max_consecutive_errors
                || entry.timeout_count >= 3
        } else {
            false
        }
    }

    /// Check if a plugin can register more tools.
    pub async fn can_register_tool(&self, plugin_name: &str) -> bool {
        let usage = self.usage.lock().await;
        if let Some(entry) = usage.get(plugin_name) {
            entry.registered_tools < self.limits.max_tools_per_plugin
        } else {
            true
        }
    }

    /// Get a summary of all plugin resource usage.
    pub async fn summary(&self) -> HashMap<String, PluginResourceUsage> {
        let usage = self.usage.lock().await;
        usage.clone()
    }
}
```

## Layer 4: The Fault Boundary

The fault boundary wraps the entire plugin interaction, combining panic catching, timeouts, and resource tracking:

```rust
pub struct FaultBoundary {
    timeouts: TimeoutPolicy,
    tracker: ResourceTracker,
}

impl FaultBoundary {
    pub fn new(timeouts: TimeoutPolicy, limits: ResourceLimits) -> Self {
        Self {
            timeouts,
            tracker: ResourceTracker::new(limits),
        }
    }

    /// Execute a tool call through a plugin with full isolation.
    pub async fn execute_tool(
        &self,
        plugin_name: &str,
        tool_name: &str,
        handler: ToolHandler,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, PluginError> {
        // Check if this plugin has been disabled
        if self.tracker.should_disable(plugin_name).await {
            return Err(PluginError::InitFailed(format!(
                "Plugin '{}' has been disabled due to repeated failures",
                plugin_name
            )));
        }

        let start = std::time::Instant::now();

        // Execute with timeout and panic protection
        let result = with_timeout(
            plugin_name,
            self.timeouts.tool_call_timeout,
            async {
                match catch_unwind(AssertUnwindSafe(|| handler(params)))  {
                    Ok(future) => {
                        future.await.map_err(|e| PluginError::InitFailed(
                            format!("Tool '{}' error: {}", tool_name, e)
                        ))
                    }
                    Err(_) => Err(PluginError::InitFailed(
                        format!("Tool '{}' panicked", tool_name)
                    )),
                }
            },
        )
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        // Record the result for tracking
        match &result {
            Ok(_) => {
                self.tracker
                    .record_call(plugin_name, duration_ms, true)
                    .await;
            }
            Err(_) => {
                self.tracker
                    .record_call(plugin_name, duration_ms, false)
                    .await;
            }
        }

        result
    }

    /// Execute a hook through a plugin with isolation.
    pub async fn execute_hook(
        &self,
        plugin_name: &str,
        handler: HookHandler,
        context: HookContext,
    ) -> HookAction {
        let result = with_timeout(
            plugin_name,
            self.timeouts.hook_timeout,
            async {
                Ok(handler(context.clone()).await)
            },
        )
        .await;

        match result {
            Ok(action) => action,
            Err(_) => {
                // On failure, continue rather than blocking the agent
                HookAction::Continue(context)
            }
        }
    }

    /// Get a report of plugin health.
    pub async fn health_report(&self) -> Vec<PluginHealthStatus> {
        let usage = self.tracker.summary().await;

        usage
            .into_iter()
            .map(|(name, usage)| {
                let health = if usage.error_count == 0 && usage.timeout_count == 0 {
                    HealthLevel::Healthy
                } else if usage.error_count < 3 && usage.timeout_count < 2 {
                    HealthLevel::Degraded
                } else {
                    HealthLevel::Failing
                };

                PluginHealthStatus {
                    plugin_name: name,
                    health,
                    call_count: usage.call_count,
                    error_count: usage.error_count,
                    timeout_count: usage.timeout_count,
                    avg_latency_ms: if usage.call_count > 0 {
                        usage.total_cpu_ms / usage.call_count
                    } else {
                        0
                    },
                }
            })
            .collect()
    }
}

#[derive(Debug)]
pub struct PluginHealthStatus {
    pub plugin_name: String,
    pub health: HealthLevel,
    pub call_count: u64,
    pub error_count: u64,
    pub timeout_count: u64,
    pub avg_latency_ms: u64,
}

#[derive(Debug)]
pub enum HealthLevel {
    Healthy,
    Degraded,
    Failing,
}
```

## Layer 5: Process Isolation for Untrusted Plugins

For plugins you do not trust at all, run them in a separate process. This is the strongest isolation boundary -- a crash in the plugin process cannot affect the agent process:

```rust
use tokio::process::Command;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde_json::Value;

/// Run a plugin tool as a separate process.
pub async fn execute_in_subprocess(
    command: &str,
    args: &[String],
    input: Value,
    timeout: Duration,
) -> Result<Value, PluginError> {
    let mut child = Command::new(command)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| PluginError::InitFailed(
            format!("Failed to spawn plugin process: {}", e)
        ))?;

    // Send input as JSON via stdin
    if let Some(mut stdin) = child.stdin.take() {
        let input_json = serde_json::to_string(&input)
            .map_err(|e| PluginError::InitFailed(e.to_string()))?;
        stdin.write_all(input_json.as_bytes()).await
            .map_err(|e| PluginError::InitFailed(e.to_string()))?;
        drop(stdin); // Close stdin
    }

    // Wait for output with timeout
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| {
            // Kill the process on timeout
            PluginError::InitFailed("Plugin process timed out".to_string())
        })?
        .map_err(|e| PluginError::InitFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PluginError::InitFailed(format!(
            "Plugin process exited with {}: {}",
            output.status, stderr
        )));
    }

    // Parse output as JSON
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout)
        .map_err(|e| PluginError::InitFailed(
            format!("Invalid JSON output from plugin: {}", e)
        ))
}
```

This is exactly what MCP provides in a standardized way. The MCP protocol is essentially a structured version of this subprocess communication pattern. Use direct subprocess calls for simple plugins, and MCP for plugins that need the full discovery and lifecycle protocol.

::: info In the Wild
Claude Code achieves plugin isolation naturally through its architecture: built-in tools run in-process for performance, while hooks run as separate shell commands (subprocess isolation), and MCP servers run as separate processes with full protocol-level isolation. This tiered approach matches the trust level to the isolation level -- core code is trusted and runs fast in-process, while user-defined hooks and community MCP servers are sandboxed by process boundaries.
:::

## Key Takeaways

- Plugin isolation uses defense in depth: panic catching prevents crashes, timeouts prevent hangs, resource tracking detects misbehavior, and process isolation prevents memory corruption
- `catch_unwind` catches panics before they propagate to the agent, but it only catches panics -- `Result` errors are handled through the normal type system
- Resource tracking per plugin enables automatic disabling of plugins that exceed error budgets, preventing a failing plugin from degrading the entire agent
- The fault boundary combines all isolation layers into a single wrapper that the agent uses for every plugin interaction, keeping the isolation logic centralized
- Process isolation (subprocess or MCP) provides the strongest guarantees -- a crash in a separate process cannot affect the agent -- and is the right choice for untrusted third-party code
