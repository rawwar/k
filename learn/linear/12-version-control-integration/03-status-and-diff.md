---
title: Status and Diff
description: Implementing Git status and diff operations that show the agent and user what has changed, with parsing of unified diff format and status codes.
---

# Status and Diff

> **What you'll learn:**
> - How to retrieve repository status (staged, unstaged, untracked, conflicted) using both git CLI output and git2 status APIs
> - Parsing unified diff format to extract file-level and hunk-level changes with line numbers and content
> - Building a structured diff representation that the agent can use to understand what changed and generate meaningful commit messages

Status and diff are the two operations your agent will call most frequently. Before every tool execution, the agent should know the state of the working tree. After every file modification, it needs to see what changed. These operations form the feedback loop that keeps the agent aware of its own impact on the codebase.

## Git Status: The Three-Way View

As you learned in the object model subchapter, Git maintains three "trees" that matter for status:

1. **HEAD** -- the last committed snapshot
2. **Index** (staging area) -- what will go into the next commit
3. **Working tree** -- the actual files on disk

`git status` compares these three pairwise: HEAD vs. index shows staged changes, and index vs. working tree shows unstaged changes. Files that exist in the working tree but not in the index are untracked.

### Parsing Porcelain Status

The `--porcelain` flag produces machine-readable output that is stable across Git versions -- exactly what you need for parsing:

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Ignored,
    Conflicted,
}

#[derive(Debug, Clone)]
pub struct StatusEntry {
    pub index_status: Option<FileStatus>,
    pub worktree_status: Option<FileStatus>,
    pub path: PathBuf,
    pub original_path: Option<PathBuf>, // for renames
}

pub fn parse_status(repo_dir: &Path) -> Result<Vec<StatusEntry>, String> {
    let output = Command::new("git")
        .args(["status", "--porcelain=v1", "-uall"])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to run git status: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();

    for line in stdout.lines() {
        if line.len() < 4 {
            continue;
        }

        let index_char = line.as_bytes()[0];
        let worktree_char = line.as_bytes()[1];
        let path_str = &line[3..];

        // Handle renames: "R  old -> new"
        let (path, original) = if path_str.contains(" -> ") {
            let parts: Vec<&str> = path_str.splitn(2, " -> ").collect();
            (PathBuf::from(parts[1]), Some(PathBuf::from(parts[0])))
        } else {
            (PathBuf::from(path_str), None)
        };

        entries.push(StatusEntry {
            index_status: parse_status_char(index_char),
            worktree_status: parse_status_char(worktree_char),
            path,
            original_path: original,
        });
    }

    Ok(entries)
}

fn parse_status_char(c: u8) -> Option<FileStatus> {
    match c {
        b'A' => Some(FileStatus::Added),
        b'M' => Some(FileStatus::Modified),
        b'D' => Some(FileStatus::Deleted),
        b'R' => Some(FileStatus::Renamed),
        b'C' => Some(FileStatus::Copied),
        b'?' => Some(FileStatus::Untracked),
        b'!' => Some(FileStatus::Ignored),
        b'U' => Some(FileStatus::Conflicted),
        b' ' => None,
        _ => None,
    }
}
```

The two-character status code is the key. The first character describes the index (staging area) status, and the second describes the working tree status. A status of `M ` means "modified in the index, clean in the working tree" -- the change is fully staged. ` M` means "clean in the index, modified in the working tree" -- the change is unstaged. `MM` means the file has both staged and unstaged changes.

### Status via git2

For lower overhead in hot paths, use `git2` to check status without spawning a subprocess:

```rust
use git2::{Repository, StatusOptions, Status};
use std::path::Path;

fn get_status_git2(repo_path: &Path) -> Result<Vec<(String, String)>, git2::Error> {
    let repo = Repository::discover(repo_path)?;

    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true);

    let statuses = repo.statuses(Some(&mut opts))?;
    let mut result = Vec::new();

    for entry in statuses.iter() {
        let path = entry.path().unwrap_or("").to_string();
        let status = entry.status();

        let description = format_git2_status(status);
        result.push((path, description));
    }

    Ok(result)
}

