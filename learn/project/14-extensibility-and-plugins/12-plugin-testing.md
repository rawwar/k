---
title: Plugin Testing
description: Building a test harness for plugin developers that provides mock agent context, simulated tool calls, and assertion helpers for verifying plugin behavior.
---

# Plugin Testing

> **What you'll learn:**
> - How to build a plugin test harness that simulates the agent environment for isolated testing
> - Techniques for testing event handlers and hooks with synthetic event sequences
> - How to provide assertion helpers and test utilities that make plugin testing ergonomic

Plugin developers need to test their plugins without running the full agent. A good test harness provides a mock environment that simulates the agent's registries, event bus, and hook chain, letting developers write fast, deterministic tests. In this section, you build that harness and establish patterns for testing every type of plugin interaction.

## The Test Harness

The test harness creates a self-contained plugin environment with mock versions of all the registries:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A test environment for plugin development.
pub struct PluginTestHarness {
    pub tool_registry: Arc<RwLock<ToolRegistry>>,
    pub event_bus: Arc<EventBus>,
    pub hook_registry: Arc<RwLock<HookRegistry>>,
    pub command_registry: Arc<RwLock<CommandRegistry>>,
    pub context: PluginContext,
    /// Captured events for assertion.
    captured_events: Arc<RwLock<Vec<AgentEvent>>>,
}

impl PluginTestHarness {
    /// Create a fresh test environment.
    pub fn new() -> Self {
        let tool_registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let event_bus = Arc::new(EventBus::new(256));
        let hook_registry = Arc::new(RwLock::new(HookRegistry::new()));
        let command_registry = Arc::new(RwLock::new(CommandRegistry::new()));

        let context = PluginContext::new(
            tool_registry.clone(),
            event_bus.clone(),
            hook_registry.clone(),
            command_registry.clone(),
        );

        let captured_events = Arc::new(RwLock::new(Vec::new()));

        // Set up event capture for assertions
        let capture = captured_events.clone();
        let bus = event_bus.clone();
        tokio::spawn(async move {
            let mut rx = bus.receiver();
            while let Ok(event) = rx.recv().await {
                let mut captured = capture.write().await;
                captured.push(event);
            }
        });

        Self {
            tool_registry,
            event_bus,
            hook_registry,
            command_registry,
            context,
            captured_events,
        }
    }

    /// Initialize and activate a plugin under test.
    pub async fn load_plugin(
        &mut self,
        mut plugin: Box<dyn Plugin>,
    ) -> Result<(), PluginError> {
        plugin.initialize(&mut self.context).await?;
        plugin.activate(&mut self.context).await?;
        Ok(())
    }

    /// Get the tool registry for inspection.
    pub async fn tools(&self) -> Vec<ToolDefinition> {
        let registry = self.tool_registry.read().await;
        registry.list_definitions()
    }

