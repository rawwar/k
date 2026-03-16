---
title: Sandboxing Deep Dive
description: Explore advanced sandboxing techniques that isolate agent-executed code from the host system to contain potential damage.
---

# Sandboxing Deep Dive

> **What you'll learn:**
> - How to implement multi-layer sandboxing using OS-level mechanisms (namespaces, seccomp, macOS sandbox-exec) and container runtimes
> - The tradeoffs between sandboxing strictness and agent capability -- how tight sandboxing limits useful operations
> - How to design escape hatches that allow controlled access to resources outside the sandbox when explicitly approved

Permissions and denylists are advisory controls -- they operate at the application level and can be bypassed if the agent finds a creative way around them. Sandboxing adds enforcement at the operating system level, creating boundaries that cannot be crossed regardless of what commands the agent tries to run. If permissions are the rules, sandboxing is the walls.

## Why Application-Level Checks Are Not Enough

Consider this scenario: your denylist blocks `rm -rf /`, but the agent runs `find / -delete` instead. Or it writes a Python script that does `os.remove()` and then executes that script. Or it uses `curl` to download a binary that performs the destructive operation. Application-level filters operate on the command string, but they cannot anticipate every way to express the same operation. OS-level sandboxing solves this by restricting the process's actual capabilities -- regardless of what program runs, it cannot access files or network resources outside its sandbox.

## Sandboxing Layers

A robust sandboxing strategy uses multiple layers, each catching different classes of escape:

```rust
/// Represents the layers of sandboxing available on a system.
#[derive(Debug, Clone)]
enum SandboxLayer {
    /// Restrict the filesystem view to a specific directory tree
    FilesystemRestriction {
        allowed_paths: Vec<String>,
        read_only_paths: Vec<String>,
    },
    /// Restrict network access
    NetworkRestriction {
        allow_loopback: bool,
        allowed_hosts: Vec<String>,
    },
    /// Restrict which system calls the process can make
    SyscallFilter {
        allowed_syscalls: Vec<String>,
    },
    /// Run in a separate user/mount/pid namespace
    NamespaceIsolation {
        new_user_ns: bool,
        new_mount_ns: bool,
        new_pid_ns: bool,
        new_net_ns: bool,
    },
    /// Run inside a container (Docker, Podman, etc.)
    ContainerIsolation {
        image: String,
        mount_project: String,
        network_mode: String,
    },
}

/// Configuration for the complete sandbox environment.
#[derive(Debug)]
struct SandboxConfig {
    layers: Vec<SandboxLayer>,
    /// Working directory inside the sandbox
    workdir: String,
    /// Environment variables to pass through
    env_passthrough: Vec<String>,
    /// Maximum execution time for any single command
    timeout_seconds: u64,
}

impl SandboxConfig {
    /// Create a restrictive sandbox suitable for untrusted agent operations.
    fn strict(project_path: &str) -> Self {
        Self {
            layers: vec![
                SandboxLayer::FilesystemRestriction {
                    allowed_paths: vec![
                        project_path.to_string(),
                        "/usr".to_string(),
                        "/bin".to_string(),
                        "/lib".to_string(),
                    ],
                    read_only_paths: vec![
                        "/usr".to_string(),
                        "/bin".to_string(),
                        "/lib".to_string(),
                    ],
                },
                SandboxLayer::NetworkRestriction {
                    allow_loopback: true,
                    allowed_hosts: vec![], // No external network
                },
                SandboxLayer::NamespaceIsolation {
                    new_user_ns: true,
                    new_mount_ns: true,
                    new_pid_ns: true,
                    new_net_ns: true,
                },
            ],
            workdir: project_path.to_string(),
            env_passthrough: vec!["PATH".into(), "HOME".into(), "TERM".into()],
            timeout_seconds: 30,
        }
    }

    /// Create a permissive sandbox for trusted operations.
    fn permissive(project_path: &str) -> Self {
        Self {
            layers: vec![
                SandboxLayer::FilesystemRestriction {
                    allowed_paths: vec![
                        project_path.to_string(),
                        "/usr".to_string(),
                        "/bin".to_string(),
                        "/lib".to_string(),
                        "/tmp".to_string(),
                    ],
                    read_only_paths: vec![],
                },
                SandboxLayer::NetworkRestriction {
                    allow_loopback: true,
                    allowed_hosts: vec!["*".into()], // Full network access
                },
            ],
            workdir: project_path.to_string(),
            env_passthrough: vec!["PATH".into(), "HOME".into(), "TERM".into()],
            timeout_seconds: 120,
        }
    }
}

fn main() {
    let strict = SandboxConfig::strict("/home/user/myproject");
    let permissive = SandboxConfig::permissive("/home/user/myproject");

    println!("Strict sandbox layers: {}", strict.layers.len());
    for layer in &strict.layers {
        println!("  {:?}", layer);
    }

    println!("\nPermissive sandbox layers: {}", permissive.layers.len());
    for layer in &permissive.layers {
        println!("  {:?}", layer);
    }
}
```

