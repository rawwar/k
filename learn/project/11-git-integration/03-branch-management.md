---
title: Branch Management
description: Creating, switching, and deleting branches programmatically, handling detached HEAD states, and implementing branch naming conventions for agent-created branches.
---

# Branch Management

> **What you'll learn:**
> - How to create and switch branches safely from agent code without data loss
> - Strategies for automatic branch naming that communicates agent intent
> - How to detect and recover from detached HEAD and dirty working tree states

Branches are the mechanism that lets your agent work on changes without affecting the main codebase. When a user asks the agent to "refactor the authentication module," the agent should create a branch, do its work there, and only merge when the user approves. This isolation pattern is fundamental to safe agent operations -- it turns every task into a reversible experiment.

## Listing and Inspecting Branches

Before creating a new branch, your agent should know what branches already exist and which one is currently checked out. The `git branch` command with `--format` gives you parseable output:

```rust
use std::path::Path;
use std::process::Command;

#[derive(Debug)]
pub struct BranchInfo {
    pub name: String,
    pub is_current: bool,
    pub commit_hash: String,
    pub upstream: Option<String>,
}

pub fn list_branches(repo_path: &Path) -> Result<Vec<BranchInfo>, String> {
    let output = Command::new("git")
        .args([
            "branch",
            "--format=%(HEAD)|%(refname:short)|%(objectname:short)|%(upstream:short)",
            "--list",
        ])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to list branches: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut branches = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() >= 3 {
            branches.push(BranchInfo {
                is_current: parts[0].trim() == "*",
                name: parts[1].to_string(),
                commit_hash: parts[2].to_string(),
                upstream: if parts.len() > 3 && !parts[3].is_empty() {
                    Some(parts[3].to_string())
                } else {
                    None
                },
            });
        }
    }

    Ok(branches)
}

/// Get just the current branch name
pub fn current_branch(repo_path: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to get current branch: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        // Might be in detached HEAD state
        Err("HEAD is detached (not on any branch)".to_string())
    }
}

fn main() {
    let repo = Path::new(".");

    match current_branch(repo) {
        Ok(name) => println!("Current branch: {}", name),
        Err(e) => println!("{}", e),
    }

    match list_branches(repo) {
        Ok(branches) => {
            for b in &branches {
                let marker = if b.is_current { "* " } else { "  " };
                let upstream = b.upstream.as_deref().unwrap_or("(no upstream)");
                println!("{}{} [{}] -> {}", marker, b.name, b.commit_hash, upstream);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Creating Branches Safely

Creating a branch is simple, but the agent needs to handle several edge cases: the branch name might already exist, the working tree might have uncommitted changes, or the user might be in a detached HEAD state.

```rust
use std::path::Path;
use std::process::Command;

