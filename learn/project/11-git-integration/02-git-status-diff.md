---
title: Git Status and Diff
description: Implementing git status and diff commands as agent tools, parsing porcelain output for machine-readable state, and presenting repository changes to the LLM context.
---

# Git Status and Diff

> **What you'll learn:**
> - How to invoke git status with porcelain flags for reliable machine-readable output
> - Techniques for parsing staged, unstaged, and untracked file lists into structured data
> - How to generate and present diffs that fit within LLM context windows

The first thing your agent should do before making any change is check the current state of the repository. Is the working tree clean? Are there uncommitted changes that might conflict with the agent's planned edits? `git status` and `git diff` are the agent's eyes into the repository, and parsing their output reliably is the foundation for every other git operation.

## Porcelain vs. Human-Readable Output

Git has two output modes. The default is designed for humans reading a terminal -- it includes colored text, section headers, and helpful suggestions. The **porcelain** mode is designed for scripts and programs -- it produces stable, parseable output that will not change between git versions.

Always use porcelain mode in your agent. The human-readable format changes across git versions and locales (yes, git translates its output into the user's language). Porcelain output is guaranteed to be stable.

```rust
use std::path::Path;
use std::process::Command;

/// Never do this -- human-readable output is unstable
fn bad_status(repo_path: &Path) -> String {
    let output = Command::new("git")
        .arg("status")
        .current_dir(repo_path)
        .output()
        .expect("git failed");
    String::from_utf8_lossy(&output.stdout).to_string()
    // Output might be in French, German, or Japanese depending on locale!
}

/// Always use porcelain format for machine parsing
fn good_status(repo_path: &Path) -> String {
    let output = Command::new("git")
        .args(["status", "--porcelain=v2", "--branch"])
        .current_dir(repo_path)
        .output()
        .expect("git failed");
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn main() {
    let repo = Path::new(".");
    println!("Porcelain output:\n{}", good_status(repo));
}
```

## Parsing Porcelain v2 Output

The `--porcelain=v2` format gives each file a line starting with a type indicator:

- `1` -- ordinary changed entry (modified, type-changed, etc.)
- `2` -- renamed or copied entry
- `u` -- unmerged entry (conflict)
- `?` -- untracked file
- `!` -- ignored file

Each ordinary entry has the format:
```
1 XY sub mH mI mW hH hI path
```

Where `X` is the staged status and `Y` is the unstaged status. The status codes you will encounter most are: `M` (modified), `A` (added), `D` (deleted), `.` (not modified).

Let's build a proper parser:

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed { from: String },
    Untracked,
    Unmerged,
}

#[derive(Debug, Clone)]
pub struct StatusEntry {
    pub path: PathBuf,
    pub staged: Option<FileStatus>,
    pub unstaged: Option<FileStatus>,
}

#[derive(Debug)]
pub struct RepoStatus {
    pub branch: Option<String>,
    pub entries: Vec<StatusEntry>,
}

impl RepoStatus {
    /// Files with staged changes ready to commit
    pub fn staged_files(&self) -> Vec<&StatusEntry> {
        self.entries.iter().filter(|e| e.staged.is_some()).collect()
    }

    /// Files with unstaged working tree changes
    pub fn unstaged_files(&self) -> Vec<&StatusEntry> {
        self.entries.iter().filter(|e| e.unstaged.is_some()).collect()
    }

    /// Untracked files not yet known to git
    pub fn untracked_files(&self) -> Vec<&StatusEntry> {
        self.entries
            .iter()
            .filter(|e| matches!(e.unstaged, Some(FileStatus::Untracked)))
            .collect()
    }

    /// True if the working tree has no changes at all
    pub fn is_clean(&self) -> bool {
        self.entries.is_empty()
    }
}

fn parse_status_code(code: char) -> Option<FileStatus> {
    match code {
        'M' => Some(FileStatus::Modified),
        'A' => Some(FileStatus::Added),
        'D' => Some(FileStatus::Deleted),
        '.' => None,
        _ => None,
    }
}

