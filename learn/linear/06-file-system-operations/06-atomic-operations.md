---
title: Atomic Operations
description: Implementing atomic file operations that either complete fully or leave the original file untouched, preventing corruption from crashes or interrupts.
---

# Atomic Operations

> **What you'll learn:**
> - What atomicity means for file operations and why it matters when an agent might be interrupted mid-write
> - How to implement atomic writes using temporary files and rename operations on POSIX and Windows
> - How to extend atomicity to multi-file operations using journaling or git-based transaction patterns

We touched on atomic writes in [Writing Safely](/linear/06-file-system-operations/02-writing-safely), but atomicity deserves its own deeper treatment. When a coding agent makes changes to a codebase, it frequently modifies multiple files in a single logical operation -- renaming a function requires changing the definition and every call site. If the agent crashes or the user interrupts it midway, you want the codebase to be in a consistent state: either all the changes are applied or none of them are.

## What Atomicity Means

An operation is **atomic** if it either completes entirely or has no effect at all. There is no observable intermediate state. The classic example is a bank transfer: you don't want the money deducted from one account without being added to another.

For file operations, atomicity means:
- A file write either succeeds completely or the original file remains unchanged.
- A multi-file edit either applies all changes or none of them.
- A crash at any point during the operation doesn't leave corrupted files on disk.

## The Rename Primitive

The foundation of atomic file operations on Unix is the `rename` system call. When you rename a file from path A to path B:

- If path B doesn't exist, the file appears at path B atomically.
- If path B already exists, it is **atomically replaced**. At no point is path B missing or partially written.
- The operation is guaranteed to be atomic by the POSIX specification (as long as both paths are on the same filesystem).

This is why the write-to-temp-then-rename pattern works. Let's revisit it with more detail:

```rust
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

pub fn atomic_write(path: &Path, content: &str) -> Result<(), std::io::Error> {
    let parent = path.parent().unwrap_or(Path::new("."));

    // Step 1: Create temp file in the same directory
    let mut temp = NamedTempFile::new_in(parent)?;

    // Step 2: Write all content to temp file
    temp.write_all(content.as_bytes())?;

    // Step 3: Flush userspace buffers to OS
    temp.flush()?;

    // Step 4: Sync to disk (ensures data survives power loss)
    temp.as_file().sync_all()?;

    // Step 5: Atomically replace the target
    temp.persist(path)?;

    Ok(())
}
```

The `sync_all()` call in step 4 is important for durability. Without it, the data might be in the OS page cache but not yet on the physical disk. If power fails between steps 3 and 5, the temp file might be empty on disk even though `flush` succeeded. `sync_all` forces the OS to write the data to the storage device.

::: python Coming from Python
Python's `os.rename()` has the same atomic guarantee on Unix. The `atomicwrites` PyPI package provides a convenient `atomic_write` context manager that handles the temp file creation and rename for you. Rust's `tempfile::NamedTempFile::persist()` serves the same purpose. The key insight is the same in both languages: never write directly to the target file.
:::

## Atomicity on Windows

Windows complicates things. The Win32 `MoveFileEx` function with the `MOVEFILE_REPLACE_EXISTING` flag is *mostly* atomic, but there are edge cases:

- If the target file is open by another process, the rename may fail.
- Some Windows filesystems (notably FAT32) don't guarantee atomic replace.
- Anti-virus software may interfere with file renames.

The `tempfile` crate handles most of these differences for you. Its `persist` method uses the appropriate platform-specific call. On Windows, it uses `MoveFileEx` with `MOVEFILE_REPLACE_EXISTING`. For maximum safety on Windows, you can fall back to a write-then-backup pattern:

```rust
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

fn atomic_write_windows_safe(
    path: &Path,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let parent = path.parent().unwrap_or(Path::new("."));

    // Write to temp file
    let mut temp = NamedTempFile::new_in(parent)?;
    temp.write_all(content.as_bytes())?;
    temp.flush()?;

    // Try the atomic persist first
    match temp.persist(path) {
        Ok(_) => Ok(()),
        Err(persist_error) => {
            // Fallback: rename original to backup, then rename temp to target
            let backup = path.with_extension("bak");
            if path.exists() {
                fs::rename(path, &backup)?;
            }

            // Rename temp to target
            match fs::rename(persist_error.file.path(), path) {
                Ok(_) => {
                    // Clean up backup
                    let _ = fs::remove_file(&backup);
                    Ok(())
                }
                Err(rename_err) => {
                    // Restore from backup
                    if backup.exists() {
                        let _ = fs::rename(&backup, path);
                    }
                    Err(Box::new(rename_err))
                }
            }
        }
    }
}
```

This fallback is not truly atomic (there is a brief window where neither the original nor the new file exists at the target path), but it minimizes the risk of data loss.

## Flushing and Syncing

There are three levels of "written to disk" that you should understand:

```rust
use std::fs::File;
use std::io::Write;

fn write_levels(file: &mut File, data: &[u8]) -> std::io::Result<()> {
    // Level 1: Write to userspace buffer
    // Data is in your process's memory, not yet sent to OS
    file.write_all(data)?;

    // Level 2: Flush to OS kernel buffer
    // Data is in the OS page cache, will eventually be written to disk
    file.flush()?;

    // Level 3: Sync to physical storage
    // Data is guaranteed to survive power loss
    file.sync_all()?;

    Ok(())
}
```

