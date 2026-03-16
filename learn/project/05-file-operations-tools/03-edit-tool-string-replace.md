---
title: Edit Tool String Replace
description: Implement a precise string-replacement edit tool that modifies specific sections of a file without rewriting the whole thing.
---

# Edit Tool String Replace

> **What you'll learn:**
> - How to implement exact string matching and replacement that modifies only the intended section of a file
> - How to handle the case where the search string appears zero times or more than once and report it as an error
> - How to support multi-line replacements and preserve the file's original indentation and line endings

The edit tool is the most important tool in any coding agent. While the write tool creates files from scratch, the edit tool makes targeted changes to existing code -- fixing a bug on one line, adding a function parameter, renaming a variable. This is the tool the model uses 80% of the time during a coding session.

The approach we take here is exact string replacement: the model provides a string to search for and a string to replace it with. If the search string appears exactly once in the file, we replace it. If it appears zero times or more than once, we return an error. This strict matching is the same strategy Claude Code uses, and it works remarkably well in practice -- the model learns to include enough surrounding context to make matches unique.

## Why String Replacement?

You might wonder why we use string replacement instead of line-based edits (like sed) or AST-based transformations. There are good reasons:

**Line-based edits are fragile.** If you tell the model "replace line 15," the line numbers shift as soon as another edit happens. The model would need to re-read the file after every single edit to get fresh line numbers.

**AST-based edits are language-specific.** You would need a parser for every language the agent works with. String replacement is language-agnostic -- it works on Rust, Python, TOML, Markdown, anything.

**String replacement is what the model is good at.** Large language models are excellent at pattern matching on text. When the model reads a file and decides to change a function, it can easily reproduce the exact text it needs to find and the exact text it wants to replace it with.

## The EditFile Struct

Create `src/tools/edit_file.rs`:

```rust
use crate::tools::Tool;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

pub struct EditFileTool {
    pub base_dir: PathBuf,
}

impl EditFileTool {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Count how many times `needle` appears in `haystack`
    fn count_occurrences(haystack: &str, needle: &str) -> usize {
        if needle.is_empty() {
            return 0;
        }
        haystack.matches(needle).count()
    }
}
```

The `count_occurrences` helper is a core piece of the edit tool -- it lets us check whether a match is unique before performing the replacement.

## The JSON Schema

The edit tool accepts a file path, the old string to find, and the new string to replace it with:

```rust
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Make a targeted edit to a file by replacing an exact string match. The \
         old_string must appear exactly once in the file. Provide enough context \
         in old_string to make the match unique."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to edit, relative to the project root"
                },
                "old_string": {
                    "type": "string",
                    "description": "The exact string to search for in the file. Must match exactly once."
                },
                "new_string": {
                    "type": "string",
                    "description": "The string to replace old_string with"
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    fn execute(&self, input: &Value) -> Result<String, String> {
        todo!()
    }
}
```

All three parameters are required. The description for `old_string` explicitly tells the model it must match exactly once -- this instruction matters because the model reads tool descriptions as part of the system prompt and uses them to decide how much context to include.

## Implementing Execute

Here is the complete execute method. The logic is straightforward: read the file, verify the old string appears exactly once, perform the replacement, write it back.

```rust
fn execute(&self, input: &Value) -> Result<String, String> {
    // Extract parameters
    let path_str = input
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: path".to_string())?;

    let old_string = input
        .get("old_string")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: old_string".to_string())?;

    let new_string = input
        .get("new_string")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: new_string".to_string())?;

    let path = self.base_dir.join(path_str);

    // Read the current content
    let content = fs::read_to_string(&path).map_err(|e| {
        format!("Failed to read file '{}': {}", path.display(), e)
    })?;

    // Check that old_string is not empty
    if old_string.is_empty() {
        return Err("old_string cannot be empty".to_string());
    }

    // Count occurrences
    let count = Self::count_occurrences(&content, old_string);

    match count {
        0 => {
            // Build a helpful error message
            Err(format!(
                "The string was not found in '{}'. Make sure old_string matches \
                 the file content exactly, including whitespace and indentation.",
                path_str
            ))
        }
        1 => {
            // Perform the replacement
            let new_content = content.replacen(old_string, new_string, 1);

            // Write the modified content back
            fs::write(&path, &new_content).map_err(|e| {
                format!("Failed to write file '{}': {}", path.display(), e)
            })?;

            // Return a summary of the change
            let old_lines = old_string.lines().count();
            let new_lines = new_string.lines().count();
            Ok(format!(
                "Edited '{}': replaced {} line(s) with {} line(s)",
                path_str, old_lines, new_lines
            ))
        }
        n => {
            Err(format!(
                "Found {} occurrences of the search string in '{}'. The old_string \
                 must match exactly once. Include more surrounding context to make \
                 the match unique.",
                n, path_str
            ))
        }
    }
}
```

The logic has three branches, and the error messages in each are carefully crafted to help the model self-correct:

**Zero matches:** The message tells the model to check whitespace and indentation. This is the most common failure mode -- the model gets the text right but misses a tab vs. spaces difference.

**Exactly one match:** The happy path. We use `replacen(old_string, new_string, 1)` instead of `replace` to ensure we only replace the first match, even though we already verified there is exactly one. Belt and suspenders.

**Multiple matches:** The message tells the model to include more context. This nudges the model to expand its `old_string` to include surrounding lines that make the match unique.

