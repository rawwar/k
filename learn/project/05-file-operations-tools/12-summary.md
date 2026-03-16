---
title: Summary
description: Review the complete file operations tool suite and reflect on the agent's new ability to read, write, and edit code.
---

# Summary

> **What you'll learn:**
> - How the read, write, and edit tools combine to give the agent a complete file manipulation workflow
> - How the safety, atomicity, and testing patterns you built apply to every future tool you add
> - What your agent can now accomplish end-to-end: read code, reason about changes, edit files, and verify results

You started this chapter with an agent that could talk but could not touch code. Now it has hands. Let's review everything you built, how the pieces fit together, and what this means for the agent's capabilities.

## What You Built

Over the course of this chapter, you implemented a complete file operations toolkit:

**ReadFile** reads a file from disk and returns its contents with line numbers. It supports offset and limit parameters for navigating large files, detects binary files, and provides metadata when content cannot be returned directly.

**WriteFile** creates or overwrites files with complete content. It creates parent directories automatically, normalizes line endings, and returns confirmation with byte and line counts.

**EditFile** performs exact string replacement: find `old_string` in a file, verify it appears exactly once, replace it with `new_string`. The strict uniqueness requirement prevents accidental edits, and descriptive error messages guide the model to fix its input when matches fail.

**GlobSearch** discovers files by pattern, returning results sorted by modification time. The model uses this to find files before reading or editing them.

Supporting these four tools, you built several infrastructure layers:

**Path resolution** canonicalizes paths, resolves symlinks, and rejects any path that falls outside the allowed base directory.

**Safety checks** enforce blocklists for sensitive files, read-only restrictions for generated files, write size limits, and content-based secret detection.

**Permission handling** pre-checks file access, preserves original permissions across edits, and detects binary files.

**Diffing** generates unified diffs for every edit, displayed with color in the terminal for the user and included as text in the tool result for the model.

**Atomic writes** use the write-temp-rename pattern so files are never left in a corrupted state, even if the process crashes mid-write.

## The Agent Workflow

With these tools registered in your tool system from Chapter 4, the agent can now follow the complete development workflow:

```
1. Model receives a task: "Add error handling to the process function"
2. Model calls glob_search(pattern="**/*.rs") to find relevant files
3. Model calls read_file(path="src/process.rs") to see the current code
4. Model reasons about what needs to change
5. Model calls edit_file(
       path="src/process.rs",
       old_string="fn process(data: &str) {",
       new_string="fn process(data: &str) -> Result<(), Error> {"
   )
6. Model sees the diff in the tool result, confirming the edit
7. Model calls read_file(path="src/process.rs") again to verify
8. Model makes additional edits if needed
9. Model reports the changes to the user
```

This is exactly how a human developer works: find the file, read it, understand it, change it, verify the change. The difference is that the agent does it through structured tool calls rather than keyboard strokes.

::: tip Coming from Python
If you have used Python's `pathlib` for file operations, you have seen most of these patterns:
```python
# The Python equivalent of our tool suite
from pathlib import Path

# Read
content = Path("src/main.rs").read_text()

# Write
Path("src/new.rs").write_text("fn main() {}")

# Edit (no built-in -- you'd write this yourself)
content = Path("src/main.rs").read_text()
content = content.replace(old, new, 1)
Path("src/main.rs").write_text(content)

# Glob
files = list(Path(".").glob("**/*.rs"))
```
The Rust implementations are more verbose because of explicit error handling, but the logic is identical. The biggest difference is the safety infrastructure -- in Python you would typically not add path containment, atomic writes, or permission preservation unless you were building a security-sensitive application. For a coding agent, these safeguards are essential.
:::

## Architecture Diagram

Here is how the file tools fit into the overall agent architecture:

```
┌──────────────────────────────────────────────────┐
│                  Agentic Loop                     │
│  (Chapter 3)                                     │
│                                                  │
│  ┌────────────────────────────────────────────┐  │
│  │           Tool Registry (Ch 4)             │  │
│  │                                            │  │
│  │  ┌──────────┐ ┌──────────┐ ┌───────────┐  │  │
│  │  │ ReadFile │ │WriteFile │ │ EditFile  │  │  │
│  │  └────┬─────┘ └────┬─────┘ └─────┬─────┘  │  │
│  │       │             │             │        │  │
│  │  ┌────┴─────────────┴─────────────┴────┐   │  │
│  │  │        Path Resolution Layer        │   │  │
│  │  │   (canonicalize, boundary check)    │   │  │
│  │  └────┬────────────────────────────────┘   │  │
│  │       │                                    │  │
│  │  ┌────┴────────────────────────────────┐   │  │
│  │  │         Safety Checker              │   │  │
│  │  │  (blocklist, read-only, size limit) │   │  │
│  │  └────┬────────────────────────────────┘   │  │
│  │       │                                    │  │
│  │  ┌────┴────────────────────────────────┐   │  │
│  │  │    std::fs (with atomic writes)     │   │  │
│  │  └────────────────────────────────────-┘   │  │
│  └────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────┘
```