For a coding agent, `flush()` is usually sufficient. Source code files are small, and the OS will write them to disk within seconds. But if you want to guarantee durability (the file survives a power failure immediately after the write), you need `sync_all()`. The trade-off is performance: `sync_all()` can be 10-100x slower than `flush()` because it waits for the physical write to complete.

::: details Why sync_all is slow
On an SSD, `sync_all` typically takes 1-5 milliseconds per file. On a spinning hard drive, it can take 10-20 milliseconds because the drive has to wait for the platter to rotate to the right position. For a single file write, this is imperceptible. But if your agent is writing 50 files as part of a large refactor, those milliseconds add up. In practice, most agents skip `sync_all` for individual file writes and rely on the OS to flush data within its normal write-back interval (typically 5-30 seconds).
:::

## Multi-File Atomicity

Single-file atomicity is solved by rename. Multi-file atomicity is harder. Consider a refactoring that renames a struct from `Config` to `Settings` across 15 files. If the agent crashes after editing 8 files, the codebase is broken -- half the files refer to `Config` and half to `Settings`.

### Approach 1: Git as a Transaction Log

The simplest multi-file atomicity strategy is to leverage Git:

```rust
use std::process::Command;
use std::path::Path;

fn atomic_multi_file_edit(
    repo_root: &Path,
    edits: Vec<(String, String)>, // (file_path, new_content) pairs
) -> Result<(), String> {
    // Step 1: Ensure clean working directory
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_root)
        .output()
        .map_err(|e| format!("Git status failed: {e}"))?;

    if !status.stdout.is_empty() {
        return Err("Working directory is not clean. Commit or stash changes first.".into());
    }

    // Step 2: Apply all edits
    for (file_path, content) in &edits {
        let full_path = repo_root.join(file_path);
        std::fs::write(&full_path, content)
            .map_err(|e| format!("Failed to write {file_path}: {e}"))?;
    }

    // Step 3: If any edit failed, restore from Git
    // (In this simple version, we trust the writes above succeeded
    //  or failed before modifying the file, due to Err early return)

    Ok(())
}

fn rollback_all_changes(repo_root: &Path) -> Result<(), String> {
    Command::new("git")
        .args(["checkout", "."])
        .current_dir(repo_root)
        .output()
        .map_err(|e| format!("Git rollback failed: {e}"))?;
    Ok(())
}
```

If anything goes wrong, `git checkout .` restores all files to their last committed state. This is not true atomicity (there is a window where files are partially modified), but it provides easy rollback.

### Approach 2: Staging Directory

For true multi-file atomicity without Git, you can use a staging directory:

```rust
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn atomic_multi_file_write(
    base_dir: &Path,
    files: &[(&str, &str)], // (relative_path, content) pairs
) -> Result<(), String> {
    // Create a staging directory
    let staging = TempDir::new_in(base_dir)
        .map_err(|e| format!("Cannot create staging dir: {e}"))?;

    // Write all files to staging
    for (rel_path, content) in files {
        let staged_path = staging.path().join(rel_path);
        if let Some(parent) = staged_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create dir: {e}"))?;
        }
        fs::write(&staged_path, content)
            .map_err(|e| format!("Cannot write staged file: {e}"))?;
    }

    // All writes succeeded -- now move files from staging to target
    // This is not fully atomic, but minimizes the vulnerability window
    for (rel_path, _) in files {
        let staged = staging.path().join(rel_path);
        let target = base_dir.join(rel_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create dir: {e}"))?;
        }
        fs::rename(&staged, &target)
            .map_err(|e| format!("Cannot rename: {e}"))?;
    }

    Ok(())
}
```

Each individual rename is atomic, so you never get a partially written file. But the series of renames is not atomic -- if the process crashes between the third and fourth rename, you have an inconsistent state. For complete multi-file atomicity, you would need filesystem-level transactions (available on NTFS via `TxF`, but deprecated) or a write-ahead log.

::: wild In the Wild
Claude Code leverages Git's version control as its primary safety net for multi-file operations. Before making changes, it relies on the user having a clean Git working directory. If something goes wrong, the user can `git diff` to see what changed and `git checkout` to undo it. Codex CLI takes a stronger approach by automatically creating Git checkpoints before multi-file operations and offering to roll back if the user is unhappy with the result.
:::

## Key Takeaways

- Single-file atomicity is achieved through the write-to-temp-then-rename pattern, which leverages the POSIX guarantee that `rename` is atomic on the same filesystem
- Use `flush()` for most agent writes and `sync_all()` only when you need the data to survive immediate power loss
- Windows atomicity is less guaranteed than POSIX -- use the `tempfile` crate's `persist()` method which handles platform differences
- Multi-file atomicity is fundamentally harder than single-file -- leverage Git as a transaction log for easy rollback
- The staging directory pattern minimizes the vulnerability window for multi-file writes but does not achieve true atomicity