## macOS Sandboxing with sandbox-exec

On macOS, the `sandbox-exec` command (built on the Seatbelt framework) lets you define a policy in a Scheme-like language that restricts what a process can do. Here is how you would use it from Rust:

```rust
use std::io::Write;
use std::process::Command;

/// Build a macOS sandbox profile that restricts a child process.
fn build_macos_sandbox_profile(
    project_path: &str,
    allow_network: bool,
) -> String {
    let network_rule = if allow_network {
        "(allow network*)"
    } else {
        "(deny network*)"
    };

    format!(
        r#"(version 1)
(deny default)

; Allow reading standard system paths
(allow file-read*
    (subpath "/usr/lib")
    (subpath "/usr/bin")
    (subpath "/bin")
    (subpath "/Library/Frameworks")
    (subpath "/System"))

; Allow reading and writing within the project directory
(allow file-read* file-write*
    (subpath "{project_path}"))

; Allow reading and writing to temp directories
(allow file-read* file-write*
    (subpath "/tmp")
    (subpath "/private/tmp"))

; Allow process execution (needed for cargo, git, etc.)
(allow process-exec)
(allow process-fork)

; Network access (configurable)
{network_rule}

; Allow basic system operations
(allow sysctl-read)
(allow mach-lookup)
"#,
        project_path = project_path,
        network_rule = network_rule,
    )
}

/// Execute a command inside a macOS sandbox.
fn execute_sandboxed_macos(
    command: &str,
    args: &[&str],
    project_path: &str,
    allow_network: bool,
) -> Result<String, String> {
    let profile = build_macos_sandbox_profile(project_path, allow_network);

    // Write the profile to a temporary file
    let profile_path = "/tmp/agent-sandbox.sb";
    std::fs::write(profile_path, &profile)
        .map_err(|e| format!("Failed to write sandbox profile: {}", e))?;

    // Execute the command through sandbox-exec
    let output = Command::new("sandbox-exec")
        .args(["-f", profile_path, command])
        .args(args)
        .current_dir(project_path)
        .output()
        .map_err(|e| format!("Sandbox execution failed: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(format!(
            "Command failed in sandbox: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn main() {
    let profile = build_macos_sandbox_profile("/home/user/myproject", false);
    println!("Generated macOS sandbox profile:\n{}", profile);

    // In production, you would call:
    // execute_sandboxed_macos("cargo", &["test"], "/home/user/myproject", false)
    println!("\nTo run a sandboxed command:");
    println!("  sandbox-exec -f /tmp/agent-sandbox.sb cargo test");
}
```

## Linux Sandboxing with Namespaces and seccomp

On Linux, namespaces and seccomp provide powerful isolation primitives. Namespaces create isolated views of system resources (filesystem, network, PIDs), while seccomp filters restrict which system calls a process can make:

