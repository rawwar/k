---
title: File Permissions
description: Handle file permission checks and errors gracefully when the agent attempts to read or write protected files.
---

# File Permissions

> **What you'll learn:**
> - How to check file permissions before attempting read or write operations to produce clear error messages
> - How to handle permission-denied errors from the OS and report them as tool errors rather than panics
> - How to preserve original file permissions when writing or editing so executable files stay executable

File permissions are the operating system's own safety layer. Even if your safety checker approves an operation, the OS might deny it -- the file might be read-only, the directory might not be writable, or the agent process might not have the right user/group membership. Handling these errors gracefully is the difference between a tool that gives helpful feedback and one that crashes with a cryptic "Permission denied" panic.

## How Unix Permissions Work

On Unix systems (macOS and Linux), every file has three sets of permission bits: owner, group, and others. Each set controls read (r), write (w), and execute (x) access. When your agent process tries to read or write a file, the OS checks these bits against the process's user and group identity.

You can inspect file permissions in Rust using the `fs::metadata` function:

```rust
use std::fs;
use std::os::unix::fs::PermissionsExt;

fn describe_permissions(path: &std::path::Path) -> Result<String, String> {
    let metadata = fs::metadata(path)
        .map_err(|e| format!("Cannot read metadata for '{}': {}", path.display(), e))?;

    let permissions = metadata.permissions();
    let mode = permissions.mode();

    let file_type = if metadata.is_dir() {
        "directory"
    } else if metadata.is_symlink() {
        "symlink"
    } else {
        "file"
    };

    let readonly = permissions.readonly();

    Ok(format!(
        "{}: type={}, mode={:o}, readonly={}",
        path.display(),
        file_type,
        mode & 0o777,
        readonly
    ))
}
```

The `mode()` method returns the full Unix mode bits. We mask with `0o777` to get just the permission bits (owner/group/other read/write/execute). The `readonly()` method is a platform-independent check for whether the file can be written.

::: python Coming from Python
In Python, you check permissions with `os.access()` or by inspecting `os.stat()`:
```python
import os
import stat

def check_writable(path: str) -> bool:
    return os.access(path, os.W_OK)

def get_mode(path: str) -> str:
    st = os.stat(path)
    return oct(stat.S_IMODE(st.st_mode))
```
Rust's `std::os::unix::fs::PermissionsExt` provides the same information through the `mode()` method. The key difference is that this is a Unix-only extension -- on Windows, you would use `std::os::windows::fs::MetadataExt` instead. The `readonly()` method on `Permissions` works cross-platform, making it the safer choice for simple checks.
:::

## Pre-checking Permissions

Rather than attempting an operation and handling the error after, you can pre-check permissions to give better error messages. Here is a helper function that checks whether a read or write will succeed:

```rust
use std::path::Path;
use std::fs;

/// Check if a file can be read, providing a detailed error if not.
pub fn check_readable(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("File not found: '{}'", path.display()));
    }

    let metadata = fs::metadata(path)
        .map_err(|e| format!("Cannot access '{}': {}", path.display(), e))?;

    if metadata.is_dir() {
        return Err(format!(
            "'{}' is a directory, not a file. Use a glob pattern to list \
             directory contents.",
            path.display()
        ));
    }

    // Try opening for read to check actual permission
    match fs::File::open(path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            Err(format!(
                "Permission denied: cannot read '{}'. Check file permissions.",
                path.display()
            ))
        }
        Err(e) => Err(format!("Cannot open '{}': {}", path.display(), e)),
    }
}

/// Check if a file can be written, providing a detailed error if not.
pub fn check_writable(path: &Path) -> Result<(), String> {
    if path.exists() {
        // File exists -- check if it is writable
        let metadata = fs::metadata(path)
            .map_err(|e| format!("Cannot access '{}': {}", path.display(), e))?;

        if metadata.permissions().readonly() {
            return Err(format!(
                "File '{}' is read-only. Remove the read-only flag before editing.",
                path.display()
            ));
        }
    } else {
        // File does not exist -- check if the parent directory is writable
        let parent = path.parent().ok_or_else(|| {
            format!("Path '{}' has no parent directory", path.display())
        })?;

        if !parent.exists() {
            // Parent does not exist yet -- will be created by create_dir_all.
            // Check the nearest existing ancestor.
            let mut ancestor = parent.to_path_buf();
            while !ancestor.exists() {
                if !ancestor.pop() {
                    return Err("Cannot find any writable ancestor directory".to_string());
                }
            }
            // Check if we can write in the existing ancestor
            check_dir_writable(&ancestor)?;
        } else {
            check_dir_writable(parent)?;
        }
    }

    Ok(())
}

fn check_dir_writable(dir: &Path) -> Result<(), String> {
    if !dir.is_dir() {
        return Err(format!("'{}' is not a directory", dir.display()));
    }

    let metadata = fs::metadata(dir)
        .map_err(|e| format!("Cannot access directory '{}': {}", dir.display(), e))?;

    if metadata.permissions().readonly() {
        return Err(format!(
            "Directory '{}' is read-only. Cannot create files here.",
            dir.display()
        ));
    }

    Ok(())
}
```

