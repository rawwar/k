---
title: Shell vs Exec
description: The critical distinction between invoking commands through a shell interpreter versus direct exec, with implications for security, performance, and correctness.
---

# Shell vs Exec

> **What you'll learn:**
> - The difference between executing a binary directly (exec) versus invoking it through a shell (sh -c) and why it matters
> - Security risks of shell invocation including injection attacks, glob expansion, and unintended variable substitution
> - When shell features like pipes, redirects, and globbing justify the tradeoffs of shell-based execution

This is one of the most important design decisions in a coding agent's process execution layer. When the LLM asks the agent to run a command, should the agent pass it directly to the OS via exec, or should it route it through a shell interpreter like `/bin/sh` or `/bin/bash`? The answer has profound implications for security, correctness, and the range of commands the agent can execute.

## Direct Exec: The Safe Default

When you use Rust's `Command::new("cargo").arg("test")`, the Rust runtime calls the `execvp` system call directly. No shell is involved. The arguments are passed to the `cargo` binary as-is, with no interpretation of special characters.

```rust
use std::process::Command;

fn main() {
    // Direct exec: each argument is passed literally
    let output = Command::new("echo")
        .arg("hello; rm -rf /")  // This is one literal argument string!
        .output()
        .expect("failed to run");

    // Prints: hello; rm -rf /
    // The semicolon and rm command are NOT interpreted
    println!("{}", String::from_utf8_lossy(&output.stdout));
}
```

In this example, `echo` receives a single argument: the literal string `"hello; rm -rf /"`. The semicolon is just a character in a string. No shell sees it, no shell interprets it, and `rm` is never executed.

### Advantages of Direct Exec

- **Security**: No shell injection is possible. Special characters like `;`, `|`, `$`, backticks, and `&&` are treated as literal text.
- **Performance**: No shell process is spawned. One less fork/exec cycle.
- **Predictability**: What you pass is exactly what the program receives.

### Limitations of Direct Exec

- No pipes (`command1 | command2`)
- No redirects (`> file`, `2>&1`)
- No glob expansion (`*.rs` stays as the literal string `*.rs`)
- No environment variable substitution (`$HOME` stays as the literal string `$HOME`)
- No command chaining (`&&`, `||`, `;`)

## Shell Invocation: The Powerful Option

When you need shell features, you invoke the shell explicitly and pass the command as a string:

```rust
use std::process::Command;

fn main() {
    // Shell invocation: the entire command is interpreted by sh
    let output = Command::new("sh")
        .args(["-c", "ls *.rs | wc -l"])
        .output()
        .expect("failed to run");

    println!("Rust files: {}", String::from_utf8_lossy(&output.stdout).trim());
}
```

Here, `/bin/sh` receives `"ls *.rs | wc -l"` as a command string. The shell parses it, expands `*.rs` to matching filenames, creates a pipe between `ls` and `wc`, and executes both programs. This is powerful but comes with significant risks.

::: python Coming from Python
Python's `subprocess.run` has a `shell=True` parameter that does exactly this:
```python
import subprocess
# shell=False (default) -- safe, direct exec
subprocess.run(["echo", "hello; rm -rf /"])  # literal string

# shell=True -- command interpreted by /bin/sh
subprocess.run("ls *.py | wc -l", shell=True)  # shell features available
```
The Python documentation strongly warns against using `shell=True` with untrusted input. The same warning applies in Rust: when you pass a string to `sh -c`, you are trusting that string to be safe.
:::

## Shell Injection: The Primary Risk

Shell injection occurs when untrusted input is embedded in a shell command string without proper escaping. In a coding agent, the LLM generates commands, and the LLM can be manipulated (through prompt injection or hallucination) into generating malicious commands.

Consider this dangerous pattern:

```rust
// DANGEROUS: Never do this with untrusted input!
use std::process::Command;

fn search_files_dangerous(pattern: &str) -> String {
    let cmd = format!("grep -r '{}' /home/user/project", pattern);
    let output = Command::new("sh")
        .args(["-c", &cmd])
        .output()
        .expect("failed");
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn main() {
    // Normal use works fine
    let result = search_files_dangerous("TODO");
    println!("{}", result);

    // But a malicious pattern breaks out of the quotes:
    // Pattern: ' ; rm -rf / ; echo '
    // Resulting command: grep -r '' ; rm -rf / ; echo '' /home/user/project
    // This would execute: rm -rf /
}
```

The attacker closes the single quote, injects a command, and opens a new quote to absorb the trailing characters. This is the classic shell injection attack.

### The Safe Alternative

Use direct exec with structured arguments:

```rust
use std::process::Command;

fn search_files_safe(pattern: &str) -> String {
    let output = Command::new("grep")
        .args(["-r", pattern, "/home/user/project"])
        .output()
        .expect("failed");
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn main() {
    // Even a "malicious" pattern is safe -- it's just a literal string argument
    let result = search_files_safe("' ; rm -rf / ; echo '");
    println!("{}", result);
    // grep tries to find the literal string "' ; rm -rf / ; echo '" in files
    // No shell interprets the semicolons or rm command
}
```