Every file operation flows through three layers: path resolution, safety checking, and then the actual filesystem call. This layered design means you can add new file tools (copy, move, delete) and they automatically get path safety and permission checking by using the same infrastructure.

## Patterns That Transfer to Future Tools

Several patterns from this chapter apply to every tool you will build:

**Input validation before execution.** Every tool extracts and validates its JSON parameters before doing any work. This catches malformed inputs early and returns clear error messages.

**Layered safety.** Path resolution, safety checks, and permission checks form a pipeline that every file operation passes through. When you build the shell execution tool in Chapter 6, you will apply the same pattern: validate the command, check against a blocklist, then execute.

**Descriptive error messages.** Error messages are written for the model, not just for humans. "Found 3 occurrences, include more context" is more useful than "match error" because it tells the model exactly how to fix the problem.

**Diff-based feedback.** Showing the model what changed gives it a self-verification loop. You will use the same pattern for shell command output, git diffs, and search results.

**Atomic operations.** Write-temp-rename prevents corruption. You will apply this to any operation where partial completion would leave the system in a bad state.

**Thorough testing.** TempDir-based tests with both return-value and filesystem-state verification. This pattern scales to any tool that has side effects.

::: tip In the Wild
Claude Code, OpenCode, and Codex all implement these same three core file tools (read, write, edit) with remarkably similar designs. The string-replacement edit tool with strict uniqueness checking has proven to be the most reliable approach across all production coding agents. The main variations between agents are in their safety systems: Claude Code uses a permission model with user approval, OpenCode uses a stricter blocklist, and Codex relies on Docker-level sandboxing. Our implementation provides a solid middle ground that you can extend in either direction.
:::

## What Comes Next

In Chapter 6, you will build the shell execution tool. The agent will be able to run commands like `cargo build`, `cargo test`, and `git status`. Combined with file operations, this means the agent can:

1. Read the code
2. Edit the code
3. Run the compiler to check for errors
4. Read the error output
5. Fix the errors
6. Run the tests to verify

This edit-compile-test loop is the core workflow of software development, and your agent is one tool away from doing it autonomously.

## Exercises

1. **(Easy)** Add a `delete_file` tool that removes a file after safety checks. Make sure it returns an error if the file does not exist.

2. **(Medium)** Extend the edit tool to support a `replace_all` boolean parameter that replaces all occurrences instead of requiring exactly one match. When `replace_all` is true, the tool should report how many replacements were made.

3. **(Medium)** Add a `copy_file` tool that copies a file from one path to another within the base directory. Preserve the original file's permissions on the copy.

4. **(Hard)** Implement a `search_content` tool that searches file contents for a regex pattern (using the `regex` crate) and returns matching lines with file paths and line numbers. This is the `grep` equivalent for your agent.

5. **(Hard)** Add an undo system to the edit tool: before every edit, save a backup of the original file content. Implement an `undo_edit` tool that restores the last version. Support multiple levels of undo.

## Key Takeaways

- The three core file tools (read, write, edit) combined with glob search give the agent everything it needs to navigate and modify a codebase, following the same find-read-edit-verify workflow a human developer uses.
- Safety is layered: path resolution prevents directory escapes, the safety checker blocks sensitive files, permission checks prevent access violations, and atomic writes prevent corruption. Each layer catches a different class of problem.
- Error messages are tool outputs: they go back to the model as observations, so they must be specific and actionable. "Include more context to make the match unique" is worth more than "match failed."
- The patterns from this chapter -- input validation, layered safety, descriptive errors, diff feedback, atomic operations, TempDir testing -- form a template for every tool you build from here on.
- Your agent can now read code, make targeted edits, and create new files. In the next chapter, it gains the ability to run commands, completing the edit-compile-test loop that is the heart of software development.
