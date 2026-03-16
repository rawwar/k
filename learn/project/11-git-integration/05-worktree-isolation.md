---
title: Worktree Isolation
description: Using git worktrees to give each agent task its own working directory, enabling parallel operations without interference and clean rollback of failed tasks.
---

# Worktree Isolation

> **What you'll learn:**
> - How git worktrees provide isolated working directories sharing a single repository
> - How to create, manage, and clean up worktrees for parallel agent tasks
> - Patterns for routing agent operations to the correct worktree based on task context

When a human developer works on one thing at a time, a single working directory is fine. But an agent might handle multiple tasks concurrently -- fixing a bug on one branch while refactoring a module on another. If both tasks share the same working directory, their file changes collide. Git worktrees solve this elegantly: they let you check out multiple branches of the same repository into separate directories, all sharing the same `.git` database.

## What Are Git Worktrees?

A git worktree is an additional working directory linked to the same repository. Each worktree has its own checked-out branch, its own staging area, and its own HEAD. But they all share the same object database, the same refs, and the same history.

Think of it like this: your main working directory is the "main worktree." When you add a worktree, git creates a new directory with the project files checked out at a different branch. Commits made in either worktree are immediately visible to the other (because they share the same object database).

```rust
use std::path::{Path, PathBuf};
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
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub commit: String,
    pub is_main: bool,
}

/// List all worktrees for this repository
pub fn list_worktrees(repo_path: &Path) -> Result<Vec<WorktreeInfo>, String> {
    let output = run_git_checked(repo_path, &["worktree", "list", "--porcelain"])?;

    let mut worktrees = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_commit: Option<String> = None;
    let mut current_branch: Option<String> = None;
    let mut is_first = true;

    for line in output.lines() {
        if line.starts_with("worktree ") {
            // Save previous worktree if we have one
            if let (Some(path), Some(commit)) = (current_path.take(), current_commit.take()) {
                worktrees.push(WorktreeInfo {
                    path,
                    branch: current_branch.take(),
                    commit,
                    is_main: is_first,
                });
                is_first = false;
            }
            current_path = Some(PathBuf::from(&line["worktree ".len()..]));
        } else if line.starts_with("HEAD ") {
            current_commit = Some(line["HEAD ".len()..].to_string());
        } else if line.starts_with("branch ") {
            let full_ref = &line["branch ".len()..];
            // Convert refs/heads/main to just main
            current_branch = Some(
                full_ref
                    .strip_prefix("refs/heads/")
                    .unwrap_or(full_ref)
                    .to_string(),
            );
        } else if line == "detached" {
            current_branch = None;
        }
    }

    // Don't forget the last worktree
    if let (Some(path), Some(commit)) = (current_path, current_commit) {
        worktrees.push(WorktreeInfo {
            path,
            branch: current_branch,
            commit,
            is_main: is_first,
        });
    }

    Ok(worktrees)
}

fn main() {
    let repo = Path::new(".");

    match list_worktrees(repo) {
        Ok(trees) => {
            println!("Worktrees:");
            for wt in &trees {
                let branch = wt.branch.as_deref().unwrap_or("(detached)");
                let main_marker = if wt.is_main { " [main]" } else { "" };
                println!("  {} -> {} ({}){}", wt.path.display(), branch, &wt.commit[..7], main_marker);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

::: python Coming from Python
Python's `subprocess` library does not have a built-in concept for parsing multi-record porcelain output. You would typically do `output.split('\n\n')` to separate records and then parse each one. In Rust, we iterate line by line and build up each `WorktreeInfo` struct incrementally. The `Option` types track which fields we have seen so far, and the compiler ensures we handle the case where a field is missing (like `branch` in detached HEAD state).
:::

## Creating Worktrees for Agent Tasks

When the agent starts a new task, it can create a worktree with a dedicated branch. This gives it a completely isolated environment:

```rust
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
pub struct AgentWorktree {
    pub path: PathBuf,
    pub branch: String,
    pub repo_root: PathBuf,
}

