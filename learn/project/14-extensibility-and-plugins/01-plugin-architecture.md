---
title: Plugin Architecture
description: Designing the overall plugin system with clear extension points, plugin manifests, lifecycle management, and dependency resolution between plugins.
---

# Plugin Architecture

> **What you'll learn:**
> - How to identify and define extension points where plugins can hook into the agent
> - How to design a plugin manifest format that declares capabilities, dependencies, and metadata
> - Patterns for plugin lifecycle management including initialization, activation, and teardown

Until now, every capability your agent has -- file operations, shell execution, git commands -- lives inside the main codebase. Adding a new tool means editing source files, recompiling, and redeploying. That works when you are the only developer, but the moment other people want to extend your agent, you need a different approach. You need a plugin architecture.

A plugin architecture defines clear boundaries between the core agent and its extensions. The core provides a stable API surface -- extension points -- and plugins attach to those points without modifying core code. This is the same pattern that powers VS Code extensions, Vim plugins, and browser add-ons. Let's design one for our agent.

## Identifying Extension Points

The first step is deciding *where* plugins can attach. Look at your agent's architecture and identify the seams -- places where behavior could vary:

1. **Tool registration** -- plugins contribute new tools the LLM can invoke
2. **Event handling** -- plugins react to lifecycle events (session start, tool execution, errors)
3. **Hook interception** -- plugins modify or veto tool inputs/outputs before they reach the LLM
4. **Command registration** -- plugins add new slash commands for users
5. **Prompt injection** -- plugins append to the system prompt based on context
6. **Resource provision** -- plugins expose files, databases, or APIs as MCP resources

Each extension point needs a well-defined trait that plugins implement. Let's start with the core `Plugin` trait that all plugins must satisfy:

```rust
use std::any::Any;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Metadata describing a plugin's identity and requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub dependencies: Vec<PluginDependency>,
    pub capabilities: Vec<PluginCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDependency {
    pub plugin_name: String,
    pub version_req: String, // semver requirement like ">=1.0.0"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PluginCapability {
    Tools,
    Events,
    Hooks,
    Commands,
    Prompts,
    Resources,
}

/// The core trait that every plugin must implement.
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Returns the plugin's manifest with metadata and capabilities.
    fn manifest(&self) -> &PluginManifest;

    /// Called once when the plugin is loaded. Use this for setup.
    async fn initialize(&mut self, ctx: &mut PluginContext) -> Result<(), PluginError>;

    /// Called when the plugin is activated (after dependency resolution).
    async fn activate(&mut self, ctx: &mut PluginContext) -> Result<(), PluginError>;

    /// Called when the plugin is being unloaded. Clean up resources here.
    async fn deactivate(&mut self) -> Result<(), PluginError>;

    /// Allows downcasting to the concrete plugin type.
    fn as_any(&self) -> &dyn Any;
}
```

This trait captures the plugin lifecycle: initialize, activate, deactivate. The `PluginManifest` declares what the plugin provides and what it needs. The `as_any` method enables type-safe downcasting, which you will need when the plugin manager queries specific capabilities.

::: tip Coming from Python
In Python, plugins are often loaded dynamically through `importlib` or entry points:
```python
import importlib

def load_plugin(module_path: str):
    module = importlib.import_module(module_path)
    return module.create_plugin()
```
Rust does not have runtime module loading in the same way. Instead, plugins implement a trait and are either compiled into the binary (static plugins) or loaded through a process boundary (MCP servers, WASM modules). We will start with compiled-in plugins and explore process-based isolation later in this chapter.
:::

## The Plugin Context

Plugins need a way to interact with the agent. Rather than exposing the entire agent internals, you provide a controlled `PluginContext` that grants access only to the APIs plugins should use:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// The agent-facing API surface available to plugins.
pub struct PluginContext {
    /// Register a new tool with the agent.
    pub tool_registry: Arc<RwLock<ToolRegistry>>,
    /// Subscribe to agent events.
    pub event_bus: Arc<EventBus>,
    /// Register hook handlers.
    pub hook_registry: Arc<RwLock<HookRegistry>>,
    /// Register slash commands.
    pub command_registry: Arc<RwLock<CommandRegistry>>,
    /// Key-value storage scoped to this plugin.
    pub storage: HashMap<String, String>,
}

