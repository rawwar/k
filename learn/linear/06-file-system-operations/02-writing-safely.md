---
title: Writing Safely
description: Preventing data loss during file writes through atomic operations, backup creation, and write validation techniques.
---

# Writing Safely

> **What you'll learn:**
> - Why naive file writes can corrupt data and how atomic write operations prevent partial writes
> - How to implement write-to-temp-then-rename patterns that guarantee all-or-nothing file updates
> - Strategies for creating backups before destructive writes and validating written content matches intent

When a coding agent writes a file, the stakes are high. A half-written file can break a build. A truncated configuration file can crash a service. A write that fails midway through can leave the codebase in a state that is neither the old version nor the new version -- just broken. This is why safe writing is not optional for a coding agent. It is foundational.

In this subchapter, you'll learn why the naive approach to file writing is dangerous, then build progressively safer alternatives until you arrive at the write-to-temp-then-rename pattern used by production agents.

## The Naive Write and Its Problems

The simplest way to write a file in Rust:

```rust
use std::fs;
use std::path::Path;

fn write_file(path: &Path, content: &str) -> Result<(), std::io::Error> {
    fs::write(path, content)
}
```

This opens the file (creating it if it doesn't exist, truncating it if it does), writes the content, and closes it. What could go wrong?

Consider this sequence of events:

1. `fs::write` opens the file and truncates it to zero bytes. The original content is now gone.
2. The write begins, sending bytes to the OS.
3. Halfway through, the agent process crashes, the user hits Ctrl-C, or the disk fills up.
4. The file now contains half the new content. The old content is gone forever.

This is called a **partial write**, and it is the most common form of data corruption in file operations. The window of vulnerability is the gap between truncation (step 1) and completion (step 2). During that window, the file is incomplete.

::: python Coming from Python
Python's `Path("file.txt").write_text(content)` and `open("file.txt", "w").write(content)` have exactly the same problem. The file is truncated before writing begins. If the write is interrupted, you get a partial file. Python does not provide built-in atomic write facilities -- you have to build them yourself, just as in Rust.
:::

## Write-to-Temp-Then-Rename

The standard solution is to never write directly to the target file. Instead, you write to a temporary file in the same directory, then atomically rename it to the target:

```rust
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

fn write_file_atomic(path: &Path, content: &str) -> Result<(), std::io::Error> {
    // Get the parent directory (temp file must be on the same filesystem)
    let parent = path.parent().unwrap_or(Path::new("."));

    // Create a temporary file in the same directory
    let mut temp_file = NamedTempFile::new_in(parent)?;

    // Write content to the temporary file
    temp_file.write_all(content.as_bytes())?;

    // Ensure all data is flushed to disk
    temp_file.flush()?;

    // Atomically rename the temp file to the target path
    temp_file.persist(path)?;

    Ok(())
}
```

Here's why this is safe:

1. The temporary file is created in the same directory as the target. This is critical because `rename` is only atomic when both paths are on the same filesystem.
2. All content is written to the temp file first. If the write fails or is interrupted, the original file is untouched -- it was never opened.
3. `persist` calls `rename` under the hood, which is an atomic operation on all major operating systems. The target path either points to the old file or the new file, never to a partial file.

The `tempfile` crate handles creating the temporary file with a unique name to avoid collisions. If the process crashes before `persist` is called, the temp file is automatically cleaned up on drop.

## Preserving File Permissions

A subtle problem with write-to-temp-then-rename: the new file may not have the same permissions as the original. If the original file was executable (`chmod +x`), the replacement might not be. Here's how to preserve permissions:

```rust
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::NamedTempFile;

fn write_file_preserving_permissions(
    path: &Path,
    content: &str,
) -> Result<(), std::io::Error> {
    let parent = path.parent().unwrap_or(Path::new("."));

    // Read existing permissions before creating the new file
    let original_permissions = if path.exists() {
        Some(fs::metadata(path)?.permissions())
    } else {
        None
    };

    let mut temp_file = NamedTempFile::new_in(parent)?;
    temp_file.write_all(content.as_bytes())?;
    temp_file.flush()?;

    // Apply original permissions to the temp file before renaming
    if let Some(perms) = original_permissions {
        fs::set_permissions(temp_file.path(), perms)?;
    }

    temp_file.persist(path)?;
    Ok(())
}
```

::: wild In the Wild
Claude Code's Write tool uses atomic write operations to prevent partial writes. When it creates a new file, it also ensures the parent directory exists by creating intermediate directories. OpenCode takes a similar approach and additionally preserves the file's original permissions and ownership metadata when replacing an existing file.
:::

## Creating Backups

For an extra safety layer, you can create a backup of the original file before replacing it:

```rust
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

fn backup_path(path: &Path) -> PathBuf {
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!("{}.bak", file_name))
}

fn write_file_with_backup(
    path: &Path,
    content: &str,
) -> Result<(), std::io::Error> {
    // Create backup of existing file
    if path.exists() {
        let backup = backup_path(path);
        fs::copy(path, &backup)?;
    }

    // Atomic write
    let parent = path.parent().unwrap_or(Path::new("."));
    let mut temp_file = NamedTempFile::new_in(parent)?;
    temp_file.write_all(content.as_bytes())?;
    temp_file.flush()?;
    temp_file.persist(path)?;

    Ok(())
}
```

This gives you a `.bak` file you can restore from. In practice, most production agents rely on Git as their backup mechanism -- the user can always `git checkout` to restore a file. But for files not tracked by Git, explicit backups are a useful safety net.

## Ensuring Parent Directories Exist

When a write tool is asked to create a file in a directory that doesn't exist yet, it should create the intermediate directories:

```rust
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

fn write_file_creating_dirs(
    path: &Path,
    content: &str,
) -> Result<(), std::io::Error> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    let parent = path.parent().unwrap_or(Path::new("."));
    let mut temp_file = NamedTempFile::new_in(parent)?;
    temp_file.write_all(content.as_bytes())?;
    temp_file.flush()?;
    temp_file.persist(path)?;

    Ok(())
}
```

`fs::create_dir_all` is the Rust equivalent of `mkdir -p`. It creates all missing intermediate directories in the path. This is essential for a write tool because the LLM might generate a file path like `src/tools/filesystem/reader.rs` where `src/tools/filesystem/` doesn't exist yet.

## Validating After Write

A belt-and-suspenders approach reads the file back after writing to verify the content matches:

```rust
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

fn write_file_validated(
    path: &Path,
    content: &str,
) -> Result<(), String> {
    let parent = path.parent().unwrap_or(Path::new("."));

    if let Some(parent_dir) = path.parent() {
        if !parent_dir.exists() {
            fs::create_dir_all(parent_dir)
                .map_err(|e| format!("Failed to create directories: {e}"))?;
        }
    }

    let mut temp_file = NamedTempFile::new_in(parent)
        .map_err(|e| format!("Failed to create temp file: {e}"))?;
    temp_file.write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write content: {e}"))?;
    temp_file.flush()
        .map_err(|e| format!("Failed to flush: {e}"))?;
    temp_file.persist(path)
        .map_err(|e| format!("Failed to persist: {e}"))?;

    // Read back and verify
    let written = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read back: {e}"))?;
    if written != content {
        return Err("Written content does not match expected content".into());
    }

    Ok(())
}
```

This catches rare edge cases like filesystem encoding issues or disk corruption. The cost is an extra read, which is negligible for source code files.

::: python Coming from Python
Python developers often use `pathlib.Path.write_text()` without a second thought about atomicity. For a scripting context that is fine. But when building a tool that modifies someone's codebase autonomously, the extra care of atomic writes, permission preservation, and validation is worth the effort. The `atomicwrites` PyPI package provides similar functionality in Python if you ever need it there.
:::

## Complete Write Tool

Combining everything, here is a production-ready write tool:

```rust
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

pub fn write_tool(path: &Path, content: &str) -> Result<String, String> {
    let is_new = !path.exists();

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directories: {e}"))?;
        }
    }

    // Store original permissions
    let original_perms = if path.exists() {
        fs::metadata(path).ok().map(|m| m.permissions())
    } else {
        None
    };

    // Atomic write via temp file
    let parent = path.parent().unwrap_or(Path::new("."));
    let mut temp_file = NamedTempFile::new_in(parent)
        .map_err(|e| format!("Failed to create temp file: {e}"))?;
    temp_file.write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write: {e}"))?;
    temp_file.flush()
        .map_err(|e| format!("Failed to flush: {e}"))?;

    // Restore permissions
    if let Some(perms) = original_perms {
        let _ = fs::set_permissions(temp_file.path(), perms);
    }

    temp_file.persist(path)
        .map_err(|e| format!("Failed to persist: {e}"))?;

    let line_count = content.lines().count();
    if is_new {
        Ok(format!("Created {} ({} lines)", path.display(), line_count))
    } else {
        Ok(format!("Updated {} ({} lines)", path.display(), line_count))
    }
}
```

The tool returns a human-readable summary that the LLM can include in its response to the user. It distinguishes between creating a new file and updating an existing one, and it reports the line count so the model can verify it wrote the expected amount of content.

## Key Takeaways

- Never write directly to a target file -- use the write-to-temp-then-rename pattern to guarantee atomicity and prevent partial writes
- The temporary file must be on the same filesystem as the target for `rename` to be atomic -- creating it in the same directory ensures this
- Preserve file permissions when replacing existing files, especially on Unix systems where executable permissions matter
- Create parent directories automatically with `fs::create_dir_all` so the LLM doesn't need to worry about directory structure
- Return informative results (created vs. updated, line counts) so the model can verify its writes succeeded