    /// Invoke a tool by name and return the result.
    pub async fn call_tool(
        &self,
        name: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, ToolError> {
        let registry = self.tool_registry.read().await;
        registry.invoke(name, params).await
    }

    /// Emit an event and wait for all handlers to process it.
    pub async fn emit_event(&self, event: AgentEvent) {
        self.event_bus.emit_and_wait(event).await;
        // Give async handlers a moment to complete
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    /// Run a hook chain and return the action.
    pub async fn run_hook(
        &self,
        point: &HookPoint,
        context: HookContext,
    ) -> HookAction {
        let registry = self.hook_registry.read().await;
        registry.execute(point, context).await
    }

    /// Dispatch a slash command and return the result.
    pub async fn dispatch_command(
        &self,
        input: &str,
    ) -> Result<CommandResult, CommandError> {
        let registry = self.command_registry.read().await;
        let context = CommandContext {
            session_id: "test-session".to_string(),
            working_directory: "/tmp/test".to_string(),
            active_skills: vec![],
            tool_count: 0,
            conversation_turn_count: 0,
        };
        registry.dispatch(input, &context).await
    }

    /// Get all events that have been emitted during the test.
    pub async fn captured_events(&self) -> Vec<AgentEvent> {
        let events = self.captured_events.read().await;
        events.clone()
    }

    /// Assert that a specific event type was emitted.
    pub async fn assert_event_emitted(&self, event_type: &str) {
        let events = self.captured_events.read().await;
        let found = events.iter().any(|e| e.event_type() == event_type);
        assert!(
            found,
            "Expected event '{}' to be emitted, but it was not. \
             Emitted events: {:?}",
            event_type,
            events.iter().map(|e| e.event_type()).collect::<Vec<_>>()
        );
    }

    /// Clear all captured events (useful between test phases).
    pub async fn clear_events(&self) {
        let mut events = self.captured_events.write().await;
        events.clear();
    }
}
```

::: tip Coming from Python
Python testing with pytest often uses fixtures to set up mock environments:
```python
import pytest

@pytest.fixture
def plugin_harness():
    harness = PluginTestHarness()
    yield harness
    harness.cleanup()

def test_word_count_plugin(plugin_harness):
    plugin = WordCountPlugin()
    plugin_harness.load_plugin(plugin)

    result = plugin_harness.call_tool("word_count", {"text": "hello world"})
    assert result["word_count"] == 2
```
The Rust test harness serves the same purpose. Rust uses `#[tokio::test]` instead of pytest fixtures, and the harness struct replaces the fixture. The pattern is the same: create an isolated environment, load the plugin, exercise it, assert results.
:::

## Testing Tool Registration

Verify that a plugin registers the expected tools with the correct schemas:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_registers_tools() {
        let mut harness = PluginTestHarness::new();

        // Load the plugin
        let plugin = Box::new(WordCountPlugin::new());
        harness.load_plugin(plugin).await.expect("Plugin should load");

        // Verify tools were registered
        let tools = harness.tools().await;
        assert_eq!(tools.len(), 1, "Should register exactly one tool");
        assert_eq!(tools[0].name, "word_count");
        assert!(tools[0].description.contains("word"));

        // Verify the schema has the expected parameters
        let params = &tools[0].parameters;
        assert!(
            params.get("properties").unwrap().get("text").is_some(),
            "Should have a 'text' parameter"
        );
    }

    #[tokio::test]
    async fn test_tool_execution() {
        let mut harness = PluginTestHarness::new();
        harness
            .load_plugin(Box::new(WordCountPlugin::new()))
            .await
            .unwrap();

        // Test with normal input
        let result = harness
            .call_tool(
                "word_count",
                serde_json::json!({ "text": "the quick brown fox" }),
            )
            .await
            .expect("Tool should succeed");

        assert_eq!(result["word_count"], 4);
    }

    #[tokio::test]
    async fn test_tool_with_empty_input() {
        let mut harness = PluginTestHarness::new();
        harness
            .load_plugin(Box::new(WordCountPlugin::new()))
            .await
            .unwrap();

        let result = harness
            .call_tool("word_count", serde_json::json!({ "text": "" }))
            .await
            .expect("Tool should handle empty input");

        assert_eq!(result["word_count"], 0);
    }

    #[tokio::test]
    async fn test_tool_missing_required_param() {
        let mut harness = PluginTestHarness::new();
        harness
            .load_plugin(Box::new(WordCountPlugin::new()))
            .await
            .unwrap();

        let result = harness
            .call_tool("word_count", serde_json::json!({}))
            .await;

        assert!(result.is_err(), "Should fail with missing required param");
    }
}
```

## Testing Event Handlers

Test that plugins respond correctly to events:

```rust
#[cfg(test)]
mod event_tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_plugin_tracks_tool_calls() {
        let mut harness = PluginTestHarness::new();

        let metrics_plugin = Box::new(MetricsPlugin::new());
        harness.load_plugin(metrics_plugin).await.unwrap();

        // Simulate tool completion events
        harness
            .emit_event(AgentEvent::ToolCallCompleted {
                tool_name: "read_file".to_string(),
                result: serde_json::json!({"content": "hello"}),
                duration_ms: 50,
            })
            .await;

