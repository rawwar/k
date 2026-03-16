---
title: String Replace vs Patch
description: A detailed comparison of string replacement and unified diff patch approaches to file editing, with guidance on when to use each.
---

# String Replace vs Patch

> **What you'll learn:**
> - How string replacement works by finding an exact match and substituting it, and why uniqueness matters
> - How unified diff patches specify changes with line context and why LLMs struggle to generate them accurately
> - The practical reasons most production agents favor string replacement over patch-based editing

The previous subchapter surveyed four editing strategies. Now let's zoom into the two most commonly debated approaches: string replacement and unified diff patches. Both are text-based (no AST required), both work across any programming language, and both have been used in production agents. But they have fundamentally different characteristics when paired with LLM-generated output.

## String Replacement in Depth

String replacement is conceptually simple: find an exact substring in the file and replace it. Let's look at a more complete implementation that handles edge cases:

```rust
use std::fs;
use std::path::Path;

#[derive(Debug)]
pub enum EditError {
    FileRead(std::io::Error),
    NotFound { old_string: String, path: String },
    Ambiguous { count: usize, path: String },
    WriteError(String),
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditError::FileRead(e) => write!(f, "Cannot read file: {e}"),
            EditError::NotFound { old_string, path } => {
                let preview = if old_string.len() > 80 {
                    format!("{}...", &old_string[..80])
                } else {
                    old_string.clone()
                };
                write!(f, "String not found in {path}: \"{preview}\"")
            }
            EditError::Ambiguous { count, path } => {
                write!(
                    f,
                    "Found {count} matches in {path}. \
                     Include more surrounding context to make the match unique."
                )
            }
            EditError::WriteError(msg) => write!(f, "Write failed: {msg}"),
        }
    }
}

pub fn string_replace_edit(
    path: &Path,
    old_string: &str,
    new_string: &str,
) -> Result<String, EditError> {
    let content = fs::read_to_string(path).map_err(EditError::FileRead)?;

    let match_count = content.matches(old_string).count();

    match match_count {
        0 => Err(EditError::NotFound {
            old_string: old_string.to_string(),
            path: path.display().to_string(),
        }),
        1 => {
            let new_content = content.replacen(old_string, new_string, 1);
            fs::write(path, &new_content)
                .map_err(|e| EditError::WriteError(e.to_string()))?;
            Ok(format!("Successfully edited {}", path.display()))
        }
        n => Err(EditError::Ambiguous {
            count: n,
            path: path.display().to_string(),
        }),
    }
}
```

The error types tell the model exactly what went wrong. If the string is not found, the LLM knows to re-read the file (it might have stale content). If the match is ambiguous, the LLM knows to include more context lines in its next attempt.

### Why Uniqueness Matters

Consider this Python file:

```python
def process(items):
    for item in items:
        if item.is_valid():
            result = item.process()
            return result
    return None
```

If the LLM wants to add error handling to the `item.process()` call, it might specify:

```
old_string: "result = item.process()"
new_string: "try:\n            result = item.process()\n        except ProcessError:\n            continue"
```

This works perfectly because `result = item.process()` appears exactly once. But what if the file had two similar functions? The uniqueness check catches this and asks for more context:

```
old_string: "    if item.is_valid():\n            result = item.process()\n            return result"
```

By including the surrounding lines, the match becomes unique even when the inner line appears elsewhere.

## Unified Diff Patches

A unified diff represents changes with context:

```diff
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,7 +10,9 @@
     for item in items {
         if item.is_valid() {
-            result = item.process()
-            return result
+            try:
+                result = item.process()
+            except ProcessError:
+                continue
     return None
```

Applying a patch in Rust requires a patch parser and applicator. Here's a simplified version:

```rust
use std::fs;
use std::path::Path;

#[derive(Debug)]
struct Hunk {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
    lines: Vec<DiffLine>,
}

#[derive(Debug)]
enum DiffLine {
    Context(String),
    Remove(String),
    Add(String),
}

fn apply_patch(
    path: &Path,
    hunks: &[Hunk],
) -> Result<String, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Cannot read file: {e}"))?;
    let mut lines: Vec<String> = content.lines().map(String::from).collect();

    // Apply hunks in reverse order to maintain line numbers
    for hunk in hunks.iter().rev() {
        let start = hunk.old_start.saturating_sub(1);
        let mut new_lines: Vec<String> = Vec::new();
        let mut old_idx = start;

        for diff_line in &hunk.lines {
            match diff_line {
                DiffLine::Context(text) => {
                    // Verify context matches
                    if old_idx < lines.len() && lines[old_idx] != *text {
                        return Err(format!(
                            "Context mismatch at line {}: expected '{}', found '{}'",
                            old_idx + 1,
                            text,
                            lines[old_idx]
                        ));
                    }
                    new_lines.push(text.clone());
                    old_idx += 1;
                }
                DiffLine::Remove(_) => {
                    old_idx += 1; // Skip the removed line
                }
                DiffLine::Add(text) => {
                    new_lines.push(text.clone());
                }
            }
        }

        // Replace the hunk range with new content
        let end = old_idx.min(lines.len());
        lines.splice(start..end, new_lines);
    }

    let result = lines.join("\n");
    fs::write(path, &result)
        .map_err(|e| format!("Write failed: {e}"))?;
    Ok(format!("Applied {} hunks to {}", hunks.len(), path.display()))
}
```

