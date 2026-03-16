---
title: File Watching
description: Monitoring the file system for changes using OS-level notifications to enable live-reload and external change detection.
---

# File Watching

> **What you'll learn:**
> - How file system notification APIs (inotify, FSEvents, ReadDirectoryChanges) enable efficient change detection
> - How to use the notify crate in Rust to watch directories for changes and debounce rapid event sequences
> - Use cases for file watching in agents including detecting external edits and triggering re-analysis

File watching is the ability to be notified when files change on disk. Instead of repeatedly reading a file to check if it has been modified (polling), you ask the operating system to tell you when something changes. This is useful for a coding agent in several scenarios: detecting when the user edits a file outside the agent, triggering recompilation or test runs after changes, and keeping an in-memory cache of file contents synchronized with disk.

## OS-Level File Notification APIs

Each operating system provides its own file watching mechanism:

- **Linux**: `inotify` -- watches individual files and directories for events like create, modify, delete, and move. Efficient and well-supported, but has a per-user limit on the number of watches (typically 8,192 by default, configurable via `/proc/sys/fs/inotify/max_user_watches`).

- **macOS**: `FSEvents` -- watches directory trees rather than individual files. It batches events and delivers them with a configurable latency (typically 0.5-2 seconds). Less granular than inotify but scales to very large directory trees.

- **Windows**: `ReadDirectoryChangesW` -- watches directories for changes with configurable filters. Can watch recursively and supports overlapped I/O for async operation.

You don't need to interact with these APIs directly. The `notify` crate provides a cross-platform abstraction that uses the right backend for each OS.

## Using the Notify Crate

Here's a basic file watcher that prints events as they occur:

```rust
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

fn watch_directory(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = mpsc::channel::<Result<Event, notify::Error>>();

    // Create a watcher that sends events to our channel
    let mut watcher = RecommendedWatcher::new(
        move |result: Result<Event, notify::Error>| {
            let _ = tx.send(result);
        },
        Config::default(),
    )?;

    // Start watching the directory recursively
    watcher.watch(path, RecursiveMode::Recursive)?;

    println!("Watching {} for changes...", path.display());

    // Process events as they arrive
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Ok(event)) => {
                println!("Change detected: {:?}", event.kind);
                for path in &event.paths {
                    println!("  File: {}", path.display());
                }
            }
            Ok(Err(error)) => {
                eprintln!("Watch error: {error}");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No events in the last second, continue waiting
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    Ok(())
}
```

`RecommendedWatcher` automatically selects the best backend for your platform: `inotify` on Linux, `FSEvents` on macOS, `ReadDirectoryChanges` on Windows.

::: python Coming from Python
Python's `watchdog` library serves the same purpose as Rust's `notify` crate. If you've used `watchdog.observers.Observer` with a `FileSystemEventHandler`, the concepts map directly. Rust's channel-based approach replaces Python's callback-based handler pattern. The `notify` crate is more performant because it runs the event loop without the GIL, but the conceptual model is the same: register paths to watch, receive events when things change.
:::

## Debouncing Events

File system events are noisy. Saving a file in an editor might generate multiple events: a modify event when the editor writes the new content, another modify when it updates the metadata, possibly a rename event if the editor uses atomic saves (write to temp + rename). You don't want to trigger three actions for a single save.

Debouncing collects events over a short window and delivers them as a single batch:

```rust
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

struct DebouncedWatcher {
    rx: mpsc::Receiver<Result<Event, notify::Error>>,
    _watcher: RecommendedWatcher,
    debounce_duration: Duration,
}

impl DebouncedWatcher {
    fn new(
        path: &Path,
        debounce_ms: u64,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (tx, rx) = mpsc::channel();

        let mut watcher = RecommendedWatcher::new(
            move |result| {
                let _ = tx.send(result);
            },
            Config::default(),
        )?;

        watcher.watch(path, RecursiveMode::Recursive)?;

        Ok(Self {
            rx,
            _watcher: watcher,
            debounce_duration: Duration::from_millis(debounce_ms),
        })
    }

    fn next_batch(&self) -> Vec<PathBuf> {
        let mut changed_paths: HashSet<PathBuf> = HashSet::new();
        let mut last_event_time = Instant::now();

        loop {
            match self.rx.recv_timeout(self.debounce_duration) {
                Ok(Ok(event)) => {
                    for path in event.paths {
                        changed_paths.insert(path);
                    }
                    last_event_time = Instant::now();
                }
                Ok(Err(_)) => continue,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if !changed_paths.is_empty()
                        && last_event_time.elapsed() >= self.debounce_duration
                    {
                        return changed_paths.into_iter().collect();
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return changed_paths.into_iter().collect();
                }
            }
        }
    }
}
```

