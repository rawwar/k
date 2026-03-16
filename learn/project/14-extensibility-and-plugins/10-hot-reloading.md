---
title: Hot Reloading
description: Implementing hot-reloading for plugins and configurations so that developers can iterate on extensions without restarting the agent or losing session state.
---

# Hot Reloading

> **What you'll learn:**
> - How to watch plugin files and configurations for changes using filesystem notifications
> - Techniques for safely unloading and reloading plugins without corrupting agent state
> - How to preserve session context across plugin reloads for a seamless development experience

When developing plugins or tuning configuration, restarting the agent for every change is painful. You lose your conversation context, MCP connections need to reconnect, and the feedback loop becomes slow. Hot reloading solves this by watching for file changes and applying them without restarting. The agent stays running, the session continues, and your changes take effect immediately.

## File Watching with notify

The `notify` crate provides cross-platform filesystem watching. It wraps platform-specific APIs (FSEvents on macOS, inotify on Linux, ReadDirectoryChanges on Windows) behind a unified interface:

```rust
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Watches files and directories for changes, sending events through a channel.
pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    rx: mpsc::Receiver<FileChangeEvent>,
    watched_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct FileChangeEvent {
    pub path: PathBuf,
    pub kind: FileChangeKind,
}

#[derive(Debug, Clone)]
pub enum FileChangeKind {
    Created,
    Modified,
    Removed,
}

impl FileWatcher {
    pub fn new() -> Result<Self, notify::Error> {
        let (tx, rx) = mpsc::channel(100);

        let watcher = RecommendedWatcher::new(
            move |result: Result<Event, notify::Error>| {
                if let Ok(event) = result {
                    let kind = match event.kind {
                        EventKind::Create(_) => Some(FileChangeKind::Created),
                        EventKind::Modify(_) => Some(FileChangeKind::Modified),
                        EventKind::Remove(_) => Some(FileChangeKind::Removed),
                        _ => None,
                    };

                    if let Some(change_kind) = kind {
                        for path in event.paths {
                            let _ = tx.blocking_send(FileChangeEvent {
                                path,
                                kind: change_kind.clone(),
                            });
                        }
                    }
                }
            },
            Config::default()
                .with_poll_interval(Duration::from_secs(2)),
        )?;

        Ok(Self {
            _watcher: watcher,
            rx,
            watched_paths: Vec::new(),
        })
    }

    /// Start watching a path for changes.
    pub fn watch(&mut self, path: &Path) -> Result<(), notify::Error> {
        self._watcher.watch(path, RecursiveMode::NonRecursive)?;
        self.watched_paths.push(path.to_path_buf());
        println!("[hot-reload] Watching: {}", path.display());
        Ok(())
    }

    /// Receive the next file change event.
    pub async fn next_event(&mut self) -> Option<FileChangeEvent> {
        self.rx.recv().await
    }
}
```

::: tip Coming from Python
Python developers often use `watchdog` for file watching:
```python
from watchdog.observers import Observer
from watchdog.events import FileSystemEventHandler

class ReloadHandler(FileSystemEventHandler):
    def on_modified(self, event):
        if event.src_path.endswith('.toml'):
            print(f"Config changed: {event.src_path}")
            reload_config()

observer = Observer()
observer.schedule(ReloadHandler(), path='.', recursive=False)
observer.start()
```
Rust's `notify` crate serves the same purpose. The key difference is that `notify` sends events through a callback (which we bridge to a `tokio::mpsc` channel), while `watchdog` uses a handler class. Both abstract away platform-specific APIs.
:::

## The Reload Manager

The reload manager coordinates the entire hot-reload cycle: detect changes, determine what is affected, safely unload the old version, load the new version, and restore state:

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ReloadManager {
    config_path: PathBuf,
    skill_dir: Option<PathBuf>,
    file_watcher: FileWatcher,
    tool_registry: Arc<RwLock<ToolRegistry>>,
    hook_registry: Arc<RwLock<HookRegistry>>,
    skill_loader: Arc<RwLock<SkillLoader>>,
    /// Track which config-defined tools are currently registered.
    config_tools: Vec<String>,
    /// Debounce timer to avoid reacting to rapid successive writes.
    last_reload: std::time::Instant,
}

impl ReloadManager {
    pub fn new(
        config_path: PathBuf,
        skill_dir: Option<PathBuf>,
        tool_registry: Arc<RwLock<ToolRegistry>>,
        hook_registry: Arc<RwLock<HookRegistry>>,
        skill_loader: Arc<RwLock<SkillLoader>>,
    ) -> Result<Self, anyhow::Error> {
        let mut file_watcher = FileWatcher::new()?;

        // Watch the config file
        if config_path.exists() {
            file_watcher.watch(&config_path)?;
        }

        // Watch the skill directory
        if let Some(ref dir) = skill_dir {
            if dir.exists() {
                file_watcher.watch(dir)?;
            }
        }

        Ok(Self {
            config_path,
            skill_dir,
            file_watcher,
            tool_registry,
            hook_registry,
            skill_loader,
            config_tools: Vec::new(),
            last_reload: std::time::Instant::now(),
        })
    }