fn format_git2_status(status: Status) -> String {
    let mut parts = Vec::new();

    if status.contains(Status::INDEX_NEW) { parts.push("staged:new"); }
    if status.contains(Status::INDEX_MODIFIED) { parts.push("staged:modified"); }
    if status.contains(Status::INDEX_DELETED) { parts.push("staged:deleted"); }
    if status.contains(Status::WT_NEW) { parts.push("untracked"); }
    if status.contains(Status::WT_MODIFIED) { parts.push("modified"); }
    if status.contains(Status::WT_DELETED) { parts.push("deleted"); }
    if status.contains(Status::CONFLICTED) { parts.push("conflicted"); }

    parts.join(", ")
}
```

::: python Coming from Python
In Python, you might use `subprocess.run(["git", "status", "--porcelain"])` and split the output by lines, or use `GitPython` with `repo.index.diff(None)`. The Rust approach with `git2` is closer to `pygit2`'s API, but with the added benefit of Rust's type system: `Status` is a bitflag type, so you check for specific states with `.contains()` rather than string comparison. This prevents bugs like misspelling a status string.
:::

## Parsing Unified Diff Format

When your agent modifies files, it needs to understand the exact changes. The unified diff format is Git's default, and you need to parse it into structured data.

A unified diff looks like this:

```text
diff --git a/src/main.rs b/src/main.rs
index a1b2c3d..e4f5a6b 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,6 +10,8 @@ fn main() {
     let config = Config::load();
     let client = Client::new(&config);

+    // Initialize git integration
+    let repo = GitRepo::open(".").expect("Not in a git repo");
+
     loop {
         let input = read_input();
```

Let's parse this into a structured representation:

```rust
#[derive(Debug, Clone)]
pub struct DiffFile {
    pub old_path: String,
    pub new_path: String,
    pub hunks: Vec<DiffHunk>,
    pub is_new: bool,
    pub is_deleted: bool,
    pub is_renamed: bool,
}

#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_count: u32,
    pub new_start: u32,
    pub new_count: u32,
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone)]
pub enum DiffLineType {
    Context,
    Addition,
    Deletion,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub line_type: DiffLineType,
    pub content: String,
}

pub fn parse_diff(diff_text: &str) -> Vec<DiffFile> {
    let mut files = Vec::new();
    let mut current_file: Option<DiffFile> = None;
    let mut current_hunk: Option<DiffHunk> = None;

    for line in diff_text.lines() {
        if line.starts_with("diff --git ") {
            // Save previous hunk and file
            if let (Some(ref mut file), Some(hunk)) = (&mut current_file, current_hunk.take()) {
                file.hunks.push(hunk);
            }
            if let Some(file) = current_file.take() {
                files.push(file);
            }

            // Parse "diff --git a/path b/path"
            let parts: Vec<&str> = line.splitn(4, ' ').collect();
            let old_path = parts.get(2).unwrap_or(&"").trim_start_matches("a/").to_string();
            let new_path = parts.get(3).unwrap_or(&"").trim_start_matches("b/").to_string();

            current_file = Some(DiffFile {
                old_path,
                new_path,
                hunks: Vec::new(),
                is_new: false,
                is_deleted: false,
                is_renamed: false,
            });
        } else if line.starts_with("new file mode") {
            if let Some(ref mut file) = current_file {
                file.is_new = true;
            }
        } else if line.starts_with("deleted file mode") {
            if let Some(ref mut file) = current_file {
                file.is_deleted = true;
            }
        } else if line.starts_with("rename from") || line.starts_with("rename to") {
            if let Some(ref mut file) = current_file {
                file.is_renamed = true;
            }
        } else if line.starts_with("@@ ") {
            // Save previous hunk
            if let (Some(ref mut file), Some(hunk)) = (&mut current_file, current_hunk.take()) {
                file.hunks.push(hunk);
            }

            // Parse "@@ -old_start,old_count +new_start,new_count @@ header"
            if let Some(hunk) = parse_hunk_header(line) {
                current_hunk = Some(hunk);
            }
        } else if let Some(ref mut hunk) = current_hunk {
            let diff_line = if let Some(content) = line.strip_prefix('+') {
                DiffLine { line_type: DiffLineType::Addition, content: content.to_string() }
            } else if let Some(content) = line.strip_prefix('-') {
                DiffLine { line_type: DiffLineType::Deletion, content: content.to_string() }
            } else if let Some(content) = line.strip_prefix(' ') {
                DiffLine { line_type: DiffLineType::Context, content: content.to_string() }
            } else {
                continue;
            };
            hunk.lines.push(diff_line);
        }
    }

    // Save final hunk and file
    if let (Some(ref mut file), Some(hunk)) = (&mut current_file, current_hunk.take()) {
        file.hunks.push(hunk);
    }
    if let Some(file) = current_file.take() {
        files.push(file);
    }

    files
}

fn parse_hunk_header(line: &str) -> Option<DiffHunk> {
    // Format: @@ -old_start,old_count +new_start,new_count @@ optional_header
    let parts: Vec<&str> = line.split("@@").collect();
    if parts.len() < 3 {
        return None;
    }

    let range_str = parts[1].trim();
    let header = parts[2..].join("@@").trim().to_string();

    let ranges: Vec<&str> = range_str.split(' ').collect();
    if ranges.len() < 2 {
        return None;
    }

    let (old_start, old_count) = parse_range(ranges[0].trim_start_matches('-'))?;
    let (new_start, new_count) = parse_range(ranges[1].trim_start_matches('+'))?;

    Some(DiffHunk {
        old_start,
        old_count,
        new_start,
        new_count,
        header,
        lines: Vec::new(),
    })
}

