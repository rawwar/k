---
title: Plugin Architecture Patterns
description: Survey the major plugin architecture patterns — monolithic, microkernel, and modular — and evaluate their fit for coding agents.
---

# Plugin Architecture Patterns

> **What you'll learn:**
> - The core plugin architecture patterns (microkernel, pipes-and-filters, modular monolith) and how each maps to coding agent requirements
> - How to define plugin contracts that specify what a plugin can provide (tools, providers, UI elements) and what the host guarantees in return
> - The tradeoffs between compile-time plugins (static linking), dynamic loading (shared libraries), and process-level plugins (IPC/subprocess)

Every time you add a new tool to your coding agent -- a file reader, a web searcher, a database connector -- you face a design decision. Do you hardcode it into the binary? Do you let users drop in a shared library? Do you spawn an external process? The answer depends on your plugin architecture, and getting it right early saves enormous refactoring pain later.

In this subchapter, you will survey the three dominant plugin architecture patterns, see how each maps to Rust's type system and ownership model, and learn how production coding agents make these choices.

## The Monolithic Starting Point

Most agents start monolithic: every tool, every provider, every behavior lives in one binary. This is fine for prototyping, but it creates a problem as the agent grows. Every new capability requires modifying the core codebase, recompiling, and redeploying. Worse, contributors need to understand the entire codebase to add a single tool.

```rust
// The monolithic approach: everything is hardcoded
fn dispatch_tool(name: &str, args: serde_json::Value) -> Result<String> {
    match name {
        "read_file" => tools::read_file(args),
        "write_file" => tools::write_file(args),
        "shell" => tools::shell_exec(args),
        "web_search" => tools::web_search(args),
        // Every new tool means editing this match statement
        _ => Err(anyhow!("Unknown tool: {name}")),
    }
}
```

This works, but every new tool requires a code change. Let's look at three patterns that break this coupling.

## Pattern 1: The Microkernel Architecture

The microkernel pattern splits your agent into a small, stable core and a set of plugins that provide all the interesting functionality. The core handles the agentic loop, LLM communication, and plugin lifecycle. Everything else -- tools, providers, UI components -- lives in plugins.

```rust
/// The core trait every plugin must implement.
/// This is the contract between the host and the plugin.
pub trait Plugin: Send + Sync {
    /// Unique identifier for this plugin
    fn name(&self) -> &str;

    /// Plugin version (semver)
    fn version(&self) -> &str;

    /// Called once when the plugin is loaded.
    /// The host provides a registration context for the plugin to register
    /// its tools, event handlers, and hooks.
    fn init(&self, ctx: &mut PluginContext) -> Result<()>;

    /// Called when the plugin is about to be unloaded.
    /// Clean up resources, cancel background tasks, flush buffers.
    fn shutdown(&self) -> Result<()>;
}

/// The context provided to plugins during initialization.
/// This is what the host guarantees to every plugin.
pub struct PluginContext {
    pub tool_registry: Arc<dyn ToolRegistry>,
    pub event_bus: Arc<dyn EventBus>,
    pub config: Arc<dyn ConfigProvider>,
}
```

The microkernel is the most popular pattern for extensible developer tools. VS Code uses it: the core is an Electron shell with an editor, and everything from language support to themes to debuggers is an extension.

::: python Coming from Python
Python's plugin systems often use entry points or module discovery:
```python
# setup.py / pyproject.toml entry points
[project.entry-points."myagent.plugins"]
git_tool = "myagent_git:GitPlugin"

# Runtime discovery
import importlib.metadata
for ep in importlib.metadata.entry_points(group="myagent.plugins"):
    plugin_class = ep.load()
    plugin = plugin_class()
    plugin.init(context)
```
Rust does not have a built-in equivalent of entry points, but crates like `inventory` and `linkme` provide compile-time registration that achieves the same goal with zero runtime overhead and full type safety.
:::

## Pattern 2: Pipes-and-Filters

In the pipes-and-filters pattern, data flows through a pipeline of processing stages. Each stage transforms the data and passes it to the next. This maps well to certain agent operations like message processing, where a message might flow through a content filter, a token counter, a context compactor, and finally the LLM.

```rust
/// A filter that transforms messages as they flow through the pipeline.
#[async_trait]
pub trait MessageFilter: Send + Sync {
    /// Process the message and return a potentially modified version.
    /// Return None to drop the message entirely.
    async fn filter(&self, message: Message) -> Result<Option<Message>>;

    /// Filters run in priority order (lower numbers first).
    fn priority(&self) -> i32 { 0 }
}

/// The pipeline composes filters into a processing chain.
pub struct MessagePipeline {
    filters: Vec<Box<dyn MessageFilter>>,
}

impl MessagePipeline {
    pub fn new() -> Self {
        Self { filters: Vec::new() }
    }

    pub fn add_filter(&mut self, filter: Box<dyn MessageFilter>) {
        self.filters.push(filter);
        self.filters.sort_by_key(|f| f.priority());
    }

    pub async fn process(&self, mut message: Message) -> Result<Option<Message>> {
        for filter in &self.filters {
            match filter.filter(message).await? {
                Some(modified) => message = modified,
                None => return Ok(None), // Message was dropped
            }
        }
        Ok(Some(message))
    }
}
```

