---
title: Diff Generation
description: Generating unified diffs, stat summaries, and word-level diffs programmatically for change review, context injection, and user-facing output.
---

# Diff Generation

> **What you'll learn:**
> - How to generate unified diffs between arbitrary refs, commits, and working tree states
> - Techniques for producing stat summaries and word-level diffs for fine-grained review
> - How to truncate and summarize large diffs for LLM context without losing critical information

Diffs are the language of change. When the agent modifies code, diffs show exactly what changed. When the user wants to review agent work, diffs provide the evidence. When the LLM needs context about recent modifications, diffs deliver it concisely. This subchapter covers generating diffs in multiple formats and adapting them for different consumers: the LLM, the terminal UI, and the commit history.

## Unified Diff Basics

The unified diff format (the output of `git diff`) is the standard for showing changes. Each change is presented as a "hunk" with context lines (prefixed with a space), added lines (prefixed with `+`), and removed lines (prefixed with `-`). Let's build a structured diff generator:

```rust
use std::path::Path;
use std::process::Command;

fn run_git_checked(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Different ways to compare changes
pub enum DiffTarget {
    /// Working tree vs staging area (unstaged changes)
    Unstaged,
    /// Staging area vs HEAD (staged changes)
    Staged,
    /// Working tree vs HEAD (all uncommitted changes)
    Head,
    /// Compare two commits
    Commits { from: String, to: String },
    /// Compare a commit to the working tree
    CommitToWorking { commit: String },
}

/// Generate a unified diff for the given target
pub fn generate_diff(
    repo_path: &Path,
    target: &DiffTarget,
    context_lines: usize,
    paths: Option<&[&str]>,
) -> Result<String, String> {
    let context_arg = format!("-U{}", context_lines);
    let mut args: Vec<&str> = vec!["diff", &context_arg];

    match target {
        DiffTarget::Unstaged => {
            // default: working tree vs index
        }
        DiffTarget::Staged => {
            args.push("--cached");
        }
        DiffTarget::Head => {
            args.push("HEAD");
        }
        DiffTarget::Commits { from, to } => {
            args.push(from);
            args.push(to);
        }
        DiffTarget::CommitToWorking { commit } => {
            args.push(commit);
        }
    }

    if let Some(file_paths) = paths {
        args.push("--");
        args.extend_from_slice(file_paths);
    }

    run_git_checked(repo_path, &args)
}

fn main() {
    let repo = Path::new(".");

    // Show unstaged changes with 3 lines of context
    match generate_diff(repo, &DiffTarget::Unstaged, 3, None) {
        Ok(diff) => {
            if diff.is_empty() {
                println!("No unstaged changes");
            } else {
                println!("Unstaged changes:\n{}", diff);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    // Show staged changes for a specific file
    match generate_diff(
        repo,
        &DiffTarget::Staged,
        3,
        Some(&["src/main.rs"]),
    ) {
        Ok(diff) => println!("Staged changes to main.rs:\n{}", diff),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Stat Summaries

A diff stat provides a high-level overview without the line-by-line details. This is ideal for giving the LLM a quick picture of what changed:

```rust
use std::path::Path;
use std::process::Command;

fn run_git_checked(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

#[derive(Debug)]
pub struct DiffStatEntry {
    pub path: String,
    pub insertions: usize,
    pub deletions: usize,
}

#[derive(Debug)]
pub struct DiffStat {
    pub files: Vec<DiffStatEntry>,
    pub total_insertions: usize,
    pub total_deletions: usize,
}

impl DiffStat {
    /// Format as a compact summary suitable for LLM context
    pub fn summarize(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "{} files changed, {} insertions(+), {} deletions(-)",
            self.files.len(),
            self.total_insertions,
            self.total_deletions
        ));

        for file in &self.files {
            let bar_len = (file.insertions + file.deletions).min(30);
            let plus_count = if bar_len > 0 {
                (file.insertions * bar_len) / (file.insertions + file.deletions).max(1)
            } else {
                0
            };
            let minus_count = bar_len - plus_count;

            let bar = format!(
                "{}{}",
                "+".repeat(plus_count),
                "-".repeat(minus_count)
            );
            lines.push(format!(
                "  {} | {} {}",
                file.path,
                file.insertions + file.deletions,
                bar
            ));
        }

        lines.join("\n")
    }
}

