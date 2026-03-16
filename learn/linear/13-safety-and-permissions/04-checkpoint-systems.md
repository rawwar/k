---
title: Checkpoint Systems
description: Build automatic checkpoint mechanisms that capture the state of a codebase before agent modifications, enabling safe recovery.
---

# Checkpoint Systems

> **What you'll learn:**
> - How to use git commits, stashes, and worktrees as lightweight checkpointing primitives for agent operations
> - Strategies for determining checkpoint granularity -- when to snapshot at the tool level, turn level, or task level
> - How to implement efficient checkpoint storage that avoids bloating the repository with temporary state

Permissions and approval flows prevent the agent from doing things it should not do. But what about things it *should* do that go wrong? The agent might write perfectly valid code that breaks the build, restructure files in a way that does not work, or apply a series of changes where the fifth one creates an error and you need to undo everything back to the beginning. Checkpoint systems are your safety net -- they capture the state of the codebase at known-good points so you can always get back to a working state.

## Git as Your Checkpoint Engine

The most natural checkpoint mechanism for a coding agent is the tool you already use for version control: git. Every git commit is an immutable snapshot of your entire project state. By creating commits at strategic points during agent execution, you build a timeline of checkpoints that you can navigate backward through.

Let's build a checkpoint manager that wraps git operations:

```rust
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Represents a saved checkpoint of the codebase state.
#[derive(Debug, Clone)]
struct Checkpoint {
    /// The git commit hash for this checkpoint
    commit_hash: String,
    /// Human-readable label for this checkpoint
    label: String,
    /// Unix timestamp when the checkpoint was created
    created_at: u64,
    /// What triggered this checkpoint
    trigger: CheckpointTrigger,
}

#[derive(Debug, Clone)]
enum CheckpointTrigger {
    /// Automatic checkpoint before a tool executes
    PreToolExecution { tool_name: String },
    /// Checkpoint at the start of an agent turn
    TurnBoundary { turn_number: u32 },
    /// User explicitly requested a checkpoint
    UserRequested,
    /// Checkpoint before a risky operation
    PreRiskOperation { description: String },
}

/// Manages checkpoint creation and retrieval using git.
struct CheckpointManager {
    /// Path to the git repository
    repo_path: String,
    /// All checkpoints created during this session
    checkpoints: Vec<Checkpoint>,
    /// Prefix for checkpoint branch names
    branch_prefix: String,
}

impl CheckpointManager {
    fn new(repo_path: &str) -> Self {
        Self {
            repo_path: repo_path.to_string(),
            checkpoints: Vec::new(),
            branch_prefix: "agent-checkpoint".to_string(),
        }
    }

    /// Create a checkpoint by committing all current changes.
    fn create_checkpoint(
        &mut self,
        label: &str,
        trigger: CheckpointTrigger,
    ) -> Result<Checkpoint, String> {
        // Stage all changes (including untracked files)
        self.run_git(&["add", "-A"])?;

        // Check if there are actually changes to commit
        let status = self.run_git(&["status", "--porcelain"])?;
        if status.trim().is_empty() {
            // No changes -- create an empty checkpoint marker
            return self.create_empty_checkpoint(label, trigger);
        }

        // Create the checkpoint commit with a structured message
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let message = format!(
            "[agent-checkpoint] {}\nTrigger: {:?}\nTimestamp: {}",
            label, trigger, timestamp
        );

        self.run_git(&["commit", "-m", &message])?;

        // Get the commit hash
        let hash = self.run_git(&["rev-parse", "HEAD"])?;
        let hash = hash.trim().to_string();

        let checkpoint = Checkpoint {
            commit_hash: hash,
            label: label.to_string(),
            created_at: timestamp,
            trigger,
        };

        self.checkpoints.push(checkpoint.clone());
        Ok(checkpoint)
    }

    fn create_empty_checkpoint(
        &mut self,
        label: &str,
        trigger: CheckpointTrigger,
    ) -> Result<Checkpoint, String> {
        let hash = self.run_git(&["rev-parse", "HEAD"])?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let checkpoint = Checkpoint {
            commit_hash: hash.trim().to_string(),
            label: label.to_string(),
            created_at: timestamp,
            trigger,
        };
        self.checkpoints.push(checkpoint.clone());
        Ok(checkpoint)
    }

    /// List all checkpoints created during this session.
    fn list_checkpoints(&self) -> &[Checkpoint] {
        &self.checkpoints
    }

    /// Get the most recent checkpoint.
    fn latest_checkpoint(&self) -> Option<&Checkpoint> {
        self.checkpoints.last()
    }

    /// Helper to run a git command and capture its output.
    fn run_git(&self, args: &[&str]) -> Result<String, String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| format!("Failed to run git: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}

fn main() {
    // In a real agent, this would point to the actual project directory
    let repo_path = ".";
    let mut manager = CheckpointManager::new(repo_path);

    println!("Checkpoint system initialized for: {}", repo_path);
    println!("Checkpoints created this session: {}", manager.list_checkpoints().len());
}
```

