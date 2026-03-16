---
title: Resource Limits
description: Enforcing CPU time, memory, file size, and file descriptor limits on child processes using rlimit and cgroups to prevent resource exhaustion.
---

# Resource Limits

> **What you'll learn:**
> - How to set per-process resource limits using setrlimit/getrlimit for CPU time, memory, and file sizes
> - Using cgroups v2 to enforce memory and CPU limits on process groups
> - Designing a resource limit policy for an agent that balances safety with usability

A coding agent must protect itself and the host system from resource exhaustion. A command that allocates unbounded memory can trigger the OOM killer and crash the entire system. A tight loop that consumes 100% CPU makes the machine unresponsive. A command that creates millions of temporary files fills the disk. Resource limits cap these behaviors before they cause damage, acting as guardrails that complement the sandboxing and timeout techniques you have already learned.

## Understanding Resource Limits (rlimit)

Every Unix process has a set of resource limits, managed through the `setrlimit` and `getrlimit` system calls. Each limit has two values:

- **Soft limit**: The effective limit. The process can raise its own soft limit up to the hard limit.
- **Hard limit**: The ceiling. Only root can raise the hard limit.

When a process exceeds a soft limit, the kernel takes action -- typically sending a signal or returning an error from the system call that exceeded the limit.

The most useful limits for a coding agent:

| Resource | Constant | What It Limits | Effect When Exceeded |
|----------|----------|---------------|---------------------|
| CPU time | RLIMIT_CPU | Seconds of CPU time | SIGXCPU, then SIGKILL |
| Address space | RLIMIT_AS | Virtual memory size | malloc returns NULL |
| File size | RLIMIT_FSIZE | Maximum file size | SIGXFSZ |
| Open files | RLIMIT_NOFILE | Number of file descriptors | open() returns EMFILE |
| Processes | RLIMIT_NPROC | Number of child processes | fork() returns EAGAIN |
| Core dump | RLIMIT_CORE | Core dump file size | Truncated or no core dump |

## Setting Resource Limits in Rust

Use the `nix` crate's `setrlimit` function, typically inside a `pre_exec` hook so the limits apply to the child process:

```rust
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use nix::sys::resource::{setrlimit, Resource};

fn spawn_with_limits(
    program: &str,
    args: &[&str],
    cpu_secs: u64,
    mem_bytes: u64,
    max_files: u64,
) -> std::io::Result<std::process::Child> {
    unsafe {
        Command::new(program)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .pre_exec(move || {
                // Limit CPU time
                setrlimit(Resource::RLIMIT_CPU, cpu_secs, cpu_secs)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

                // Limit virtual memory
                setrlimit(Resource::RLIMIT_AS, mem_bytes, mem_bytes)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

                // Limit open file descriptors
                setrlimit(Resource::RLIMIT_NOFILE, max_files, max_files)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

                // Prevent fork bombs
                setrlimit(Resource::RLIMIT_NPROC, 100, 100)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

                Ok(())
            })
            .spawn()
    }
}

fn main() {
    let result = spawn_with_limits(
        "sh",
        &["-c", "echo hello; ulimit -a"],
        30,                      // 30 seconds CPU time
        512 * 1024 * 1024,       // 512 MB memory
        256,                     // 256 open files
    );

    match result {
        Ok(mut child) => {
            let output = child.wait_with_output().expect("wait failed");
            println!("{}", String::from_utf8_lossy(&output.stdout));
        }
        Err(e) => eprintln!("Failed to spawn: {}", e),
    }
}
```

::: tip Coming from Python
Python has the `resource` module in its standard library:
```python
import resource
import subprocess

def set_limits():
    # 30 seconds CPU time
    resource.setrlimit(resource.RLIMIT_CPU, (30, 30))
    # 512 MB memory
    resource.setrlimit(resource.RLIMIT_AS, (512 * 1024 * 1024, 512 * 1024 * 1024))

result = subprocess.run(["some_command"], preexec_fn=set_limits, capture_output=True)
```
Rust's `pre_exec` hook serves the same purpose as Python's `preexec_fn` -- both run code in the child process after fork but before exec. Rust's version is `unsafe` because of the fork-safety constraints.
:::

