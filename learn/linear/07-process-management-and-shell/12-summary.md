---
title: Summary
description: Review of the Unix process model, Rust process APIs, and security practices covered in this chapter with connections to upcoming streaming and shell integration topics.
---

# Summary

> **What you'll learn:**
> - How the Unix process model, Rust's process APIs, and security practices fit together in a coding agent
> - Which patterns from this chapter you will apply in later chapters on streaming and terminal UIs
> - Key decision points when designing a process execution layer for production agent systems

You have covered a lot of ground in this chapter -- from the fundamentals of how Unix creates processes to the practical security measures that keep a coding agent from causing damage. Let's review the key concepts and see how they connect into the larger agent architecture.

## The Foundation: Unix Process Model

Everything in this chapter builds on a few core Unix primitives:

**fork/exec** creates new processes. The parent calls fork to create a copy of itself, and the child calls exec to replace itself with a new program. The gap between fork and exec is where you configure the child's environment -- setting up pipes, changing the working directory, modifying environment variables, and installing resource limits.

**File descriptors** connect processes. Pipes created before fork allow the parent to read the child's stdout and stderr, and to write to the child's stdin. These three standard streams (fd 0, 1, 2) are the communication backbone between your agent and the tools it runs.

**Signals** control processes. SIGTERM asks a process to shut down gracefully. SIGKILL forces immediate termination. Process groups let you send signals to an entire tree of related processes, ensuring that timeouts kill not just the top-level command but all its descendants.

**Exit codes** report results. By convention, 0 means success and non-zero means failure. Codes above 128 indicate the process was killed by a signal (128 + signal number).

## Rust's Process APIs

Rust wraps these Unix primitives in two complementary APIs:

**`std::process::Command`** provides synchronous process spawning. Its builder pattern configures the program, arguments, environment, working directory, and stdio before launching. The three execution methods -- `status()`, `output()`, and `spawn()` -- give you increasing levels of control. For quick one-off commands during initialization, the synchronous API is perfectly adequate.

**`tokio::process::Command`** mirrors the same API but returns futures that integrate with the async runtime. This is what your agent uses during normal operation, because blocking on a synchronous process call would freeze the entire event loop. The async `spawn()` method combined with `tokio::io::BufReader` gives you line-by-line streaming of command output -- essential for providing real-time feedback to users.

::: python Coming from Python
The relationship between `std::process::Command` and `tokio::process::Command` mirrors Python's `subprocess.run()` versus `asyncio.create_subprocess_exec()`. In both languages, the synchronous version is simpler but blocks the thread, while the async version integrates with the event loop. Rust makes the distinction more explicit through separate types in separate crates, while Python uses the same function patterns in different modules.
:::

## Stream Handling

Capturing output correctly requires understanding pipe mechanics:

- **Bulk capture** (`output()`) waits for the process to finish and returns all stdout/stderr at once. Simple but unsuitable for long-running commands.
- **Streaming** (`spawn()` with `BufReader`) reads output line by line as the process runs. This is what agents use to show real-time progress.
- **Concurrent reading** is essential: always read stdout and stderr in separate tasks to avoid pipe deadlocks. The OS pipe buffer (typically 64 KB) fills up quickly, and a blocked pipe freezes both processes.
- **Output truncation** prevents large outputs from overwhelming the LLM's context window. Capture the first N lines or bytes, and tell the LLM the output was truncated.

## Timeout and Signal Management

Commands can hang, and a coding agent must not hang with them:

- **`tokio::time::timeout`** wraps process execution with a deadline. When the timeout expires, kill the process.
- **Graceful shutdown** sends SIGTERM first, waits a grace period, then escalates to SIGKILL. This gives well-behaved processes a chance to clean up.
- **Process groups** (`.process_group(0)` on spawn, `killpg` on kill) ensure that timeouts clean up the entire process tree, not just the top-level shell.

## Security Layers

The security model is layered, with each layer catching what the others miss:

| Layer | What It Does | What It Catches |
|-------|-------------|-----------------|
| Command validation | Blocks known dangerous patterns | Obvious destructive commands |
| User confirmation | Prompts for approval of risky commands | Commands that pass validation but seem dangerous |
| Environment isolation | Clears secrets, restricts PATH | Information leakage, access to unauthorized tools |
| Sandboxing | Restricts file/network/process access | Commands that bypass validation |
| Resource limits | Caps CPU, memory, file descriptors, processes | Resource exhaustion, fork bombs |
| Timeouts | Kills long-running processes | Infinite loops, hanging commands |
| Audit logging | Records everything | Post-incident investigation |

The key principle is **defense in depth**: assume each layer will fail and design the next layer to catch what slips through.

## Shell vs. Direct Exec

This design decision affects everything else:

- **Direct exec** (`Command::new("cargo").arg("test")`) is inherently safe from injection. Arguments are passed literally. Use this whenever you can construct the command as a program name plus a list of arguments.
- **Shell invocation** (`sh -c "command string"`) enables pipes, redirects, and globbing but opens the door to injection. LLM-generated commands often need the shell because they contain these features.
- **In practice**, most agents use shell invocation with validation layers. The LLM generates natural command strings, and the agent validates them before passing to `sh -c`.

## What Comes Next

The patterns from this chapter feed directly into later chapters:

**Streaming and Real-Time (Chapter 8)**: The line-by-line output streaming you learned here is the foundation for real-time tool output display. You will connect process output streams to the terminal UI, showing command results as they arrive.

**Terminal User Interfaces (Chapter 9)**: The agent's TUI needs to display command output, show progress indicators for long-running commands, and handle user input for permission prompts -- all while the async runtime manages process execution in the background.

