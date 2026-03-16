---
title: Sandboxing Basics
description: Introduce foundational sandboxing techniques to restrict what shell commands can access, including filesystem and network boundaries.
---

# Sandboxing Basics

> **What you'll learn:**
> - How to restrict filesystem access for spawned processes using allow/deny path lists
> - How to implement allow/deny lists for commands the agent is permitted to execute
> - How to leverage macOS sandbox-exec and Linux seccomp for lightweight process isolation

Giving an AI agent shell access is inherently dangerous. Even with dangerous command detection (covered later in this chapter), a determined or confused LLM could find creative ways to cause harm. Sandboxing adds a second layer of defense: even if a dangerous command slips through the detection filter, the sandbox prevents it from actually doing damage.

This subchapter covers practical sandboxing techniques you can implement in your agent today. We start with application-level restrictions (which work everywhere) and then touch on OS-level sandboxing (which provides stronger guarantees but is platform-specific).

## Defense in Depth

No single safety mechanism is sufficient. A production agent uses **defense in depth** -- multiple overlapping layers:

1. **Dangerous command detection** (application layer): Pattern matching to catch obviously bad commands.
2. **Command allow/deny lists** (application layer): Restrict which programs the agent can execute.
3. **Filesystem restrictions** (application layer): Limit which directories commands can read and write.
4. **OS-level sandboxing** (kernel layer): Use the operating system's own isolation mechanisms.
5. **User confirmation** (UI layer): Ask the user before running high-risk commands.

You have already built some of these pieces. This subchapter focuses on layers 2, 3, and 4.

## Command Allow/Deny Lists

The simplest form of sandboxing is controlling which commands the agent is allowed to run. You maintain two lists:

- **Allow list**: Only these commands are permitted. Everything else is blocked.
- **Deny list**: These commands are never permitted. Everything else is allowed.

An allow list is more secure (default-deny), but a deny list is more practical for a coding agent that needs to run diverse commands. In practice, you use a combination: a deny list of known-dangerous commands plus some restrictions on what counts as an acceptable command.

```rust
use anyhow::{anyhow, Result};
use std::collections::HashSet;

/// Controls which commands the agent is allowed to execute.
#[derive(Debug, Clone)]
pub struct CommandPolicy {
    /// Commands that are never allowed, regardless of other settings.
    denied_programs: HashSet<String>,
    /// If non-empty, only these commands are allowed.
    allowed_programs: Option<HashSet<String>>,
    /// Whether to allow shell mode (sh -c "...") commands.
    allow_shell_mode: bool,
}

impl Default for CommandPolicy {
    fn default() -> Self {
        let denied: HashSet<String> = [
            "sudo", "su", "chmod", "chown", "mkfs", "mount", "umount",
            "dd", "shutdown", "reboot", "systemctl", "launchctl",
            "curl | sh", "wget | sh",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        Self {
            denied_programs: denied,
            allowed_programs: None, // No allow list = allow everything not denied
            allow_shell_mode: true,
        }
    }
}

impl CommandPolicy {
    /// Check if a command is permitted by this policy.
    pub fn check(&self, command: &str) -> Result<()> {
        let trimmed = command.trim();

        // Extract the first word (the program name)
        let program = trimmed
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_lowercase();

        // Check the deny list
        if self.denied_programs.contains(&program) {
            return Err(anyhow!(
                "Command '{}' is blocked by security policy", program
            ));
        }

        // Check the allow list (if configured)
        if let Some(ref allowed) = self.allowed_programs {
            if !allowed.contains(&program) {
                return Err(anyhow!(
                    "Command '{}' is not in the allowed commands list", program
                ));
            }
        }

        Ok(())
    }

    /// Add a program to the deny list.
    pub fn deny(&mut self, program: impl Into<String>) {
        self.denied_programs.insert(program.into());
    }

    /// Set the allow list. When set, only listed programs can be executed.
    pub fn set_allowed(&mut self, programs: Vec<String>) {
        self.allowed_programs = Some(programs.into_iter().collect());
    }
}
```

