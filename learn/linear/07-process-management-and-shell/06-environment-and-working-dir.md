---
title: Environment and Working Dir
description: Controlling the execution context of child processes through environment variables, working directory selection, and PATH manipulation.
---

# Environment and Working Dir

> **What you'll learn:**
> - How environment variables are inherited by child processes and techniques for setting, clearing, and isolating them
> - Setting the working directory for a child process and why this matters for tools that use relative paths
> - Managing PATH and other critical environment variables to ensure tools resolve correctly

When your agent runs a command, the command executes in a specific context: a set of environment variables that configure its behavior and a working directory that determines how relative paths resolve. Getting these right is essential for correctness. A build tool needs to run in the project's root directory. A test might require specific environment variables. A sandboxed command might need a stripped-down environment to prevent information leakage. This subchapter covers how to control both dimensions.

## Environment Variable Inheritance

By default, a child process inherits a complete copy of its parent's environment variables. When your agent process has `HOME=/home/user`, `PATH=/usr/bin:/bin`, and `CARGO_HOME=/home/user/.cargo`, the child sees all of these.

Rust's `Command` builder provides fine-grained control:

### Setting Individual Variables

```rust
use std::process::Command;

fn main() {
    let output = Command::new("sh")
        .args(["-c", "echo Hello $NAME, your role is $ROLE"])
        .env("NAME", "Agent")
        .env("ROLE", "code_assistant")
        .output()
        .expect("failed to run");

    println!("{}", String::from_utf8_lossy(&output.stdout));
}
```

The `.env()` method adds or overrides a single variable. The child still inherits all other variables from the parent.

### Removing Specific Variables

```rust
use std::process::Command;

fn main() {
    let output = Command::new("env")
        .env_remove("CARGO_HOME")
        .env_remove("RUSTUP_HOME")
        .output()
        .expect("failed to run");

    let env_output = String::from_utf8_lossy(&output.stdout);
    println!("Child environment:\n{}", env_output);
}
```

`.env_remove()` removes a variable from the inherited set. The child sees everything the parent has except the removed variables.

### Starting with a Clean Environment

For maximum isolation, use `.env_clear()` to wipe the entire inherited environment, then add back only what you need:

```rust
use std::process::Command;

fn main() {
    let output = Command::new("env")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", "/tmp")
        .env("TERM", "dumb")
        .output()
        .expect("failed to run");

    let env_output = String::from_utf8_lossy(&output.stdout);
    println!("Minimal environment:\n{}", env_output);
}
```

This is the foundation of environment sandboxing. By clearing the environment and adding back only essential variables, you prevent the child from accessing sensitive information like API keys, database credentials, or authentication tokens that might be in the agent's environment.

::: python Coming from Python
Python's `subprocess.run()` accepts an `env` parameter that replaces the entire environment:
```python
import subprocess
result = subprocess.run(["env"], env={"PATH": "/usr/bin:/bin", "HOME": "/tmp"}, capture_output=True, text=True)
```
When `env` is set, the child gets *only* those variables -- equivalent to Rust's `.env_clear()` followed by `.env()` calls. Python has no direct equivalent of Rust's `.env_remove()` for selectively removing variables while keeping the rest; you would need to copy `os.environ`, delete keys, and pass the modified dict.
:::

## Setting the Working Directory

The `.current_dir()` method sets the working directory for the child process:

```rust
use std::process::Command;
use std::path::Path;

fn main() {
    let project_dir = Path::new("/tmp/my-project");

    let output = Command::new("pwd")
        .current_dir(project_dir)
        .output()
        .expect("failed to run pwd");

    println!("Child CWD: {}", String::from_utf8_lossy(&output.stdout).trim());
}
```

This is critical for tools that use relative paths. When your agent needs to run `cargo test` in a user's project, you must set the working directory to the project root. Otherwise, Cargo will not find the `Cargo.toml` and the command will fail.

### Dynamic Working Directory

In a coding agent, the working directory often comes from the user's context or the agent's current project:

```rust
use std::process::Command;
use std::path::PathBuf;

fn run_in_project(project_root: &PathBuf, program: &str, args: &[&str]) -> Result<String, String> {
    // Validate the directory exists
    if !project_root.is_dir() {
        return Err(format!("Directory does not exist: {:?}", project_root));
    }

    let output = Command::new(program)
        .args(args)
        .current_dir(project_root)
        .output()
        .map_err(|e| format!("Failed to spawn {}: {}", program, e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

fn main() {
    let project = PathBuf::from("/tmp");
    match run_in_project(&project, "ls", &["-la"]) {
        Ok(output) => println!("{}", output),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

Always validate that the directory exists before setting it. If the directory does not exist, `spawn()` will fail with an IO error -- but the error message may not clearly indicate that the problem was the working directory.

## Managing PATH

The `PATH` variable deserves special attention. It controls which programs are found when you specify a command by name. For a coding agent, PATH management affects whether tools like `cargo`, `rustc`, `node`, `python`, and `git` are available to child processes.

### Extending PATH

To add directories to the child's PATH without replacing it:

```rust
use std::process::Command;
use std::env;

