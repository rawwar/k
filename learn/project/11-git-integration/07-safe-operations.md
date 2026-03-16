---
title: Safe Operations
description: Designing git operations that avoid data loss by preventing force pushes, hard resets, and destructive rebases, with guardrails for agent-initiated commands.
---

# Safe Operations

> **What you'll learn:**
> - Which git operations are destructive and how to block or gate them in agent workflows
> - How to implement pre-flight checks that verify repository state before risky commands
> - Patterns for making every agent git operation reversible through reflog awareness

Safety is not a feature you add after building git integration -- it is the foundation everything else rests on. An agent that can modify code, create commits, and switch branches has enormous power. Without guardrails, a single bad command can destroy hours of user work. This subchapter establishes the safety principles and concrete mechanisms that prevent your agent from ever losing data.

## The Destructive Operations Blacklist

Some git commands are inherently dangerous because they permanently alter or discard data. Your agent should never run these commands, period:

```rust
/// Git subcommands that should NEVER be executed by the agent
pub const BLOCKED_COMMANDS: &[&str] = &[
    "push --force",
    "push -f",
    "reset --hard",
    "clean -f",
    "clean -fd",
    "clean -fx",
    "checkout .",         // discards all working tree changes
    "checkout -- .",      // same
    "rebase",             // rewrites history
    "filter-branch",      // rewrites history
    "reflog expire",      // deletes reflog entries
    "gc --prune=now",     // garbage collects unreachable objects immediately
];

/// Git subcommands that require explicit user approval before execution
pub const GATED_COMMANDS: &[&str] = &[
    "push",               // sending data to a remote
    "merge",              // combining branches
    "branch -D",          // force-deleting a branch
    "branch -d",          // deleting a branch
    "stash drop",         // discarding a stash entry
    "tag -d",             // deleting a tag
    "remote",             // modifying remote configuration
];
```

Let's build a command validator that enforces these rules:

```rust
use std::path::Path;
use std::process::Command;

#[derive(Debug)]
pub enum GitCommandSafety {
    /// Safe to execute without any approval
    Safe,
    /// Requires user approval before execution
    NeedsApproval(String),
    /// Blocked entirely -- the agent must not run this
    Blocked(String),
}

/// Check if a git command is safe for the agent to execute
pub fn check_command_safety(args: &[&str]) -> GitCommandSafety {
    let command_str = args.join(" ");

    // Check against the blocklist
    let blocked_patterns = [
        "push --force", "push -f",
        "reset --hard",
        "clean -f", "clean -fd", "clean -fx",
        "rebase",
        "filter-branch",
        "reflog expire",
    ];

    for pattern in &blocked_patterns {
        if command_str.contains(pattern) {
            return GitCommandSafety::Blocked(format!(
                "'git {}' is a destructive operation that can cause data loss. \
                 The agent is not allowed to run this command.",
                command_str
            ));
        }
    }

    // Check "checkout ." or "checkout -- ." which discard all changes
    if args.len() >= 2 && args[0] == "checkout" {
        if args.contains(&".") || (args.contains(&"--") && args.last() == Some(&".")) {
            return GitCommandSafety::Blocked(
                "'git checkout .' discards all working tree changes. Use git stash instead."
                    .to_string(),
            );
        }
    }

    // Check against the gated list
    let gated_patterns = [
        "push", "merge", "branch -D", "branch -d", "stash drop", "tag -d", "remote",
    ];

    for pattern in &gated_patterns {
        if command_str.starts_with(pattern) || command_str.contains(&format!(" {}", pattern)) {
            return GitCommandSafety::NeedsApproval(format!(
                "'git {}' modifies shared state or deletes data. Approval required.",
                command_str
            ));
        }
    }

    GitCommandSafety::Safe
}

/// Execute a git command only if it passes safety checks
pub fn safe_git(repo_path: &Path, args: &[&str], user_approved: bool) -> Result<String, String> {
    match check_command_safety(args) {
        GitCommandSafety::Blocked(reason) => Err(reason),
        GitCommandSafety::NeedsApproval(reason) => {
            if user_approved {
                execute_git(repo_path, args)
            } else {
                Err(format!("Approval required: {}", reason))
            }
        }
        GitCommandSafety::Safe => execute_git(repo_path, args),
    }
}

fn execute_git(repo_path: &Path, args: &[&str]) -> Result<String, String> {
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

fn main() {
    let repo = Path::new(".");

    // Safe command -- will execute
    match safe_git(repo, &["status", "--short"], false) {
        Ok(output) => println!("Status: {}", output),
        Err(e) => eprintln!("Error: {}", e),
    }

    // Blocked command -- will be rejected
    match safe_git(repo, &["reset", "--hard", "HEAD~1"], false) {
        Ok(_) => println!("This should not happen"),
        Err(e) => println!("Correctly blocked: {}", e),
    }

    // Gated command without approval -- will be rejected
    match safe_git(repo, &["push", "origin", "main"], false) {
        Ok(_) => println!("This should not happen"),
        Err(e) => println!("Correctly gated: {}", e),
    }

    // Gated command with approval -- will execute
    match safe_git(repo, &["push", "origin", "main"], true) {
        Ok(output) => println!("Push result: {}", output),
        Err(e) => println!("Push failed: {}", e),
    }
}
```

