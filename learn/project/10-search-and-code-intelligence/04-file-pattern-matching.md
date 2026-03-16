---
title: File Pattern Matching
description: Implement efficient file pattern matching combining gitignore rules, glob patterns, and file type detection for search scope control.
---

# File Pattern Matching

> **What you'll learn:**
> - How to parse and apply `.gitignore` rules to exclude files from search results
> - How to combine multiple pattern sources (gitignore, custom ignore, glob filters) into a unified matcher
> - How to detect binary files and skip them to avoid corrupted search output

Both the grep and glob tools walk directory trees, and both need to decide which files to include and which to skip. You do not want your grep tool searching inside `node_modules/`, `target/`, or `.git/` -- these directories contain thousands of files that are almost never relevant to the agent's task, and including them would drown real results in noise while burning time and tokens.

This subchapter builds a unified file filter that combines `.gitignore` rules, binary detection, and custom ignore patterns into a single reusable component that all search tools share.

## Why Filtering Matters

Consider a typical Rust project. Running `find . -type f | wc -l` in the project root might show 50 source files. But after `cargo build`, the `target/` directory alone can contain 10,000+ files -- object files, debug symbols, dependency sources, and cached artifacts. Without filtering, your grep tool spends 99% of its time searching files that are guaranteed to be irrelevant.

Production search tools like ripgrep automatically respect `.gitignore` rules, which is why they feel instant even on large projects. Your agent's search tools should do the same.

## The `ignore` Crate

The `ignore` crate (also from the ripgrep ecosystem) provides a directory walker that automatically respects `.gitignore`, `.ignore`, and `.git/info/exclude` rules. It is the single best shortcut for building a filtered directory traversal:

```toml
# In Cargo.toml
[dependencies]
ignore = "0.4"
```

Here is how it compares to plain `walkdir`:

```rust
use ignore::WalkBuilder;
use walkdir::WalkDir;

fn count_files_walkdir(path: &str) -> usize {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .count()
}

fn count_files_ignore(path: &str) -> usize {
    WalkBuilder::new(path)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .count()
}

fn main() {
    let path = ".";
    println!("walkdir: {} files", count_files_walkdir(path));
    println!("ignore:  {} files", count_files_ignore(path));
    // Typical output for a Rust project:
    // walkdir: 12847 files
    // ignore:  53 files
}
```

That is a 240x reduction in files to search. The `ignore` crate achieves this by reading `.gitignore` at each directory level during the walk, pruning entire subtrees (like `target/`) before descending into them.

::: python Coming from Python
Python has no built-in gitignore-aware walker. You would typically use `pathspec` or `gitignore-parser`:
```python
import pathspec
from pathlib import Path

# Load .gitignore rules
gitignore = Path(".gitignore").read_text()
spec = pathspec.PathSpec.from_lines("gitwildmatch", gitignore.splitlines())

for path in Path(".").rglob("*"):
    if path.is_file() and not spec.match_file(str(path)):
        print(path)
```
The Rust `ignore` crate handles the full complexity of nested `.gitignore` files, `.ignore` files, and global gitignore config automatically -- features that require significant manual work in Python.
:::

## Building a Unified File Filter

Let's build a `FileFilter` struct that wraps the `ignore` crate with additional filtering capabilities for our search tools:

```rust
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

pub struct FileFilter {
    root: PathBuf,
    include_pattern: Option<String>,
    exclude_hidden: bool,
    max_filesize: u64,
}

impl FileFilter {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            include_pattern: None,
            exclude_hidden: true,
            max_filesize: 1024 * 1024, // 1MB default max
        }
    }

    /// Only include files matching this glob pattern
    pub fn include(mut self, pattern: impl Into<String>) -> Self {
        self.include_pattern = Some(pattern.into());
        self
    }

    /// Set maximum file size to search (in bytes)
    pub fn max_filesize(mut self, bytes: u64) -> Self {
        self.max_filesize = bytes;
        self
    }

    /// Include hidden files and directories
    pub fn include_hidden(mut self) -> Self {
        self.exclude_hidden = false;
        self
    }

    /// Build a directory walker with all filters applied
    pub fn walk(&self) -> impl Iterator<Item = PathBuf> {
        let mut builder = WalkBuilder::new(&self.root);

        builder
            .hidden(self.exclude_hidden)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .max_filesize(Some(self.max_filesize));

        // Apply include pattern as an override
        if let Some(ref pattern) = self.include_pattern {
            let mut overrides = OverrideBuilder::new(&self.root);
            // The override syntax requires ! prefix for ignoring everything
            // then a positive pattern for what to include
            if overrides.add(&format!("!*")).is_ok()
                && overrides.add(pattern).is_ok()
            {
                if let Ok(built) = overrides.build() {
                    builder.overrides(built);
                }
            }
        }

        builder
            .build()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_file())
            .map(|entry| entry.into_path())
    }
}
```

Usage in the grep tool becomes straightforward:

```rust
pub fn grep_with_filter(
    pattern: &str,
    root: &str,
    include: Option<&str>,
) -> Result<Vec<GrepMatch>, String> {
    let regex = regex::Regex::new(pattern)
        .map_err(|e| format!("Invalid regex: {e}"))?;

    let mut filter = FileFilter::new(root);
    if let Some(include_pat) = include {
        filter = filter.include(include_pat.to_string());
    }

    let mut matches = Vec::new();
    for path in filter.walk() {
        if is_binary(&path) {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for (i, line) in content.lines().enumerate() {
            if regex.is_match(line) {
                matches.push(GrepMatch {
                    path: path.clone(),
                    line_number: i + 1,
                    line_content: line.to_string(),
                    context_before: Vec::new(),
                    context_after: Vec::new(),
                });
            }
        }
    }

    Ok(matches)
}
```

## Binary File Detection

Binary files are the enemy of text search. Grepping a compiled binary produces lines of garbage that look like `^@^C^E^BELF^B^A^A` and waste context window tokens. Here is a robust binary detection function:

```rust
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Detect whether a file is binary by checking for null bytes
/// and the ratio of non-printable characters.
pub fn is_binary(path: &Path) -> bool {
    let Ok(mut file) = File::open(path) else {
        return true; // Cannot open, assume binary
    };

    let mut buffer = [0u8; 8192];
    let bytes_read = match file.read(&mut buffer) {
        Ok(0) => return false, // Empty files are not binary
        Ok(n) => n,
        Err(_) => return true,
    };

    let sample = &buffer[..bytes_read];

    // Null bytes are a strong indicator of binary content
    if sample.contains(&0) {
        return true;
    }

    // Count non-text bytes (control chars excluding common whitespace)
    let non_text_count = sample
        .iter()
        .filter(|&&b| {
            b < 0x20 // Control characters
            && b != b'\n'
            && b != b'\r'
            && b != b'\t'
        })
        .count();

    // If more than 10% of bytes are non-text, treat as binary
    non_text_count as f64 / bytes_read as f64 > 0.10
}
```

This approach uses two heuristics: null byte detection (catches most binary formats) and control character ratio (catches files with unusual encodings). The 10% threshold is generous enough to pass files with a few control characters (like form feeds in old C headers) while catching true binary content.

## Custom Ignore Patterns

Beyond `.gitignore`, you may want the agent to respect a project-specific `.agentignore` file that excludes files the agent should never search -- vendor directories, generated code, large data files:

```rust
use std::fs;
use std::path::Path;

pub fn load_agent_ignore_patterns(root: &Path) -> Vec<String> {
    let ignore_path = root.join(".agentignore");

    let content = match fs::read_to_string(&ignore_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .map(|line| line.trim().to_string())
        .collect()
}
```

An example `.agentignore` file:

```gitignore
# Large generated files
*.min.js
*.min.css
*.bundle.js

# Vendor directories
vendor/
third_party/

# Data files
*.csv
*.parquet
fixtures/large/
```