pub fn parse_git_status(repo_path: &Path) -> Result<RepoStatus, String> {
    let output = Command::new("git")
        .args(["status", "--porcelain=v2", "--branch"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut branch = None;
    let mut entries = Vec::new();

    for line in stdout.lines() {
        if line.starts_with("# branch.head ") {
            branch = Some(line["# branch.head ".len()..].to_string());
            continue;
        }

        if line.starts_with("1 ") {
            // Ordinary changed entry: 1 XY sub mH mI mW hH hI path
            let parts: Vec<&str> = line.splitn(9, ' ').collect();
            if parts.len() >= 9 {
                let xy: Vec<char> = parts[1].chars().collect();
                if xy.len() == 2 {
                    entries.push(StatusEntry {
                        path: PathBuf::from(parts[8]),
                        staged: parse_status_code(xy[0]),
                        unstaged: parse_status_code(xy[1]),
                    });
                }
            }
        } else if line.starts_with("2 ") {
            // Renamed entry: 2 XY sub mH mI mW hH hI X-score path\torigPath
            let parts: Vec<&str> = line.splitn(10, ' ').collect();
            if parts.len() >= 10 {
                let path_parts: Vec<&str> = parts[9].splitn(2, '\t').collect();
                entries.push(StatusEntry {
                    path: PathBuf::from(path_parts[0]),
                    staged: Some(FileStatus::Renamed {
                        from: path_parts.get(1).unwrap_or(&"").to_string(),
                    }),
                    unstaged: None,
                });
            }
        } else if line.starts_with("u ") {
            // Unmerged entry
            let parts: Vec<&str> = line.splitn(11, ' ').collect();
            if parts.len() >= 11 {
                entries.push(StatusEntry {
                    path: PathBuf::from(parts[10]),
                    staged: Some(FileStatus::Unmerged),
                    unstaged: Some(FileStatus::Unmerged),
                });
            }
        } else if line.starts_with("? ") {
            // Untracked file
            entries.push(StatusEntry {
                path: PathBuf::from(&line[2..]),
                staged: None,
                unstaged: Some(FileStatus::Untracked),
            });
        }
    }

    Ok(RepoStatus { branch, entries })
}

fn main() {
    let repo = Path::new(".");
    match parse_git_status(repo) {
        Ok(status) => {
            println!("Branch: {:?}", status.branch);
            println!("Clean: {}", status.is_clean());
            for entry in &status.entries {
                println!("  {:?} -> staged: {:?}, unstaged: {:?}",
                    entry.path, entry.staged, entry.unstaged);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

::: python Coming from Python
In Python, you might use `subprocess.run(["git", "status", "--porcelain"], capture_output=True, text=True)` and split on newlines. The Rust version is structurally identical -- the main difference is that you get typed structs (`StatusEntry`, `FileStatus`) instead of dictionaries, and the compiler ensures you handle every variant of the `FileStatus` enum. You cannot accidentally forget to handle the "renamed" case.
:::

## Generating Diffs for the Agent

Raw diff output can be enormous. A single file refactor might produce thousands of lines of diff. Your agent needs two things: a concise summary for deciding what to do, and a detailed diff for understanding specific changes.

```rust
use std::path::Path;
use std::process::Command;

#[derive(Debug)]
pub struct DiffSummary {
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub file_stats: Vec<FileDiffStat>,
}

#[derive(Debug)]
pub struct FileDiffStat {
    pub path: String,
    pub insertions: usize,
    pub deletions: usize,
}

/// Get a high-level summary of changes (like git diff --stat)
pub fn diff_summary(repo_path: &Path, cached: bool) -> Result<DiffSummary, String> {
    let mut args = vec!["diff", "--numstat"];
    if cached {
        args.push("--cached");
    }

    let output = Command::new("git")
        .args(&args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git diff: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut file_stats = Vec::new();
    let mut total_insertions = 0;
    let mut total_deletions = 0;

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() == 3 {
            let ins = parts[0].parse::<usize>().unwrap_or(0);
            let del = parts[1].parse::<usize>().unwrap_or(0);
            total_insertions += ins;
            total_deletions += del;
            file_stats.push(FileDiffStat {
                path: parts[2].to_string(),
                insertions: ins,
                deletions: del,
            });
        }
    }

    Ok(DiffSummary {
        files_changed: file_stats.len(),
        insertions: total_insertions,
        deletions: total_deletions,
        file_stats,
    })
}

/// Get detailed diff for specific files, truncated to a token budget
pub fn diff_detail(
    repo_path: &Path,
    paths: &[&str],
    max_lines: usize,
) -> Result<String, String> {
    let mut args = vec!["diff", "-U3", "--"];
    args.extend_from_slice(paths);

    let output = Command::new("git")
        .args(&args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git diff: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    if lines.len() <= max_lines {
        Ok(stdout.to_string())
    } else {
        let truncated: String = lines[..max_lines].join("\n");
        Ok(format!(
            "{}\n\n... truncated ({} more lines, {} total)",
            truncated,
            lines.len() - max_lines,
            lines.len()
        ))
    }
}

fn main() {
    let repo = Path::new(".");

    // Show unstaged changes summary
    match diff_summary(repo, false) {
        Ok(summary) => {
            println!("Unstaged changes: {} files, +{} -{}",
                summary.files_changed, summary.insertions, summary.deletions);
            for stat in &summary.file_stats {
                println!("  {} (+{} -{})", stat.path, stat.insertions, stat.deletions);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    // Show staged changes summary
    match diff_summary(repo, true) {
        Ok(summary) => {
            println!("Staged changes: {} files, +{} -{}",
                summary.files_changed, summary.insertions, summary.deletions);
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Formatting Status for the LLM

When your agent feeds repository state into the LLM context, formatting matters. A wall of raw porcelain output wastes tokens and confuses the model. Here is a formatter that produces clean, readable output:

```rust
use std::path::PathBuf;

// Using the types from our earlier parser
#[derive(Debug)]
enum FileStatus { Modified, Added, Deleted, Renamed { from: String }, Untracked, Unmerged }

#[derive(Debug)]
struct StatusEntry {
    path: PathBuf,
    staged: Option<FileStatus>,
    unstaged: Option<FileStatus>,
}

struct RepoStatus {
    branch: Option<String>,
    entries: Vec<StatusEntry>,
}

/// Format status for injection into an LLM prompt
fn format_status_for_llm(status: &RepoStatus) -> String {
    let mut output = String::new();

    if let Some(ref branch) = status.branch {
        output.push_str(&format!("On branch: {}\n", branch));
    }

    if status.entries.is_empty() {
        output.push_str("Working tree is clean -- no pending changes.\n");
        return output;
    }

    let staged: Vec<_> = status.entries.iter()
        .filter(|e| e.staged.is_some())
        .collect();
    let unstaged: Vec<_> = status.entries.iter()
        .filter(|e| matches!(&e.unstaged, Some(s) if !matches!(s, FileStatus::Untracked)))
        .collect();
    let untracked: Vec<_> = status.entries.iter()
        .filter(|e| matches!(e.unstaged, Some(FileStatus::Untracked)))
        .collect();

    if !staged.is_empty() {
        output.push_str(&format!("Staged for commit ({} files):\n", staged.len()));
        for entry in &staged {
            let label = match &entry.staged {
                Some(FileStatus::Modified) => "modified",
                Some(FileStatus::Added) => "new file",
                Some(FileStatus::Deleted) => "deleted",
                Some(FileStatus::Renamed { from }) => {
                    output.push_str(&format!("  renamed: {} -> {}\n", from, entry.path.display()));
                    continue;
                }
                _ => "changed",
            };
            output.push_str(&format!("  {}: {}\n", label, entry.path.display()));
        }
    }

    if !unstaged.is_empty() {
        output.push_str(&format!("Unstaged changes ({} files):\n", unstaged.len()));
        for entry in &unstaged {
            output.push_str(&format!("  modified: {}\n", entry.path.display()));
        }
    }

    if !untracked.is_empty() {
        output.push_str(&format!("Untracked files ({}):\n", untracked.len()));
        for entry in &untracked {
            output.push_str(&format!("  {}\n", entry.path.display()));
        }
    }

    output
}

fn main() {
    // Example with some test data
    let status = RepoStatus {
        branch: Some("feature/add-git-tools".to_string()),
        entries: vec![
            StatusEntry {
                path: PathBuf::from("src/tools/git.rs"),
                staged: Some(FileStatus::Added),
                unstaged: None,
            },
            StatusEntry {
                path: PathBuf::from("src/main.rs"),
                staged: None,
                unstaged: Some(FileStatus::Modified),
            },
            StatusEntry {
                path: PathBuf::from("TODO.md"),
                staged: None,
                unstaged: Some(FileStatus::Untracked),
            },
        ],
    };

    println!("{}", format_status_for_llm(&status));
}
```

::: wild In the Wild
Claude Code injects a concise git status summary into the system prompt at the start of every conversation. This gives the LLM immediate awareness of whether there are pending changes, what branch the user is on, and whether there is anything that needs to be committed before starting new work. The summary is kept deliberately short -- just branch name and a file count -- to avoid consuming context tokens.
:::

## Key Takeaways

- Always use `--porcelain=v2` for git status output -- human-readable format is unstable across git versions and locales.
- Parse porcelain output into structured Rust types (`StatusEntry`, `FileStatus`) so your agent can reason about file states programmatically.
- Use `git diff --numstat` for summaries and `git diff -U3` for details, truncating large diffs to fit LLM context windows.
- Format repository state concisely for the LLM -- include branch name, file counts, and status categories without raw git output noise.
- Check status before every mutating operation so the agent knows what it is working with and can avoid conflicts.
