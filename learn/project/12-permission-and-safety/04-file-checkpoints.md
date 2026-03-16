---
title: File Checkpoints
description: Implementing a checkpoint system that snapshots file state before modifications, enabling precise rollback of individual tool invocations or entire conversation turns.
---

# File Checkpoints

> **What you'll learn:**
> - How to capture file content snapshots before write operations for rollback capability
> - Strategies for efficient checkpoint storage using git stashes or shadow copies
> - How to organize checkpoints by turn and tool invocation for granular undo

The threat model identified accidental destruction as the highest-risk threat for coding agents. Files will get overwritten with incorrect content — not because the agent is malicious, but because LLMs make mistakes. The only reliable defense is the ability to undo: capture the state before every modification so you can restore it when things go wrong.

File checkpoints are the foundation of that undo capability. They record what a file looked like *before* the agent changed it, organized in a way that lets you revert a single edit, an entire turn, or all changes in a session.

## Checkpoint Design

A checkpoint needs to answer three questions: *what* file was changed, *when* it was changed (which turn and tool invocation), and *what was the original content*. Let's model this:

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A snapshot of a single file's content before modification.
#[derive(Debug, Clone)]
pub struct FileSnapshot {
    /// Absolute path to the file.
    pub path: PathBuf,
    /// Content before the modification. None means the file did not exist.
    pub original_content: Option<String>,
    /// When the snapshot was taken.
    pub timestamp: Instant,
}

/// A checkpoint groups all file snapshots from a single tool invocation.
#[derive(Debug, Clone)]
pub struct Checkpoint {
    /// Unique identifier for this checkpoint.
    pub id: u64,
    /// Which conversation turn triggered this checkpoint.
    pub turn_id: u64,
    /// Which tool call within the turn.
    pub tool_call_id: String,
    /// The tool that was invoked.
    pub tool_name: String,
    /// Human-readable description of what the tool did.
    pub description: String,
    /// File snapshots captured before the tool executed.
    pub snapshots: Vec<FileSnapshot>,
    /// When the checkpoint was created.
    pub created_at: Instant,
}

impl Checkpoint {
    /// Get all file paths affected by this checkpoint.
    pub fn affected_paths(&self) -> Vec<&Path> {
        self.snapshots.iter().map(|s| s.path.as_path()).collect()
    }
}
```

::: python Coming from Python
In Python, you might implement checkpoints with a simple dictionary mapping file paths to their previous content:
```python
checkpoints = {}
def save_checkpoint(path):
    if os.path.exists(path):
        checkpoints[path] = open(path).read()
    else:
        checkpoints[path] = None  # file didn't exist
```
The Rust version is more structured — using typed structs instead of dictionaries — but the core idea is identical. The key difference is that Rust's `PathBuf` handles path manipulation safely across operating systems, while Python's `os.path` has some cross-platform edge cases.
:::

## The Checkpoint Manager

The `CheckpointManager` is the central piece. It creates checkpoints before tool execution and provides methods to query and restore them:

```rust
use std::fs;

/// Manages file checkpoints for undo/revert capability.
pub struct CheckpointManager {
    /// All checkpoints in chronological order.
    checkpoints: Vec<Checkpoint>,
    /// Next checkpoint ID.
    next_id: u64,
    /// Maximum number of checkpoints to retain.
    max_checkpoints: usize,
}

impl CheckpointManager {
    pub fn new(max_checkpoints: usize) -> Self {
        Self {
            checkpoints: Vec::new(),
            next_id: 1,
            max_checkpoints,
        }
    }

    /// Create a checkpoint before modifying the given files.
    /// Call this BEFORE the tool writes anything.
    pub fn create_checkpoint(
        &mut self,
        turn_id: u64,
        tool_call_id: &str,
        tool_name: &str,
        description: &str,
        paths: &[&Path],
    ) -> Result<u64, CheckpointError> {
        let mut snapshots = Vec::new();

        for path in paths {
            let original_content = if path.exists() {
                Some(fs::read_to_string(path).map_err(|e| {
                    CheckpointError::ReadFailed {
                        path: path.to_path_buf(),
                        source: e.to_string(),
                    }
                })?)
            } else {
                None
            };

            snapshots.push(FileSnapshot {
                path: path.to_path_buf(),
                original_content,
                timestamp: Instant::now(),
            });
        }

        let id = self.next_id;
        self.next_id += 1;

        let checkpoint = Checkpoint {
            id,
            turn_id,
            tool_call_id: tool_call_id.to_string(),
            tool_name: tool_name.to_string(),
            description: description.to_string(),
            snapshots,
            created_at: Instant::now(),
        };

        self.checkpoints.push(checkpoint);

        // Evict old checkpoints if over the limit
        if self.checkpoints.len() > self.max_checkpoints {
            let remove_count = self.checkpoints.len() - self.max_checkpoints;
            self.checkpoints.drain(..remove_count);
        }

        Ok(id)
    }

