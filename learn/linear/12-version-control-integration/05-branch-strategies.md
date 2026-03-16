---
title: Branch Strategies
description: Creating and managing branches for agent work — feature branches, stacking strategies, naming conventions, and branch lifecycle management.
---

# Branch Strategies

> **What you'll learn:**
> - How to create feature branches for agent work that isolate changes and enable clean review and rollback
> - Branch naming conventions that encode task context (agent/task-description) for easy identification in branch lists
> - Managing branch lifecycle: creation from the correct base, periodic rebasing to stay current, and cleanup after merge

When an agent modifies code, it should rarely commit directly to `main`. Feature branches provide isolation: the agent works on a branch, the user reviews the changes, and only merged branches become permanent history. This pattern is not just a best practice borrowed from human workflows -- it is a safety mechanism that lets users discard entire agent work sessions with a single `git branch -D`.

## Creating Branches Programmatically

Branch creation is one of Git's cheapest operations -- it just writes a 40-character SHA-1 to a file. Your agent should create branches liberally.

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct BranchManager {
    repo_dir: PathBuf,
}

impl BranchManager {
    pub fn new(repo_dir: impl Into<PathBuf>) -> Self {
        Self { repo_dir: repo_dir.into() }
    }

    /// Create a new branch from the current HEAD and check it out
    pub fn create_and_checkout(&self, branch_name: &str) -> Result<(), String> {
        self.run_git(&["checkout", "-b", branch_name])
            .map(|_| ())
    }

    /// Create a new branch from a specific base without checking it out
    pub fn create_from_base(
        &self,
        branch_name: &str,
        base: &str,
    ) -> Result<(), String> {
        self.run_git(&["branch", branch_name, base])
            .map(|_| ())
    }

    /// List all local branches
    pub fn list_branches(&self) -> Result<Vec<BranchInfo>, String> {
        let output = self.run_git(&[
            "branch", "--format=%(refname:short) %(objectname:short) %(upstream:short)"
        ])?;

        let current = self.current_branch()?;

        Ok(output.lines()
            .filter(|l| !l.is_empty())
            .map(|line| {
                let parts: Vec<&str> = line.splitn(3, ' ').collect();
                BranchInfo {
                    name: parts[0].to_string(),
                    short_hash: parts.get(1).unwrap_or(&"").to_string(),
                    upstream: parts.get(2)
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string()),
                    is_current: parts[0] == current,
                }
            })
            .collect())
    }

    pub fn current_branch(&self) -> Result<String, String> {
        self.run_git(&["branch", "--show-current"])
            .map(|s| s.trim().to_string())
    }

    /// Switch to an existing branch
    pub fn checkout(&self, branch_name: &str) -> Result<(), String> {
        self.run_git(&["checkout", branch_name])
            .map(|_| ())
    }

    /// Delete a branch (only if fully merged)
    pub fn delete(&self, branch_name: &str) -> Result<(), String> {
        self.run_git(&["branch", "-d", branch_name])
            .map(|_| ())
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
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    }
}

#[derive(Debug)]
pub struct BranchInfo {
    pub name: String,
    pub short_hash: String,
    pub upstream: Option<String>,
    pub is_current: bool,
}
```

## Naming Conventions

Branch names are the first thing a developer sees in `git branch` output or a pull request list. A good naming convention makes agent branches instantly identifiable:

```rust
use chrono::Local;

pub struct BranchNamer;

impl BranchNamer {
    /// Generate a branch name for an agent task
    /// Format: agent/<date>-<short-description>
    pub fn for_task(description: &str) -> String {
        let date = Local::now().format("%Y%m%d");
        let slug = Self::slugify(description);
        format!("agent/{}-{}", date, slug)
    }

    /// Generate a branch name for a specific tool action
    /// Format: agent/<tool>/<short-description>
    pub fn for_tool_action(tool: &str, description: &str) -> String {
        let slug = Self::slugify(description);
        format!("agent/{}/{}", tool, slug)
    }

    /// Convert a free-form description into a branch-safe slug
    fn slugify(text: &str) -> String {
        text.to_lowercase()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() { c }
                else { '-' }
            })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-")
            .chars()
            .take(50) // Keep branch names reasonable
            .collect()
    }
}
```

Good branch naming conventions for agents include:

- `agent/20260316-add-error-handling` -- prefixed with `agent/` so developers can filter them easily
- `agent/fix/null-pointer-in-parser` -- categorized by type (fix, feature, refactor)
- `claude/implement-git-status` -- named after the specific agent for multi-agent environments

::: python Coming from Python
Python developers working with Git typically use string formatting for branch names: `f"feature/{task_id}-{description}"`. The Rust approach is similar, but the slugify function handles more edge cases at compile time. In Python, you might use `re.sub(r'[^a-z0-9-]', '-', text.lower())`. The Rust version uses iterator chains instead of regex, which avoids the regex dependency for this simple transformation.
:::

## Branch Lifecycle Management

An agent branch goes through a predictable lifecycle: creation, work, review, merge or discard. Your agent should manage each phase:

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct BranchLifecycle {
    repo_dir: PathBuf,
    manager: BranchManager,
}

impl BranchLifecycle {
    pub fn new(repo_dir: impl Into<PathBuf>) -> Self {
        let repo_dir = repo_dir.into();
        let manager = BranchManager::new(repo_dir.clone());
        Self { repo_dir, manager }
    }

    /// Start a new agent task on a fresh branch
    pub fn start_task(&self, description: &str) -> Result<String, String> {
        // Ensure we start from a clean, up-to-date main branch
        let main_branch = self.detect_main_branch()?;
        self.manager.checkout(&main_branch)?;

        // Create and switch to the task branch
        let branch_name = BranchNamer::for_task(description);
        self.manager.create_and_checkout(&branch_name)?;

        Ok(branch_name)
    }

    /// Check if the branch has diverged from main and needs rebasing
    pub fn needs_rebase(&self, branch_name: &str) -> Result<bool, String> {
        let main_branch = self.detect_main_branch()?;
        let behind = self.run_git(&[
            "rev-list", "--count",
            &format!("{}..{}", branch_name, &main_branch),
        ])?;

        let count: usize = behind.trim().parse().unwrap_or(0);
        Ok(count > 0) // main has new commits since our branch point
    }

    /// Clean up merged branches with the agent/ prefix
    pub fn cleanup_merged(&self) -> Result<Vec<String>, String> {
        let main_branch = self.detect_main_branch()?;
        let output = self.run_git(&[
            "branch", "--merged", &main_branch,
            "--format=%(refname:short)",
        ])?;

        let mut cleaned = Vec::new();
        for branch in output.lines() {
            let branch = branch.trim();
            if branch.starts_with("agent/") && branch != main_branch {
                self.manager.delete(branch)?;
                cleaned.push(branch.to_string());
            }
        }

        Ok(cleaned)
    }

    /// Detect whether the main branch is called "main" or "master"
    fn detect_main_branch(&self) -> Result<String, String> {
        let branches = self.manager.list_branches()?;
        for name in &["main", "master"] {
            if branches.iter().any(|b| b.name == *name) {
                return Ok(name.to_string());
            }
        }
        Err("Could not detect main branch (expected 'main' or 'master')".to_string())
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
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    }
}
```

