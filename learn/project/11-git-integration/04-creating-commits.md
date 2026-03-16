---
title: Creating Commits
description: Building a commit tool that stages specific files, generates meaningful commit messages, handles co-authorship attribution, and follows conventional commit patterns.
---

# Creating Commits

> **What you'll learn:**
> - How to stage files selectively and create atomic commits from agent workflows
> - Techniques for generating descriptive commit messages from change analysis
> - How to handle co-authorship trailers and conventional commit formatting

Commits are the agent's save points. Every time the agent completes a logical unit of work -- fixing a bug, adding a function, refactoring a module -- it should create a commit. This gives the user a clear history of what the agent did and, critically, the ability to revert any individual change. In this subchapter, you will build a commit tool that stages files precisely, generates informative messages, and follows the conventions of the project.

## Selective Staging

The most common mistake in automated git workflows is staging everything with `git add -A`. This catches temporary files, debug output, and changes the agent did not intend to make. Your agent should stage only the files it deliberately modified:

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

/// Stage specific files for commit
pub fn stage_files(repo_path: &Path, files: &[&str]) -> Result<Vec<String>, String> {
    let mut staged = Vec::new();
    let mut errors = Vec::new();

    for file in files {
        match run_git_checked(repo_path, &["add", "--", file]) {
            Ok(_) => staged.push(file.to_string()),
            Err(e) => errors.push(format!("{}: {}", file, e)),
        }
    }

    if !errors.is_empty() && staged.is_empty() {
        return Err(format!("Failed to stage any files:\n{}", errors.join("\n")));
    }

    if !errors.is_empty() {
        // Partial success -- some files staged, some failed
        eprintln!("Warning: could not stage some files:\n{}", errors.join("\n"));
    }

    Ok(staged)
}

/// Stage all modifications to already-tracked files (safer than git add -A)
pub fn stage_tracked_changes(repo_path: &Path) -> Result<String, String> {
    run_git_checked(repo_path, &["add", "--update"])
}

/// Verify what is currently staged before committing
pub fn verify_staging(repo_path: &Path) -> Result<Vec<String>, String> {
    let output = run_git_checked(repo_path, &["diff", "--cached", "--name-only"])?;
    let files: Vec<String> = output.lines().map(String::from).collect();
    Ok(files)
}