These checks give the model useful feedback. Instead of a generic "Permission denied" from the OS, the model sees messages like "File is read-only" or "Directory is read-only" that it can include in its response to the user.

## Preserving Permissions During Writes

When the edit tool modifies a file, it should preserve the file's original permissions. If the user has a shell script marked as executable (`chmod +x script.sh`), the edit tool must not reset it to non-executable. Here is the pattern:

```rust
use std::fs::{self, Permissions};
use std::path::Path;

/// Save a file's current permissions before modification.
pub fn get_permissions(path: &Path) -> Option<Permissions> {
    fs::metadata(path).ok().map(|m| m.permissions())
}

/// Restore permissions after writing.
pub fn restore_permissions(path: &Path, permissions: Option<Permissions>) {
    if let Some(perms) = permissions {
        let _ = fs::set_permissions(path, perms);
    }
}
```

Integrate this into the edit tool's execute method:

```rust
fn execute(&self, input: &Value) -> Result<String, String> {
    // ... path resolution, safety checks, etc.

    let path = resolve_path(&self.base_dir, path_str)?;

    // Save original permissions before modifying
    let original_permissions = get_permissions(&path);

    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))?;

    // ... perform the replacement ...

    fs::write(&path, &new_content)
        .map_err(|e| format!("Failed to write '{}': {}", path.display(), e))?;

    // Restore original permissions
    restore_permissions(&path, original_permissions);

    Ok(format!("Edited '{}'", path_str))
}
```

The `get_permissions` call happens before we read the file, and `restore_permissions` happens after we write. The `let _ =` in `restore_permissions` silently ignores errors from `set_permissions` -- if we cannot restore the permissions, we still want the edit to succeed. A warning in the log would be appropriate here in a production system.

## Detecting Binary Files

An important permission-adjacent concern is detecting binary files. The agent should not try to read or edit binary files (executables, images, compiled objects) as text. Here is a simple heuristic:

```rust
/// Check if a file appears to be binary by reading its first bytes.
/// Returns true if the file contains null bytes in the first 8KB,
/// which is a strong indicator of binary content.
pub fn is_binary_file(path: &Path) -> Result<bool, String> {
    use std::io::Read;

    let mut file = fs::File::open(path)
        .map_err(|e| format!("Cannot open '{}': {}", path.display(), e))?;

    let mut buffer = vec![0u8; 8192];
    let bytes_read = file
        .read(&mut buffer)
        .map_err(|e| format!("Cannot read '{}': {}", path.display(), e))?;

    Ok(buffer[..bytes_read].contains(&0))
}
```

This checks the first 8KB of the file for null bytes. Text files almost never contain null bytes; binary files almost always do. It is the same heuristic Git uses to detect binary files.

Integrate the binary check into the read tool:

```rust
// In ReadFileTool::execute, before reading:
if is_binary_file(&path)? {
    let metadata = fs::metadata(&path)
        .map_err(|e| format!("Cannot read metadata: {}", e))?;
    return Ok(format!(
        "[Binary file: {} ({} bytes). Cannot display as text.]",
        path.display(),
        metadata.len()
    ));
}
```

