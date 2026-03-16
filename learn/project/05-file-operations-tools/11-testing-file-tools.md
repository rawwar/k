---
title: Testing File Tools
description: Write comprehensive tests for all file operations using temporary directories, fixtures, and edge-case inputs.
---

# Testing File Tools

> **What you'll learn:**
> - How to use `tempdir` to create isolated test environments that are automatically cleaned up after each test
> - How to write test cases that cover edge cases like empty files, binary files, Unicode content, and deeply nested paths
> - How to test the full tool lifecycle from JSON input parsing through file modification to observation output

File tools interact with the real filesystem, which makes testing them more complex than testing pure functions. Each test needs an isolated environment so tests do not interfere with each other, and you need to verify not just the tool's return value but the actual state of files on disk. This subchapter shows you how to build a thorough test suite that catches bugs before they reach production.

## Setting Up Test Infrastructure

Every file tool test follows the same pattern: create a temporary directory, set up initial files, run the tool, verify both the return value and the filesystem state. Let's build helpers that make this pattern concise.

First, make sure `tempfile` is in your dev-dependencies:

```toml
[dev-dependencies]
tempfile = "3"
serde_json = "1"  # For json! macro in tests
```

Now create a test helper module. You can place this in `src/tools/test_helpers.rs` or at the top of your test modules:

```rust
#[cfg(test)]
pub mod test_helpers {
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    /// A test fixture that creates a temporary directory
    /// with a predefined file structure.
    pub struct TestProject {
        pub dir: TempDir,
    }

    impl TestProject {
        /// Create a bare test project (empty directory).
        pub fn new() -> Self {
            Self {
                dir: TempDir::new().unwrap(),
            }
        }

        /// Create a test project with a typical Rust project structure.
        pub fn with_rust_project() -> Self {
            let project = Self::new();
            let base = project.path();

            // Create directory structure
            fs::create_dir_all(base.join("src/tools")).unwrap();
            fs::create_dir_all(base.join("tests")).unwrap();

            // Create files
            project.write_file("Cargo.toml", r#"[package]
name = "test-agent"
version = "0.1.0"
edition = "2021"
"#);
            project.write_file("src/main.rs", r#"mod tools;

fn main() {
    println!("Hello, agent!");
}
"#);
            project.write_file("src/tools/mod.rs", r#"pub mod read;
pub mod write;
"#);
            project.write_file("src/tools/read.rs", r#"pub fn read_file(path: &str) -> String {
    std::fs::read_to_string(path).unwrap()
}
"#);
            project.write_file("tests/basic.rs", r#"#[test]
fn it_works() {
    assert_eq!(2 + 2, 4);
}
"#);

            project
        }

        pub fn path(&self) -> &Path {
            self.dir.path()
        }

        pub fn write_file(&self, relative_path: &str, content: &str) {
            let full_path = self.dir.path().join(relative_path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(full_path, content).unwrap();
        }

        pub fn read_file(&self, relative_path: &str) -> String {
            fs::read_to_string(self.dir.path().join(relative_path)).unwrap()
        }

        pub fn file_exists(&self, relative_path: &str) -> bool {
            self.dir.path().join(relative_path).exists()
        }
    }
}
```

The `TestProject` struct encapsulates everything a file tool test needs: a temporary directory, helper methods for file setup and verification, and automatic cleanup when the test finishes.

::: python Coming from Python
In Python, you would use `pytest`'s `tmp_path` fixture:
```python
def test_read_file(tmp_path):
    (tmp_path / "hello.txt").write_text("Hello, world!")
    tool = ReadFileTool(base_dir=tmp_path)
    result = tool.execute({"path": "hello.txt"})
    assert "Hello, world!" in result
```
Rust's `TempDir` works the same way -- it creates a temporary directory and deletes it when dropped. The difference is that Rust does not have a framework-level fixture system like pytest. Instead, you create the `TempDir` in each test or use a helper function. The `TestProject` struct we built serves the same purpose as a pytest fixture class.
:::

## Testing the Read Tool

Let's write a comprehensive test suite for the read tool using the `TestProject` helper:

