---
title: Write Tool
description: Implement a tool that creates or overwrites files with full content, handling directory creation and encoding.
---

# Write Tool

> **What you'll learn:**
> - How to implement the WriteFile tool that writes complete content to a specified file path
> - How to automatically create parent directories when the target path does not yet exist
> - How to handle encoding, line endings, and trailing newlines consistently across platforms

Now that the agent can read files, it needs the ability to create them. The WriteFile tool takes a path and content string, and writes the content to that file -- creating any intermediate directories as needed. This is the tool the model reaches for when it needs to create a new file from scratch, such as a new module, a test file, or a configuration file.

The write tool is deliberately simple: it writes the entire content you give it. It does not do partial updates -- that is what the edit tool is for. This clean separation between "write the whole file" and "modify part of a file" makes the tools easier for the model to use correctly. When the model needs to create something new, it uses write. When it needs to change something existing, it uses edit.

## The WriteFile Struct

Create `src/tools/write_file.rs`:

```rust
use crate::tools::Tool;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

pub struct WriteFileTool {
    pub base_dir: PathBuf,
}

impl WriteFileTool {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }
}
```

Like the read tool, the write tool holds a `base_dir` that constrains where it can operate. The pattern is consistent across all file tools -- they all know their boundaries.

## The JSON Schema

The write tool accepts two required parameters: `path` and `content`.

```rust
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file at the given path. Creates the file if it does not \
         exist. Creates parent directories as needed. Overwrites the file if it \
         already exists."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to write to, relative to the project root"
                },
                "content": {
                    "type": "string",
                    "description": "The complete content to write to the file"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn execute(&self, input: &Value) -> Result<String, String> {
        todo!()
    }
}
```

Both parameters are required. Unlike the read tool where offset/limit are optional, a write without content is meaningless -- if the model wants to create an empty file, it passes an empty string.

## Implementing Execute

The execute method needs to handle several concerns: path resolution, parent directory creation, writing the content, and returning a useful confirmation message.

```rust
fn execute(&self, input: &Value) -> Result<String, String> {
    // Extract required parameters
    let path_str = input
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: path".to_string())?;

    let content = input
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: content".to_string())?;

    // Resolve the full path
    let path = self.base_dir.join(path_str);

    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create directory '{}': {}",
                parent.display(),
                e
            )
        })?;
    }

    // Check if file already exists (for the status message)
    let existed = path.exists();

    // Write the content
    fs::write(&path, content).map_err(|e| {
        format!("Failed to write file '{}': {}", path.display(), e)
    })?;

    // Return a confirmation with useful metadata
    let line_count = content.lines().count();
    let byte_count = content.len();
    let status = if existed { "Updated" } else { "Created" };

    Ok(format!(
        "{} {} ({} lines, {} bytes)",
        status,
        path.display(),
        line_count,
        byte_count
    ))
}
```

Let's examine the important decisions.

**`create_dir_all` handles nested directories.** When the model asks to write `src/tools/search/mod.rs`, you need `src/`, `src/tools/`, and `src/tools/search/` to exist. The `create_dir_all` function creates the entire directory chain in one call -- it is the equivalent of `mkdir -p`. If the directories already exist, it succeeds silently.

**The return message includes metadata.** Rather than just saying "done," the tool returns the action taken ("Created" or "Updated"), the full path, the line count, and the byte count. This gives the model useful feedback without requiring a follow-up read call. If it wrote 150 lines and expects 150 lines, it knows the write worked.

**`fs::write` replaces the entire file.** This function creates the file if it does not exist, or truncates and overwrites it if it does. There is no append mode -- the write tool always replaces the full content. This makes the tool's behavior predictable: the file will contain exactly what you passed as `content`, nothing more.

::: python Coming from Python
In Python you would write a file like this:
```python
from pathlib import Path

def write_file(path: str, content: str) -> str:
    p = Path(path)
    p.parent.mkdir(parents=True, exist_ok=True)
    existed = p.exists()
    p.write_text(content)
    status = "Updated" if existed else "Created"
    return f"{status} {path} ({len(content.splitlines())} lines, {len(content)} bytes)"
```
The Rust version is almost identical in structure. `fs::create_dir_all` maps to `Path.mkdir(parents=True, exist_ok=True)`, and `fs::write` maps to `Path.write_text()`. The key difference is that every I/O operation in Rust returns a `Result` you must explicitly handle -- there are no uncaught `OSError` exceptions.
:::

## Handling Line Endings

A subtle issue with writing files is line ending consistency. On Windows, lines end with `\r\n` (carriage return + newline), while on macOS and Linux they end with `\n`. The model typically generates content with `\n` endings, which is what you want for source code.

However, you should be aware of what `fs::write` does: it writes bytes exactly as given. If the content string contains `\n`, that is what ends up on disk -- regardless of platform. This is actually the correct behavior for a coding agent, because source code should have consistent `\n` line endings everywhere.

