---
title: Execution Models
description: The different ways tools can be executed within an agent — in-process, subprocess, sandboxed, and remote execution patterns.
---

# Execution Models

> **What you'll learn:**
> - The trade-offs between in-process tool execution (fast, shared state) and subprocess execution (isolated, safe)
> - How sandboxed execution prevents tools from affecting the host system in unintended ways
> - When to use remote execution models for tools that require network access or specialized environments

Once you have validated the inputs for a tool call, you need to actually execute the tool. But "execute" is more nuanced than it first appears. Where does the tool run? In the same process as the agent? In a child process? In a container? On a remote server? Each choice carries different trade-offs in speed, safety, isolation, and complexity.

This subchapter maps out the four primary execution models and helps you decide which one fits each kind of tool.

## Model 1: In-Process Execution

In-process execution means the tool runs directly inside the agent's process, as a regular function call. This is the simplest and fastest model.

```rust
use std::fs;

pub fn execute_read_file(path: &str) -> Result<String, String> {
    fs::read_to_string(path)
        .map_err(|e| format!("Failed to read '{}': {}", path, e))
}
```

When the agent calls this function, it runs synchronously (or asynchronously, as we will explore in the next subchapter) within the agent's own process. There is no process boundary, no serialization overhead, and no startup cost.

**Advantages:**
- Lowest latency -- just a function call
- Full access to the agent's state (project root, configuration, conversation history)
- No serialization overhead for inputs or outputs
- Easy to debug -- the tool code runs in the same debugger session as the agent

**Disadvantages:**
- No isolation -- a bug in the tool can crash the entire agent
- No resource limits -- a tool that enters an infinite loop blocks the agent
- Shares memory with the agent -- a memory leak in a tool affects the whole process

In-process execution is the right choice for tools that are:
- Simple and well-understood (file reads, directory listings)
- Fast-completing (milliseconds, not seconds)
- Read-only or low-risk

Most of your coding agent's tools will use in-process execution. File reading, code search, directory listing, and similar tools are natural fits.

::: python Coming from Python
In Python, most tools in a simple agent are just functions that you call directly:
```python
def read_file(path: str) -> str:
    with open(path) as f:
        return f.read()
```
Rust's in-process model works the same way. The key difference is that Rust's type system and ownership rules catch bugs at compile time that would be runtime errors in Python. A tool that accidentally holds onto a file handle too long is a runtime bug in Python; in Rust, the borrow checker prevents it.
:::

## Model 2: Subprocess Execution

Subprocess execution runs the tool in a separate operating system process. The agent spawns the process, passes input via command-line arguments or stdin, waits for it to complete, and reads the output from stdout/stderr.

```rust
use std::process::Command;

pub fn execute_shell_command(command: &str, cwd: &str) -> Result<String, String> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("Failed to spawn process: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(stdout)
    } else {
        Err(format!(
            "Command exited with status {}.\nstdout: {}\nstderr: {}",
            output.status, stdout, stderr
        ))
    }
}
```

**Advantages:**
- Process isolation -- a crash in the tool does not crash the agent
- Resource control -- you can set timeouts, memory limits, and CPU limits via OS mechanisms
- Security boundaries -- the subprocess can run as a different user or in a restricted environment
- Natural fit for shell commands, compilers, test runners, and other external programs

**Disadvantages:**
- Higher latency -- process creation has overhead (typically 1-10ms on Linux)
- Serialization overhead -- inputs and outputs must pass through stdin/stdout as bytes
- No shared state -- the subprocess cannot directly access the agent's configuration or conversation history
- More complex error handling -- you must interpret exit codes and parse stderr

Subprocess execution is the right choice for:
- Shell command execution (this is the obvious one)
- Running compilers, linters, and test suites
- Any operation where you need to enforce resource limits
- Any operation where a crash should not take down the agent

## Model 3: Sandboxed Execution

Sandboxed execution extends subprocess isolation with additional constraints that restrict what the subprocess can do. Sandboxing can limit file system access, network access, system calls, and more.

On macOS, you can use the built-in `sandbox-exec` facility. On Linux, you might use seccomp, namespaces, or a container runtime:

```rust
use std::process::Command;

pub fn execute_sandboxed(
    command: &str,
    cwd: &str,
    allowed_paths: &[&str],
) -> Result<String, String> {
    // Build a macOS sandbox profile that restricts file access
    let profile = format!(
        r#"
        (version 1)
        (deny default)
        (allow process-exec)
        (allow file-read* (subpath "{}"))
        (allow file-write* (subpath "{}"))
        "#,
        cwd, cwd
    );

    let output = Command::new("sandbox-exec")
        .arg("-p")
        .arg(&profile)
        .arg("sh")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("Sandbox execution failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    if output.status.success() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(format!("Sandboxed command failed: {}", stderr))
    }
}
```