fn parse_range(s: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = s.split(',').collect();
    let start: u32 = parts[0].parse().ok()?;
    let count: u32 = if parts.len() > 1 { parts[1].parse().ok()? } else { 1 };
    Some((start, count))
}
```

## Generating Diffs Programmatically

Your agent typically generates diffs in two contexts: showing the user what changed, and providing context to the LLM for commit message generation.

```rust
use std::path::Path;
use std::process::Command;

/// Get the diff of staged changes (what will be committed)
pub fn staged_diff(repo_dir: &Path) -> Result<String, String> {
    run_git(repo_dir, &["diff", "--cached"])
}

/// Get the diff of unstaged changes (modified but not yet staged)
pub fn unstaged_diff(repo_dir: &Path) -> Result<String, String> {
    run_git(repo_dir, &["diff"])
}

/// Get the diff between HEAD and the working tree (all changes)
pub fn all_changes_diff(repo_dir: &Path) -> Result<String, String> {
    run_git(repo_dir, &["diff", "HEAD"])
}

/// Get a summary of changes (just file names and stats)
pub fn diff_stat(repo_dir: &Path) -> Result<String, String> {
    run_git(repo_dir, &["diff", "--stat", "HEAD"])
}

/// Get the diff between two commits
pub fn diff_between(repo_dir: &Path, from: &str, to: &str) -> Result<String, String> {
    run_git(repo_dir, &["diff", from, to])
}

fn run_git(repo_dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
```

## Building a Change Summary for the LLM

Raw diffs are verbose. For commit message generation, you want a concise summary that highlights the important parts:

```rust
pub fn summarize_changes(diff_files: &[DiffFile]) -> String {
    let mut summary = String::new();

    let added: Vec<_> = diff_files.iter().filter(|f| f.is_new).collect();
    let deleted: Vec<_> = diff_files.iter().filter(|f| f.is_deleted).collect();
    let modified: Vec<_> = diff_files.iter()
        .filter(|f| !f.is_new && !f.is_deleted)
        .collect();

    if !added.is_empty() {
        summary.push_str(&format!("New files ({}):\n", added.len()));
        for f in &added {
            let lines: usize = f.hunks.iter()
                .flat_map(|h| &h.lines)
                .filter(|l| matches!(l.line_type, DiffLineType::Addition))
                .count();
            summary.push_str(&format!("  + {} ({} lines)\n", f.new_path, lines));
        }
    }

    if !deleted.is_empty() {
        summary.push_str(&format!("Deleted files ({}):\n", deleted.len()));
        for f in &deleted {
            summary.push_str(&format!("  - {}\n", f.old_path));
        }
    }

    if !modified.is_empty() {
        summary.push_str(&format!("Modified files ({}):\n", modified.len()));
        for f in &modified {
            let additions: usize = f.hunks.iter()
                .flat_map(|h| &h.lines)
                .filter(|l| matches!(l.line_type, DiffLineType::Addition))
                .count();
            let deletions: usize = f.hunks.iter()
                .flat_map(|h| &h.lines)
                .filter(|l| matches!(l.line_type, DiffLineType::Deletion))
                .count();
            summary.push_str(&format!("  ~ {} (+{} -{} lines)\n",
                f.new_path, additions, deletions));
        }
    }

    summary
}
```

::: wild In the Wild
Claude Code uses `git diff` output as context for generating commit messages. When it creates a commit, it runs `git diff --cached` to capture the staged changes, summarizes them, and feeds that summary to the LLM to produce a descriptive commit message. This pattern of using structured diff data as LLM context is one of the most practical Git integrations in any coding agent -- it turns raw changes into human-readable descriptions without requiring the agent to re-read every modified file.
:::

## Key Takeaways

- Git status compares three trees (HEAD, index, working tree) pairwise -- use `--porcelain` output for reliable machine parsing of the two-character status codes.
- Unified diff format follows a predictable structure: file headers, hunk headers with line ranges, and content lines prefixed with `+`, `-`, or space -- parse these into structured types for the agent to reason about.
- Build both detailed diff representations (for displaying to users) and compact summaries (for feeding to the LLM for commit message generation).
- Use `git2` for frequent status checks to avoid subprocess overhead, and the CLI for generating diff output that matches what users expect to see.
- The diff stat format (`--stat`) provides a quick overview of change scope that is useful for both user display and LLM context.
