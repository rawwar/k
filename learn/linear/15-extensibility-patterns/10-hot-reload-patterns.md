---
title: Hot Reload Patterns
description: Implement hot reload for plugins and configuration so developers can iterate on extensions without restarting the agent.
---

# Hot Reload Patterns

> **What you'll learn:**
> - How to implement file-watching and reload triggers that detect changes to plugin code, configuration, or MCP server definitions
> - Techniques for safely unloading and reloading plugins at runtime, including state migration and graceful cleanup of old plugin instances
> - The challenges of hot reload in Rust (shared library reloading, type identity across reloads) and practical workarounds

When you are developing an extension -- building a new MCP server, tweaking a hook configuration, or adjusting skill prompts -- restarting the agent for every change kills your flow. Hot reload lets the agent detect changes to configuration files, MCP servers, or plugins and apply them without dropping the current session. The user's conversation continues uninterrupted while the extension ecosystem updates around it.

Hot reload is one of those features that sounds simple but hides surprising complexity, especially in Rust. Let's work through the practical approaches, starting with the easiest wins and progressing to the harder problems.

## Watching for File Changes

The foundation of hot reload is file watching. The `notify` crate provides cross-platform file system event monitoring:

```rust
use notify::{Watcher, RecursiveMode, Event, EventKind};
use std::path::PathBuf;
use tokio::sync::mpsc;

pub struct ConfigWatcher {
    _watcher: notify::RecommendedWatcher,
    events: mpsc::Receiver<WatchEvent>,
}

#[derive(Debug)]
pub enum WatchEvent {
    ConfigChanged(PathBuf),
    McpServerChanged(String),
    PluginChanged(PathBuf),
}

impl ConfigWatcher {
    pub fn new(config_paths: &[PathBuf]) -> Result<Self> {
        let (tx, rx) = mpsc::channel(100);

        let sender = tx.clone();
        let mut watcher = notify::recommended_watcher(
            move |event: Result<Event, notify::Error>| {
                if let Ok(event) = event {
                    match event.kind {
                        EventKind::Modify(_) | EventKind::Create(_) => {
                            for path in &event.paths {
                                let watch_event = classify_change(path);
                                let _ = sender.blocking_send(watch_event);
                            }
                        }
                        _ => {}
                    }
                }
            },
        )?;

        // Watch all configured paths
        for path in config_paths {
            if path.exists() {
                watcher.watch(path, RecursiveMode::NonRecursive)?;
            }
        }

        Ok(Self {
            _watcher: watcher,
            events: rx,
        })
    }

    /// Poll for the next file change event.
    pub async fn next_event(&mut self) -> Option<WatchEvent> {
        self.events.recv().await
    }
}

fn classify_change(path: &std::path::Path) -> WatchEvent {
    let filename = path.file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    if filename.ends_with("config.toml") || filename.ends_with("settings.json") {
        WatchEvent::ConfigChanged(path.to_path_buf())
    } else if filename.ends_with(".so") || filename.ends_with(".dylib") || filename.ends_with(".dll") {
        WatchEvent::PluginChanged(path.to_path_buf())
    } else {
        WatchEvent::ConfigChanged(path.to_path_buf())
    }
}
```

## Hot Reloading Configuration

Configuration is the easiest thing to hot reload because it is pure data -- there is no code to unload, no state to migrate. When the config file changes, re-read it, validate it, and apply the differences:

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct HotReloadManager {
    current_config: Arc<RwLock<AgentConfig>>,
    watcher: ConfigWatcher,
}

impl HotReloadManager {
    /// Run the hot reload loop in a background task.
    pub async fn run(
        mut self,
        mcp_manager: Arc<McpServerManager>,
        hook_registry: Arc<HookRegistry>,
    ) {
        loop {
            match self.watcher.next_event().await {
                Some(WatchEvent::ConfigChanged(path)) => {
                    println!("[reload] Config changed: {}", path.display());
                    self.reload_config(&mcp_manager, &hook_registry).await;
                }
                Some(WatchEvent::McpServerChanged(name)) => {
                    println!("[reload] MCP server changed: {name}");
                    self.reload_mcp_server(&name, &mcp_manager).await;
                }
                Some(WatchEvent::PluginChanged(path)) => {
                    println!("[reload] Plugin changed: {}", path.display());
                    // Plugin reloading is more complex; see below
                }
                None => break, // Watcher dropped
            }
        }
    }

