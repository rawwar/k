---
title: Glob Patterns
description: Implement glob-based file searching so the agent can discover files matching patterns like `**/*.rs` or `src/*.toml`.
---

# Glob Patterns

> **What you'll learn:**
> - How to use the `glob` crate to expand patterns like `**/*.rs` into a list of matching file paths
> - How to implement a GlobSearch tool that returns matching paths sorted by relevance or modification time
> - How to set limits on glob expansion to prevent accidental enumeration of massive directory trees

Before the agent can read or edit a file, it needs to know that file exists. Often the model does not know the exact path -- it knows it needs a Rust source file somewhere in `src/`, or a TOML configuration file at the project root. The GlobSearch tool lets the model discover files by pattern, returning a list of matching paths it can then pass to the read or edit tools.

## Adding the `glob` Crate

Add the `glob` crate to your `Cargo.toml`:

```toml
[dependencies]
glob = "0.3"
```

The `glob` crate supports standard Unix glob syntax:
- `*` matches any sequence of characters within a single directory component
- `**` matches any number of directories (recursive)
- `?` matches exactly one character
- `[abc]` matches any character in the set
- `{a,b}` matches either `a` or `b` (brace expansion)

## The GlobSearch Tool

Create `src/tools/glob_search.rs`:

```rust
use crate::tools::Tool;
use glob::glob;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::SystemTime;

pub struct GlobSearchTool {
    pub base_dir: PathBuf,
    /// Maximum number of results to return
    pub max_results: usize,
}

impl GlobSearchTool {
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            max_results: 100,
        }
    }

    pub fn with_max_results(mut self, max: usize) -> Self {
        self.max_results = max;
        self
    }
}
```

The `max_results` field is critical. A pattern like `**/*` in a node_modules directory could return hundreds of thousands of files. Without a limit, the tool would consume enormous memory and flood the model's context window.

## Implementing the Tool Trait

```rust
impl Tool for GlobSearchTool {
    fn name(&self) -> &str {
        "glob_search"
    }

    fn description(&self) -> &str {
        "Search for files matching a glob pattern. Returns a list of matching file \
         paths relative to the project root, sorted by modification time (most \
         recent first). Use patterns like '**/*.rs' to find all Rust files or \
         'src/**/*.toml' to find TOML files under src/."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The glob pattern to match against file paths. \
                                    Supports *, **, ?, and [abc] wildcards."
                }
            },
            "required": ["pattern"]
        })
    }

    fn execute(&self, input: &Value) -> Result<String, String> {
        let pattern_str = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: pattern".to_string())?;

        // Build the full pattern by joining with the base directory
        let full_pattern = self.base_dir.join(pattern_str);
        let pattern_string = full_pattern.to_string_lossy().to_string();

        // Expand the glob pattern
        let entries = glob(&pattern_string).map_err(|e| {
            format!("Invalid glob pattern '{}': {}", pattern_str, e)
        })?;

        // Collect matching paths with their modification times
        let mut matches: Vec<(PathBuf, SystemTime)> = Vec::new();

        for entry in entries {
            match entry {
                Ok(path) => {
                    // Skip directories -- we only want files
                    if path.is_dir() {
                        continue;
                    }

                    // Get modification time for sorting
                    let modified = path
                        .metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(SystemTime::UNIX_EPOCH);

                    matches.push((path, modified));

                    // Stop collecting if we've hit the limit
                    // (collect a bit more than max for sorting)
                    if matches.len() > self.max_results * 2 {
                        break;
                    }
                }
                Err(_) => continue, // Skip entries we can't access
            }
        }

        // Sort by modification time, most recent first
        matches.sort_by(|a, b| b.1.cmp(&a.1));

        // Truncate to max_results
        matches.truncate(self.max_results);

        if matches.is_empty() {
            return Ok(format!(
                "No files found matching pattern '{}'",
                pattern_str
            ));
        }

        // Format the results as relative paths
        let total_found = matches.len();
        let result_lines: Vec<String> = matches
            .iter()
            .map(|(path, _)| {
                path.strip_prefix(&self.base_dir)
                    .unwrap_or(path)
                    .display()
                    .to_string()
            })
            .collect();

        let mut output = format!(
            "Found {} file(s) matching '{}':\n",
            total_found, pattern_str
        );
        output.push_str(&result_lines.join("\n"));

        Ok(output)
    }
}
```

