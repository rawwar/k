---
title: Tool Categories
description: Classifying tools by function — file operations, shell execution, search, code editing, and information retrieval — and their design patterns.
---

# Tool Categories

> **What you'll learn:**
> - The five primary categories of coding agent tools and the unique design considerations for each
> - How read-only tools differ from mutating tools in terms of safety, confirmation, and rollback requirements
> - The minimum viable tool set for a useful coding agent and how to prioritize tool development

Now that you understand how tools are defined, validated, executed, and reported, let's look at the *kinds* of tools a coding agent needs. Every coding agent tool falls into one of five categories, each with its own design constraints and safety profile.

## Category 1: File Reading Tools

File reading tools let the agent examine the current state of the codebase. They are the agent's eyes.

**Typical tools in this category:**
- `read_file` -- read a file's content (or a slice of it)
- `list_files` -- list files in a directory, optionally matching a glob pattern
- `file_info` -- get metadata about a file (size, modification time, type)

**Design considerations:**

File reading tools are inherently safe because they do not modify anything. However, they still require careful design:

```rust
use serde::Deserialize;
use schemars::JsonSchema;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadFileInput {
    /// Absolute path to the file to read.
    pub path: String,

    /// 1-based line number to start reading from. Defaults to 1.
    pub offset: Option<u32>,

    /// Maximum number of lines to return. Defaults to 2000.
    pub limit: Option<u32>,
}

pub fn execute_read_file(input: ReadFileInput) -> Result<String, String> {
    let content = std::fs::read_to_string(&input.path)
        .map_err(|e| format!("Cannot read '{}': {}", input.path, e))?;

    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();

    let offset = input.offset.unwrap_or(1) as usize;
    let limit = input.limit.unwrap_or(2000) as usize;

    // Convert from 1-based to 0-based indexing
    let start = (offset - 1).min(total);
    let end = (start + limit).min(total);

    let selected: String = lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{:>4} | {}", start + i + 1, line))
        .collect::<Vec<_>>()
        .join("\n");

    if end < total {
        Ok(format!(
            "{}\n\n[Showing lines {}-{} of {}. Use offset={} to see more.]",
            selected,
            start + 1,
            end,
            total,
            end + 1
        ))
    } else {
        Ok(selected)
    }
}
```

Notice the line numbering in the output. Adding line numbers makes it easier for the model to reference specific lines when it needs to make edits later. This is a small detail that significantly improves edit accuracy.

**Safety profile:** Low risk. Restrict to the project directory to prevent reading sensitive system files.

## Category 2: File Mutation Tools

File mutation tools let the agent change files. They are the agent's hands.

**Typical tools in this category:**
- `write_file` -- create or overwrite a file
- `edit_file` -- make a targeted edit (find and replace a specific string)
- `create_directory` -- create a directory structure

**Design considerations:**

Mutation tools need safeguards that reading tools do not:

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EditFileInput {
    /// Absolute path to the file to edit.
    pub path: String,

    /// The exact text to find in the file. Must match exactly one location.
    pub old_string: String,

    /// The replacement text.
    pub new_string: String,
}

pub fn execute_edit_file(input: EditFileInput) -> Result<String, String> {
    let content = std::fs::read_to_string(&input.path)
        .map_err(|e| format!("Cannot read '{}': {}", input.path, e))?;

    // Verify the old_string appears exactly once
    let count = content.matches(&input.old_string).count();

    if count == 0 {
        return Err(format!(
            "The old_string was not found in '{}'. \
             Make sure you copied the exact text including whitespace and indentation. \
             Use read_file to see the current file contents.",
            input.path
        ));
    }

    if count > 1 {
        return Err(format!(
            "The old_string was found {} times in '{}'. \
             Provide a longer string with more surrounding context to match exactly one location.",
            count, input.path
        ));
    }

    let new_content = content.replacen(&input.old_string, &input.new_string, 1);
    std::fs::write(&input.path, &new_content)
        .map_err(|e| format!("Cannot write '{}': {}", input.path, e))?;

    Ok(format!(
        "Edited '{}': replaced {} characters with {} characters.",
        input.path,
        input.old_string.len(),
        input.new_string.len()
    ))
}
```

The edit tool validates that the `old_string` appears exactly once. This prevents accidental replacements in the wrong location -- a critical safety measure for a tool that modifies source code.

**Safety profile:** Medium to high risk. Require confirmation for destructive operations. Consider creating backups before edits.

::: python Coming from Python
Python developers might be tempted to implement file editing with regex:
```python
import re
new_content = re.sub(pattern, replacement, content)
```
Resist this temptation for agent tools. Regex-based editing is fragile because the model might not correctly escape special characters. String-based find-and-replace (like the `old_string`/`new_string` approach) is far more reliable because it matches exactly what the model sees in the file.
:::

## Category 3: Shell Execution Tools

Shell execution tools let the agent run arbitrary commands. They are the agent's general-purpose power tool.

**Typical tools in this category:**
- `shell` or `bash` -- run a shell command and return output
- `background_shell` -- run a long-running command in the background

**Design considerations:**

Shell tools are the most powerful and most dangerous tools in your agent. They can do almost anything -- install packages, compile code, run tests, delete files, or exfiltrate data.

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ShellInput {
    /// The shell command to execute.
    pub command: String,

    /// Working directory for the command. Defaults to the project root.
    pub cwd: Option<String>,

    /// Timeout in seconds. Defaults to 120.
    pub timeout: Option<u64>,
}
```

