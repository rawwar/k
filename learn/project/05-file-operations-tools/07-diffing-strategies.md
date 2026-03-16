---
title: Diffing Strategies
description: Generate and display diffs of file changes so the user can review what the agent modified before and after edits.
---

# Diffing Strategies

> **What you'll learn:**
> - How to generate unified diffs between the original and modified file content using the `similar` crate
> - How to display diffs with color-coded additions and deletions in the terminal for user review
> - How to store diffs as part of the tool result so the model can verify its own edits in the next turn

When the agent edits a file, the user should see exactly what changed. And the model itself benefits from seeing the diff -- it can verify its edit did what it intended and catch mistakes before moving on. In this subchapter you will build a diffing system that generates unified diffs (the same format `git diff` uses) and renders them with color in the terminal.

## Why Diffs Matter for Agent Workflows

Diffs serve two audiences in a coding agent:

**The user** needs to review what the agent changed. Without diffs, the user would have to read the entire file before and after to spot the modification. With a diff, the change is immediately visible.

**The model** uses diffs as feedback. When the edit tool returns a diff, the model can verify that the replacement happened where it expected. If the model intended to change a function signature but the diff shows a change in a comment instead, it knows to try again. This self-verification loop is one of the things that makes agentic coding work.

## Adding the `similar` Crate

Rust's standard library does not include a diffing algorithm, so we will use the `similar` crate. Add it to your `Cargo.toml`:

```toml
[dependencies]
similar = "2"
```

The `similar` crate implements the Myers diff algorithm (the same one Git uses) and can produce unified diffs, inline diffs, and change-by-change iterators.

## Generating a Unified Diff

Here is the core function that takes the old and new content and produces a unified diff string:

```rust
use similar::{ChangeTag, TextDiff};

/// Generate a unified diff between old and new content.
/// The `path` parameter is used in the diff header.
pub fn generate_diff(path: &str, old_content: &str, new_content: &str) -> String {
    let diff = TextDiff::from_lines(old_content, new_content);

    let mut output = String::new();

    // Generate the unified diff with 3 lines of context
    let unified = diff.unified_diff();
    let formatted = unified
        .context_radius(3)
        .header(&format!("a/{}", path), &format!("b/{}", path))
        .to_string();

    if formatted.is_empty() {
        output.push_str("(no changes)");
    } else {
        output.push_str(&formatted);
    }

    output
}
```

The `context_radius(3)` gives three lines of surrounding context around each change, which is the standard for unified diffs. The header uses the `a/` and `b/` prefix convention that `git diff` uses.

Let's see what the output looks like:

```rust
fn main() {
    let old = "fn greet(name: &str) {\n    println!(\"Hello, {}\", name);\n}\n";
    let new = "fn greet(name: &str) {\n    println!(\"Hello, {}! Welcome.\", name);\n}\n";

    let diff = generate_diff("src/main.rs", old, new);
    println!("{}", diff);
}
```

This produces:

```diff
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,3 @@
 fn greet(name: &str) {
-    println!("Hello, {}", name);
+    println!("Hello, {}! Welcome.", name);
 }
```

Lines starting with `-` were removed, lines with `+` were added, and unmarked lines are context.

## Adding Color Output

Terminal color makes diffs much easier to scan. We will use ANSI escape codes directly -- no need for a color library for this:

```rust
/// ANSI color codes for terminal output
mod colors {
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const CYAN: &str = "\x1b[36m";
    pub const DIM: &str = "\x1b[2m";
    pub const RESET: &str = "\x1b[0m";
}

/// Generate a colored diff suitable for terminal display.
pub fn generate_colored_diff(
    path: &str,
    old_content: &str,
    new_content: &str,
) -> String {
    let diff = TextDiff::from_lines(old_content, new_content);
    let mut output = String::new();

    // Header
    output.push_str(&format!(
        "{}--- a/{}{}\n",
        colors::DIM,
        path,
        colors::RESET
    ));
    output.push_str(&format!(
        "{}+++ b/{}{}\n",
        colors::DIM,
        path,
        colors::RESET
    ));

    for hunk in diff.unified_diff().context_radius(3).iter_hunks() {
        // Hunk header (@@ -1,3 +1,3 @@)
        output.push_str(&format!(
            "{}{}{}\n",
            colors::CYAN,
            hunk.header(),
            colors::RESET
        ));

        for change in hunk.iter_changes() {
            match change.tag() {
                ChangeTag::Delete => {
                    output.push_str(&format!(
                        "{}-{}{}",
                        colors::RED,
                        change.value(),
                        colors::RESET
                    ));
                }
                ChangeTag::Insert => {
                    output.push_str(&format!(
                        "{}+{}{}",
                        colors::GREEN,
                        change.value(),
                        colors::RESET
                    ));
                }
                ChangeTag::Equal => {
                    output.push_str(&format!(" {}", change.value()));
                }
            }
        }
    }

    output
}
```

This produces output where deleted lines are red, added lines are green, hunk headers are cyan, and the file header is dimmed. The result is immediately readable in any terminal that supports ANSI colors.

::: python Coming from Python
In Python, you would use the `difflib` module from the standard library:
```python
import difflib

def generate_diff(path: str, old: str, new: str) -> str:
    old_lines = old.splitlines(keepends=True)
    new_lines = new.splitlines(keepends=True)
    diff = difflib.unified_diff(
        old_lines, new_lines,
        fromfile=f"a/{path}", tofile=f"b/{path}",
        lineterm=""
    )
    return "\n".join(diff)
```
Python has diffs built in, while Rust requires an external crate. The `similar` crate provides better output quality than `difflib` though -- it uses the same algorithm as Git and produces cleaner hunks with minimal context.
:::

## Integrating Diffs into the Edit Tool

Now let's modify the edit tool to include a diff in its response. The model gets the diff as part of the tool result, which it can use to verify the edit:

```rust
use crate::tools::diff::generate_diff;

// In EditFileTool::execute, after performing the replacement:
fn execute(&self, input: &Value) -> Result<String, String> {
    // ... parameter extraction and validation ...

    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))?;

    // Verify unique match and perform replacement
    let count = Self::count_occurrences(&content, old_string);
    if count != 1 {
        // ... error handling ...
    }

    let new_content = content.replacen(old_string, new_string, 1);

    // Generate the diff BEFORE writing, using the original and new content
    let diff_output = generate_diff(path_str, &content, &new_content);

    // Write the modified content
    fs::write(&path, &new_content)
        .map_err(|e| format!("Failed to write '{}': {}", path.display(), e))?;

    // Return the diff as part of the result
    Ok(format!(
        "Edited '{}'\n\n{}",
        path_str, diff_output
    ))
}
```

The diff is generated before writing to the file. This is important -- if the write fails, you do not want to generate a diff that implies the edit succeeded.

## Diff Statistics

Adding statistics to the diff output gives both the user and the model a quick summary:

```rust
/// Count the number of additions and deletions in a diff.
pub fn diff_stats(old_content: &str, new_content: &str) -> (usize, usize) {
    let diff = TextDiff::from_lines(old_content, new_content);
    let mut additions = 0;
    let mut deletions = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => additions += 1,
            ChangeTag::Delete => deletions += 1,
            ChangeTag::Equal => {}
        }
    }

    (additions, deletions)
}

/// Format diff statistics as a summary line.
pub fn format_stats(additions: usize, deletions: usize) -> String {
    format!(
        "{} insertion(s)(+), {} deletion(s)(-)",
        additions, deletions
    )
}
```

Add this to the edit tool's result:

```rust
let (additions, deletions) = diff_stats(&content, &new_content);
Ok(format!(
    "Edited '{}': {}\n\n{}",
    path_str,
    format_stats(additions, deletions),
    diff_output
))
```

