---
title: Sandboxing Approaches
description: Techniques for isolating child process execution including chroot, namespaces, seccomp, and macOS sandbox-exec to limit the blast radius of agent-run commands.
---

# Sandboxing Approaches

> **What you'll learn:**
> - The spectrum of sandboxing techniques from filesystem chroot to full container isolation
> - How Linux namespaces and seccomp filters restrict what a child process can access and do
> - Platform-specific sandboxing on macOS using sandbox-exec profiles and entitlements

A coding agent that executes arbitrary shell commands is inherently dangerous. Even with command validation and deny lists, a sufficiently creative (or hallucinating) LLM can produce commands that delete files, exfiltrate data, or modify system configuration. Sandboxing adds a structural layer of defense: instead of trying to predict every dangerous command, you constrain what any command *can* do. If a sandboxed process tries to write to `/etc/passwd` or open a network connection, the OS blocks it -- regardless of what the command is.

## The Sandboxing Spectrum

Sandboxing techniques form a spectrum from lightweight to heavyweight:

| Technique | Restricts | Platform | Overhead | Complexity |
|-----------|-----------|----------|----------|------------|
| Working directory + PATH | File access (soft), available programs | All | None | Low |
| Environment clearing | Information leakage | All | None | Low |
| chroot | Filesystem visibility | Linux/macOS | Low | Medium |
| seccomp-bpf | System calls | Linux | Very low | High |
| Linux namespaces | PID, network, mount, user spaces | Linux | Low | High |
| macOS sandbox-exec | File, network, process operations | macOS | Low | Medium |
| Container (Docker) | Full isolation | All (with runtime) | Medium | Medium |

For a coding agent, you typically combine several lightweight techniques rather than deploying full container isolation. The goal is to prevent catastrophic damage while keeping the agent useful -- a fully locked-down sandbox that cannot read project files is useless for a coding agent.

## chroot: Filesystem Isolation

The `chroot` system call changes the apparent root directory for a process. After chroot to `/home/user/project`, the process sees `/home/user/project` as `/` and cannot access anything outside it.

In Rust, you can use chroot via the `nix` crate's `chroot` function, but it requires root privileges on most systems. A more practical approach for an agent is to use chroot indirectly through container runtimes.

```rust
// Conceptual example -- requires root privileges
use std::os::unix::process::CommandExt;
use std::process::Command;

fn spawn_chrooted(root: &str, program: &str, args: &[&str]) -> std::io::Result<std::process::Child> {
    unsafe {
        Command::new(program)
            .args(args)
            .pre_exec(move || {
                // This runs in the child after fork, before exec
                nix::unistd::chroot(root)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                nix::unistd::chdir("/")
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                Ok(())
            })
            .spawn()
    }
}
```

The `pre_exec` hook runs in the child process after fork but before exec -- exactly where Unix expects you to configure the child's environment. Note the `unsafe` block: `pre_exec` is unsafe because the closure runs in a forked process where many things (like memory allocators) may not be in a consistent state.

::: tip Coming from Python
Python does not have a built-in `pre_exec` equivalent. The closest option is `subprocess.Popen`'s `preexec_fn` parameter, which has similar semantics and similar caveats about safety in forked processes. Python's documentation warns that `preexec_fn` is "not safe to use in the presence of threads," and Rust's `pre_exec` carries comparable warnings about async-signal-safety.
:::

## Linux Namespaces

Linux namespaces provide lightweight isolation without requiring root (for user namespaces) or full virtualization. Each namespace type isolates a different resource:

- **PID namespace**: The child sees its own PID space. It cannot see or signal processes outside its namespace.
- **Mount namespace**: The child has its own view of the filesystem mount table. Mounts created inside do not affect the host.
- **Network namespace**: The child gets its own network stack. It cannot access the host's network interfaces.
- **User namespace**: The child can map its user IDs independently. An unprivileged user can appear as root inside the namespace.

Creating namespaces from Rust requires the `clone` system call with namespace flags. The `nix` crate provides bindings:

```rust
use nix::sched::{CloneFlags, unshare};

fn isolate_network() -> nix::Result<()> {
    // Create a new network namespace for the current process
    // After this, the process has no network access (no interfaces, no routes)
    unshare(CloneFlags::CLONE_NEWNET)?;
    Ok(())
}

fn main() {
    // Note: CLONE_NEWNET requires either root or user namespace mapping
    match isolate_network() {
        Ok(()) => println!("Network isolated"),
        Err(e) => eprintln!("Failed to isolate network: {}", e),
    }
}
```

For a coding agent, the most useful namespaces are:

- **Network namespace**: Prevents commands from making network requests (no data exfiltration).
- **Mount namespace**: Allows you to create a restricted filesystem view.
- **PID namespace**: Prevents the command from seeing or killing other processes.

### Practical Namespace Sandbox

Here is how you might combine namespaces with `pre_exec`:

```rust
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

fn spawn_sandboxed(program: &str, args: &[&str], working_dir: &str) -> std::io::Result<std::process::Child> {
    unsafe {
        Command::new(program)
            .args(args)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .pre_exec(|| {
                // Attempt to create a new network namespace (blocks network access)
                // This may fail if the user doesn't have permission
                let _ = nix::sched::unshare(nix::sched::CloneFlags::CLONE_NEWNET);
                Ok(())
            })
            .spawn()
    }
}

fn main() {
    match spawn_sandboxed("curl", &["https://example.com"], "/tmp") {
        Ok(mut child) => {
            let output = child.wait_with_output().expect("wait failed");
            // curl should fail because the network namespace has no interfaces
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            println!("exit code: {:?}", output.status.code());
        }
        Err(e) => eprintln!("spawn failed: {}", e),
    }
}
```

