---
title: Repo Analysis
description: Analyzing repository structure, file change frequency, contributor patterns, and codebase evolution to give agents contextual understanding of a project.
---

# Repo Analysis

> **What you'll learn:**
> - How to extract repository statistics like file counts, language breakdown, and size metrics
> - Techniques for identifying hot files and frequently changed areas of the codebase
> - How to use contributor and commit patterns to understand project conventions

A coding agent that understands a project's structure, history, and conventions makes better decisions. When the agent knows that `src/auth/` is the most frequently modified directory, it can anticipate complexity there. When it sees that the project follows a pattern of small, focused commits, it mirrors that style. Repository analysis turns git history into actionable intelligence.

## Repository Overview

The first thing the agent should learn about a project is its basic shape: how big is it, what languages does it use, how active is it? This overview helps the LLM understand the scope of the project:

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
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

#[derive(Debug)]
pub struct RepoOverview {
    pub total_files: usize,
    pub total_commits: usize,
    pub contributors: usize,
    pub first_commit_date: String,
    pub latest_commit_date: String,
    pub language_breakdown: Vec<(String, usize)>, // (extension, file_count)
    pub total_lines: usize,
}

/// Gather a high-level overview of the repository
pub fn repo_overview(repo_path: &Path) -> Result<RepoOverview, String> {
    // Count tracked files
    let files_output = run_git_checked(repo_path, &["ls-files"])?;
    let total_files = files_output.lines().count();

    // Count commits
    let commit_count = run_git_checked(repo_path, &["rev-list", "--count", "HEAD"])?;
    let total_commits = commit_count.parse::<usize>().unwrap_or(0);

    // Count unique contributors
    let authors = run_git_checked(repo_path, &["shortlog", "-sn", "--all", "--no-merges"])?;
    let contributors = authors.lines().count();

    // First and latest commit dates
    let first_date = run_git_checked(
        repo_path,
        &["log", "--reverse", "--format=%ci", "-1"],
    )
    .unwrap_or_else(|_| "unknown".to_string());

    let latest_date = run_git_checked(
        repo_path,
        &["log", "--format=%ci", "-1"],
    )
    .unwrap_or_else(|_| "unknown".to_string());

    // Language breakdown by file extension
    let mut ext_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for line in files_output.lines() {
        if let Some(ext) = Path::new(line).extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            *ext_counts.entry(ext_str).or_insert(0) += 1;
        }
    }

    let mut language_breakdown: Vec<(String, usize)> = ext_counts.into_iter().collect();
    language_breakdown.sort_by(|a, b| b.1.cmp(&a.1));

    // Approximate total lines (using wc on tracked files would be expensive,
    // so we use git's built-in diff stat against an empty tree)
    let empty_tree = "4b825dc642cb6eb9a060e54bf899d15006a2d65d"; // git's empty tree hash
    let line_stat = run_git_checked(
        repo_path,
        &["diff", "--stat", empty_tree, "HEAD"],
    )
    .unwrap_or_default();

    let total_lines = line_stat
        .lines()
        .last()
        .and_then(|l| {
            // Parse "X files changed, Y insertions(+)" format
            l.split(',')
                .find(|s| s.contains("insertion"))
                .and_then(|s| s.trim().split_whitespace().next())
                .and_then(|n| n.parse::<usize>().ok())
        })
        .unwrap_or(0);

    Ok(RepoOverview {
        total_files,
        total_commits,
        contributors,
        first_commit_date: first_date,
        latest_commit_date: latest_date,
        language_breakdown,
        total_lines,
    })
}

impl RepoOverview {
    /// Format the overview for injection into an LLM prompt
    pub fn summarize(&self) -> String {
        let mut output = String::new();
        output.push_str(&format!("Repository: {} files, ~{} lines of code\n", self.total_files, self.total_lines));
        output.push_str(&format!("History: {} commits by {} contributors\n", self.total_commits, self.contributors));
        output.push_str(&format!("Active from {} to {}\n", self.first_commit_date, self.latest_commit_date));

        output.push_str("Languages: ");
        let top_langs: Vec<String> = self.language_breakdown
            .iter()
            .take(5)
            .map(|(ext, count)| format!(".{} ({})", ext, count))
            .collect();
        output.push_str(&top_langs.join(", "));
        output.push('\n');

        output
    }
}

