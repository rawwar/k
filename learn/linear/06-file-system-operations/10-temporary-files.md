---
title: Temporary Files
description: Using temporary files and directories for safe staging of writes, scratch computation, and cleanup-guaranteed operations.
---

# Temporary Files

> **What you'll learn:**
> - How to create and manage temporary files in Rust using the tempfile crate with automatic cleanup on drop
> - When to use named temporary files versus anonymous temporary files and the implications for atomicity
> - Patterns for using temporary directories as staging areas for multi-file operations that need to be committed atomically

Temporary files appear throughout this chapter. We used them for atomic writes, for staging multi-file operations, and for large file editing. Now let's look at the `tempfile` crate in detail and understand the different types of temporary files, when to use each, and how Rust's ownership system makes cleanup automatic and reliable.

## The tempfile Crate

The `tempfile` crate is the standard Rust library for temporary file operations. It provides three main types:

1. **`tempfile()`** -- creates an unnamed temporary file that is deleted when the handle is dropped
2. **`NamedTempFile`** -- creates a temporary file with a visible path in the filesystem
3. **`TempDir`** -- creates a temporary directory that is recursively deleted when dropped

Let's look at each one.

## Anonymous Temporary Files

The simplest temporary file has no name:

```rust
use std::io::{Read, Seek, SeekFrom, Write};
use tempfile::tempfile;

fn use_anonymous_temp() -> Result<(), std::io::Error> {
    // Creates a temp file in the system temp directory
    let mut file = tempfile()?;

    // Write data to it
    file.write_all(b"intermediate computation result")?;

    // Seek back to the beginning to read it
    file.seek(SeekFrom::Start(0))?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    println!("Temp file contains: {}", contents);

    // File is automatically deleted when `file` goes out of scope
    Ok(())
}
```

Anonymous temp files are created using `O_TMPFILE` on Linux (the file exists only as a file descriptor with no directory entry) or with a random name that is immediately unlinked. They are perfect for scratch data that no other process needs to access.

::: tip Coming from Python
Python's `tempfile.TemporaryFile()` creates the same kind of anonymous temp file. The context manager pattern (`with tempfile.TemporaryFile() as f:`) maps to Rust's drop semantics -- when the variable goes out of scope (or the `with` block exits), the file is cleaned up. The key difference is that Rust's cleanup is deterministic via `Drop`, while Python's cleanup depends on the garbage collector (though the `with` statement makes it deterministic in practice).
:::

## Named Temporary Files

Named temp files have a path in the filesystem, which means other processes (or your own code) can access them by path:

```rust
use std::io::Write;
use tempfile::NamedTempFile;

fn use_named_temp() -> Result<(), std::io::Error> {
    let mut temp = NamedTempFile::new()?;

    // You can get the path to pass to other tools
    println!("Temp file at: {}", temp.path().display());

    temp.write_all(b"data for another tool")?;
    temp.flush()?;

    // The file exists on disk and can be read by other processes
    let content = std::fs::read_to_string(temp.path())?;
    assert_eq!(content, "data for another tool");

    // File is deleted when `temp` is dropped
    Ok(())
}
```

Named temp files are what you need for atomic writes (the `persist` method renames them to the target) and for passing data to external tools via file paths.

### Controlling Temp File Location

By default, temporary files are created in the system temp directory (`/tmp` on Unix, `%TEMP%` on Windows). For atomic writes, you need the temp file on the same filesystem as the target. Use `new_in`:

```rust
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

fn temp_in_same_dir(target: &Path) -> Result<(), std::io::Error> {
    let parent = target.parent().unwrap_or(Path::new("."));

    // Create temp file in the same directory as the target
    let mut temp = NamedTempFile::new_in(parent)?;
    temp.write_all(b"new content")?;
    temp.flush()?;

    // This rename is guaranteed atomic because same filesystem
    temp.persist(target)?;

    Ok(())
}
```

### Persist vs. Keep

`NamedTempFile` offers two ways to prevent deletion:

```rust
use std::io::Write;
use tempfile::NamedTempFile;

fn persist_vs_keep() -> Result<(), Box<dyn std::error::Error>> {
    // persist() renames the file to a new path (atomic on same filesystem)
    let mut temp1 = NamedTempFile::new()?;
    temp1.write_all(b"persisted content")?;
    temp1.persist("/tmp/my_output.txt")?;
    // The temp file is now at /tmp/my_output.txt and will NOT be deleted

    // keep() retains the file at its temporary path
    let mut temp2 = NamedTempFile::new()?;
    temp2.write_all(b"kept content")?;
    let (file, path) = temp2.keep()?;
    println!("File kept at: {}", path.display());
    // The temp file remains at its random path and will NOT be deleted
    // You are responsible for cleaning it up

    drop(file); // Close the file handle, but don't delete the file

    Ok(())
}
```

Use `persist` for atomic writes. Use `keep` when you need the temp file to outlive the current scope (e.g., passing it to a long-running subprocess).

## Temporary Directories

`TempDir` creates a directory that is recursively deleted on drop:

```rust
use std::fs;
use std::io::Write;
use tempfile::TempDir;

fn use_temp_dir() -> Result<(), Box<dyn std::error::Error>> {
    let staging = TempDir::new()?;
    println!("Staging area: {}", staging.path().display());

    // Create files inside the temp directory
    let file1 = staging.path().join("config.toml");
    fs::write(&file1, "[settings]\nverbose = true\n")?;

    let subdir = staging.path().join("src");
    fs::create_dir(&subdir)?;
    fs::write(subdir.join("main.rs"), "fn main() {}\n")?;

    // List everything we created
    for entry in fs::read_dir(staging.path())? {
        let entry = entry?;
        println!("  {}", entry.path().display());
    }

    // Everything is recursively deleted when staging is dropped
    Ok(())
}
```

::: tip Coming from Python
Python's `tempfile.TemporaryDirectory()` is the direct equivalent. You'd write `with tempfile.TemporaryDirectory() as tmpdir:` and the directory is recursively deleted when the block exits. Rust's `TempDir` works identically, with drop serving as the automatic cleanup mechanism.
:::

## Staging Patterns for Agents

Temporary directories are ideal for staging complex operations. Here's a pattern for preparing a multi-file write:

```rust
use std::fs;
use std::path::Path;
use tempfile::TempDir;

pub struct StagedOperation {
    staging_dir: TempDir,
    files: Vec<String>, // relative paths
}

impl StagedOperation {
    pub fn new() -> Result<Self, std::io::Error> {
        Ok(Self {
            staging_dir: TempDir::new()?,
            files: Vec::new(),
        })
    }

    pub fn stage_file(
        &mut self,
        relative_path: &str,
        content: &str,
    ) -> Result<(), String> {
        let staged_path = self.staging_dir.path().join(relative_path);

        // Create parent directories in staging area
        if let Some(parent) = staged_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create dir: {e}"))?;
        }

        fs::write(&staged_path, content)
            .map_err(|e| format!("Cannot stage file: {e}"))?;

        self.files.push(relative_path.to_string());
        Ok(())
    }

    pub fn commit(self, target_dir: &Path) -> Result<Vec<String>, String> {
        let mut committed = Vec::new();

        for rel_path in &self.files {
            let source = self.staging_dir.path().join(rel_path);
            let target = target_dir.join(rel_path);

            // Ensure target parent dirs exist
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Cannot create dir: {e}"))?;
            }

            fs::rename(&source, &target)
                .map_err(|e| format!("Cannot commit {rel_path}: {e}"))?;

            committed.push(rel_path.clone());
        }

        // staging_dir is dropped here, cleaning up any remaining files
        Ok(committed)
    }

    pub fn abort(self) {
        // Simply drop self -- the TempDir destructor cleans everything up
        drop(self);
    }
}
```

This pattern lets you prepare multiple files in a staging area, verify everything is correct, and then commit them all. If anything goes wrong, aborting is as simple as dropping the struct.

## Handling Temp File Cleanup Failures

On some systems, particularly Windows, temp file cleanup can fail if the file is still open by another process. Handle this gracefully:

```rust
use tempfile::TempDir;

fn cleanup_handling() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let path = temp_dir.path().to_path_buf();

    // If cleanup fails on drop, the destructor silently ignores the error.
    // To handle cleanup errors explicitly:
    match temp_dir.close() {
        Ok(()) => println!("Temp dir cleaned up successfully"),
        Err(e) => {
            eprintln!(
                "Warning: Could not clean up temp dir at {}: {}",
                path.display(),
                e
            );
            // The temp dir still exists -- it will be cleaned up
            // by the OS eventually (on reboot or by temp file cleanup)
        }
    }
}
```

The `close()` method consumes the `TempDir` and returns a `Result` for the cleanup operation. If you don't call `close()`, the `Drop` implementation silently ignores cleanup errors.

## Temp Files for Tool Output

When your agent runs external tools (compilers, linters, formatters), temp files are useful for passing data:

```rust
use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;

fn format_code_snippet(code: &str) -> Result<String, String> {
    // Write the code to a temp file
    let mut temp = NamedTempFile::with_suffix(".rs")
        .map_err(|e| format!("Cannot create temp file: {e}"))?;
    temp.write_all(code.as_bytes())
        .map_err(|e| format!("Cannot write: {e}"))?;
    temp.flush()
        .map_err(|e| format!("Cannot flush: {e}"))?;

    // Run rustfmt on the temp file
    let output = Command::new("rustfmt")
        .arg(temp.path())
        .output()
        .map_err(|e| format!("Cannot run rustfmt: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "rustfmt failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Read the formatted result
    let formatted = std::fs::read_to_string(temp.path())
        .map_err(|e| format!("Cannot read formatted: {e}"))?;

    Ok(formatted)
    // temp file is deleted on drop
}
```

The `.with_suffix(".rs")` ensures the temp file has the right extension, which tools like `rustfmt` use to determine the language.

::: wild In the Wild
Claude Code uses temporary files when it needs to pass content to external tools like linters or formatters. The temp file is created, used for the tool invocation, and cleaned up automatically. This avoids modifying the user's actual files when the agent just needs to check syntax or formatting on a code snippet.
:::

## Key Takeaways

- Use anonymous temp files (`tempfile()`) for scratch data that no other process needs -- they leave no trace on disk
- Use `NamedTempFile` when you need a filesystem path (for atomic writes via `persist` or for passing to external tools)
- Use `TempDir` for staging multi-file operations -- the entire directory is recursively cleaned up on drop
- Always create temp files in the same directory as the target when using `persist` to ensure atomic rename works
- Rust's `Drop` trait makes temp file cleanup deterministic and automatic -- no need for try/finally blocks or context managers