    /// Run the hot-reload loop as a background task.
    pub async fn run(mut self) {
        println!("[hot-reload] Watching for changes...");

        while let Some(event) = self.file_watcher.next_event().await {
            // Debounce: ignore events within 500ms of the last reload
            if self.last_reload.elapsed() < Duration::from_millis(500) {
                continue;
            }

            println!(
                "[hot-reload] Change detected: {:?} ({:?})",
                event.path, event.kind
            );

            // Determine what to reload based on the changed file
            if event.path == self.config_path {
                if let Err(e) = self.reload_config().await {
                    eprintln!("[hot-reload] Config reload failed: {}", e);
                }
            } else if event
                .path
                .extension()
                .map_or(false, |ext| ext == "toml")
            {
                // Skill definition changed
                if let Err(e) = self.reload_skills().await {
                    eprintln!("[hot-reload] Skill reload failed: {}", e);
                }
            }

            self.last_reload = std::time::Instant::now();
        }
    }

    /// Reload the main configuration file.
    async fn reload_config(&mut self) -> Result<(), anyhow::Error> {
        println!("[hot-reload] Reloading configuration...");

        // Load the new config
        let new_config = ConfigLoader::load(&self.config_path)?;

        // Remove old config-defined tools
        {
            let mut registry = self.tool_registry.write().await;
            registry.deregister_all_by_owner("config");
        }

        // Register new config-defined tools
        {
            let mut registry = self.tool_registry.write().await;
            let mut new_tool_names = Vec::new();

            for (name, tool_config) in &new_config.tools {
                let (definition, handler) = create_config_tool_handler(
                    name.clone(),
                    tool_config.clone(),
                );
                match registry.register("config", definition, handler) {
                    Ok(()) => {
                        new_tool_names.push(name.clone());
                    }
                    Err(e) => {
                        eprintln!(
                            "[hot-reload] Could not register tool '{}': {}",
                            name, e
                        );
                    }
                }
            }

            self.config_tools = new_tool_names;
        }

        // Reload hooks
        {
            let mut hooks = self.hook_registry.write().await;
            hooks.deregister_all_by_owner("config");
            // Re-register hooks from new config
            register_config_hooks(&new_config.hooks, &mut hooks);
        }

        println!(
            "[hot-reload] Config reloaded: {} tools registered",
            self.config_tools.len()
        );

        Ok(())
    }

    /// Reload skill definitions from the skill directory.
    async fn reload_skills(&mut self) -> Result<(), anyhow::Error> {
        let skill_dir = match &self.skill_dir {
            Some(dir) => dir.clone(),
            None => return Ok(()),
        };

        println!("[hot-reload] Reloading skills from {:?}...", skill_dir);

        let mut loader = self.skill_loader.write().await;

        // Get currently active skills before reload
        let active_skills: Vec<String> = loader
            .list_skills()
            .into_iter()
            .filter(|s| s.active)
            .map(|s| s.name)
            .collect();

        // Deactivate all active skills
        for name in &active_skills {
            if let Err(e) = loader.deactivate(name).await {
                eprintln!("[hot-reload] Could not deactivate skill '{}': {}", name, e);
            }
        }

        // Reload skill definitions from disk
        loader.load_skills_from_dir(&skill_dir)?;

        // Re-activate previously active skills
        for name in &active_skills {
            match loader.activate(name).await {
                Ok(()) => println!("[hot-reload] Re-activated skill '{}'", name),
                Err(e) => eprintln!(
                    "[hot-reload] Could not re-activate skill '{}': {}",
                    name, e
                ),
            }
        }

        Ok(())
    }
}

/// Register shell-based hooks from configuration.
fn register_config_hooks(
    config: &HookConfigs,
    registry: &mut HookRegistry,
) {
    for hook_config in &config.pre_tool_use {
        let cmd = hook_config.command.clone();
        let matcher = hook_config.matcher.clone();
        let timeout = hook_config.timeout_secs;

        registry.register(
            "config",
            HookPoint::PreToolUse,
            100, // Standard priority for config hooks
            Arc::new(move |ctx: HookContext| {
                let cmd = cmd.clone();
                let matcher = matcher.clone();
                Box::pin(async move {
                    // Check if the tool matches
                    if let Some(ref pattern) = matcher {
                        if let Some(ref tool_name) = ctx.tool_name {
                            if !tool_name.contains(pattern) {
                                return HookAction::Continue(ctx);
                            }
                        }
                    }

                    // Execute the hook command
                    match run_shell_hook(&cmd, &ctx, timeout).await {
                        Ok(true) => HookAction::Continue(ctx),
                        Ok(false) => HookAction::Abort(
                            "Blocked by config hook".to_string()
                        ),
                        Err(e) => {
                            eprintln!("[hook] Config hook error: {}", e);
                            HookAction::Continue(ctx) // Fail open
                        }
                    }
                })
            }),
        );
    }
}