::: python Coming from Python
In Python, you might check commands against a blocklist using `if any(pattern in cmd for pattern in blocked)`. The Rust version does the same thing but encodes the three safety levels into an enum (`Safe`, `NeedsApproval`, `Blocked`). The `match` expression forces you to handle all three cases -- you cannot accidentally forget to check the "needs approval" case, because the compiler will reject it.
:::

## Pre-Flight Checks

Before the agent executes any git operation that modifies state, it should verify that the repository is in a known-good condition. Pre-flight checks catch problems early:

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
pub struct PreFlightResult {
    pub passed: bool,
    pub checks: Vec<(String, bool, String)>, // (check_name, passed, message)
}

impl PreFlightResult {
    pub fn summary(&self) -> String {
        let mut lines = Vec::new();
        for (name, passed, msg) in &self.checks {
            let icon = if *passed { "OK" } else { "FAIL" };
            lines.push(format!("[{}] {}: {}", icon, name, msg));
        }
        lines.join("\n")
    }
}

/// Run pre-flight checks before a mutating operation
pub fn pre_flight_checks(repo_path: &Path) -> Result<PreFlightResult, String> {
    let mut checks = Vec::new();

    // Check 1: Are we in a git repo?
    let in_repo = run_git_checked(repo_path, &["rev-parse", "--is-inside-work-tree"]);
    checks.push((
        "Git repository".to_string(),
        in_repo.is_ok(),
        if in_repo.is_ok() {
            "Inside a git repository".to_string()
        } else {
            "Not inside a git repository".to_string()
        },
    ));

    if in_repo.is_err() {
        return Ok(PreFlightResult {
            passed: false,
            checks,
        });
    }

    // Check 2: Are we on a branch (not detached HEAD)?
    let branch = run_git_checked(repo_path, &["symbolic-ref", "--short", "HEAD"]);
    checks.push((
        "Branch status".to_string(),
        branch.is_ok(),
        match &branch {
            Ok(name) => format!("On branch '{}'", name),
            Err(_) => "HEAD is detached -- commits may be lost".to_string(),
        },
    ));

    // Check 3: Are there any merge conflicts?
    let conflicts = run_git_checked(repo_path, &["diff", "--name-only", "--diff-filter=U"]);
    let has_conflicts = conflicts.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
    checks.push((
        "No merge conflicts".to_string(),
        !has_conflicts,
        if has_conflicts {
            "Unresolved merge conflicts detected".to_string()
        } else {
            "No merge conflicts".to_string()
        },
    ));

    // Check 4: Is the git index locked (another git process running)?
    let git_dir = run_git_checked(repo_path, &["rev-parse", "--git-dir"]);
    let index_locked = if let Ok(dir) = &git_dir {
        Path::new(dir).join("index.lock").exists()
    } else {
        false
    };
    checks.push((
        "Index not locked".to_string(),
        !index_locked,
        if index_locked {
            "index.lock exists -- another git process may be running".to_string()
        } else {
            "Git index is available".to_string()
        },
    ));

    // Check 5: Is there enough disk space for the operation?
    // (simplified check -- just make sure the .git directory is accessible)
    let git_accessible = git_dir.is_ok();
    checks.push((
        "Git directory accessible".to_string(),
        git_accessible,
        if git_accessible {
            "Git directory is accessible".to_string()
        } else {
            "Cannot access .git directory".to_string()
        },
    ));

    let passed = checks.iter().all(|(_, p, _)| *p);
    Ok(PreFlightResult { passed, checks })
}

fn main() {
    let repo = Path::new(".");

    match pre_flight_checks(repo) {
        Ok(result) => {
            println!("Pre-flight checks {}:\n{}",
                if result.passed { "PASSED" } else { "FAILED" },
                result.summary());
        }
        Err(e) => eprintln!("Error running pre-flight checks: {}", e),
    }
}
```

## Creating Safety Checkpoints

Before any risky operation, the agent should save a checkpoint that it can restore if things go wrong. The cheapest checkpoint is a lightweight tag pointing to the current HEAD:

```rust
use std::path::Path;
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