Integrate the policy into your command execution path:

```rust
pub async fn execute_with_policy(
    command: &str,
    policy: &CommandPolicy,
) -> Result<ShellOutput> {
    // Check policy BEFORE spawning the process
    policy.check(command)?;

    // If we get here, the command is permitted
    ShellCommand::new(command)
        .execute()
        .await
}
```

The key principle: **check before you spawn**. Once a process is running, it is too late to prevent damage.

## Filesystem Path Restrictions

Beyond controlling which programs can run, you can restrict which directories those programs can access. This is particularly important for write operations:

```rust
use std::path::{Path, PathBuf};

/// Restricts filesystem access for shell commands.
#[derive(Debug, Clone)]
pub struct PathPolicy {
    /// Directories the agent is allowed to read from.
    readable_paths: Vec<PathBuf>,
    /// Directories the agent is allowed to write to.
    writable_paths: Vec<PathBuf>,
}

impl PathPolicy {
    /// Create a new policy that allows read/write within a project directory.
    pub fn for_project(project_root: &Path) -> Self {
        Self {
            readable_paths: vec![
                project_root.to_path_buf(),
                PathBuf::from("/usr"),           // System binaries
                PathBuf::from("/bin"),
                PathBuf::from("/etc"),           // Config files (read-only)
            ],
            writable_paths: vec![
                project_root.to_path_buf(),
                std::env::temp_dir(),            // Temp directory
            ],
        }
    }

    /// Check if a path is readable under this policy.
    pub fn can_read(&self, path: &Path) -> bool {
        let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        self.readable_paths.iter().any(|allowed| resolved.starts_with(allowed))
    }

    /// Check if a path is writable under this policy.
    pub fn can_write(&self, path: &Path) -> bool {
        let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        self.writable_paths.iter().any(|allowed| resolved.starts_with(allowed))
    }
}
```

::: python Coming from Python
Python's `subprocess` module does not have built-in sandboxing. You would typically use OS-level tools or Docker containers:
```python
import subprocess
# No built-in filesystem restriction in Python's subprocess
# You'd use Docker or OS sandboxing:
result = subprocess.run(
    ["docker", "run", "--rm", "-v", "/project:/project:rw",
     "ubuntu", "bash", "-c", "cargo test"],
    capture_output=True, text=True,
)
```
Rust does not have built-in sandboxing either, but the type system makes it easier to enforce policies at compile time -- your `execute` function can require a `&PathPolicy` parameter, making it impossible to forget the check.
:::

## OS-Level Sandboxing on macOS

macOS provides `sandbox-exec`, a command-line tool that runs a process inside a sandbox defined by a profile. You can use this to restrict filesystem and network access at the kernel level:

```rust
use std::process::Stdio;
use tokio::process::Command as TokioCommand;

/// Generate a macOS sandbox profile that restricts filesystem access.
fn macos_sandbox_profile(allowed_paths: &[&str]) -> String {
    let mut profile = String::from("(version 1)\n");
    profile.push_str("(deny default)\n");
    profile.push_str("(allow process-exec)\n");
    profile.push_str("(allow process-fork)\n");
    profile.push_str("(allow sysctl-read)\n");
    profile.push_str("(allow mach-lookup)\n");

    // Allow reading from standard system paths
    profile.push_str("(allow file-read*\n");
    profile.push_str("  (subpath \"/usr\")\n");
    profile.push_str("  (subpath \"/bin\")\n");
    profile.push_str("  (subpath \"/Library\")\n");
    profile.push_str("  (subpath \"/System\")\n");
    profile.push_str("  (subpath \"/dev\")\n");
    profile.push_str("  (subpath \"/private/tmp\")\n");

    for path in allowed_paths {
        profile.push_str(&format!("  (subpath \"{}\")\n", path));
    }
    profile.push_str(")\n");

    // Allow writing only to specified paths
    profile.push_str("(allow file-write*\n");
    profile.push_str("  (subpath \"/private/tmp\")\n");
    profile.push_str("  (subpath \"/dev\")\n");
    for path in allowed_paths {
        profile.push_str(&format!("  (subpath \"{}\")\n", path));
    }
    profile.push_str(")\n");

    // Deny network access
    profile.push_str("(deny network*)\n");

    profile
}

/// Execute a command inside a macOS sandbox.
pub async fn execute_sandboxed_macos(
    command: &str,
    allowed_paths: &[&str],
) -> anyhow::Result<ShellOutput> {
    let profile = macos_sandbox_profile(allowed_paths);

    let output = TokioCommand::new("sandbox-exec")
        .arg("-p")
        .arg(&profile)
        .arg("sh")
        .arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Sandbox execution failed: {}", e))?;

    Ok(ShellOutput {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
        timed_out: false,
    })
}
```

