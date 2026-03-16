---
title: Loading Strategies
description: Implement plugin discovery and loading mechanisms including static registration, dynamic library loading, and subprocess-based plugins.
---

# Loading Strategies

> **What you'll learn:**
> - How to implement static plugin registration using Rust's inventory or linkme crates for compile-time plugin discovery
> - Techniques for dynamic loading of shared libraries at runtime using libloading, including ABI stability concerns
> - How to implement subprocess-based plugin loading where plugins run as separate processes communicating over stdin/stdout or sockets

You have defined what a plugin looks like with the `Plugin` trait. Now you need to answer the practical question: how does a plugin get from "exists somewhere on disk" to "running inside the agent"? This is the loading strategy, and it determines everything from your development workflow to your security model.

There are three main approaches: embedded (compile-time), config-based (runtime shared libraries), and discovery (subprocess). Each has distinct strengths, and most production agents combine at least two of them.

## Strategy 1: Embedded Plugins (Compile-Time Registration)

The simplest loading strategy is to compile plugins directly into the binary. Rust's module system already gives you this for free with `mod` and `use`. But for a more plugin-like experience -- where plugin authors register themselves without modifying a central dispatch table -- you can use the `inventory` or `linkme` crates.

The `inventory` crate lets plugins register themselves at link time. You declare a registerable type, and any crate in the dependency tree can submit instances of it. At runtime, you iterate over all submitted items:

```rust
use inventory;

/// A tool descriptor that plugins submit at compile time.
pub struct ToolDescriptor {
    pub name: &'static str,
    pub description: &'static str,
    pub factory: fn() -> Box<dyn Tool>,
}

// Tell inventory this type can be collected
inventory::collect!(ToolDescriptor);

// In the file_read plugin (could be a separate crate):
inventory::submit! {
    ToolDescriptor {
        name: "read_file",
        description: "Read the contents of a file",
        factory: || Box::new(FileReadTool::new()),
    }
}

// In the shell plugin:
inventory::submit! {
    ToolDescriptor {
        name: "shell",
        description: "Execute a shell command",
        factory: || Box::new(ShellTool::new()),
    }
}

// In the host, at startup:
fn load_embedded_plugins(registry: &mut ToolRegistry) {
    for descriptor in inventory::iter::<ToolDescriptor> {
        let tool = (descriptor.factory)();
        println!("Registered built-in tool: {}", descriptor.name);
        registry.register(tool);
    }
}
```

The key advantage here is that adding a new tool requires only adding a dependency and an `inventory::submit!` block -- no modification of the host's dispatch logic. The compiler checks everything, and the resulting binary has zero runtime loading overhead.

::: python Coming from Python
Python's equivalent is the entry-point system in `setuptools` / `importlib.metadata`:
```python
# Plugin package's pyproject.toml
[project.entry-points."agent.tools"]
read_file = "file_tools:FileReadTool"

# Host discovers all installed plugins
for ep in importlib.metadata.entry_points(group="agent.tools"):
    tool_class = ep.load()  # Dynamic import
    registry.register(tool_class())
```
Rust's `inventory` achieves the same decoupled registration but resolves everything at link time rather than at runtime. There is no dynamic import, no `ImportError` at runtime -- if it compiles, it is registered.
:::

## Strategy 2: Dynamic Loading (Shared Libraries)

When you need users to load plugins without recompiling the agent, dynamic loading through shared libraries (`.so` on Linux, `.dylib` on macOS, `.dll` on Windows) is the traditional approach. Rust can produce and consume these through the `libloading` crate.

This is the most powerful and the most dangerous strategy. You get full runtime extensibility, but you lose Rust's compile-time safety at the library boundary.