pub fn run_git_checked(repo_path: &Path, args: &[&str]) -> Result<String, String> {
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

pub fn has_uncommitted_changes(repo_path: &Path) -> Result<bool, String> {
    let output = run_git_checked(repo_path, &["status", "--porcelain"])?;
    Ok(!output.is_empty())
}

/// Create a new branch and optionally switch to it
pub fn create_branch(
    repo_path: &Path,
    branch_name: &str,
    switch: bool,
) -> Result<String, String> {
    // Validate branch name (git check-ref-format)
    let check = Command::new("git")
        .args(["check-ref-format", "--branch", branch_name])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to validate branch name: {}", e))?;

    if !check.status.success() {
        return Err(format!("Invalid branch name: '{}'", branch_name));
    }

    // Check if branch already exists
    let exists = Command::new("git")
        .args(["rev-parse", "--verify", &format!("refs/heads/{}", branch_name)])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to check branch: {}", e))?;

    if exists.status.success() {
        return Err(format!("Branch '{}' already exists", branch_name));
    }

    if switch {
        // Check for uncommitted changes that might block checkout
        if has_uncommitted_changes(repo_path)? {
            // Use switch -c which carries uncommitted changes to the new branch
            run_git_checked(repo_path, &["switch", "-c", branch_name])?;
            Ok(format!(
                "Created and switched to '{}' (uncommitted changes preserved)",
                branch_name
            ))
        } else {
            run_git_checked(repo_path, &["switch", "-c", branch_name])?;
            Ok(format!("Created and switched to '{}'", branch_name))
        }
    } else {
        run_git_checked(repo_path, &["branch", branch_name])?;
        Ok(format!("Created branch '{}' (not switched)", branch_name))
    }
}

fn main() {
    let repo = Path::new(".");

    match create_branch(repo, "agent/refactor-auth", true) {
        Ok(msg) => println!("{}", msg),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

::: python Coming from Python
Python's `subprocess.run(["git", "switch", "-c", branch])` would accomplish the same thing, but you would need to manually check the return code: `if result.returncode != 0: raise Exception(result.stderr)`. In Rust, the `run_git_checked` helper does this automatically, and the `Result` type forces every caller to handle the error case -- you cannot accidentally ignore a failure.
:::

## Agent Branch Naming Conventions

When the agent creates branches, the names should communicate intent and avoid colliding with human-created branches. A good naming scheme includes a prefix, a timestamp or ID, and a slug:

```rust
use std::time::{SystemTime, UNIX_EPOCH};

/// Generate a branch name for an agent task
pub fn agent_branch_name(task_description: &str) -> String {
    // Create a slug from the task description
    let slug: String = task_description
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<&str>>()
        .join("-");

    // Truncate slug to keep branch name manageable
    let slug = if slug.len() > 40 {
        &slug[..40]
    } else {
        &slug
    };

    // Add a short timestamp for uniqueness
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let short_ts = timestamp % 100_000; // Last 5 digits

    format!("agent/{}-{}", slug, short_ts)
}

fn main() {
    let branch = agent_branch_name("Refactor authentication module");
    println!("Branch name: {}", branch);
    // Output: agent/refactor-authentication-module-45321

    let branch = agent_branch_name("Fix bug in user login flow");
    println!("Branch name: {}", branch);
    // Output: agent/fix-bug-in-user-login-flow-45321
}
```

The `agent/` prefix makes it immediately obvious which branches were created by the agent versus by a human developer. This is important for cleanup -- the agent or user can easily delete all agent branches with `git branch --list 'agent/*'`.

## Switching Branches Safely

Switching branches is the most dangerous branch operation because it modifies the working tree. If the working tree has uncommitted changes that conflict with the target branch, git will refuse to switch. Your agent needs to detect this situation and handle it gracefully:

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

pub enum SwitchStrategy {
    /// Fail if there are uncommitted changes that would conflict
    Strict,
    /// Stash changes, switch, then apply stash
    StashAndSwitch,
    /// Carry uncommitted changes to the target branch (default git behavior)
    CarryChanges,
}

pub fn switch_branch(
    repo_path: &Path,
    branch_name: &str,
    strategy: SwitchStrategy,
) -> Result<String, String> {
    match strategy {
        SwitchStrategy::Strict => {
            // Check for any uncommitted changes
            let status = run_git_checked(repo_path, &["status", "--porcelain"])?;
            if !status.is_empty() {
                return Err(format!(
                    "Cannot switch to '{}': working tree has uncommitted changes. \
                     Commit or stash changes first.",
                    branch_name
                ));
            }
            run_git_checked(repo_path, &["switch", branch_name])?;
            Ok(format!("Switched to '{}'", branch_name))
        }
        SwitchStrategy::StashAndSwitch => {
            // Stash any uncommitted changes
            let status = run_git_checked(repo_path, &["status", "--porcelain"])?;
            let had_changes = !status.is_empty();

            if had_changes {
                run_git_checked(
                    repo_path,
                    &["stash", "push", "-m", &format!("agent: auto-stash before switching to {}", branch_name)],
                )?;
            }

            // Switch branch
            let switch_result = run_git_checked(repo_path, &["switch", branch_name]);

            if let Err(e) = switch_result {
                // If switch fails, restore stash
                if had_changes {
                    let _ = run_git_checked(repo_path, &["stash", "pop"]);
                }
                return Err(format!("Failed to switch to '{}': {}", branch_name, e));
            }

            if had_changes {
                // Try to apply stash on the new branch
                match run_git_checked(repo_path, &["stash", "pop"]) {
                    Ok(_) => Ok(format!(
                        "Switched to '{}' and restored stashed changes",
                        branch_name
                    )),
                    Err(_) => Ok(format!(
                        "Switched to '{}' but stash could not be applied cleanly. \
                         Changes saved in stash.",
                        branch_name
                    )),
                }
            } else {
                Ok(format!("Switched to '{}'", branch_name))
            }
        }
        SwitchStrategy::CarryChanges => {
            // Let git decide -- it will carry compatible changes or refuse
            run_git_checked(repo_path, &["switch", branch_name])
                .map(|_| format!("Switched to '{}'", branch_name))
                .map_err(|e| format!(
                    "Cannot switch to '{}': {}. Try committing or stashing first.",
                    branch_name, e
                ))
        }
    }
}

fn main() {
    let repo = Path::new(".");

    match switch_branch(repo, "main", SwitchStrategy::StashAndSwitch) {
        Ok(msg) => println!("{}", msg),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

::: wild In the Wild
Claude Code typically works on whatever branch the user has checked out rather than creating its own branches. However, when the user asks for a large refactor, Claude Code will suggest creating a branch first. The agent respects the user's existing git workflow rather than imposing its own. This is a good design principle: let the user control branch strategy, and have the agent provide the tools to execute it.
:::

## Detecting and Handling Detached HEAD

A detached HEAD means the repository is not on any branch -- usually because someone checked out a specific commit hash or tag. Your agent needs to detect this state because creating commits in detached HEAD mode is dangerous (the commits are not reachable from any branch and can be garbage collected):

```rust
use std::path::Path;
use std::process::Command;

#[derive(Debug)]
pub enum HeadState {
    Branch(String),
    Detached(String), // commit hash
}

pub fn get_head_state(repo_path: &Path) -> Result<HeadState, String> {
    let output = Command::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to check HEAD: {}", e))?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(HeadState::Branch(branch))
    } else {
        // HEAD is detached -- get the commit hash instead
        let hash_output = Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(repo_path)
            .output()
            .map_err(|e| format!("Failed to get HEAD hash: {}", e))?;

        let hash = String::from_utf8_lossy(&hash_output.stdout).trim().to_string();
        Ok(HeadState::Detached(hash))
    }
}

/// If HEAD is detached, create a branch to preserve any work
pub fn ensure_on_branch(repo_path: &Path) -> Result<String, String> {
    match get_head_state(repo_path)? {
        HeadState::Branch(name) => Ok(format!("Already on branch '{}'", name)),
        HeadState::Detached(hash) => {
            let branch_name = format!("agent/detached-recovery-{}", &hash);
            let output = Command::new("git")
                .args(["switch", "-c", &branch_name])
                .current_dir(repo_path)
                .output()
                .map_err(|e| format!("Failed to create recovery branch: {}", e))?;

            if output.status.success() {
                Ok(format!(
                    "Was in detached HEAD at {}. Created branch '{}'",
                    hash, branch_name
                ))
            } else {
                Err(format!("Failed to recover from detached HEAD at {}", hash))
            }
        }
    }
}

fn main() {
    let repo = Path::new(".");

    match get_head_state(repo) {
        Ok(HeadState::Branch(name)) => println!("On branch: {}", name),
        Ok(HeadState::Detached(hash)) => {
            println!("Detached HEAD at {}", hash);
            match ensure_on_branch(repo) {
                Ok(msg) => println!("{}", msg),
                Err(e) => eprintln!("{}", e),
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Key Takeaways

- Always validate branch names with `git check-ref-format` and check for existing branches before creating new ones.
- Use an `agent/` prefix for agent-created branches so they are easy to identify and clean up.
- Handle uncommitted changes explicitly when switching branches -- offer stash, carry, or strict strategies depending on the situation.
- Detect detached HEAD state before committing and automatically create a recovery branch to prevent orphaned commits.
- Let users control their branch strategy -- the agent provides tools, not opinions about workflow.