```rust
use std::process::Command;

/// Build command-line arguments for running a command in a Linux namespace sandbox.
/// Uses `unshare` for namespace isolation and an optional seccomp profile.
fn build_linux_sandbox_command(
    command: &str,
    args: &[&str],
    project_path: &str,
    isolate_network: bool,
) -> Command {
    let mut cmd = Command::new("unshare");

    // Create new mount namespace (isolate filesystem view)
    cmd.arg("--mount");

    // Create new PID namespace (isolate process view)
    cmd.arg("--pid");
    cmd.arg("--fork");

    // Optionally create new network namespace (isolate network)
    if isolate_network {
        cmd.arg("--net");
    }

    // Map current user to root inside the namespace (unprivileged)
    cmd.arg("--map-root-user");

    // The actual command to run
    cmd.arg("--");
    cmd.arg(command);
    cmd.args(args);

    cmd.current_dir(project_path);
    cmd
}

/// Generate a seccomp BPF filter configuration (JSON format for use with tools
/// like `seccomp-tools` or directly with the `seccomp` crate).
fn build_seccomp_profile() -> String {
    // This is a simplified representation -- in practice you would use
    // the seccomp crate or generate a BPF program
    let profile = serde_json_like_string(&[
        // Allow basic I/O
        "read", "write", "open", "close", "stat", "fstat", "lstat",
        // Allow memory management
        "mmap", "mprotect", "munmap", "brk",
        // Allow process management (for cargo, git)
        "fork", "vfork", "clone", "execve", "wait4",
        // Allow file operations within the sandbox
        "access", "getcwd", "chdir", "rename", "unlink", "mkdir", "rmdir",
        // DENY: dangerous syscalls
        // "reboot", "mount", "umount", "ptrace", "kexec_load",
        // "init_module", "delete_module", "pivot_root"
    ]);
    profile
}

fn serde_json_like_string(syscalls: &[&str]) -> String {
    let items: Vec<String> = syscalls
        .iter()
        .map(|s| format!("    \"{}\"", s))
        .collect();
    format!("{{\n  \"allowed_syscalls\": [\n{}\n  ]\n}}", items.join(",\n"))
}

fn main() {
    let cmd = build_linux_sandbox_command(
        "cargo",
        &["test"],
        "/home/user/myproject",
        true,
    );
    println!("Linux sandbox command: {:?}\n", cmd);

    let seccomp = build_seccomp_profile();
    println!("Seccomp profile:\n{}", seccomp);
}
```

## Container-Based Sandboxing

For the strongest isolation, run agent commands inside a container. This provides filesystem, network, and process isolation with well-tested tooling:

```rust
use std::process::Command;

/// Configuration for running agent commands in a Docker container.
struct ContainerSandbox {
    /// Docker image to use
    image: String,
    /// Host path to the project directory
    project_path: String,
    /// Where to mount the project inside the container
    container_mount: String,
    /// Whether to allow network access
    network_enabled: bool,
    /// Memory limit in megabytes
    memory_limit_mb: u32,
    /// CPU limit (number of cores)
    cpu_limit: f32,
}

impl ContainerSandbox {
    fn new(project_path: &str) -> Self {
        Self {
            image: "rust:latest".into(),
            project_path: project_path.into(),
            container_mount: "/workspace".into(),
            network_enabled: false,
            memory_limit_mb: 2048,
            cpu_limit: 2.0,
        }
    }

    /// Build the docker run command for executing a command in the sandbox.
    fn build_command(&self, command: &str, args: &[&str]) -> Command {
        let mut cmd = Command::new("docker");
        cmd.arg("run");

        // Remove container after execution
        cmd.arg("--rm");

        // Mount project directory
        cmd.args([
            "-v",
            &format!("{}:{}", self.project_path, self.container_mount),
        ]);

        // Set working directory
        cmd.args(["-w", &self.container_mount]);

        // Network isolation
        if !self.network_enabled {
            cmd.args(["--network", "none"]);
        }

        // Resource limits
        cmd.args(["--memory", &format!("{}m", self.memory_limit_mb)]);
        cmd.args(["--cpus", &format!("{}", self.cpu_limit)]);

        // Security options: no new privileges, drop all capabilities
        cmd.arg("--security-opt=no-new-privileges");
        cmd.arg("--cap-drop=ALL");

        // Read-only root filesystem (project mount is still writable)
        cmd.arg("--read-only");

        // Temp filesystem for cargo/build artifacts
        cmd.args(["--tmpfs", "/tmp:rw,noexec,nosuid"]);

        // The image and command
        cmd.arg(&self.image);
        cmd.arg(command);
        cmd.args(args);

        cmd
    }
}

fn main() {
    let sandbox = ContainerSandbox::new("/home/user/myproject");
    let cmd = sandbox.build_command("cargo", &["test"]);

    println!("Container sandbox command: {:?}", cmd);
    println!("\nSandbox properties:");
    println!("  Network: {}", if sandbox.network_enabled { "enabled" } else { "disabled" });
    println!("  Memory limit: {}MB", sandbox.memory_limit_mb);
    println!("  CPU limit: {} cores", sandbox.cpu_limit);
    println!("  Root filesystem: read-only");
    println!("  Capabilities: all dropped");
}
```