```rust
use libloading::{Library, Symbol};
use std::path::Path;

/// The C-compatible function signature that every plugin DLL must export.
/// Using `extern "C"` ensures a stable ABI across compiler versions.
type CreatePluginFn = unsafe extern "C" fn() -> *mut dyn Plugin;

/// Holds a loaded dynamic plugin and its library handle.
/// The library must outlive the plugin -- dropping it unloads the code.
pub struct DynamicPlugin {
    _library: Library,  // Must keep this alive
    plugin: Box<dyn Plugin>,
}

/// Load a plugin from a shared library file.
///
/// # Safety
/// The shared library must export a `create_plugin` function with the
/// correct signature. Loading arbitrary libraries is inherently unsafe.
pub fn load_dynamic_plugin(path: &Path) -> Result<DynamicPlugin> {
    unsafe {
        let library = Library::new(path)
            .map_err(|e| anyhow!("Failed to load {}: {e}", path.display()))?;

        let create_fn: Symbol<CreatePluginFn> = library
            .get(b"create_plugin")
            .map_err(|e| anyhow!("Plugin missing create_plugin symbol: {e}"))?;

        let raw_plugin = create_fn();
        let plugin = Box::from_raw(raw_plugin);

        Ok(DynamicPlugin {
            _library: library,
            plugin,
        })
    }
}
```

The plugin side needs to export the creation function with `extern "C"`:

```rust
// In the plugin crate (compiled as cdylib)

pub struct MyCustomTool;

impl Plugin for MyCustomTool {
    fn name(&self) -> &str { "custom_tool" }
    fn version(&self) -> &str { "0.1.0" }

    fn init(&self, ctx: &mut PluginContext) -> Result<()> {
        // Register tools, hooks, event handlers...
        Ok(())
    }

    fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

/// The entry point called by the host to create the plugin.
/// Must use `extern "C"` for ABI stability and `#[no_mangle]`
/// so the host can find the symbol by name.
#[no_mangle]
pub extern "C" fn create_plugin() -> *mut dyn Plugin {
    let plugin = MyCustomTool;
    Box::into_raw(Box::new(plugin))
}
```

The plugin's `Cargo.toml` must specify the `cdylib` crate type:

```toml
[lib]
crate-type = ["cdylib"]
```

### ABI Stability Pitfalls

The biggest challenge with dynamic loading in Rust is ABI stability. Rust does not guarantee a stable ABI -- the memory layout of structs, vtable layout of trait objects, and even `String` representation can change between compiler versions. This means:

- The plugin and host **must** be compiled with the same Rust compiler version.
- The shared plugin trait crate **must** use the exact same version in both host and plugin.
- `extern "C"` functions should pass only C-compatible types at the boundary, converting to Rust types inside.

For these reasons, many Rust projects avoid dynamic loading entirely in favor of the subprocess approach.

## Strategy 3: Subprocess Plugins (Process-Level Isolation)

The subprocess strategy runs each plugin as a separate process. The host communicates with plugins over stdin/stdout, Unix sockets, or TCP connections using a defined protocol (often JSON-RPC or a custom protocol). This is the foundation of MCP and the approach most modern agents favor.

```rust
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// A plugin that runs as a child process, communicating over stdin/stdout.
pub struct SubprocessPlugin {
    name: String,
    child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
}

impl SubprocessPlugin {
    pub async fn spawn(name: String, command: &str, args: &[&str]) -> Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.take()
            .ok_or_else(|| anyhow!("Failed to capture stdin"))?;
        let stdout = BufReader::new(
            child.stdout.take()
                .ok_or_else(|| anyhow!("Failed to capture stdout"))?
        );