## Checkpoint Granularity

One of the most important design decisions is how often to create checkpoints. Too few and you lose important intermediate states. Too many and you clutter the git history and slow down execution.

There are three common granularity levels:

### Tool-Level Checkpoints

Create a checkpoint before every tool invocation that modifies state (file writes, shell commands). This gives you maximum rollback precision but generates many commits:

```rust
/// Wraps tool execution with automatic checkpointing.
struct CheckpointedToolExecutor {
    checkpoint_manager: CheckpointManager,
}

/// Represents the outcome of a tool invocation.
#[derive(Debug)]
struct ToolResult {
    success: bool,
    output: String,
}

/// Categories of tools, used to decide checkpoint strategy.
#[derive(Debug, Clone)]
enum ToolKind {
    ReadOnly,
    Mutating,
    Destructive,
}

impl CheckpointedToolExecutor {
    fn new(repo_path: &str) -> Self {
        Self {
            checkpoint_manager: CheckpointManager::new(repo_path),
        }
    }

    fn execute_tool(
        &mut self,
        tool_name: &str,
        tool_kind: ToolKind,
        execute_fn: impl FnOnce() -> ToolResult,
    ) -> Result<ToolResult, String> {
        // Only checkpoint before mutating or destructive operations
        match tool_kind {
            ToolKind::ReadOnly => {
                // No checkpoint needed for reads
                Ok(execute_fn())
            }
            ToolKind::Mutating => {
                // Create a checkpoint before the mutation
                let cp = self.checkpoint_manager.create_checkpoint(
                    &format!("Before {}", tool_name),
                    CheckpointTrigger::PreToolExecution {
                        tool_name: tool_name.to_string(),
                    },
                )?;
                println!("Checkpoint created: {} ({})", cp.label, &cp.commit_hash[..8]);
                Ok(execute_fn())
            }
            ToolKind::Destructive => {
                // For destructive ops, checkpoint and log prominently
                let cp = self.checkpoint_manager.create_checkpoint(
                    &format!("SAFETY: Before destructive {}", tool_name),
                    CheckpointTrigger::PreRiskOperation {
                        description: format!("Destructive tool: {}", tool_name),
                    },
                )?;
                println!(
                    "SAFETY CHECKPOINT: {} ({})",
                    cp.label, &cp.commit_hash[..8]
                );
                Ok(execute_fn())
            }
        }
    }
}

// Re-declare the types needed for compilation
struct CheckpointManager {
    repo_path: String,
    checkpoints: Vec<Checkpoint>,
}

#[derive(Debug, Clone)]
struct Checkpoint {
    commit_hash: String,
    label: String,
    created_at: u64,
    trigger: CheckpointTrigger,
}

#[derive(Debug, Clone)]
enum CheckpointTrigger {
    PreToolExecution { tool_name: String },
    TurnBoundary { turn_number: u32 },
    UserRequested,
    PreRiskOperation { description: String },
}

impl CheckpointManager {
    fn new(repo_path: &str) -> Self {
        Self {
            repo_path: repo_path.to_string(),
            checkpoints: Vec::new(),
        }
    }

    fn create_checkpoint(
        &mut self,
        label: &str,
        trigger: CheckpointTrigger,
    ) -> Result<Checkpoint, String> {
        let checkpoint = Checkpoint {
            commit_hash: "abc12345".to_string(),
            label: label.to_string(),
            created_at: 0,
            trigger,
        };
        self.checkpoints.push(checkpoint.clone());
        Ok(checkpoint)
    }
}

fn main() {
    let mut executor = CheckpointedToolExecutor::new(".");

    // Simulate a read-only tool -- no checkpoint created
    let result = executor.execute_tool("read_file", ToolKind::ReadOnly, || {
        ToolResult {
            success: true,
            output: "file contents...".into(),
        }
    });
    println!("Read result: {:?}", result);

    // Simulate a mutating tool -- checkpoint created before execution
    let result = executor.execute_tool("write_file", ToolKind::Mutating, || {
        ToolResult {
            success: true,
            output: "wrote 42 bytes".into(),
        }
    });
    println!("Write result: {:?}", result);

    // Simulate a destructive tool -- safety checkpoint created
    let result = executor.execute_tool("shell_rm", ToolKind::Destructive, || {
        ToolResult {
            success: true,
            output: "removed temp files".into(),
        }
    });
    println!("Destructive result: {:?}", result);
}
```