fn main() {
    let repo = Path::new(".");

    // Stage specific files the agent modified
    let files_to_stage = vec!["src/tools/git.rs", "src/main.rs"];
    match stage_files(repo, &files_to_stage) {
        Ok(staged) => {
            println!("Staged {} files:", staged.len());
            for f in &staged {
                println!("  {}", f);
            }
        }
        Err(e) => eprintln!("Staging error: {}", e),
    }

    // Double-check what is staged
    match verify_staging(repo) {
        Ok(files) => {
            println!("Currently staged for commit:");
            for f in &files {
                println!("  {}", f);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

::: python Coming from Python
In Python you would call `subprocess.run(["git", "add", "--", "file.py"])` for each file. The Rust version follows the same pattern but wraps it in a function that collects successes and failures separately. This is a common Rust pattern: rather than raising an exception on the first failure, you process everything and report partial results. The type system makes it natural to return `Result<Vec<String>, String>` where the `Ok` variant contains the files that were staged successfully.
:::

## Creating Commits with Messages

Once files are staged, creating the commit itself is straightforward. The interesting part is generating a good commit message. Your agent can analyze the staged diff to produce a descriptive message:

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
pub struct CommitResult {
    pub hash: String,
    pub message: String,
    pub files_changed: usize,
}

/// Create a commit with the currently staged changes
pub fn create_commit(
    repo_path: &Path,
    message: &str,
    author: Option<&str>,
) -> Result<CommitResult, String> {
    // Verify something is staged
    let staged = run_git_checked(repo_path, &["diff", "--cached", "--name-only"])?;
    if staged.is_empty() {
        return Err("Nothing staged for commit. Use stage_files() first.".to_string());
    }

    let files_changed = staged.lines().count();

    // Build the commit command
    let mut args = vec!["commit", "-m", message];

    // If an author is specified (for co-authorship), add it
    if let Some(author_str) = author {
        args.push("--author");
        args.push(author_str);
    }

    run_git_checked(repo_path, &args)?;

    // Get the hash of the commit we just created
    let hash = run_git_checked(repo_path, &["rev-parse", "--short", "HEAD"])?;

    Ok(CommitResult {
        hash,
        message: message.to_string(),
        files_changed,
    })
}

fn main() {
    let repo = Path::new(".");

    match create_commit(repo, "feat: add git status tool to agent", None) {
        Ok(result) => {
            println!("Created commit {} ({} files changed)", result.hash, result.files_changed);
            println!("Message: {}", result.message);
        }
        Err(e) => eprintln!("Commit failed: {}", e),
    }
}
```

## Generating Commit Messages from Diffs

Rather than requiring the LLM to generate a commit message (which costs tokens and latency), you can build a heuristic message generator that analyzes the staged diff:

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

/// Analyze staged changes and generate a descriptive commit message
pub fn generate_commit_message(repo_path: &Path) -> Result<String, String> {
    // Get the list of changed files with their status
    let stat = run_git_checked(repo_path, &["diff", "--cached", "--numstat"])?;
    let name_status = run_git_checked(repo_path, &["diff", "--cached", "--name-status"])?;

    let mut added_files = Vec::new();
    let mut modified_files = Vec::new();
    let mut deleted_files = Vec::new();
    let mut total_insertions: usize = 0;
    let mut total_deletions: usize = 0;

    // Parse numstat for line counts
    for line in stat.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() == 3 {
            total_insertions += parts[0].parse::<usize>().unwrap_or(0);
            total_deletions += parts[1].parse::<usize>().unwrap_or(0);
        }
    }

    // Parse name-status for file operations
    for line in name_status.lines() {
        let parts: Vec<&str> = line.splitn(2, '\t').collect();
        if parts.len() == 2 {
            match parts[0] {
                "A" => added_files.push(parts[1]),
                "M" => modified_files.push(parts[1]),
                "D" => deleted_files.push(parts[1]),
                _ => modified_files.push(parts[1]),
            }
        }
    }

    // Determine the primary action
    let action = if !added_files.is_empty() && modified_files.is_empty() && deleted_files.is_empty()
    {
        "add"
    } else if !deleted_files.is_empty() && added_files.is_empty() && modified_files.is_empty() {
        "remove"
    } else if !modified_files.is_empty() && added_files.is_empty() && deleted_files.is_empty() {
        if total_deletions > total_insertions * 2 {
            "refactor"
        } else {
            "update"
        }
    } else {
        "update"
    };

    // Identify the primary area being changed
    let all_files: Vec<&str> = added_files
        .iter()
        .chain(modified_files.iter())
        .chain(deleted_files.iter())
        .copied()
        .collect();

    let area = identify_change_area(&all_files);

    // Build the message
    let total_files = all_files.len();
    let summary = if total_files == 1 {
        format!("{}: {} {}", action, area, all_files[0])
    } else {
        format!("{}: {} ({} files, +{} -{})", action, area, total_files, total_insertions, total_deletions)
    };

    Ok(summary)
}

/// Identify the area of the codebase being changed based on file paths
fn identify_change_area(files: &[&str]) -> String {
    if files.is_empty() {
        return "files".to_string();
    }

    // Find common directory prefix
    let first_parts: Vec<&str> = files[0].split('/').collect();

    for depth in (1..first_parts.len()).rev() {
        let prefix: String = first_parts[..depth].join("/");
        if files.iter().all(|f| f.starts_with(&prefix)) {
            return prefix;
        }
    }

    // No common prefix -- describe by file type or just say "multiple areas"
    if files.iter().all(|f| f.ends_with(".rs")) {
        "Rust source".to_string()
    } else if files.iter().all(|f| f.ends_with(".md")) {
        "documentation".to_string()
    } else {
        "multiple areas".to_string()
    }
}

fn main() {
    let repo = Path::new(".");

    match generate_commit_message(repo) {
        Ok(msg) => println!("Generated message: {}", msg),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Co-authorship and Attribution

When an agent creates commits, it is important to attribute the work properly. The conventional approach uses git trailers to indicate co-authorship:

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

/// Create a commit with co-authorship trailer
pub fn create_agent_commit(
    repo_path: &Path,
    summary: &str,
    agent_name: &str,
    agent_email: &str,
) -> Result<String, String> {
    // Build the full commit message with trailer
    let full_message = format!(
        "{}\n\nCo-Authored-By: {} <{}>",
        summary, agent_name, agent_email
    );

    run_git_checked(repo_path, &["commit", "-m", &full_message])?;
    let hash = run_git_checked(repo_path, &["rev-parse", "--short", "HEAD"])?;
    Ok(hash)
}

/// Create a conventional commit message
pub fn conventional_commit(
    commit_type: &str,   // feat, fix, refactor, docs, test, chore
    scope: Option<&str>, // optional scope like "auth", "api"
    description: &str,
    breaking: bool,
) -> String {
    let breaking_marker = if breaking { "!" } else { "" };

    match scope {
        Some(s) => format!("{}({}){}: {}", commit_type, s, breaking_marker, description),
        None => format!("{}{}: {}", commit_type, breaking_marker, description),
    }
}

fn main() {
    // Generate conventional commit messages
    let msg = conventional_commit("feat", Some("git"), "add status and diff tools", false);
    println!("{}", msg);
    // Output: feat(git): add status and diff tools

    let msg = conventional_commit("fix", None, "handle empty repository gracefully", false);
    println!("{}", msg);
    // Output: fix: handle empty repository gracefully

    let msg = conventional_commit("refactor", Some("tools"), "unify error handling", true);
    println!("{}", msg);
    // Output: refactor(tools)!: unify error handling
}
```

::: wild In the Wild
Claude Code adds a `Co-Authored-By` trailer to every commit it creates, making it immediately visible in `git log` that an AI agent contributed to the change. This transparency is important for code review -- reviewers know which commits were machine-generated and may warrant closer inspection. The trailer format is a standard GitHub convention that platforms recognize and display in the commit UI.
:::

## Atomic Commits During Multi-Step Tasks

When the agent performs a complex task like "add error handling to all API endpoints," it should create multiple small commits rather than one giant commit. This makes it easy to revert individual steps:

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

/// A checkpoint represents a commit the agent can roll back to
#[derive(Debug, Clone)]
pub struct Checkpoint {
    pub hash: String,
    pub description: String,
}

/// Manages a series of atomic commits during a multi-step task
pub struct CommitSequence {
    repo_path: std::path::PathBuf,
    checkpoints: Vec<Checkpoint>,
    base_commit: String,
}

impl CommitSequence {
    pub fn new(repo_path: &Path) -> Result<Self, String> {
        let base = run_git_checked(repo_path, &["rev-parse", "HEAD"])?;
        Ok(Self {
            repo_path: repo_path.to_path_buf(),
            checkpoints: Vec::new(),
            base_commit: base,
        })
    }

    /// Stage files and create a checkpoint commit
    pub fn checkpoint(&mut self, files: &[&str], message: &str) -> Result<&Checkpoint, String> {
        // Stage the specified files
        for file in files {
            run_git_checked(&self.repo_path, &["add", "--", file])?;
        }

        // Create the commit
        run_git_checked(&self.repo_path, &["commit", "-m", message])?;
        let hash = run_git_checked(&self.repo_path, &["rev-parse", "--short", "HEAD"])?;

        self.checkpoints.push(Checkpoint {
            hash,
            description: message.to_string(),
        });

        Ok(self.checkpoints.last().unwrap())
    }

    /// Roll back to a specific checkpoint (or to before the sequence started)
    pub fn rollback_to(&self, checkpoint_index: Option<usize>) -> Result<String, String> {
        let target = match checkpoint_index {
            Some(idx) => {
                let cp = self.checkpoints.get(idx)
                    .ok_or_else(|| format!("Checkpoint {} does not exist", idx))?;
                cp.hash.clone()
            }
            None => self.base_commit.clone(), // Roll back to before the sequence
        };

        // Use git reset --soft to preserve changes in the working tree
        run_git_checked(&self.repo_path, &["reset", "--soft", &target])?;
        Ok(format!("Rolled back to {}", target))
    }

    /// Get all checkpoints created during this sequence
    pub fn history(&self) -> &[Checkpoint] {
        &self.checkpoints
    }
}

fn main() {
    let repo = Path::new(".");

    match CommitSequence::new(repo) {
        Ok(mut seq) => {
            println!("Starting multi-step task...");

            // Each step creates a checkpoint
            // (In real usage, the agent would modify files between checkpoints)
            match seq.checkpoint(&["src/api/users.rs"], "feat(api): add error handling to user endpoints") {
                Ok(cp) => println!("Checkpoint 1: {} - {}", cp.hash, cp.description),
                Err(e) => eprintln!("Error: {}", e),
            }

            println!("Checkpoints: {:?}", seq.history());
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Key Takeaways

- Always stage files selectively with `git add -- <path>` rather than `git add -A` -- the agent should only commit files it intentionally modified.
- Verify the staging area with `git diff --cached --name-only` before creating a commit to ensure no unexpected files are included.
- Generate commit messages from diff analysis when possible -- it is faster and cheaper than asking the LLM, and the heuristic messages are often sufficient.
- Add `Co-Authored-By` trailers to agent-created commits for transparency in code review.
- Use a checkpoint pattern for multi-step tasks so each logical unit of work is a separate, revertable commit.