The debouncer collects paths into a `HashSet` (deduplicating multiple events for the same file) and waits until no new events arrive for the debounce duration before returning the batch. A 200-500ms debounce window works well for most editing workflows.

## Filtering Events

Not all file changes are interesting to a coding agent. You probably want to ignore:
- Hidden files and directories (`.git/`, `.DS_Store`)
- Build artifacts (`target/`, `node_modules/`, `__pycache__/`)
- Temporary files created by your own agent

```rust
use std::path::Path;

fn should_watch(path: &Path) -> bool {
    let path_str = path.to_string_lossy();

    // Ignore hidden directories and files
    for component in path.components() {
        let name = component.as_os_str().to_string_lossy();
        if name.starts_with('.') && name != "." && name != ".." {
            return false;
        }
    }

    // Ignore known build/dependency directories
    let ignore_dirs = [
        "target", "node_modules", "__pycache__",
        "dist", "build", ".next", "venv",
    ];
    for dir in &ignore_dirs {
        if path_str.contains(&format!("/{dir}/")) {
            return false;
        }
    }

    // Ignore common temporary and backup files
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if name.ends_with('~')
            || name.ends_with(".swp")
            || name.ends_with(".swo")
            || name.ends_with(".tmp")
        {
            return false;
        }
    }

    true
}
```

A more robust approach would read `.gitignore` patterns and apply them, but the simple ignore list above covers the common cases.

## Async File Watching

For an agent using Tokio, you'll want async file watching. The `notify` crate works with async by sending events through a Tokio channel:

```rust
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use tokio::sync::mpsc as tokio_mpsc;

async fn watch_async(
    path: &Path,
) -> Result<tokio_mpsc::Receiver<Vec<String>>, Box<dyn std::error::Error>> {
    let (notify_tx, mut notify_rx) = tokio_mpsc::channel::<Event>(100);

    let mut watcher = RecommendedWatcher::new(
        move |result: Result<Event, notify::Error>| {
            if let Ok(event) = result {
                let _ = notify_tx.blocking_send(event);
            }
        },
        Config::default(),
    )?;

    watcher.watch(path, RecursiveMode::Recursive)?;

    let (batch_tx, batch_rx) = tokio_mpsc::channel::<Vec<String>>(10);

    tokio::spawn(async move {
        let _watcher = watcher; // Keep watcher alive
        let mut pending: Vec<String> = Vec::new();

        loop {
            tokio::select! {
                Some(event) = notify_rx.recv() => {
                    for p in event.paths {
                        if let Some(s) = p.to_str() {
                            if !pending.contains(&s.to_string()) {
                                pending.push(s.to_string());
                            }
                        }
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(300)) => {
                    if !pending.is_empty() {
                        let batch = std::mem::take(&mut pending);
                        if batch_tx.send(batch).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });

    Ok(batch_rx)
}
```

This spawns a background task that collects file change events and delivers them in debounced batches through a Tokio channel. The rest of your async agent code can `recv` from the channel without blocking.

## Agent Use Cases

File watching in a coding agent enables several useful patterns:

1. **External edit detection**: If the user edits a file in their IDE while the agent is running, the agent can detect the change and re-read the file before its next edit. This prevents the agent from working with stale content.

2. **Build feedback**: After the agent modifies source files, watch for changes to build output files to determine if the build succeeded or failed without explicitly running the build command.

3. **Test re-run triggers**: Watch source files and automatically suggest re-running tests when the code under test changes.

4. **Session state invalidation**: If you cache file contents for context management, file watching tells you when the cache is stale.

::: wild In the Wild
Claude Code does not use file watching directly -- it re-reads files before each edit to ensure it has current content. This simpler approach avoids the complexity of maintaining a file watcher but means the agent might not notice external changes between reads. Some IDE-integrated agents like Cursor use file watching to keep their understanding of the codebase synchronized in real time.
:::

## Key Takeaways

- Use the `notify` crate for cross-platform file watching -- it abstracts over inotify (Linux), FSEvents (macOS), and ReadDirectoryChanges (Windows)
- Always debounce file system events -- a single file save can generate multiple events, and you want to process them as one batch
- Filter out hidden files, build artifacts, and temporary files to reduce noise from events your agent doesn't care about
- For async agents, bridge `notify` events into Tokio channels so the watcher integrates with your async event loop
- File watching is optional but enables useful patterns like external edit detection and build feedback in interactive agent sessions