        harness
            .emit_event(AgentEvent::ToolCallCompleted {
                tool_name: "read_file".to_string(),
                result: serde_json::json!({"content": "world"}),
                duration_ms: 30,
            })
            .await;

        harness
            .emit_event(AgentEvent::ToolCallFailed {
                tool_name: "write_file".to_string(),
                error: "Permission denied".to_string(),
            })
            .await;

        // Verify the metrics plugin recorded the events
        // (In practice, you would expose metrics through a command or method)
        harness
            .assert_event_emitted("tool.completed")
            .await;
        harness
            .assert_event_emitted("tool.failed")
            .await;
    }
}
```

## Testing Hooks

Verify that hooks correctly modify, skip, or abort operations:

```rust
#[cfg(test)]
mod hook_tests {
    use super::*;

    #[tokio::test]
    async fn test_security_hook_blocks_dangerous_commands() {
        let mut harness = PluginTestHarness::new();

        // Register the security hook
        {
            let mut registry = harness.hook_registry.write().await;
            registry.register(
                "security",
                HookPoint::PreToolUse,
                0,
                create_security_hook(),
            );
        }

        // Test with a safe command
        let safe_context = HookContext {
            hook_point: HookPoint::PreToolUse,
            tool_name: Some("shell".to_string()),
            tool_input: Some(serde_json::json!({
                "command": "ls -la"
            })),
            tool_output: None,
            message: None,
            metadata: serde_json::json!({}),
        };

        let action = harness
            .run_hook(&HookPoint::PreToolUse, safe_context)
            .await;

        assert!(
            matches!(action, HookAction::Continue(_)),
            "Safe commands should continue"
        );

        // Test with a dangerous command
        let dangerous_context = HookContext {
            hook_point: HookPoint::PreToolUse,
            tool_name: Some("shell".to_string()),
            tool_input: Some(serde_json::json!({
                "command": "rm -rf /"
            })),
            tool_output: None,
            message: None,
            metadata: serde_json::json!({}),
        };

        let action = harness
            .run_hook(&HookPoint::PreToolUse, dangerous_context)
            .await;

        assert!(
            matches!(action, HookAction::Abort(_)),
            "Dangerous commands should be blocked"
        );
    }

    #[tokio::test]
    async fn test_hook_chain_ordering() {
        let mut harness = PluginTestHarness::new();

        let execution_order = Arc::new(RwLock::new(Vec::<String>::new()));

        // Register hooks at different priorities
        let order_clone = execution_order.clone();
        {
            let mut registry = harness.hook_registry.write().await;

            let order = order_clone.clone();
            registry.register(
                "first",
                HookPoint::PreToolUse,
                10,
                Arc::new(move |ctx| {
                    let order = order.clone();
                    Box::pin(async move {
                        order.write().await.push("first".to_string());
                        HookAction::Continue(ctx)
                    })
                }),
            );

            let order = order_clone.clone();
            registry.register(
                "second",
                HookPoint::PreToolUse,
                20,
                Arc::new(move |ctx| {
                    let order = order.clone();
                    Box::pin(async move {
                        order.write().await.push("second".to_string());
                        HookAction::Continue(ctx)
                    })
                }),
            );
        }

        let ctx = HookContext {
            hook_point: HookPoint::PreToolUse,
            tool_name: Some("test".to_string()),
            tool_input: None,
            tool_output: None,
            message: None,
            metadata: serde_json::json!({}),
        };

        harness.run_hook(&HookPoint::PreToolUse, ctx).await;

        let order = execution_order.read().await;
        assert_eq!(
            *order,
            vec!["first".to_string(), "second".to_string()],
            "Hooks should execute in priority order"
        );
    }
}
```

## Testing Commands

Verify that slash commands parse arguments correctly and produce the right output:

```rust
#[cfg(test)]
mod command_tests {
    use super::*;