```rust
/// Normalize line endings to \n (Unix-style)
fn normalize_line_endings(content: &str) -> String {
    content.replace("\r\n", "\n")
}
```

You can add this normalization step before writing if you want to guarantee consistency:

```rust
// In the execute method, before writing:
let content = normalize_line_endings(content);
fs::write(&path, content.as_bytes()).map_err(|e| {
    format!("Failed to write file '{}': {}", path.display(), e)
})?;
```

## Ensuring a Trailing Newline

Most coding conventions expect files to end with a newline character. Many linters and diff tools complain about missing trailing newlines. You can enforce this in the write tool:

```rust
fn ensure_trailing_newline(content: &str) -> String {
    if content.is_empty() || content.ends_with('\n') {
        content.to_string()
    } else {
        format!("{}\n", content)
    }
}
```

Whether you enforce this is a design decision. Claude Code does not add trailing newlines automatically -- it trusts the model to include them when appropriate. For our agent, let's add it as a safety net since it rarely hurts and often helps.

## Testing the Write Tool

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_write_new_file() {
        let tmp = TempDir::new().unwrap();
        let tool = WriteFileTool::new(tmp.path().to_path_buf());

        let result = tool.execute(&json!({
            "path": "hello.txt",
            "content": "Hello, world!\n"
        }));

        assert!(result.is_ok());
        let msg = result.unwrap();
        assert!(msg.contains("Created"));
        assert!(msg.contains("1 lines"));

        // Verify the file was actually written
        let content = fs::read_to_string(tmp.path().join("hello.txt")).unwrap();
        assert_eq!(content, "Hello, world!\n");
    }

    #[test]
    fn test_write_creates_parent_directories() {
        let tmp = TempDir::new().unwrap();
        let tool = WriteFileTool::new(tmp.path().to_path_buf());

        let result = tool.execute(&json!({
            "path": "src/tools/search/mod.rs",
            "content": "pub mod search;\n"
        }));

        assert!(result.is_ok());
        assert!(tmp.path().join("src/tools/search/mod.rs").exists());
    }

    #[test]
    fn test_write_overwrites_existing_file() {
        let tmp = TempDir::new().unwrap();
        let tool = WriteFileTool::new(tmp.path().to_path_buf());

        // Write initial content
        tool.execute(&json!({
            "path": "data.txt",
            "content": "version 1"
        }))
        .unwrap();

        // Overwrite
        let result = tool.execute(&json!({
            "path": "data.txt",
            "content": "version 2"
        }));

        assert!(result.is_ok());
        let msg = result.unwrap();
        assert!(msg.contains("Updated"));

        let content = fs::read_to_string(tmp.path().join("data.txt")).unwrap();
        assert_eq!(content, "version 2");
    }

    #[test]
    fn test_write_missing_content() {
        let tmp = TempDir::new().unwrap();
        let tool = WriteFileTool::new(tmp.path().to_path_buf());

        let result = tool.execute(&json!({"path": "test.txt"}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing required parameter"));
    }
}
```

These tests cover the core behaviors: creating a new file, creating parent directories, overwriting an existing file, and handling missing parameters. Notice the pattern: each test creates its own `TempDir`, performs operations through the tool's JSON interface, and verifies both the return message and the actual file system state.

::: wild In the Wild
Claude Code's Write tool requires the model to provide the complete file content -- there is no "append" mode. This is a deliberate design choice: it keeps the tool simple and predictable. If the model needs to add content to a file, it reads the current content first, then writes the combined result. OpenCode follows the same pattern. The alternative -- supporting append, prepend, and insert-at-line modes -- adds complexity that makes the tool harder for the model to use correctly.
:::

## Registering the Write Tool

Update `src/tools/mod.rs` to include the write tool:

```rust
pub mod read_file;
pub mod write_file;

use read_file::ReadFileTool;
use write_file::WriteFileTool;
use std::path::PathBuf;

pub fn create_tools(base_dir: PathBuf) -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(ReadFileTool::new(base_dir.clone())),
        Box::new(WriteFileTool::new(base_dir.clone())),
    ]
}
```

With read and write in place, the agent can now create files from scratch. But most real-world coding tasks are not about creating new files -- they are about modifying existing ones. That is where the edit tool comes in, which you will build next.

## Key Takeaways

- The WriteFile tool takes a path and content string, creates parent directories with `fs::create_dir_all`, and writes the complete content with `fs::write`.
- The return message includes whether the file was created or updated, plus line count and byte count, giving the model immediate feedback without a follow-up read.
- Line ending normalization (`\r\n` to `\n`) ensures consistent source code formatting across platforms.
- The write tool always replaces the entire file content -- there is no append mode, which keeps the tool simple and its behavior predictable for the model.
- Always verify both the tool's return value and the actual file system state in tests, because a tool can return "success" while silently writing the wrong content.