## When You Need the Shell

Despite the risks, there are legitimate reasons to use shell invocation in a coding agent:

### 1. Pipe Chains

```rust
use std::process::Command;

fn count_rust_files(dir: &str) -> Result<String, String> {
    let output = Command::new("sh")
        .args(["-c", &format!("find {} -name '*.rs' | wc -l",
            shell_escape::escape(dir.into()))])
        .output()
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn main() {
    // Uses the shell_escape crate for safety
    println!("{:?}", count_rust_files("/tmp"));
}
```

### 2. Complex Conditionals

```rust
use std::process::Command;

fn main() {
    let output = Command::new("sh")
        .args(["-c", "cargo check 2>&1 && echo 'BUILD OK' || echo 'BUILD FAILED'"])
        .output()
        .expect("failed");
    println!("{}", String::from_utf8_lossy(&output.stdout));
}
```

### 3. User-Provided Commands

When the LLM generates a full command string like `"cat src/main.rs | head -20"`, you need a shell to interpret the pipe. Most coding agents accept this tradeoff but add safety layers.

## Building a Safe Shell Executor

A practical approach for a coding agent: accept that you need the shell for some commands, but add validation layers:

```rust
use tokio::process::Command;
use std::process::Stdio;

const FORBIDDEN_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf /*",
    "mkfs.",
    "dd if=",
    ":(){:|:&};:",  // fork bomb
    "> /dev/sda",
    "chmod -R 777 /",
    "curl | sh",
    "wget | sh",
];

fn validate_command(cmd: &str) -> Result<(), String> {
    let lower = cmd.to_lowercase();
    for pattern in FORBIDDEN_PATTERNS {
        if lower.contains(pattern) {
            return Err(format!("Blocked dangerous pattern: {}", pattern));
        }
    }
    Ok(())
}

pub async fn execute_shell_command(
    cmd: &str,
    working_dir: &str,
) -> Result<(String, String, i32), String> {
    validate_command(cmd)?;

    let output = Command::new("sh")
        .args(["-c", cmd])
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("Spawn failed: {}", e))?;

    Ok((
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code().unwrap_or(-1),
    ))
}

#[tokio::main]
async fn main() {
    match execute_shell_command("echo hello | tr 'a-z' 'A-Z'", "/tmp").await {
        Ok((stdout, _, code)) => println!("Output: {} (exit {})", stdout.trim(), code),
        Err(e) => eprintln!("Blocked: {}", e),
    }

    match execute_shell_command("rm -rf /", "/tmp").await {
        Ok(_) => println!("This should not happen"),
        Err(e) => eprintln!("Blocked: {}", e),
    }
}
```

This pattern-based blocking is a defense-in-depth measure, not a complete solution. It should be combined with sandboxing (subchapter 8) and user confirmation (subchapter 11).

## Decision Framework

| Question | Direct Exec | Shell |
|----------|------------|-------|
| Does the command have pipes or redirects? | No | Yes |
| Is the command constructed from untrusted input? | Preferred | Requires validation |
| Do you need glob expansion? | No | Yes |
| Is performance critical? | Slightly faster | Slightly slower |
| Do you need environment variable expansion? | Handle in Rust | Shell does it |

As a rule of thumb for a coding agent:

1. **Parse the LLM's command string** -- if it contains shell metacharacters (`|`, `>`, `&&`, `||`, `;`, `*`, `$`), you need the shell.
2. **Validate before execution** -- apply deny patterns and possibly require user confirmation.
3. **Use direct exec when possible** -- for simple commands like `cargo test` or `git status`, avoid the shell entirely.

::: wild In the Wild
Claude Code passes most commands through `sh -c` because LLMs generate natural command strings that often include pipes and redirects. To mitigate the risks, it validates commands against a deny list of dangerous patterns and uses a sandboxing layer to limit file system access. Codex takes a more restrictive approach -- running commands in a sandboxed Docker container where even dangerous commands have limited impact because the filesystem is disposable.
:::

## Key Takeaways

- Direct exec (`Command::new("program").arg("arg")`) is inherently safe from injection because no shell interprets the arguments. Use it whenever possible.
- Shell invocation (`sh -c "command string"`) enables pipes, redirects, and globbing but opens the door to injection attacks.
- Never embed untrusted input into a shell command string without escaping or validation. Use direct exec with structured arguments for parameterized commands.
- A coding agent typically needs shell invocation for LLM-generated commands but should layer defenses: deny-pattern validation, sandboxing, and user confirmation.
- The decision between shell and exec depends on whether the command needs shell features -- pipes, redirects, and glob expansion are the most common reasons to reach for the shell.