/// Generate a diff stat between two refs
pub fn diff_stat(
    repo_path: &Path,
    from: Option<&str>,
    to: Option<&str>,
    cached: bool,
) -> Result<DiffStat, String> {
    let mut args = vec!["diff", "--numstat"];

    if cached {
        args.push("--cached");
    }

    if let Some(f) = from {
        args.push(f);
    }
    if let Some(t) = to {
        args.push(t);
    }

    let output = run_git_checked(repo_path, &args)?;

    let mut files = Vec::new();
    let mut total_insertions = 0;
    let mut total_deletions = 0;

    for line in output.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() == 3 {
            // Binary files show as "-" for numstat
            let ins = parts[0].parse::<usize>().unwrap_or(0);
            let del = parts[1].parse::<usize>().unwrap_or(0);
            total_insertions += ins;
            total_deletions += del;
            files.push(DiffStatEntry {
                path: parts[2].to_string(),
                insertions: ins,
                deletions: del,
            });
        }
    }

    Ok(DiffStat {
        files,
        total_insertions,
        total_deletions,
    })
}

fn main() {
    let repo = Path::new(".");

    // Stat for all uncommitted changes
    match diff_stat(repo, Some("HEAD"), None, false) {
        Ok(stat) => println!("{}", stat.summarize()),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

::: python Coming from Python
In Python, you might use `subprocess.run(["git", "diff", "--numstat"], ...)` and split each line by tabs. The parsing logic is identical in both languages. What Rust adds is a structured `DiffStat` type that guarantees the data is well-formed. If you need to display the stat in multiple formats (terminal, JSON, LLM prompt), you implement different methods on the same struct rather than re-parsing the git output each time.
:::

## Word-Level Diffs

Sometimes line-level diffs are too coarse. If the agent changed a single variable name on a long line, the unified diff shows the entire line as removed and re-added. Word-level diffs highlight exactly what changed within the line:

```rust
use std::path::Path;
use std::process::Command;

fn run_git_checked(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Generate a word-level diff that highlights individual word changes
pub fn word_diff(
    repo_path: &Path,
    cached: bool,
    paths: Option<&[&str]>,
) -> Result<String, String> {
    let mut args = vec!["diff", "--word-diff=plain"];

    if cached {
        args.push("--cached");
    }

    if let Some(file_paths) = paths {
        args.push("--");
        for p in file_paths {
            args.push(p);
        }
    }

    run_git_checked(repo_path, &args)
    // Output marks changes inline:
    // let [-old_name-]{+new_name+} = value;
}

/// Generate a color-words diff (words only, no +/- line markers)
pub fn color_words_diff(
    repo_path: &Path,
    from: &str,
    to: &str,
    path: &str,
) -> Result<String, String> {
    run_git_checked(
        repo_path,
        &["diff", "--color-words", from, to, "--", path],
    )
}

fn main() {
    let repo = Path::new(".");

    match word_diff(repo, false, None) {
        Ok(diff) => {
            if diff.is_empty() {
                println!("No word-level changes");
            } else {
                println!("Word-level diff:\n{}", diff);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Truncating Large Diffs for LLM Context

The biggest practical challenge with diffs is size. A refactor touching 50 files might produce a 10,000-line diff. You cannot send all of that to the LLM -- it would consume most of the context window and the model would struggle to make sense of it. Instead, you need intelligent truncation:

```rust
/// Configuration for diff truncation
pub struct DiffTruncationConfig {
    /// Maximum total lines in the output
    pub max_total_lines: usize,
    /// Maximum lines per file
    pub max_lines_per_file: usize,
    /// Prioritize files with fewer changes (they are easier to review)
    pub prioritize_small_changes: bool,
}

impl Default for DiffTruncationConfig {
    fn default() -> Self {
        Self {
            max_total_lines: 500,
            max_lines_per_file: 100,
            prioritize_small_changes: true,
        }
    }
}

/// A single file's diff content
struct FileDiff {
    header: String,
    content: Vec<String>,
    insertions: usize,
    deletions: usize,
}

/// Parse a unified diff into per-file sections
fn parse_into_file_diffs(raw_diff: &str) -> Vec<FileDiff> {
    let mut file_diffs = Vec::new();
    let mut current_header = String::new();
    let mut current_lines: Vec<String> = Vec::new();
    let mut ins = 0;
    let mut del = 0;

    for line in raw_diff.lines() {
        if line.starts_with("diff --git") {
            // Save previous file diff
            if !current_header.is_empty() {
                file_diffs.push(FileDiff {
                    header: current_header.clone(),
                    content: current_lines.clone(),
                    insertions: ins,
                    deletions: del,
                });
            }
            current_header = line.to_string();
            current_lines = Vec::new();
            ins = 0;
            del = 0;
        } else {
            if line.starts_with('+') && !line.starts_with("+++") {
                ins += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                del += 1;
            }
            current_lines.push(line.to_string());
        }
    }

    // Don't forget the last file
    if !current_header.is_empty() {
        file_diffs.push(FileDiff {
            header: current_header,
            content: current_lines,
            insertions: ins,
            deletions: del,
        });
    }

    file_diffs
}

/// Truncate a diff to fit within the configured limits
pub fn truncate_diff(raw_diff: &str, config: &DiffTruncationConfig) -> String {
    let mut file_diffs = parse_into_file_diffs(raw_diff);

    if config.prioritize_small_changes {
        // Sort so smaller diffs come first -- they are easier to review fully
        file_diffs.sort_by_key(|fd| fd.insertions + fd.deletions);
    }

    let mut output = Vec::new();
    let mut total_lines = 0;
    let mut truncated_files = 0;
    let mut skipped_files = 0;

    for fd in &file_diffs {
        if total_lines >= config.max_total_lines {
            skipped_files += 1;
            continue;
        }

        output.push(fd.header.clone());
        total_lines += 1;

        let available = (config.max_total_lines - total_lines).min(config.max_lines_per_file);

        if fd.content.len() <= available {
            for line in &fd.content {
                output.push(line.clone());
            }
            total_lines += fd.content.len();
        } else {
            for line in fd.content.iter().take(available) {
                output.push(line.clone());
            }
            total_lines += available;
            output.push(format!(
                "... ({} more lines in this file)",
                fd.content.len() - available
            ));
            truncated_files += 1;
            total_lines += 1;
        }
    }

    if truncated_files > 0 || skipped_files > 0 {
        output.push(String::new());
        output.push(format!(
            "--- Diff summary: {} files shown ({} truncated, {} skipped entirely) ---",
            file_diffs.len() - skipped_files,
            truncated_files,
            skipped_files
        ));
    }

    output.join("\n")
}

fn main() {
    // Example with a large diff
    let large_diff = "diff --git a/src/main.rs b/src/main.rs\n\
        --- a/src/main.rs\n\
        +++ b/src/main.rs\n\
        @@ -1,3 +1,5 @@\n\
        +use std::io;\n\
        +\n\
         fn main() {\n\
        -    println!(\"hello\");\n\
        +    println!(\"hello world\");\n\
         }";

    let config = DiffTruncationConfig::default();
    let truncated = truncate_diff(large_diff, &config);
    println!("{}", truncated);
}
```

::: wild In the Wild
Claude Code uses a tiered approach to diff presentation. For small changes (under a few hundred lines), it shows the full diff. For medium changes, it shows full diffs for the most important files and stat summaries for the rest. For very large changes, it shows only stat summaries with a count of total insertions and deletions. This adaptive strategy balances completeness with context window efficiency.
:::

## Comparing Arbitrary Refs

Your agent needs to compare not just working tree changes but arbitrary points in history. This is useful for answering questions like "what changed since the last release?" or "how does this branch differ from main?":

```rust
use std::path::Path;
use std::process::Command;

fn run_git_checked(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Compare what a branch adds relative to another (three-dot diff)
/// This shows changes on source_branch since it diverged from target_branch
pub fn branch_diff(
    repo_path: &Path,
    target_branch: &str,
    source_branch: &str,
) -> Result<String, String> {
    let range = format!("{}...{}", target_branch, source_branch);
    run_git_checked(repo_path, &["diff", "--stat", &range])
}

/// Show what changed in the last N commits
pub fn recent_changes(repo_path: &Path, commit_count: usize) -> Result<String, String> {
    let range = format!("HEAD~{}..HEAD", commit_count);
    run_git_checked(repo_path, &["diff", "--stat", &range])
}

fn main() {
    let repo = Path::new(".");

    // What did the agent's branch change compared to main?
    match branch_diff(repo, "main", "agent/fix-login-bug-12345") {
        Ok(diff) => println!("Branch changes:\n{}", diff),
        Err(e) => eprintln!("Error: {}", e),
    }

    // What changed in the last 3 commits?
    match recent_changes(repo, 3) {
        Ok(diff) => println!("Recent changes:\n{}", diff),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Key Takeaways

- Use the `DiffTarget` enum to generate diffs between any combination of working tree, staging area, HEAD, and arbitrary commits.
- Generate stat summaries with `--numstat` for high-level overviews and full unified diffs with `-U<n>` for detailed review.
- Word-level diffs (`--word-diff=plain`) are valuable when the agent makes small, targeted changes within long lines.
- Always truncate large diffs before injecting them into LLM context -- use a tiered strategy that prioritizes smaller, fully-shown diffs over large truncated ones.
- Three-dot diffs (`main...feature`) show what a branch adds since it diverged, which is exactly what you need for reviewing agent work.