### Turn-Level Checkpoints

Create a checkpoint at the boundary of each agent turn (one user message and the agent's complete response). This is less granular but produces a cleaner history:

```rust
/// Manages turn-level checkpointing in the agent loop.
struct TurnCheckpointer {
    current_turn: u32,
    checkpoints: Vec<(u32, String)>, // (turn_number, commit_hash)
}

impl TurnCheckpointer {
    fn new() -> Self {
        Self {
            current_turn: 0,
            checkpoints: Vec::new(),
        }
    }

    /// Called at the start of each new turn.
    fn begin_turn(&mut self) -> u32 {
        self.current_turn += 1;
        println!("--- Turn {} begins ---", self.current_turn);
        // In a real implementation, this would create a git commit
        let fake_hash = format!("turn-{}-checkpoint", self.current_turn);
        self.checkpoints.push((self.current_turn, fake_hash));
        self.current_turn
    }

    /// Get the checkpoint for a specific turn, enabling rollback to any turn.
    fn checkpoint_for_turn(&self, turn: u32) -> Option<&str> {
        self.checkpoints
            .iter()
            .find(|(t, _)| *t == turn)
            .map(|(_, hash)| hash.as_str())
    }

    /// Get all turns that have checkpoints (for displaying to the user).
    fn available_rollback_points(&self) -> Vec<u32> {
        self.checkpoints.iter().map(|(t, _)| *t).collect()
    }
}

fn main() {
    let mut checkpointer = TurnCheckpointer::new();

    // Simulate three turns of agent interaction
    checkpointer.begin_turn(); // Turn 1
    println!("Agent: Reading project structure...\n");

    checkpointer.begin_turn(); // Turn 2
    println!("Agent: Writing new module...\n");

    checkpointer.begin_turn(); // Turn 3
    println!("Agent: Refactoring imports...\n");

    // User wants to undo the last two turns
    println!("Available rollback points: {:?}", checkpointer.available_rollback_points());
    if let Some(hash) = checkpointer.checkpoint_for_turn(1) {
        println!("To undo turns 2 and 3, rollback to: {}", hash);
    }
}
```

## Using Git Stash for Lightweight Checkpoints

For very frequent checkpoints (like before every file write), creating full commits can be noisy. Git stash provides a lighter-weight alternative:

```rust
use std::process::Command;

/// A stash-based checkpoint that does not pollute the commit history.
struct StashCheckpoint {
    stash_index: usize,
    label: String,
}

/// Use git stash for lightweight, temporary checkpoints.
fn create_stash_checkpoint(repo_path: &str, label: &str) -> Result<StashCheckpoint, String> {
    // First, stage everything
    run_git(repo_path, &["add", "-A"])?;

    // Create a stash with a descriptive message
    let message = format!("[agent-cp] {}", label);
    let output = run_git(repo_path, &["stash", "push", "-m", &message])?;

    if output.contains("No local changes") {
        return Err("No changes to stash".into());
    }

    // Immediately re-apply the stash so work continues
    // The stash remains in the stash list as a checkpoint
    run_git(repo_path, &["stash", "apply"])?;

    Ok(StashCheckpoint {
        stash_index: 0, // Most recent stash
        label: label.to_string(),
    })
}

fn run_git(repo_path: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Git error: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

fn main() {
    println!("Stash-based checkpointing:");
    println!("  1. git add -A           (stage all changes)");
    println!("  2. git stash push -m .. (save checkpoint)");
    println!("  3. git stash apply      (restore working state)");
    println!("  4. Stash remains as a rollback point");
    println!("\nTo rollback: git stash pop (restores stashed state)");
    println!("To list:    git stash list (shows all checkpoints)");
}
```

::: wild In the Wild
Claude Code creates checkpoints using git commits on a dedicated branch pattern. Before executing a series of changes, it captures the current state so the user can undo the entire operation. Codex runs in an isolated environment and uses git worktrees to maintain a clean separation between the agent's changes and the user's working tree -- if the agent's changes are rejected, the worktree is simply discarded without affecting the main working directory.
:::

::: python Coming from Python
Python developers might reach for `shutil.copytree()` to snapshot a directory before making changes. While this works, it is extremely wasteful for large projects -- copying gigabytes of node_modules or target directories just to save state. Git's content-addressable storage is far more efficient because it only stores the *differences* between snapshots. Rust's strong error handling also makes git operations more reliable -- every `Command` call returns a `Result` that you handle explicitly, whereas Python's `subprocess.run()` can silently succeed with a nonzero exit code.
:::

## Cleanup: Preventing Checkpoint Bloat

Agent checkpoints are temporary -- they exist to support rollback during and shortly after a session, not as permanent history. You need a cleanup strategy:

```rust
/// Clean up agent checkpoints after a session completes successfully.
fn cleanup_checkpoints(repo_path: &str, keep_last_n: usize) -> Result<(), String> {
    // Squash all agent checkpoint commits into one
    // This preserves the final state while removing intermediate noise
    let log = run_git(repo_path, &["log", "--oneline", "-50"])?;

    let agent_commits: Vec<&str> = log
        .lines()
        .filter(|line| line.contains("[agent-checkpoint]"))
        .collect();

    println!("Found {} agent checkpoint commits", agent_commits.len());

    if agent_commits.len() > keep_last_n {
        println!(
            "Would squash {} old checkpoints (keeping last {})",
            agent_commits.len() - keep_last_n,
            keep_last_n
        );
        // In production, you would use:
        // git rebase -i to squash old checkpoint commits
        // or git reset --soft to collapse them into one commit
    }

    // Also clean up stashes
    let stash_list = run_git(repo_path, &["stash", "list"])?;
    let agent_stashes: Vec<&str> = stash_list
        .lines()
        .filter(|line| line.contains("[agent-cp]"))
        .collect();

    println!("Found {} agent stash entries", agent_stashes.len());

    Ok(())
}

fn run_git(repo_path: &str, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Git error: {}", e))?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn main() {
    match cleanup_checkpoints(".", 3) {
        Ok(()) => println!("Checkpoint cleanup complete"),
        Err(e) => println!("Cleanup error: {}", e),
    }
}
```

## Key Takeaways

- Git commits are the most natural checkpoint mechanism for coding agents because they integrate with the existing version control workflow and provide efficient storage through content-addressable deduplication
- Checkpoint granularity involves a tradeoff: tool-level checkpoints give maximum rollback precision but clutter history, while turn-level checkpoints are cleaner but less granular
- Git stash provides a lightweight alternative for very frequent checkpoints that should not pollute the commit log
- Always create checkpoints *before* executing mutating or destructive operations, not after -- you need the pre-modification state to rollback to
- Checkpoint cleanup is essential to prevent repository bloat; squash or delete agent checkpoints after the session completes successfully
