---
title: Glob Tool
description: Implement a fast file pattern matching tool that finds files by name using glob patterns across directory trees.
---

# Glob Tool

> **What you'll learn:**
> - How to use the `glob` and `globset` crates to match file paths against patterns like `**/*.rs` and `src/{lib,main}.rs`
> - How to optimize directory traversal by pruning non-matching branches early
> - How to present glob results sorted by relevance and modification time

Where the grep tool searches file *contents*, the glob tool searches file *names*. When the LLM needs to find all Rust source files, locate test files, or discover configuration files with a specific naming pattern, it reaches for glob. This tool is the agent's directory-level awareness -- it answers "what files exist?" so the agent can decide which ones to read or search.

## Glob Patterns 101

Glob patterns are simpler than regular expressions and purpose-built for file path matching. If you have used shell wildcards, you already know the basics:

| Pattern | Matches | Example |
|---------|---------|---------|
| `*` | Any sequence of non-separator characters | `*.rs` matches `main.rs` but not `src/main.rs` |
| `**` | Any sequence of characters including path separators | `**/*.rs` matches `src/main.rs` and `lib/utils/helpers.rs` |
| `?` | Any single character | `test?.rs` matches `test1.rs` but not `test12.rs` |
| `{a,b}` | Either `a` or `b` | `*.{rs,toml}` matches both `.rs` and `.toml` files |
| `[abc]` | Any one of the characters listed | `[Mm]akefile` matches `Makefile` and `makefile` |

The `**` pattern (sometimes called a "globstar") is the most important for a coding agent. It enables recursive matching, which is how you search an entire project tree.

::: tip Coming from Python
Python's `pathlib.Path.glob()` and `Path.rglob()` support the same basic patterns:
```python
from pathlib import Path

# Find all Rust files recursively
for path in Path(".").rglob("*.rs"):
    print(path)

# Find specific config files
for path in Path(".").glob("**/Cargo.toml"):
    print(path)
```
The Rust `glob` crate works similarly but returns an iterator of `Result<PathBuf>` values, so you handle errors per-entry rather than getting a runtime exception partway through.
:::

## Choosing Between Glob Crates

Rust has several glob crates, each with different trade-offs:

- **`glob`** -- The standard-library-adjacent crate. Simple API, matches one pattern at a time. Good for basic use cases.
- **`globset`** -- Part of the `grep` family of crates (from the ripgrep author). Compiles multiple patterns into an optimized automaton for fast matching. Better for filtering during directory walks.
- **`globwalk`** -- Combines `walkdir` with `globset` for a single-call "walk and filter" API. Convenient but less flexible.

For our agent, we will use `globset` because it integrates well with directory walking and supports the brace expansion syntax (`{a,b}`) that LLMs commonly generate:

```toml
# In Cargo.toml
[dependencies]
globset = "0.4"
walkdir = "2"
```

## Implementing the Glob Tool

The glob tool takes a pattern, walks the directory tree, and returns matching file paths. Here is the complete implementation:

```rust
use globset::{Glob, GlobMatcher};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

#[derive(Debug, Deserialize, Serialize)]
pub struct GlobToolInput {
    /// The glob pattern to match files against (e.g., "**/*.rs")
    pub pattern: String,

    /// Directory to search in (defaults to current working directory)
    #[serde(default)]
    pub path: Option<String>,

    /// Maximum number of results to return
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    100
}

#[derive(Debug)]
pub struct GlobResult {
    pub path: PathBuf,
    pub modified: Option<SystemTime>,
    pub size: u64,
    pub is_dir: bool,
}

pub fn glob_search(input: &GlobToolInput) -> Result<Vec<GlobResult>, String> {
    // Compile the glob pattern
    let glob = Glob::new(&input.pattern)
        .map_err(|e| format!("Invalid glob pattern '{}': {e}", input.pattern))?;
    let matcher: GlobMatcher = glob.compile_matcher();

    let search_path = input.path.as_deref().unwrap_or(".");

    let mut results: Vec<GlobResult> = Vec::new();

    for entry in WalkDir::new(search_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Get relative path for matching
        let relative = path
            .strip_prefix(search_path)
            .unwrap_or(path);

        // Skip the root directory itself
        if relative.as_os_str().is_empty() {
            continue;
        }

        if matcher.is_match(relative) {
            let metadata = entry.metadata().ok();
            results.push(GlobResult {
                path: path.to_path_buf(),
                modified: metadata.as_ref().and_then(|m| m.modified().ok()),
                size: metadata.as_ref().map(|m| m.len()).unwrap_or(0),
                is_dir: path.is_dir(),
            });
        }

        // Stop early if we have enough results
        if results.len() >= input.limit * 2 {
            break;
        }
    }

    // Sort by modification time (most recent first)
    results.sort_by(|a, b| {
        b.modified
            .unwrap_or(SystemTime::UNIX_EPOCH)
            .cmp(&a.modified.unwrap_or(SystemTime::UNIX_EPOCH))
    });

    results.truncate(input.limit);
    Ok(results)
}
```

### Key Design Decisions

**Relative path matching.** We strip the search path prefix before matching so that patterns like `**/*.rs` work correctly regardless of the absolute path. Without this, a pattern like `src/*.rs` would fail to match `/home/user/project/src/main.rs`.