## Strategies for Agent Branching

Different branching strategies suit different agent workflows. Here are three common patterns:

### 1. Single Feature Branch

The simplest strategy: one branch per agent session. All changes go on a single branch, and the entire session is either merged or discarded.

```rust
// Simple: one branch per session
pub fn single_branch_workflow(repo_dir: &Path, task: &str) -> Result<String, String> {
    let lifecycle = BranchLifecycle::new(repo_dir);
    let branch = lifecycle.start_task(task)?;
    // Agent does all work on this branch
    // User reviews and merges or discards
    Ok(branch)
}
```

### 2. Stacked Branches

For complex tasks, the agent creates a chain of branches where each builds on the previous one. This enables incremental review:

```rust
use std::path::Path;

pub fn stacked_branch_workflow(
    repo_dir: &Path,
    steps: &[&str],
) -> Result<Vec<String>, String> {
    let lifecycle = BranchLifecycle::new(repo_dir);
    let mut branches = Vec::new();

    for (i, step) in steps.iter().enumerate() {
        let branch_name = format!("agent/stack-{:02}-{}", i + 1,
            BranchNamer::for_task(step).trim_start_matches("agent/"));

        if i == 0 {
            let main = lifecycle.detect_main_branch()?;
            lifecycle.manager.checkout(&main)?;
        }
        // Each subsequent branch builds on the previous
        lifecycle.manager.create_and_checkout(&branch_name)?;
        branches.push(branch_name);
    }

    Ok(branches)
}
```

### 3. Checkpoint Branches

The agent creates a new branch at each significant checkpoint, allowing the user to pick which state they want:

```rust
use std::path::Path;

pub fn checkpoint_branch(
    repo_dir: &Path,
    base_name: &str,
    checkpoint_num: u32,
    description: &str,
) -> Result<String, String> {
    let manager = BranchManager::new(repo_dir);
    let branch_name = format!(
        "agent/{}/checkpoint-{:02}-{}",
        base_name,
        checkpoint_num,
        BranchNamer::for_task(description)
            .trim_start_matches("agent/")
    );

    // Create the checkpoint branch at the current HEAD without switching
    let current = manager.current_branch()?;
    manager.create_from_base(&branch_name, &current)?;

    Ok(branch_name)
}
```

::: wild In the Wild
Claude Code typically works on the user's current branch rather than creating its own branches. This direct approach is appropriate for an interactive agent where the user is present and can review changes in real time. However, when Claude Code uses worktrees (covered in the next subchapter), it creates dedicated branches for parallel work. The choice between branch-per-task and working-on-current-branch depends on whether the agent operates interactively (where branch overhead is friction) or autonomously (where branch isolation is safety).
:::

## Guarding Against Branch Conflicts

Before switching branches or creating new ones, check for uncommitted changes that would be lost:

```rust
use std::path::Path;

pub fn safe_branch_switch(repo_dir: &Path, target_branch: &str) -> Result<(), String> {
    let manager = BranchManager::new(repo_dir);

    // Check for uncommitted changes
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to check status: {}", e))?;

    let status = String::from_utf8_lossy(&output.stdout);
    if !status.trim().is_empty() {
        return Err(format!(
            "Cannot switch to '{}': you have uncommitted changes. \
             Commit or stash them first.",
            target_branch
        ));
    }

    manager.checkout(target_branch)
}
```

## Key Takeaways

- Always create feature branches for agent work rather than committing directly to `main` -- branches are cheap and provide isolation for review and rollback.
- Use a consistent naming convention with a recognizable prefix like `agent/` so developers can easily filter, review, and clean up agent-created branches.
- Choose a branching strategy that matches the agent's workflow: single branch for simple tasks, stacked branches for complex multi-step work, or checkpoint branches for user-selectable states.
- Always check for uncommitted changes before switching branches, and detect the main branch name programmatically rather than hardcoding `main` or `master`.
- Clean up merged agent branches automatically to prevent branch proliferation -- the `agent/` prefix makes this safe because you can target only agent-created branches.
