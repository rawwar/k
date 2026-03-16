---
title: "Chapter 6: Shell Execution"
description: Spawning processes, capturing output, and implementing a shell execution tool with safety and timeout handling.
---

# Shell Execution

One of the most powerful capabilities an AI coding agent can have is the ability to execute shell commands. Running `cargo test`, `git diff`, `grep -r`, or any arbitrary command transforms your agent from a text-processing assistant into a genuine coding partner that can observe the real state of a project and take action on it.

This chapter takes you through the full journey of building a robust shell execution tool in Rust, from the fundamentals of process spawning to advanced concerns like sandboxing and dangerous command detection. You will learn how to use Rust's `std::process::Command` and Tokio's async process APIs to spawn child processes, capture their stdout and stderr streams, and enforce timeouts so your agent never hangs on a runaway command. We cover environment variable injection, working directory management, and output truncation strategies that keep context windows lean.

Safety is paramount when giving an AI agent shell access. An unchecked `rm -rf /` or a `curl | bash` piped from the internet could be catastrophic. You will implement a layered defense strategy including sandboxing basics, dangerous command detection heuristics, and configurable allow/deny lists. By the end of this chapter, you will have a production-quality shell execution tool integrated into your agent's tool system, complete with a comprehensive test suite.

## Learning Objectives

- Spawn and manage child processes using Rust's standard library and Tokio
- Capture and separate stdout and stderr streams from executed commands
- Implement timeout enforcement and signal handling for long-running processes
- Build a command builder abstraction with environment and working directory control
- Detect and block dangerous commands before execution
- Truncate oversized output to stay within LLM context window limits
- Write integration tests for shell tool behavior including edge cases

## Subchapters

1. [Process Spawning](/project/06-shell-execution/01-process-spawning) -- spawn child processes with `std::process::Command` and Tokio's async equivalent
2. [Stdout Stderr Capture](/project/06-shell-execution/02-stdout-stderr-capture) -- pipe and collect output streams into structured results
3. [Command Builder](/project/06-shell-execution/03-command-builder) -- design a fluent builder for configuring shell commands
4. [Timeouts](/project/06-shell-execution/04-timeouts) -- prevent runaway processes with `tokio::time::timeout`
5. [Signal Handling](/project/06-shell-execution/05-signal-handling) -- send SIGTERM/SIGKILL and manage process groups
6. [Environment Variables](/project/06-shell-execution/06-environment-variables) -- control variable inheritance and injection
7. [Working Directory](/project/06-shell-execution/07-working-directory) -- run commands in the correct project context
8. [Sandboxing Basics](/project/06-shell-execution/08-sandboxing-basics) -- restrict filesystem and network access for spawned processes
9. [Output Truncation](/project/06-shell-execution/09-output-truncation) -- keep command output within context window limits
10. [Dangerous Command Detection](/project/06-shell-execution/10-dangerous-command-detection) -- detect and block destructive command patterns
11. [Testing Shell Tool](/project/06-shell-execution/11-testing-shell-tool) -- unit and integration tests for the complete shell tool
12. [Summary](/project/06-shell-execution/12-summary) -- review the full implementation and safety design

## Prerequisites

- Chapter 4: Tool system architecture and the `Tool` trait
- Chapter 5: File operation tools and permission patterns
- Basic familiarity with terminal commands (`ls`, `echo`, `grep`, etc.)