**Safety and Permissions (Chapter 13)**: The security hardening patterns from this chapter -- validation, risk assessment, user confirmation -- become a full permission system that governs not just shell commands but all tool invocations.

## Decision Checklist for Process Execution

When implementing process execution in your agent, consider these decision points:

1. **Sync or async?** Use async (`tokio::process::Command`) for everything during agent operation. Sync is acceptable only during startup before the runtime is running.

2. **Shell or direct exec?** If the command has pipes, redirects, or glob patterns, you need the shell. Otherwise, use direct exec.

3. **Capture or stream?** For quick commands (under a few seconds), bulk capture is simpler. For anything longer, stream output to provide real-time feedback.

4. **How long can it run?** Set a timeout on every command. The default might be 30 seconds for most tools, with longer limits for builds and test suites.

5. **What can it access?** Clear secrets from the environment. Consider sandboxing for commands that do not need network access or broad file system access.

6. **What if it fails?** Capture both stdout and stderr. Report the exit code. Return enough context for the LLM to understand what went wrong.

::: wild In the Wild
Claude Code, Codex, and OpenCode all implement variations of the patterns covered in this chapter. They share common design choices -- async process management, timeout enforcement, output capture, and security validation -- but differ in their approach to sandboxing (Claude Code uses platform-native sandboxing, Codex uses Docker containers, OpenCode relies more heavily on its permission system). These differences reflect tradeoffs between security, performance, and setup complexity that each team has made based on their specific deployment context.
:::

## Exercises

These exercises focus on reasoning about process management trade-offs, security design, and the challenges of letting an LLM execute arbitrary commands.

### Exercise 1: Dangerous Command Classification (Easy)

Classify each of these commands into a risk tier (safe/read-only, moderate/needs-confirmation, dangerous/should-block) and justify your classification:

1. `git log --oneline -20`
2. `cargo test`
3. `rm -rf target/`
4. `git push --force origin main`
5. `chmod -R 777 .`
6. `curl -s https://api.github.com/repos/owner/repo`
7. `docker run --privileged -v /:/host ubuntu bash`
8. `find . -name "*.rs" -type f`

**Deliverable:** A classification for each command with a one-sentence justification. Then propose a general rule for each tier that would correctly classify most commands without a hardcoded list.

### Exercise 2: Process Lifecycle Design for Long-Running Commands (Medium)

Design the lifecycle management for a `cargo build` command that might take 2-5 minutes on a large project. Your design should address: how the agent shows progress to the user, when (if ever) the agent should time out, how Ctrl+C is handled at different stages, what happens if the user sends a new message while the build is running, and how build output is streamed vs. captured.

**What to consider:** A 30-second default timeout would kill most builds. But an infinite timeout means a hanging build freezes the agent. Think about adaptive timeouts based on command type. Consider whether the agent should continue reasoning while the build runs, or block until it finishes.

**Deliverable:** A state diagram showing the process lifecycle from spawn to completion (or cancellation), a timeout strategy, a progress display design, and a handling plan for user interruption at each stage.

### Exercise 3: Signal Handling Edge Cases (Medium)

For each of these scenarios, describe what happens at the OS level and what your agent should do:

1. The agent sends SIGTERM to a process group, but one child process has installed a SIGTERM handler that ignores the signal
2. The agent spawns `sh -c "sleep 100 | wc -l"` and then needs to kill it -- how many processes exist and which ones need signals?
3. The agent's own process receives SIGTERM while it has a child process running -- what happens to the child?
4. A subprocess forks a daemon that detaches from the process group before the agent can kill the group

**What to consider:** Process groups and sessions in Unix are subtle. Think about what `setsid` does, how orphaned process groups work, and why `kill(0, sig)` sends to the entire group. Consider the gap between "theoretically correct" signal handling and "practically reliable" signal handling.

**Deliverable:** For each scenario, a description of the OS-level behavior, the failure mode for a naive implementation, and the correct handling strategy.

### Exercise 4: Sandboxing Strategy Comparison (Hard)

Compare three sandboxing approaches for a coding agent's shell execution: (a) macOS Seatbelt profiles (`sandbox-exec`), (b) Linux namespaces and seccomp filters, and (c) Docker/container-based isolation. For each approach, analyze: what resources it can restrict (filesystem, network, processes, syscalls), how it affects the agent's ability to run common development commands (`cargo build`, `npm install`, `git push`), the setup complexity for end users, and the failure mode when the sandbox blocks a legitimate operation.

**What to consider:** Development workflows often need network access (downloading dependencies), broad filesystem access (reading source code, writing build artifacts), and process spawning (compilers, test runners). A sandbox that blocks these is unusable. Think about the minimum viable sandbox that provides meaningful security without breaking common workflows.

**Deliverable:** A comparison matrix for the three approaches across the four dimensions, a recommendation for which to use on each platform, and a design for communicating sandbox-related failures to the LLM so it can adjust its approach.

## Key Takeaways

- The Unix process model (fork/exec/wait/signals) underpins all command execution in a coding agent. Understanding these primitives helps you debug hanging pipes, orphaned processes, and signal delivery issues.
- Use `tokio::process::Command` with async streaming for real-time output during agent operation. Reserve `std::process::Command` for synchronous startup tasks.
- Layer security defenses: command validation, user confirmation, environment isolation, sandboxing, resource limits, timeouts, and audit logging. No single layer is sufficient.
- Kill process groups (not just individual processes) when timeouts expire, to prevent orphaned descendant processes from accumulating.
- The patterns from this chapter -- process spawning, output streaming, timeout enforcement, and security validation -- are the building blocks for the tool execution layer that the rest of the agent depends on.
