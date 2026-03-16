---
title: Repo Analysis Techniques
description: Analyzing repository structure, history, and patterns to help the agent understand project conventions, hot files, and contributor patterns.
---

# Repo Analysis Techniques

> **What you'll learn:**
> - How to extract project structure insights from the Git tree: language distribution, directory organization, and configuration files
> - Using git log and blame to identify hot files (frequently changed), key contributors, and recent activity patterns
> - Building a repository profile that the agent uses to adapt its behavior to project conventions and coding standards

An agent that understands a repository beyond the current file is a fundamentally better collaborator. Repository analysis gives your agent context about the project's history, conventions, and structure. This context helps the agent make better decisions: it follows existing patterns, focuses on files that are actively developed, and respects the project's norms for commit messages, code organization, and testing.

## Mapping Repository Structure

The first step in understanding a repository is surveying its structure. Git's tree objects give you a fast way to enumerate every file without scanning the disk:

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Default)]
pub struct RepoProfile {
    pub total_files: usize,
    pub language_distribution: HashMap<String, usize>,
    pub top_level_dirs: Vec<String>,
    pub has_cargo_toml: bool,
    pub has_package_json: bool,
    pub has_ci_config: bool,
    pub has_tests: bool,
}

pub fn analyze_structure(repo_dir: &Path) -> Result<RepoProfile, String> {
    // List all tracked files from the index (fast, no disk scan)
    let output = Command::new("git")
        .args(["ls-files"])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to list files: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<&str> = stdout.lines().collect();

    let mut profile = RepoProfile {
        total_files: files.len(),
        ..Default::default()
    };

    let mut top_dirs = std::collections::HashSet::new();

    for file in &files {
        let path = PathBuf::from(file);

        // Collect language stats from file extensions
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let language = extension_to_language(ext);
            *profile.language_distribution
                .entry(language.to_string())
                .or_insert(0) += 1;
        }

        // Collect top-level directories
        if let Some(first_component) = path.components().next() {
            let dir_name = first_component.as_os_str().to_string_lossy().to_string();
            top_dirs.insert(dir_name);
        }

        // Check for specific files
        match *file {
            "Cargo.toml" => profile.has_cargo_toml = true,
            "package.json" => profile.has_package_json = true,
            _ => {}
        }

        // Check for CI configs
        if file.starts_with(".github/workflows/")
            || file.starts_with(".gitlab-ci")
            || file.starts_with(".circleci/")
        {
            profile.has_ci_config = true;
        }

        // Check for test directories
        if file.contains("/tests/") || file.contains("/test/") || file.ends_with("_test.rs") {
            profile.has_tests = true;
        }
    }

    profile.top_level_dirs = top_dirs.into_iter().collect();
    profile.top_level_dirs.sort();

    Ok(profile)
}

fn extension_to_language(ext: &str) -> &str {
    match ext {
        "rs" => "Rust",
        "py" => "Python",
        "js" | "jsx" => "JavaScript",
        "ts" | "tsx" => "TypeScript",
        "go" => "Go",
        "java" => "Java",
        "rb" => "Ruby",
        "toml" => "TOML",
        "yaml" | "yml" => "YAML",
        "json" => "JSON",
        "md" => "Markdown",
        "sh" | "bash" => "Shell",
        _ => "Other",
    }
}
```

::: python Coming from Python
In Python, you might use `os.walk()` or `pathlib.Path.rglob()` to scan a directory tree. The Git-based approach with `git ls-files` is faster for large repositories because it reads from the index (a single binary file) rather than traversing the filesystem. It also automatically excludes files in `.gitignore`, which is exactly what you want when analyzing the project structure.
:::

## Identifying Hot Files

Hot files are files that change frequently. They represent the active development areas and are where the agent is most likely to need to make changes or understand context:

```rust
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct HotFile {
    pub path: String,
    pub commit_count: usize,
    pub last_modified: String,
}

pub fn find_hot_files(
    repo_dir: &Path,
    days: u32,
    limit: usize,
) -> Result<Vec<HotFile>, String> {
    let since = format!("--since={} days ago", days);

    // Count commits per file in the given time period
    let output = Command::new("git")
        .args([
            "log", &since, "--format=", "--name-only",
            "--diff-filter=AMRC",
        ])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to run git log: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut file_counts: HashMap<String, usize> = HashMap::new();

    for line in stdout.lines() {
        let line = line.trim();
        if !line.is_empty() {
            *file_counts.entry(line.to_string()).or_insert(0) += 1;
        }
    }

    // Sort by commit count (descending)
    let mut hot_files: Vec<(String, usize)> = file_counts.into_iter().collect();
    hot_files.sort_by(|a, b| b.1.cmp(&a.1));
    hot_files.truncate(limit);

    // Enrich with last modified date
    let mut result = Vec::new();
    for (path, count) in hot_files {
        let date_output = Command::new("git")
            .args(["log", "-1", "--format=%ai", "--", &path])
            .current_dir(repo_dir)
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        result.push(HotFile {
            path,
            commit_count: count,
            last_modified: date_output,
        });
    }

    Ok(result)
}

