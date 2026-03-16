---
title: Extension Testing
description: Build testing infrastructure that helps extension authors verify their plugins work correctly against the agent's plugin API.
---

# Extension Testing

> **What you'll learn:**
> - How to provide a test harness that simulates the agent host environment so plugin authors can test their extensions in isolation
> - Techniques for writing integration tests that verify plugins interact correctly with the event bus, hook system, and tool registry
> - How to implement conformance test suites that extension authors run to validate their plugins meet the expected behavioral contracts

An extension ecosystem lives or dies on the quality of its testing infrastructure. If plugin authors cannot easily test their extensions, bugs proliferate, users lose trust, and the ecosystem stagnates. Your job as the agent platform developer is to provide testing tools that make writing correct plugins easy and writing broken plugins hard.

This subchapter covers three levels of testing: unit tests for individual plugin logic, integration tests with a simulated host environment, and conformance tests that validate a plugin meets the behavioral contracts of the plugin API.

## The Test Harness: A Simulated Agent Environment

Plugin authors need to test their plugins without running the full agent. A test harness provides mock versions of all the services a plugin interacts with:

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

/// A simulated agent environment for testing plugins.
/// Provides mock implementations of all plugin-facing services.
pub struct TestHarness {
    pub tool_registry: MockToolRegistry,
    pub event_bus: TestEventBus,
    pub hook_registry: TestHookRegistry,
    pub config: serde_json::Value,
    recorded_events: Arc<RwLock<Vec<AgentEvent>>>,
}

impl TestHarness {
    pub fn new() -> Self {
        let recorded_events = Arc::new(RwLock::new(Vec::new()));
        Self {
            tool_registry: MockToolRegistry::new(),
            event_bus: TestEventBus::new(recorded_events.clone()),
            hook_registry: TestHookRegistry::new(),
            config: serde_json::json!({}),
            recorded_events,
        }
    }

    /// Create a PluginContext from this harness for plugin initialization.
    pub fn plugin_context(&self) -> PluginContext {
        PluginContext {
            tool_registry: Arc::new(self.tool_registry.clone()),
            event_bus: Arc::new(self.event_bus.clone()),
            config: Arc::new(self.config.clone()),
        }
    }

    /// Get all events that were emitted during the test.
    pub async fn recorded_events(&self) -> Vec<AgentEvent> {
        self.recorded_events.read().await.clone()
    }

    /// Assert that a specific event type was emitted.
    pub async fn assert_event_emitted<F>(&self, predicate: F)
    where
        F: Fn(&AgentEvent) -> bool,
    {
        let events = self.recorded_events.read().await;
        assert!(
            events.iter().any(|e| predicate(e)),
            "Expected event not found. Recorded events: {:#?}",
            *events
        );
    }

    /// Assert that no events matching the predicate were emitted.
    pub async fn assert_no_event<F>(&self, predicate: F)
    where
        F: Fn(&AgentEvent) -> bool,
    {
        let events = self.recorded_events.read().await;
        assert!(
            !events.iter().any(|e| predicate(e)),
            "Unexpected event found in recorded events"
        );
    }
}
```

## Mock Tool Registry

The mock tool registry records tool registrations and lets tests verify that a plugin registered the expected tools:

```rust
#[derive(Clone)]
pub struct MockToolRegistry {
    registered_tools: Arc<RwLock<Vec<RegisteredTool>>>,
    /// Pre-configured tool results for testing tool-calling plugins
    mock_results: Arc<RwLock<HashMap<String, String>>>,
}

#[derive(Debug, Clone)]
pub struct RegisteredTool {
    pub name: String,
    pub description: String,
    pub schema: serde_json::Value,
}