**Advantages:**
- Strong isolation -- even if the tool is compromised, it cannot access the broader system
- Defense in depth -- sandboxing protects against both bugs and prompt injection attacks
- Auditable -- sandbox policies are explicit and reviewable

**Disadvantages:**
- Platform-specific -- sandbox mechanisms differ between macOS, Linux, and Windows
- Configuration complexity -- getting sandbox policies right is tricky
- Performance overhead -- sandbox enforcement adds latency
- Compatibility issues -- some tools require system calls or file access that the sandbox blocks

Sandboxed execution is the right choice for:
- Shell command execution where the command content comes from an LLM (prompt injection risk)
- Operations on untrusted codebases
- Production deployments where security is paramount

::: wild In the Wild
Claude Code uses macOS `sandbox-exec` to restrict shell commands to the project directory and its subdirectories. This means even if a prompt injection tricks the model into running `cat /etc/passwd`, the sandbox blocks the read. OpenCode takes a different approach, relying on its permission system and command deny-lists rather than OS-level sandboxing. The trade-off is clear: sandbox-based isolation is stronger but less portable, while permission-based isolation is simpler but relies on correctly predicting dangerous commands.
:::

## Model 4: Remote Execution

Remote execution sends the tool call to a separate service, typically over HTTP or gRPC. The tool runs on a different machine entirely.

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct RemoteToolRequest {
    tool: String,
    input: serde_json::Value,
}

#[derive(Deserialize)]
struct RemoteToolResponse {
    success: bool,
    output: String,
    error: Option<String>,
}

pub async fn execute_remote(
    client: &reqwest::Client,
    endpoint: &str,
    tool_name: &str,
    input: serde_json::Value,
) -> Result<String, String> {
    let request = RemoteToolRequest {
        tool: tool_name.to_string(),
        input,
    };

    let response = client
        .post(endpoint)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Remote execution request failed: {}", e))?;

    let result: RemoteToolResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse remote response: {}", e))?;

    if result.success {
        Ok(result.output)
    } else {
        Err(result.error.unwrap_or_else(|| "Unknown remote error".to_string()))
    }
}
```

**Advantages:**
- Complete isolation -- the tool runs on a separate machine
- Scalability -- remote tools can run on more powerful hardware
- Specialization -- the remote service can have dependencies that you do not want on the agent machine
- Multi-tenancy -- one tool service can serve many agents

**Disadvantages:**
- Network latency -- every tool call requires a round-trip
- Network reliability -- the remote service might be down or slow
- Complexity -- you need to deploy, monitor, and maintain a separate service
- State synchronization -- the remote service needs access to the project files

Remote execution is the right choice for:
- Tools that require expensive computation (large-scale code analysis, AI-powered code review)
- Multi-agent architectures where tools are shared across agents
- Cloud-hosted agent deployments where isolation requirements are strict

## Choosing the Right Model

Here is a decision framework for choosing the execution model for each tool:

| Consideration | In-Process | Subprocess | Sandboxed | Remote |
|---|---|---|---|---|
| Read-only, simple | Best | Unnecessary | Unnecessary | Unnecessary |
| Mutating, trusted | Good | Better | Best | Depends |
| External program | N/A | Best | Better | Depends |
| Untrusted input | Risky | Good | Best | Best |
| Latency-sensitive | Best | Good | Fair | Poor |
| Production security | Fair | Good | Best | Best |

For a CLI coding agent -- which is what you are building -- the typical breakdown is:

- **In-process**: file read, file write, file edit, directory listing, code search
- **Subprocess**: shell command execution, running tests, running compilers
- **Sandboxed subprocess**: shell command execution in production mode
- **Remote**: not typically needed for a CLI agent

## Key Takeaways

- In-process execution is fastest and simplest -- use it for read-only tools and simple mutations that you control completely
- Subprocess execution provides process isolation and resource control -- use it for shell commands, compilers, and test runners
- Sandboxed execution adds security constraints on top of subprocess isolation -- use it when the command content comes from an LLM
- Remote execution is for specialized cases like expensive computation or shared tool services
- Most CLI coding agent tools use in-process execution, with subprocess execution for shell commands and sandboxed execution for security-critical operations