use std::collections::HashMap;
```

## Git Blame for Ownership Insights

`git blame` tells you who last modified each line of a file. This helps the agent understand code ownership and identify who to attribute questions or changes to:

```rust
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct BlameInfo {
    pub line_number: usize,
    pub commit_hash: String,
    pub author: String,
    pub date: String,
    pub content: String,
}

pub fn blame_file(
    repo_dir: &Path,
    file_path: &str,
) -> Result<Vec<BlameInfo>, String> {
    let output = Command::new("git")
        .args([
            "blame", "--porcelain", file_path,
        ])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to run git blame: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();
    let mut current_hash = String::new();
    let mut current_author = String::new();
    let mut current_date = String::new();
    let mut line_number = 0usize;

    for line in stdout.lines() {
        if line.len() >= 40 && line.chars().take(40).all(|c| c.is_ascii_hexdigit()) {
            // This is a commit header line: <hash> <orig_line> <final_line> [<num_lines>]
            let parts: Vec<&str> = line.split_whitespace().collect();
            current_hash = parts[0][..8].to_string();
            if parts.len() >= 3 {
                line_number = parts[2].parse().unwrap_or(0);
            }
        } else if let Some(author) = line.strip_prefix("author ") {
            current_author = author.to_string();
        } else if let Some(date) = line.strip_prefix("author-time ") {
            current_date = date.to_string();
        } else if let Some(content) = line.strip_prefix('\t') {
            entries.push(BlameInfo {
                line_number,
                commit_hash: current_hash.clone(),
                author: current_author.clone(),
                date: current_date.clone(),
                content: content.to_string(),
            });
        }
    }

    Ok(entries)
}

/// Summarize file ownership by author
pub fn file_ownership_summary(
    blame_entries: &[BlameInfo],
) -> Vec<(String, usize, f64)> {
    let total_lines = blame_entries.len();
    let mut author_lines: HashMap<String, usize> = HashMap::new();

    for entry in blame_entries {
        *author_lines.entry(entry.author.clone()).or_insert(0) += 1;
    }

    let mut ownership: Vec<(String, usize, f64)> = author_lines.into_iter()
        .map(|(author, lines)| {
            let percentage = (lines as f64 / total_lines as f64) * 100.0;
            (author, lines, percentage)
        })
        .collect();

    ownership.sort_by(|a, b| b.1.cmp(&a.1));
    ownership
}
```

## Commit Message Patterns

Understanding a project's commit message conventions helps the agent generate messages that fit in:

```rust
use std::path::Path;
use std::process::Command;
use std::collections::HashMap;

#[derive(Debug)]
pub struct CommitConventions {
    pub uses_conventional_commits: bool,
    pub common_prefixes: Vec<(String, usize)>,
    pub average_message_length: usize,
    pub uses_issue_references: bool,
}

pub fn analyze_commit_conventions(
    repo_dir: &Path,
    sample_size: usize,
) -> Result<CommitConventions, String> {
    let count = format!("-{}", sample_size);
    let output = Command::new("git")
        .args(["log", &count, "--format=%s"])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to read commit log: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let messages: Vec<&str> = stdout.lines().collect();

    if messages.is_empty() {
        return Err("No commits found".to_string());
    }

    // Check for conventional commit prefixes
    let conventional_prefixes = [
        "feat", "fix", "docs", "style", "refactor",
        "test", "chore", "perf", "ci", "build",
    ];
    let mut prefix_counts: HashMap<String, usize> = HashMap::new();
    let mut conventional_count = 0;
    let mut issue_ref_count = 0;
    let total_length: usize = messages.iter().map(|m| m.len()).sum();

    for msg in &messages {
        // Check for conventional commit format: "type: description" or "type(scope): description"
        for prefix in &conventional_prefixes {
            if msg.starts_with(&format!("{}: ", prefix))
                || msg.starts_with(&format!("{}(", prefix))
            {
                conventional_count += 1;
                *prefix_counts.entry(prefix.to_string()).or_insert(0) += 1;
                break;
            }
        }

        // Check for issue references (#123, JIRA-456)
        if msg.contains('#') || msg.contains("JIRA-") || msg.contains("ISSUE-") {
            issue_ref_count += 1;
        }
    }

    let mut common_prefixes: Vec<(String, usize)> = prefix_counts.into_iter().collect();
    common_prefixes.sort_by(|a, b| b.1.cmp(&a.1));

    Ok(CommitConventions {
        uses_conventional_commits: conventional_count as f64 / messages.len() as f64 > 0.5,
        common_prefixes,
        average_message_length: total_length / messages.len(),
        uses_issue_references: issue_ref_count as f64 / messages.len() as f64 > 0.2,
    })
}
```

::: wild In the Wild
Claude Code analyzes the repository context when generating commit messages and code changes. It examines existing code patterns, import styles, and naming conventions in the files it reads to produce changes that match the project's style. This "convention following" behavior comes from feeding repository context into the LLM prompt, not from explicit rule configuration. The git log and blame techniques covered here are the foundation for building that context automatically.
:::

## Git Bisect for Bug Localization

`git bisect` performs a binary search through commit history to find the commit that introduced a bug. For an agent, this is a powerful tool for automated bug localization:

```rust
use std::path::Path;
use std::process::Command;

