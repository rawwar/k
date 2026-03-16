---
title: Working Directory
description: Manage the working directory for command execution, enabling the agent to run commands in the correct project context.
---

# Working Directory

> **What you'll learn:**
> - How to set and validate the working directory before process execution
> - How to track and persist the current working directory across tool invocations
> - How to handle relative and absolute path resolution for command targets

When a developer types `cargo test` in their terminal, the command runs in whatever directory the terminal is currently in. Your agent needs to do the same thing -- run commands in the correct project directory. If the user is working on a Rust project at `/home/user/projects/my-app`, then `cargo test` must execute in that directory or it will fail with "could not find Cargo.toml."

## Setting the Working Directory

Tokio's `Command` (and the standard library's version) both support `.current_dir()` to set the working directory for a child process:

```rust
use tokio::process::Command as TokioCommand;

#[tokio::main]
async fn main() {
    let output = TokioCommand::new("pwd")
        .current_dir("/tmp")
        .output()
        .await
        .expect("failed to execute");

    println!("{}", String::from_utf8_lossy(&output.stdout));
    // Output: /tmp
}
```

The child process starts in `/tmp` regardless of where your agent binary is running. If you do not call `.current_dir()`, the child inherits the agent's working directory.

## Validating the Directory

Before setting a working directory, you should verify it exists and is actually a directory. Running a command in a nonexistent directory produces an OS error that is confusing to the LLM:

```rust
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

/// Validate and resolve a working directory path.
pub fn validate_working_dir(path: &str) -> Result<PathBuf> {
    let path = PathBuf::from(path);

    // Resolve to absolute path
    let resolved = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|e| anyhow!("Cannot get current directory: {}", e))?
            .join(path)
    };

    // Canonicalize to resolve symlinks and ../ components
    let canonical = resolved
        .canonicalize()
        .map_err(|e| anyhow!("Directory '{}' does not exist: {}", resolved.display(), e))?;

    if !canonical.is_dir() {
        return Err(anyhow!("'{}' is not a directory", canonical.display()));
    }

    Ok(canonical)
}

fn main() {
    match validate_working_dir("/tmp") {
        Ok(path) => println!("Valid directory: {}", path.display()),
        Err(e) => eprintln!("Invalid: {}", e),
    }

    match validate_working_dir("/nonexistent") {
        Ok(path) => println!("Valid directory: {}", path.display()),
        Err(e) => eprintln!("Invalid: {}", e),
    }
}
```

`canonicalize()` resolves symlinks and `../` components, giving you the real absolute path. This is important for security: without canonicalization, a path like `/safe/dir/../../etc/passwd` might bypass directory restrictions.

::: tip Coming from Python
Python's `subprocess.run()` also takes a `cwd` parameter:
```python
import subprocess
result = subprocess.run(["pwd"], cwd="/tmp", capture_output=True, text=True)
print(result.stdout)  # /tmp
```
The main difference is that Python raises `FileNotFoundError` if the directory does not exist, while Rust returns an `io::Error` that you must handle. In both cases, you should validate the directory before attempting to use it, rather than relying on the error from the process spawn.
:::

## Tracking Working Directory Across Tool Calls

An agent session might involve multiple tool calls. The LLM might run:

1. `cd /home/user/project` (conceptually -- `cd` does not actually work across process spawns)
2. `cargo test`
3. `cd src && cat main.rs`

Each tool call spawns a new process. Unlike a terminal where `cd` changes the shell's state, each process spawn starts fresh. You need to maintain working directory state in your agent:

```rust
use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Manages the working directory state for the agent's shell tool.
#[derive(Debug, Clone)]
pub struct WorkingDirManager {
    current_dir: Arc<Mutex<PathBuf>>,
    /// The root directory -- commands cannot escape above this.
    root_dir: PathBuf,
}

impl WorkingDirManager {
    /// Create a new manager rooted at the given directory.
    pub fn new(root: PathBuf) -> Self {
        Self {
            current_dir: Arc::new(Mutex::new(root.clone())),
            root_dir: root,
        }
    }

    /// Get the current working directory.
    pub async fn current(&self) -> PathBuf {
        self.current_dir.lock().await.clone()
    }

    /// Change the working directory. Returns an error if the target
    /// is outside the root directory or does not exist.
    pub async fn change_dir(&self, target: &str) -> Result<PathBuf> {
        let current = self.current_dir.lock().await.clone();

        let new_path = if target.starts_with('/') {
            PathBuf::from(target)
        } else {
            current.join(target)
        };

        // Resolve symlinks and ../
        let resolved = new_path
            .canonicalize()
            .map_err(|e| anyhow!("Cannot access '{}': {}", target, e))?;

        if !resolved.is_dir() {
            return Err(anyhow!("'{}' is not a directory", resolved.display()));
        }

        // Ensure the new path is within the root directory
        if !resolved.starts_with(&self.root_dir) {
            return Err(anyhow!(
                "Cannot change to '{}': outside project root '{}'",
                resolved.display(),
                self.root_dir.display()
            ));
        }

        *self.current_dir.lock().await = resolved.clone();
        Ok(resolved)
    }

    /// Get the root directory.
    pub fn root(&self) -> &PathBuf {
        &self.root_dir
    }
}
```

