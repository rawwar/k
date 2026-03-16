---
title: Unix Process Model
description: The foundational concepts of Unix processes including fork, exec, wait, and the process lifecycle that underpins all command execution in a coding agent.
---

# Unix Process Model

> **What you'll learn:**
> - How Unix processes are created via the fork/exec model and why this design persists
> - The relationship between parent and child processes, including process IDs, exit codes, and zombie processes
> - How file descriptor inheritance connects parent and child processes through pipes

Before you write a single line of Rust process-spawning code, you need a mental model of what the operating system actually does when you "run a command." Every coding agent -- whether it is compiling code, running tests, or executing shell scripts on the user's behalf -- ultimately asks the OS kernel to create a new process. Understanding this mechanism will help you debug hanging pipes, orphaned processes, and mysterious exit codes that would otherwise feel like black magic.

## The Process: A Running Program

A **process** is the OS's abstraction for a running program. Each process has:

- A **process ID (PID)** -- a unique integer assigned by the kernel.
- A **parent process ID (PPID)** -- the PID of the process that created it.
- An **address space** -- its own private view of memory (code, stack, heap).
- A set of **file descriptors** -- numbered handles to open files, pipes, sockets, and devices.
- An **environment** -- a collection of key-value string pairs (environment variables).
- A **working directory** -- the directory that relative paths resolve against.

When your coding agent process spawns `cargo test`, the kernel creates a new process with its own PID, its own copy of file descriptors, and its own memory space. The agent process becomes the **parent**, and `cargo test` becomes the **child**.

::: tip Coming from Python
In Python, you rarely think about processes at this level. When you call `subprocess.run(["cargo", "test"])`, Python hides fork/exec behind a friendly API. Rust's `std::process::Command` does the same abstraction, but understanding what happens underneath helps you reason about pipe deadlocks, zombie processes, and signal delivery -- problems that surface more often when you are building a long-running agent rather than a one-shot script.
:::

## Fork and Exec: The Two-Step Dance

Unix creates new processes through a two-step mechanism that dates back to the 1970s but remains the foundation of every modern Unix-like system (Linux, macOS, BSDs).

### Step 1: fork()

The `fork()` system call creates an almost exact copy of the calling process. After fork returns, there are two processes running the same code:

- The **parent** receives the child's PID as the return value of fork.
- The **child** receives 0 as the return value.

Both processes share the same code, the same open file descriptors (by duplication), and the same environment variables. The child gets a copy of the parent's memory -- though modern kernels use **copy-on-write** to avoid physically duplicating pages until one process modifies them.

### Step 2: exec()

The child process then calls one of the `exec()` family of system calls (`execve`, `execvp`, etc.) to replace its own program image with a new one. After exec, the child is running entirely different code -- say, the `cargo` binary. Its PID stays the same, its file descriptors stay open (unless marked close-on-exec), but the code and data segments are replaced.

Here is the conceptual flow in pseudocode:

```
parent process (PID 100)
  |
  |-- fork() --> child process (PID 101)
  |                |
  |                |-- exec("cargo", ["test"])
  |                |   (child is now running cargo)
  |                |
  |-- wait()       |-- ... cargo does its work ...
  |                |
  |   <-- exit(0) --  (cargo finishes)
  |
  |-- (parent resumes, reads exit code 0)
```

This two-step design may seem redundant -- why not have a single "create process from this program" call? The answer is that the gap between fork and exec is where you configure the child. In that window, the child can:

- Close or redirect file descriptors (set up pipes for stdout capture).
- Change its working directory.
- Modify environment variables.
- Set resource limits.
- Drop privileges.

Rust's `Command` builder configures all of these things, and under the hood it executes them in that fork-to-exec gap.

## Process Groups and Sessions

Processes are organized into **process groups**, and process groups are organized into **sessions**. This hierarchy matters for signal delivery.

A **process group** is a collection of related processes identified by a **process group ID (PGID)**, which is typically the PID of the group leader. When your agent spawns `sh -c "cargo test && cargo clippy"`, the shell may create a process group containing the shell itself, `cargo test`, and `cargo clippy`. Sending a signal to the process group (using a negative PID) delivers it to every member.