    async fn reload_config(
        &self,
        mcp_manager: &McpServerManager,
        hook_registry: &HookRegistry,
    ) {
        // Load the new config
        let new_config = match AgentConfig::load() {
            Ok(config) => config,
            Err(e) => {
                eprintln!("[reload] Failed to parse config: {e}");
                return;
            }
        };

        let old_config = self.current_config.read().await.clone();

        // Diff MCP servers: disconnect removed, connect added
        self.diff_and_apply_mcp_servers(
            &old_config, &new_config, mcp_manager,
        ).await;

        // Update the stored config
        *self.current_config.write().await = new_config;

        println!("[reload] Configuration reloaded successfully");
    }

    async fn diff_and_apply_mcp_servers(
        &self,
        old: &AgentConfig,
        new: &AgentConfig,
        mcp_manager: &McpServerManager,
    ) {
        // Find removed servers
        for name in old.mcp_servers.keys() {
            if !new.mcp_servers.contains_key(name) {
                println!("[reload] Disconnecting removed MCP server: {name}");
                let _ = mcp_manager.disconnect(name).await;
            }
        }

        // Find added or changed servers
        for (name, new_entry) in &new.mcp_servers {
            let should_reconnect = match old.mcp_servers.get(name) {
                None => true, // New server
                Some(old_entry) => {
                    // Changed if command, args, or env differ
                    old_entry.command != new_entry.command
                        || old_entry.args != new_entry.args
                        || old_entry.env != new_entry.env
                }
            };

            if should_reconnect && new_entry.enabled {
                println!("[reload] (Re)connecting MCP server: {name}");
                let _ = mcp_manager.disconnect(name).await;

                let mcp_config = McpServerConfig {
                    name: name.clone(),
                    command: new_entry.command.clone(),
                    args: new_entry.args.clone(),
                    env: new_entry.env.clone(),
                    init_timeout_secs: 30,
                };

                match mcp_manager.connect(mcp_config).await {
                    Ok(tools) => {
                        println!(
                            "[reload] MCP server '{name}' reconnected: {} tools",
                            tools.len()
                        );
                    }
                    Err(e) => {
                        eprintln!("[reload] Failed to reconnect '{name}': {e}");
                    }
                }
            }
        }
    }

    async fn reload_mcp_server(
        &self,
        server_name: &str,
        mcp_manager: &McpServerManager,
    ) {
        let config = self.current_config.read().await;
        if let Some(entry) = config.mcp_servers.get(server_name) {
            let _ = mcp_manager.disconnect(server_name).await;
            let mcp_config = McpServerConfig {
                name: server_name.to_string(),
                command: entry.command.clone(),
                args: entry.args.clone(),
                env: entry.env.clone(),
                init_timeout_secs: 30,
            };
            let _ = mcp_manager.connect(mcp_config).await;
        }
    }
}
```

::: python Coming from Python
Python's dynamic nature makes hot reload relatively straightforward:
```python
import importlib

def reload_plugin(module_name):
    module = importlib.import_module(module_name)
    importlib.reload(module)  # Re-executes the module file
    return module.create_plugin()
```
Python can reload modules because code is interpreted at runtime. Rust compiles to native code, so "reloading" a module within the same process is not possible without dynamic linking. This is why Rust hot reload typically operates at the configuration or process level rather than the code level.
:::

## Hot Reloading MCP Servers

MCP servers are the sweet spot for hot reload in Rust agents. Since MCP servers run as separate processes, "reloading" a server just means killing the old process and spawning a new one. No code within the agent process needs to change:

```rust
impl McpServerManager {
    /// Reload an MCP server: disconnect, respawn, and rediscover tools.
    pub async fn reload_server(&self, server_name: &str) -> Result<Vec<McpToolDefinition>> {
        let config = {
            let connections = self.connections.read().await;
            connections
                .get(server_name)
                .map(|c| c.config.clone())
                .ok_or_else(|| anyhow!("Server '{}' not found", server_name))?
        };

        // Disconnect the old instance
        self.disconnect(server_name).await?;

        // Small delay to let the process fully exit
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Reconnect with the same config
        let tools = self.connect(config).await?;

        println!(
            "[reload] MCP server '{}' reloaded: {} tools available",
            server_name,
            tools.len()
        );

        Ok(tools)
    }
}
```

This is particularly useful when developing an MCP server. You edit the server code, rebuild it, and the agent picks up the new version automatically.

## The Challenges of Dynamic Library Hot Reload

Hot reloading shared libraries (`.so` / `.dylib` files) in Rust is technically possible but fraught with danger. Here are the challenges:

**Type identity**: When you reload a library, Rust considers the types from the old and new libraries as different types, even if they have the same name and layout. An `Arc<dyn Plugin>` from the old library is not compatible with the new library's `Plugin` trait.

**Dangling references**: If any part of the agent holds a reference to data allocated by the old library, unloading the library causes a use-after-free crash.

**Static state**: If the plugin has `static` variables, unloading the library deallocates them, potentially causing crashes in other threads that still reference them.

If you need dynamic library hot reload despite these challenges, the safest approach is the "swap-and-restart" pattern:

```rust
/// Safe-ish dynamic library reload: fully shut down the old plugin
/// before loading the new one. No overlapping lifetimes.
pub async fn reload_dynamic_plugin(
    plugin_manager: &mut PluginManager,
    plugin_name: &str,
    library_path: &std::path::Path,
) -> Result<()> {
    // Phase 1: Shut down the old plugin completely.
    // This must drain all pending operations and drop all references.
    plugin_manager.shutdown_plugin(plugin_name).await?;

    // Phase 2: Remove the old plugin from all registries.
    // No references to old plugin code should exist after this.
    plugin_manager.remove_plugin(plugin_name).await?;

    // Phase 3: Load the new library.
    // This is safe because nothing references the old library.
    let new_plugin = load_dynamic_plugin(library_path)?;

    // Phase 4: Initialize the new plugin.
    plugin_manager.add_and_init_plugin(new_plugin).await?;

    Ok(())
}
```

::: wild In the Wild
Most production Rust projects avoid dynamic library hot reload entirely. Instead, they focus on hot reloading configuration and subprocess-based extensions. The `hot-lib-reloader` crate exists for development-time hot reload of Rust libraries, but it is explicitly not recommended for production use. The practical approach is: use hot reload for config and MCP servers (which are easy and safe), and accept a restart for changes to the core binary.
:::

## Debouncing File Events

File system events often arrive in bursts -- a single save operation might trigger multiple modify events. Without debouncing, you would reload the config several times for one edit. A simple debounce mechanism waits for a quiet period before triggering a reload:

```rust
use std::time::Duration;
use tokio::time::Instant;