Let's examine the key design decisions.

**Sorting by modification time.** The most recently modified files appear first. This is usually what the model wants -- when looking for a Rust file to edit, the one modified most recently is likely the most relevant. This matches how Claude Code's Glob tool orders results.

**Skipping directories.** The tool returns only files, not directories. The model uses this tool to find files it can then read or edit -- directory entries are noise.

**Double-collect for sorting.** We collect up to `max_results * 2` entries before sorting. This gives the sorting step a larger pool to work with, so the "most recent" results are more accurate even when the glob matches far more files than the limit.

**Silently skipping errors.** Permission-denied entries are skipped rather than causing the whole operation to fail. In a typical project, there might be a few unreadable files in `node_modules` or `.git` -- these should not prevent the tool from returning the files it can access.

::: python Coming from Python
In Python, glob matching is built into `pathlib`:
```python
from pathlib import Path

def glob_search(base_dir: str, pattern: str, max_results: int = 100) -> list[str]:
    base = Path(base_dir)
    matches = sorted(
        [p for p in base.glob(pattern) if p.is_file()],
        key=lambda p: p.stat().st_mtime,
        reverse=True,
    )[:max_results]
    return [str(p.relative_to(base)) for p in matches]
```
Python's `Path.glob()` handles recursive `**` patterns natively. In Rust, the `glob` crate provides similar functionality but works with string patterns rather than path objects. The Rust version also needs explicit error handling for each matched entry.
:::

## Common Glob Patterns for Coding Agents

Here are the patterns the model will use most frequently:

| Pattern | Matches |
|---------|---------|
| `**/*.rs` | All Rust source files, recursively |
| `src/**/*.rs` | Rust files under src/ only |
| `*.toml` | TOML files in the project root |
| `**/Cargo.toml` | All Cargo.toml files (workspace members) |
| `src/**/mod.rs` | All module root files |
| `tests/**/*.rs` | All test files |
| `**/*.{rs,toml}` | All Rust and TOML files |

Including examples like these in the tool description helps the model use effective patterns on its first attempt.

## Filtering Hidden and Ignored Files

By default, the glob crate does not skip hidden files (those starting with `.`). You probably want to skip `.git/`, `node_modules/`, and other directories that contain noise. Here is a filtering function:

```rust
/// Directories to skip during glob expansion
const IGNORED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "__pycache__",
    ".next",
    "dist",
    "build",
    ".cache",
];

/// Check if a path contains any ignored directory component.
fn is_in_ignored_dir(path: &std::path::Path) -> bool {
    path.components().any(|component| {
        if let std::path::Component::Normal(name) = component {
            IGNORED_DIRS
                .iter()
                .any(|ignored| name.to_string_lossy() == *ignored)
        } else {
            false
        }
    })
}
```

Add this filter to the glob expansion loop:

```rust
for entry in entries {
    match entry {
        Ok(path) => {
            if path.is_dir() || is_in_ignored_dir(&path) {
                continue;
            }
            // ... rest of processing
        }
        Err(_) => continue,
    }
}
```

This prevents the agent from wading through thousands of dependency files to find the project's actual source code.