impl PluginContext {
    pub fn new(
        tool_registry: Arc<RwLock<ToolRegistry>>,
        event_bus: Arc<EventBus>,
        hook_registry: Arc<RwLock<HookRegistry>>,
        command_registry: Arc<RwLock<CommandRegistry>>,
    ) -> Self {
        Self {
            tool_registry,
            event_bus,
            hook_registry,
            command_registry,
            storage: HashMap::new(),
        }
    }
}
```

Each field in `PluginContext` corresponds to an extension point. The plugin uses the registries during `activate()` to contribute its tools, commands, and hooks. The `storage` field gives each plugin its own key-value store for persisting state between calls.

## Plugin Manager and Lifecycle

The `PluginManager` orchestrates loading, dependency resolution, and lifecycle management for all plugins:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct PluginManager {
    plugins: HashMap<String, Box<dyn Plugin>>,
    load_order: Vec<String>,
    context: PluginContext,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PluginState {
    Registered,
    Initialized,
    Active,
    Failed(String),
    Deactivated,
}

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin '{0}' not found")]
    NotFound(String),
    #[error("Dependency '{dep}' required by '{plugin}' is not available")]
    MissingDependency { plugin: String, dep: String },
    #[error("Circular dependency detected involving '{0}'")]
    CircularDependency(String),
    #[error("Plugin initialization failed: {0}")]
    InitFailed(String),
    #[error("Version conflict: {0}")]
    VersionConflict(String),
}

impl PluginManager {
    pub fn new(context: PluginContext) -> Self {
        Self {
            plugins: HashMap::new(),
            load_order: Vec::new(),
            context,
        }
    }

    /// Register a plugin without activating it.
    pub fn register(&mut self, plugin: Box<dyn Plugin>) -> Result<(), PluginError> {
        let name = plugin.manifest().name.clone();
        if self.plugins.contains_key(&name) {
            return Err(PluginError::VersionConflict(
                format!("Plugin '{}' is already registered", name),
            ));
        }
        self.plugins.insert(name, plugin);
        Ok(())
    }

    /// Resolve dependencies and determine the activation order.
    pub fn resolve_load_order(&mut self) -> Result<(), PluginError> {
        let mut visited: HashMap<String, bool> = HashMap::new();
        let mut order: Vec<String> = Vec::new();

        for name in self.plugins.keys().cloned().collect::<Vec<_>>() {
            self.topological_sort(&name, &mut visited, &mut order)?;
        }

        self.load_order = order;
        Ok(())
    }

    fn topological_sort(
        &self,
        name: &str,
        visited: &mut HashMap<String, bool>,
        order: &mut Vec<String>,
    ) -> Result<(), PluginError> {
        if let Some(&in_progress) = visited.get(name) {
            if in_progress {
                return Err(PluginError::CircularDependency(name.to_string()));
            }
            return Ok(()); // Already processed
        }

        visited.insert(name.to_string(), true); // Mark as in-progress

        if let Some(plugin) = self.plugins.get(name) {
            for dep in &plugin.manifest().dependencies {
                if !self.plugins.contains_key(&dep.plugin_name) {
                    return Err(PluginError::MissingDependency {
                        plugin: name.to_string(),
                        dep: dep.plugin_name.clone(),
                    });
                }
                self.topological_sort(&dep.plugin_name, visited, order)?;
            }
        }

        visited.insert(name.to_string(), false); // Mark as completed
        order.push(name.to_string());
        Ok(())
    }

    /// Initialize and activate all plugins in dependency order.
    pub async fn activate_all(&mut self) -> Result<(), PluginError> {
        self.resolve_load_order()?;

        for name in self.load_order.clone() {
            if let Some(plugin) = self.plugins.get_mut(&name) {
                plugin
                    .initialize(&mut self.context)
                    .await
                    .map_err(|e| PluginError::InitFailed(
                        format!("{}: {}", name, e),
                    ))?;

                plugin
                    .activate(&mut self.context)
                    .await
                    .map_err(|e| PluginError::InitFailed(
                        format!("{}: activation failed: {}", name, e),
                    ))?;

                println!("[plugin] Activated: {}", name);
            }
        }

        Ok(())
    }

    /// Deactivate all plugins in reverse order.
    pub async fn deactivate_all(&mut self) -> Result<(), PluginError> {
        for name in self.load_order.iter().rev() {
            if let Some(plugin) = self.plugins.get_mut(name) {
                if let Err(e) = plugin.deactivate().await {
                    eprintln!("[plugin] Warning: {} deactivation error: {}", name, e);
                    // Continue deactivating other plugins
                }
            }
        }
        Ok(())
    }
}
```

