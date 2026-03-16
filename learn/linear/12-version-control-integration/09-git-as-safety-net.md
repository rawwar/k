---
title: Git as Safety Net
description: Using Git's immutable history as a safety mechanism — auto-committing before agent changes, checkpoint branches, and stash-based recovery points.
---

# Git as Safety Net

> **What you'll learn:**
> - How to create automatic checkpoint commits before agent modifications so every change is reversible
> - Using lightweight tags or refs to mark known-good states that the user can return to if agent changes go wrong
> - The stash as a quick safety mechanism: stashing uncommitted work before risky operations and restoring on failure

The most important property of an agent that modifies code is not intelligence -- it is reversibility. Users will tolerate mistakes if they can undo them. Users will not tolerate an agent that corrupts their working tree with no path back. Git's immutable object model, which you learned about in the first subchapter, is the foundation for building safety mechanisms that make agent modifications trustworthy.

## The Safety Principle

Before any destructive or complex operation, save the current state. After the operation, the user can always return to that saved state. This principle sounds obvious, but implementing it correctly requires understanding which Git mechanisms to use and when.

The three main safety mechanisms are:

1. **Checkpoint commits** -- save the working tree state as a commit
2. **Lightweight refs** -- mark specific commits as recovery points
3. **Stash entries** -- quick save-and-restore for uncommitted work

## Automatic Checkpoint Commits

The most robust safety mechanism is committing the current state before the agent starts working. Even if the agent makes a mess of the working tree, the user can always `git reset --hard` back to the checkpoint:

```rust
use std::path::{Path, PathBuf};
use std::process::Command;
use chrono::Local;

pub struct SafetyNet {
    repo_dir: PathBuf,
}

impl SafetyNet {
    pub fn new(repo_dir: impl Into<PathBuf>) -> Self {
        Self { repo_dir: repo_dir.into() }
    }

    /// Create a checkpoint commit with all current changes before agent work begins.
    /// Returns the checkpoint commit hash, or None if working tree is clean.
    pub fn create_checkpoint(&self) -> Result<Option<String>, String> {
        // Check if there are any changes to save
        if self.is_clean()? {
            return Ok(None);
        }

        let timestamp = Local::now().format("%Y%m%d-%H%M%S");
        let message = format!(
            "agent: checkpoint before modifications ({})",
            timestamp
        );

        // Stage everything
        self.run_git(&["add", "-A"])?;

        // Create the checkpoint commit
        self.run_git(&["commit", "-m", &message])?;

        // Get the commit hash
        let hash = self.run_git(&["rev-parse", "HEAD"])?;
        let hash = hash.trim().to_string();

        // Tag it for easy reference
        let tag_name = format!("agent-checkpoint/{}", timestamp);
        self.run_git(&["tag", &tag_name, &hash])?;

        Ok(Some(hash))
    }

    /// Restore the working tree to a checkpoint commit
    pub fn restore_checkpoint(&self, commit_hash: &str) -> Result<(), String> {
        self.run_git(&["reset", "--hard", commit_hash])?;
        Ok(())
    }

    /// Check if the working tree is clean (no staged or unstaged changes)
    fn is_clean(&self) -> Result<bool, String> {
        let output = self.run_git(&["status", "--porcelain"])?;
        Ok(output.trim().is_empty())
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

::: python Coming from Python
Python developers might save backups by copying files with `shutil.copytree()`. Git checkpoints are better because they are deduplicating (only changed content takes space), they preserve history (you can diff between checkpoints), and they integrate with the developer's existing workflow. There is no need for a separate backup directory -- the checkpoint is just another commit in the Git DAG.
:::

## Lightweight Refs for Recovery Points

Tags are more visible than bare commit hashes. By tagging checkpoints, you give the user a way to see and navigate to saved states:

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct RecoveryPoints {
    repo_dir: PathBuf,
    prefix: String,
}

impl RecoveryPoints {
    pub fn new(repo_dir: impl Into<PathBuf>) -> Self {
        Self {
            repo_dir: repo_dir.into(),
            prefix: "agent-recovery".to_string(),
        }
    }

    /// Mark the current HEAD as a named recovery point
    pub fn mark(&self, name: &str) -> Result<String, String> {
        let ref_name = format!("{}/{}", self.prefix, name);

        // Create or update the ref to point to HEAD
        let hash = self.run_git(&["rev-parse", "HEAD"])?;
        let hash = hash.trim();

        self.run_git(&["tag", "-f", &ref_name, hash])?;

        Ok(format!("Recovery point '{}' set at {}", name, &hash[..8]))
    }

    /// List all recovery points with their commit info
    pub fn list(&self) -> Result<Vec<RecoveryPoint>, String> {
        let pattern = format!("refs/tags/{}/*", self.prefix);
        let output = self.run_git(&[
            "tag", "-l", &format!("{}/*", self.prefix),
            "--format=%(refname:short) %(objectname:short) %(creatordate:relative)"
        ])?;

        Ok(output.lines()
            .filter(|l| !l.is_empty())
            .map(|line| {
                let parts: Vec<&str> = line.splitn(3, ' ').collect();
                RecoveryPoint {
                    name: parts[0]
                        .strip_prefix(&format!("{}/", self.prefix))
                        .unwrap_or(parts[0])
                        .to_string(),
                    commit_hash: parts.get(1).unwrap_or(&"").to_string(),
                    created: parts.get(2).unwrap_or(&"").to_string(),
                }
            })
            .collect())
    }

    /// Restore to a named recovery point
    pub fn restore(&self, name: &str) -> Result<(), String> {
        let ref_name = format!("{}/{}", self.prefix, name);
        self.run_git(&["reset", "--hard", &ref_name])?;
        Ok(())
    }

    /// Clean up all recovery point tags
    pub fn cleanup(&self) -> Result<usize, String> {
        let points = self.list()?;
        let count = points.len();

        for point in &points {
            let tag = format!("{}/{}", self.prefix, point.name);
            let _ = self.run_git(&["tag", "-d", &tag]);
        }

        Ok(count)
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
pub struct RecoveryPoint {
    pub name: String,
    pub commit_hash: String,
    pub created: String,
}
```

