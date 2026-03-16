---
title: Path Handling
description: Resolve, canonicalize, and normalize file paths to ensure tools operate on the correct files safely.
---

# Path Handling

> **What you'll learn:**
> - How to resolve relative paths against the project root and canonicalize them to eliminate symlinks and `..` components
> - How to normalize paths across platforms so tools work consistently on macOS, Linux, and Windows
> - How to define an allowed base directory and reject any path that resolves outside it

Path handling sounds simple until you start thinking about all the ways a path can be ambiguous. The model might send `src/main.rs`, `./src/main.rs`, `../other-project/secret.txt`, or `/etc/passwd`. Your file tools need to resolve all of these unambiguously and reject the dangerous ones. This subchapter builds the path resolution layer that sits between the raw input from the model and the actual filesystem operations.

## The Problem with Raw Paths

When the model sends a path like `src/main.rs`, what does that mean? It depends on what directory your agent is running in. If the agent was started in `/home/user/myproject`, then `src/main.rs` should resolve to `/home/user/myproject/src/main.rs`. But what about these cases?

- `../secrets/api_key.txt` -- tries to escape the project directory
- `/etc/shadow` -- an absolute path to a sensitive system file
- `src/../../etc/passwd` -- looks relative but escapes via `..`
- `src/link` -- a symlink that points outside the project

Every one of these is a potential security issue. The path resolution layer must handle all of them.

## Building the Path Resolver

Let's create a dedicated module for path handling. Create `src/tools/paths.rs`:

```rust
use std::path::{Path, PathBuf};
use std::io;

/// Resolves and validates a path against a base directory.
/// Returns the canonical (absolute, symlink-resolved) path if it falls
/// within the base directory, or an error if it escapes.
pub fn resolve_path(base_dir: &Path, requested: &str) -> Result<PathBuf, String> {
    let requested_path = Path::new(requested);

    // Step 1: If the path is absolute, use it directly.
    // If relative, join it with the base directory.
    let joined = if requested_path.is_absolute() {
        PathBuf::from(requested)
    } else {
        base_dir.join(requested)
    };

    // Step 2: Canonicalize the base directory
    let canonical_base = base_dir
        .canonicalize()
        .map_err(|e| format!("Cannot resolve base directory '{}': {}", base_dir.display(), e))?;

    // Step 3: For existing paths, canonicalize to resolve symlinks and ..
    // For new paths (write operations), canonicalize the parent.
    let canonical = if joined.exists() {
        joined
            .canonicalize()
            .map_err(|e| format!("Cannot resolve path '{}': {}", joined.display(), e))?
    } else {
        // The file doesn't exist yet (it might be a write target).
        // Canonicalize the parent directory, then append the file name.
        let parent = joined.parent().ok_or_else(|| {
            format!("Path '{}' has no parent directory", requested)
        })?;

        // The parent must exist for us to write into it
        // (create_dir_all happens later, but we resolve against what exists now)
        let canonical_parent = if parent.exists() {
            parent.canonicalize().map_err(|e| {
                format!("Cannot resolve parent '{}': {}", parent.display(), e)
            })?
        } else {
            // Walk up to find the nearest existing ancestor
            resolve_nearest_ancestor(parent, &canonical_base)?
        };

        let file_name = joined.file_name().ok_or_else(|| {
            format!("Path '{}' has no file name", requested)
        })?;

        canonical_parent.join(file_name)
    };

    // Step 4: Verify the resolved path starts with the base directory
    if !canonical.starts_with(&canonical_base) {
        return Err(format!(
            "Path '{}' resolves to '{}' which is outside the allowed directory '{}'",
            requested,
            canonical.display(),
            canonical_base.display()
        ));
    }

    Ok(canonical)
}

/// Walk up the directory tree until we find an ancestor that exists,
/// canonicalize it, then re-append the remaining components.
fn resolve_nearest_ancestor(path: &Path, base: &Path) -> Result<PathBuf, String> {
    let mut current = path.to_path_buf();
    let mut components_to_add = Vec::new();

    while !current.exists() {
        if let Some(name) = current.file_name() {
            components_to_add.push(name.to_os_string());
        }
        if !current.pop() {
            break;
        }
    }

    let canonical = current.canonicalize().map_err(|e| {
        format!("Cannot resolve ancestor '{}': {}", current.display(), e)
    })?;

    // Safety check: the existing ancestor must be within the base
    if !canonical.starts_with(base) {
        return Err(format!(
            "Path resolves outside the allowed directory '{}'",
            base.display()
        ));
    }

    // Re-build the path from the canonical ancestor
    let mut result = canonical;
    for component in components_to_add.into_iter().rev() {
        result.push(component);
    }
    Ok(result)
}
```

This is a substantial piece of code, so let's walk through each step.

**Step 1: Handle absolute vs. relative.** If the model sends an absolute path like `/etc/passwd`, we keep it as-is for the canonicalization step (where it will fail the boundary check). If it sends a relative path like `src/main.rs`, we join it with the base directory.

**Step 2: Canonicalize the base.** We need the canonical form of the base directory to compare against later. `canonicalize()` resolves symlinks and produces an absolute path with no `.` or `..` components.

**Step 3: Canonicalize the target.** For existing files, `canonicalize()` resolves the full path. For new files (the write tool might be creating a file that does not exist yet), we canonicalize the parent directory and append the filename. This handles the tricky case where the file does not exist but the directory does.