/// Create a safety checkpoint before a risky operation
pub fn create_checkpoint(repo_path: &Path, description: &str) -> Result<String, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let tag_name = format!("agent-checkpoint/{}-{}", description, timestamp);

    // If there are uncommitted changes, stash them
    let status = run_git_checked(repo_path, &["status", "--porcelain"])?;
    let had_changes = !status.is_empty();

    if had_changes {
        run_git_checked(
            repo_path,
            &[
                "stash",
                "push",
                "-m",
                &format!("agent checkpoint: {}", description),
            ],
        )?;
    }

    // Create a tag at the current HEAD
    run_git_checked(
        repo_path,
        &["tag", &tag_name, "-m", &format!("Agent checkpoint: {}", description)],
    )?;

    // Restore the stash if we created one
    if had_changes {
        let _ = run_git_checked(repo_path, &["stash", "pop"]);
    }

    Ok(tag_name)
}

/// Restore to a previously created checkpoint
pub fn restore_checkpoint(repo_path: &Path, tag_name: &str) -> Result<String, String> {
    // Soft reset to the checkpoint -- keeps changes in the working tree
    run_git_checked(repo_path, &["reset", "--soft", tag_name])?;
    Ok(format!("Restored to checkpoint '{}'", tag_name))
}

/// List all agent checkpoints
pub fn list_checkpoints(repo_path: &Path) -> Result<Vec<String>, String> {
    let output = run_git_checked(
        repo_path,
        &["tag", "--list", "agent-checkpoint/*", "--sort=-creatordate"],
    )?;

    Ok(output.lines().map(String::from).collect())
}

/// Clean up old checkpoints (keep the last N)
pub fn cleanup_checkpoints(repo_path: &Path, keep: usize) -> Result<usize, String> {
    let checkpoints = list_checkpoints(repo_path)?;

    if checkpoints.len() <= keep {
        return Ok(0);
    }

    let to_delete = &checkpoints[keep..];
    let mut deleted = 0;

    for tag in to_delete {
        if run_git_checked(repo_path, &["tag", "-d", tag]).is_ok() {
            deleted += 1;
        }
    }

    Ok(deleted)
}

fn main() {
    let repo = Path::new(".");

    // Create a checkpoint before doing something risky
    match create_checkpoint(repo, "before-refactor") {
        Ok(tag) => println!("Checkpoint created: {}", tag),
        Err(e) => eprintln!("Failed to create checkpoint: {}", e),
    }

    // List all checkpoints
    match list_checkpoints(repo) {
        Ok(cps) => {
            println!("Checkpoints:");
            for cp in &cps {
                println!("  {}", cp);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

::: wild In the Wild
Claude Code creates an automatic stash before performing file edits, giving users a one-command rollback if the agent's changes are not what they wanted. The agent does not ask permission to create the checkpoint -- it just does it silently. This "save before you edit" pattern is so important that it happens on every tool invocation, not just when the operation seems risky. The principle is that the cost of a checkpoint (nearly zero) is always less than the cost of lost work.
:::

## The Reflog as a Safety Net

Even if your agent does something unexpected, git's reflog keeps a record of every HEAD movement for 90 days. You can use the reflog to find and recover lost commits:

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
pub struct ReflogEntry {
    pub hash: String,
    pub action: String,
    pub message: String,
}

/// Read recent reflog entries to find recovery points
pub fn recent_reflog(repo_path: &Path, count: usize) -> Result<Vec<ReflogEntry>, String> {
    let output = run_git_checked(
        repo_path,
        &[
            "reflog",
            "--format=%H|%gs",
            &format!("-n{}", count),
        ],
    )?;

    let entries = output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(2, '|').collect();
            if parts.len() == 2 {
                let msg = parts[1].to_string();
                let action = msg.split(':').next().unwrap_or("unknown").to_string();
                Some(ReflogEntry {
                    hash: parts[0].to_string(),
                    action,
                    message: msg,
                })
            } else {
                None
            }
        })
        .collect();

    Ok(entries)
}

fn main() {
    let repo = Path::new(".");

    match recent_reflog(repo, 10) {
        Ok(entries) => {
            println!("Recent reflog (recovery points):");
            for (i, entry) in entries.iter().enumerate() {
                println!("  HEAD@{{{}}} {} -- {}", i, &entry.hash[..7], entry.message);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Key Takeaways

- Maintain a blocklist of destructive git commands (`reset --hard`, `push --force`, `clean -f`, `rebase`) that the agent must never execute.
- Classify commands into three safety tiers: safe (execute freely), gated (require user approval), and blocked (never execute).
- Run pre-flight checks before every mutating operation to verify the repository is in a known-good state.
- Create lightweight tag checkpoints before risky operations so the agent can always roll back to a known state.
- Remember that git's reflog keeps 90 days of HEAD history -- even if something goes wrong, recovery is almost always possible.
