---
title: Command Builder
description: Design a fluent command builder abstraction that encapsulates process configuration, arguments, and execution policies.
---

# Command Builder

> **What you'll learn:**
> - How to design a builder pattern for configuring shell commands
> - How to separate command construction from execution for testability
> - How to support both shell-interpreted and direct-exec command modes

So far you have been calling `TokioCommand::new("echo").arg("hello")` directly. That works, but as your shell tool grows in features -- timeouts, environment variables, working directory, output limits, safety checks -- you need a clean abstraction that collects all this configuration before execution. In this subchapter, you will build a `ShellCommand` builder that encapsulates the full command specification and produces a configured `tokio::process::Command` ready to run.

## Why a Builder?

The builder pattern solves two problems:

1. **Configuration accumulation**: You need to set the program, arguments, environment, working directory, timeout, and output limits before running the command. A builder lets you chain these calls fluently.
2. **Separation of concerns**: The code that parses the LLM's tool call should not be the same code that manages process execution. A builder struct acts as a clean boundary between "what to run" and "how to run it."

## Shell Mode vs. Direct Exec Mode

There are two ways to run a command:

- **Direct exec**: You specify the program and arguments as separate values. `Command::new("ls").args(&["-la", "/tmp"])` runs the `ls` binary directly with two arguments. No shell is involved.
- **Shell mode**: You pass a command string to a shell interpreter. `Command::new("sh").args(&["-c", "ls -la /tmp | head -5"])` runs the string through `sh`, which handles pipes, redirects, globbing, and other shell features.

For a coding agent, shell mode is almost always what you want. The LLM generates natural command strings like `grep -r "TODO" src/ | wc -l`, and these require a shell to interpret the pipe. Direct exec mode is useful when you have a known program and structured arguments.

## The `ShellCommand` Builder

Here is the complete builder struct with the fields you will fill in throughout this chapter:

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// A builder for configuring a shell command before execution.
#[derive(Debug, Clone)]
pub struct ShellCommand {
    /// The raw command string (for shell mode) or program name (for direct mode).
    command: String,
    /// Arguments for direct exec mode. Empty in shell mode.
    args: Vec<String>,
    /// Whether to run through a shell interpreter.
    shell: bool,
    /// Working directory for the command.
    working_dir: Option<PathBuf>,
    /// Environment variables to set (in addition to inherited ones).
    env_vars: HashMap<String, String>,
    /// Whether to inherit the parent process's environment.
    inherit_env: bool,
    /// Maximum execution time before the process is killed.
    timeout: Duration,
    /// Maximum bytes to capture from stdout.
    max_output_bytes: usize,
}

impl Default for ShellCommand {
    fn default() -> Self {
        Self {
            command: String::new(),
            args: Vec::new(),
            shell: true,
            working_dir: None,
            env_vars: HashMap::new(),
            inherit_env: true,
            timeout: Duration::from_secs(30),
            max_output_bytes: 512 * 1024, // 512 KB
        }
    }
}
```

The `Default` implementation sets sensible defaults: shell mode enabled, 30-second timeout, 512 KB output limit, and full environment inheritance. Every field can be overridden through builder methods.

## Fluent Builder Methods

The builder pattern in Rust uses methods that take `mut self` and return `Self`, enabling method chaining:

```rust
impl ShellCommand {
    /// Create a new shell command from a command string.
    /// The command will be executed through `sh -c` by default.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            ..Default::default()
        }
    }

    /// Create a command that executes a program directly without a shell.
    pub fn direct(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            command: program.into(),
            args,
            shell: false,
            ..Default::default()
        }
    }

    /// Set the working directory for the command.
    pub fn working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(path.into());
        self
    }

    /// Add an environment variable.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.insert(key.into(), value.into());
        self
    }

    /// Set whether to inherit the parent's environment variables.
    pub fn inherit_env(mut self, inherit: bool) -> Self {
        self.inherit_env = inherit;
        self
    }

    /// Set the maximum execution time.
    pub fn timeout(mut self, duration: Duration) -> Self {
        self.timeout = duration;
        self
    }

    /// Set the maximum output size in bytes.
    pub fn max_output(mut self, bytes: usize) -> Self {
        self.max_output_bytes = bytes;
        self
    }

    /// Get the timeout duration.
    pub fn get_timeout(&self) -> Duration {
        self.timeout
    }

    /// Get the command string for display or logging.
    pub fn display_command(&self) -> &str {
        &self.command
    }
}
```

::: tip Coming from Python
Python's `subprocess` module uses keyword arguments for configuration:
```python
result = subprocess.run(
    "ls -la | head -5",
    shell=True,
    cwd="/tmp",
    env={**os.environ, "MY_VAR": "value"},
    timeout=30,
    capture_output=True,
)
```
Rust does not have keyword arguments, so the builder pattern is the idiomatic equivalent. Each keyword becomes a method call:
```rust
let cmd = ShellCommand::new("ls -la | head -5")
    .working_dir("/tmp")
    .env("MY_VAR", "value")
    .timeout(Duration::from_secs(30));