impl MockToolRegistry {
    pub fn new() -> Self {
        Self {
            registered_tools: Arc::new(RwLock::new(Vec::new())),
            mock_results: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Pre-configure a mock result for a tool call.
    pub async fn mock_tool_result(&self, tool_name: &str, result: &str) {
        self.mock_results
            .write()
            .await
            .insert(tool_name.to_string(), result.to_string());
    }

    /// Get all tools that were registered during the test.
    pub async fn registered_tools(&self) -> Vec<RegisteredTool> {
        self.registered_tools.read().await.clone()
    }

    /// Assert that a tool with the given name was registered.
    pub async fn assert_tool_registered(&self, name: &str) {
        let tools = self.registered_tools.read().await;
        assert!(
            tools.iter().any(|t| t.name == name),
            "Expected tool '{name}' to be registered. Registered tools: {:?}",
            tools.iter().map(|t| &t.name).collect::<Vec<_>>()
        );
    }
}

#[async_trait::async_trait]
impl ToolRegistry for MockToolRegistry {
    fn register(&self, tool: Box<dyn Tool>) {
        let registered = RegisteredTool {
            name: tool.name().to_string(),
            description: tool.description().to_string(),
            schema: tool.parameters_schema().clone(),
        };
        // Use try_write to avoid blocking in sync context
        if let Ok(mut tools) = self.registered_tools.try_write() {
            tools.push(registered);
        }
    }

    async fn execute(&self, name: &str, _args: serde_json::Value) -> Result<String> {
        let results = self.mock_results.read().await;
        results
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No mock result configured for tool '{name}'"))
    }
}
```

## Test Event Bus

The test event bus captures all emitted events for later assertions:

```rust
#[derive(Clone)]
pub struct TestEventBus {
    recorded: Arc<RwLock<Vec<AgentEvent>>>,
}

impl TestEventBus {
    pub fn new(recorded: Arc<RwLock<Vec<AgentEvent>>>) -> Self {
        Self { recorded }
    }
}

#[async_trait::async_trait]
impl EventBus for TestEventBus {
    async fn emit(&self, event: &AgentEvent) {
        self.recorded.write().await.push(event.clone());
    }

    async fn subscribe(
        &self,
        _handler: Arc<dyn EventHandler>,
        _priority: i32,
    ) -> SubscriptionId {
        // In tests, events are captured directly rather than dispatched
        SubscriptionId(0)
    }
}
```

::: python Coming from Python
Python testing frameworks like `pytest` use fixtures and mocks extensively:
```python
@pytest.fixture
def agent_context():
    return MockAgentContext(
        tool_registry=MockToolRegistry(),
        event_bus=MockEventBus(),
    )

def test_my_plugin(agent_context):
    plugin = MyPlugin()
    plugin.init(agent_context)
    assert "my_tool" in agent_context.tool_registry.registered_tools
```
Rust's approach is structurally similar -- you provide mock implementations of the interfaces the plugin depends on. The key difference is that Rust's trait system ensures the mocks implement the exact same interface as the real implementations, so a test that passes with mocks will not fail due to interface mismatches in production.
:::

## Writing Plugin Unit Tests

Here is how a plugin author uses the test harness to verify their plugin:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_registers_expected_tools() {
        let harness = TestHarness::new();
        let mut ctx = harness.plugin_context();

        let plugin = MyCustomPlugin::new();
        plugin.init(&mut ctx).expect("Plugin init failed");

        // Verify the plugin registered its tools
        harness.tool_registry
            .assert_tool_registered("my_custom_tool")
            .await;
    }

    #[tokio::test]
    async fn test_plugin_handles_tool_execution() {
        let harness = TestHarness::new();

        // Set up mock results for tools the plugin depends on
        harness.tool_registry
            .mock_tool_result("read_file", "file contents here")
            .await;

        let mut ctx = harness.plugin_context();
        let plugin = MyCustomPlugin::new();
        plugin.init(&mut ctx).expect("Plugin init failed");

        // Execute the plugin's tool
        let result = harness.tool_registry
            .execute(
                "my_custom_tool",
                serde_json::json!({"path": "/test.txt"}),
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_plugin_emits_events() {
        let harness = TestHarness::new();
        let mut ctx = harness.plugin_context();

        let plugin = MyCustomPlugin::new();
        plugin.init(&mut ctx).expect("Plugin init failed");

        // Trigger some action that should emit an event
        // ...

        harness.assert_event_emitted(|event| {
            matches!(event, AgentEvent::PluginStateChanged {
                plugin_name, new_state
            } if plugin_name == "my_custom_plugin" && new_state == "ready")
        }).await;
    }

    #[tokio::test]
    async fn test_plugin_shutdown_cleans_up() {
        let harness = TestHarness::new();
        let mut ctx = harness.plugin_context();

        let plugin = MyCustomPlugin::new();
        plugin.init(&mut ctx).expect("Plugin init failed");

        // Shutdown should not panic or return an error
        plugin.shutdown().expect("Plugin shutdown failed");
    }
}
```

## Integration Tests for Hook Interactions

Hooks require more sophisticated testing because they modify data flowing through the agent. The test harness needs to simulate the hook pipeline:

```rust
pub struct TestHookRunner {
    hooks: Vec<Arc<dyn HookHandler<ToolExecutionHookData>>>,
}

impl TestHookRunner {
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    pub fn add_hook(&mut self, hook: Arc<dyn HookHandler<ToolExecutionHookData>>) {
        self.hooks.push(hook);
        self.hooks.sort_by_key(|h| h.priority());
    }

    /// Run all hooks against the given data and return the result.
    pub async fn run(
        &self,
        mut data: ToolExecutionHookData,
    ) -> Result<HookAction<ToolExecutionHookData>> {
        for hook in &self.hooks {
            match hook.pre_hook(data).await? {
                HookAction::Continue(modified) => data = modified,
                action @ HookAction::Skip { .. } => return Ok(action),
                action @ HookAction::Replace { .. } => return Ok(action),
            }
        }
        Ok(HookAction::Continue(data))
    }
}

#[cfg(test)]
mod hook_tests {
    use super::*;

    #[tokio::test]
    async fn test_security_hook_blocks_dangerous_commands() {
        let mut runner = TestHookRunner::new();
        runner.add_hook(Arc::new(DangerousCommandBlocker::new()));

        let data = ToolExecutionHookData {
            tool_name: "shell".to_string(),
            arguments: serde_json::json!({"command": "rm -rf /"}),
            invocation_id: "test-1".to_string(),
        };

        let result = runner.run(data).await.unwrap();
        assert!(
            matches!(result, HookAction::Skip { .. }),
            "Expected dangerous command to be blocked"
        );
    }

    #[tokio::test]
    async fn test_security_hook_allows_safe_commands() {
        let mut runner = TestHookRunner::new();
        runner.add_hook(Arc::new(DangerousCommandBlocker::new()));

        let data = ToolExecutionHookData {
            tool_name: "shell".to_string(),
            arguments: serde_json::json!({"command": "ls -la"}),
            invocation_id: "test-2".to_string(),
        };

        let result = runner.run(data).await.unwrap();
        assert!(
            matches!(result, HookAction::Continue(_)),
            "Expected safe command to be allowed"
        );
    }

    #[tokio::test]
    async fn test_hook_priority_ordering() {
        let mut runner = TestHookRunner::new();

        // Add hooks with different priorities
        runner.add_hook(Arc::new(LowPriorityHook)); // priority 100
        runner.add_hook(Arc::new(HighPriorityHook)); // priority -100

        let data = ToolExecutionHookData {
            tool_name: "shell".to_string(),
            arguments: serde_json::json!({"command": "test"}),
            invocation_id: "test-3".to_string(),
        };

        // If HighPriorityHook blocks, LowPriorityHook should never run
        let result = runner.run(data).await.unwrap();
        // Assert based on expected behavior
    }
}
```

## Conformance Test Suites

A conformance test suite is a set of tests that the agent project provides for plugin authors to run against their plugins. It verifies that the plugin meets the behavioral contracts of the API:

```rust
/// Run the standard conformance tests against a plugin.
/// Plugin authors call this from their own test suite.
pub async fn run_conformance_tests(plugin: Box<dyn Plugin>) -> ConformanceReport {
    let mut report = ConformanceReport::new(&plugin);

    // Test 1: Plugin provides a non-empty name
    report.check(
        "name_not_empty",
        "Plugin name must not be empty",
        !plugin.name().is_empty(),
    );

    // Test 2: Plugin provides a valid semver version
    report.check(
        "valid_version",
        "Plugin version must be valid semver",
        SemVer::parse(plugin.version()).is_ok(),
    );

    // Test 3: Plugin initializes without error
    let harness = TestHarness::new();
    let mut ctx = harness.plugin_context();
    let init_result = plugin.init(&mut ctx);
    report.check(
        "init_succeeds",
        "Plugin initialization must succeed",
        init_result.is_ok(),
    );

    // Test 4: Plugin shuts down without error
    let shutdown_result = plugin.shutdown();
    report.check(
        "shutdown_succeeds",
        "Plugin shutdown must succeed",
        shutdown_result.is_ok(),
    );

    // Test 5: Plugin can be initialized and shut down multiple times
    let mut ctx2 = harness.plugin_context();
    let reinit = plugin.init(&mut ctx2);
    let reshutdown = plugin.shutdown();
    report.check(
        "reinit_succeeds",
        "Plugin must support re-initialization after shutdown",
        reinit.is_ok() && reshutdown.is_ok(),
    );

    // Test 6: Health check returns a valid status
    let health = plugin.health_check();
    report.check(
        "health_check_valid",
        "Health check must return a valid status",
        matches!(
            health,
            HealthStatus::Healthy
                | HealthStatus::Degraded(_)
                | HealthStatus::Unhealthy(_)
        ),
    );

    report
}

pub struct ConformanceReport {
    plugin_name: String,
    results: Vec<ConformanceCheck>,
}

struct ConformanceCheck {
    id: String,
    description: String,
    passed: bool,
}

impl ConformanceReport {
    fn new(plugin: &dyn Plugin) -> Self {
        Self {
            plugin_name: plugin.name().to_string(),
            results: Vec::new(),
        }
    }

    fn check(&mut self, id: &str, description: &str, passed: bool) {
        self.results.push(ConformanceCheck {
            id: id.to_string(),
            description: description.to_string(),
            passed,
        });
    }

    pub fn all_passed(&self) -> bool {
        self.results.iter().all(|c| c.passed)
    }

    pub fn print_report(&self) {
        println!("Conformance Report for '{}':", self.plugin_name);
        println!("{}", "-".repeat(60));

        for check in &self.results {
            let status = if check.passed { "PASS" } else { "FAIL" };
            println!("  [{status}] {}: {}", check.id, check.description);
        }

        let passed = self.results.iter().filter(|c| c.passed).count();
        let total = self.results.len();
        println!("{}", "-".repeat(60));
        println!("  {passed}/{total} checks passed");
    }
}
```

Plugin authors use the conformance suite in their own test files:

```rust
#[cfg(test)]
mod conformance {
    use super::*;
    use agent_sdk::testing::run_conformance_tests;

    #[tokio::test]
    async fn test_plugin_conformance() {
        let plugin = Box::new(MyCustomPlugin::new());
        let report = run_conformance_tests(plugin).await;
        report.print_report();
        assert!(report.all_passed(), "Plugin failed conformance tests");
    }
}
```

::: wild In the Wild
VS Code's extension testing infrastructure provides a complete test runner that spins up a headless VS Code instance for integration testing. Extension authors run their tests in this environment to verify their extension works correctly with the real editor APIs. For a coding agent, the test harness serves the same purpose: it simulates enough of the agent environment that plugins can be tested without running the full agent binary.
:::

## Key Takeaways

- A **test harness** with mock implementations of the tool registry, event bus, and hook system lets plugin authors test in isolation without running the full agent.
- **Mock tool registries** record registrations and return pre-configured results, making it easy to verify that a plugin registers the right tools and handles tool results correctly.
- **Hook testing** requires simulating the hook pipeline so you can verify that hooks correctly modify, skip, or replace operations based on their logic and priority ordering.
- **Conformance test suites** are published by the agent platform and run by plugin authors to verify their plugins meet the behavioral contracts -- this catches compatibility issues before users encounter them.
- Good testing infrastructure is an **ecosystem investment**: the easier it is to test plugins, the more high-quality plugins your ecosystem will have.