**Step 4: Boundary check.** After canonicalization, we check that the resolved path starts with the canonical base directory. This is the security gate -- it catches `..` escapes, symlink escapes, and absolute paths outside the project.

::: python Coming from Python
In Python, you would use `pathlib` for this:
```python
from pathlib import Path

def resolve_path(base_dir: str, requested: str) -> Path:
    base = Path(base_dir).resolve()
    target = (base / requested).resolve()

    if not str(target).startswith(str(base)):
        raise ValueError(f"Path escapes base directory: {requested}")

    return target
```
The Python version is shorter because `Path.resolve()` handles both canonicalization and `..` resolution in one call. Rust's `canonicalize()` does the same thing, but you must handle the `io::Result` explicitly. Also note the subtle bug in the Python version: `str(target).startswith(str(base))` can give a false positive if the base is `/home/user` and the target is `/home/user2/file`. Rust's `Path::starts_with` does component-wise comparison, which is correct.
:::

## Updating the File Tools

Now integrate the path resolver into the file tools. In each tool's `execute` method, replace the simple `self.base_dir.join(path_str)` with a call to `resolve_path`:

```rust
use crate::tools::paths::resolve_path;

// In ReadFileTool::execute
fn execute(&self, input: &Value) -> Result<String, String> {
    let path_str = input
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: path".to_string())?;

    // Use resolve_path instead of simple join
    let path = resolve_path(&self.base_dir, path_str)?;

    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read file '{}': {}", path.display(), e))?;

    // ... rest of the method
}
```

Apply the same change to `WriteFileTool::execute` and `EditFileTool::execute`. The pattern is identical: extract the path string, resolve it, and use the resolved path for all filesystem operations.

## Platform Differences

Path handling has some platform-specific behaviors you should be aware of.

**Path separators.** On Unix systems (macOS, Linux), paths use `/`. On Windows, paths use `\` but also accept `/`. Rust's `Path` and `PathBuf` handle this automatically -- you can use `/` in your code and it works everywhere.

**Case sensitivity.** On macOS (HFS+/APFS), the filesystem is case-insensitive by default: `README.md` and `readme.md` refer to the same file. On Linux (ext4), they are different files. `canonicalize()` returns the actual casing from the filesystem, which helps, but you should be aware that path comparisons might behave differently across platforms.

**Symlink resolution.** `canonicalize()` follows symlinks to their real target. If `src/utils` is a symlink to `../shared/utils`, the canonical path will be the real location. This is important for the boundary check -- the real location must be within the base directory, not just the symlink.

## Testing Path Resolution

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_resolve_relative_path() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("hello.txt"), "hi").unwrap();

        let result = resolve_path(tmp.path(), "hello.txt");
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert!(resolved.ends_with("hello.txt"));
    }

    #[test]
    fn test_reject_parent_traversal() {
        let tmp = TempDir::new().unwrap();

        let result = resolve_path(tmp.path(), "../../../etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("outside the allowed directory"));
    }

    #[test]
    fn test_reject_absolute_path_outside_base() {
        let tmp = TempDir::new().unwrap();

        let result = resolve_path(tmp.path(), "/etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("outside the allowed directory"));
    }

    #[test]
    fn test_resolve_new_file_in_existing_dir() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("src")).unwrap();

        let result = resolve_path(tmp.path(), "src/new_file.rs");
        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("new_file.rs"));
    }

    #[test]
    fn test_resolve_subdirectory_path() {
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("src/tools");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("mod.rs"), "// tools").unwrap();

        let result = resolve_path(tmp.path(), "src/tools/mod.rs");
        assert!(result.is_ok());
    }

    #[test]
    fn test_dot_components_resolved() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("file.txt"), "content").unwrap();

        let result = resolve_path(tmp.path(), "./file.txt");
        assert!(result.is_ok());
    }
}
```

These tests cover the essential cases: normal relative paths, parent traversal attacks, absolute path escapes, new file creation, subdirectories, and dot components. Each test uses `TempDir` so there are no dependencies on any specific filesystem layout.

## Displaying Paths to the Model

When reporting paths back to the model in tool results, use paths relative to the base directory rather than absolute paths. Absolute paths leak information about the host machine (usernames, directory structure) and consume unnecessary tokens.

```rust
/// Convert an absolute path back to a path relative to the base directory.
/// Falls back to the absolute path if the relative conversion fails.
pub fn display_path(base_dir: &Path, absolute: &Path) -> String {
    absolute
        .strip_prefix(base_dir)
        .unwrap_or(absolute)
        .display()
        .to_string()
}
```

Use this in your tool result messages:

```rust
// Instead of: format!("Read {}", path.display())
// Use:        format!("Read {}", display_path(&self.base_dir, &path))
```

This keeps the tool output clean and focused on project-relative paths that the model can use directly in future tool calls.

## Key Takeaways

- `canonicalize()` resolves symlinks, `..` components, and produces absolute paths -- it is the foundation of safe path resolution in Rust.
- The boundary check (`canonical.starts_with(&canonical_base)`) must happen after canonicalization, not before, because `..` components are only eliminated during canonicalization.
- New files that do not yet exist require special handling: canonicalize the parent directory and append the filename, since `canonicalize()` fails on nonexistent paths.
- Rust's `Path::starts_with` does component-wise comparison (not string prefix matching), so `/home/user` does not accidentally match `/home/user2` -- a common bug in string-based approaches.
- Always display paths relative to the base directory in tool results to keep output clean and avoid leaking host filesystem information.