    #[tokio::test]
    async fn test_status_command() {
        let mut harness = PluginTestHarness::new();

        // Register the status command
        {
            let mut registry = harness.command_registry.write().await;
            registry
                .register(Box::new(StatusCommand::new()))
                .unwrap();
        }

        let result = harness.dispatch_command("/status").await.unwrap();

        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Session:"));
                assert!(text.contains("Working Directory:"));
                assert!(text.contains("Available Tools:"));
            }
            other => panic!("Expected Output, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_unknown_command() {
        let harness = PluginTestHarness::new();

        let result = harness.dispatch_command("/nonexistent").await;

        assert!(result.is_err(), "Unknown commands should return an error");
    }
}
```

## Integration Test Pattern

For end-to-end testing where you need to verify the full plugin lifecycle:

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_full_plugin_lifecycle() {
        let mut harness = PluginTestHarness::new();

        // Phase 1: Load the plugin
        let plugin = Box::new(WordCountPlugin::new());
        harness.load_plugin(plugin).await.unwrap();

        // Verify it registered its tools
        let tools = harness.tools().await;
        let has_word_count = tools.iter().any(|t| t.name == "word_count");
        assert!(has_word_count, "word_count tool should be registered");

        // Phase 2: Use the tool
        let result = harness
            .call_tool(
                "word_count",
                serde_json::json!({ "text": "hello world" }),
            )
            .await
            .unwrap();
        assert_eq!(result["word_count"], 2);

        // Phase 3: Simulate events the plugin might react to
        harness.emit_event(AgentEvent::SessionStarted {
            session_id: "test-123".to_string(),
        }).await;

        // Phase 4: Verify tools are gone after deregistration
        {
            let mut registry = harness.tool_registry.write().await;
            registry.deregister_all_by_owner("word-count");
        }

        let tools_after = harness.tools().await;
        let still_has_it = tools_after.iter().any(|t| t.name == "word_count");
        assert!(!still_has_it, "Tool should be removed after deregistration");
    }
}
```

::: info In the Wild
Testing in the Claude Code ecosystem relies on the MCP specification itself as a test contract. MCP server developers write tests that verify their server handles `initialize`, `tools/list`, and `tools/call` correctly according to the spec. The protocol acts as the test surface -- if your server speaks MCP correctly, it works with any MCP client. This is a powerful pattern: protocol-based testing gives you compatibility guarantees across the entire ecosystem without testing against every specific client.
:::

## Test Utilities

Provide helpers that make common assertions concise:

```rust
/// Assertion helpers for plugin tests.
pub mod test_utils {
    use super::*;

    /// Assert that a tool with the given name is registered.
    pub async fn assert_tool_registered(
        harness: &PluginTestHarness,
        name: &str,
    ) {
        let tools = harness.tools().await;
        assert!(
            tools.iter().any(|t| t.name == name),
            "Expected tool '{}' to be registered. Available: {:?}",
            name,
            tools.iter().map(|t| &t.name).collect::<Vec<_>>()
        );
    }

    /// Assert a tool call succeeds and return the result.
    pub async fn assert_tool_succeeds(
        harness: &PluginTestHarness,
        name: &str,
        params: serde_json::Value,
    ) -> serde_json::Value {
        harness
            .call_tool(name, params)
            .await
            .unwrap_or_else(|e| panic!(
                "Expected tool '{}' to succeed, but got error: {}",
                name, e
            ))
    }

    /// Assert a tool call fails.
    pub async fn assert_tool_fails(
        harness: &PluginTestHarness,
        name: &str,
        params: serde_json::Value,
    ) {
        let result = harness.call_tool(name, params).await;
        assert!(
            result.is_err(),
            "Expected tool '{}' to fail, but it succeeded",
            name
        );
    }
}
```

## Key Takeaways

- The `PluginTestHarness` provides a self-contained environment with mock registries, an event bus, and captured events, letting plugin developers test without running the full agent
- Test tool registration by checking that the expected tools appear in the registry with the correct names, descriptions, and schemas after plugin activation
- Test event handlers by emitting synthetic events and verifying that the plugin's state changes or side effects are correct
- Test hooks by building `HookContext` objects and asserting that the returned `HookAction` is `Continue`, `Skip`, or `Abort` as expected
- Protocol-based testing (like MCP's spec-driven approach) provides ecosystem-wide compatibility guarantees that are more valuable than testing against a specific agent implementation
