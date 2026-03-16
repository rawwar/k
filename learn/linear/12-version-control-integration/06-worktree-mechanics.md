---
title: Worktree Mechanics
description: Using Git worktrees to maintain multiple working directories from a single repository, enabling parallel agent workstreams without branch switching.
---

# Worktree Mechanics

> **What you'll learn:**
> - How Git worktrees provide separate working directories that share the same object database and ref namespace
> - Creating, listing, and removing worktrees programmatically for parallel agent tasks that need isolated file systems
> - The constraints and gotchas of worktrees: no two worktrees can have the same branch checked out, and lock/prune mechanics

Worktrees are one of Git's most underused features, but for a coding agent, they are transformative. A worktree is a separate working directory linked to the same repository. It has its own checked-out branch, its own index, and its own file state, but shares the same object database and refs with the main working tree. This means your agent can work on multiple tasks simultaneously without any branch switching, stashing, or file conflicts.

## Why Worktrees Matter for Agents

Consider a typical agent scenario: the user asks the agent to fix a bug while a refactoring task is in progress. Without worktrees, the agent must either stash the refactoring changes (risking conflicts when unstashing), commit a half-done state, or tell the user to wait. With worktrees, the agent creates a new working directory, checks out a fresh branch there, and works on the bug fix in complete isolation. The refactoring files in the main working tree are untouched.

```
repo/                         # Main worktree (refactoring in progress)
  .git/
  src/
  Cargo.toml

/tmp/agent-worktrees/
  bugfix-null-check/          # Worktree 1 (bug fix)
    src/
    Cargo.toml
  feature-add-logging/        # Worktree 2 (another task)
    src/
    Cargo.toml
```

All three directories share the same `.git` object database. Commits made in any worktree are visible from all others.

## Creating and Managing Worktrees

Here is a complete worktree manager for your agent:

```rust
use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;

pub struct WorktreeManager {
    repo_dir: PathBuf,
    worktree_base: PathBuf,
}

#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: String,
    pub head_commit: String,
    pub is_main: bool,
}

impl WorktreeManager {
    pub fn new(repo_dir: impl Into<PathBuf>, worktree_base: impl Into<PathBuf>) -> Self {
        Self {
            repo_dir: repo_dir.into(),
            worktree_base: worktree_base.into(),
        }
    }

    /// Create a new worktree with a new branch based on the given ref
    pub fn create(
        &self,
        name: &str,
        base_ref: &str,
    ) -> Result<WorktreeInfo, String> {
        // Ensure the worktree base directory exists
        fs::create_dir_all(&self.worktree_base)
            .map_err(|e| format!("Failed to create worktree base dir: {}", e))?;

        let worktree_path = self.worktree_base.join(name);
        let branch_name = format!("agent/worktree/{}", name);

        let path_str = worktree_path.to_string_lossy().to_string();
        let output = Command::new("git")
            .args([
                "worktree", "add",
                "-b", &branch_name,
                &path_str,
                base_ref,
            ])
            .current_dir(&self.repo_dir)
            .output()
            .map_err(|e| format!("Failed to create worktree: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git worktree add failed: {}", stderr));
        }

        // Get the HEAD commit of the new worktree
        let head = self.run_git_in(&worktree_path, &["rev-parse", "--short", "HEAD"])?;

        Ok(WorktreeInfo {
            path: worktree_path,
            branch: branch_name,
            head_commit: head.trim().to_string(),
            is_main: false,
        })
    }

    /// Create a worktree that checks out an existing branch
    pub fn create_for_branch(
        &self,
        branch_name: &str,
    ) -> Result<WorktreeInfo, String> {
        let safe_name = branch_name.replace('/', "-");
        let worktree_path = self.worktree_base.join(&safe_name);

        fs::create_dir_all(&self.worktree_base)
            .map_err(|e| format!("Failed to create worktree base dir: {}", e))?;

        let path_str = worktree_path.to_string_lossy().to_string();
        let output = Command::new("git")
            .args(["worktree", "add", &path_str, branch_name])
            .current_dir(&self.repo_dir)
            .output()
            .map_err(|e| format!("Failed to create worktree: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git worktree add failed: {}", stderr));
        }

        let head = self.run_git_in(&worktree_path, &["rev-parse", "--short", "HEAD"])?;

        Ok(WorktreeInfo {
            path: worktree_path,
            branch: branch_name.to_string(),
            head_commit: head.trim().to_string(),
            is_main: false,
        })
    }

    /// List all worktrees for this repository
    pub fn list(&self) -> Result<Vec<WorktreeInfo>, String> {
        let output = Command::new("git")
            .args(["worktree", "list", "--porcelain"])
            .current_dir(&self.repo_dir)
            .output()
            .map_err(|e| format!("Failed to list worktrees: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut worktrees = Vec::new();
        let mut current = WorktreeInfo {
            path: PathBuf::new(),
            branch: String::new(),
            head_commit: String::new(),
            is_main: false,
        };

        for line in stdout.lines() {
            if line.starts_with("worktree ") {
                current.path = PathBuf::from(&line[9..]);
            } else if line.starts_with("HEAD ") {
                current.head_commit = line[5..13].to_string();
            } else if line.starts_with("branch ") {
                current.branch = line[7..]
                    .strip_prefix("refs/heads/")
                    .unwrap_or(&line[7..])
                    .to_string();
            } else if line == "bare" {
                current.is_main = true;
            } else if line.is_empty() {
                // End of entry -- save and reset
                if !current.path.as_os_str().is_empty() {
                    worktrees.push(current.clone());
                }
                current = WorktreeInfo {
                    path: PathBuf::new(),
                    branch: String::new(),
                    head_commit: String::new(),
                    is_main: false,
                };
            }
        }

        // Handle the last entry if the output does not end with a blank line
        if !current.path.as_os_str().is_empty() {
            worktrees.push(current);
        }

        // The first worktree is always the main one
        if let Some(first) = worktrees.first_mut() {
            first.is_main = true;
        }

        Ok(worktrees)
    }

    /// Remove a worktree and clean up its files
    pub fn remove(&self, name: &str, force: bool) -> Result<(), String> {
        let worktree_path = self.worktree_base.join(name);
        let path_str = worktree_path.to_string_lossy().to_string();

        let mut args = vec!["worktree", "remove", &path_str];
        if force {
            args.push("--force");
        }

        let output = Command::new("git")
            .args(&args)
            .current_dir(&self.repo_dir)
            .output()
            .map_err(|e| format!("Failed to remove worktree: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("git worktree remove failed: {}", stderr))
        } else {
            Ok(())
        }
    }

    /// Prune stale worktree entries (worktrees whose directories no longer exist)
    pub fn prune(&self) -> Result<(), String> {
        let output = Command::new("git")
            .args(["worktree", "prune"])
            .current_dir(&self.repo_dir)
            .output()
            .map_err(|e| format!("Failed to prune worktrees: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    fn run_git_in(&self, dir: &Path, args: &[&str]) -> Result<String, String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
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

::: python Coming from Python
Python does not have a direct equivalent to Git worktrees, but the concept is similar to creating virtual environments: each worktree is an isolated "environment" for a branch. In Python, you might clone the repo multiple times to achieve isolation, but that duplicates the entire object database. Git worktrees share the objects, making them nearly free in terms of disk space -- just the working tree files are duplicated.
:::

## Worktree Constraints and Gotchas

Worktrees have important constraints your agent must respect:

**No two worktrees can check out the same branch.** If `main` is checked out in the primary worktree, you cannot check out `main` in a secondary worktree. This is a safety mechanism -- having two worktrees on the same branch could lead to conflicting index states.

```rust
/// Check if a branch is already checked out in any worktree
pub fn is_branch_checked_out(
    repo_dir: &Path,
    branch_name: &str,
) -> Result<bool, String> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to list worktrees: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let target = format!("refs/heads/{}", branch_name);

    for line in stdout.lines() {
        if line.starts_with("branch ") && line[7..] == target {
            return Ok(true);
        }
    }

    Ok(false)
}
```

**Worktrees share refs but not the index.** Each worktree has its own index file and its own HEAD, but branches and tags are shared. A commit made in one worktree is immediately visible from all others.

**Stale worktrees accumulate.** If the agent crashes or the user kills it mid-task, worktree directories may be left behind. Run `git worktree prune` periodically to clean up entries whose directories no longer exist.

::: tip In the Wild
Claude Code uses worktrees extensively for parallel task execution. When a user asks Claude Code to work on multiple tasks simultaneously, it creates separate worktrees so each task has its own file system context. This avoids the "stash and switch" dance that would be necessary with a single working directory. The worktree approach is especially important for long-running tasks: if one task is waiting on a CI check, the agent can work on another task in a different worktree without any file conflicts. Claude Code automatically cleans up worktrees when tasks complete.
:::

## Integrating Worktrees with the Agent Loop

Here is how worktree management fits into a multi-task agent:

```rust
use std::path::{Path, PathBuf};
use std::collections::HashMap;

