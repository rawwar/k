---
title: Atomic Writes
description: Use write-to-temp-then-rename to ensure file writes are atomic and cannot leave files in a corrupted state.
---

# Atomic Writes

> **What you'll learn:**
> - Why writing directly to the target file can cause data corruption if the process crashes mid-write
> - How to implement atomic writes by writing to a temporary file in the same directory and then renaming
> - How to handle edge cases like cross-device renames, symlinked targets, and preserving file metadata

So far our write and edit tools use `fs::write` to write content directly to the target file. This works fine when everything goes right. But what happens when the process crashes halfway through a write? The file could end up with half the old content and half the new content -- corrupted, unusable, and possibly unrecoverable. Atomic writes solve this problem by ensuring that a file is either fully written or not modified at all.

## The Corruption Problem

Consider what happens when you call `fs::write("config.toml", new_content)`:

1. The OS opens the file and truncates it to zero bytes
2. The OS begins writing `new_content` byte by byte (or in chunks)
3. If the process crashes, the power goes out, or the disk fills up partway through step 2, the file now contains a partial write

The result? Your `config.toml` is corrupted. The old content is gone (truncated in step 1) and the new content is incomplete. For a coding agent that might be editing critical files, this is unacceptable.

## The Write-Temp-Rename Pattern

The solution is a two-step process:

1. Write the new content to a temporary file in the same directory as the target
2. Rename the temporary file to the target path

On Unix filesystems, `rename()` is an atomic operation -- it either completes entirely or does not happen at all. The key requirement is that the temporary file and the target must be on the same filesystem (same mount point), which is why we create the temp file in the same directory.

Here is the implementation:

```rust
use std::fs::{self, File, Permissions};
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

/// Write content to a file atomically.
/// Uses write-to-temp-then-rename to prevent corruption.
pub fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    // Determine the directory for the temp file.
    // It must be the same directory as the target for rename to work.
    let parent = path.parent().ok_or_else(|| {
        format!("Path '{}' has no parent directory", path.display())
    })?;

    // Ensure the parent directory exists
    fs::create_dir_all(parent).map_err(|e| {
        format!("Failed to create directory '{}': {}", parent.display(), e)
    })?;

    // Create a temporary file in the same directory
    let mut temp_file = NamedTempFile::new_in(parent).map_err(|e| {
        format!(
            "Failed to create temporary file in '{}': {}",
            parent.display(),
            e
        )
    })?;

    // Write the content to the temp file
    temp_file
        .write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write to temporary file: {}", e))?;

    // Flush to ensure all data reaches the disk
    temp_file
        .flush()
        .map_err(|e| format!("Failed to flush temporary file: {}", e))?;

    // Rename the temp file to the target path.
    // persist() does an atomic rename on Unix.
    temp_file.persist(path).map_err(|e| {
        format!(
            "Failed to rename temporary file to '{}': {}",
            path.display(),
            e
        )
    })?;

    Ok(())
}
```

The `tempfile` crate's `NamedTempFile` handles the details: it creates a file with a unique name in the specified directory, and `persist()` performs the atomic rename. If anything goes wrong before `persist()` is called, the temporary file is automatically deleted when `NamedTempFile` is dropped.

::: tip Coming from Python
In Python, you would use `tempfile.NamedTemporaryFile` for the same pattern:
```python
import tempfile
import os
from pathlib import Path

def atomic_write(path: str, content: str) -> None:
    parent = str(Path(path).parent)
    with tempfile.NamedTemporaryFile(
        mode='w', dir=parent, delete=False, suffix='.tmp'
    ) as tmp:
        tmp.write(content)
        tmp.flush()
        os.fsync(tmp.fileno())  # force to disk
        tmp_path = tmp.name

    os.replace(tmp_path, path)  # atomic on POSIX
```
The structure is identical: create temp in same directory, write, flush, rename. Python's `os.replace` is the equivalent of Rust's `persist()`. One difference is `os.fsync()` -- Python does not have an equivalent of Rust's strong guarantees about flush, so you explicitly call fsync. In Rust, `flush()` does not guarantee data hits disk either, but it gets the data out of the process's buffer. For true durability, you would use `sync_all()` on the file.
:::

## Adding Durability with sync_all

The `flush()` call pushes data from the application buffer to the OS, but the OS may still hold it in a write cache. For maximum durability (surviving power outages), you need `sync_all()`:

```rust
pub fn atomic_write_durable(path: &Path, content: &str) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| {
        format!("Path '{}' has no parent directory", path.display())
    })?;

    fs::create_dir_all(parent).map_err(|e| {
        format!("Failed to create directory '{}': {}", parent.display(), e)
    })?;

    let mut temp_file = NamedTempFile::new_in(parent).map_err(|e| {
        format!("Failed to create temporary file: {}", e)
    })?;

    temp_file
        .write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write to temporary file: {}", e))?;

    // sync_all forces data AND metadata to disk
    temp_file
        .as_file()
        .sync_all()
        .map_err(|e| format!("Failed to sync temporary file to disk: {}", e))?;

    temp_file.persist(path).map_err(|e| {
        format!("Failed to rename temporary file to '{}': {}", path.display(), e)
    })?;

    Ok(())
}
```

The difference between `flush()` and `sync_all()`:
- `flush()`: application buffer to OS buffer (fast)
- `sync_all()`: application buffer all the way to disk (slower, but crash-safe)

