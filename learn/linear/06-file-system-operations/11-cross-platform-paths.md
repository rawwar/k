---
title: Cross Platform Paths
description: Handling file path differences across operating systems using Rust's PathBuf and platform-agnostic path manipulation.
---

# Cross Platform Paths

> **What you'll learn:**
> - How Rust's Path and PathBuf types abstract over OS-specific path separators and conventions
> - Common pitfalls in cross-platform path handling including drive letters, UNC paths, and case sensitivity
> - How to canonicalize paths, resolve relative references, and prevent path traversal attacks safely

File paths look different on every operating system. Unix uses forward slashes (`/home/user/code`), Windows uses backslashes (`C:\Users\user\code`), and there are countless subtle differences in root representation, case sensitivity, maximum length, and reserved characters. A coding agent that works on macOS needs to work on Linux and Windows too. Rust's `Path` and `PathBuf` types help, but they don't solve everything automatically.

## Path and PathBuf Basics

Rust provides two path types, mirroring the `str`/`String` pattern:

```rust
use std::path::{Path, PathBuf};

fn path_basics() {
    // Path is an unsized reference type (like &str)
    let path: &Path = Path::new("/home/user/code/main.rs");

    // PathBuf is an owned, mutable type (like String)
    let mut path_buf: PathBuf = PathBuf::from("/home/user/code");
    path_buf.push("src");
    path_buf.push("main.rs");
    // path_buf is now "/home/user/code/src/main.rs"

    // Extract components
    println!("File name: {:?}", path.file_name());     // Some("main.rs")
    println!("Extension: {:?}", path.extension());       // Some("rs")
    println!("Stem: {:?}", path.file_stem());            // Some("main")
    println!("Parent: {:?}", path.parent());             // Some("/home/user/code")
}
```

The `push` method automatically inserts the correct path separator for the current platform. On Unix it adds `/`, on Windows it adds `\`.

::: tip Coming from Python
Rust's `Path`/`PathBuf` map directly to Python's `pathlib.PurePath`/`pathlib.Path`. Python's `Path("dir") / "file.txt"` is Rust's `PathBuf::from("dir").join("file.txt")` or using `push`. The `/` operator overload in Python is elegant, but Rust's `join` and `push` methods are explicit and work the same way. Python's `Path.name`, `Path.stem`, `Path.suffix`, and `Path.parent` correspond to Rust's `file_name()`, `file_stem()`, `extension()`, and `parent()`.
:::

## Building Paths Safely

Never construct paths by concatenating strings. Use `join` or `push`:

```rust
use std::path::{Path, PathBuf};

fn safe_path_construction() {
    // WRONG: String concatenation
    let bad = format!("{}/{}", "/home/user", "file.txt");
    // This works on Unix but fails on Windows

    // RIGHT: Use join
    let good = Path::new("/home/user").join("file.txt");
    // Uses correct separator on every platform

    // join handles trailing separators correctly
    let path1 = Path::new("/home/user/").join("file.txt");
    let path2 = Path::new("/home/user").join("file.txt");
    assert_eq!(path1, path2);

    // join with an absolute path replaces the base
    let replaced = Path::new("/home/user").join("/etc/passwd");
    assert_eq!(replaced, Path::new("/etc/passwd"));
    // This is important for security! See path traversal section below.
}
```

The fact that `join` with an absolute path replaces the entire base is a critical security consideration. If an LLM generates a path like `/etc/passwd` and you join it with your project root, the result is `/etc/passwd`, not `project_root/etc/passwd`. Always validate paths before using them.

## Path Canonicalization

Canonicalization resolves all symbolic links, `.` and `..` components, and produces an absolute path:

```rust
use std::path::Path;

fn canonicalize_example() -> Result<(), std::io::Error> {
    let relative = Path::new("./src/../src/main.rs");
    let canonical = relative.canonicalize()?;
    println!("Canonical: {}", canonical.display());
    // On Unix: /home/user/project/src/main.rs
    // On Windows: \\?\C:\Users\user\project\src\main.rs

    Ok(())
}
```

Note that `canonicalize` requires the path to exist on disk (it resolves symlinks, which requires filesystem access). If you need to normalize a path that might not exist yet, you need a manual approach:

```rust
use std::path::{Component, Path, PathBuf};

fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();

    for component in path.components() {
        match component {
            Component::ParentDir => {
                // Go up one level, but don't go above root
                if !components.is_empty() {
                    components.pop();
                }
            }
            Component::CurDir => {
                // Skip "." -- it means current directory
            }
            other => {
                components.push(other);
            }
        }
    }

    components.iter().collect()
}
```

## Preventing Path Traversal Attacks

A path traversal attack tricks your agent into reading or writing files outside the intended directory. If the LLM (or a malicious prompt) generates a path like `../../etc/shadow`, your agent must not follow it:

```rust
use std::path::{Path, PathBuf};