fn main() {
    let current_path = env::var("PATH").unwrap_or_default();
    let extended_path = format!("/home/user/.cargo/bin:{}", current_path);

    let output = Command::new("sh")
        .args(["-c", "which cargo"])
        .env("PATH", &extended_path)
        .output()
        .expect("failed to run");

    println!("{}", String::from_utf8_lossy(&output.stdout).trim());
}
```

### Restricting PATH

For security, you might want to limit which programs are available:

```rust
use std::process::Command;

fn main() {
    // Only allow standard system directories
    let restricted_path = "/usr/local/bin:/usr/bin:/bin";

    let output = Command::new("sh")
        .args(["-c", "echo PATH=$PATH; which python3 || echo 'python3 not found'"])
        .env("PATH", restricted_path)
        .output()
        .expect("failed to run");

    println!("{}", String::from_utf8_lossy(&output.stdout));
}
```

## Combining Environment and Working Directory

A practical agent executor combines all of these controls:

```rust
use tokio::process::Command;
use std::path::PathBuf;
use std::process::Stdio;
use std::collections::HashMap;

pub struct ExecutionContext {
    pub working_dir: PathBuf,
    pub env_vars: HashMap<String, String>,
    pub env_removals: Vec<String>,
    pub clean_env: bool,
}

pub async fn execute_in_context(
    ctx: &ExecutionContext,
    program: &str,
    args: &[&str],
) -> Result<(String, String, i32), String> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(&ctx.working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if ctx.clean_env {
        cmd.env_clear();
    }

    for key in &ctx.env_removals {
        cmd.env_remove(key);
    }

    for (key, value) in &ctx.env_vars {
        cmd.env(key, value);
    }

    let output = cmd.output().await
        .map_err(|e| format!("Spawn failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    Ok((stdout, stderr, code))
}

#[tokio::main]
async fn main() {
    let ctx = ExecutionContext {
        working_dir: PathBuf::from("/tmp"),
        env_vars: HashMap::from([
            ("PATH".to_string(), "/usr/bin:/bin".to_string()),
            ("LANG".to_string(), "en_US.UTF-8".to_string()),
        ]),
        env_removals: vec!["SECRET_KEY".to_string()],
        clean_env: false,
    };

    match execute_in_context(&ctx, "ls", &["-la"]).await {
        Ok((stdout, stderr, code)) => {
            println!("Exit code: {}", code);
            println!("Stdout:\n{}", stdout);
            if !stderr.is_empty() {
                println!("Stderr:\n{}", stderr);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

This `ExecutionContext` struct becomes a reusable configuration object that your agent applies to every command execution. Different tools or security levels can have different contexts.

::: wild In the Wild
Claude Code carefully manages environment variables when spawning child processes. It ensures that tools like `cargo` and `git` are available by preserving PATH, while being cautious about what other environment variables are passed through. Some agents use a clean environment approach with an explicit allowlist of variables that are safe to inherit -- this prevents accidental leakage of API keys or tokens into child process environments.
:::

## Common Pitfalls

**Missing HOME**: Many tools expect `HOME` to be set. If you use `env_clear()`, make sure to set `HOME` to something reasonable, or tools like `git` and `cargo` will fail with confusing errors.

**Missing TERM**: Some tools detect whether they are in a terminal by checking `TERM`. If missing, they may suppress colored output or progress indicators. Set `TERM=dumb` in clean environments to signal that the output is not going to a real terminal.

**Platform differences**: On macOS, some system tools rely on environment variables like `TMPDIR` that point to user-specific temporary directories. Clearing the environment may cause these tools to use `/tmp` instead, which has different permissions.

**Locale variables**: `LANG`, `LC_ALL`, and related variables affect how tools format output (date formats, number separators, sort order). For reliable parsing of tool output, consider setting `LC_ALL=C` to force a predictable locale.

## Key Takeaways

- Child processes inherit the parent's full environment by default. Use `.env()` to add variables, `.env_remove()` to remove specific ones, and `.env_clear()` to start with a blank slate.
- `.current_dir()` sets the child's working directory, which is essential for tools that depend on relative paths like build systems and test runners.
- PATH management determines which programs are available to child processes. Extend it to make tools accessible or restrict it for security.
- When using `.env_clear()`, remember to set essential variables like `PATH`, `HOME`, `TERM`, and locale variables (`LC_ALL=C`) to prevent tools from failing.
- Wrap environment and working directory configuration into a reusable context struct that your agent applies consistently across all command executions.