::: wild In the Wild
Claude Code respects a `.claudeignore` file that uses gitignore syntax. This lets developers exclude files from the agent's view -- build outputs, secrets, or large data files that would overwhelm the context window. OpenCode has a similar concept through its configuration file. Supporting a custom ignore file is a small feature with outsized impact on agent usability.
:::

## Putting It All Together

Here is the enhanced `FileFilter` that integrates custom ignore patterns:

```rust
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

pub struct FileFilter {
    root: PathBuf,
    include_pattern: Option<String>,
    exclude_hidden: bool,
    max_filesize: u64,
    custom_ignores: Vec<String>,
}

impl FileFilter {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let custom_ignores = load_agent_ignore_patterns(&root);

        Self {
            root,
            include_pattern: None,
            exclude_hidden: true,
            max_filesize: 1024 * 1024,
            custom_ignores,
        }
    }

    pub fn include(mut self, pattern: impl Into<String>) -> Self {
        self.include_pattern = Some(pattern.into());
        self
    }

    pub fn max_filesize(mut self, bytes: u64) -> Self {
        self.max_filesize = bytes;
        self
    }

    pub fn walk(&self) -> impl Iterator<Item = PathBuf> {
        let mut builder = WalkBuilder::new(&self.root);

        builder
            .hidden(self.exclude_hidden)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .max_filesize(Some(self.max_filesize));

        // Build overrides for include pattern and custom ignores
        let mut overrides = OverrideBuilder::new(&self.root);

        // Add custom ignore patterns (negated = excluded)
        for pattern in &self.custom_ignores {
            let _ = overrides.add(&format!("!{pattern}"));
        }

        // Add include pattern if specified
        if let Some(ref include) = self.include_pattern {
            let _ = overrides.add(include);
        }

        if let Ok(built) = overrides.build() {
            builder.overrides(built);
        }

        builder
            .build()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_file())
            .filter(|entry| !is_binary(entry.path()))
            .map(|entry| entry.into_path())
    }
}
```

## Testing File Filters

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_gitignore_respected() {
        let dir = TempDir::new().unwrap();

        // Create a .gitignore that excludes build/
        fs::write(dir.path().join(".gitignore"), "build/\n").unwrap();
        fs::create_dir(dir.path().join("build")).unwrap();
        fs::write(dir.path().join("build/output.js"), "compiled code").unwrap();
        fs::write(dir.path().join("src.rs"), "fn main() {}").unwrap();

        let files: Vec<PathBuf> = FileFilter::new(dir.path()).walk().collect();

        let filenames: Vec<&str> = files
            .iter()
            .filter_map(|p| p.file_name()?.to_str())
            .collect();

        assert!(filenames.contains(&"src.rs"));
        assert!(!filenames.contains(&"output.js"));
    }

    #[test]
    fn test_binary_detection() {
        let dir = TempDir::new().unwrap();

        // Create a text file
        fs::write(dir.path().join("source.rs"), "fn main() {}").unwrap();

        // Create a binary file (with null bytes)
        fs::write(
            dir.path().join("binary.dat"),
            b"\x00\x01\x02\x03compiled\x00",
        ).unwrap();

        assert!(!is_binary(&dir.path().join("source.rs")));
        assert!(is_binary(&dir.path().join("binary.dat")));
    }
}
```

## Key Takeaways

- Use the `ignore` crate instead of raw `walkdir` to automatically respect `.gitignore` rules -- this can reduce the number of files searched by 100x or more in typical projects.
- Binary file detection using null byte checks and control character ratios prevents garbage output from wasting context window tokens.
- A custom `.agentignore` file gives developers explicit control over what the agent can search, complementing `.gitignore` with agent-specific exclusions.
- The `FileFilter` struct encapsulates all filtering logic in one place, so grep, glob, and future search tools share the same behavior.
- Always apply file size limits to prevent the search tools from attempting to read and process multi-megabyte files that would overwhelm the context window.
