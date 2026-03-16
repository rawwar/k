---
title: Log and Blame
description: Leveraging git log and git blame to understand code history, trace when and why changes were made, and provide the agent with authorship context for smarter edits.
---

# Log and Blame

> **What you'll learn:**
> - How to query git log with format strings and filters for structured commit history
> - How to use git blame to attribute lines to specific commits and authors
> - Patterns for feeding historical context into agent prompts to improve edit quality

Git log and blame are the agent's historical memory. When the agent needs to modify a function, knowing who wrote it, when, and why provides critical context. Was this code recently rewritten as part of a major refactor? Was it a quick bug fix? Is there a pattern of changes that suggests ongoing instability? This historical context helps the LLM make better editing decisions.

## Structured Log Queries

Git log is incredibly flexible -- you can control exactly what information appears and in what format. For agent use, you want structured output that is easy to parse:

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

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
    pub files_changed: Vec<String>,
}

/// Query git log with structured output
pub fn git_log(
    repo_path: &Path,
    count: usize,
    path_filter: Option<&str>,
    author_filter: Option<&str>,
    since: Option<&str>,
    grep_filter: Option<&str>,
) -> Result<Vec<LogEntry>, String> {
    // Use a delimiter that is unlikely to appear in commit messages
    let delimiter = "<<|>>";
    let format = format!(
        "{}%H{}%h{}%an{}%ci{}%s",
        delimiter, delimiter, delimiter, delimiter, delimiter
    );

    let mut args = vec![
        "log",
        &format!("-{}", count),
        &format!("--format={}", format),
        "--no-merges",
        "--name-only",
    ];

    // Build a vector to hold owned strings that we need to reference
    let author_arg;
    let since_arg;
    let grep_arg;

    if let Some(author) = author_filter {
        author_arg = format!("--author={}", author);
        args.push(&author_arg);
    }

    if let Some(since_date) = since {
        since_arg = format!("--since={}", since_date);
        args.push(&since_arg);
    }

    if let Some(grep) = grep_filter {
        grep_arg = format!("--grep={}", grep);
        args.push(&grep_arg);
    }

    if let Some(path) = path_filter {
        args.push("--");
        args.push(path);
    }

    let output = run_git_checked(repo_path, &args)?;

    let mut entries = Vec::new();
    let mut current_files = Vec::new();
    let mut last_entry: Option<LogEntry> = None;

    for line in output.lines() {
        if line.starts_with(delimiter) {
            // Save previous entry
            if let Some(mut entry) = last_entry.take() {
                entry.files_changed = current_files.clone();
                entries.push(entry);
                current_files.clear();
            }

            // Parse new entry
            let parts: Vec<&str> = line.split(delimiter).collect();
            // parts: ["", hash, short_hash, author, date, message]
            if parts.len() >= 6 {
                last_entry = Some(LogEntry {
                    hash: parts[1].to_string(),
                    short_hash: parts[2].to_string(),
                    author: parts[3].to_string(),
                    date: parts[4].to_string(),
                    message: parts[5].to_string(),
                    files_changed: Vec::new(),
                });
            }
        } else if !line.trim().is_empty() {
            current_files.push(line.trim().to_string());
        }
    }

    // Don't forget the last entry
    if let Some(mut entry) = last_entry {
        entry.files_changed = current_files;
        entries.push(entry);
    }

    Ok(entries)
}