pub struct DebouncedWatcher {
    watcher: ConfigWatcher,
    debounce_duration: Duration,
}

impl DebouncedWatcher {
    pub fn new(watcher: ConfigWatcher, debounce: Duration) -> Self {
        Self {
            watcher,
            debounce_duration: debounce,
        }
    }

    /// Wait for a debounced event. Collapses rapid-fire events into one.
    pub async fn next_debounced_event(&mut self) -> Option<WatchEvent> {
        // Wait for the first event
        let first_event = self.watcher.next_event().await?;
        let mut last_event = first_event;

        // Keep consuming events until there is a quiet period
        loop {
            match tokio::time::timeout(
                self.debounce_duration,
                self.watcher.next_event(),
            ).await {
                Ok(Some(event)) => {
                    // Another event arrived within the debounce window;
                    // reset the timer
                    last_event = event;
                }
                Ok(None) => return None, // Channel closed
                Err(_) => {
                    // Timeout: no more events within the debounce window
                    return Some(last_event);
                }
            }
        }
    }
}
```

## Notifying the User

When a hot reload occurs, the user should know about it. Integrate reload notifications into your UI:

```rust
pub enum ReloadNotification {
    ConfigReloaded { changes: Vec<String> },
    McpServerReloaded { server: String, tool_count: usize },
    McpServerDisconnected { server: String, reason: String },
    ReloadFailed { source: String, error: String },
}

/// Format a reload notification for the terminal UI.
pub fn format_reload_notification(notification: &ReloadNotification) -> String {
    match notification {
        ReloadNotification::ConfigReloaded { changes } => {
            format!(
                "[Hot Reload] Configuration updated: {}",
                changes.join(", ")
            )
        }
        ReloadNotification::McpServerReloaded { server, tool_count } => {
            format!(
                "[Hot Reload] MCP server '{server}' reloaded ({tool_count} tools)"
            )
        }
        ReloadNotification::McpServerDisconnected { server, reason } => {
            format!(
                "[Hot Reload] MCP server '{server}' disconnected: {reason}"
            )
        }
        ReloadNotification::ReloadFailed { source, error } => {
            format!(
                "[Hot Reload] Failed to reload {source}: {error}"
            )
        }
    }
}
```

## Key Takeaways

- **Configuration hot reload** is the easiest and most impactful: watch config files for changes, diff the old and new configs, and apply only the differences.
- **MCP server hot reload** is straightforward because servers run as separate processes -- disconnect the old process and spawn a new one with no in-process state to migrate.
- **Dynamic library hot reload** in Rust is technically possible but dangerous due to type identity, dangling references, and static state issues -- avoid it in production.
- **Debouncing** file system events prevents redundant reloads when editors write files in multiple steps (save triggers several file system events).
- The practical hot reload strategy for Rust agents is: **hot reload configuration and subprocess plugins, restart for core binary changes**.