```rust
#[cfg(test)]
mod read_tool_tests {
    use super::*;
    use crate::tools::test_helpers::TestProject;
    use serde_json::json;

    fn make_tool(project: &TestProject) -> ReadFileTool {
        ReadFileTool::new(project.path().to_path_buf())
    }

    #[test]
    fn test_read_simple_file() {
        let project = TestProject::with_rust_project();
        let tool = make_tool(&project);

        let result = tool.execute(&json!({"path": "src/main.rs"}));
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.contains("fn main()"));
        assert!(output.contains("   1\t")); // Line numbers present
    }

    #[test]
    fn test_read_empty_file() {
        let project = TestProject::new();
        project.write_file("empty.txt", "");
        let tool = make_tool(&project);

        let result = tool.execute(&json!({"path": "empty.txt"}));
        assert!(result.is_ok());
        // Empty file should return empty string (no lines to number)
        assert!(result.unwrap().is_empty() || result.unwrap().trim().is_empty());
    }

    #[test]
    fn test_read_unicode_content() {
        let project = TestProject::new();
        project.write_file("unicode.txt", "Hello, world!\nHola, mundo!\n");
        let tool = make_tool(&project);

        let result = tool.execute(&json!({"path": "unicode.txt"}));
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Hola, mundo!"));
    }

    #[test]
    fn test_read_file_with_special_characters() {
        let project = TestProject::new();
        project.write_file(
            "special.rs",
            "fn main() {\n    let s = \"tabs\\there\";\n    let n = 42; // backslash: \\\\\n}\n",
        );
        let tool = make_tool(&project);

        let result = tool.execute(&json!({"path": "special.rs"}));
        assert!(result.is_ok());
    }

    #[test]
    fn test_read_nonexistent_file() {
        let project = TestProject::new();
        let tool = make_tool(&project);

        let result = tool.execute(&json!({"path": "nope.txt"}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to read"));
    }

    #[test]
    fn test_read_with_offset_and_limit() {
        let project = TestProject::new();
        let mut content = String::new();
        for i in 1..=100 {
            content.push_str(&format!("Line {}\n", i));
        }
        project.write_file("hundred.txt", &content);
        let tool = make_tool(&project);

        let result = tool.execute(&json!({
            "path": "hundred.txt",
            "offset": 50,
            "limit": 10
        }));

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Line 50"));
        assert!(output.contains("Line 59"));
        assert!(!output.contains("Line 49"));
        assert!(!output.contains("Line 60"));
    }

    #[test]
    fn test_read_missing_path_parameter() {
        let project = TestProject::new();
        let tool = make_tool(&project);

        let result = tool.execute(&json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing required parameter"));
    }

    #[test]
    fn test_read_deeply_nested_file() {
        let project = TestProject::new();
        project.write_file("a/b/c/d/e/deep.txt", "deep content");
        let tool = make_tool(&project);

        let result = tool.execute(&json!({"path": "a/b/c/d/e/deep.txt"}));
        assert!(result.is_ok());
        assert!(result.unwrap().contains("deep content"));
    }
}
```

## Testing the Edit Tool

The edit tool has more failure modes than read or write, so it needs a more thorough test suite:

```rust
#[cfg(test)]
mod edit_tool_tests {
    use super::*;
    use crate::tools::test_helpers::TestProject;
    use serde_json::json;

    fn make_tool(project: &TestProject) -> EditFileTool {
        EditFileTool::new(project.path().to_path_buf())
    }

    #[test]
    fn test_edit_simple_replacement() {
        let project = TestProject::new();
        project.write_file("greet.rs", "fn greet() {\n    println!(\"hello\");\n}\n");
        let tool = make_tool(&project);

        let result = tool.execute(&json!({
            "path": "greet.rs",
            "old_string": "println!(\"hello\")",
            "new_string": "println!(\"goodbye\")"
        }));

        assert!(result.is_ok());
        let content = project.read_file("greet.rs");
        assert!(content.contains("goodbye"));
        assert!(!content.contains("hello"));
    }

    #[test]
    fn test_edit_preserves_surrounding_content() {
        let project = TestProject::new();
        project.write_file("math.rs", "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\nfn sub(a: i32, b: i32) -> i32 {\n    a - b\n}\n");
        let tool = make_tool(&project);

        let result = tool.execute(&json!({
            "path": "math.rs",
            "old_string": "    a + b",
            "new_string": "    a.wrapping_add(b)"
        }));

        assert!(result.is_ok());
        let content = project.read_file("math.rs");
        assert!(content.contains("wrapping_add"));
        assert!(content.contains("fn sub")); // Other function untouched
        assert!(content.contains("a - b")); // Sub body untouched
    }

    #[test]
    fn test_edit_no_match_leaves_file_unchanged() {
        let project = TestProject::new();
        let original = "fn main() {}\n";
        project.write_file("stable.rs", original);
        let tool = make_tool(&project);

        let result = tool.execute(&json!({
            "path": "stable.rs",
            "old_string": "this does not exist",
            "new_string": "replacement"
        }));

        assert!(result.is_err());
        // File must be completely unchanged
        assert_eq!(project.read_file("stable.rs"), original);
    }

    #[test]
    fn test_edit_multiple_matches_leaves_file_unchanged() {
        let project = TestProject::new();
        let original = "let x = 1;\nlet y = 1;\n";
        project.write_file("dupes.rs", original);
        let tool = make_tool(&project);

        let result = tool.execute(&json!({
            "path": "dupes.rs",
            "old_string": " = 1;",
            "new_string": " = 2;"
        }));

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("2 occurrences"));
        assert_eq!(project.read_file("dupes.rs"), original);
    }

    #[test]
    fn test_edit_multiline_replacement() {
        let project = TestProject::new();
        project.write_file("func.rs", "fn process() {\n    step_one();\n    step_two();\n}\n");
        let tool = make_tool(&project);

        let result = tool.execute(&json!({
            "path": "func.rs",
            "old_string": "    step_one();\n    step_two();",
            "new_string": "    step_one();\n    step_middle();\n    step_two();"
        }));

        assert!(result.is_ok());
        let content = project.read_file("func.rs");
        assert!(content.contains("step_middle"));
    }

    #[test]
    fn test_edit_empty_old_string() {
        let project = TestProject::new();
        project.write_file("test.rs", "content\n");
        let tool = make_tool(&project);

        let result = tool.execute(&json!({
            "path": "test.rs",
            "old_string": "",
            "new_string": "something"
        }));

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be empty"));
    }

    #[test]
    fn test_edit_with_indentation_mismatch() {
        let project = TestProject::new();
        // File uses 4 spaces for indentation
        project.write_file("indent.rs", "fn main() {\n    let x = 1;\n}\n");
        let tool = make_tool(&project);

        // Try to match with 2 spaces -- should fail
        let result = tool.execute(&json!({
            "path": "indent.rs",
            "old_string": "  let x = 1;",
            "new_string": "  let x = 2;"
        }));

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_edit_delete_content() {
        let project = TestProject::new();
        project.write_file("cleanup.rs", "fn main() {\n    // TODO: remove this\n    let x = 1;\n}\n");
        let tool = make_tool(&project);

        // Replace the comment with nothing
        let result = tool.execute(&json!({
            "path": "cleanup.rs",
            "old_string": "    // TODO: remove this\n",
            "new_string": ""
        }));

        assert!(result.is_ok());
        let content = project.read_file("cleanup.rs");
        assert!(!content.contains("TODO"));
        assert!(content.contains("let x = 1"));
    }
}
```