For a coding agent, `flush()` is usually sufficient. The agent is not a database -- if the power goes out mid-edit, losing the last edit is acceptable. But if you are writing to critical configuration files, `sync_all()` adds a real safety margin.

## Preserving Permissions During Atomic Writes

There is a subtle problem with the write-temp-rename approach: the new file (originally the temp file) gets the default permissions, not the original file's permissions. If you are editing a shell script that was `chmod +x`, the atomic write removes the execute permission.

The fix is to save and restore permissions:

```rust
pub fn atomic_write_preserving_permissions(
    path: &Path,
    content: &str,
) -> Result<(), String> {
    // Save original permissions if the file exists
    let original_permissions = if path.exists() {
        Some(
            fs::metadata(path)
                .map_err(|e| format!("Cannot read metadata: {}", e))?
                .permissions(),
        )
    } else {
        None
    };

    let parent = path.parent().ok_or_else(|| {
        format!("Path '{}' has no parent directory", path.display())
    })?;

    fs::create_dir_all(parent).map_err(|e| {
        format!("Failed to create directory '{}': {}", parent.display(), e)
    })?;

    let mut temp_file = NamedTempFile::new_in(parent).map_err(|e| {
        format!("Failed to create temporary file: {}", e)
    })?;

    temp_file
        .write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write to temporary file: {}", e))?;

    temp_file
        .flush()
        .map_err(|e| format!("Failed to flush temporary file: {}", e))?;

    // Set permissions on temp file BEFORE the rename
    if let Some(perms) = &original_permissions {
        temp_file
            .as_file()
            .set_permissions(perms.clone())
            .map_err(|e| format!("Failed to set permissions: {}", e))?;
    }

    temp_file.persist(path).map_err(|e| {
        format!("Failed to rename temporary file: {}", path.display())
    })?;

    Ok(())
}
```

Setting permissions on the temp file before the rename means the target file gets the correct permissions atomically along with the new content. There is no window where the file exists with wrong permissions.

## Updating the File Tools

Replace `fs::write` calls in your write and edit tools with `atomic_write`:

```rust
// In WriteFileTool::execute, replace:
//   fs::write(&path, content)
// with:
atomic_write(&path, content)?;

// In EditFileTool::execute, replace:
//   fs::write(&path, &new_content)
// with:
atomic_write_preserving_permissions(&path, &new_content)?;
```

The write tool uses the basic `atomic_write` because it creates new files where permissions are not yet established. The edit tool uses `atomic_write_preserving_permissions` because it modifies existing files whose permissions matter.

## Handling Failures Gracefully

One of the best properties of atomic writes is their failure behavior. If anything goes wrong -- the disk is full, the process crashes, permissions prevent the rename -- the original file is untouched. The temporary file is either cleaned up by `NamedTempFile`'s destructor or left as a small orphan file that can be cleaned up later.

Let's verify this property in a test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_atomic_write_creates_new_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("new.txt");

        atomic_write(&path, "hello world").unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_atomic_write_replaces_existing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("existing.txt");

        fs::write(&path, "old content").unwrap();
        atomic_write(&path, "new content").unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "new content");
    }

    #[test]
    fn test_atomic_write_creates_directories() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("a/b/c/deep.txt");

        atomic_write(&path, "deep content").unwrap();

        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "deep content");
    }

    #[test]
    fn test_atomic_write_preserves_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("script.sh");

        // Create an executable file
        fs::write(&path, "#!/bin/bash\necho old").unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o755)).unwrap();

        // Atomic write preserving permissions
        atomic_write_preserving_permissions(&path, "#!/bin/bash\necho new").unwrap();

        let perms = fs::metadata(&path).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o755);

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "#!/bin/bash\necho new");
    }

    #[test]
    fn test_original_unchanged_on_no_persist() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("original.txt");
        fs::write(&path, "original content").unwrap();

        // Simulate a failed write by creating a temp file but NOT persisting
        let parent = path.parent().unwrap();
        let mut temp = NamedTempFile::new_in(parent).unwrap();
        temp.write_all(b"new content").unwrap();
        // Drop temp without persist -- temp file is cleaned up
        drop(temp);

        // Original is untouched
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "original content");
    }
}
```

The last test demonstrates the safety guarantee: if the temp file is dropped without `persist()`, the original file remains untouched and the temp file is cleaned up automatically.

::: tip In the Wild
Claude Code uses atomic writes for all file modifications. The write-temp-rename pattern is so fundamental to file safety that it appears in virtually every production tool. Databases use the same technique (write-ahead logging followed by atomic commit), and Git itself uses temporary files with atomic renames when updating refs. Our implementation follows the same well-proven pattern.
:::

## Key Takeaways

- Direct `fs::write` can corrupt files if the process crashes mid-write because the file is truncated before new content is fully written.
- The write-temp-rename pattern creates a temporary file in the same directory, writes to it, then atomically renames it to the target path -- the file is either fully updated or untouched.
- The temp file and target must be on the same filesystem for `rename` to be atomic; `NamedTempFile::new_in(parent)` ensures this by creating the temp in the target's directory.
- Preserve original file permissions by saving them before the write and applying them to the temp file before the rename, so executable scripts keep their execute bits.
- `flush()` pushes data to the OS buffer (sufficient for most agent use cases), while `sync_all()` forces data to disk (needed for crash-safe durability guarantees).
