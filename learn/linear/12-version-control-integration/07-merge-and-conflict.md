---
title: Merge and Conflict
description: Handling merge operations and conflict resolution programmatically — detecting conflicts, parsing conflict markers, and presenting resolution options.
---

# Merge and Conflict

> **What you'll learn:**
> - How Git detects and represents merge conflicts with conflict markers (<<<<<<, =======, >>>>>>>) in the working tree
> - Parsing conflict markers to extract the base, ours, and theirs versions for programmatic resolution
> - Strategies for automatic conflict resolution (accept ours/theirs, semantic merge) and presenting manual resolution UI to the user

Merge conflicts are inevitable when an agent works alongside human developers. The agent modifies files on a feature branch, the developer pushes changes to main, and when the branches merge, Git cannot automatically reconcile the differences. Your agent needs to detect conflicts, understand what they mean, and either resolve them automatically or present clear options to the user.

## How Merge Conflicts Arise

A conflict occurs when two branches modify the same lines in the same file. Git can automatically merge changes that touch different files or different parts of the same file, but when both branches alter the same region, Git writes both versions into the file with conflict markers and stops the merge.

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct MergeManager {
    repo_dir: PathBuf,
}

impl MergeManager {
    pub fn new(repo_dir: impl Into<PathBuf>) -> Self {
        Self { repo_dir: repo_dir.into() }
    }

    /// Attempt to merge a branch into the current branch
    pub fn merge(&self, branch: &str) -> Result<MergeResult, String> {
        let output = Command::new("git")
            .args(["merge", "--no-edit", branch])
            .current_dir(&self.repo_dir)
            .output()
            .map_err(|e| format!("Failed to run git merge: {}", e))?;

        if output.status.success() {
            Ok(MergeResult::Clean)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("CONFLICT") || stderr.contains("Automatic merge failed") {
                // Collect the conflicted files
                let conflicts = self.list_conflicted_files()?;
                Ok(MergeResult::Conflict(conflicts))
            } else {
                Err(format!("Merge failed: {}", stderr))
            }
        }
    }