fn main() {
    let repo = Path::new(".");

    match repo_overview(repo) {
        Ok(overview) => println!("{}", overview.summarize()),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

::: python Coming from Python
In Python, you would build this same overview using `subprocess.run` calls and string parsing. Libraries like `gitpython` offer higher-level APIs (`repo.iter_commits()`, `repo.heads`), but they load objects into memory which can be slow for large repositories. The git CLI approach we use here streams output efficiently regardless of repository size. In Rust, the `HashMap` for extension counting works the same as Python's `defaultdict(int)`, though the syntax for incrementing (`*entry.or_insert(0) += 1`) is admittedly more verbose.
:::

## Identifying Hot Files

Hot files are files that change frequently. They tend to be where bugs cluster and where the most important business logic lives. Knowing which files are hot helps the agent prioritize its attention:

```rust
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

fn run_git_checked(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

#[derive(Debug)]
pub struct HotFile {
    pub path: String,
    pub change_count: usize,
    pub last_changed: String,
}

/// Find the most frequently changed files in the repository
pub fn find_hot_files(
    repo_path: &Path,
    limit: usize,
    since: Option<&str>, // e.g., "6 months ago"
) -> Result<Vec<HotFile>, String> {
    let mut args = vec![
        "log",
        "--format=",       // empty format -- we only want the file names
        "--name-only",
    ];

    if let Some(since_date) = since {
        args.push("--since");
        args.push(since_date);
    }

    let output = run_git_checked(repo_path, &args)?;

    // Count occurrences of each file path
    let mut file_counts: HashMap<String, usize> = HashMap::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            *file_counts.entry(trimmed.to_string()).or_insert(0) += 1;
        }
    }

    // Sort by count (descending) and take the top N
    let mut sorted: Vec<(String, usize)> = file_counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.truncate(limit);

    // Get the last change date for each hot file
    let mut hot_files = Vec::new();
    for (path, count) in sorted {
        let last_changed = run_git_checked(
            repo_path,
            &["log", "-1", "--format=%ci", "--", &path],
        )
        .unwrap_or_else(|_| "unknown".to_string());

        hot_files.push(HotFile {
            path,
            change_count: count,
            last_changed,
        });
    }

    Ok(hot_files)
}

fn main() {
    let repo = Path::new(".");

    match find_hot_files(repo, 10, Some("6 months ago")) {
        Ok(files) => {
            println!("Hot files (last 6 months):");
            for (i, f) in files.iter().enumerate() {
                println!(
                    "  {}. {} ({} changes, last: {})",
                    i + 1,
                    f.path,
                    f.change_count,
                    f.last_changed
                );
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Directory-Level Change Frequency

Sometimes you want to zoom out from individual files to see which directories are the most active. This helps the agent understand the project's architecture:

```rust
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

fn run_git_checked(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Analyze change frequency at the directory level
pub fn directory_activity(
    repo_path: &Path,
    depth: usize,
    since: Option<&str>,
) -> Result<Vec<(String, usize)>, String> {
    let mut args = vec!["log", "--format=", "--name-only"];

    if let Some(since_date) = since {
        args.push("--since");
        args.push(since_date);
    }

    let output = run_git_checked(repo_path, &args)?;

    let mut dir_counts: HashMap<String, usize> = HashMap::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Extract directory at the requested depth
        let parts: Vec<&str> = trimmed.split('/').collect();
        let dir = if parts.len() <= depth {
            // File is shallower than requested depth -- use full parent path
            if parts.len() > 1 {
                parts[..parts.len() - 1].join("/")
            } else {
                "(root)".to_string()
            }
        } else {
            parts[..depth].join("/")
        };

        *dir_counts.entry(dir).or_insert(0) += 1;
    }

    let mut sorted: Vec<(String, usize)> = dir_counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    Ok(sorted)
}

fn main() {
    let repo = Path::new(".");

    match directory_activity(repo, 2, Some("3 months ago")) {
        Ok(dirs) => {
            println!("Most active directories (last 3 months):");
            for (dir, count) in dirs.iter().take(10) {
                let bar_len = (*count).min(40);
                let bar = "#".repeat(bar_len);
                println!("  {:40} {:4} {}", dir, count, bar);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Commit Pattern Analysis

Understanding how a project makes commits helps the agent follow conventions. Does the project use conventional commits? Are commits small and focused or large and sweeping? The agent should match the project's style:

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
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

#[derive(Debug)]
pub struct CommitPatterns {
    pub total_commits: usize,
    pub avg_files_per_commit: f64,
    pub uses_conventional_commits: bool,
    pub common_prefixes: Vec<(String, usize)>,
    pub avg_message_length: f64,
}

/// Analyze commit patterns to understand project conventions
pub fn analyze_commit_patterns(
    repo_path: &Path,
    sample_size: usize,
) -> Result<CommitPatterns, String> {
    // Get recent commit messages and file counts
    let log_output = run_git_checked(
        repo_path,
        &[
            "log",
            &format!("-{}", sample_size),
            "--format=%s|%H",
            "--no-merges",
        ],
    )?;

    let mut messages = Vec::new();
    let mut total_files = 0;
    let mut prefix_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for line in log_output.lines() {
        let parts: Vec<&str> = line.splitn(2, '|').collect();
        if parts.len() != 2 {
            continue;
        }

        let message = parts[0];
        let hash = parts[1];
        messages.push(message.to_string());

        // Count files changed in this commit
        let numstat = run_git_checked(
            repo_path,
            &["diff", "--numstat", &format!("{}~1", hash), hash],
        );
        if let Ok(stat) = numstat {
            total_files += stat.lines().count();
        }

        // Extract conventional commit prefix (e.g., "feat", "fix", "refactor")
        if let Some(prefix) = message.split(':').next() {
            let prefix = prefix.split('(').next().unwrap_or(prefix).trim();
            if prefix.len() < 15 && prefix.chars().all(|c| c.is_alphabetic() || c == '!') {
                *prefix_counts.entry(prefix.to_lowercase()).or_insert(0) += 1;
            }
        }
    }

    let total_commits = messages.len();
    let avg_files = if total_commits > 0 {
        total_files as f64 / total_commits as f64
    } else {
        0.0
    };

    let avg_msg_len = if total_commits > 0 {
        messages.iter().map(|m| m.len()).sum::<usize>() as f64 / total_commits as f64
    } else {
        0.0
    };

    // Determine if conventional commits are used
    let conventional_prefixes = ["feat", "fix", "refactor", "docs", "test", "chore", "ci", "style"];
    let conventional_count: usize = prefix_counts
        .iter()
        .filter(|(k, _)| conventional_prefixes.contains(&k.as_str()))
        .map(|(_, v)| v)
        .sum();

    let uses_conventional = total_commits > 0 && conventional_count as f64 / total_commits as f64 > 0.5;

    let mut common_prefixes: Vec<(String, usize)> = prefix_counts.into_iter().collect();
    common_prefixes.sort_by(|a, b| b.1.cmp(&a.1));
    common_prefixes.truncate(5);

    Ok(CommitPatterns {
        total_commits,
        avg_files_per_commit: avg_files,
        uses_conventional_commits: uses_conventional,
        common_prefixes,
        avg_message_length: avg_msg_len,
    })
}

fn main() {
    let repo = Path::new(".");

    match analyze_commit_patterns(repo, 50) {
        Ok(patterns) => {
            println!("Commit patterns (last {} commits):", patterns.total_commits);
            println!("  Avg files per commit: {:.1}", patterns.avg_files_per_commit);
            println!("  Avg message length: {:.0} chars", patterns.avg_message_length);
            println!("  Uses conventional commits: {}", patterns.uses_conventional_commits);
            println!("  Common prefixes:");
            for (prefix, count) in &patterns.common_prefixes {
                println!("    {}: {}", prefix, count);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

::: wild In the Wild
Claude Code reads the recent git log at the start of a session to understand the project's commit conventions. When it creates commits, it mirrors the style it observed -- using conventional commit prefixes if the project uses them, matching the typical message length, and keeping the scope of each commit similar to the project's norm. This subtle adaptation makes agent-created commits blend in naturally with human-created ones.
:::

## Key Takeaways

- Build a repository overview at agent startup to give the LLM context about project size, language, and activity level.
- Use file change frequency analysis to identify "hot files" that are likely to be complex and important for the agent to understand deeply.
- Analyze directory-level change patterns to understand the project's architecture without reading every file.
- Study commit patterns to match the project's conventions -- conventional commits, typical scope, message length.
- All analysis data should be formatted concisely for LLM context injection, not dumped as raw git output.