The `WorkingDirManager` uses `Arc<Mutex<PathBuf>>` because the agent might handle concurrent tool calls. The mutex ensures that directory changes are atomic.

Here is how the shell tool uses the manager:

```rust
use anyhow::Result;
use std::path::PathBuf;

// Assume ShellCommand is defined as in the command builder subchapter

impl WorkingDirManager {
    /// Execute a command in the current working directory.
    pub async fn execute_in_current_dir(&self, command: &str) -> Result<ShellOutput> {
        let cwd = self.current().await;

        let result = ShellCommand::new(command)
            .working_dir(cwd)
            .execute()
            .await?;

        Ok(result)
    }
}

// Usage in the tool dispatch:
async fn handle_shell_tool(
    manager: &WorkingDirManager,
    command: &str,
) -> Result<String> {
    let result = manager.execute_in_current_dir(command).await?;
    Ok(result.to_tool_result())
}
```

## Handling `cd` Commands

The LLM often generates `cd` commands because that is what humans type in terminals. But `cd` is a shell builtin that changes the shell's own state -- it does not affect the parent process or other processes. When the agent runs `sh -c "cd /tmp && pwd"`, the `pwd` sees `/tmp` because it runs in the same shell instance. But the next tool call starts a fresh process and is back to the original directory.

You have two options:

1. **Detect and intercept `cd`**: Parse the command, extract the directory change, and update the `WorkingDirManager` instead of (or in addition to) running the command.
2. **Prepend `cd` to every command**: Always prefix commands with `cd $CURRENT_DIR &&` so they run in the right place.

Option 1 is cleaner. Here is a simple `cd` interceptor:

```rust
use anyhow::Result;

/// Check if a command is a bare `cd` command and handle it specially.
pub async fn handle_command(
    manager: &WorkingDirManager,
    command: &str,
) -> Result<ShellOutput> {
    let trimmed = command.trim();

    // Handle bare "cd path" commands
    if trimmed == "cd" || trimmed == "cd ~" {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
        manager.change_dir(&home).await?;
        let cwd = manager.current().await;
        return Ok(ShellOutput {
            exit_code: 0,
            stdout: format!("Changed directory to {}\n", cwd.display()),
            stderr: String::new(),
            success: true,
            timed_out: false,
        });
    }

    if let Some(target) = trimmed.strip_prefix("cd ") {
        let target = target.trim();
        manager.change_dir(target).await?;
        let cwd = manager.current().await;
        return Ok(ShellOutput {
            exit_code: 0,
            stdout: format!("Changed directory to {}\n", cwd.display()),
            stderr: String::new(),
            success: true,
            timed_out: false,
        });
    }

    // Regular command -- run in current directory
    manager.execute_in_current_dir(command).await
}
```

This approach handles the common case cleanly. For compound commands like `cd src && cargo test`, you let the shell handle it (the `cd` within the `sh -c` invocation works fine since both commands run in the same shell instance).

::: info In the Wild
Claude Code tracks the working directory as persistent agent state. When the LLM runs a command that includes `cd`, Claude Code parses the command to detect the directory change and updates its internal state accordingly. This way, subsequent commands automatically execute in the correct directory. OpenCode takes a similar approach, maintaining a "session working directory" that persists across tool calls.
:::

## Key Takeaways

- Use `.current_dir()` on `Command` to set where the child process runs. Always validate the directory exists before using it.
- Maintain a `WorkingDirManager` to track the agent's working directory across tool calls, since each `spawn()` creates a fresh process that does not remember previous `cd` commands.
- Use `canonicalize()` to resolve symlinks and relative path components, preventing path traversal attacks.
- Intercept bare `cd` commands from the LLM and update the manager's state instead of just running them (since they have no effect across process boundaries).
- Enforce a root directory boundary to prevent the agent from navigating to directories outside the project scope.
