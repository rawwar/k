---
title: Diff Algorithms
description: Understanding diff algorithms from Myers to patience diff, and how they are used to verify edits and generate human-readable change summaries.
---

# Diff Algorithms

> **What you'll learn:**
> - How the Myers diff algorithm works and why it produces minimal edit scripts between two texts
> - The difference between Myers, patience, and histogram diff algorithms and their suitability for code
> - How to use diff algorithms to verify that an edit produced the intended change and generate summaries for the user

In the previous subchapter, we established that string replacement is best for *making* edits, while diffs are best for *displaying* edits. Now let's understand the diff algorithms that power that display. You'll use diff algorithms in your agent for three purposes: showing users what changed, verifying that edits only modified the intended section, and generating summaries of multi-file changes.

## What a Diff Algorithm Does

A diff algorithm takes two sequences (usually lines of text) and produces the minimal set of insertions and deletions that transform the first into the second. This set is called an **edit script**.

Given these two versions:

```
Old:                    New:
1: fn add(a: i32) {     1: fn add(a: i32, b: i32) {
2:     a + 1            2:     a + b
3: }                    3: }
```

A diff algorithm produces:

```
Delete line 1: "fn add(a: i32) {"
Insert line 1: "fn add(a: i32, b: i32) {"
Delete line 2: "    a + 1"
Insert line 2: "    a + b"
Keep   line 3: "}"
```

The algorithm must figure out which lines are unchanged (the "}" is the same), which were deleted, and which were added. For two-line differences this is trivial. For real files with hundreds of lines, many similar-looking sections, and moved blocks, it becomes a genuinely hard computational problem.

## The Myers Diff Algorithm

The Myers algorithm (1986) is the default diff algorithm in Git and the most widely used in programming tools. It finds the shortest edit script -- the minimum number of insertions and deletions to transform one sequence into another.

Here's how to use it with the `similar` crate, which is the best Rust diff library:

```rust
use similar::{ChangeTag, TextDiff};

fn generate_unified_diff(
    old_text: &str,
    new_text: &str,
    file_path: &str,
) -> String {
    let diff = TextDiff::from_lines(old_text, new_text);

    let mut output = String::new();
    output.push_str(&format!("--- a/{}\n", file_path));
    output.push_str(&format!("+++ b/{}\n", file_path));

    for hunk in diff.unified_diff().header("a/file", "b/file").iter_hunks() {
        output.push_str(&format!("{}", hunk));
    }

    output
}

fn main() {
    let old = "fn add(a: i32) {\n    a + 1\n}\n";
    let new = "fn add(a: i32, b: i32) {\n    a + b\n}\n";

    let diff = generate_unified_diff(old, new, "src/math.rs");
    println!("{}", diff);
}
```

The `similar` crate uses the Myers algorithm by default. It produces standard unified diff output that looks exactly like `git diff` output.

### How Myers Works (Intuition)

The Myers algorithm models the diff problem as a shortest path search on a graph. Imagine a grid where the x-axis represents lines of the old file and the y-axis represents lines of the new file:

- Moving right means **deleting** a line from the old file.
- Moving down means **inserting** a line from the new file.
- Moving diagonally (right and down) means the lines **match** -- no edit needed.

The algorithm finds the path from the top-left corner to the bottom-right corner that uses the fewest right and down moves (non-diagonal moves). Diagonal moves are free because matching lines require no editing.

Myers uses a clever breadth-first search that expands the frontier one edit at a time. It's O(ND) in complexity, where N is the total length and D is the number of differences. For files that are mostly similar (the common case for code edits), D is small and the algorithm is fast.

::: python Coming from Python
Python's `difflib` module provides `unified_diff` and `SequenceMatcher` based on a variation of the Ratcliff/Obershelp algorithm. It works, but it's slower than Myers for large files and sometimes produces less intuitive diffs. Rust's `similar` crate is inspired by Python's `difflib` API design but uses the faster Myers algorithm under the hood. If you've used `difflib.unified_diff()`, the `similar` API will feel familiar.
:::

## Patience Diff

Patience diff (invented by Bram Cohen of BitTorrent fame) takes a different approach. Instead of finding the shortest edit script, it first identifies lines that appear exactly once in both files and uses those as anchors. It then recursively diffs the sections between anchors.

```rust
use similar::{Algorithm, TextDiff};

fn patience_diff(old_text: &str, new_text: &str) -> String {
    let diff = TextDiff::configure()
        .algorithm(Algorithm::Patience)
        .diff_lines(old_text, new_text);

    let mut output = String::new();
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            similar::ChangeTag::Delete => "-",
            similar::ChangeTag::Insert => "+",
            similar::ChangeTag::Equal => " ",
        };
        output.push_str(&format!("{}{}", sign, change));
    }
    output
}
```

### When Patience Diff Shines

Patience diff produces better output when code has been *moved* or when there are many similar-looking lines. Consider this scenario:

```
Old:                           New:
fn foo() {                     fn foo() {
    println!("hello");             println!("hello");
}                                  println!("world");
                               }
fn bar() {
    println!("world");         fn bar() {
}                              }
```