pub struct BisectSession {
    repo_dir: std::path::PathBuf,
}

impl BisectSession {
    pub fn new(repo_dir: &Path) -> Self {
        Self { repo_dir: repo_dir.to_path_buf() }
    }

    /// Start a bisect session between a known good and bad commit
    pub fn start(&self, good_commit: &str, bad_commit: &str) -> Result<String, String> {
        self.run_git(&["bisect", "start"])?;
        self.run_git(&["bisect", "bad", bad_commit])?;
        self.run_git(&["bisect", "good", good_commit])
    }

    /// Run bisect with an automated test command
    /// The command should exit 0 for "good" and non-zero for "bad"
    pub fn run_automated(
        &self,
        good_commit: &str,
        bad_commit: &str,
        test_command: &str,
    ) -> Result<String, String> {
        self.run_git(&["bisect", "start", bad_commit, good_commit])?;
        let result = self.run_git(&["bisect", "run", "sh", "-c", test_command]);

        // Always reset bisect state, even on error
        let _ = self.run_git(&["bisect", "reset"]);

        result
    }

    /// Mark the current commit as good or bad
    pub fn mark(&self, is_good: bool) -> Result<String, String> {
        let label = if is_good { "good" } else { "bad" };
        self.run_git(&["bisect", "mark", label])
    }

    /// End the bisect session and return to the original state
    pub fn reset(&self) -> Result<(), String> {
        self.run_git(&["bisect", "reset"]).map(|_| ())
    }

    fn run_git(&self, args: &[&str]) -> Result<String, String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_dir)
            .output()
            .map_err(|e| format!("Failed to run git: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}
```

## Building a Repository Profile

Combine all analysis techniques into a single profile that your agent can reference throughout a session:

```rust
use std::path::Path;

pub fn build_repo_profile(repo_dir: &Path) -> Result<String, String> {
    let structure = analyze_structure(repo_dir)?;
    let hot_files = find_hot_files(repo_dir, 30, 10)?;
    let conventions = analyze_commit_conventions(repo_dir, 50)?;

    let mut profile = String::new();

    profile.push_str("# Repository Profile\n\n");
    profile.push_str(&format!("Total tracked files: {}\n", structure.total_files));
    profile.push_str(&format!("Project type: {}\n",
        if structure.has_cargo_toml { "Rust (Cargo)" }
        else if structure.has_package_json { "JavaScript/TypeScript (npm)" }
        else { "Unknown" }));
    profile.push_str(&format!("Has CI: {}\n", structure.has_ci_config));
    profile.push_str(&format!("Has tests: {}\n\n", structure.has_tests));

    profile.push_str("Language distribution:\n");
    let mut languages: Vec<_> = structure.language_distribution.iter().collect();
    languages.sort_by(|a, b| b.1.cmp(a.1));
    for (lang, count) in languages.iter().take(5) {
        profile.push_str(&format!("  {} -- {} files\n", lang, count));
    }

    if !hot_files.is_empty() {
        profile.push_str("\nMost actively changed files (last 30 days):\n");
        for file in hot_files.iter().take(5) {
            profile.push_str(&format!("  {} ({} commits)\n",
                file.path, file.commit_count));
        }
    }

    if conventions.uses_conventional_commits {
        profile.push_str("\nCommit style: Conventional Commits\n");
    }

    Ok(profile)
}
```

## Key Takeaways

- Use `git ls-files` to enumerate repository contents quickly from the index, avoiding expensive filesystem traversal and automatically respecting `.gitignore`.
- Hot file analysis (`git log --name-only`) identifies the most actively developed parts of the codebase, helping the agent prioritize its understanding of the code.
- `git blame --porcelain` provides per-line authorship data that helps the agent understand code ownership and adapt its communication accordingly.
- Commit convention analysis lets the agent match existing project norms for commit messages, improving the quality and consistency of agent-generated commits.
- Combine structure, activity, blame, and convention analysis into a repository profile that serves as persistent context for the agent session.