This pattern shines for cross-cutting concerns like logging, rate limiting, and content filtering. It is less suited for the general plugin case where plugins provide diverse capabilities rather than transforming a uniform data stream.

## Pattern 3: The Modular Monolith

The modular monolith keeps everything in one binary but enforces strict module boundaries. Each module exposes a public API and hides its internals. This is Rust's natural architecture thanks to its module system and visibility rules.

```rust
// Each module declares its public API and hides internals
pub mod tools {
    mod file_ops;    // Private: implementation details
    mod shell;       // Private: implementation details

    pub use file_ops::FileReadTool;   // Public: the tool
    pub use file_ops::FileWriteTool;  // Public: the tool
    pub use shell::ShellTool;         // Public: the tool

    /// Every module registers its tools through this function.
    pub fn register_all(registry: &mut ToolRegistry) {
        registry.register(Box::new(FileReadTool));
        registry.register(Box::new(FileWriteTool));
        registry.register(Box::new(ShellTool));
    }
}

pub mod providers {
    mod anthropic;
    mod openai;

    pub use anthropic::AnthropicProvider;
    pub use openai::OpenAIProvider;

    pub fn register_all(registry: &mut ProviderRegistry) {
        registry.register(Box::new(AnthropicProvider::new()));
        registry.register(Box::new(OpenAIProvider::new()));
    }
}
```

The modular monolith gives you compile-time safety, zero runtime overhead, and easy refactoring. Its limitation is that all modules must be known at compile time -- users cannot add capabilities without rebuilding the binary.

## Choosing Your Pattern

For a coding agent, the pragmatic choice is usually a **hybrid approach**: start with a modular monolith for core features, add microkernel-style plugin support for user extensions, and use pipes-and-filters for specific data flows like message processing.

Here is how the patterns compare:

| Concern | Microkernel | Pipes & Filters | Modular Monolith |
|---------|------------|-----------------|-------------------|
| Adding capabilities | Plugin authors, no core changes | Add a filter stage | Requires recompilation |
| Type safety | At trait boundary only | At filter interface | Full compile-time checking |
| Performance | Indirect calls, possible IPC | Sequential overhead | Direct calls, inlinable |
| Isolation | Can sandbox plugins | Filters share memory | Everything shares memory |
| Complexity | High (lifecycle, versioning) | Medium (ordering) | Low (it is just Rust modules) |

::: wild In the Wild
Claude Code uses a modular monolith approach internally -- its tools (Bash, Read, Write, Edit, etc.) are compiled into the binary. However, it extends outward through MCP, treating external MCP servers as process-level plugins. This hybrid gives it compile-time safety for core features and runtime extensibility for the ecosystem. OpenCode takes a similar approach, with built-in tools compiled into the Go binary and MCP providing the extension surface.
:::

## Defining the Plugin Contract

Regardless of which pattern you choose, you need a clear contract between the host and its plugins. The contract answers two questions:

1. **What can the plugin provide?** Tools, event handlers, hooks, configuration schemas, UI elements.
2. **What does the host guarantee?** Access to the event bus, tool registry, configuration, and logging.

```rust
/// Declares the capabilities a plugin provides.
#[derive(Debug, Clone)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub provides: Vec<Capability>,
    pub requires: Vec<Dependency>,
}

#[derive(Debug, Clone)]
pub enum Capability {
    Tool { name: String, description: String },
    EventHandler { event_type: String },
    Hook { hook_point: String },
    ConfigSchema { schema: serde_json::Value },
}

#[derive(Debug, Clone)]
pub struct Dependency {
    pub name: String,
    pub version_req: String, // semver requirement, e.g., ">=1.0, <2.0"
}
```

This manifest lets the host validate plugins before loading them, resolve dependencies between plugins, and present a clear catalog of available capabilities to users.

## Key Takeaways

- The **microkernel** pattern separates the core from plugins and is the go-to for user-extensible systems, though it adds complexity for lifecycle management and versioning.
- **Pipes-and-filters** excels at composable data transformations (message processing, content filtering) but is not a general-purpose plugin architecture.
- The **modular monolith** is Rust's natural fit and gives you maximum type safety and performance, at the cost of requiring recompilation for new capabilities.
- Production agents typically use a **hybrid**: modular monolith for core features, microkernel for third-party extensions, and MCP as the standard protocol for process-level plugins.
- A clear **plugin contract** (manifest, capabilities, dependencies) is essential regardless of which architecture pattern you choose.
