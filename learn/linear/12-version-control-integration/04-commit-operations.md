---
title: Commit Operations
description: Creating commits programmatically — staging files, writing tree objects, constructing commit messages, and handling author/committer identity configuration.
---

# Commit Operations

> **What you'll learn:**
> - The sequence of operations to create a commit: stage files to the index, write the index as a tree, create the commit object with parent and message
> - Handling author and committer identity: reading from git config, environment variables, and providing agent-specific attribution
> - Best practices for agent-generated commit messages: summarizing changes, referencing tool actions, and maintaining conventional commit formats

Commit creation is the most consequential Git operation your agent performs. Every other operation -- status, diff, branch -- is read-only. Committing writes to the permanent record. Getting it right means your agent produces clean, attributable history that developers trust. Getting it wrong means confusing commit messages, broken authorship, or accidental inclusion of files that should not be committed.

## The Commit Sequence

Creating a commit involves three distinct steps that mirror the object model you learned earlier:

1. **Stage files** -- copy changes from the working tree into the index
2. **Write the tree** -- serialize the index into a tree object
3. **Create the commit** -- link the tree to a parent commit with a message

### Staging Files via CLI

The simplest approach stages files by running `git add`:

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct CommitBuilder {
    repo_dir: PathBuf,
    files_to_stage: Vec<PathBuf>,
    message: Option<String>,
}

impl CommitBuilder {
    pub fn new(repo_dir: impl Into<PathBuf>) -> Self {
        Self {
            repo_dir: repo_dir.into(),
            files_to_stage: Vec::new(),
            message: None,
        }
    }

    /// Stage specific files for commit
    pub fn add_file(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.files_to_stage.push(path.into());
        self
    }

    /// Stage all modified and new files
    pub fn add_all(&mut self) -> &mut Self {
        self.files_to_stage.clear();
        self.files_to_stage.push(PathBuf::from("."));
        self
    }

    pub fn message(&mut self, msg: impl Into<String>) -> &mut Self {
        self.message = Some(msg.into());
        self
    }