impl AgentWorktree {
    /// Create a new worktree for an agent task
    pub fn create(
        repo_path: &Path,
        task_slug: &str,
    ) -> Result<Self, String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            % 100_000;

        let branch_name = format!("agent/{}-{}", task_slug, timestamp);

        // Place the worktree next to the repo, not inside it
        let repo_root = Path::new(
            &run_git_checked(repo_path, &["rev-parse", "--show-toplevel"])?
        ).to_path_buf();

        let worktree_dir = repo_root
            .parent()
            .unwrap_or(Path::new("/tmp"))
            .join(format!(".agent-worktree-{}-{}", task_slug, timestamp));

        // Create the worktree with a new branch based on HEAD
        run_git_checked(
            repo_path,
            &[
                "worktree",
                "add",
                "-b",
                &branch_name,
                worktree_dir.to_str().unwrap_or("/tmp/agent-worktree"),
            ],
        )?;

        Ok(Self {
            path: worktree_dir,
            branch: branch_name,
            repo_root,
        })
    }

    /// Run a git command in this worktree's directory
    pub fn git(&self, args: &[&str]) -> Result<String, String> {
        run_git_checked(&self.path, args)
    }

    /// Remove this worktree and its branch
    pub fn cleanup(&self) -> Result<(), String> {
        // Remove the worktree
        run_git_checked(&self.repo_root, &["worktree", "remove", "--force",
            self.path.to_str().unwrap_or("")])?;

        // Delete the branch
        run_git_checked(&self.repo_root, &["branch", "-D", &self.branch])?;

        Ok(())
    }
}

// Implement Drop to auto-cleanup if the worktree goes out of scope
impl Drop for AgentWorktree {
    fn drop(&mut self) {
        // Best-effort cleanup -- don't panic in drop
        let _ = self.cleanup();
    }
}