### Why LLMs Struggle with Patches

Unified diff format looks precise, but generating it correctly requires the LLM to:

1. **Count line numbers accurately** -- the `@@ -10,7 +10,9 @@` header specifies exact line ranges. LLMs frequently miscalculate these.
2. **Include correct context lines** -- the lines without `+` or `-` prefixes must exactly match the file. Whitespace differences cause patch application failures.
3. **Track cumulative offsets** -- if a patch has multiple hunks, later hunks must account for line number shifts from earlier hunks.
4. **Use exact diff syntax** -- missing or extra spaces after the `+`, `-`, or ` ` prefix cause parsing failures.

In practice, LLMs generate invalid patches at an unacceptably high rate. The syntax is fiddly, the line counting is error-prone, and a single wrong context line causes the entire hunk to fail.

::: tip Coming from Python
If you've used `subprocess.run(["git", "apply", "-"], input=patch_text)` to apply patches in Python, you know how fragile it can be. Even human-written patches break when whitespace changes or line numbers shift. Now imagine an LLM generating that patch text -- the failure rate is much higher than with a simple string find-and-replace.
:::

## Head-to-Head Comparison

Let's compare the two approaches across the dimensions that matter for a coding agent:

### Precision of Intent

**String replace**: The model explicitly states what it expects to find and what it wants to put there. If the expectation doesn't match reality, the edit fails safely.

**Patch**: The model encodes its intent in a format that also encodes structural information (line numbers, context). Structural errors can cause the intent to be applied to the wrong location.

### Error Recovery

**String replace**: Failure modes are simple. Either the string wasn't found (stale file), or it matched multiple times (ambiguous). Both can be resolved by re-reading the file.

**Patch**: Failure modes are complex. Context mismatch, incorrect line numbers, malformed syntax, wrong offset calculations. Diagnosing the problem is harder for both the LLM and the human.

### Multi-Hunk Edits

**String replace**: To make multiple changes, the model calls the edit tool multiple times. Each call is independent and self-contained.

**Patch**: Multiple changes can be expressed in a single patch with multiple hunks. This is more efficient in terms of tool calls but more fragile because a failure in any hunk may leave the file in a partially edited state.

### Token Efficiency

**String replace**: The model outputs the old string (for matching) and the new string. For small edits, the old string context is typically 3-10 lines.

**Patch**: Similar token count for the actual changes, but the diff headers and context lines add overhead. The overall token cost is comparable.

## When Patches Still Make Sense

Despite their drawbacks for LLM generation, patches are useful in specific scenarios:

1. **Showing changes to users**: After an edit, generating a unified diff of the before and after is an excellent way to display what changed.
2. **Applying human-written patches**: If your agent needs to apply a patch from a git commit or pull request, patch application is the right tool.
3. **Verifying edits**: Generating a diff between old and new content to verify the edit only changed what was intended.

In your agent, you'll likely use string replacement for *making* edits and diff algorithms for *displaying* edits. This gives you the reliability of string matching with the readability of diffs.

::: wild In the Wild
Claude Code uses string replacement exclusively for its Edit tool. Codex CLI experimented with diff-based editing and found similar reliability challenges. The pattern that has emerged across production agents is clear: string replacement for applying edits, unified diff for displaying what changed. Aider is a notable exception that uses a diff-like "search/replace" block format, but even its format is closer to string replacement than true unified diff.
:::

## A Hybrid Approach

You can combine the best of both worlds by using string replacement to make the edit and then generating a diff to show the user:

```rust
use std::fs;
use std::path::Path;

pub fn edit_with_diff_output(
    path: &Path,
    old_string: &str,
    new_string: &str,
) -> Result<String, String> {
    let original = fs::read_to_string(path)
        .map_err(|e| format!("Cannot read file: {e}"))?;

    // Perform the string replacement
    let count = original.matches(old_string).count();
    if count != 1 {
        return Err(format!(
            "Expected 1 match, found {}. Include more context.",
            count
        ));
    }

    let modified = original.replacen(old_string, new_string, 1);

    // Generate a simple diff for display
    let diff = generate_simple_diff(&original, &modified, path);

    // Write the modified content
    fs::write(path, &modified)
        .map_err(|e| format!("Write failed: {e}"))?;

    Ok(diff)
}

fn generate_simple_diff(old: &str, new: &str, path: &Path) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let mut diff = format!("--- a/{}\n+++ b/{}\n", path.display(), path.display());

    for (i, (o, n)) in old_lines.iter().zip(new_lines.iter()).enumerate() {
        if o != n {
            diff.push_str(&format!("@@ line {} @@\n", i + 1));
            diff.push_str(&format!("-{}\n", o));
            diff.push_str(&format!("+{}\n", n));
        }
    }

    diff
}
```

This gives you the best of both worlds: the reliability of string replacement for the actual edit, and the clarity of a diff for the output.

## Key Takeaways

- String replacement is the preferred editing approach for LLM-driven agents because LLMs reliably generate exact text matches but struggle with diff syntax and line numbers
- The uniqueness constraint (match must appear exactly once) is the critical safety mechanism that prevents wrong-location edits
- Unified diff patches are fragile for LLM generation due to line counting errors, context mismatches, and cumulative offset calculations
- Use string replacement for applying edits and diff algorithms for displaying what changed -- this hybrid approach combines reliability with readability
- When a string replacement fails, the error message should guide the model toward a successful retry (re-read the file, include more context)