## Testing the Write Tool

```rust
#[cfg(test)]
mod write_tool_tests {
    use super::*;
    use crate::tools::test_helpers::TestProject;
    use serde_json::json;

    fn make_tool(project: &TestProject) -> WriteFileTool {
        WriteFileTool::new(project.path().to_path_buf())
    }

    #[test]
    fn test_write_new_file() {
        let project = TestProject::new();
        let tool = make_tool(&project);

        let result = tool.execute(&json!({
            "path": "new.rs",
            "content": "fn main() {}\n"
        }));

        assert!(result.is_ok());
        assert!(result.unwrap().contains("Created"));
        assert_eq!(project.read_file("new.rs"), "fn main() {}\n");
    }

    #[test]
    fn test_write_creates_nested_directories() {
        let project = TestProject::new();
        let tool = make_tool(&project);

        let result = tool.execute(&json!({
            "path": "src/tools/search/mod.rs",
            "content": "pub mod search;\n"
        }));

        assert!(result.is_ok());
        assert!(project.file_exists("src/tools/search/mod.rs"));
    }

    #[test]
    fn test_write_overwrites_existing() {
        let project = TestProject::new();
        project.write_file("config.toml", "version = 1\n");
        let tool = make_tool(&project);

        let result = tool.execute(&json!({
            "path": "config.toml",
            "content": "version = 2\n"
        }));

        assert!(result.is_ok());
        assert!(result.unwrap().contains("Updated"));
        assert_eq!(project.read_file("config.toml"), "version = 2\n");
    }

    #[test]
    fn test_write_empty_content() {
        let project = TestProject::new();
        let tool = make_tool(&project);

        let result = tool.execute(&json!({
            "path": "empty.txt",
            "content": ""
        }));

        assert!(result.is_ok());
        assert_eq!(project.read_file("empty.txt"), "");
    }

    #[test]
    fn test_write_large_content() {
        let project = TestProject::new();
        let tool = make_tool(&project);

        let content: String = (0..1000)
            .map(|i| format!("line {}\n", i))
            .collect();

        let result = tool.execute(&json!({
            "path": "large.txt",
            "content": content
        }));

        assert!(result.is_ok());
        assert!(result.unwrap().contains("1000 lines"));
    }
}
```

## Testing Edge Cases

Some of the most important tests cover edge cases that are easy to overlook:

```rust
#[cfg(test)]
mod edge_case_tests {
    use super::*;
    use crate::tools::test_helpers::TestProject;
    use serde_json::json;

    #[test]
    fn test_file_with_no_trailing_newline() {
        let project = TestProject::new();
        project.write_file("no_newline.txt", "last line has no newline");
        let tool = ReadFileTool::new(project.path().to_path_buf());

        let result = tool.execute(&json!({"path": "no_newline.txt"}));
        assert!(result.is_ok());
        assert!(result.unwrap().contains("last line has no newline"));
    }

    #[test]
    fn test_file_with_only_newlines() {
        let project = TestProject::new();
        project.write_file("newlines.txt", "\n\n\n");
        let tool = ReadFileTool::new(project.path().to_path_buf());

        let result = tool.execute(&json!({"path": "newlines.txt"}));
        assert!(result.is_ok());
    }

    #[test]
    fn test_file_with_very_long_lines() {
        let project = TestProject::new();
        let long_line = "x".repeat(10_000);
        project.write_file("long.txt", &format!("{}\n", long_line));
        let tool = ReadFileTool::new(project.path().to_path_buf());

        let result = tool.execute(&json!({"path": "long.txt"}));
        assert!(result.is_ok());
    }

    #[test]
    fn test_edit_preserves_windows_line_endings() {
        let project = TestProject::new();
        project.write_file("windows.txt", "line1\r\nline2\r\nline3\r\n");
        let tool = EditFileTool::new(project.path().to_path_buf());

        let result = tool.execute(&json!({
            "path": "windows.txt",
            "old_string": "line2",
            "new_string": "modified"
        }));

        assert!(result.is_ok());
        let content = project.read_file("windows.txt");
        assert!(content.contains("modified"));
        // The \r\n around the edit should be preserved
        assert!(content.contains("\r\n"));
    }

    #[test]
    fn test_write_and_read_roundtrip() {
        let project = TestProject::new();
        let write_tool = WriteFileTool::new(project.path().to_path_buf());
        let read_tool = ReadFileTool::new(project.path().to_path_buf());

        let original_content = "fn hello() {\n    println!(\"world\");\n}\n";

        // Write
        write_tool.execute(&json!({
            "path": "roundtrip.rs",
            "content": original_content
        })).unwrap();

        // Read back
        let read_result = read_tool.execute(&json!({"path": "roundtrip.rs"})).unwrap();

        // The content should be there (with line numbers added)
        assert!(read_result.contains("fn hello()"));
        assert!(read_result.contains("println!(\"world\")"));
    }

    #[test]
    fn test_edit_then_read_shows_changes() {
        let project = TestProject::with_rust_project();
        let edit_tool = EditFileTool::new(project.path().to_path_buf());
        let read_tool = ReadFileTool::new(project.path().to_path_buf());

        // Edit the main function
        edit_tool.execute(&json!({
            "path": "src/main.rs",
            "old_string": "println!(\"Hello, agent!\")",
            "new_string": "println!(\"Modified by agent!\")"
        })).unwrap();

        // Read and verify
        let content = read_tool.execute(&json!({"path": "src/main.rs"})).unwrap();
        assert!(content.contains("Modified by agent!"));
        assert!(!content.contains("Hello, agent!"));
    }
}
```

The roundtrip and edit-then-read tests are especially valuable. They verify that the tools work together as a system, not just individually. This is how the model uses them -- read, edit, read again to verify.

## Organizing Your Test Suite

As your test suite grows, organize it by tool and concern:

```
src/tools/
    mod.rs
    read_file.rs          # ReadFileTool + unit tests
    write_file.rs         # WriteFileTool + unit tests
    edit_file.rs          # EditFileTool + unit tests
    paths.rs              # Path resolution + unit tests
    safety.rs             # Safety checker + unit tests
    test_helpers.rs       # TestProject and shared helpers

tests/
    file_tools_integration.rs  # Cross-tool tests
```

Unit tests go in the same file as the code they test (inside `#[cfg(test)]` blocks). Integration tests that span multiple tools go in the `tests/` directory.

::: wild In the Wild
Claude Code tests its file tools with extensive edge-case coverage, including files with mixed line endings, Unicode content in various encodings, deeply nested directory structures, and symlinks. The test suite includes both unit tests for individual operations and integration tests that simulate multi-step agent workflows (read, edit, verify). This two-level testing strategy catches both low-level bugs (wrong byte offset calculation) and high-level bugs (edit tool reports success but read tool shows no change).
:::

## Key Takeaways

- Use `tempfile::TempDir` to create isolated test environments that are automatically cleaned up, ensuring tests never interfere with each other or leave artifacts on disk.
- Build a `TestProject` helper that encapsulates directory creation, file setup, and verification methods -- this reduces boilerplate and makes tests readable.
- Test both the tool's return value and the actual filesystem state: a tool might return "success" while writing the wrong content, or return an error while still modifying the file.
- Edge cases that catch real bugs: empty files, missing trailing newlines, very long lines, Windows line endings, Unicode content, and deeply nested paths.
- Integration tests that chain multiple tools (write then read, edit then verify) catch system-level bugs that unit tests miss, and they mirror how the model actually uses the tools.