fn main() {
    let repo = Path::new(".");

    // Recent commits
    match git_log(repo, 5, None, None, None, None) {
        Ok(entries) => {
            println!("Recent commits:");
            for entry in &entries {
                println!("  {} {} - {} (by {})",
                    entry.short_hash, entry.date, entry.message, entry.author);
                for f in &entry.files_changed {
                    println!("    {}", f);
                }
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    // Commits that mention "fix" in a specific file
    match git_log(repo, 10, Some("src/main.rs"), None, None, Some("fix")) {
        Ok(entries) => {
            println!("\nBug fixes touching main.rs:");
            for entry in &entries {
                println!("  {} {}", entry.short_hash, entry.message);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

::: python Coming from Python
Python's `gitpython` library offers `repo.iter_commits()` which yields `Commit` objects with attributes like `.author`, `.message`, `.committed_datetime`. In Rust, we parse git's format string output instead. The advantage of the format-string approach is that it works identically whether you are in Python or Rust, and it is faster than loading full commit objects for large repositories. The `delimiter` trick (using an unlikely string like `<<|>>`) is a common pattern for parsing multi-field records from command-line output.
:::

## Git Blame for Line Attribution

`git blame` annotates each line of a file with the commit that last changed it. This is invaluable for the agent: when it needs to modify a function, blame tells it who wrote each line, when, and in which commit. This context helps the LLM understand the code's history:

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

#[derive(Debug, Clone)]
pub struct BlameLine {
    pub commit_hash: String,
    pub author: String,
    pub date: String,
    pub line_number: usize,
    pub content: String,
}

/// Run git blame on a file (or specific line range)
pub fn git_blame(
    repo_path: &Path,
    file_path: &str,
    line_range: Option<(usize, usize)>,
) -> Result<Vec<BlameLine>, String> {
    let mut args = vec!["blame", "--porcelain"];

    let range_arg;
    if let Some((start, end)) = line_range {
        range_arg = format!("-L{},{}", start, end);
        args.push(&range_arg);
    }

    args.push(file_path);

    let output = run_git_checked(repo_path, &args)?;

    let mut blame_lines = Vec::new();
    let mut current_hash = String::new();
    let mut current_author = String::new();
    let mut current_date = String::new();
    let mut current_line_num = 0;

    for line in output.lines() {
        // Porcelain format: first line of each entry is "hash origline finalline numlines"
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.len() >= 3 && parts[0].len() == 40 && parts[0].chars().all(|c| c.is_ascii_hexdigit()) {
            current_hash = parts[0].to_string();
            // finalline is the line number in the current file
            current_line_num = parts[2].parse::<usize>().unwrap_or(0);
        } else if line.starts_with("author ") {
            current_author = line["author ".len()..].to_string();
        } else if line.starts_with("author-time ") {
            // Convert Unix timestamp to readable date
            let timestamp = line["author-time ".len()..].parse::<i64>().unwrap_or(0);
            current_date = format_timestamp(timestamp);
        } else if line.starts_with('\t') {
            // The actual line content (prefixed with a tab)
            blame_lines.push(BlameLine {
                commit_hash: current_hash[..8.min(current_hash.len())].to_string(),
                author: current_author.clone(),
                date: current_date.clone(),
                line_number: current_line_num,
                content: line[1..].to_string(), // Remove the leading tab
            });
        }
    }

    Ok(blame_lines)
}

fn format_timestamp(timestamp: i64) -> String {
    // Simple date formatting without external crates
    // In production, you would use the chrono crate
    let days_since_epoch = timestamp / 86400;
    let years = 1970 + days_since_epoch / 365;
    format!("{}", years) // Simplified -- just the year
}

/// Summarize blame information for a file
pub fn blame_summary(repo_path: &Path, file_path: &str) -> Result<String, String> {
    let blame = git_blame(repo_path, file_path, None)?;

    // Count lines per author
    let mut author_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for line in &blame {
        *author_counts.entry(line.author.clone()).or_insert(0) += 1;
    }

    let mut sorted_authors: Vec<(String, usize)> = author_counts.into_iter().collect();
    sorted_authors.sort_by(|a, b| b.1.cmp(&a.1));

    let total_lines = blame.len();
    let mut summary = format!("{}: {} lines\n", file_path, total_lines);
    summary.push_str("Authors:\n");
    for (author, count) in &sorted_authors {
        let pct = (*count as f64 / total_lines as f64 * 100.0) as usize;
        summary.push_str(&format!("  {} - {} lines ({}%)\n", author, count, pct));
    }

    Ok(summary)
}

fn main() {
    let repo = Path::new(".");

    // Blame a specific file
    match git_blame(repo, "src/main.rs", Some((1, 20))) {
        Ok(lines) => {
            println!("Blame for src/main.rs lines 1-20:");
            for bl in &lines {
                println!("  {} {:15} L{:3}: {}",
                    bl.commit_hash, bl.author, bl.line_number, bl.content);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    // Get a blame summary
    match blame_summary(repo, "src/main.rs") {
        Ok(summary) => println!("\n{}", summary),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## File History for Context

When the agent is about to modify a file, understanding its recent history helps it make better decisions. Did someone just refactor this file? Is it being actively worked on by another developer? Has it been stable for months?

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
pub struct FileHistory {
    pub path: String,
    pub total_commits: usize,
    pub last_modified: String,
    pub last_author: String,
    pub recent_messages: Vec<String>,
    pub change_frequency: String, // "stable", "moderate", "volatile"
}

/// Get the history of a specific file for context
pub fn file_history(repo_path: &Path, file_path: &str) -> Result<FileHistory, String> {
    // Total commits touching this file
    let count_output = run_git_checked(
        repo_path,
        &["rev-list", "--count", "HEAD", "--", file_path],
    )?;
    let total_commits = count_output.parse::<usize>().unwrap_or(0);

    // Last modified date and author
    let last_info = run_git_checked(
        repo_path,
        &["log", "-1", "--format=%ci|%an", "--", file_path],
    )?;
    let info_parts: Vec<&str> = last_info.splitn(2, '|').collect();
    let last_modified = info_parts.first().unwrap_or(&"unknown").to_string();
    let last_author = info_parts.get(1).unwrap_or(&"unknown").to_string();

    // Recent commit messages for this file
    let messages_output = run_git_checked(
        repo_path,
        &["log", "-5", "--format=%s", "--", file_path],
    )?;
    let recent_messages: Vec<String> = messages_output.lines().map(String::from).collect();

    // Determine change frequency based on commits in the last 3 months
    let recent_count = run_git_checked(
        repo_path,
        &["rev-list", "--count", "--since=3 months ago", "HEAD", "--", file_path],
    )
    .ok()
    .and_then(|s| s.parse::<usize>().ok())
    .unwrap_or(0);

    let change_frequency = match recent_count {
        0..=1 => "stable",
        2..=5 => "moderate",
        _ => "volatile",
    };

    Ok(FileHistory {
        path: file_path.to_string(),
        total_commits,
        last_modified,
        last_author,
        recent_messages,
        change_frequency: change_frequency.to_string(),
    })
}

impl FileHistory {
    /// Format for LLM context injection
    pub fn for_llm(&self) -> String {
        let mut output = format!(
            "File: {} ({})\n  {} total changes, last modified by {} on {}\n",
            self.path, self.change_frequency, self.total_commits,
            self.last_author, self.last_modified
        );

        if !self.recent_messages.is_empty() {
            output.push_str("  Recent changes:\n");
            for msg in &self.recent_messages {
                output.push_str(&format!("    - {}\n", msg));
            }
        }

        output
    }
}

fn main() {
    let repo = Path::new(".");

    match file_history(repo, "src/main.rs") {
        Ok(history) => println!("{}", history.for_llm()),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Searching Commit History

Sometimes the agent needs to find when a specific change was introduced -- perhaps to understand why a certain pattern exists. Git log's `--grep` and `-S` flags are powerful search tools:

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

/// Search for commits whose messages match a pattern
pub fn search_commits_by_message(
    repo_path: &Path,
    pattern: &str,
    limit: usize,
) -> Result<Vec<(String, String, String)>, String> {
    let output = run_git_checked(
        repo_path,
        &[
            "log",
            &format!("-{}", limit),
            &format!("--grep={}", pattern),
            "-i", // case insensitive
            "--format=%h|%ci|%s",
        ],
    )?;

    Ok(output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(3, '|').collect();
            if parts.len() == 3 {
                Some((parts[0].to_string(), parts[1].to_string(), parts[2].to_string()))
            } else {
                None
            }
        })
        .collect())
}

/// Search for commits that added or removed a specific string (pickaxe search)
pub fn search_commits_by_content(
    repo_path: &Path,
    search_string: &str,
    limit: usize,
) -> Result<Vec<(String, String, String)>, String> {
    let output = run_git_checked(
        repo_path,
        &[
            "log",
            &format!("-{}", limit),
            &format!("-S{}", search_string),
            "--format=%h|%ci|%s",
        ],
    )?;

    Ok(output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(3, '|').collect();
            if parts.len() == 3 {
                Some((parts[0].to_string(), parts[1].to_string(), parts[2].to_string()))
            } else {
                None
            }
        })
        .collect())
}

fn main() {
    let repo = Path::new(".");

    // Find commits mentioning "authentication"
    match search_commits_by_message(repo, "authentication", 10) {
        Ok(commits) => {
            println!("Commits mentioning 'authentication':");
            for (hash, date, msg) in &commits {
                println!("  {} {} - {}", hash, date, msg);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    // Find when a specific function was introduced
    match search_commits_by_content(repo, "fn authenticate", 5) {
        Ok(commits) => {
            println!("\nCommits that added/removed 'fn authenticate':");
            for (hash, date, msg) in &commits {
                println!("  {} {} - {}", hash, date, msg);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

::: wild In the Wild
Claude Code uses blame data to understand code ownership when making edits. If a section of code was recently changed by a specific developer, and the agent needs to modify it further, this context helps the LLM understand whether the code is in a stable state or actively being developed. Some coding agents go further and include blame information directly in the system prompt for files the agent is about to edit, giving the LLM the full historical context to make informed modifications.
:::

## Key Takeaways

- Use git log with custom `--format` strings and a unique delimiter to parse structured commit data reliably.
- Git blame in porcelain mode attributes every line to a commit, author, and timestamp -- essential for understanding code provenance.
- Build a `FileHistory` type that summarizes a file's change frequency and recent modifications for LLM context injection.
- Use pickaxe search (`git log -S"string"`) to find when specific code was introduced or removed -- this answers "why does this exist?"
- Feed historical context into agent prompts to improve edit quality -- the LLM makes better decisions when it knows a file's story.