    pub fn execute(&self) -> Result<String, String> {
        // Step 1: Stage files
        for file in &self.files_to_stage {
            let file_str = file.to_string_lossy();
            self.run_git(&["add", &file_str])?;
        }

        // Step 2 & 3: Create commit (git commit internally writes
        // the tree and creates the commit object)
        let message = self.message.as_deref()
            .ok_or_else(|| "No commit message provided".to_string())?;

        let output = self.run_git(&["commit", "-m", message])?;
        Ok(output)
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

### Staging and Committing via git2

For operations that need to be atomic or avoid subprocess overhead, use `git2` to stage and commit directly:

```rust
use git2::{Repository, Signature, IndexAddOption};
use std::path::Path;

fn commit_with_git2(
    repo_path: &Path,
    paths: &[&str],
    message: &str,
) -> Result<String, git2::Error> {
    let repo = Repository::discover(repo_path)?;

    // Step 1: Stage files into the index
    let mut index = repo.index()?;
    for path in paths {
        index.add_path(Path::new(path))?;
    }
    index.write()?;

    // Step 2: Write the index as a tree
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    // Step 3: Get the parent commit (HEAD)
    let parent_commit = repo.head()?.peel_to_commit()?;

    // Step 4: Create the signature (author/committer)
    let signature = repo.signature()?; // reads from git config

    // Step 5: Create the commit
    let commit_oid = repo.commit(
        Some("HEAD"),       // update HEAD to point to new commit
        &signature,         // author
        &signature,         // committer
        message,            // commit message
        &tree,              // tree object
        &[&parent_commit],  // parent commits
    )?;

    Ok(commit_oid.to_string())
}
```

::: python Coming from Python
In Python with `GitPython`, you would stage and commit with `repo.index.add(files)` followed by `repo.index.commit("message")`. The Rust `git2` approach is more explicit: you manually write the index, create the tree, and construct the commit with explicit parent and signature. This verbosity is actually an advantage for an agent -- each step is visible and can be error-handled independently.
:::

## Handling Author Identity

When your agent creates commits, the author and committer fields need careful attention. There are several sources of identity, and your agent should check them in order:

```rust
use std::env;
use std::process::Command;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct GitIdentity {
    pub name: String,
    pub email: String,
}

impl GitIdentity {
    /// Resolve the identity Git would use for commits in the given repo
    pub fn resolve(repo_dir: &Path) -> Result<Self, String> {
        // Priority 1: Environment variables (GIT_AUTHOR_NAME, GIT_AUTHOR_EMAIL)
        if let (Ok(name), Ok(email)) = (
            env::var("GIT_AUTHOR_NAME"),
            env::var("GIT_AUTHOR_EMAIL"),
        ) {
            return Ok(Self { name, email });
        }

        // Priority 2: Local git config (repo-level .git/config)
        if let Ok(name) = git_config(repo_dir, "user.name") {
            if let Ok(email) = git_config(repo_dir, "user.email") {
                return Ok(Self { name, email });
            }
        }

        // Priority 3: Global git config (~/.gitconfig)
        if let Ok(name) = git_config_global("user.name") {
            if let Ok(email) = git_config_global("user.email") {
                return Ok(Self { name, email });
            }
        }

        Err("No git identity configured. Set user.name and user.email in git config.".to_string())
    }
}

fn git_config(repo_dir: &Path, key: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args(["config", "--local", key])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err("Not set".to_string())
    }
}

fn git_config_global(key: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args(["config", "--global", key])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err("Not set".to_string())
    }
}
```

### Agent Attribution in Commit Messages

A common pattern is to include a trailer line that identifies the agent as a co-author or tool, while preserving the human user as the primary author:

```rust
pub fn format_agent_commit_message(
    summary: &str,
    body: Option<&str>,
    agent_name: &str,
    agent_email: &str,
) -> String {
    let mut message = summary.to_string();

    if let Some(body_text) = body {
        message.push_str("\n\n");
        message.push_str(body_text);
    }

    // Add co-author trailer for agent attribution
    message.push_str(&format!(
        "\n\nCo-Authored-By: {} <{}>",
        agent_name, agent_email
    ));

    message
}
```

::: wild In the Wild
Claude Code uses the `Co-Authored-By` trailer to attribute commits it helps create. The human user remains the author (from their git config), and Claude is listed as a co-author. This preserves the human-in-the-loop model: the user is responsible for the commit, but the tooling that helped produce it is transparently recorded. This pattern is also recognized by GitHub, which displays co-authors in the commit UI.
:::

## Selective Staging

Agents often modify multiple files but should only commit a subset. Selective staging requires knowing which files belong to the current task:

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

/// Stage only files that were modified by the agent's current task
pub fn stage_task_files(
    repo_dir: &Path,
    task_files: &[PathBuf],
) -> Result<Vec<PathBuf>, String> {
    let mut staged = Vec::new();

    // Get the list of files with changes
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to run git status: {}", e))?;

    let status_output = String::from_utf8_lossy(&output.stdout);
    let changed_files: Vec<PathBuf> = status_output.lines()
        .filter_map(|line| {
            if line.len() >= 4 {
                Some(PathBuf::from(line[3..].trim()))
            } else {
                None
            }
        })
        .collect();

    // Only stage files that are both changed and in the task list
    for file in task_files {
        if changed_files.iter().any(|f| f == file) {
            let file_str = file.to_string_lossy();
            Command::new("git")
                .args(["add", &file_str])
                .current_dir(repo_dir)
                .output()
                .map_err(|e| format!("Failed to stage {}: {}", file_str, e))?;
            staged.push(file.clone());
        }
    }

    Ok(staged)
}
```

## Generating Commit Messages

Good commit messages are essential for agent credibility. Here is a pattern that uses the diff summary to generate structured messages:

```rust
use std::path::Path;
use std::process::Command;

pub struct CommitMessageGenerator;

impl CommitMessageGenerator {
    /// Generate a conventional-style commit message from staged changes
    pub fn from_staged_diff(repo_dir: &Path) -> Result<String, String> {
        let diff = Self::get_staged_diff(repo_dir)?;
        let stats = Self::get_staged_stats(repo_dir)?;

        // For simple changes, the agent can construct a message directly
        // For complex changes, feed the diff to the LLM
        if stats.files_changed <= 3 && stats.total_lines_changed <= 20 {
            Ok(Self::generate_simple_message(&stats))
        } else {
            // Return the diff for the LLM to summarize
            Err(format!("Complex change -- feed to LLM:\n{}", diff))
        }
    }

    fn get_staged_diff(repo_dir: &Path) -> Result<String, String> {
        let output = Command::new("git")
            .args(["diff", "--cached"])
            .current_dir(repo_dir)
            .output()
            .map_err(|e| format!("git diff failed: {}", e))?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn get_staged_stats(repo_dir: &Path) -> Result<DiffStats, String> {
        let output = Command::new("git")
            .args(["diff", "--cached", "--stat"])
            .current_dir(repo_dir)
            .output()
            .map_err(|e| format!("git diff --stat failed: {}", e))?;

        let stat_text = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stat_text.lines().collect();

        // The last line of --stat output is "N files changed, N insertions(+), N deletions(-)"
        let summary = lines.last().unwrap_or(&"");
        let files_changed = extract_number(summary, "file");
        let insertions = extract_number(summary, "insertion");
        let deletions = extract_number(summary, "deletion");

        Ok(DiffStats {
            files_changed,
            insertions,
            deletions,
            total_lines_changed: insertions + deletions,
        })
    }

    fn generate_simple_message(stats: &DiffStats) -> String {
        if stats.files_changed == 1 {
            format!("Update {} file ({} lines changed)",
                stats.files_changed, stats.total_lines_changed)
        } else {
            format!("Update {} files ({} lines changed)",
                stats.files_changed, stats.total_lines_changed)
        }
    }
}

struct DiffStats {
    files_changed: usize,
    insertions: usize,
    deletions: usize,
    total_lines_changed: usize,
}

fn extract_number(text: &str, keyword: &str) -> usize {
    // Find "N keyword" pattern in the stat summary line
    for word_pair in text.split(", ") {
        if word_pair.contains(keyword) {
            if let Some(num_str) = word_pair.trim().split_whitespace().next() {
                return num_str.parse().unwrap_or(0);
            }
        }
    }
    0
}
```

## Handling Empty Commits and Edge Cases

Your agent must handle several edge cases that arise in practice:

```rust
use std::path::Path;
use std::process::Command;

/// Check if there is anything to commit before attempting
pub fn has_staged_changes(repo_dir: &Path) -> Result<bool, String> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to check staged changes: {}", e))?;

    // Exit code 1 means there are differences (changes staged)
    // Exit code 0 means no differences (nothing staged)
    Ok(!output.status.success())
}

/// Commit with proper error handling for common failures
pub fn safe_commit(repo_dir: &Path, message: &str) -> Result<String, String> {
    // Check for staged changes first
    if !has_staged_changes(repo_dir)? {
        return Err("Nothing to commit -- no staged changes".to_string());
    }

    let output = Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to run git commit: {}", e))?;

    if output.status.success() {
        // Extract the commit hash from the output
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Check for common failure modes
        if stderr.contains("nothing to commit") {
            Err("Nothing to commit".to_string())
        } else if stderr.contains("Please tell me who you are") {
            Err("Git identity not configured. Run: git config user.name 'name' && git config user.email 'email'".to_string())
        } else if stderr.contains("pre-commit hook") {
            Err(format!("Pre-commit hook failed:\n{}", stderr))
        } else {
            Err(format!("Commit failed: {}", stderr))
        }
    }
}
```

## Key Takeaways

- Committing is a three-step process (stage, write tree, create commit) -- using `git2` makes each step explicit, while the CLI bundles them into `git commit`.
- Always resolve author identity from environment variables, local config, then global config -- and provide clear error messages when no identity is found.
- Use `Co-Authored-By` trailers to transparently attribute agent involvement while keeping the human user as the primary commit author.
- Stage files selectively based on the current task rather than using `git add -A`, which risks committing unrelated changes or sensitive files.
- Always check for staged changes before committing, and handle common failures (empty commits, missing identity, hook failures) with informative error messages.
