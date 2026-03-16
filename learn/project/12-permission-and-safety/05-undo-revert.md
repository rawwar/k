---
title: Undo and Revert
description: Building undo and revert commands that restore files from checkpoints, with support for reverting a single operation, an entire turn, or all changes in a session.
---

# Undo and Revert

> **What you'll learn:**
> - How to implement undo at different granularities: single file, single tool call, or full turn
> - How to handle undo of operations that have downstream dependencies
> - Patterns for communicating revert results back to the LLM so it can adjust its plan

You built the checkpoint system in the previous subchapter. Now you need the user-facing commands that put it to work. "Undo" is deceptively simple as a concept — the complexity lies in deciding *what* to undo and handling the cascading effects when later operations depend on earlier ones.

This subchapter builds three undo commands with increasing scope: undo the last operation, undo an entire turn, and undo all changes in the session. You will also handle the tricky case of informing the LLM about reverts so it does not try to build on top of undone work.

## Undo Granularity

Let's define the three levels of undo that cover the most common user needs:

```rust
use std::path::PathBuf;

/// The scope of an undo operation.
#[derive(Debug, Clone)]
pub enum UndoScope {
    /// Undo the most recent tool invocation.
    LastOperation,
    /// Undo a specific checkpoint by ID.
    Specific(u64),
    /// Undo all checkpoints from a specific conversation turn.
    Turn(u64),
    /// Undo all checkpoints from the entire session.
    Session,
}

/// The result of an undo operation.
#[derive(Debug)]
pub struct UndoResult {
    /// Checkpoints that were reverted.
    pub reverted_checkpoints: Vec<u64>,
    /// Files that were restored to their original state.
    pub restored_files: Vec<PathBuf>,
    /// Files that could not be restored (with error messages).
    pub failed_files: Vec<(PathBuf, String)>,
    /// Human-readable summary for the user.
    pub summary: String,
}

impl UndoResult {
    pub fn is_success(&self) -> bool {
        self.failed_files.is_empty() && !self.reverted_checkpoints.is_empty()
    }
}
```

## The Undo Engine

The `UndoEngine` wraps the `CheckpointManager` and adds undo logic. The key insight is that when you undo multiple checkpoints, you need to process them in *reverse chronological order* — newest first — to avoid conflicts:

```rust
use std::collections::HashSet;

/// Engine that performs undo operations using the checkpoint system.
pub struct UndoEngine<'a> {
    checkpoint_mgr: &'a CheckpointManager,
}

impl<'a> UndoEngine<'a> {
    pub fn new(checkpoint_mgr: &'a CheckpointManager) -> Self {
        Self { checkpoint_mgr }
    }

    /// Perform an undo operation with the given scope.
    pub fn undo(&self, scope: UndoScope) -> UndoResult {
        let checkpoints_to_revert = match &scope {
            UndoScope::LastOperation => {
                match self.checkpoint_mgr.latest() {
                    Some(cp) => vec![cp],
                    None => {
                        return UndoResult {
                            reverted_checkpoints: vec![],
                            restored_files: vec![],
                            failed_files: vec![],
                            summary: "Nothing to undo — no checkpoints exist.".to_string(),
                        };
                    }
                }
            }
            UndoScope::Specific(id) => {
                match self.checkpoint_mgr.all_checkpoints().iter().find(|c| c.id == *id) {
                    Some(cp) => vec![cp],
                    None => {
                        return UndoResult {
                            reverted_checkpoints: vec![],
                            restored_files: vec![],
                            failed_files: vec![],
                            summary: format!("Checkpoint {} not found.", id),
                        };
                    }
                }
            }
            UndoScope::Turn(turn_id) => {
                let mut cps: Vec<&Checkpoint> = self
                    .checkpoint_mgr
                    .checkpoints_for_turn(*turn_id);
                // Reverse chronological order
                cps.reverse();
                cps
            }
            UndoScope::Session => {
                let mut cps: Vec<&Checkpoint> = self
                    .checkpoint_mgr
                    .all_checkpoints()
                    .iter()
                    .collect();
                cps.reverse();
                cps
            }
        };

        self.execute_reverts(&checkpoints_to_revert, &scope)
    }

    fn execute_reverts(
        &self,
        checkpoints: &[&Checkpoint],
        scope: &UndoScope,
    ) -> UndoResult {
        let mut reverted_ids = Vec::new();
        let mut restored_files = Vec::new();
        let mut failed_files = Vec::new();
        let mut seen_paths = HashSet::new();

        for checkpoint in checkpoints {
            match self.checkpoint_mgr.restore_checkpoint(checkpoint.id) {
                Ok(report) => {
                    reverted_ids.push(checkpoint.id);
                    for path in report.restored {
                        if seen_paths.insert(path.clone()) {
                            restored_files.push(path);
                        }
                    }
                    for (path, err) in report.errors {
                        failed_files.push((path, err));
                    }
                }
                Err(e) => {
                    failed_files.push((
                        PathBuf::from(format!("checkpoint-{}", checkpoint.id)),
                        e.to_string(),
                    ));
                }
            }
        }

        let summary = match scope {
            UndoScope::LastOperation => {
                format!(
                    "Undid last operation: {} file(s) restored.",
                    restored_files.len()
                )
            }
            UndoScope::Specific(id) => {
                format!(
                    "Undid checkpoint {}: {} file(s) restored.",
                    id,
                    restored_files.len()
                )
            }
            UndoScope::Turn(turn_id) => {
                format!(
                    "Undid turn {}: {} checkpoint(s) reverted, {} file(s) restored.",
                    turn_id,
                    reverted_ids.len(),
                    restored_files.len()
                )
            }
            UndoScope::Session => {
                format!(
                    "Undid all session changes: {} checkpoint(s) reverted, {} file(s) restored.",
                    reverted_ids.len(),
                    restored_files.len()
                )
            }
        };

        UndoResult {
            reverted_checkpoints: reverted_ids,
            restored_files,
            failed_files,
            summary,
        }
    }
}
```