```
The builder pattern has an advantage over keyword arguments: it is type-checked at compile time. You cannot accidentally pass a number where a path is expected.
:::

## Converting to a `tokio::process::Command`

The builder's job is to accumulate configuration. A separate method converts it into a `tokio::process::Command` ready for execution:

```rust
use std::process::Stdio;
use tokio::process::Command as TokioCommand;

impl ShellCommand {
    /// Build a configured `tokio::process::Command` from this builder.
    pub fn build(&self) -> TokioCommand {
        let mut cmd = if self.shell {
            let mut c = TokioCommand::new("sh");
            c.arg("-c");
            c.arg(&self.command);
            c
        } else {
            let mut c = TokioCommand::new(&self.command);
            for arg in &self.args {
                c.arg(arg);
            }
            c
        };

        // Configure stdio
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.stdin(Stdio::null());

        // Set working directory
        if let Some(ref dir) = self.working_dir {
            cmd.current_dir(dir);
        }

        // Configure environment
        if !self.inherit_env {
            cmd.env_clear();
        }
        for (key, value) in &self.env_vars {
            cmd.env(key, value);
        }

        cmd
    }
}
```

Notice how `build()` takes `&self` (an immutable reference). This means you can build multiple commands from the same configuration -- useful if you want to retry a failed command or run the same command in different directories.

## Using the Builder

Here is a complete example showing the builder in action:

```rust
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command as TokioCommand;

// Include the ShellCommand struct and all impl blocks from above

#[tokio::main]
async fn main() -> Result<()> {
    // Shell mode: run a pipeline
    let cmd = ShellCommand::new("echo 'hello world' | tr a-z A-Z")
        .timeout(Duration::from_secs(10));

    let mut tokio_cmd = cmd.build();
    let output = tokio_cmd.output().await?;
    println!(
        "Pipeline result: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    // Direct exec mode: run a specific program
    let cmd = ShellCommand::direct(
        "echo".to_string(),
        vec!["structured".to_string(), "args".to_string()],
    );

    let mut tokio_cmd = cmd.build();
    let output = tokio_cmd.output().await?;
    println!(
        "Direct result: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    // With environment and working directory
    let cmd = ShellCommand::new("pwd && echo $MY_VAR")
        .working_dir("/tmp")
        .env("MY_VAR", "agent-value");

    let mut tokio_cmd = cmd.build();
    let output = tokio_cmd.output().await?;
    println!(
        "Configured result: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    Ok(())
}
```

## Separating Construction from Execution

The builder creates a clear seam between "what do we want to run?" and "how do we actually run it?" This is valuable for three reasons:

1. **Safety checks**: Before calling `build()`, you can inspect the `ShellCommand` to detect dangerous patterns. You will implement this in the dangerous command detection subchapter.
2. **Logging**: You can log the command configuration without running it. The `display_command()` method gives you a human-readable representation.
3. **Testing**: You can test that your command parsing produces the correct `ShellCommand` without spawning any processes.

```rust
impl ShellCommand {
    /// Check if this command uses shell mode.
    pub fn is_shell_mode(&self) -> bool {
        self.shell
    }

    /// Get the raw command string.
    pub fn command_str(&self) -> &str {
        &self.command
    }

    /// Get the configured working directory, if any.
    pub fn get_working_dir(&self) -> Option<&PathBuf> {
        self.working_dir.as_ref()
    }
}
```

These accessor methods let safety-checking code inspect the command without needing access to the internal fields.

::: info In the Wild
Claude Code parses the LLM's tool call into a structured command representation before execution. This intermediate representation is where safety checks, permission lookups, and logging all happen. The command only reaches the OS after passing through every validation layer. This is exactly the pattern you are building with `ShellCommand` -- a structured intermediate form between the LLM's request and the actual process spawn.
:::

## Key Takeaways

- The builder pattern is Rust's idiomatic replacement for Python's keyword arguments, providing type-safe, fluent configuration.
- Support both **shell mode** (for pipelines and shell features via `sh -c`) and **direct exec mode** (for structured, predictable commands).
- Separating command construction (`ShellCommand`) from execution (`build()` + `output()`) creates clean boundaries for safety checks, logging, and testing.
- Use `impl Into<String>` and `impl Into<PathBuf>` in builder methods to accept both `&str` and `String` without forcing callers to convert.
- The `Default` trait provides sensible defaults for timeout, output limits, and environment inheritance so callers only configure what they need to change.