    /// Restore files from a specific checkpoint.
    pub fn restore_checkpoint(&self, checkpoint_id: u64) -> Result<RestoreReport, CheckpointError> {
        let checkpoint = self
            .checkpoints
            .iter()
            .find(|c| c.id == checkpoint_id)
            .ok_or(CheckpointError::NotFound { id: checkpoint_id })?;

        let mut restored = Vec::new();
        let mut errors = Vec::new();

        for snapshot in &checkpoint.snapshots {
            match &snapshot.original_content {
                Some(content) => {
                    // File existed before — restore its content
                    match fs::write(&snapshot.path, content) {
                        Ok(()) => restored.push(snapshot.path.clone()),
                        Err(e) => errors.push((snapshot.path.clone(), e.to_string())),
                    }
                }
                None => {
                    // File did not exist before — delete it
                    if snapshot.path.exists() {
                        match fs::remove_file(&snapshot.path) {
                            Ok(()) => restored.push(snapshot.path.clone()),
                            Err(e) => errors.push((snapshot.path.clone(), e.to_string())),
                        }
                    }
                }
            }
        }

        Ok(RestoreReport { restored, errors })
    }

    /// Get all checkpoints for a specific turn.
    pub fn checkpoints_for_turn(&self, turn_id: u64) -> Vec<&Checkpoint> {
        self.checkpoints
            .iter()
            .filter(|c| c.turn_id == turn_id)
            .collect()
    }

    /// Get the most recent checkpoint.
    pub fn latest(&self) -> Option<&Checkpoint> {
        self.checkpoints.last()
    }

    /// Get all checkpoints in chronological order.
    pub fn all_checkpoints(&self) -> &[Checkpoint] {
        &self.checkpoints
    }
}

/// Report of a restore operation.
#[derive(Debug)]
pub struct RestoreReport {
    pub restored: Vec<PathBuf>,
    pub errors: Vec<(PathBuf, String)>,
}

impl RestoreReport {
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }
}

impl std::fmt::Display for RestoreReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Restore report:")?;
        for path in &self.restored {
            writeln!(f, "  restored: {}", path.display())?;
        }
        for (path, err) in &self.errors {
            writeln!(f, "  FAILED: {} - {}", path.display(), err)?;
        }
        Ok(())
    }
}

/// Errors that can occur during checkpoint operations.
#[derive(Debug)]
pub enum CheckpointError {
    ReadFailed { path: PathBuf, source: String },
    NotFound { id: u64 },
}

impl std::fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckpointError::ReadFailed { path, source } => {
                write!(f, "Failed to read {}: {}", path.display(), source)
            }
            CheckpointError::NotFound { id } => {
                write!(f, "Checkpoint {} not found", id)
            }
        }
    }
}
```

## Git-Based Checkpoints

For projects under git (which is most of them), you can leverage git itself as the checkpoint storage. This has several advantages: git handles binary files, deduplicates identical content, and the user can inspect checkpoints with familiar git tools.

```rust
use std::process::Command;

/// A checkpoint strategy that uses git stash for storage.
pub struct GitCheckpointManager {
    /// Path to the git repository root.
    repo_root: PathBuf,
    /// Stash references for each checkpoint.
    stash_map: HashMap<u64, String>,
    next_id: u64,
}

impl GitCheckpointManager {
    pub fn new(repo_root: PathBuf) -> Self {
        Self {
            repo_root,
            stash_map: HashMap::new(),
            next_id: 1,
        }
    }

    /// Create a checkpoint by stashing the current state of specified files.
    /// This uses `git stash push` with specific paths.
    pub fn create_checkpoint(
        &mut self,
        paths: &[&Path],
        message: &str,
    ) -> Result<u64, String> {
        let id = self.next_id;
        let stash_message = format!("agent-checkpoint-{}: {}", id, message);

        // Stage the specific files
        let path_args: Vec<&str> = paths.iter().map(|p| {
            p.to_str().unwrap_or("")
        }).collect();

        let output = Command::new("git")
            .current_dir(&self.repo_root)
            .arg("stash")
            .arg("push")
            .arg("-m")
            .arg(&stash_message)
            .arg("--")
            .args(&path_args)
            .output()
            .map_err(|e| format!("Failed to run git stash: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git stash failed: {}", stderr));
        }