        Ok(Self { name, child, stdin, stdout })
    }

    /// Send a JSON-RPC request to the plugin process.
    pub async fn send_request(&mut self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });

        let mut line = serde_json::to_string(&request)?;
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.flush().await?;

        let mut response_line = String::new();
        self.stdout.read_line(&mut response_line).await?;
        let response: serde_json::Value = serde_json::from_str(&response_line)?;

        Ok(response)
    }

    /// Gracefully shut down the plugin process.
    pub async fn shutdown(&mut self) -> Result<()> {
        let _ = self.send_request("shutdown", serde_json::json!({})).await;
        self.child.kill().await?;
        Ok(())
    }
}
```

::: wild In the Wild
MCP (the Model Context Protocol) is essentially a standardized version of this subprocess plugin pattern. When Claude Code connects to an MCP server, it spawns the server as a child process and communicates over stdin/stdout using JSON-RPC. This gives full process isolation -- an MCP server crash cannot take down the agent -- while the standardized protocol means any MCP server works with any MCP client. We will explore MCP in depth in the upcoming subchapters.
:::

## The Plugin Manager

Regardless of which loading strategies you support, you need a central plugin manager that handles the full lifecycle:

```rust
pub struct PluginManager {
    plugins: Vec<LoadedPlugin>,
    context: Arc<PluginContext>,
}

struct LoadedPlugin {
    manifest: PluginManifest,
    instance: PluginInstance,
    state: PluginState,
}

enum PluginInstance {
    Embedded(Box<dyn Plugin>),
    Dynamic(DynamicPlugin),
    Subprocess(SubprocessPlugin),
}

#[derive(Debug, PartialEq)]
enum PluginState {
    Loaded,
    Initialized,
    Running,
    Error(String),
    ShutDown,
}

impl PluginManager {
    pub async fn load_all(&mut self, config: &AgentConfig) -> Result<()> {
        // Phase 1: Load embedded plugins (always available)
        self.load_embedded_plugins();

        // Phase 2: Load dynamic libraries from configured paths
        for path in &config.plugin_library_paths {
            self.load_dynamic_plugin(path)?;
        }

        // Phase 3: Spawn subprocess plugins from config
        for plugin_config in &config.subprocess_plugins {
            self.spawn_subprocess_plugin(plugin_config).await?;
        }

        // Phase 4: Initialize all loaded plugins
        for plugin in &mut self.plugins {
            if plugin.state == PluginState::Loaded {
                // Each plugin gets a mutable context to register capabilities
                let mut ctx = PluginContext::from(self.context.clone());
                match &plugin.instance {
                    PluginInstance::Embedded(p) => p.init(&mut ctx)?,
                    PluginInstance::Dynamic(p) => p.plugin.init(&mut ctx)?,
                    PluginInstance::Subprocess(_) => {
                        // Subprocess plugins self-initialize; we just
                        // query their capabilities
                    }
                }
                plugin.state = PluginState::Initialized;
            }
        }

        Ok(())
    }
}
```

## Choosing Your Strategy

| Concern | Embedded | Dynamic (.so/.dll) | Subprocess |
|---------|----------|-------------------|------------|
| User can add without recompiling | No | Yes | Yes |
| Type safety | Full | At boundary only | Protocol-level only |
| Isolation | None (shares process) | None (shares process) | Full (separate process) |
| Performance | Direct function calls | Indirect (vtable) | IPC overhead (serialization) |
| ABI concerns | None | Major (compiler version) | None (uses protocol) |
| Language support | Rust only | C ABI languages | Any language |

Most coding agents settle on **embedded for core tools** and **subprocess for extensions**, with MCP as the subprocess protocol. This combination gives you the performance of compiled code for the tools you use constantly, and the flexibility of external processes for everything else.

## Key Takeaways

- **Embedded plugins** (using `inventory` or `linkme`) give you zero-overhead, type-safe registration but require recompilation to add new plugins.
- **Dynamic loading** via shared libraries provides runtime extensibility but introduces serious ABI stability challenges in Rust -- the host and plugin must use identical compiler versions and dependency versions.
- **Subprocess plugins** offer the best isolation and language flexibility, communicating over stdin/stdout or sockets using structured protocols like JSON-RPC.
- A **plugin manager** handles the full lifecycle (load, initialize, run, shutdown) and abstracts over the loading strategy so the rest of the agent does not need to know how a plugin was loaded.
- The emerging standard is **embedded for core + subprocess (MCP) for extensions**, giving you both performance and ecosystem compatibility.