**Safety profile:** Highest risk. Requires deny-listing of dangerous commands, sandboxing, and often user confirmation. We cover this extensively in the Security Considerations subchapter.

## Category 4: Search Tools

Search tools let the agent find relevant code and information without reading entire files. They are the agent's flashlight in a dark codebase.

**Typical tools in this category:**
- `search_files` -- grep-like content search across files
- `find_definition` -- find where a function, class, or variable is defined
- `find_references` -- find all usages of a symbol

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchFilesInput {
    /// Regex pattern to search for.
    pub pattern: String,

    /// Directory to search in. Defaults to the project root.
    pub directory: Option<String>,

    /// Glob pattern to filter files (e.g., "*.rs", "*.py").
    pub file_glob: Option<String>,

    /// Maximum number of results to return. Defaults to 50.
    pub max_results: Option<u32>,
}
```

**Design considerations:**

Search tools need to be fast, because the model uses them frequently to explore unfamiliar code. They also need sensible defaults for result limits -- returning 10,000 matches overwhelms the context window.

**Safety profile:** Low risk (read-only), but can be expensive in terms of compute if the search scope is too broad.

::: wild In the Wild
Claude Code includes both a regex-based search tool (similar to ripgrep) and a more structured code search. OpenCode bundles a file search tool that supports glob-based file finding and content-based searching. Both agents cap search results to prevent flooding the context window -- typically returning 50-200 matches with a note indicating if results were truncated.
:::

## Category 5: Information and Utility Tools

This is a catch-all category for tools that provide information or perform utility functions without directly operating on the codebase.

**Typical tools in this category:**
- `think` -- a "scratchpad" tool that lets the model reason without taking action
- `web_search` -- search the web for documentation or solutions
- `ask_user` -- ask the user a clarifying question
- `get_context` -- retrieve the project root, current branch, or other environment info

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ThinkInput {
    /// The model's reasoning or analysis. This text is recorded but no action is taken.
    pub thought: String,
}

pub fn execute_think(input: ThinkInput) -> Result<String, String> {
    // The think tool does nothing except acknowledge the thought.
    // Its purpose is to give the model a place to reason
    // without taking action.
    Ok("Thought recorded. Continue with your plan.".to_string())
}
```

The `think` tool might seem pointless -- why would a model need a tool to think? The answer is that some tasks benefit from explicit reasoning before action. When the model uses the `think` tool, it forces itself to articulate a plan before diving into edits, which reduces mistakes on complex tasks.

**Safety profile:** Minimal to none. These tools are information-only.

## The Minimum Viable Tool Set

If you are building a coding agent from scratch, here is the minimum set of tools that makes the agent genuinely useful:

1. **`read_file`** -- the agent must be able to examine code
2. **`write_file`** -- the agent must be able to create files
3. **`edit_file`** -- the agent must be able to modify existing files without rewriting them entirely
4. **`list_files`** -- the agent must be able to discover project structure
5. **`search_files`** -- the agent must be able to find relevant code
6. **`shell`** -- the agent must be able to run commands (compile, test, install)

These six tools cover the complete development workflow: explore, understand, modify, and verify. Everything else is an optimization -- more specialized tools that make specific tasks faster or more reliable.

In the next chapter (File System Operations), you will implement the first four tools. In the chapter after that (Shell Execution), you will implement the shell tool. The search tool comes in the Code Search chapter.

## Read-Only vs Mutating: A Summary

| Aspect | Read-Only Tools | Mutating Tools |
|---|---|---|
| Examples | read_file, list_files, search_files | write_file, edit_file, shell |
| Side effects | None | Modifies file system or runs processes |
| Confirmation | Never needed | Often needed for destructive ops |
| Rollback | N/A | Should support undo or backups |
| Permission model | Allow by default | Restrict by default |
| Error impact | Low (wasted tokens) | High (corrupted files) |

## Key Takeaways

- Coding agent tools fall into five categories: file reading, file mutation, shell execution, search, and information/utility
- Read-only tools are inherently safer than mutating tools, but all tools need validation and project-directory restrictions
- The minimum viable tool set is six tools: read_file, write_file, edit_file, list_files, search_files, and shell
- Each category has unique design constraints -- mutation tools need confirmation and rollback, search tools need result limits, shell tools need sandboxing
- Start with the minimum viable set and add specialized tools only when you observe that the agent struggles with specific tasks