::: tip Coming from Python
In Python, you might implement the same logic like this:
```python
def edit_file(path: str, old_string: str, new_string: str) -> str:
    content = Path(path).read_text()
    count = content.count(old_string)

    if count == 0:
        raise ValueError(f"String not found in {path}")
    elif count > 1:
        raise ValueError(f"Found {count} occurrences, need exactly 1")

    new_content = content.replace(old_string, new_string, 1)
    Path(path).write_text(new_content)
    return f"Edited {path}"
```
The Rust version is structurally identical. The main difference is `Result<String, String>` vs. exceptions -- in Rust, the caller is forced to handle both success and failure cases. The `match` on `count` also makes the three-way branching more explicit than Python's if/elif/else chain.
:::

## Multi-line Replacements

The string replacement approach works naturally with multi-line edits. The model can replace an entire function:

```json
{
    "path": "src/main.rs",
    "old_string": "fn greet(name: &str) {\n    println!(\"Hello, {}\", name);\n}",
    "new_string": "fn greet(name: &str) {\n    println!(\"Hello, {}! Welcome back.\", name);\n}"
}
```

Because we operate on the raw string content, newlines, indentation, and any other whitespace are part of the match. The model must reproduce them exactly. This is a feature, not a bug -- it forces precise edits and prevents accidental matches.

## Handling Indentation

One common gotcha: the model sometimes gets indentation wrong, especially when mixing tabs and spaces. Consider a file indented with 4 spaces:

```rust
fn process() {
    let x = 1;
    let y = 2;
}
```

If the model sends `old_string` with tabs instead of spaces, the match fails. The error message we wrote ("check whitespace and indentation") guides the model to fix this. You could add a fuzzy matching mode that normalizes whitespace, but that introduces the risk of matching the wrong section. Strict matching is safer.

If you want to help the model get indentation right, you can include a hint in the tool description:

```rust
fn description(&self) -> &str {
    "Make a targeted edit to a file by replacing an exact string match. The \
     old_string must appear exactly once in the file. Include enough context \
     to make the match unique. Whitespace and indentation must match exactly."
}
```

## Testing the Edit Tool

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn setup_file(tmp: &TempDir, name: &str, content: &str) -> PathBuf {
        let path = tmp.path().join(name);
        let mut f = fs::File::create(&path).unwrap();
        write!(f, "{}", content).unwrap();
        path
    }

    #[test]
    fn test_edit_single_line() {
        let tmp = TempDir::new().unwrap();
        setup_file(&tmp, "test.rs", "fn main() {\n    println!(\"hello\");\n}\n");

        let tool = EditFileTool::new(tmp.path().to_path_buf());
        let result = tool.execute(&json!({
            "path": "test.rs",
            "old_string": "    println!(\"hello\");",
            "new_string": "    println!(\"goodbye\");"
        }));

        assert!(result.is_ok());
        let content = fs::read_to_string(tmp.path().join("test.rs")).unwrap();
        assert!(content.contains("goodbye"));
        assert!(!content.contains("hello"));
    }

    #[test]
    fn test_edit_no_match() {
        let tmp = TempDir::new().unwrap();
        setup_file(&tmp, "test.rs", "fn main() {}\n");

        let tool = EditFileTool::new(tmp.path().to_path_buf());
        let result = tool.execute(&json!({
            "path": "test.rs",
            "old_string": "this does not exist",
            "new_string": "replacement"
        }));

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_edit_multiple_matches() {
        let tmp = TempDir::new().unwrap();
        setup_file(&tmp, "test.rs", "let x = 1;\nlet y = 1;\n");

        let tool = EditFileTool::new(tmp.path().to_path_buf());
        let result = tool.execute(&json!({
            "path": "test.rs",
            "old_string": " = 1;",
            "new_string": " = 2;"
        }));

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("2 occurrences"));

        // File should be unchanged
        let content = fs::read_to_string(tmp.path().join("test.rs")).unwrap();
        assert_eq!(content, "let x = 1;\nlet y = 1;\n");
    }

    #[test]
    fn test_edit_multiline_replacement() {
        let tmp = TempDir::new().unwrap();
        let original = "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}";
        setup_file(&tmp, "math.rs", original);

        let tool = EditFileTool::new(tmp.path().to_path_buf());
        let result = tool.execute(&json!({
            "path": "math.rs",
            "old_string": "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}",
            "new_string": "fn add(a: i32, b: i32) -> i32 {\n    // Overflow-safe addition\n    a.checked_add(b).unwrap_or(i32::MAX)\n}"
        }));

        assert!(result.is_ok());
        let content = fs::read_to_string(tmp.path().join("math.rs")).unwrap();
        assert!(content.contains("checked_add"));
        assert!(content.contains("Overflow-safe"));
    }
}
```

The multiple-matches test is especially important: it verifies that when an edit fails, the file is not modified. This is a safety property -- failed edits must be atomic. Either the edit succeeds completely or the file is untouched.

::: tip In the Wild
Claude Code's Edit tool uses this exact same "old_string must match exactly once" strategy. When the match fails, the error message guides the model to include more context. This creates a natural retry loop: the model tries an edit, gets "found 3 occurrences," expands the old_string to include the surrounding function signature, and tries again. The model learns quickly -- after a few interactions, it almost always includes enough context on the first try.
:::

## Key Takeaways

- The edit tool performs exact string replacement: find `old_string` in the file, verify it appears exactly once, replace it with `new_string`, and write back the result.
- When the match count is not exactly one, the tool returns descriptive error messages that guide the model to fix its input -- either by checking whitespace (zero matches) or including more context (multiple matches).
- Multi-line replacements work naturally because the match operates on the raw file content string, including newlines and indentation.
- Failed edits leave the file untouched -- the replacement only happens when exactly one match is found, and the file is only written after the new content is computed.
- This string replacement approach is language-agnostic, works on any text file, and aligns with how language models naturally think about code changes.