## Stash-Based Safety

The stash is ideal for quick save-and-restore operations where you do not want to pollute the commit history. It is particularly useful before operations that might fail (rebasing, merging, complex refactors):

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct StashSafety {
    repo_dir: PathBuf,
}

impl StashSafety {
    pub fn new(repo_dir: impl Into<PathBuf>) -> Self {
        Self { repo_dir: repo_dir.into() }
    }

    /// Save current changes to the stash with a descriptive message
    pub fn save(&self, message: &str) -> Result<bool, String> {
        let output = Command::new("git")
            .args(["stash", "push", "-m", message, "--include-untracked"])
            .current_dir(&self.repo_dir)
            .output()
            .map_err(|e| format!("Failed to stash: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        // "No local changes to save" means nothing was stashed
        Ok(!stdout.contains("No local changes"))
    }

    /// Restore the most recent stash entry
    pub fn restore(&self) -> Result<(), String> {
        let output = Command::new("git")
            .args(["stash", "pop"])
            .current_dir(&self.repo_dir)
            .output()
            .map_err(|e| format!("Failed to restore stash: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Stash restore failed: {}", stderr))
        }
    }

    /// Execute an operation with automatic stash save/restore on failure
    pub fn with_safety<F, T>(&self, operation_name: &str, f: F) -> Result<T, String>
    where
        F: FnOnce() -> Result<T, String>,
    {
        let message = format!("agent: safety stash before {}", operation_name);
        let stashed = self.save(&message)?;

        match f() {
            Ok(result) => {
                // Operation succeeded -- discard the stash if we made one
                if stashed {
                    // Drop the stash since we don't need it
                    let _ = Command::new("git")
                        .args(["stash", "drop"])
                        .current_dir(&self.repo_dir)
                        .output();
                }
                Ok(result)
            }
            Err(e) => {
                // Operation failed -- restore the stash
                if stashed {
                    let _ = self.restore();
                }
                Err(format!("{} failed (changes restored): {}", operation_name, e))
            }
        }
    }
}
```

## Combining Safety Mechanisms

In practice, you use different safety mechanisms at different levels of the agent:

```rust
use std::path::Path;

pub struct AgentSafety {
    safety_net: SafetyNet,
    recovery: RecoveryPoints,
    stash: StashSafety,
}

impl AgentSafety {
    pub fn new(repo_dir: &Path) -> Self {
        Self {
            safety_net: SafetyNet::new(repo_dir),
            recovery: RecoveryPoints::new(repo_dir),
            stash: StashSafety::new(repo_dir),
        }
    }

