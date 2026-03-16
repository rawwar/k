---
title: Read Tool
description: Implement a tool that reads file contents from disk and returns them with line numbers for precise reference.
---

# Read Tool

> **What you'll learn:**
> - How to implement the ReadFile tool struct with the Tool trait, including its JSON schema and execute method
> - How to add line numbers to file output so the model can reference specific lines in subsequent edits
> - How to support optional offset and limit parameters for reading specific ranges of large files

The very first thing any coding agent needs is the ability to see what it is working with. Before you can write code, fix bugs, or refactor anything, you need to read the existing files. The ReadFile tool is the foundation every other file operation builds on -- the edit tool needs to read a file before modifying it, and the model needs to inspect the result after a write to verify it worked.

In this subchapter you will implement a complete ReadFile tool that reads a file from disk, adds line numbers to the output, and optionally supports reading specific line ranges. The tool plugs into the tool system you built in Chapter 4, so by the end of this section, your agent can ask to read any file and get back numbered lines it can reference in future tool calls.

## The ReadFile Tool Struct

Let's start with the struct and its Tool trait implementation. Create a new file at `src/tools/read_file.rs`:

```rust
use crate::tools::Tool;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

pub struct ReadFileTool {
    /// The base directory the agent is allowed to read from
    pub base_dir: PathBuf,
}

impl ReadFileTool {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }
}
```

The struct holds a `base_dir` field -- the root directory the agent operates in. We will use this later for safety checks. For now, it establishes the pattern that every file tool knows its boundaries.

## Defining the JSON Schema

The model needs to know what arguments the ReadFile tool accepts. You define this as a JSON schema that gets sent as part of the tool definitions in the API request. The schema tells the model: "give me a `path` string, and optionally an `offset` and `limit` for line ranges."

```rust
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file at the given path. Returns the file contents \
         with line numbers. Optionally specify offset and limit to read a specific \
         range of lines."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to read, relative to the project root"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (1-based). Defaults to 1."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read. Defaults to reading the entire file."
                }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, input: &Value) -> Result<String, String> {
        // We'll implement this next
        todo!()
    }
}
```

Notice that `offset` and `limit` are not in the `required` array. The model can call `read_file` with just a path, and it gets the whole file. When dealing with a large file, it can request specific ranges -- we will explore that pattern more in the handling-large-files subchapter.

## Implementing Execute

The `execute` method is where the real work happens. It needs to:

1. Extract the `path` from the JSON input
2. Resolve it against the base directory
3. Read the file contents
4. Add line numbers
5. Apply offset/limit if provided
6. Return the numbered content as a string

```rust
fn execute(&self, input: &Value) -> Result<String, String> {
    // Extract the path argument
    let path_str = input
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: path".to_string())?;

    // Resolve against base directory
    let path = self.base_dir.join(path_str);

    // Read the file
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read file '{}': {}", path.display(), e))?;

    // Parse optional offset and limit
    let offset = input
        .get("offset")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(1);

    let limit = input
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    // Add line numbers and apply range
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    // offset is 1-based, convert to 0-based index
    let start = (offset.saturating_sub(1)).min(total_lines);
    let end = match limit {
        Some(lim) => (start + lim).min(total_lines),
        None => total_lines,
    };

    let numbered_lines: Vec<String> = lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let line_num = start + i + 1;
            format!("{:>4}\t{}", line_num, line)
        })
        .collect();

    let mut result = numbered_lines.join("\n");

    // Add metadata header when reading a range
    if offset > 1 || limit.is_some() {
        let header = format!(
            "[Showing lines {}-{} of {} total]\n",
            start + 1,
            end,
            total_lines
        );
        result = header + &result;
    }

    Ok(result)
}
```

Let's walk through the key decisions here.

**Line numbers use `{:>4}\t` formatting.** The line number is right-aligned in a 4-character field, followed by a tab. This produces output like:

```
   1	fn main() {
   2	    println!("Hello, world!");
   3	}
```

This format is easy for the model to parse when it needs to reference specific lines. The tab character provides a clean visual separator. The 4-character width handles files up to 9,999 lines without misalignment.

**Offset is 1-based.** When the model says "start at line 10", it means the 10th line, not the 11th. This matches how humans think about line numbers, and it is what the model will see in the numbered output it got from a previous read.

**Bounds checking uses `min` and `saturating_sub`.** Rather than returning an error for out-of-range offsets, we clamp to valid ranges. If the model asks to start at line 1000 in a 50-line file, it gets an empty result with the metadata header showing "0 of 50 lines." This is more useful than an error -- the model learns the file is shorter than expected.

