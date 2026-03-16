---
title: Unit Testing Tools
description: Write thorough unit tests for agent tools, covering input validation, execution logic, output formatting, and error handling.
---

# Unit Testing Tools

> **What you'll learn:**
> - How to structure tool unit tests that verify input schema validation, execution behavior, and output formatting independently
> - Techniques for testing tools that interact with the filesystem, network, or shell by using temp directories, mock servers, and command stubs
> - How to test tool error paths including invalid inputs, permission failures, timeouts, and partial results

Tools are the most testable part of your coding agent. Each tool takes a well-defined input, performs a deterministic operation, and returns a structured output. There is no LLM in the loop. This makes tools ideal candidates for thorough unit testing, and they deserve the most attention in your test suite.

Let's build out the testing patterns for each category of tool your agent provides.

## Anatomy of a Tool Test

Every tool test follows a three-phase pattern: arrange the environment, execute the tool, and assert on the result. In Rust, you write these as functions annotated with `#[test]` inside a `#[cfg(test)]` module at the bottom of the tool's source file.

Here is the structure for a `ReadFile` tool:

```rust
use std::fs;
use tempfile::tempdir;

pub struct ReadFileTool;

impl ReadFileTool {
    pub fn execute(&self, path: &str) -> Result<String, ToolError> {
        let content = fs::read_to_string(path)
            .map_err(|e| ToolError::ExecutionFailed(format!("Cannot read {}: {}", path, e)))?;
        if content.len() > 100_000 {
            return Err(ToolError::OutputTooLarge {
                size: content.len(),
                limit: 100_000,
            });
        }
        Ok(content)
    }
}

#[derive(Debug, PartialEq)]
pub enum ToolError {
    ExecutionFailed(String),
    OutputTooLarge { size: usize, limit: usize },
    InvalidInput(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn reads_existing_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.rs");
        fs::write(&file, "fn main() {}").unwrap();

        let tool = ReadFileTool;
        let result = tool.execute(file.to_str().unwrap()).unwrap();
        assert_eq!(result, "fn main() {}");
    }

    #[test]
    fn returns_error_for_missing_file() {
        let tool = ReadFileTool;
        let result = tool.execute("/nonexistent/path/file.txt");
        assert!(result.is_err());
    }

    #[test]
    fn rejects_oversized_files() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("big.txt");
        let content = "x".repeat(200_000);
        fs::write(&file, &content).unwrap();

        let tool = ReadFileTool;
        let result = tool.execute(file.to_str().unwrap());
        assert!(matches!(result, Err(ToolError::OutputTooLarge { .. })));
    }
}
```

Notice that each test uses a real temporary directory. The `tempfile` crate handles cleanup automatically when the `TempDir` value is dropped, so tests do not leave files behind.

::: tip Coming from Python
In pytest, you use the `tmp_path` fixture to get a temporary directory:
```python
def test_read_file(tmp_path):
    f = tmp_path / "test.rs"
    f.write_text("fn main() {}")
    result = read_file_tool(str(f))
    assert result == "fn main() {}"
```
Rust's `tempfile::tempdir()` serves the same purpose. The key difference is that Rust's temp directory is cleaned up deterministically when the `TempDir` goes out of scope, while Python relies on garbage collection or explicit cleanup.
:::

## Testing Input Validation

Tools should validate their inputs before executing. This catches malformed arguments from the LLM early, before they cause confusing downstream errors. Test both valid and invalid inputs:

```rust
pub struct WriteFileTool;

impl WriteFileTool {
    pub fn validate_input(&self, path: &str, content: &str) -> Result<(), ToolError> {
        if path.is_empty() {
            return Err(ToolError::InvalidInput("path cannot be empty".into()));
        }
        if path.contains("..") {
            return Err(ToolError::InvalidInput(
                "path cannot contain '..' (path traversal)".into(),
            ));
        }
        if content.len() > 500_000 {
            return Err(ToolError::InvalidInput(
                "content exceeds maximum size of 500KB".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod write_tests {
    use super::*;

    #[test]
    fn rejects_empty_path() {
        let tool = WriteFileTool;
        let result = tool.validate_input("", "content");
        assert!(matches!(result, Err(ToolError::InvalidInput(_))));
    }

    #[test]
    fn rejects_path_traversal() {
        let tool = WriteFileTool;
        let result = tool.validate_input("../../etc/passwd", "evil");
        assert!(matches!(result, Err(ToolError::InvalidInput(_))));
    }

    #[test]
    fn accepts_valid_input() {
        let tool = WriteFileTool;
        let result = tool.validate_input("src/main.rs", "fn main() {}");
        assert!(result.is_ok());
    }

    #[test]
    fn rejects_oversized_content() {
        let tool = WriteFileTool;
        let big = "x".repeat(600_000);
        let result = tool.validate_input("file.txt", &big);
        assert!(matches!(result, Err(ToolError::InvalidInput(_))));
    }
}
```

## Testing Shell Execution Tools

Shell tools are trickier because they spawn real processes. The key is to execute simple, predictable commands that behave the same on every platform your tests run on:

```rust
use std::process::Command;
use std::time::Duration;

pub struct ShellTool {
    pub timeout: Duration,
    pub working_dir: String,
}

impl ShellTool {
    pub fn execute(&self, command: &str) -> Result<ShellOutput, ToolError> {
        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.working_dir)
            .output()
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(ShellOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }
}

pub struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[cfg(test)]
mod shell_tests {
    use super::*;

    fn tool_in(dir: &std::path::Path) -> ShellTool {
        ShellTool {
            timeout: Duration::from_secs(5),
            working_dir: dir.to_str().unwrap().to_string(),
        }
    }

    #[test]
    fn runs_echo_command() {
        let dir = tempfile::tempdir().unwrap();
        let tool = tool_in(dir.path());

        let result = tool.execute("echo hello").unwrap();
        assert_eq!(result.stdout.trim(), "hello");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn captures_stderr() {
        let dir = tempfile::tempdir().unwrap();
        let tool = tool_in(dir.path());

        let result = tool.execute("echo error >&2").unwrap();
        assert_eq!(result.stderr.trim(), "error");
    }

    #[test]
    fn reports_nonzero_exit_code() {
        let dir = tempfile::tempdir().unwrap();
        let tool = tool_in(dir.path());

        let result = tool.execute("exit 42").unwrap();
        assert_eq!(result.exit_code, 42);
    }

    #[test]
    fn runs_in_specified_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("marker.txt"), "found").unwrap();
        let tool = tool_in(dir.path());

        let result = tool.execute("cat marker.txt").unwrap();
        assert_eq!(result.stdout.trim(), "found");
    }
}
```

## Testing Tool Output Formatting

Your tools format their results into strings that get sent back to the LLM as tool results. Test that this formatting is correct and consistent:

```rust
pub fn format_shell_output(output: &ShellOutput) -> String {
    let mut formatted = String::new();
    if !output.stdout.is_empty() {
        formatted.push_str(&output.stdout);
    }
    if !output.stderr.is_empty() {
        if !formatted.is_empty() {
            formatted.push('\n');
        }
        formatted.push_str("[stderr]\n");
        formatted.push_str(&output.stderr);
    }
    formatted.push_str(&format!("\n[exit code: {}]", output.exit_code));
    formatted
}

#[cfg(test)]
mod format_tests {
    use super::*;

    #[test]
    fn formats_stdout_only() {
        let output = ShellOutput {
            stdout: "hello\n".into(),
            stderr: String::new(),
            exit_code: 0,
        };
        let formatted = format_shell_output(&output);
        assert!(formatted.contains("hello"));
        assert!(formatted.contains("[exit code: 0]"));
        assert!(!formatted.contains("[stderr]"));
    }

    #[test]
    fn formats_stderr_section() {
        let output = ShellOutput {
            stdout: String::new(),
            stderr: "warning: unused variable\n".into(),
            exit_code: 0,
        };
        let formatted = format_shell_output(&output);
        assert!(formatted.contains("[stderr]"));
        assert!(formatted.contains("unused variable"));
    }
}
```

## Testing Error Paths

Error paths are where bugs hide. The LLM will send your tools unexpected inputs, and your tools will encounter unexpected system states. Test each error condition explicitly:

```rust
#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn read_file_permission_denied() {
        // Create a file and remove read permissions
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("secret.txt");
        std::fs::write(&file, "secret").unwrap();

        // Remove read permission (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o000);
            std::fs::set_permissions(&file, perms).unwrap();
        }

        let tool = ReadFileTool;
        let result = tool.execute(file.to_str().unwrap());

        #[cfg(unix)]
        assert!(result.is_err());

        // Restore permissions so tempdir cleanup succeeds
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o644);
            std::fs::set_permissions(&file, perms).unwrap();
        }
    }

    #[test]
    fn read_directory_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let tool = ReadFileTool;
        // Passing a directory path instead of a file path
        let result = tool.execute(dir.path().to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn read_binary_file_returns_content() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("data.bin");
        std::fs::write(&file, &[0xFF, 0xFE, 0x00, 0x01]).unwrap();

        let tool = ReadFileTool;
        // read_to_string will fail on non-UTF8 content
        let result = tool.execute(file.to_str().unwrap());
        assert!(result.is_err());
    }
}
```

::: info In the Wild
Claude Code's tool implementations are heavily tested at the unit level. Each tool has tests for happy paths, error paths, edge cases (empty files, binary files, very large files), and input validation. This thorough tool-level testing means that most bugs are caught before they ever reach the agentic loop, keeping integration tests focused on conversation flow rather than tool correctness.
:::

## Organizing Tool Tests

As your test suite grows, keep tests organized with a consistent structure:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Group tests by concern
    mod input_validation {
        use super::*;

        #[test]
        fn rejects_empty_path() { /* ... */ }

        #[test]
        fn rejects_path_traversal() { /* ... */ }
    }

    mod execution {
        use super::*;

        #[test]
        fn reads_existing_file() { /* ... */ }

        #[test]
        fn handles_large_files() { /* ... */ }
    }

    mod error_handling {
        use super::*;

        #[test]
        fn missing_file_returns_error() { /* ... */ }

        #[test]
        fn permission_denied_returns_error() { /* ... */ }
    }

    mod output_formatting {
        use super::*;

        #[test]
        fn formats_content_correctly() { /* ... */ }
    }
}
```

This nested module structure keeps related tests together and makes it easy to run just one group: `cargo test read_file::tests::error_handling`.

## Key Takeaways

- Test tools with real but isolated environments — use `tempfile::tempdir()` for filesystem operations, real shell commands for shell tools, and real git repos for git tools
- Structure tool tests around four concerns: input validation, execution behavior, error handling, and output formatting
- Test error paths explicitly — permission denied, missing files, binary content, oversized files, and malformed inputs are all conditions the LLM will trigger
- Organize tests into nested modules by concern so they stay maintainable as the test suite grows
- Tools are the highest-value testing target in your agent because they are fully deterministic and make up the majority of your codebase
