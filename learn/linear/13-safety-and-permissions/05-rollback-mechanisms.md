---
title: Rollback Mechanisms
description: Implement reliable rollback systems that restore codebase state when agent actions produce incorrect or dangerous results.
---

# Rollback Mechanisms

> **What you'll learn:**
> - How to implement multi-level rollback that can undo individual tool calls, entire turns, or complete agent sessions
> - The challenges of rolling back side effects like shell commands, network requests, and file deletions
> - How to design rollback UX that gives users clear visibility into what will be undone and what cannot be reversed

Checkpoints capture state; rollback restores it. While checkpointing is relatively straightforward (create a git commit), rollback is where the complexity lives. Not all actions are reversible. A shell command that sent an email cannot be unsent. A `git push` that published code to a remote repository is visible to others. Your rollback system must clearly distinguish between what it can undo and what it cannot, and communicate this to the user before they make a decision.

## The Rollback Spectrum

Agent actions fall on a spectrum of reversibility:

| Reversibility | Examples | Rollback Strategy |
|---------------|----------|-------------------|
| **Fully reversible** | File writes, file creates, local git commits | `git reset`/`git checkout` to checkpoint |
| **Partially reversible** | File deletes (if checkpointed), directory restructuring | Restore from checkpoint, but permissions/metadata may be lost |
| **Irreversible** | `git push`, `cargo publish`, network requests, emails sent | Cannot undo -- must be prevented or approved beforehand |

Understanding this spectrum is critical because your rollback system should never give the user false confidence that everything can be undone. Let's build a rollback engine that tracks this:

```rust
use std::process::Command;

/// Records a single action taken by the agent, with reversibility metadata.
#[derive(Debug, Clone)]
struct ActionRecord {
    /// Unique identifier for this action
    id: u32,
    /// Which turn this action belongs to
    turn: u32,
    /// Human-readable description
    description: String,
    /// Whether and how this action can be undone
    reversibility: Reversibility,
    /// Git commit hash from the checkpoint before this action
    checkpoint_before: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum Reversibility {
    /// Can be fully undone by restoring the checkpoint
    Full,
    /// Can be partially undone -- some side effects remain
    Partial { remaining_effects: Vec<String> },
    /// Cannot be undone at all
    Irreversible { reason: String },
}

/// The rollback engine tracks all agent actions and performs undo operations.
struct RollbackEngine {
    actions: Vec<ActionRecord>,
    repo_path: String,
    next_id: u32,
}

impl RollbackEngine {
    fn new(repo_path: &str) -> Self {
        Self {
            actions: Vec::new(),
            repo_path: repo_path.to_string(),
            next_id: 1,
        }
    }

    /// Record an action that was taken by the agent.
    fn record_action(
        &mut self,
        turn: u32,
        description: &str,
        reversibility: Reversibility,
        checkpoint: Option<String>,
    ) -> u32 {
        let id = self.next_id;
        self.next_id += 1;

        self.actions.push(ActionRecord {
            id,
            turn,
            description: description.to_string(),
            reversibility,
            checkpoint_before: checkpoint,
        });

        id
    }

    /// Show the user what would happen if they rolled back to a specific point.
    fn preview_rollback(&self, to_action_id: u32) -> RollbackPreview {
        let actions_to_undo: Vec<&ActionRecord> = self
            .actions
            .iter()
            .filter(|a| a.id >= to_action_id)
            .rev()
            .collect();

        let mut reversible = Vec::new();
        let mut irreversible = Vec::new();
        let mut partial = Vec::new();

        for action in &actions_to_undo {
            match &action.reversibility {
                Reversibility::Full => {
                    reversible.push(action.description.clone());
                }
                Reversibility::Partial { remaining_effects } => {
                    partial.push((
                        action.description.clone(),
                        remaining_effects.clone(),
                    ));
                }
                Reversibility::Irreversible { reason } => {
                    irreversible.push((action.description.clone(), reason.clone()));
                }
            }
        }

        // Find the checkpoint to restore to
        let target_checkpoint = self
            .actions
            .iter()
            .find(|a| a.id == to_action_id)
            .and_then(|a| a.checkpoint_before.clone());

        RollbackPreview {
            reversible,
            partial,
            irreversible,
            target_checkpoint,
        }
    }

    /// Execute the rollback by resetting to the checkpoint.
    fn execute_rollback(&mut self, to_action_id: u32) -> Result<RollbackResult, String> {
        let preview = self.preview_rollback(to_action_id);

        if !preview.irreversible.is_empty() {
            println!("WARNING: The following actions CANNOT be undone:");
            for (desc, reason) in &preview.irreversible {
                println!("  - {} ({})", desc, reason);
            }
        }

        let checkpoint = preview.target_checkpoint
            .ok_or("No checkpoint found for this rollback point")?;

        // Perform the git reset to the checkpoint
        let output = Command::new("git")
            .args(["reset", "--hard", &checkpoint])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| format!("Git reset failed: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "Git reset failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Remove rolled-back actions from our record
        self.actions.retain(|a| a.id < to_action_id);

        Ok(RollbackResult {
            actions_undone: preview.reversible.len(),
            actions_partially_undone: preview.partial.len(),
            actions_not_undone: preview.irreversible.len(),
            restored_to: checkpoint,
        })
    }
}

#[derive(Debug)]
struct RollbackPreview {
    reversible: Vec<String>,
    partial: Vec<(String, Vec<String>)>,
    irreversible: Vec<(String, String)>,
    target_checkpoint: Option<String>,
}

#[derive(Debug)]
struct RollbackResult {
    actions_undone: usize,
    actions_partially_undone: usize,
    actions_not_undone: usize,
    restored_to: String,
}

impl std::fmt::Display for RollbackPreview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== Rollback Preview ===")?;

        if !self.reversible.is_empty() {
            writeln!(f, "\nWill be undone:")?;
            for action in &self.reversible {
                writeln!(f, "  [OK] {}", action)?;
            }
        }

        if !self.partial.is_empty() {
            writeln!(f, "\nPartially undone:")?;
            for (action, effects) in &self.partial {
                writeln!(f, "  [~] {}", action)?;
                for effect in effects {
                    writeln!(f, "      Remaining: {}", effect)?;
                }
            }
        }

        if !self.irreversible.is_empty() {
            writeln!(f, "\nCANNOT be undone:")?;
            for (action, reason) in &self.irreversible {
                writeln!(f, "  [X] {} -- {}", action, reason)?;
            }
        }

        Ok(())
    }
}

fn main() {
    let mut engine = RollbackEngine::new(".");

    // Simulate a series of agent actions
    engine.record_action(
        1, "Read src/main.rs", Reversibility::Full,
        Some("abc123".into()),
    );

    engine.record_action(
        1, "Write src/lib.rs (new module)",
        Reversibility::Full,
        Some("abc123".into()),
    );

    engine.record_action(
        2, "Run cargo test",
        Reversibility::Partial {
            remaining_effects: vec!["target/ directory modified".into()],
        },
        Some("def456".into()),
    );

    engine.record_action(
        2, "git push origin feature-branch",
        Reversibility::Irreversible {
            reason: "Code already published to remote".into(),
        },
        Some("def456".into()),
    );

    engine.record_action(
        3, "Write src/main.rs (modified)",
        Reversibility::Full,
        Some("ghi789".into()),
    );

    // Preview rolling back to action 2 (undo actions 2-5)
    let preview = engine.preview_rollback(2);
    println!("{}", preview);
}
```