**Collecting extra results before sorting.** We gather up to `limit * 2` results before sorting and truncating. This gives the sort a larger pool to draw from, so the most recently modified files bubble to the top. For a truly large codebase you might want to use a bounded priority queue instead, but this approach is simple and effective for typical projects.

**Modification time sorting.** Recently modified files are usually more relevant to the current task. When the agent asks "find all test files," the ones modified today are more likely to be the tests it needs to look at than tests that haven't changed in months.

## Formatting Results

The glob tool output should be clean, scannable, and token-efficient:

```rust
use std::time::UNIX_EPOCH;

pub fn format_glob_results(results: &[GlobResult]) -> String {
    if results.is_empty() {
        return "No files matched the pattern.".to_string();
    }

    let mut output = String::new();
    output.push_str(&format!("Found {} matching files:\n\n", results.len()));

    for result in results {
        let size_str = format_file_size(result.size);
        let modified_str = result
            .modified
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| {
                let secs = d.as_secs();
                let hours_ago = (SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    .saturating_sub(secs))
                    / 3600;
                if hours_ago < 1 {
                    "just now".to_string()
                } else if hours_ago < 24 {
                    format!("{hours_ago}h ago")
                } else {
                    format!("{}d ago", hours_ago / 24)
                }
            })
            .unwrap_or_else(|| "unknown".to_string());

        let type_marker = if result.is_dir { "dir " } else { "    " };
        output.push_str(&format!(
            "{type_marker}{} ({size_str}, {modified_str})\n",
            result.path.display()
        ));
    }

    output
}

fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes}B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
```

The relative timestamps ("2h ago", "3d ago") are more useful to the LLM than absolute dates -- they convey recency without requiring the model to do date arithmetic.

## Wiring Up the Tool Trait

```rust
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files by name pattern using glob syntax. Supports ** for \
         recursive matching, {a,b} for alternatives, and ? for single \
         characters. Use this to discover project structure, find test \
         files, locate configs, or identify all files of a specific type."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern (e.g., '**/*.rs', 'src/**/*.{ts,tsx}')"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (defaults to cwd)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results to return (default 100)"
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<String, String> {
        let input: GlobToolInput = serde_json::from_value(input)
            .map_err(|e| format!("Invalid input: {e}"))?;

        let results = glob_search(&input)?;
        Ok(format_glob_results(&results))
    }
}
```

::: info In the Wild
Claude Code's glob tool returns results sorted by modification time, with the most recently changed files first. This heuristic works well because the agent's task usually involves files that were recently created or modified. OpenCode takes a similar approach but also tracks which files the user has mentioned in conversation, boosting those paths in search results.
:::

## Handling Edge Cases

Glob patterns from the LLM can be surprisingly creative -- or broken. Let's handle the common pitfalls:

```rust
pub fn normalize_glob_pattern(pattern: &str) -> String {
    let mut normalized = pattern.to_string();

    // If the pattern has no path separators or **, prepend **/ to make it recursive
    if !normalized.contains('/') && !normalized.starts_with("**") {
        normalized = format!("**/{normalized}");
    }

    // Remove leading ./ if present
    if normalized.starts_with("./") {
        normalized = normalized[2..].to_string();
    }

    normalized
}
```

This normalization handles a common LLM behavior: sending just `*.rs` when it means `**/*.rs`. Without the `**/` prefix, the pattern only matches files in the root directory. The normalization adds it automatically, which matches what the LLM intended.

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_tree(dir: &Path) {
        fs::create_dir_all(dir.join("src/utils")).unwrap();
        fs::write(dir.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(dir.join("src/lib.rs"), "pub mod utils;").unwrap();
        fs::write(dir.join("src/utils/helpers.rs"), "pub fn help() {}").unwrap();
        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        fs::write(dir.join("README.md"), "# Test").unwrap();
    }

    #[test]
    fn test_recursive_glob() {
        let dir = TempDir::new().unwrap();
        create_test_tree(dir.path());

        let input = GlobToolInput {
            pattern: "**/*.rs".to_string(),
            path: Some(dir.path().to_string_lossy().to_string()),
            limit: 100,
        };

        let results = glob_search(&input).unwrap();
        assert_eq!(results.len(), 3); // main.rs, lib.rs, helpers.rs
    }

    #[test]
    fn test_specific_directory_glob() {
        let dir = TempDir::new().unwrap();
        create_test_tree(dir.path());

        let input = GlobToolInput {
            pattern: "src/*.rs".to_string(),
            path: Some(dir.path().to_string_lossy().to_string()),
            limit: 100,
        };

        let results = glob_search(&input).unwrap();
        assert_eq!(results.len(), 2); // main.rs, lib.rs (not helpers.rs)
    }

    #[test]
    fn test_invalid_pattern() {
        let input = GlobToolInput {
            pattern: "[invalid".to_string(),
            path: Some(".".to_string()),
            limit: 100,
        };

        let result = glob_search(&input);
        assert!(result.is_err());
    }
}
```

## Key Takeaways

- The glob tool answers "what files exist?" -- complementing the grep tool which answers "which files contain this text?"
- Use `globset` over the basic `glob` crate for brace expansion support (`{a,b}`) and compiled pattern matching performance.
- Sort results by modification time (most recent first) because the agent's task usually involves recently changed files.
- Normalize LLM-generated patterns by prepending `**/` to bare filename patterns, since LLMs often omit the recursive prefix.
- Include file size and relative timestamps in results to help the LLM make informed decisions about which files to read next.