## Testing the Glob Tool

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_project(tmp: &TempDir) {
        let dirs = ["src", "src/tools", "tests", "docs"];
        for dir in &dirs {
            fs::create_dir_all(tmp.path().join(dir)).unwrap();
        }

        let files = [
            ("src/main.rs", "fn main() {}"),
            ("src/lib.rs", "pub mod tools;"),
            ("src/tools/mod.rs", "pub mod read;"),
            ("src/tools/read.rs", "pub fn read() {}"),
            ("tests/integration.rs", "#[test] fn it_works() {}"),
            ("Cargo.toml", "[package]\nname = \"agent\""),
            ("docs/README.md", "# Agent"),
        ];

        for (path, content) in &files {
            fs::write(tmp.path().join(path), content).unwrap();
        }
    }

    #[test]
    fn test_find_all_rust_files() {
        let tmp = TempDir::new().unwrap();
        setup_project(&tmp);

        let tool = GlobSearchTool::new(tmp.path().to_path_buf());
        let result = tool.execute(&json!({"pattern": "**/*.rs"}));

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("main.rs"));
        assert!(output.contains("lib.rs"));
        assert!(output.contains("read.rs"));
        assert!(output.contains("integration.rs"));
    }

    #[test]
    fn test_find_files_in_subdirectory() {
        let tmp = TempDir::new().unwrap();
        setup_project(&tmp);

        let tool = GlobSearchTool::new(tmp.path().to_path_buf());
        let result = tool.execute(&json!({"pattern": "src/tools/*.rs"}));

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("mod.rs"));
        assert!(output.contains("read.rs"));
        assert!(!output.contains("main.rs"));
    }

    #[test]
    fn test_no_matches() {
        let tmp = TempDir::new().unwrap();
        setup_project(&tmp);

        let tool = GlobSearchTool::new(tmp.path().to_path_buf());
        let result = tool.execute(&json!({"pattern": "**/*.py"}));

        assert!(result.is_ok());
        assert!(result.unwrap().contains("No files found"));
    }

    #[test]
    fn test_max_results_limit() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("many")).unwrap();
        for i in 0..50 {
            fs::write(
                tmp.path().join(format!("many/file{}.txt", i)),
                format!("content {}", i),
            )
            .unwrap();
        }

        let tool = GlobSearchTool::new(tmp.path().to_path_buf())
            .with_max_results(10);
        let result = tool.execute(&json!({"pattern": "many/*.txt"}));

        assert!(result.is_ok());
        let output = result.unwrap();
        // Should only show 10 files
        let file_count = output.lines().filter(|l| l.contains("file")).count();
        assert!(file_count <= 10);
    }

    #[test]
    fn test_invalid_pattern() {
        let tmp = TempDir::new().unwrap();
        let tool = GlobSearchTool::new(tmp.path().to_path_buf());
        let result = tool.execute(&json!({"pattern": "[invalid"}));

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid glob pattern"));
    }
}
```

The tests cover the main scenarios: recursive search, subdirectory search, no matches, result limiting, and invalid patterns. The `setup_project` helper creates a realistic directory structure that mirrors a small Rust project.

::: wild In the Wild
Claude Code's Glob tool returns results sorted by modification time, matching our implementation. It also filters out common noise directories like `node_modules` and `.git` by default. The tool is one of the most frequently used in practice -- the model calls it at the start of almost every task to discover the relevant files before reading or editing them. OpenCode uses a similar search tool but adds file size information to each result, which helps the model decide whether to read the full file or use range-based reading.
:::

## Key Takeaways

- The `glob` crate expands patterns like `**/*.rs` into lists of matching file paths, supporting wildcards (`*`, `**`, `?`) and character sets (`[abc]`).
- Result limits prevent runaway expansion -- a pattern like `**/*` in a large project could match hundreds of thousands of files without a cap.
- Sorting by modification time puts the most relevant files first, which is what the model typically wants when searching for code to read or edit.
- Filtering hidden and ignored directories (`.git`, `node_modules`, `target`) removes noise and keeps results focused on the project's actual source code.
- The glob tool bridges the gap between the model's knowledge ("I need to find a Rust file about tools") and the read/edit tools' requirement for exact paths.
