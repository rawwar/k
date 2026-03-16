---
title: Editing Approaches
description: Comparing the major approaches to file editing in coding agents — full rewrite, string replacement, line-based edits, and AST manipulation.
---

# Editing Approaches

> **What you'll learn:**
> - The four main editing strategies agents use and the trade-offs of each for correctness, simplicity, and LLM compatibility
> - Why string replacement is the most reliable approach for LLM-driven edits and how Claude Code implements it
> - When full file rewrites are acceptable and when they introduce unacceptable risk of data loss

Editing files is where things get interesting -- and dangerous. Reading a file is non-destructive. Writing a new file from scratch is relatively safe (you can always delete it). But editing an existing file means transforming content that someone cares about, and getting the transformation wrong can introduce bugs, delete important code, or corrupt the file entirely.

In this subchapter, you'll survey the four main editing strategies used by coding agents and understand why string replacement emerged as the dominant approach. The next two subchapters ([String Replace vs Patch](/linear/06-file-system-operations/04-string-replace-vs-patch) and [Diff Algorithms](/linear/06-file-system-operations/05-diff-algorithms)) will dive deeper into the technical details.

## Strategy 1: Full File Rewrite

The simplest editing approach: the LLM generates the entire file from scratch.

```rust
use std::path::Path;

fn edit_by_rewrite(path: &Path, new_content: &str) -> Result<(), String> {
    // Just write the whole file
    // (using the safe write_tool from the previous subchapter)
    write_tool(path, new_content)
}
```

This is what happens when you tell an LLM "rewrite this file" and it outputs the complete new version. It is conceptually the simplest approach and requires zero diffing logic.

**When it works well:**
- Small files (under 100 lines) where the whole file fits comfortably in the output
- Creating new files from scratch
- Major refactors where most of the file changes

**When it fails:**
- Large files where the LLM might forget or hallucinate sections it didn't intend to change
- Files with subtle formatting that the LLM "normalizes" (changing indentation, reordering imports)
- When the token cost of outputting an entire 500-line file to change 3 lines is prohibitive

The biggest risk is **silent data loss**: the LLM rewrites a 400-line file but accidentally drops a helper function at the bottom. The write succeeds, the agent reports success, and nobody notices until the build breaks.

## Strategy 2: String Replacement

The LLM specifies an exact substring to find and its replacement:

```rust
use std::fs;
use std::path::Path;

pub fn edit_by_string_replace(
    path: &Path,
    old_string: &str,
    new_string: &str,
) -> Result<String, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Cannot read file: {e}"))?;

    // Check that old_string appears exactly once
    let count = content.matches(old_string).count();
    if count == 0 {
        return Err(format!(
            "The old_string was not found in {}",
            path.display()
        ));
    }
    if count > 1 {
        return Err(format!(
            "The old_string was found {} times in {} — it must be unique. \
             Include more surrounding context to make it unique.",
            count,
            path.display()
        ));
    }

    let new_content = content.replacen(old_string, new_string, 1);

    // Write atomically (using safe write from previous subchapter)
    write_tool(path, &new_content)?;

    Ok(format!(
        "Replaced {} characters with {} characters in {}",
        old_string.len(),
        new_string.len(),
        path.display()
    ))
}
```

This is the approach Claude Code uses for its Edit tool. The key insight is the **uniqueness requirement**: the old string must appear exactly once in the file. This prevents ambiguous edits where the same code pattern exists in multiple places.

**Why LLMs are good at this:** The model just needs to output the exact text it wants to replace and the exact replacement. It doesn't need to count line numbers, generate diff headers, or maintain context lines. It just copies a chunk from what it read and provides the modified version.

**Why this is safe:** If the old string is not found (perhaps the file changed between reading and editing), the operation fails cleanly. No modification happens. The model gets clear feedback about what went wrong.

::: wild In the Wild
Claude Code's Edit tool uses exactly this string replacement approach. The tool requires `old_string` to be unique in the file. If the match is not unique, the tool returns an error asking the model to include more surrounding context. This design means the model never has to generate line numbers or diff syntax -- it just specifies "find this exact text and replace it with this new text." This has proven to be the most reliable editing approach across millions of edits.
:::

## Strategy 3: Line-Based Edits

The LLM specifies edits by line number:

