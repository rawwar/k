---
title: Grep Tool
description: Build a grep tool that searches file contents with regex support, file type filtering, and configurable context lines.
---

# Grep Tool

> **What you'll learn:**
> - How to implement recursive file content search using the `regex` and `walkdir` crates
> - How to add context line display (before/after matching lines) like grep's `-C` flag
> - How to filter search scope by file type, glob pattern, and directory boundaries

The grep tool is arguably the most important search tool in a coding agent's toolkit. When the LLM needs to find where a function is called, locate an error message, or understand how a module is used across a codebase, it reaches for grep. In this subchapter you will build a `GrepTool` that recursively searches file contents using regular expressions, displays context lines around matches, and filters results by file type and pattern.

## Why Grep Matters for Agents

Think about how you search a codebase. You open your editor, press `Ctrl+Shift+F`, type a pattern, and scan the results. A coding agent needs the same capability, but programmatically. Without grep, the agent has to guess which files to read -- and with large codebases containing hundreds or thousands of files, that guess is almost always wrong.

The grep tool converts a vague intent ("find where `parse_config` is called") into a precise set of file locations with surrounding context. This context is critical -- the LLM does not just need the matching line, it needs enough surrounding code to understand the call site.

## Designing the Tool Schema

Before writing code, let's design the JSON schema that the LLM will use to invoke the tool. Good schema design directly affects how well the agent uses the tool:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct GrepToolInput {
    /// The regex pattern to search for
    pub pattern: String,

    /// Directory to search in (defaults to current working directory)
    #[serde(default)]
    pub path: Option<String>,

    /// Glob pattern to filter files (e.g., "*.rs", "*.{ts,tsx}")
    #[serde(default)]
    pub include: Option<String>,

    /// Number of context lines before and after each match
    #[serde(default = "default_context")]
    pub context_lines: usize,

    /// If true, search case-insensitively
    #[serde(default)]
    pub case_insensitive: bool,
}

fn default_context() -> usize {
    2
}
```

Notice how each field has a clear purpose and sensible defaults. The `path` defaults to the current directory, `context_lines` defaults to 2, and `case_insensitive` defaults to false. This means the LLM can invoke the tool with just a `pattern` string and get useful results.

::: tip Coming from Python
In Python, you might search files using `subprocess.run(["grep", "-r", pattern, path])` or build something with `pathlib` and `re`:
```python
import re
from pathlib import Path

def grep(pattern: str, directory: str):
    regex = re.compile(pattern)
    for path in Path(directory).rglob("*"):
        if path.is_file():
            for i, line in enumerate(path.read_text().splitlines()):
                if regex.search(line):
                    print(f"{path}:{i+1}: {line}")
```
The Rust version is structurally similar but handles errors at every step, uses compiled regex for performance, and leverages `walkdir` for efficient directory traversal.
:::

## Building the Search Engine

The core of the grep tool walks a directory tree, reads each file, and tests every line against a compiled regex. Here is the complete implementation:

```rust
use regex::RegexBuilder;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug)]
pub struct GrepMatch {
    pub path: PathBuf,
    pub line_number: usize,
    pub line_content: String,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

pub fn grep_search(input: &GrepToolInput) -> Result<Vec<GrepMatch>, String> {
    let regex = RegexBuilder::new(&input.pattern)
        .case_insensitive(input.case_insensitive)
        .build()
        .map_err(|e| format!("Invalid regex pattern: {e}"))?;

    let search_path = input
        .path
        .as_deref()
        .unwrap_or(".");

    let include_glob = input.include.as_ref().map(|pattern| {
        globset::GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .and_then(|g| Ok(globset::GlobMatcher::from(g)))
    });

    let mut matches = Vec::new();

    for entry in WalkDir::new(search_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Skip directories and non-files
        if !path.is_file() {
            continue;
        }

        // Apply include filter if specified
        if let Some(ref glob_result) = include_glob {
            if let Ok(ref matcher) = glob_result {
                let file_name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                if !matcher.is_match(file_name) {
                    continue;
                }
            }
        }

        // Skip binary files by checking the first 512 bytes
        if is_binary(path) {
            continue;
        }

        // Read and search the file
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue, // Skip files we cannot read
        };

        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            if regex.is_match(line) {
                let context_before = get_context_before(&lines, i, input.context_lines);
                let context_after = get_context_after(&lines, i, input.context_lines);

                matches.push(GrepMatch {
                    path: path.to_path_buf(),
                    line_number: i + 1,
                    line_content: line.to_string(),
                    context_before,
                    context_after,
                });
            }
        }
    }

    Ok(matches)
}

fn get_context_before(lines: &[&str], index: usize, count: usize) -> Vec<String> {
    let start = index.saturating_sub(count);
    lines[start..index]
        .iter()
        .map(|l| l.to_string())
        .collect()
}

fn get_context_after(lines: &[&str], index: usize, count: usize) -> Vec<String> {
    let end = (index + 1 + count).min(lines.len());
    lines[index + 1..end]
        .iter()
        .map(|l| l.to_string())
        .collect()
}