This returns a helpful message instead of garbled binary data. The model can see the file exists and its size, and it knows not to try editing it as text.

## Handling Permission Errors Gracefully

Even with pre-checks, permission errors can still happen (another process might change permissions between your check and your operation, a network filesystem might have unpredictable behavior). Your tools should always handle the `PermissionDenied` error kind specifically:

```rust
use std::io;

fn handle_fs_error(operation: &str, path: &Path, error: io::Error) -> String {
    match error.kind() {
        io::ErrorKind::NotFound => {
            format!("File not found: '{}'", path.display())
        }
        io::ErrorKind::PermissionDenied => {
            format!(
                "Permission denied: cannot {} '{}'. \
                 Check that the file is not read-only and that you have \
                 the necessary access rights.",
                operation,
                path.display()
            )
        }
        io::ErrorKind::AlreadyExists => {
            format!("'{}' already exists", path.display())
        }
        _ => {
            format!(
                "Failed to {} '{}': {}",
                operation,
                path.display(),
                error
            )
        }
    }
}
```

Use this helper in your tools:

```rust
let content = fs::read_to_string(&path)
    .map_err(|e| handle_fs_error("read", &path, e))?;
```

The specific error messages help the model (and the user) understand what went wrong and how to fix it.

::: wild In the Wild
Claude Code checks file permissions before editing and provides detailed error messages when operations fail. It also detects binary files and refuses to display them as text, showing metadata instead. Codex takes a different approach: it attempts the operation first and interprets the OS error, arguing that pre-checks add latency and can be unreliable on certain filesystems. Both approaches are valid -- our implementation does pre-checks for better error messages but also handles post-operation errors as a fallback.
:::

## Testing Permission Handling

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    #[test]
    fn test_check_readable_existing_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("readable.txt");
        fs::write(&path, "content").unwrap();

        assert!(check_readable(&path).is_ok());
    }

    #[test]
    fn test_check_readable_nonexistent_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("missing.txt");

        let result = check_readable(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_check_readable_directory() {
        let tmp = TempDir::new().unwrap();
        let result = check_readable(tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("directory"));
    }

    #[test]
    fn test_check_writable_new_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("new.txt");
        assert!(check_writable(&path).is_ok());
    }

    #[test]
    fn test_detect_binary_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("binary.dat");
        fs::write(&path, b"\x00\x01\x02\x03").unwrap();

        assert!(is_binary_file(&path).unwrap());
    }

    #[test]
    fn test_detect_text_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("text.rs");
        fs::write(&path, "fn main() {}\n").unwrap();

        assert!(!is_binary_file(&path).unwrap());
    }

    #[test]
    fn test_preserve_permissions() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("script.sh");
        fs::write(&path, "#!/bin/bash\necho hello\n").unwrap();

        // Make executable
        fs::set_permissions(&path, Permissions::from_mode(0o755)).unwrap();

        let saved = get_permissions(&path);

        // Simulate a write that resets permissions
        fs::write(&path, "#!/bin/bash\necho goodbye\n").unwrap();

        // Restore
        restore_permissions(&path, saved);

        let perms = fs::metadata(&path).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o755);
    }
}
```

The permission preservation test is particularly important. It verifies the full cycle: save permissions, write (which resets them on some systems), restore. The `mode() & 0o777` mask strips the file type bits so we only compare the permission bits.

## Key Takeaways

- Pre-check permissions with `check_readable` and `check_writable` to provide clear error messages before operations fail.
- Detect binary files by checking for null bytes in the first 8KB, and return metadata instead of garbled content when a binary file is encountered.
- Preserve original file permissions across edits by saving with `fs::metadata` before the write and restoring with `fs::set_permissions` after, so executable scripts stay executable.
- Map `io::ErrorKind` variants to specific, helpful messages -- `PermissionDenied` should tell the user to check access rights, not just say "OS error 13."
- Permission checks are best-effort: always handle errors from the actual operation as well, because race conditions and filesystem quirks can cause pre-checks to pass while the operation still fails.