        let stash_ref = format!("stash@{{{}}}", 0); // Most recent stash
        self.stash_map.insert(id, stash_ref);
        self.next_id += 1;

        Ok(id)
    }

    /// Restore files from a git stash checkpoint.
    pub fn restore_checkpoint(&self, checkpoint_id: u64) -> Result<String, String> {
        let stash_ref = self
            .stash_map
            .get(&checkpoint_id)
            .ok_or_else(|| format!("Checkpoint {} not found", checkpoint_id))?;

        let output = Command::new("git")
            .current_dir(&self.repo_root)
            .arg("stash")
            .arg("apply")
            .arg(stash_ref)
            .output()
            .map_err(|e| format!("Failed to run git stash apply: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}
```

::: wild In the Wild
Claude Code creates a git checkpoint (using a commit on a hidden branch) before every file write operation. This means every change the agent makes is individually revertible through git. If the agent writes five files in a turn, there are five separate checkpoints. The user can undo the most recent change, undo an entire turn, or undo all changes in the session — all through git operations. This approach is elegant because it reuses git's existing infrastructure rather than building a parallel checkpoint system.
:::

## Integrating Checkpoints with Tool Execution

Here is how checkpoints fit into the tool execution flow. The critical rule is: **create the checkpoint before executing the tool, not after**:

```rust
/// Wrap a file write operation with automatic checkpointing.
pub fn write_file_with_checkpoint(
    checkpoint_mgr: &mut CheckpointManager,
    turn_id: u64,
    tool_call_id: &str,
    path: &Path,
    new_content: &str,
) -> Result<u64, String> {
    // Step 1: Create checkpoint BEFORE writing
    let checkpoint_id = checkpoint_mgr
        .create_checkpoint(
            turn_id,
            tool_call_id,
            "write_file",
            &format!("Write to {}", path.display()),
            &[path],
        )
        .map_err(|e| format!("Checkpoint failed: {}", e))?;

    // Step 2: Perform the actual write
    match fs::write(path, new_content) {
        Ok(()) => Ok(checkpoint_id),
        Err(e) => {
            // Write failed — restore from checkpoint to ensure consistency
            let _ = checkpoint_mgr.restore_checkpoint(checkpoint_id);
            Err(format!("Write failed: {}", e))
        }
    }
}

fn main() {
    let mut mgr = CheckpointManager::new(100);
    let test_path = Path::new("/tmp/agent-test-file.txt");

    // Simulate an initial file
    fs::write(test_path, "original content").unwrap();

    // Write with checkpoint
    let cp_id = write_file_with_checkpoint(
        &mut mgr,
        1, // turn 1
        "call_001",
        test_path,
        "modified content",
    )
    .unwrap();

    // Verify the write happened
    let content = fs::read_to_string(test_path).unwrap();
    assert_eq!(content, "modified content");
    println!("After write: {}", content);

    // Restore from checkpoint
    let report = mgr.restore_checkpoint(cp_id).unwrap();
    println!("{}", report);

    // Verify the restore worked
    let content = fs::read_to_string(test_path).unwrap();
    assert_eq!(content, "original content");
    println!("After restore: {}", content);

    // Clean up
    let _ = fs::remove_file(test_path);
}
```

Notice the error handling: if the write fails *after* the checkpoint is created, we immediately restore from the checkpoint. This ensures the file is never left in a partially written state.

## Checkpoint Limits and Garbage Collection

Keeping every checkpoint forever would consume unbounded memory (for in-memory checkpoints) or disk space (for git-based ones). The `max_checkpoints` parameter in `CheckpointManager` handles this by evicting the oldest checkpoints when the limit is reached. Choose the limit based on your use case:

- **Development sessions**: 100-200 checkpoints covers several hours of active agent use.
- **CI/automated runs**: 20-50 checkpoints is usually sufficient since you have version control as a fallback.
- **Long-running sessions**: Consider flushing old checkpoints to disk or relying on git-based storage.

## Key Takeaways

- File checkpoints capture the state of files *before* modification, enabling precise rollback of any agent action.
- The `Checkpoint` struct groups snapshots by turn and tool invocation, supporting undo at multiple granularities: single file, single tool call, or entire turn.
- Always create the checkpoint before executing the write — if the write fails partway, you can restore immediately.
- Git-based checkpoints leverage existing infrastructure and let users inspect changes with familiar tools like `git diff` and `git stash show`.
- Set a maximum checkpoint count and evict the oldest entries to prevent unbounded memory or disk growth.