Now the model sees something like: `Edited 'src/main.rs': 2 insertion(s)(+), 1 deletion(s)(-)`

## Showing Diffs for User Review

For the user-facing output (shown in the terminal), use the colored diff. You can print this when the tool result comes back from execution:

```rust
/// Print a colored diff to the terminal for user review.
pub fn display_diff_to_user(path: &str, old_content: &str, new_content: &str) {
    if old_content == new_content {
        println!("  No changes to {}", path);
        return;
    }

    let colored = generate_colored_diff(path, old_content, new_content);
    let (additions, deletions) = diff_stats(old_content, new_content);

    println!("{}", colored);
    println!(
        "  {} {}",
        path,
        format_stats(additions, deletions)
    );
}
```

This gives the user a clear view of every change the agent makes. It is the foundation of trust between the user and the agent -- the user can see exactly what happened and intervene if something looks wrong.

## Testing Diffs

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_diff_with_changes() {
        let old = "line 1\nline 2\nline 3\n";
        let new = "line 1\nline 2 modified\nline 3\n";

        let diff = generate_diff("test.txt", old, new);
        assert!(diff.contains("-line 2"));
        assert!(diff.contains("+line 2 modified"));
        assert!(diff.contains("--- a/test.txt"));
        assert!(diff.contains("+++ b/test.txt"));
    }

    #[test]
    fn test_generate_diff_no_changes() {
        let content = "same content\n";
        let diff = generate_diff("test.txt", content, content);
        assert!(diff.contains("(no changes)"));
    }

    #[test]
    fn test_diff_stats() {
        let old = "line 1\nline 2\nline 3\n";
        let new = "line 1\nnew line\nline 3\nanother new line\n";

        let (additions, deletions) = diff_stats(old, new);
        assert_eq!(deletions, 1); // "line 2" was removed
        assert_eq!(additions, 2); // "new line" and "another new line" were added
    }

    #[test]
    fn test_multiline_diff() {
        let old = "fn main() {\n    let x = 1;\n    let y = 2;\n    println!(\"{}\", x + y);\n}\n";
        let new = "fn main() {\n    let x = 10;\n    let y = 20;\n    let sum = x + y;\n    println!(\"{}\", sum);\n}\n";

        let diff = generate_diff("main.rs", old, new);
        assert!(diff.contains("-    let x = 1;"));
        assert!(diff.contains("+    let x = 10;"));
    }

    #[test]
    fn test_colored_diff_contains_ansi() {
        let old = "old line\n";
        let new = "new line\n";

        let colored = generate_colored_diff("test.txt", old, new);
        assert!(colored.contains("\x1b[31m")); // Red for deletion
        assert!(colored.contains("\x1b[32m")); // Green for insertion
    }
}
```

These tests verify the diff output format, edge cases (no changes), statistics counting, multi-line changes, and the presence of ANSI color codes.

::: wild In the Wild
Claude Code displays diffs in the terminal after every file edit, using a similar colored format. This allows the user to review changes in real time and interrupt the agent if something looks wrong. OpenCode also shows diffs but uses a side-by-side format in its TUI. The diff is also included in the tool result that goes back to the model, so the model can self-verify its edits. This self-verification loop -- edit, see diff, confirm correct, move on -- is a key pattern in reliable agentic coding.
:::

## Key Takeaways

- The `similar` crate generates unified diffs using the Myers algorithm, the same algorithm Git uses, with configurable context radius.
- Diffs serve two audiences: the user (who sees colored terminal output for review) and the model (who receives the diff text as part of the tool result for self-verification).
- Generate the diff before writing the file -- if the write fails, you do not want the model to think the edit succeeded.
- Diff statistics (additions/deletions count) provide a quick summary that both users and models can scan without reading the full diff.
- Color-coded output uses ANSI escape codes directly: red for deletions, green for additions, cyan for hunk headers -- no external color library needed.