## CPU Time Limits in Detail

RLIMIT_CPU limits the total CPU time (not wall-clock time) consumed by the process. When the soft limit is reached, the kernel sends SIGXCPU. If the process ignores it and continues, the kernel sends SIGKILL when the hard limit is reached.

You can set the soft limit slightly below the hard limit to give the process a chance to clean up:

```rust
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use nix::sys::resource::{setrlimit, Resource};

fn spawn_with_cpu_limit(program: &str, args: &[&str]) -> std::io::Result<std::process::Child> {
    unsafe {
        Command::new(program)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .pre_exec(|| {
                // Soft limit: 25 seconds (SIGXCPU)
                // Hard limit: 30 seconds (SIGKILL)
                setrlimit(Resource::RLIMIT_CPU, 25, 30)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                Ok(())
            })
            .spawn()
    }
}

fn main() {
    // This will be killed after ~30 seconds of CPU time
    let result = spawn_with_cpu_limit("sh", &["-c", "while true; do :; done"]);
    match result {
        Ok(mut child) => {
            let status = child.wait().expect("wait failed");
            println!("Infinite loop terminated: {}", status);
        }
        Err(e) => eprintln!("Spawn failed: {}", e),
    }
}
```

Note the distinction between CPU time and wall-clock time. A process that sleeps for an hour but uses only 1 second of CPU is not affected by RLIMIT_CPU. For wall-clock timeouts, use `tokio::time::timeout` as covered in the signals and timeouts subchapter.

## Memory Limits

RLIMIT_AS limits the process's total virtual memory (address space). When the limit is reached, `mmap` and `brk` (the system calls behind `malloc`) fail, which typically causes the process to crash or exit with an error.

There is an important caveat: RLIMIT_AS limits *virtual* memory, not *physical* (resident) memory. A process might map large virtual regions that are never used. For more precise physical memory control, use cgroups (see below).

```rust
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use nix::sys::resource::{setrlimit, Resource};

fn spawn_memory_limited(program: &str, args: &[&str], max_mb: u64) -> std::io::Result<std::process::Child> {
    let max_bytes = max_mb * 1024 * 1024;
    unsafe {
        Command::new(program)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .pre_exec(move || {
                setrlimit(Resource::RLIMIT_AS, max_bytes, max_bytes)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                Ok(())
            })
            .spawn()
    }
}

fn main() {
    // Limit to 128 MB -- enough for simple commands, not enough for a full compile
    let result = spawn_memory_limited("python3", &["-c", "x = 'a' * (200 * 1024 * 1024)"], 128);
    match result {
        Ok(mut child) => {
            let output = child.wait_with_output().expect("wait failed");
            if !output.status.success() {
                eprintln!("Process failed (memory limit): {}", String::from_utf8_lossy(&output.stderr));
            }
        }
        Err(e) => eprintln!("Spawn failed: {}", e),
    }
}
```

## cgroups v2: Group-Level Resource Control

While rlimits apply to individual processes, **cgroups** (control groups) apply limits to groups of processes. This is critical because a process can fork children that each get their own rlimit quotas. With cgroups, all processes in the group share a single memory budget.

cgroups v2 uses a filesystem-based interface at `/sys/fs/cgroup/`. Creating and configuring cgroups requires appropriate permissions (root or delegated authority).