    /// Full safety setup before a new agent session
    pub fn begin_session(&self) -> Result<SessionSafety, String> {
        // 1. Create a checkpoint commit of any pending changes
        let checkpoint = self.safety_net.create_checkpoint()?;

        // 2. Mark the current state as a recovery point
        self.recovery.mark("session-start")?;

        Ok(SessionSafety {
            checkpoint_hash: checkpoint,
            start_branch: self.get_current_branch()?,
        })
    }

    /// Safety setup before a specific tool execution
    pub fn before_tool(&self, tool_name: &str) -> Result<(), String> {
        self.recovery.mark(&format!("before-{}", tool_name))?;
        Ok(())
    }

    /// Rollback to the session start state
    pub fn abort_session(&self) -> Result<(), String> {
        self.recovery.restore("session-start")
    }

    fn get_current_branch(&self) -> Result<String, String> {
        let output = std::process::Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&self.safety_net.repo_dir)
            .output()
            .map_err(|e| format!("Failed to get branch: {}", e))?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

#[derive(Debug)]
pub struct SessionSafety {
    pub checkpoint_hash: Option<String>,
    pub start_branch: String,
}
```

::: tip In the Wild
Claude Code implements a safety model where it can restore files to their previous state after modifications. Before editing files, Claude Code tracks the original state so that if the user is unhappy with the changes or if a tool execution fails, the working tree can be reverted. This checkpoint-and-restore pattern is fundamental to building user trust -- it transforms the agent from a potentially destructive tool into a safe collaborator. The key design decision is making safety automatic rather than opt-in: every modification is reversible by default.
:::

## When Safety Mechanisms Fail

Even safety mechanisms have edge cases. Be aware of these limitations:

- **Checkpoint commits change the branch history** -- if the user pushes before realizing they want to undo, the checkpoint commit is in the public history. Use `git reset` to remove it before pushing.
- **Stash conflicts** -- restoring a stash can itself produce merge conflicts if the working tree has changed. Always handle `stash pop` failures gracefully.
- **Untracked files** -- by default, `git stash` does not save untracked files. Use `--include-untracked` to capture them, but be aware this will stash files that might be in `.gitignore`.
- **Large binary files** -- checkpointing a repository with large binaries creates large objects in the Git database. Consider excluding binary paths from safety commits.

## Key Takeaways

- Every agent modification should be reversible -- create automatic checkpoints before the agent starts working, so the user can always return to a known-good state.
- Use checkpoint commits for robust state saving, lightweight tags for named recovery points, and the stash for quick save-and-restore around risky operations.
- The `with_safety` pattern (save, attempt, restore-on-failure) encapsulates the common case of trying an operation while preserving the ability to roll back.
- Combine multiple safety mechanisms at different levels: session-level checkpoints, tool-level recovery points, and operation-level stash saves.
- Always include `--include-untracked` when stashing for safety, and handle stash restore failures gracefully since they can produce merge conflicts.