fn is_binary(path: &Path) -> bool {
    let mut buffer = [0u8; 512];
    let Ok(mut file) = fs::File::open(path) else {
        return true; // If we can't open it, treat as binary
    };
    use std::io::Read;
    let bytes_read = match file.read(&mut buffer) {
        Ok(n) => n,
        Err(_) => return true,
    };
    // Check for null bytes, which indicate binary content
    buffer[..bytes_read].contains(&0)
}
```

Let's break down the key design decisions:

**Error handling on regex compilation.** The LLM might send an invalid regex pattern. Rather than panicking, we convert the regex error into a user-friendly message that the LLM can interpret and fix.

**Skipping binary files.** Searching binary files produces garbage output that wastes tokens in the context window. The `is_binary` function checks the first 512 bytes for null bytes -- a simple heuristic that catches executables, images, and compiled artifacts.

**Graceful file read failures.** The `continue` on `fs::read_to_string` failure means we silently skip files we cannot read (permission denied, encoding issues). This is intentional -- a search tool should return as many results as it can rather than failing on the first problematic file.

## Formatting Results for the LLM

The raw `GrepMatch` structs need to be formatted into text that the LLM can parse efficiently. The format should resemble what a developer sees when using grep or ripgrep:

```rust
pub fn format_grep_results(matches: &[GrepMatch], max_results: usize) -> String {
    if matches.is_empty() {
        return "No matches found.".to_string();
    }

    let mut output = String::new();
    let display_count = matches.len().min(max_results);
    let total = matches.len();

    for m in matches.iter().take(display_count) {
        output.push_str(&format!("{}:{}: ", m.path.display(), m.line_number));

        // Show context before
        for (offset, ctx_line) in m.context_before.iter().enumerate() {
            let ctx_num = m.line_number - m.context_before.len() + offset;
            output.push_str(&format!("  {ctx_num}: {ctx_line}\n"));
        }

        // Show the matching line with a marker
        output.push_str(&format!("> {}: {}\n", m.line_number, m.line_content));

        // Show context after
        for (offset, ctx_line) in m.context_after.iter().enumerate() {
            let ctx_num = m.line_number + 1 + offset;
            output.push_str(&format!("  {ctx_num}: {ctx_line}\n"));
        }

        output.push_str("---\n");
    }

    if total > display_count {
        output.push_str(&format!(
            "\n... and {} more matches (showing {display_count} of {total})\n",
            total - display_count
        ));
    }

    output
}
```

The `max_results` parameter is critical for context window management. A grep across a large codebase might return thousands of matches, but the LLM's context window can only hold so much. By capping results and telling the LLM how many were truncated, you let the agent decide whether to refine its search or read a specific file.

## Implementing the Tool Trait

Now let's wire the search engine into the agent's tool system. This assumes you have the `Tool` trait from Chapter 4:

```rust
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search file contents using regex patterns. Returns matching lines \
         with surrounding context. Use this to find function definitions, \
         error messages, imports, and any text pattern across the codebase."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (defaults to cwd)"
                },
                "include": {
                    "type": "string",
                    "description": "Glob to filter files, e.g. '*.rs'"
                },
                "context_lines": {
                    "type": "integer",
                    "description": "Lines of context around matches (default 2)"
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Case-insensitive search (default false)"
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<String, String> {
        let input: GrepToolInput = serde_json::from_value(input)
            .map_err(|e| format!("Invalid input: {e}"))?;

        let matches = grep_search(&input)?;
        Ok(format_grep_results(&matches, 50))
    }
}
```

::: info In the Wild
Claude Code's grep tool is built on top of ripgrep (`rg`) for maximum performance, spawning it as a subprocess rather than reimplementing search from scratch. This gives it access to ripgrep's highly optimized SIMD-accelerated search engine and `.gitignore`-aware file filtering. Our implementation builds the logic in Rust directly, which gives us more control over output formatting and result ranking, but for a production agent you might consider wrapping ripgrep as well.
:::

## Testing the Grep Tool

Always test search tools against edge cases -- empty directories, binary files, files with unusual encodings, and patterns that match nothing:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_basic_grep() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        fs::write(&file_path, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

        let input = GrepToolInput {
            pattern: "println".to_string(),
            path: Some(dir.path().to_string_lossy().to_string()),
            include: None,
            context_lines: 1,
            case_insensitive: false,
        };

        let results = grep_search(&input).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].line_number, 2);
        assert!(results[0].line_content.contains("println"));
        assert_eq!(results[0].context_before.len(), 1);
        assert_eq!(results[0].context_after.len(), 1);
    }

    #[test]
    fn test_no_matches() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("test.rs"), "fn main() {}\n").unwrap();

        let input = GrepToolInput {
            pattern: "nonexistent_pattern".to_string(),
            path: Some(dir.path().to_string_lossy().to_string()),
            include: None,
            context_lines: 0,
            case_insensitive: false,
        };

        let results = grep_search(&input).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_invalid_regex() {
        let input = GrepToolInput {
            pattern: "[invalid".to_string(),
            path: Some(".".to_string()),
            include: None,
            context_lines: 0,
            case_insensitive: false,
        };

        let result = grep_search(&input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid regex"));
    }
}
```

## Key Takeaways

- The grep tool is the agent's primary mechanism for discovering relevant code -- always compile the regex pattern before searching and return clear errors for invalid patterns.
- Context lines around matches give the LLM enough surrounding code to understand each result without needing a separate file read.
- Always skip binary files during content search to avoid wasting context window tokens on garbage output.
- Cap the number of returned results with a `max_results` parameter and tell the LLM how many results were truncated so it can decide whether to refine the search.
- The tool description in the JSON schema is as important as the implementation -- it guides the LLM to use the tool correctly.