A **session** is a collection of process groups associated with a controlling terminal. When the user presses Ctrl+C in a terminal, the kernel sends SIGINT to the entire foreground process group of that session.

For a coding agent, process groups are important because you often want to kill not just the child process you spawned, but all of its descendants. If you spawn `sh -c "sleep 1000"`, killing only the shell leaves `sleep` running as an orphan. Killing the entire process group cleans up everything.

## Waiting and Exit Codes

After forking a child, the parent should eventually call `wait()` (or `waitpid()`) to collect the child's exit status. This is critical for two reasons:

1. **Exit code retrieval** -- the exit status tells you whether the command succeeded (conventionally, 0 means success, non-zero means failure). Your agent needs this to decide whether a tool invocation worked.

2. **Zombie prevention** -- until the parent calls wait, a terminated child lingers in the process table as a **zombie process**. It consumes no CPU or memory, but it occupies a PID slot. A long-running agent that spawns thousands of commands without waiting will eventually exhaust the system's PID space.

Exit codes are integers, but by convention:

| Exit Code | Meaning |
|-----------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Misuse of command |
| 126 | Command found but not executable |
| 127 | Command not found |
| 128 + N | Killed by signal N (e.g., 137 = killed by SIGKILL, signal 9) |

When your agent runs `cargo test` and gets exit code 101, it knows the tests failed. When it gets 137, it knows something sent SIGKILL -- perhaps your own timeout mechanism.

## File Descriptor Inheritance

When a process forks, the child inherits copies of all the parent's open file descriptors. This is how pipes work:

1. Before forking, the parent creates a pipe -- a pair of file descriptors, one for reading and one for writing.
2. After forking, the parent closes the write end and the child closes the read end (or vice versa).
3. Now the parent can read what the child writes to its stdout, because the child's stdout has been redirected to the write end of the pipe.

This is exactly what happens when you tell `Command` to capture stdout. Rust creates the pipe, forks, wires up the child's file descriptors, and gives you the read end.

```rust
use std::process::Command;

fn main() {
    // Rust handles all the fork/exec/pipe plumbing for you
    let output = Command::new("echo")
        .arg("hello from the child process")
        .output()
        .expect("failed to spawn process");

    // The parent reads from the pipe that was connected to the child's stdout
    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("Child said: {}", stdout.trim());
    println!("Exit code: {}", output.status.code().unwrap_or(-1));
}
```

Even though Rust's API is high-level, every call to `.output()` triggers the full fork/exec/pipe/wait sequence under the hood. Understanding this helps you reason about what can go wrong -- a pipe that fills up and blocks, a child that never exits, a file descriptor that leaks to a grandchild.

::: info In the Wild
Claude Code and Codex both spawn shell commands as child processes of the agent. Claude Code routes all commands through a sandboxed executor that creates dedicated process groups for each command, ensuring that timeouts can kill the entire tree of descendant processes rather than just the top-level shell. This prevents orphaned processes from accumulating during long agent sessions.
:::

## Why This Matters for Your Agent

Every subsequent subchapter in this chapter builds on these concepts:

- **Spawning** uses fork/exec (subchapter 2).
- **Capturing output** uses pipe inheritance (subchapter 3).
- **Stdin/stdout/stderr** are file descriptors 0, 1, and 2 (subchapter 4).
- **Signals and timeouts** are delivered to processes and process groups (subchapter 5).
- **Environment and working directory** are inherited from the parent across fork (subchapter 6).
- **Sandboxing** manipulates the fork-to-exec gap to restrict the child (subchapter 8).

With this foundation, you are ready to see how Rust wraps these primitives into a safe, ergonomic API.

## Key Takeaways

- Unix creates new processes via a two-step fork/exec model: fork copies the parent, exec replaces the copy with a new program.
- The gap between fork and exec is where file descriptors, environment variables, working directory, and resource limits are configured for the child.
- Parents must wait on children to collect exit codes and prevent zombie processes.
- Process groups allow signals to be delivered to an entire tree of related processes -- essential for timeouts that cleanly kill a command and all its descendants.
- File descriptor inheritance across fork is the mechanism that enables pipes, stdout capture, and stdin feeding in child processes.