pub fn safe_resolve(
    base_dir: &Path,
    relative_path: &str,
) -> Result<PathBuf, String> {
    // Resolve the full path
    let candidate = base_dir.join(relative_path);

    // Canonicalize both paths to resolve symlinks and ..
    let base_canonical = base_dir.canonicalize()
        .map_err(|e| format!("Cannot resolve base: {e}"))?;
    let candidate_canonical = candidate.canonicalize()
        .map_err(|e| format!("Cannot resolve path: {e}"))?;

    // Check that the resolved path is inside the base directory
    if !candidate_canonical.starts_with(&base_canonical) {
        return Err(format!(
            "Path traversal detected: {} is outside {}",
            candidate_canonical.display(),
            base_canonical.display()
        ));
    }

    Ok(candidate_canonical)
}
```

This is essential for a coding agent. The agent typically operates within a project directory, and file operations should be restricted to that directory (or explicitly allowed directories).

::: wild In the Wild
Claude Code restricts file operations to the current working directory and its children. When the user provides a path, it is resolved against the project root and checked for traversal. This prevents the agent from reading sensitive files like SSH keys or system configuration, even if the model is prompted to do so. Codex CLI takes a similar approach with its sandbox mode, which blocks file access outside the project directory.
:::

## Platform-Specific Path Differences

### Windows Drive Letters and UNC Paths

Windows paths have features that Unix paths don't:

```rust
use std::path::Path;

fn windows_paths() {
    // Drive letter paths
    let drive = Path::new("C:\\Users\\user\\code");
    println!("Has root: {}", drive.has_root()); // true

    // UNC (Universal Naming Convention) paths for network shares
    let unc = Path::new("\\\\server\\share\\file.txt");

    // Extended-length paths (bypass 260 char limit)
    let extended = Path::new("\\\\?\\C:\\very\\long\\path");

    // All of these are valid Windows paths but would be unusual on Unix
}
```

When your agent constructs paths programmatically, use the `Path` and `PathBuf` APIs and they'll handle platform differences. When *displaying* paths to the user or LLM, use `path.display()` which formats the path according to platform convention.

### Case Sensitivity

- **Linux**: Paths are case-sensitive. `File.txt` and `file.txt` are different files.
- **macOS**: Paths are case-insensitive by default (HFS+/APFS) but case-preserving.
- **Windows**: Paths are case-insensitive and case-preserving.

This matters for your edit tool's string matching. If the LLM references `src/Main.rs` but the actual file is `src/main.rs`, it will work on macOS and Windows but fail on Linux:

```rust
use std::path::Path;

fn case_sensitive_check(
    requested: &Path,
    actual: &Path,
) -> bool {
    // On case-insensitive systems, compare lowercased
    if cfg!(target_os = "macos") || cfg!(target_os = "windows") {
        let req = requested.to_string_lossy().to_lowercase();
        let act = actual.to_string_lossy().to_lowercase();
        req == act
    } else {
        requested == actual
    }
}
```

### Maximum Path Length

- **Linux**: 4,096 bytes (PATH_MAX)
- **macOS**: 1,024 bytes (PATH_MAX)
- **Windows**: 260 characters (MAX_PATH) unless extended-length paths are used

In practice, source code paths rarely approach these limits, but generated files or deeply nested node_modules paths occasionally do.

## Path Display and OsStr

Rust paths are not strings -- they are `OsStr` under the hood, which can contain bytes that are not valid UTF-8 on Unix. This is why path methods return `OsStr` rather than `&str`:

```rust
use std::path::Path;

fn path_display() {
    let path = Path::new("/home/user/code/main.rs");

    // file_name() returns Option<&OsStr>, not Option<&str>
    let name = path.file_name().unwrap();

    // To get a &str, use to_str() which returns Option<&str>
    if let Some(name_str) = name.to_str() {
        println!("File: {}", name_str);
    }

    // For display purposes, to_string_lossy() always works
    // (replaces invalid UTF-8 with replacement character)
    println!("File: {}", name.to_string_lossy());

    // display() is the canonical way to format a path for output
    println!("Path: {}", path.display());
}
```

For a coding agent, `to_string_lossy()` is almost always fine since source code file paths are invariably valid UTF-8. But it's good to be aware of why the API uses `OsStr`.

::: tip Coming from Python
Python's `pathlib.Path` works with strings internally and handles encoding transparently. Rust makes the distinction explicit: paths are OS-native byte sequences (`OsStr`), not UTF-8 strings (`str`). This matters on Linux where filenames can contain arbitrary bytes. In practice, you'll use `.display()` for output and `.to_str()` when you need a `&str`, and it works seamlessly for all normal file paths.
:::

## Relative Path Utilities

Agents frequently need to compute relative paths for display:

```rust
use std::path::{Path, PathBuf};

fn relative_path(base: &Path, target: &Path) -> Option<PathBuf> {
    // Both must be absolute for reliable computation
    let base = base.canonicalize().ok()?;
    let target = target.canonicalize().ok()?;

    let mut base_iter = base.components().peekable();
    let mut target_iter = target.components().peekable();

    // Skip common prefix
    while let (Some(b), Some(t)) = (base_iter.peek(), target_iter.peek()) {
        if b != t {
            break;
        }
        base_iter.next();
        target_iter.next();
    }

    // Go up for remaining base components
    let mut result = PathBuf::new();
    for _ in base_iter {
        result.push("..");
    }

    // Then down through remaining target components
    for component in target_iter {
        result.push(component);
    }

    Some(result)
}
```

This is useful when displaying file paths to the user. Instead of showing `/home/user/project/src/tools/edit.rs`, you can show `src/tools/edit.rs` relative to the project root.

## Key Takeaways

- Use `Path::join` and `PathBuf::push` instead of string concatenation -- they handle platform-specific separators automatically
- Always validate that resolved paths stay within the project directory to prevent path traversal attacks
- `canonicalize()` resolves symlinks and `..` components but requires the path to exist -- use manual normalization for paths that might not exist yet
- Be aware of case sensitivity differences: Linux is case-sensitive, macOS and Windows are case-insensitive
- Rust paths use `OsStr`, not `str` -- use `.display()` for output and `.to_str()` when you need a string, with `.to_string_lossy()` as a reliable fallback