## seccomp-bpf: System Call Filtering

seccomp (Secure Computing) allows you to install a BPF filter that restricts which system calls a process can make. After installing a seccomp filter, any attempt to call a blocked system call either returns an error or kills the process.

For a coding agent, seccomp is useful to block dangerous system calls like:
- `mount` / `umount` -- prevent filesystem manipulation
- `reboot` -- prevent system shutdown
- `ptrace` -- prevent debugging/inspection of other processes
- `kexec_load` -- prevent kernel replacement

Using seccomp from Rust requires the `seccompiler` or `libseccomp` crate:

```rust
// Conceptual example using the seccompiler crate
use std::collections::BTreeMap;
use seccompiler::{
    BpfProgram, SeccompAction, SeccompFilter, SeccompRule,
};

fn create_agent_seccomp_filter() -> BpfProgram {
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();

    // Block dangerous system calls by not including them
    // This is a allowlist approach: only listed syscalls are permitted

    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Errno(1),  // Default: deny with EPERM
        SeccompAction::Allow,      // Matched rules: allow
        std::env::consts::ARCH.try_into().unwrap(),
    )
    .expect("failed to create filter");

    filter.try_into().expect("failed to compile BPF")
}
```

seccomp filters are complex to configure correctly. A filter that is too restrictive will break normal tools (even `ls` needs many system calls). Most production agents use seccomp sparingly, targeting only the most dangerous system calls.

## macOS sandbox-exec

On macOS, the `sandbox-exec` command runs a process under a Seatbelt sandbox profile that restricts file access, network access, and other operations.

```rust
use std::process::{Command, Stdio};

fn spawn_macos_sandboxed(program: &str, args: &[&str]) -> std::io::Result<std::process::Output> {
    let profile = r#"
(version 1)
(deny default)
(allow process-exec)
(allow process-fork)
(allow file-read*)
(allow file-write* (subpath "/tmp"))
(allow file-write* (subpath "/private/tmp"))
(allow sysctl-read)
(allow mach-lookup)
"#;

    Command::new("sandbox-exec")
        .args(["-p", profile, program])
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
}

fn main() {
    // This will work: reading files is allowed
    match spawn_macos_sandboxed("ls", &["/tmp"]) {
        Ok(output) => println!("ls output:\n{}", String::from_utf8_lossy(&output.stdout)),
        Err(e) => eprintln!("Failed: {}", e),
    }

    // This would be blocked: writing outside /tmp is denied
    match spawn_macos_sandboxed("touch", &["/etc/test-file"]) {
        Ok(output) => {
            if !output.status.success() {
                println!("Blocked as expected: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
        Err(e) => eprintln!("Failed: {}", e),
    }
}
```

The sandbox profile is a Scheme-like DSL. The `(deny default)` line blocks everything, then individual `(allow ...)` rules open up specific capabilities. Common allowances for a coding agent include:

- `file-read*` -- allow reading files (needed to compile code, run tests)
- `file-write* (subpath "/path/to/project")` -- allow writing only within the project directory
- `process-exec`, `process-fork` -- allow executing programs
- Deny `network*` -- block network access

Note: Apple has deprecated `sandbox-exec` as a public API, but it continues to work on current macOS versions and is used internally by Apple's own tools. For production use, consider the App Sandbox or the newer `EndpointSecurity` framework.

::: info In the Wild
Claude Code uses macOS sandbox-exec profiles to restrict file system access for shell commands. The profile allows reads everywhere (the agent needs to read source code) but restricts writes to the project directory and temporary files. On Linux, it leverages a combination of namespace isolation and process group management. Codex runs commands inside a Docker container with a restricted filesystem, providing stronger isolation at the cost of startup overhead.
:::

## Choosing a Sandboxing Strategy

For a coding agent, a practical sandboxing strategy layers multiple techniques:

1. **Always**: Environment isolation (clean env, restricted PATH, controlled working directory).
2. **Recommended**: Process groups for clean signal delivery and cleanup.
3. **Platform-specific**: sandbox-exec on macOS, namespaces on Linux.
4. **For high-risk commands**: Container isolation (Docker) when available.

The key insight is that sandboxing is not all-or-nothing. Each layer adds protection, and the combination is stronger than any single technique.

## Key Takeaways

- Sandboxing constrains what a process *can* do, complementing command validation that tries to predict what it *will* do. Both layers are necessary for a secure agent.
- Linux namespaces (PID, mount, network) provide lightweight isolation without containers. Network namespaces are especially useful to prevent data exfiltration.
- seccomp-bpf filters restrict available system calls but are complex to configure correctly. Use them sparingly, targeting the most dangerous syscalls.
- macOS sandbox-exec uses profile files to restrict file, network, and process operations. It is the primary sandboxing tool on macOS.
- Layer multiple techniques: environment isolation, process groups, platform-specific sandboxing, and optionally container isolation for the strongest defense.