## Turn-Level Rollback

The most common rollback granularity is at the turn level: "undo everything the agent did in its last response." This is the `/undo` command that users expect:

```rust
/// Provides turn-level undo functionality.
struct TurnRollback {
    /// Maps turn numbers to their starting checkpoint
    turn_checkpoints: Vec<(u32, String)>,
    repo_path: String,
}

impl TurnRollback {
    fn new(repo_path: &str) -> Self {
        Self {
            turn_checkpoints: Vec::new(),
            repo_path: repo_path.to_string(),
        }
    }

    /// Record the checkpoint at the start of a new turn.
    fn start_turn(&mut self, turn_number: u32, checkpoint_hash: &str) {
        self.turn_checkpoints
            .push((turn_number, checkpoint_hash.to_string()));
    }

    /// Undo the last N turns by restoring the appropriate checkpoint.
    fn undo_turns(&self, n: usize) -> Result<String, String> {
        if n > self.turn_checkpoints.len() {
            return Err(format!(
                "Cannot undo {} turns -- only {} recorded",
                n,
                self.turn_checkpoints.len()
            ));
        }

        let target_index = self.turn_checkpoints.len() - n;
        let (turn, checkpoint) = &self.turn_checkpoints[target_index];

        println!(
            "Rolling back {} turn(s) to the state before turn {}",
            n, turn
        );
        println!("Restoring checkpoint: {}", checkpoint);

        // In production, this would execute:
        // git reset --hard <checkpoint>
        Ok(checkpoint.clone())
    }

    /// Show available undo points.
    fn available_undo_points(&self) -> Vec<u32> {
        self.turn_checkpoints.iter().map(|(t, _)| *t).collect()
    }
}

fn main() {
    let mut rollback = TurnRollback::new(".");

    // Simulate recording turn checkpoints
    rollback.start_turn(1, "aaa111");
    rollback.start_turn(2, "bbb222");
    rollback.start_turn(3, "ccc333");
    rollback.start_turn(4, "ddd444");

    println!("Available undo points: {:?}", rollback.available_undo_points());

    // Undo the last 2 turns
    match rollback.undo_turns(2) {
        Ok(hash) => println!("Successfully rolled back to: {}", hash),
        Err(e) => println!("Rollback failed: {}", e),
    }
}
```