Myers might match the closing `}` of `foo` with the closing `}` of `bar` and produce a confusing diff. Patience diff anchors on the unique function signatures (`fn foo()`, `fn bar()`) and produces a more intuitive result showing that a line was added to `foo` and a line was removed from `bar`.

## Histogram Diff

Histogram diff (used as Git's `diff.algorithm=histogram`) is a refinement of patience diff. It handles files where unique lines are scarce by using a histogram of line frequencies to find the best anchors:

```rust
use similar::{Algorithm, TextDiff};

fn histogram_diff(old_text: &str, new_text: &str) -> String {
    let diff = TextDiff::configure()
        .algorithm(Algorithm::Myers) // similar crate uses Myers by default;
        // for histogram, you would use a specialized crate or implement it
        .diff_lines(old_text, new_text);

    let mut output = String::new();
    for hunk in diff.unified_diff().iter_hunks() {
        output.push_str(&format!("{}", hunk));
    }
    output
}
```

In practice, for the use cases in a coding agent, Myers and patience produce nearly identical output. The differences only become noticeable on large files with significant structural changes. Most agents use Myers (the default) and get excellent results.

## Using Diffs to Verify Edits

One of the most valuable uses of diff in a coding agent is **edit verification**. After performing a string replacement, you can diff the old and new content to confirm the change was correct:

```rust
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::path::Path;

pub struct EditResult {
    pub success: bool,
    pub diff_summary: String,
    pub lines_changed: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
}

pub fn edit_and_verify(
    path: &Path,
    old_string: &str,
    new_string: &str,
) -> Result<EditResult, String> {
    let original = fs::read_to_string(path)
        .map_err(|e| format!("Cannot read file: {e}"))?;

    let count = original.matches(old_string).count();
    if count != 1 {
        return Err(format!("Expected 1 match, found {}", count));
    }

    let modified = original.replacen(old_string, new_string, 1);

    // Generate diff statistics
    let diff = TextDiff::from_lines(&original, &modified);
    let mut lines_added = 0;
    let mut lines_removed = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => lines_added += 1,
            ChangeTag::Delete => lines_removed += 1,
            ChangeTag::Equal => {}
        }
    }

    // Generate unified diff for display
    let diff_output: String = diff
        .unified_diff()
        .context_radius(3)
        .header(
            &format!("a/{}", path.display()),
            &format!("b/{}", path.display()),
        )
        .to_string();

    // Write the file
    fs::write(path, &modified)
        .map_err(|e| format!("Write failed: {e}"))?;

    Ok(EditResult {
        success: true,
        diff_summary: diff_output,
        lines_changed: lines_added.max(lines_removed),
        lines_added,
        lines_removed,
    })
}
```

The `context_radius(3)` setting shows 3 lines of unchanged context around each change, matching the standard `git diff` format. This gives enough surrounding code for a human to understand where the change occurred.

## Generating Change Summaries

When your agent makes multiple edits across several files, a summary helps the user understand the scope of changes:

```rust
use similar::{ChangeTag, TextDiff};

pub struct ChangeSummary {
    pub file_path: String,
    pub additions: usize,
    pub deletions: usize,
}

pub fn summarize_changes(
    file_path: &str,
    old_content: &str,
    new_content: &str,
) -> ChangeSummary {
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

    ChangeSummary {
        file_path: file_path.to_string(),
        additions,
        deletions,
    }
}

pub fn format_summary(summaries: &[ChangeSummary]) -> String {
    let mut output = String::new();
    let total_adds: usize = summaries.iter().map(|s| s.additions).sum();
    let total_dels: usize = summaries.iter().map(|s| s.deletions).sum();

    for s in summaries {
        output.push_str(&format!(
            " {} | +{} -{}\n",
            s.file_path, s.additions, s.deletions
        ));
    }
    output.push_str(&format!(
        "\n{} files changed, {} insertions(+), {} deletions(-)\n",
        summaries.len(),
        total_adds,
        total_dels
    ));
    output
}
```

This produces output similar to `git diff --stat`:

```
 src/tools/edit.rs | +12 -3
 src/tools/read.rs | +5 -1

2 files changed, 17 insertions(+), 4 deletions(-)
```

::: wild In the Wild
Claude Code shows a diff to the user after every edit operation. The diff uses standard unified format with syntax highlighting, making it easy to review changes at a glance. This transparency is crucial for user trust -- the user can see exactly what the agent changed and catch errors before they propagate.
:::

## Key Takeaways

- The Myers diff algorithm (default in Git and the `similar` crate) finds the minimal edit script and works well for most code diffs
- Patience diff produces more intuitive results when code has been moved or when many lines look similar, but Myers is sufficient for most agent use cases
- Use diff algorithms to verify edits (confirming only the intended section changed) and to display changes to users in a familiar unified diff format
- The `similar` crate provides a clean API for all three algorithms and produces output compatible with standard Git diff format
- Change summaries (additions, deletions per file) help users understand the scope of multi-file edits at a glance