```rust
use std::fs;
use std::path::Path;

fn edit_by_line_range(
    path: &Path,
    start_line: usize,
    end_line: usize,
    replacement: &str,
) -> Result<String, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Cannot read file: {e}"))?;
    let mut lines: Vec<&str> = content.lines().collect();

    // Validate line numbers (1-indexed)
    let start = start_line.saturating_sub(1);
    let end = end_line.min(lines.len());
    if start >= lines.len() || start >= end {
        return Err("Invalid line range".into());
    }

    // Replace the line range with new content
    let replacement_lines: Vec<&str> = replacement.lines().collect();
    lines.splice(start..end, replacement_lines);

    let new_content = lines.join("\n");
    write_tool(path, &new_content)?;

    Ok(format!(
        "Replaced lines {}-{} in {}",
        start_line,
        end_line,
        path.display()
    ))
}
```

**The problem with line numbers:** LLMs are surprisingly bad at counting lines accurately. When the model reads a file and then generates an edit command referencing "line 47", it frequently gets the line number wrong by one or two. This is especially true after the model has made a previous edit that shifted line numbers. The off-by-one error rate makes line-based editing unreliable without additional validation.

## Strategy 4: AST-Based Editing

The most sophisticated approach: parse the file into an abstract syntax tree, modify the tree, then regenerate the source code.

```rust
// Conceptual example -- real AST editing requires a parser like tree-sitter
fn edit_by_ast(
    path: &str,
    function_name: &str,
    new_body: &str,
) -> Result<(), String> {
    // 1. Parse the file into an AST
    // 2. Find the function node by name
    // 3. Replace the function body
    // 4. Regenerate source code from modified AST
    // 5. Write back to disk
    todo!("Requires language-specific parser")
}
```

AST editing is powerful in theory: you could say "replace the body of function `process_input`" without specifying line numbers or exact text. But it has serious limitations:

- **Language-specific**: You need a parser for every language your agent supports.
- **Formatting loss**: Regenerating source from an AST often changes formatting, comments, and whitespace.
- **Complexity**: A full AST-aware editing system is an order of magnitude more complex than string replacement.

Some agents use AST parsing for *analysis* (understanding code structure) while using simpler approaches for *editing*. We will explore code intelligence features in [Chapter 11](/linear/11-code-intelligence/).

## Comparing the Approaches

| Approach | Reliability | LLM Compatibility | Complexity | Token Cost |
|----------|------------|-------------------|------------|------------|
| Full rewrite | Medium | High | Low | High (entire file) |
| String replace | High | High | Low | Low (just the changed section) |
| Line-based | Low | Medium | Medium | Low |
| AST-based | High | Low | Very high | Medium |

String replacement hits the sweet spot: it is simple to implement, works reliably with LLM output, catches errors through the uniqueness check, and uses minimal tokens because the model only outputs the changed portion plus enough context to make it unique.

::: tip Coming from Python
Python developers familiar with `str.replace()` will feel right at home. The core operation is identical: find a substring and replace it. The Rust version adds the uniqueness constraint and file I/O wrapping, but the fundamental string operation is the same. Rust's `str::replacen` with `n=1` ensures only the first occurrence is replaced, matching the semantics you'd get from Python's `str.replace(old, new, 1)`.
:::

## Design Decisions for Your Agent

When building your agent's edit tool, consider these design decisions:

1. **Default to string replacement** for most edits. It is the most battle-tested approach.
2. **Offer full rewrite as a separate tool** for cases where the model needs to create a file or completely overhaul a small one.
3. **Return clear error messages** when an edit fails. Tell the model *why* it failed (not found, multiple matches) so it can retry.
4. **Include a diff in the result** so the model and user can verify the change. We'll build this in [Diff Algorithms](/linear/06-file-system-operations/05-diff-algorithms).

## Key Takeaways

- Four main editing strategies exist: full rewrite, string replacement, line-based, and AST-based -- each with distinct trade-offs
- String replacement is the most reliable approach for LLM-driven edits because the model only needs to specify exact text, not line numbers or diff syntax
- The uniqueness constraint (old string must appear exactly once) is the key safety feature that prevents ambiguous or unintended edits
- Full rewrites work for small files but risk silent data loss on larger files where the LLM might accidentally drop content
- Return descriptive error messages when edits fail so the model can correct its approach and retry
