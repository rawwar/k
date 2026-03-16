---
title: Environment Variables
description: Control the environment variable inheritance and injection for spawned processes to ensure consistent and secure execution contexts.
---

# Environment Variables

> **What you'll learn:**
> - How to inherit, override, and clear environment variables for child processes
> - How to inject tool-specific variables like API keys or configuration paths
> - How to prevent sensitive environment variable leakage across process boundaries

When your agent runs `cargo test` or `npm run build`, those commands depend on environment variables like `PATH`, `HOME`, `CARGO_HOME`, and `NODE_ENV`. Getting the environment right is the difference between commands that work and commands that fail mysteriously. But you also need to be careful: your agent process might have API keys or tokens in its environment that you do not want leaking into arbitrary shell commands.

## How Environment Inheritance Works

By default, a child process inherits the full environment of its parent. This means that when your Rust agent spawns `sh -c "echo $HOME"`, the child sees the same `HOME`, `PATH`, `USER`, and every other variable that your agent process has:

```rust
use tokio::process::Command as TokioCommand;

#[tokio::main]
async fn main() {
    // Child inherits all of the parent's environment variables
    let output = TokioCommand::new("sh")
        .arg("-c")
        .arg("echo HOME=$HOME && echo PATH=$PATH")
        .output()
        .await
        .expect("failed to execute");

    println!("{}", String::from_utf8_lossy(&output.stdout));
}
```

This default is convenient -- most commands just work. But it is a security concern for an agent that holds sensitive credentials.

## Adding and Overriding Variables

The `.env()` method on `Command` adds or overrides a single environment variable without affecting the rest:

```rust
use tokio::process::Command as TokioCommand;

#[tokio::main]
async fn main() {
    let output = TokioCommand::new("sh")
        .arg("-c")
        .arg("echo GREETING=$GREETING && echo HOME=$HOME")
        .env("GREETING", "hello from the agent")
        .output()
        .await
        .expect("failed to execute");

    println!("{}", String::from_utf8_lossy(&output.stdout));
    // Output:
    // GREETING=hello from the agent
    // HOME=/Users/yourname      (inherited from parent)
}
```

The child sees `GREETING` (which you set) and `HOME` (which it inherited). This additive approach is what you want most of the time.

You can also pass multiple variables using `.envs()` with an iterator of key-value pairs:

```rust
use std::collections::HashMap;
use tokio::process::Command as TokioCommand;

#[tokio::main]
async fn main() {
    let mut extra_vars = HashMap::new();
    extra_vars.insert("NODE_ENV", "test");
    extra_vars.insert("CI", "true");
    extra_vars.insert("RUST_LOG", "debug");

    let output = TokioCommand::new("sh")
        .arg("-c")
        .arg("echo NODE_ENV=$NODE_ENV CI=$CI RUST_LOG=$RUST_LOG")
        .envs(extra_vars)
        .output()
        .await
        .expect("failed to execute");

    println!("{}", String::from_utf8_lossy(&output.stdout));
}
```

::: tip Coming from Python
Python's `subprocess.run()` takes an `env` parameter that **replaces** the entire environment:
```python
import os, subprocess
# This REPLACES the environment -- PATH is gone!
result = subprocess.run(["echo", "hello"], env={"MY_VAR": "value"})

# To add variables, you must merge with os.environ:
env = {**os.environ, "MY_VAR": "value"}
result = subprocess.run(["echo", "hello"], env=env)
```
Rust's `.env()` method is additive by default -- it adds to the inherited environment without removing anything. To get the "replace everything" behavior from Python, you call `.env_clear()` first, then add only the variables you want.
:::

## Clearing the Environment

For maximum isolation, you can clear the entire inherited environment and build from scratch:

```rust
use tokio::process::Command as TokioCommand;

#[tokio::main]
async fn main() {
    let output = TokioCommand::new("sh")
        .arg("-c")
        .arg("echo HOME=$HOME PATH=$PATH MY_VAR=$MY_VAR")
        .env_clear()                       // Remove ALL inherited variables
        .env("PATH", "/usr/bin:/bin")      // Add back only what we need
        .env("HOME", "/tmp")
        .env("MY_VAR", "isolated")
        .output()
        .await
        .expect("failed to execute");

    println!("{}", String::from_utf8_lossy(&output.stdout));
    // Output: HOME=/tmp PATH=/usr/bin:/bin MY_VAR=isolated
}
```

After `.env_clear()`, the child has **no** environment variables at all, not even `PATH`. You must add back at least `PATH` for the shell to find executables, and `HOME` for programs that need it.

## Removing Specific Variables

Sometimes you want to inherit everything *except* certain sensitive variables. The `.env_remove()` method selectively removes individual variables:

```rust
use tokio::process::Command as TokioCommand;

#[tokio::main]
async fn main() {
    // Suppose your agent has these set
    std::env::set_var("ANTHROPIC_API_KEY", "sk-secret-key-12345");
    std::env::set_var("DATABASE_URL", "postgres://secret@localhost/db");

    let output = TokioCommand::new("sh")
        .arg("-c")
        .arg("echo KEY=$ANTHROPIC_API_KEY DB=$DATABASE_URL HOME=$HOME")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("DATABASE_URL")
        .output()
        .await
        .expect("failed to execute");

    println!("{}", String::from_utf8_lossy(&output.stdout));
    // Output: KEY= DB= HOME=/Users/yourname
}
```

The sensitive variables are removed, but everything else (like `HOME` and `PATH`) is still inherited.

## Building an Environment Policy

For your agent, you want a systematic approach to environment management. Here is a policy struct that defines which variables to keep, remove, and inject:

```rust
use std::collections::{HashMap, HashSet};

/// Policy for environment variable handling in shell commands.
#[derive(Debug, Clone)]
pub struct EnvPolicy {
    /// Variables to always remove from the inherited environment.
    sensitive_vars: HashSet<String>,
    /// Variables to always inject into the child environment.
    injected_vars: HashMap<String, String>,
    /// Whether to inherit the parent environment (after removing sensitive vars).
    inherit: bool,
}

impl Default for EnvPolicy {
    fn default() -> Self {
        let sensitive = [
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "AWS_SECRET_ACCESS_KEY",
            "DATABASE_URL",
            "GITHUB_TOKEN",
            "SSH_AUTH_SOCK",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        Self {
            sensitive_vars: sensitive,
            injected_vars: HashMap::new(),
            inherit: true,
        }
    }
}

impl EnvPolicy {
    /// Add a variable to the sensitive list (will be removed from child env).
    pub fn add_sensitive(&mut self, var: impl Into<String>) {
        self.sensitive_vars.insert(var.into());
    }

    /// Add a variable to inject into every child process.
    pub fn inject(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.injected_vars.insert(key.into(), value.into());
    }

    /// Apply this policy to a tokio Command.
    pub fn apply(&self, cmd: &mut tokio::process::Command) {
        if !self.inherit {
            cmd.env_clear();
            // At minimum, restore PATH so commands can be found
            if let Ok(path) = std::env::var("PATH") {
                cmd.env("PATH", path);
            }
        }

        // Remove sensitive variables
        for var in &self.sensitive_vars {
            cmd.env_remove(var);
        }

        // Inject configured variables
        for (key, value) in &self.injected_vars {
            cmd.env(key, value);
        }
    }
}
```

Use the policy in your `ShellCommand` builder's `build()` method:

```rust
impl ShellCommand {
    pub fn build_with_policy(&self, policy: &EnvPolicy) -> TokioCommand {
        let mut cmd = self.build();
        policy.apply(&mut cmd);
        cmd
    }
}
```

Now every command your agent runs automatically strips API keys and other secrets from the child's environment, without the caller needing to remember to do it.

## Common Environment Variables for Coding Agents

Here are environment variables you will frequently interact with:

| Variable | Purpose | Agent Consideration |
|---|---|---|
| `PATH` | Where to find executables | Must be inherited or the child cannot run anything |
| `HOME` | User's home directory | Needed for tools that use `~/.config` |
| `TERM` | Terminal type | Set to `dumb` to disable color codes in output |
| `CI` | Continuous integration flag | Set to `true` to make tools produce non-interactive output |
| `RUST_BACKTRACE` | Rust backtrace control | Set to `1` for debugging failed commands |
| `NO_COLOR` | Disable color output | Set to `1` for clean, parseable output |

For an agent, setting `TERM=dumb` and `NO_COLOR=1` is often helpful because it prevents commands from emitting ANSI color codes that clutter the output sent to the LLM.

::: info In the Wild
Claude Code strips sensitive environment variables (API keys, tokens) before passing the environment to child processes. It also injects `CI=true` and `TERM=dumb` to encourage non-interactive, color-free output from build tools and test runners. This produces cleaner output that is easier for the LLM to parse and reason about.
:::

## Key Takeaways

- Child processes inherit the parent's full environment by default. Use `.env()` to add variables and `.env_remove()` to remove specific ones.
- Always strip sensitive variables (API keys, database credentials, tokens) from the child environment to prevent leakage.
- Create an `EnvPolicy` struct to systematically manage environment variables across all shell commands your agent executes.
- Set `TERM=dumb` and `NO_COLOR=1` to get clean, color-free output that is easier for the LLM to process.
- Use `.env_clear()` sparingly -- it removes everything including `PATH`, which will break most commands.
