---
title: Undo Strategies
description: Implementing multi-level undo for agent actions using Git reset, revert, checkout, and reflog to restore files, commits, and branches to previous states.
---

# Undo Strategies

> **What you'll learn:**
> - The spectrum of undo operations: git checkout (file-level), git reset (commit-level), git revert (history-preserving), and their appropriate use cases
> - Building a multi-level undo system that tracks agent actions and maps them to the correct Git undo operation
> - Using git reflog as the ultimate safety net to recover from even destructive operations like hard reset and force push

The previous subchapter covered proactive safety -- saving state before the agent works. This subchapter covers reactive recovery -- undoing changes after they have been made. The key insight is that Git provides multiple undo mechanisms, each appropriate for a different scope of change. Your agent needs to select the right one based on what the user wants to undo.

## The Undo Spectrum

Git undo operations form a spectrum from surgical to sweeping:

| Operation | Scope | Preserves History | Affects Index | Affects Working Tree |
|-----------|-------|-------------------|---------------|---------------------|
| `git restore <file>` | Single file | Yes | No | Yes |
| `git restore --staged <file>` | Single file (unstage) | Yes | Yes | No |
| `git reset --soft HEAD~1` | Last commit | No | No | No |
| `git reset --mixed HEAD~1` | Last commit + index | No | Yes | No |
| `git reset --hard HEAD~1` | Last commit + index + files | No | Yes | Yes |
| `git revert <commit>` | Any commit | Yes (creates new) | Yes | Yes |

Let's implement each one with proper context for agent use:

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct UndoManager {
    repo_dir: PathBuf,
}

impl UndoManager {
    pub fn new(repo_dir: impl Into<PathBuf>) -> Self {
        Self { repo_dir: repo_dir.into() }
    }

    /// Undo changes to a specific file (restore it to the last committed state)
    pub fn restore_file(&self, file_path: &str) -> Result<(), String> {
        self.run_git(&["restore", file_path])
            .map(|_| ())
    }

    /// Unstage a file (remove from index but keep working tree changes)
    pub fn unstage_file(&self, file_path: &str) -> Result<(), String> {
        self.run_git(&["restore", "--staged", file_path])
            .map(|_| ())
    }

    /// Restore a file to its state at a specific commit
    pub fn restore_file_at_commit(
        &self,
        file_path: &str,
        commit: &str,
    ) -> Result<(), String> {
        self.run_git(&["restore", "--source", commit, "--", file_path])
            .map(|_| ())
    }

    /// Undo the last N commits, keeping changes staged (soft reset)
    pub fn soft_reset(&self, count: usize) -> Result<String, String> {
        let target = format!("HEAD~{}", count);
        self.run_git(&["reset", "--soft", &target])
    }

    /// Undo the last N commits, keeping changes unstaged (mixed reset)
    pub fn mixed_reset(&self, count: usize) -> Result<String, String> {
        let target = format!("HEAD~{}", count);
        self.run_git(&["reset", "--mixed", &target])
    }

    /// Undo the last N commits and discard all changes (hard reset)
    /// WARNING: This is destructive. Ensure safety mechanisms are in place.
    pub fn hard_reset(&self, target: &str) -> Result<String, String> {
        self.run_git(&["reset", "--hard", target])
    }