    /// List files that are in a conflicted state
    pub fn list_conflicted_files(&self) -> Result<Vec<PathBuf>, String> {
        let output = Command::new("git")
            .args(["diff", "--name-only", "--diff-filter=U"])
            .current_dir(&self.repo_dir)
            .output()
            .map_err(|e| format!("Failed to list conflicts: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines()
            .filter(|l| !l.is_empty())
            .map(PathBuf::from)
            .collect())
    }

    /// Abort a merge in progress
    pub fn abort(&self) -> Result<(), String> {
        let output = Command::new("git")
            .args(["merge", "--abort"])
            .current_dir(&self.repo_dir)
            .output()
            .map_err(|e| format!("Failed to abort merge: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}

#[derive(Debug)]
pub enum MergeResult {
    Clean,
    Conflict(Vec<PathBuf>),
}
```

## Parsing Conflict Markers

When Git encounters a conflict, it writes both versions into the file using a specific marker format. With the `diff3` conflict style (recommended for agents), you get three sections:

```text
<<<<<<< HEAD
    let timeout = Duration::from_secs(30);
=======
    let timeout = Duration::from_secs(60);
>>>>>>> feature/increase-timeout
```

Or with diff3 style (which includes the common ancestor):

```text
<<<<<<< HEAD
    let timeout = Duration::from_secs(30);
||||||| merged common ancestors
    let timeout = Duration::from_secs(10);
=======
    let timeout = Duration::from_secs(60);
>>>>>>> feature/increase-timeout
```

Let's parse these into structured data:

```rust
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ConflictRegion {
    pub ours: Vec<String>,
    pub base: Option<Vec<String>>,  // Present with diff3 style
    pub theirs: Vec<String>,
    pub ours_label: String,
    pub theirs_label: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug)]
pub struct FileConflicts {
    pub path: String,
    pub regions: Vec<ConflictRegion>,
    pub non_conflict_lines: usize,
    pub total_lines: usize,
}

pub fn parse_conflict_markers(file_path: &Path) -> Result<FileConflicts, String> {
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read {}: {}", file_path.display(), e))?;

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();
    let mut regions = Vec::new();
    let mut non_conflict_lines = 0;
    let mut i = 0;

    while i < lines.len() {
        if lines[i].starts_with("<<<<<<< ") {
            let start_line = i + 1;
            let ours_label = lines[i][8..].to_string();
            let mut ours = Vec::new();
            let mut base = None;
            let mut theirs = Vec::new();
            let mut theirs_label = String::new();

            i += 1;
            // Collect "ours" lines
            let mut current_section = &mut ours;
            while i < lines.len() {
                if lines[i].starts_with("||||||| ") {
                    // Start of base section (diff3 style)
                    base = Some(Vec::new());
                    i += 1;
                    while i < lines.len() && !lines[i].starts_with("=======") {
                        base.as_mut().unwrap().push(lines[i].to_string());
                        i += 1;
                    }
                    break;
                } else if lines[i].starts_with("=======") {
                    break;
                } else {
                    current_section.push(lines[i].to_string());
                    i += 1;
                }
            }

            // Skip the ======= separator
            i += 1;

            // Collect "theirs" lines
            while i < lines.len() {
                if lines[i].starts_with(">>>>>>> ") {
                    theirs_label = lines[i][8..].to_string();
                    break;
                }
                theirs.push(lines[i].to_string());
                i += 1;
            }

            regions.push(ConflictRegion {
                ours,
                base,
                theirs,
                ours_label,
                theirs_label,
                start_line,
                end_line: i + 1,
            });
        } else {
            non_conflict_lines += 1;
        }
        i += 1;
    }

    Ok(FileConflicts {
        path: file_path.to_string_lossy().to_string(),
        regions,
        non_conflict_lines,
        total_lines,
    })
}
```

::: python Coming from Python
Python developers often parse conflict markers with regex: `re.findall(r'<{7}.*?\n(.*?)\n={7}\n(.*?)\n>{7}', content, re.DOTALL)`. The Rust approach uses explicit line-by-line parsing instead, which handles edge cases better (like conflict markers inside string literals) and gives you precise line numbers. The structured `ConflictRegion` type also prevents downstream code from accidentally accessing fields that do not exist in the non-diff3 format.
:::

## Automatic Resolution Strategies

For straightforward conflicts, your agent can resolve them without user intervention. Here are the common strategies:

```rust
use std::fs;
use std::path::Path;
use std::process::Command;

pub enum ResolutionStrategy {
    AcceptOurs,
    AcceptTheirs,
    AcceptBoth,   // Keep both changes (useful for additive changes)
    Custom(Vec<String>),  // Agent-provided resolution
}

pub fn resolve_conflict(
    file_path: &Path,
    conflicts: &FileConflicts,
    strategies: &[ResolutionStrategy],
) -> Result<String, String> {
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let lines: Vec<&str> = content.lines().collect();
    let mut resolved = Vec::new();
    let mut i = 0;
    let mut region_idx = 0;

    while i < lines.len() {
        if lines[i].starts_with("<<<<<<< ") && region_idx < strategies.len() {
            let region = &conflicts.regions[region_idx];
            let strategy = &strategies[region_idx];

            match strategy {
                ResolutionStrategy::AcceptOurs => {
                    resolved.extend(region.ours.iter().map(|s| s.as_str()));
                }
                ResolutionStrategy::AcceptTheirs => {
                    resolved.extend(region.theirs.iter().map(|s| s.as_str()));
                }
                ResolutionStrategy::AcceptBoth => {
                    resolved.extend(region.ours.iter().map(|s| s.as_str()));
                    resolved.extend(region.theirs.iter().map(|s| s.as_str()));
                }
                ResolutionStrategy::Custom(lines) => {
                    resolved.extend(lines.iter().map(|s| s.as_str()));
                }
            }

            // Skip past the conflict markers
            i = region.end_line;
            region_idx += 1;
        } else {
            resolved.push(lines[i]);
            i += 1;
        }
    }

    let resolved_content = resolved.join("\n") + "\n";
    fs::write(file_path, &resolved_content)
        .map_err(|e| format!("Failed to write resolved file: {}", e))?;

    Ok(resolved_content)
}

/// Quick resolution: accept all ours or all theirs for a file
pub fn resolve_file_simple(
    repo_dir: &Path,
    file_path: &str,
    accept_ours: bool,
) -> Result<(), String> {
    let strategy = if accept_ours { "--ours" } else { "--theirs" };

    let output = Command::new("git")
        .args(["checkout", strategy, "--", file_path])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to resolve conflict: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    // Stage the resolved file
    Command::new("git")
        .args(["add", file_path])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to stage resolved file: {}", e))?;

    Ok(())
}
```

## Presenting Conflicts to the User

When automatic resolution is not appropriate, the agent should present the conflict clearly so the user can make an informed decision:

```rust
pub fn format_conflict_for_display(conflict: &ConflictRegion) -> String {
    let mut output = String::new();

    output.push_str(&format!("--- Conflict (lines {}-{}) ---\n",
        conflict.start_line, conflict.end_line));

    output.push_str(&format!("\nOption A ({}): \n", conflict.ours_label));
    for line in &conflict.ours {
        output.push_str(&format!("  {}\n", line));
    }

    if let Some(base) = &conflict.base {
        output.push_str("\nOriginal (common ancestor):\n");
        for line in base {
            output.push_str(&format!("  {}\n", line));
        }
    }

    output.push_str(&format!("\nOption B ({}):\n", conflict.theirs_label));
    for line in &conflict.theirs {
        output.push_str(&format!("  {}\n", line));
    }

    output.push_str("\nChoose: [A]ccept ours, [B]ccept theirs, [M]erge both, [E]dit manually\n");
    output
}

pub fn summarize_merge_conflicts(files: &[FileConflicts]) -> String {
    let mut summary = format!("{} file(s) with conflicts:\n\n", files.len());

    for file in files {
        let total_conflict_lines: usize = file.regions.iter()
            .map(|r| r.ours.len() + r.theirs.len())
            .sum();
        summary.push_str(&format!(
            "  {} -- {} conflict region(s), {} conflicting lines out of {} total\n",
            file.path,
            file.regions.len(),
            total_conflict_lines,
            file.total_lines,
        ));
    }

    summary
}
```

::: wild In the Wild
Claude Code detects merge conflicts by checking for the `CONFLICTED` status in `git status` output after operations that could produce conflicts. When conflicts are found, Claude Code reads the conflicted files, understands the semantic meaning of both sides using the LLM, and proposes a resolution that preserves the intent of both changes. This is more sophisticated than simple "accept ours/theirs" -- the LLM can reason about whether two changes are truly incompatible or can be combined in a way that satisfies both.
:::

## Dry-Run Merges

Before performing a merge, your agent can check whether it will produce conflicts without actually modifying the working tree:

```rust
use std::path::Path;
use std::process::Command;

/// Check if merging a branch would produce conflicts
pub fn would_conflict(repo_dir: &Path, branch: &str) -> Result<bool, String> {
    // Use merge-tree to simulate the merge (Git 2.38+)
    let output = Command::new("git")
        .args(["merge-tree", "--write-tree", "HEAD", branch])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to run merge-tree: {}", e))?;

    // Exit code 0: clean merge possible
    // Exit code 1: conflicts detected
    Ok(!output.status.success())
}

/// Get the list of files that would conflict without performing the merge
pub fn predict_conflicts(repo_dir: &Path, branch: &str) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args(["merge-tree", "--write-tree", "--name-only", "HEAD", branch])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to run merge-tree: {}", e))?;

    if output.status.success() {
        return Ok(Vec::new()); // No conflicts
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse the output to find conflicted file names
    let conflicts: Vec<String> = stdout.lines()
        .filter(|l| !l.is_empty() && !l.starts_with(|c: char| c.is_ascii_hexdigit()))
        .map(|l| l.to_string())
        .collect();

    Ok(conflicts)
}
```

## Key Takeaways

- Merge conflicts occur when two branches modify the same lines -- Git writes both versions with conflict markers (`<<<<<<<`, `=======`, `>>>>>>>`) and your agent must parse them to understand the competing changes.
- Enable `diff3` conflict style (`git config merge.conflictstyle diff3`) to get the common ancestor in conflict markers, which gives the agent more context for intelligent resolution.
- Provide multiple resolution strategies: `--ours`/`--theirs` for quick resolution, combined resolution for additive changes, and LLM-powered semantic resolution for complex conflicts.
- Use `git merge-tree` (Git 2.38+) to predict conflicts before performing a merge, allowing the agent to inform the user of potential issues without modifying the working tree.
- Always present conflicts clearly to the user with both versions and the common ancestor, letting them choose the resolution strategy rather than silently picking one.