fn main() {
    let repo = Path::new(".");

    match AgentWorktree::create(repo, "fix-login-bug") {
        Ok(wt) => {
            println!("Created worktree:");
            println!("  Path: {}", wt.path.display());
            println!("  Branch: {}", wt.branch);

            // The agent can now operate freely in wt.path
            // without affecting the main working directory
            match wt.git(&["status", "--short"]) {
                Ok(status) => println!("  Status: {}", if status.is_empty() { "clean" } else { &status }),
                Err(e) => eprintln!("  Error: {}", e),
            }

            // Cleanup happens automatically when wt is dropped,
            // or you can call wt.cleanup() explicitly
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## The Worktree Lifecycle

A typical agent worktree lifecycle has four phases:

1. **Create** -- the agent creates a worktree when starting a new task.
2. **Work** -- the agent reads and writes files in the worktree directory, stages changes, and creates commits.
3. **Merge** -- when the task is complete and approved, the agent merges the worktree's branch into the main branch.
4. **Cleanup** -- the agent removes the worktree directory and deletes the branch.

Here is the merge step, which needs careful error handling:

```rust
use std::path::{Path, PathBuf};
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

/// Check if a branch can be merged cleanly (dry run)
pub fn can_merge_cleanly(
    repo_path: &Path,
    source_branch: &str,
    target_branch: &str,
) -> Result<bool, String> {
    // Use merge-tree to check for conflicts without modifying anything
    let output = Command::new("git")
        .args(["merge-tree", "--write-tree", "--no-messages", target_branch, source_branch])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("merge-tree failed: {}", e))?;

    // Exit code 0 means clean merge, non-zero means conflicts
    Ok(output.status.success())
}

/// Merge the worktree's branch into the target branch
pub fn merge_worktree_branch(
    main_worktree: &Path,
    branch_name: &str,
    target_branch: &str,
) -> Result<String, String> {
    // First check if merge would be clean
    if !can_merge_cleanly(main_worktree, branch_name, target_branch)? {
        return Err(format!(
            "Branch '{}' has conflicts with '{}'. Cannot auto-merge.",
            branch_name, target_branch
        ));
    }

    // Switch to target branch in the main worktree
    run_git_checked(main_worktree, &["switch", target_branch])?;

    // Perform the merge
    let result = run_git_checked(
        main_worktree,
        &["merge", "--no-ff", branch_name, "-m",
          &format!("Merge agent branch '{}' into {}", branch_name, target_branch)],
    )?;

    Ok(result)
}

fn main() {
    let repo = Path::new(".");

    match can_merge_cleanly(repo, "agent/fix-login-bug-12345", "main") {
        Ok(true) => println!("Clean merge possible"),
        Ok(false) => println!("Conflicts detected -- manual resolution needed"),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

::: wild In the Wild
Codex (OpenAI's CLI agent) uses a sandbox model where the agent runs in an isolated environment. While not using git worktrees specifically, the concept is the same: isolate the agent's changes so they cannot affect the user's work until explicitly merged. This isolation-first approach is a hallmark of safe agent design. Claude Code takes a lighter approach -- it works in the user's main directory but creates git stashes and commits as safety checkpoints.
:::

## Routing Operations to the Right Worktree

When your agent manages multiple worktrees, it needs a way to route file operations to the correct one. Here is a simple registry pattern:

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Tracks active agent worktrees and routes operations
pub struct WorktreeRegistry {
    /// Maps task ID to worktree path
    worktrees: HashMap<String, PathBuf>,
    /// The main repository path (default for operations with no task context)
    main_repo: PathBuf,
}

impl WorktreeRegistry {
    pub fn new(main_repo: PathBuf) -> Self {
        Self {
            worktrees: HashMap::new(),
            main_repo,
        }
    }

    /// Register a new worktree for a task
    pub fn register(&mut self, task_id: String, worktree_path: PathBuf) {
        self.worktrees.insert(task_id, worktree_path);
    }

    /// Remove a worktree registration
    pub fn unregister(&mut self, task_id: &str) -> Option<PathBuf> {
        self.worktrees.remove(task_id)
    }

    /// Get the working directory for a task (falls back to main repo)
    pub fn working_dir(&self, task_id: Option<&str>) -> &Path {
        match task_id {
            Some(id) => self.worktrees.get(id).map(|p| p.as_path()).unwrap_or(&self.main_repo),
            None => &self.main_repo,
        }
    }

    /// Resolve a file path relative to a task's worktree
    pub fn resolve_path(&self, task_id: Option<&str>, relative_path: &str) -> PathBuf {
        self.working_dir(task_id).join(relative_path)
    }

    /// List all active tasks and their worktree paths
    pub fn active_tasks(&self) -> Vec<(&str, &Path)> {
        self.worktrees
            .iter()
            .map(|(id, path)| (id.as_str(), path.as_path()))
            .collect()
    }
}

fn main() {
    let mut registry = WorktreeRegistry::new(PathBuf::from("/home/user/project"));

    registry.register(
        "task-123".to_string(),
        PathBuf::from("/home/user/.agent-worktree-fix-login-12345"),
    );
    registry.register(
        "task-456".to_string(),
        PathBuf::from("/home/user/.agent-worktree-refactor-auth-67890"),
    );

    // Route operations to the correct worktree
    let path = registry.resolve_path(Some("task-123"), "src/main.rs");
    println!("Task 123 main.rs: {}", path.display());
    // Output: /home/user/.agent-worktree-fix-login-12345/src/main.rs

    let path = registry.resolve_path(None, "src/main.rs");
    println!("Default main.rs: {}", path.display());
    // Output: /home/user/project/src/main.rs

    println!("Active tasks:");
    for (id, path) in registry.active_tasks() {
        println!("  {} -> {}", id, path.display());
    }
}
```

## Key Takeaways

- Git worktrees let you check out multiple branches simultaneously in separate directories, sharing one object database -- perfect for parallel agent tasks.
- Place worktree directories outside the main repository (using the parent directory or `/tmp`) to avoid path confusion.
- Implement `Drop` on your worktree type so cleanup happens automatically, even if the agent encounters an error mid-task.
- Always check for merge conflicts with `git merge-tree --write-tree` before attempting to merge a worktree branch back into the main branch.
- Use a registry pattern to route file operations to the correct worktree when the agent handles multiple concurrent tasks.