Note that `sandbox-exec` is deprecated by Apple but still works and is used by several production tools. For a more future-proof approach on macOS, consider the Endpoint Security framework or running commands inside a lightweight container.

## Linux Sandboxing with seccomp

On Linux, `seccomp` (secure computing) lets you restrict which system calls a process can make. The `seccomp` crate provides a Rust-friendly API. A simpler approach for many agents is to use `bwrap` (bubblewrap), a lightweight sandboxing tool:

```rust
/// Execute a command inside a Linux bubblewrap sandbox.
pub async fn execute_sandboxed_linux(
    command: &str,
    project_dir: &str,
) -> anyhow::Result<ShellOutput> {
    let output = TokioCommand::new("bwrap")
        .args(&[
            "--ro-bind", "/usr", "/usr",
            "--ro-bind", "/bin", "/bin",
            "--ro-bind", "/lib", "/lib",
            "--ro-bind", "/lib64", "/lib64",
            "--bind", project_dir, project_dir,  // Read-write for project
            "--tmpfs", "/tmp",
            "--dev", "/dev",
            "--proc", "/proc",
            "--unshare-net",                      // No network access
            "--die-with-parent",                  // Kill if parent dies
            "sh", "-c", command,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Sandbox execution failed: {}", e))?;

    Ok(ShellOutput {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
        timed_out: false,
    })
}
```

Bubblewrap uses Linux namespaces to create an isolated environment. The `--unshare-net` flag prevents network access, and `--ro-bind` makes system directories read-only. Only the project directory is mounted read-write.

::: wild In the Wild
Claude Code uses macOS `sandbox-exec` profiles and Linux namespace isolation to restrict shell command access. The sandbox denies network access by default (preventing data exfiltration) and limits filesystem writes to the project directory and temp directories. Codex CLI takes a similar approach, offering multiple sandbox modes from "full network access" to "network disabled" depending on the user's trust level.
:::

## Choosing Your Sandboxing Strategy

For a learning project, start with application-level restrictions (command allow/deny lists and path checks). These work on every platform and are easy to test. Add OS-level sandboxing when you need stronger guarantees:

| Strategy | Strength | Portability | Complexity |
|---|---|---|---|
| Command allow/deny lists | Low | High (all platforms) | Low |
| Path validation | Medium | High (all platforms) | Low |
| macOS sandbox-exec | High | macOS only | Medium |
| Linux bwrap/seccomp | High | Linux only | Medium |
| Docker/containers | Very high | Docker required | High |

## Key Takeaways

- Implement **defense in depth**: use multiple overlapping safety layers rather than relying on any single mechanism.
- Command allow/deny lists are the simplest sandbox and work on every platform. Always check the policy **before** spawning the process.
- Use `PathPolicy` to restrict which directories commands can read from and write to, rooted at the project directory.
- OS-level sandboxing (macOS `sandbox-exec`, Linux `bwrap`) provides kernel-enforced isolation that application code cannot bypass.
- Start with application-level restrictions for your learning project, and add OS-level sandboxing when deploying to production.