```rust
use std::fs;
use std::path::Path;

fn create_cgroup(name: &str, memory_max: u64, cpu_max_usec: u64, period_usec: u64) -> std::io::Result<()> {
    let cgroup_path = format!("/sys/fs/cgroup/{}", name);
    let path = Path::new(&cgroup_path);

    // Create the cgroup directory
    fs::create_dir_all(path)?;

    // Set memory limit
    fs::write(path.join("memory.max"), memory_max.to_string())?;

    // Set CPU limit (quota per period)
    // e.g., 50000/100000 = 50% of one CPU
    let cpu_max = format!("{} {}", cpu_max_usec, period_usec);
    fs::write(path.join("cpu.max"), cpu_max)?;

    Ok(())
}

fn add_process_to_cgroup(name: &str, pid: u32) -> std::io::Result<()> {
    let procs_path = format!("/sys/fs/cgroup/{}/cgroup.procs", name);
    fs::write(procs_path, pid.to_string())
}

fn cleanup_cgroup(name: &str) -> std::io::Result<()> {
    let cgroup_path = format!("/sys/fs/cgroup/{}", name);
    fs::remove_dir(cgroup_path)
}

fn main() {
    // Note: requires root or delegated cgroup permissions
    match create_cgroup("agent-cmd-1", 256 * 1024 * 1024, 50000, 100000) {
        Ok(()) => println!("cgroup created with 256MB memory, 50% CPU"),
        Err(e) => eprintln!("Failed (likely needs root): {}", e),
    }
}
```

In practice, many agents use container runtimes like Docker that handle cgroup management for you. But understanding the underlying mechanism helps you debug resource issues.

## Designing a Resource Limit Policy

A practical resource limit policy for a coding agent balances safety and usability:

```rust
pub struct ResourcePolicy {
    pub cpu_time_secs: u64,
    pub memory_mb: u64,
    pub max_open_files: u64,
    pub max_child_processes: u64,
    pub max_file_size_mb: u64,
}

impl ResourcePolicy {
    /// Reasonable defaults for a coding agent
    pub fn default_agent() -> Self {
        Self {
            cpu_time_secs: 120,        // 2 minutes CPU time
            memory_mb: 1024,           // 1 GB memory
            max_open_files: 1024,      // 1024 file descriptors
            max_child_processes: 256,  // Prevents fork bombs
            max_file_size_mb: 100,     // 100 MB max file size
        }
    }

    /// Stricter limits for untrusted or risky commands
    pub fn restricted() -> Self {
        Self {
            cpu_time_secs: 30,
            memory_mb: 256,
            max_open_files: 64,
            max_child_processes: 16,
            max_file_size_mb: 10,
        }
    }

    /// Relaxed limits for known-safe commands like cargo build
    pub fn build_tools() -> Self {
        Self {
            cpu_time_secs: 600,        // Builds can take a while
            memory_mb: 4096,           // Rust compiler is hungry
            max_open_files: 4096,      // Builds open many files
            max_child_processes: 512,  // Parallel compilation
            max_file_size_mb: 500,     // Build artifacts can be large
        }
    }
}
```

The key decisions:

1. **CPU time**: Long enough for legitimate builds and tests, short enough to catch infinite loops. Wall-clock timeouts (from the previous subchapter) provide a complementary backstop.

2. **Memory**: The Rust compiler (`rustc`) can use 1-4 GB for large projects. Set limits high enough for builds but low enough to prevent a command from starving the system.

3. **File descriptors**: Most tools need dozens, not thousands. Limiting file descriptors prevents a command from exhausting the system-wide FD table.

4. **Child processes**: The NPROC limit prevents fork bombs. Set it high enough for parallel compilation but low enough to prevent exponential process creation.

::: info In the Wild
Codex runs commands inside Docker containers with explicit memory and CPU limits set through Docker's resource constraints (which use cgroups under the hood). This gives strong isolation -- a runaway `cargo build` cannot consume all the host's memory. Claude Code takes a lighter approach, relying on wall-clock timeouts and process group management rather than strict memory limits, trading some safety for lower overhead and simpler setup.
:::

## Key Takeaways

- `setrlimit` sets per-process limits on CPU time, memory, file sizes, open files, and child processes. Apply limits in a `pre_exec` hook so they affect the child, not the parent.
- CPU time limits (RLIMIT_CPU) measure actual compute time, not wall-clock time. Combine them with `tokio::time::timeout` for comprehensive timeout coverage.
- Memory limits (RLIMIT_AS) cap virtual address space. For physical memory control across process groups, use cgroups v2.
- Design resource policies with multiple tiers: relaxed for trusted build tools, strict for unknown or risky commands.
- The NPROC limit is your primary defense against fork bombs -- set it on every child process your agent spawns.