::: tip In the Wild
Claude Code provides an `/undo` command that reverts all file changes from the agent's last turn. Internally, it tracks which files were modified and uses git to restore them. If the agent made git commits as part of its work, those commits are also reverted. The key design insight is that undo operates on the user's mental model of "turns" (one user message, one agent response), not on individual tool calls. Codex takes a similar approach -- since it runs in a sandboxed environment, rolling back means simply discarding the worktree changes if the user rejects the proposed modifications.
:::

::: python Coming from Python
In Python, you might implement undo by keeping copies of files before modifying them (similar to the `undo` stack in a text editor). Rust's ownership model makes this pattern cleaner: you can store `String` copies of file contents in a `Vec` without worrying about shared mutable references. But the git-based approach is superior for coding agents because it handles the entire project state atomically -- you never end up in a half-rolled-back state where some files are restored but others are not.
:::

## Handling Non-Reversible Side Effects

The hardest part of rollback is dealing with actions that have already left the system. Let's build a side-effect tracker that clearly marks what cannot be undone:

```rust
/// Track side effects that extend beyond the local filesystem.
#[derive(Debug, Clone)]
struct SideEffect {
    description: String,
    category: SideEffectCategory,
    /// Can this side effect be compensated for (not undone, but mitigated)?
    compensation: Option<String>,
}

#[derive(Debug, Clone)]
enum SideEffectCategory {
    /// Network request already sent
    NetworkRequest { url: String },
    /// Process already executed (may have written to stdout, files, etc.)
    ProcessExecution { command: String, exit_code: i32 },
    /// Git operation that affected remote state
    GitRemote { operation: String },
    /// File deleted without checkpoint
    UncheckpointedDelete { path: String },
}

/// Analyze a rollback plan and identify non-reversible side effects.
fn analyze_side_effects(actions: &[(String, SideEffectCategory)]) -> Vec<SideEffect> {
    actions
        .iter()
        .map(|(desc, category)| {
            let compensation = match category {
                SideEffectCategory::GitRemote { operation } => {
                    if operation.contains("push") {
                        Some("Run `git push --force` to overwrite remote (DANGEROUS)".into())
                    } else {
                        None
                    }
                }
                SideEffectCategory::ProcessExecution { command, .. } => {
                    if command.contains("cargo test") {
                        Some("Test artifacts in target/ can be cleaned with `cargo clean`".into())
                    } else {
                        None
                    }
                }
                _ => None,
            };

            SideEffect {
                description: desc.clone(),
                category: category.clone(),
                compensation,
            }
        })
        .collect()
}

fn main() {
    let actions = vec![
        (
            "Pushed to origin/feature-branch".into(),
            SideEffectCategory::GitRemote {
                operation: "push origin feature-branch".into(),
            },
        ),
        (
            "Ran cargo test (3 tests passed)".into(),
            SideEffectCategory::ProcessExecution {
                command: "cargo test".into(),
                exit_code: 0,
            },
        ),
        (
            "Sent HTTP request to package registry".into(),
            SideEffectCategory::NetworkRequest {
                url: "https://crates.io/api/v1/crates".into(),
            },
        ),
    ];

    let effects = analyze_side_effects(&actions);

    println!("=== Side Effect Analysis ===\n");
    for effect in &effects {
        println!("Effect: {}", effect.description);
        println!("  Category: {:?}", effect.category);
        match &effect.compensation {
            Some(comp) => println!("  Compensation: {}", comp),
            None => println!("  Compensation: NONE AVAILABLE"),
        }
        println!();
    }
}
```

## Key Takeaways

- Rollback systems must clearly distinguish between fully reversible actions (file writes), partially reversible actions (process execution), and irreversible actions (network requests, remote git operations)
- Turn-level rollback matches the user's mental model -- they think in terms of "undo what the agent just did," not "undo tool call #47"
- Always show a rollback preview before executing, so the user understands what will be undone and what cannot be reversed
- Side effects outside the local filesystem (network requests, remote git operations) cannot be truly rolled back, only compensated for -- and the compensation may itself be risky
- The rollback engine should track every agent action with reversibility metadata from the moment it is recorded, not try to figure out reversibility after the fact