/// Execute a shell hook command. Returns true if the hook approves (exit 0).
async fn run_shell_hook(
    command: &str,
    context: &HookContext,
    timeout_secs: u64,
) -> Result<bool, anyhow::Error> {
    use tokio::process::Command;
    use tokio::io::AsyncWriteExt;

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    // Pass context to the hook via stdin
    if let Some(mut stdin) = child.stdin.take() {
        let context_json = serde_json::to_string(context)?;
        stdin.write_all(context_json.as_bytes()).await?;
        drop(stdin); // Close stdin so the hook process knows we are done
    }

    let output = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        child.wait(),
    )
    .await??;

    Ok(output.success())
}
```

## Launching Hot Reload as a Background Task

Start the reload manager alongside the agent:

```rust
pub async fn start_agent_with_hot_reload(
    config_path: PathBuf,
    skill_dir: Option<PathBuf>,
) -> Result<(), anyhow::Error> {
    let tool_registry = Arc::new(RwLock::new(ToolRegistry::new()));
    let hook_registry = Arc::new(RwLock::new(HookRegistry::new()));
    let skill_loader = Arc::new(RwLock::new(
        SkillLoader::new(tool_registry.clone()),
    ));

    // Load initial configuration
    let config = ConfigLoader::load(&config_path)?;
    // Apply initial config...

    // Start the hot-reload watcher in a background task
    let reload_manager = ReloadManager::new(
        config_path,
        skill_dir,
        tool_registry.clone(),
        hook_registry.clone(),
        skill_loader.clone(),
    )?;

    tokio::spawn(async move {
        reload_manager.run().await;
    });

    // Run the main agent loop
    // The agent uses the same Arc<RwLock<_>> references,
    // so it sees changes made by the reload manager.
    println!("[agent] Running with hot-reload enabled");

    // ... main agent loop ...

    Ok(())
}
```

The `Arc<RwLock<_>>` pattern is central here. Both the agent loop and the reload manager hold references to the same registries. When the reload manager updates a registry, the agent sees the changes on its next read.

## Debouncing and Safety

File-change events can be noisy. A single save in an editor might trigger multiple events (write, metadata change, rename). Debouncing collapses rapid successive events into a single reload:

```rust
use std::time::{Duration, Instant};

pub struct Debouncer {
    delay: Duration,
    last_event: Option<Instant>,
    pending_paths: Vec<PathBuf>,
}

impl Debouncer {
    pub fn new(delay: Duration) -> Self {
        Self {
            delay,
            last_event: None,
            pending_paths: Vec::new(),
        }
    }

    /// Record a file change event. Returns true if a reload should trigger.
    pub fn event(&mut self, path: PathBuf) -> bool {
        let now = Instant::now();

        if !self.pending_paths.contains(&path) {
            self.pending_paths.push(path);
        }

        match self.last_event {
            Some(last) if now.duration_since(last) < self.delay => {
                // Within debounce window -- accumulate
                self.last_event = Some(now);
                false
            }
            _ => {
                // Debounce window expired or first event
                self.last_event = Some(now);
                true
            }
        }
    }

    /// Drain the accumulated paths, returning them for processing.
    pub fn drain(&mut self) -> Vec<PathBuf> {
        std::mem::take(&mut self.pending_paths)
    }
}
```

::: info In the Wild
Claude Code does not implement hot-reloading in the traditional sense -- configuration changes require restarting the session. However, the `/init` command regenerates the project configuration file (CLAUDE.md) on the fly, and MCP server connections can be re-established mid-session. For development workflows, some coding agents take the approach of watching `CLAUDE.md` or equivalent project files and re-reading them between conversation turns rather than doing true filesystem-watch-based hot reload.
:::

## What Can and Cannot Be Hot-Reloaded

Not everything is safe to reload mid-session:

| Component | Hot-reloadable? | Notes |
|-----------|----------------|-------|
| Config-defined tools | Yes | Deregister old, register new |
| Skill definitions | Yes | Deactivate, reload, re-activate |
| System prompt additions | Yes | Rebuilt on each LLM call |
| Hook configurations | Yes | Deregister old, register new |
| MCP servers | Partially | New servers can connect; existing connections persist |
| Core agent code | No | Requires recompilation and restart |
| Conversation history | Preserved | Not affected by plugin reloads |

The key principle: anything behind an `Arc<RwLock<_>>` can be updated without restarting. Compiled code cannot change at runtime. This is why the config-driven extension approach is so valuable -- it moves as much as possible out of compiled code into reloadable configuration.

## Key Takeaways

- The `notify` crate provides cross-platform filesystem watching, which you bridge to async Rust through a `tokio::mpsc` channel for seamless integration with the agent's async runtime
- Hot reloading follows a safe cycle: detect change, deregister old extensions, load new definitions, re-register extensions, re-activate previously active skills
- Debouncing prevents redundant reloads when editors trigger multiple filesystem events for a single save operation
- `Arc<RwLock<_>>` is the enabling pattern -- both the agent loop and the reload manager share references to the same registries, making changes visible without restart
- Not everything can be hot-reloaded: config-defined tools, skills, and hooks reload smoothly, but compiled code changes require a full restart