::: tip Coming from Python
In Python, you would read a file and add line numbers like this:
```python
def read_file(path: str, offset: int = 1, limit: int | None = None) -> str:
    with open(path, 'r') as f:
        lines = f.readlines()

    start = offset - 1
    end = start + limit if limit else len(lines)
    numbered = [f"{i+start+1:>4}\t{line.rstrip()}" for i, line in enumerate(lines[start:end])]
    return "\n".join(numbered)
```
The Rust version does the same thing but with explicit error handling through `Result`. Where Python would raise a `FileNotFoundError` or `PermissionError` as exceptions, Rust returns `Err` values that you must handle -- there is no way to accidentally ignore a file-not-found error.
:::

## Registering the Tool

With the struct complete, register it in your tool registry. In `src/tools/mod.rs`:

```rust
pub mod read_file;

use read_file::ReadFileTool;
use std::path::PathBuf;

pub fn create_tools(base_dir: PathBuf) -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(ReadFileTool::new(base_dir.clone())),
        // More tools will be added here in subsequent subchapters
    ]
}
```

## Testing the Read Tool

Let's write a quick test to verify the tool works end to end. This test creates a temporary file, reads it through the tool, and checks the output format:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_read_file_with_line_numbers() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("test.rs");
        let mut f = fs::File::create(&file_path).unwrap();
        writeln!(f, "fn main() {{").unwrap();
        writeln!(f, "    println!(\"hello\");").unwrap();
        writeln!(f, "}}").unwrap();

        let tool = ReadFileTool::new(tmp.path().to_path_buf());
        let result = tool.execute(&json!({"path": "test.rs"}));

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("   1\tfn main() {"));
        assert!(output.contains("   2\t    println!(\"hello\");"));
        assert!(output.contains("   3\t}"));
    }

    #[test]
    fn test_read_file_with_offset_and_limit() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("lines.txt");
        let mut f = fs::File::create(&file_path).unwrap();
        for i in 1..=20 {
            writeln!(f, "Line {}", i).unwrap();
        }

        let tool = ReadFileTool::new(tmp.path().to_path_buf());
        let result = tool.execute(&json!({
            "path": "lines.txt",
            "offset": 5,
            "limit": 3
        }));

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("[Showing lines 5-7 of 20 total]"));
        assert!(output.contains("   5\tLine 5"));
        assert!(output.contains("   6\tLine 6"));
        assert!(output.contains("   7\tLine 7"));
        assert!(!output.contains("Line 8"));
    }

    #[test]
    fn test_read_nonexistent_file() {
        let tmp = TempDir::new().unwrap();
        let tool = ReadFileTool::new(tmp.path().to_path_buf());
        let result = tool.execute(&json!({"path": "nope.txt"}));

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to read file"));
    }
}
```

These tests demonstrate the pattern you will use throughout this chapter: create a `TempDir`, set up files, call the tool with JSON input, and verify the output. The `tempfile` crate ensures each test gets its own clean directory that is automatically deleted when the `TempDir` value is dropped.

::: tip In the Wild
Claude Code's Read tool returns file contents with line numbers in a `{line_number}\t{content}` format -- the same pattern we use here. This is deliberate: the line numbers serve as coordinates that the model references when making subsequent edits. When the model says "replace lines 15-20", it is using the numbers it saw in the read output. OpenCode takes a similar approach but uses a slightly different format with a pipe separator.
:::

## Why Line Numbers Matter

Adding line numbers is not just a cosmetic choice. They form the coordinate system the model uses to navigate files. Without line numbers, the model would need to include large chunks of surrounding context to identify where an edit should happen. With line numbers, the model can say "the function starting at line 42" unambiguously.

This is also why the offset/limit parameters exist. When the model reads a 2,000-line file and needs to focus on a specific function, it can request `offset: 150, limit: 30` to get just the relevant section. The numbered output tells it exactly where it is in the file, so it can request the next range if needed.

## Key Takeaways

- The ReadFile tool reads a file, adds line numbers in `{:>4}\t{content}` format, and returns the result as a string that the model can parse and reference in future tool calls.
- Optional `offset` and `limit` parameters enable reading specific line ranges, which becomes essential when working with files too large for the model's context window.
- Error handling uses `Result<String, String>` -- file-not-found and permission errors become `Err` values that the tool system converts into error observations for the model.
- Line numbers serve as a coordinate system that connects the read tool to the edit tool -- the model reads numbered lines, then references those numbers when making changes.
- The `tempfile` crate provides isolated test directories that auto-clean, making file tool tests reliable and repeatable.