    /// Create a new commit that undoes a previous commit (safe, preserves history)
    pub fn revert_commit(&self, commit_hash: &str) -> Result<String, String> {
        self.run_git(&["revert", "--no-edit", commit_hash])
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
Python does not have a native undo system for file modifications. You might save copies of files before editing them, or use a version control wrapper. Git's undo capabilities are built into the data model -- every previous state is preserved in the object database, and you just need to point the right ref at the right commit. In Python terms, it is like having an automatic `deepcopy()` of your entire project before every operation, but without the memory or disk cost.
:::

## Choosing the Right Undo Operation

The agent needs to map user intent to the correct Git operation. Here is the decision logic:

```rust
#[derive(Debug, Clone)]
pub enum UndoIntent {
    /// "Undo the changes to this file"
    RestoreFile(String),

    /// "Don't commit this file after all"
    UnstageFile(String),

    /// "Undo the last commit but keep my changes"
    UndoLastCommitKeepChanges,

    /// "Undo the last commit completely"
    UndoLastCommitDiscardChanges,

    /// "Undo this specific commit but keep everything else"
    RevertSpecificCommit(String),

    /// "Go back to how things were before the agent started"
    RestoreToCheckpoint(String),
}

impl UndoManager {
    pub fn execute_undo(&self, intent: UndoIntent) -> Result<String, String> {
        match intent {
            UndoIntent::RestoreFile(path) => {
                self.restore_file(&path)?;
                Ok(format!("Restored '{}' to last committed state", path))
            }

            UndoIntent::UnstageFile(path) => {
                self.unstage_file(&path)?;
                Ok(format!("Unstaged '{}'", path))
            }

            UndoIntent::UndoLastCommitKeepChanges => {
                // Use soft reset to undo the commit but keep changes staged
                self.soft_reset(1)?;
                Ok("Undid last commit; changes remain staged".to_string())
            }

            UndoIntent::UndoLastCommitDiscardChanges => {
                // Use hard reset to fully undo the last commit
                self.hard_reset("HEAD~1")?;
                Ok("Undid last commit and discarded all changes".to_string())
            }

            UndoIntent::RevertSpecificCommit(hash) => {
                self.revert_commit(&hash)?;
                Ok(format!("Created revert commit for {}", &hash[..8]))
            }

            UndoIntent::RestoreToCheckpoint(hash) => {
                self.hard_reset(&hash)?;
                Ok(format!("Restored to checkpoint {}", &hash[..8]))
            }
        }
    }
}
```

## Tracking Agent Actions for Undo

To provide meaningful undo, your agent should track what it did and map each action to the appropriate reversal:

```rust
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct AgentAction {
    pub description: String,
    pub action_type: ActionType,
    pub commit_before: Option<String>,
    pub commit_after: Option<String>,
    pub files_affected: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ActionType {
    FileEdit,
    FileCreate,
    FileDelete,
    Commit,
    BranchCreate,
    BranchSwitch,
}

pub struct UndoStack {
    actions: VecDeque<AgentAction>,
    max_size: usize,
    undo_manager: UndoManager,
}

impl UndoStack {
    pub fn new(repo_dir: &Path, max_size: usize) -> Self {
        Self {
            actions: VecDeque::new(),
            max_size,
            undo_manager: UndoManager::new(repo_dir),
        }
    }

    pub fn push(&mut self, action: AgentAction) {
        if self.actions.len() >= self.max_size {
            self.actions.pop_front();
        }
        self.actions.push_back(action);
    }

    /// Undo the most recent agent action
    pub fn undo_last(&mut self) -> Result<String, String> {
        let action = self.actions.pop_back()
            .ok_or_else(|| "No actions to undo".to_string())?;

        match action.action_type {
            ActionType::FileEdit | ActionType::FileCreate => {
                // Restore files to their state before this action
                if let Some(ref commit) = action.commit_before {
                    for file in &action.files_affected {
                        self.undo_manager.restore_file_at_commit(file, commit)?;
                    }
                    Ok(format!("Undid: {} ({} file(s) restored)",
                        action.description, action.files_affected.len()))
                } else {
                    // No commit reference -- restore from HEAD
                    for file in &action.files_affected {
                        self.undo_manager.restore_file(file)?;
                    }
                    Ok(format!("Undid: {}", action.description))
                }
            }

            ActionType::FileDelete => {
                // Restore deleted files from the commit before deletion
                if let Some(ref commit) = action.commit_before {
                    for file in &action.files_affected {
                        self.undo_manager.restore_file_at_commit(file, commit)?;
                    }
                    Ok(format!("Restored {} deleted file(s)", action.files_affected.len()))
                } else {
                    Err("Cannot restore deleted files without a reference commit".to_string())
                }
            }

            ActionType::Commit => {
                // Undo the commit, keeping changes staged
                self.undo_manager.soft_reset(1)?;
                Ok(format!("Undid commit: {}", action.description))
            }

            ActionType::BranchCreate | ActionType::BranchSwitch => {
                Ok(format!("Branch operation '{}' noted but not auto-reversed -- use git branch commands directly", action.description))
            }
        }
    }

    /// Show the undo history
    pub fn history(&self) -> Vec<String> {
        self.actions.iter()
            .rev()
            .enumerate()
            .map(|(i, action)| {
                format!("  {}. [{}] {} ({} file(s))",
                    i + 1,
                    format!("{:?}", action.action_type),
                    action.description,
                    action.files_affected.len())
            })
            .collect()
    }
}
```

## The Reflog: Ultimate Recovery

The reflog is Git's transaction log. It records every time a ref (like HEAD or a branch) changes. Even after a hard reset, the commits you "lost" are still in the reflog for at least 30 days:

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug)]
pub struct ReflogEntry {
    pub hash: String,
    pub action: String,
    pub message: String,
    pub relative_time: String,
}