pub struct AgentTaskManager {
    worktree_mgr: WorktreeManager,
    active_tasks: HashMap<String, WorktreeInfo>,
}

impl AgentTaskManager {
    pub fn new(repo_dir: &Path) -> Self {
        let worktree_base = std::env::temp_dir().join("agent-worktrees");
        Self {
            worktree_mgr: WorktreeManager::new(repo_dir, worktree_base),
            active_tasks: HashMap::new(),
        }
    }

    /// Start a new task in its own isolated worktree
    pub fn start_task(&mut self, task_id: &str, base_ref: &str) -> Result<PathBuf, String> {
        let worktree = self.worktree_mgr.create(task_id, base_ref)?;
        let path = worktree.path.clone();
        self.active_tasks.insert(task_id.to_string(), worktree);
        Ok(path)
    }

    /// Get the working directory for a running task
    pub fn task_dir(&self, task_id: &str) -> Option<&Path> {
        self.active_tasks.get(task_id).map(|w| w.path.as_path())
    }

    /// Complete a task and clean up its worktree
    pub fn finish_task(&mut self, task_id: &str) -> Result<(), String> {
        if let Some(worktree) = self.active_tasks.remove(task_id) {
            // The worktree's branch retains the commits
            self.worktree_mgr.remove(
                worktree.path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(task_id),
                false,
            )?;
        }
        Ok(())
    }

    /// Clean up all agent worktrees (e.g., on agent shutdown)
    pub fn cleanup_all(&mut self) -> Result<(), String> {
        let task_ids: Vec<String> = self.active_tasks.keys().cloned().collect();
        for task_id in task_ids {
            let _ = self.finish_task(&task_id);
        }
        self.worktree_mgr.prune()
    }
}
```

## Key Takeaways

- Git worktrees provide separate working directories that share the same object database, enabling parallel agent tasks without branch switching or stashing.
- No two worktrees can have the same branch checked out -- always create a new branch when creating a worktree for agent work.
- Worktrees are nearly free in terms of disk space (only the working tree files are duplicated) and commits made in any worktree are immediately visible from all others.
- Always clean up worktrees when tasks complete, and run `git worktree prune` periodically to handle stale entries from crashed or interrupted sessions.
- Integrate worktree management into your agent's task system so each concurrent task gets its own isolated file system context automatically.