::: wild In the Wild
Codex uses a particularly aggressive sandboxing approach: every agent session runs in a sandboxed environment with network access disabled by default. The agent can read and write files within the project, but it cannot make outbound connections, preventing data exfiltration entirely. Claude Code takes a lighter approach on the sandboxing side, relying more on its permission and approval system to control what commands can run, but it does restrict file access to the project directory and blocks known dangerous commands. The tradeoff is clear: Codex sacrifices some agent capability (no network means no downloading dependencies during a session) in exchange for stronger isolation guarantees.
:::

::: python Coming from Python
Python developers might use `docker` or `subprocess` with limited environment variables for sandboxing. The Rust approach shown here produces the same Docker commands, but the type system ensures that all sandbox parameters are configured before the command runs. You cannot accidentally create a `ContainerSandbox` without specifying resource limits, because the struct requires them. In Python, you might forget to pass `--network none` to a `docker run` call, leaving a security gap.
:::

## Escape Hatches: Controlled Access Outside the Sandbox

Sometimes the agent legitimately needs access to resources outside the sandbox -- downloading a dependency, accessing a database for testing, or reading system configuration. Escape hatches provide controlled exceptions:

```rust
/// A temporary exception to sandbox restrictions.
#[derive(Debug, Clone)]
struct SandboxEscapeHatch {
    /// What resource is being accessed
    resource: String,
    /// Why this exception is needed
    justification: String,
    /// How long this exception lasts
    duration_seconds: u64,
    /// Was this approved by the user?
    user_approved: bool,
}

/// Manages temporary exceptions to the sandbox policy.
struct EscapeHatchManager {
    active_hatches: Vec<SandboxEscapeHatch>,
}

impl EscapeHatchManager {
    fn new() -> Self {
        Self {
            active_hatches: Vec::new(),
        }
    }

    fn request_escape(
        &mut self,
        resource: &str,
        justification: &str,
        duration: u64,
    ) -> &SandboxEscapeHatch {
        // In production, this would prompt the user for approval
        let hatch = SandboxEscapeHatch {
            resource: resource.to_string(),
            justification: justification.to_string(),
            duration_seconds: duration,
            user_approved: true, // would come from user prompt
        };
        self.active_hatches.push(hatch);
        self.active_hatches.last().unwrap()
    }

    fn is_resource_allowed(&self, resource: &str) -> bool {
        self.active_hatches
            .iter()
            .any(|h| h.resource == resource && h.user_approved)
    }
}

fn main() {
    let mut manager = EscapeHatchManager::new();

    manager.request_escape(
        "network:crates.io",
        "Need to download dependencies for cargo build",
        60,
    );

    println!("crates.io allowed: {}", manager.is_resource_allowed("network:crates.io"));
    println!("github.com allowed: {}", manager.is_resource_allowed("network:github.com"));
}
```

## Key Takeaways

- Application-level filters (allowlists/denylists) can be bypassed through creative command construction; OS-level sandboxing enforces boundaries at the kernel level
- macOS sandbox-exec and Linux namespaces/seccomp provide OS-native sandboxing without requiring containers, though containers offer the strongest isolation
- The tradeoff between sandboxing strictness and agent capability is fundamental -- stricter sandboxes prevent more attacks but also prevent legitimate operations like downloading dependencies
- Container-based sandboxing with `--network none`, `--cap-drop=ALL`, and `--read-only` provides defense-in-depth that is extremely difficult to escape
- Escape hatches provide a controlled mechanism for temporarily relaxing sandbox restrictions when the agent needs access to external resources, but they require explicit user approval