::: python Coming from Python
Python's dynamic typing makes undo systems tempting to implement with a simple list of closures:
```python
undo_stack = []
# Before each operation:
undo_stack.append(lambda: restore_file(path, old_content))
# To undo:
undo_stack.pop()()
```
This works for prototyping, but it breaks down when you need to inspect what will be undone (closures are opaque) or when you need to serialize undo state. The Rust approach with explicit `Checkpoint` structs makes the undo state inspectable, testable, and serializable — at the cost of more upfront structure.
:::

## Handling Dependency Chains

The tricky part of undo is when later operations depend on earlier ones. Consider this sequence:

1. Agent creates `src/utils.rs` with a helper function.
2. Agent modifies `src/main.rs` to import from `utils.rs`.
3. User undoes step 1.

Now `src/main.rs` has an import that references a file that no longer exists. The code is broken. How should the undo system handle this?

There are two strategies, and you should support both:

```rust
/// Strategy for handling dependencies during undo.
#[derive(Debug, Clone, Copy)]
pub enum UndoDependencyStrategy {
    /// Undo only the requested operation. The user is responsible
    /// for fixing any breakage. Fast and predictable.
    Strict,
    /// When undoing an operation, also undo all operations that
    /// came after it. Safer but more disruptive.
    Cascade,
}

/// Extended undo that respects dependency strategies.
pub fn undo_with_strategy(
    engine: &UndoEngine,
    checkpoint_mgr: &CheckpointManager,
    checkpoint_id: u64,
    strategy: UndoDependencyStrategy,
) -> UndoResult {
    match strategy {
        UndoDependencyStrategy::Strict => {
            engine.undo(UndoScope::Specific(checkpoint_id))
        }
        UndoDependencyStrategy::Cascade => {
            // Find all checkpoints after the target and undo them too
            let all = checkpoint_mgr.all_checkpoints();
            let target_idx = all.iter().position(|c| c.id == checkpoint_id);

            match target_idx {
                Some(idx) => {
                    // Undo from newest to the target (inclusive)
                    let ids: Vec<u64> = all[idx..].iter().rev().map(|c| c.id).collect();
                    let mut combined = UndoResult {
                        reverted_checkpoints: Vec::new(),
                        restored_files: Vec::new(),
                        failed_files: Vec::new(),
                        summary: String::new(),
                    };

                    for id in ids {
                        let result = engine.undo(UndoScope::Specific(id));
                        combined.reverted_checkpoints.extend(result.reverted_checkpoints);
                        combined.restored_files.extend(result.restored_files);
                        combined.failed_files.extend(result.failed_files);
                    }

                    combined.summary = format!(
                        "Cascade undo: {} checkpoint(s) reverted, {} file(s) restored.",
                        combined.reverted_checkpoints.len(),
                        combined.restored_files.len()
                    );
                    combined
                }
                None => UndoResult {
                    reverted_checkpoints: vec![],
                    restored_files: vec![],
                    failed_files: vec![],
                    summary: format!("Checkpoint {} not found.", checkpoint_id),
                },
            }
        }
    }
}
```

## Communicating Reverts to the LLM

When the user undoes an agent operation, the LLM needs to know. Otherwise it will continue planning as if the undone changes are still in place, leading to confusion and errors. The best approach is to inject a system message into the conversation:

```rust
/// Generate a message to inject into the conversation after an undo.
pub fn generate_undo_message(result: &UndoResult) -> String {
    let mut message = String::new();

    message.push_str("[SYSTEM] The user has undone recent changes.\n\n");
    message.push_str(&result.summary);
    message.push('\n');

    if !result.restored_files.is_empty() {
        message.push_str("\nRestored files:\n");
        for path in &result.restored_files {
            message.push_str(&format!("  - {}\n", path.display()));
        }
    }

    if !result.failed_files.is_empty() {
        message.push_str("\nFailed to restore:\n");
        for (path, err) in &result.failed_files {
            message.push_str(&format!("  - {}: {}\n", path.display(), err));
        }
    }

    message.push_str("\nPlease re-read any affected files before making ");
    message.push_str("further changes. Do not assume the previous content ");
    message.push_str("is still present.");

    message
}
```

The final instruction — "do not assume the previous content is still present" — is crucial. It forces the LLM to re-read files rather than relying on its memory of what it wrote, which may no longer be accurate after an undo.

::: wild In the Wild
Claude Code handles undo by reverting to the git checkpoint commit and then injecting a message into the conversation telling the model what was undone. The model sees something like "The user has undone the changes to src/main.rs" and adjusts its plan accordingly. This is effective because the model already understands file state and can re-read files to confirm the current content.
:::

## User-Facing Undo Commands

Let's build the user commands that drive the undo system. These would be triggered by the user typing `/undo` in the REPL:

```rust
/// Parse an undo command from user input.
pub fn parse_undo_command(input: &str) -> Option<UndoScope> {
    let trimmed = input.trim();

    if trimmed == "/undo" {
        return Some(UndoScope::LastOperation);
    }

    if trimmed == "/undo all" || trimmed == "/undo session" {
        return Some(UndoScope::Session);
    }

    if let Some(rest) = trimmed.strip_prefix("/undo turn ") {
        if let Ok(turn_id) = rest.trim().parse::<u64>() {
            return Some(UndoScope::Turn(turn_id));
        }
    }

    if let Some(rest) = trimmed.strip_prefix("/undo ") {
        if let Ok(checkpoint_id) = rest.trim().parse::<u64>() {
            return Some(UndoScope::Specific(checkpoint_id));
        }
    }

    None
}

fn main() {
    // Demonstrate command parsing
    let commands = vec![
        "/undo",
        "/undo all",
        "/undo turn 3",
        "/undo 42",
        "/undo session",
        "not an undo command",
    ];

    for cmd in commands {
        match parse_undo_command(cmd) {
            Some(scope) => println!("{:?} => {:?}", cmd, scope),
            None => println!("{:?} => not an undo command", cmd),
        }
    }
}
```

## Showing Undo History

Users need to see what they can undo. A `/history` command that lists recent checkpoints helps them make informed choices:

```rust
/// Format checkpoint history for display to the user.
pub fn format_checkpoint_history(
    checkpoint_mgr: &CheckpointManager,
    max_entries: usize,
) -> String {
    let checkpoints = checkpoint_mgr.all_checkpoints();

    if checkpoints.is_empty() {
        return "No checkpoints recorded yet.".to_string();
    }

    let mut output = String::from("Recent checkpoints (newest first):\n\n");
    let start = if checkpoints.len() > max_entries {
        checkpoints.len() - max_entries
    } else {
        0
    };

    for checkpoint in checkpoints[start..].iter().rev() {
        let file_count = checkpoint.snapshots.len();
        let paths: Vec<String> = checkpoint
            .affected_paths()
            .iter()
            .map(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| p.display().to_string())
            })
            .collect();

        output.push_str(&format!(
            "  [{}] Turn {} | {} | {} file(s): {}\n",
            checkpoint.id,
            checkpoint.turn_id,
            checkpoint.tool_name,
            file_count,
            paths.join(", ")
        ));
        output.push_str(&format!("       {}\n", checkpoint.description));
    }

    output.push_str(&format!(
        "\nUse /undo <id> to revert a specific checkpoint, or /undo to revert the last one."
    ));

    output
}
```

## Key Takeaways

- Undo operates at three granularities — last operation, specific checkpoint, and full turn/session — covering the most common user needs.
- When undoing multiple checkpoints, process them in reverse chronological order to avoid conflicts between overlapping file changes.
- Cascade undo (reverting everything after a given checkpoint) is safer but more disruptive than strict undo (reverting only the targeted checkpoint); support both strategies.
- Always inject a message into the conversation after an undo so the LLM knows the file state has changed and re-reads affected files instead of relying on stale memory.
- User-facing commands like `/undo`, `/undo turn 3`, and `/undo all` provide an intuitive interface that maps directly to the undo scope model.