pub fn read_reflog(
    repo_dir: &Path,
    count: usize,
) -> Result<Vec<ReflogEntry>, String> {
    let count_str = format!("-{}", count);
    let output = Command::new("git")
        .args([
            "reflog", "show",
            &count_str,
            "--format=%h %gs %ar",
        ])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to read reflog: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let entries: Vec<ReflogEntry> = stdout.lines()
        .filter(|l| !l.is_empty())
        .map(|line| {
            let parts: Vec<&str> = line.splitn(3, ' ').collect();
            let hash = parts[0].to_string();
            let rest = parts.get(2).unwrap_or(&"").to_string();

            // Split the rest into action/message and time
            let (action_msg, time) = if let Some(pos) = rest.rfind(" ago") {
                // Walk backwards to find the time portion
                let time_start = rest[..pos].rfind(", ").map(|p| p + 2)
                    .or_else(|| rest[..pos].rfind(' '))
                    .unwrap_or(0);
                (rest[..time_start].trim().to_string(),
                 rest[time_start..].trim().to_string())
            } else {
                (rest.clone(), String::new())
            };

            ReflogEntry {
                hash,
                action: action_msg.clone(),
                message: action_msg,
                relative_time: time,
            }
        })
        .collect();

    Ok(entries)
}

/// Find a specific state in the reflog by searching for a pattern
pub fn find_in_reflog(
    repo_dir: &Path,
    search_pattern: &str,
) -> Result<Option<String>, String> {
    let output = Command::new("git")
        .args(["reflog", "show", "--format=%h %gs"])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to read reflog: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.contains(search_pattern) {
            let hash = line.split_whitespace().next().unwrap_or("");
            return Ok(Some(hash.to_string()));
        }
    }

    Ok(None)
}

/// Recover a "lost" commit by its hash from the reflog
pub fn recover_from_reflog(
    repo_dir: &Path,
    commit_hash: &str,
) -> Result<String, String> {
    // Create a recovery branch pointing to the "lost" commit
    let branch_name = format!("recovered/{}", &commit_hash[..8]);
    let output = Command::new("git")
        .args(["branch", &branch_name, commit_hash])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to create recovery branch: {}", e))?;

    if output.status.success() {
        Ok(format!("Created branch '{}' pointing to recovered commit", branch_name))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
```

::: tip In the Wild
Claude Code relies on Git's built-in undo mechanisms rather than implementing a custom undo system. When users want to revert changes Claude Code made, they can use standard Git commands. This design choice is intentional: rather than building a parallel undo system that might diverge from Git's state, Claude Code ensures every modification it makes is a standard Git operation that can be undone with standard Git tools. The reflog serves as the ultimate fallback -- even if a user accidentally runs `git reset --hard`, the previous state is recoverable through the reflog for at least 30 days.
:::

## Multi-Level Undo in Practice

Here is how all the undo pieces fit together in an agent workflow:

```rust
use std::path::Path;

pub fn demonstrate_undo_workflow(repo_dir: &Path) -> Result<(), String> {
    let undo = UndoManager::new(repo_dir);
    let mut stack = UndoStack::new(repo_dir, 50);

    // Agent edits a file -- record the action
    let head_before = get_head_hash(repo_dir)?;
    // ... agent performs file edit ...
    stack.push(AgentAction {
        description: "Updated error handling in main.rs".to_string(),
        action_type: ActionType::FileEdit,
        commit_before: Some(head_before),
        commit_after: None,
        files_affected: vec!["src/main.rs".to_string()],
    });

    // User says "undo that"
    let result = stack.undo_last()?;
    println!("{}", result);

    // If even the undo goes wrong, check the reflog
    let entries = read_reflog(repo_dir, 10)?;
    for entry in &entries {
        println!("{} -- {}", entry.hash, entry.message);
    }

    Ok(())
}

fn get_head_hash(repo_dir: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to get HEAD: {}", e))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
```

## Key Takeaways

- Git provides a spectrum of undo operations, from file-level restore to commit-level reset to history-preserving revert -- choose the right scope for the user's intent.
- Build an undo stack that records every agent action with enough metadata (commit hash, affected files) to reverse it correctly.
- Use `git revert` instead of `git reset` when the commit has been shared (pushed), since revert preserves history while reset rewrites it.
- The reflog is the ultimate safety net: it records every ref change for at least 30 days, allowing recovery from even hard resets and force pushes.
- Design your agent to prefer reversible operations (revert, soft reset) over destructive ones (hard reset, force push) and always warn the user before performing irreversible actions.