The key insight is topological sorting for dependency resolution. If plugin A depends on plugin B, B must be initialized first. The reverse applies during deactivation -- A shuts down before B.

::: info In the Wild
Claude Code implements hooks as a configuration-driven system where users define shell commands that run at specific lifecycle points (pre-tool-use, post-tool-use, notification, etc.). The hooks are defined in `.claude/settings.json` rather than compiled into the binary, making them accessible to non-developers. This is a pragmatic approach: full plugin systems are powerful but complex, while config-driven hooks cover the most common extensibility needs with minimal implementation effort.
:::

## A Concrete Plugin Example

Let's see how a plugin author would use this system. Here is a simple plugin that adds a "word count" tool:

```rust
use async_trait::async_trait;
use std::any::Any;

pub struct WordCountPlugin {
    manifest: PluginManifest,
}

impl WordCountPlugin {
    pub fn new() -> Self {
        Self {
            manifest: PluginManifest {
                name: "word-count".to_string(),
                version: "1.0.0".to_string(),
                description: "Adds a word counting tool".to_string(),
                author: "Agent Community".to_string(),
                dependencies: vec![],
                capabilities: vec![PluginCapability::Tools],
            },
        }
    }
}

#[async_trait]
impl Plugin for WordCountPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn initialize(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
        // Nothing to set up for this simple plugin
        Ok(())
    }

    async fn activate(&mut self, ctx: &mut PluginContext) -> Result<(), PluginError> {
        // Register our tool with the agent's tool system
        let tool_def = ToolDefinition {
            name: "word_count".to_string(),
            description: "Count words in a given text".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "The text to count words in"
                    }
                },
                "required": ["text"]
            }),
        };

        let mut registry = ctx.tool_registry.write().await;
        registry.register_tool("word-count", tool_def, |params| {
            Box::pin(async move {
                let text = params["text"].as_str().unwrap_or("");
                let count = text.split_whitespace().count();
                Ok(serde_json::json!({ "word_count": count }))
            })
        })?;

        Ok(())
    }

    async fn deactivate(&mut self) -> Result<(), PluginError> {
        // Tool registry cleanup happens automatically
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
```

With this architecture, the agent bootstrap code registers and activates plugins before entering the main loop:

```rust
async fn main() -> Result<()> {
    // ... agent setup ...

    let mut plugin_manager = PluginManager::new(plugin_context);

    // Register built-in plugins
    plugin_manager.register(Box::new(WordCountPlugin::new()))?;

    // Discover and register external plugins from config
    // (We will build this in the config-driven extensions section)

    // Activate all plugins in dependency order
    plugin_manager.activate_all().await?;

    // Enter the main agent loop with all plugins active
    run_agent_loop().await?;

    // Clean shutdown
    plugin_manager.deactivate_all().await?;

    Ok(())
}
```

## Choosing Static vs. Dynamic Plugins

There are two broad approaches to plugin loading in Rust:

| Approach | How it works | Pros | Cons |
|----------|-------------|------|------|
| **Static (compiled-in)** | Plugins implement a trait and are registered in `main()` | Type-safe, fast, no runtime overhead | Requires recompilation to add plugins |
| **Process-based** | Plugins run as separate processes, communicate via IPC (MCP, gRPC) | Full isolation, any language | Network overhead, complex lifecycle |

We start with static plugins because they are simpler and leverage Rust's type system. Process-based plugins come naturally when we implement MCP support later in this chapter. In practice, production agents use a hybrid: core tools are compiled in, while community extensions run as MCP servers.

## Key Takeaways

- A plugin architecture separates the core agent from extensions through well-defined extension points -- tool registration, events, hooks, commands, and prompts
- The `Plugin` trait defines a lifecycle (initialize, activate, deactivate) that the `PluginManager` orchestrates in dependency order using topological sorting
- `PluginContext` provides a controlled API surface so plugins can interact with the agent without accessing its internals
- Plugin manifests declare metadata, capabilities, and dependencies, enabling the manager to validate compatibility before activation
- Start with compiled-in plugins for type safety, then add process-based plugins (MCP) for isolation and language independence
