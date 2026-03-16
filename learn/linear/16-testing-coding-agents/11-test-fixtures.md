---
title: Test Fixtures
description: Design reusable test fixtures that provide common setup for agent tests, including workspace scaffolding, mock providers, and conversation histories.
---

# Test Fixtures

> **What you'll learn:**
> - How to build test fixture factories that scaffold temporary workspaces with predefined file structures, git repos, and configuration files
> - Techniques for creating reusable conversation fixture builders that set up multi-turn histories with tool calls and results for testing specific scenarios
> - How to manage fixture data (recorded responses, sample codebases, expected outputs) in the repository without bloating the test suite

As your test suite grows, you notice the same setup code appearing in test after test: create a temp directory, write some files, set up a mock provider, build tool instances. Test fixtures extract this shared setup into reusable components. Good fixtures make tests shorter, more readable, and easier to maintain.

Rust does not have pytest-style fixture injection, but it has something just as powerful: regular functions, builders, and the type system. You build fixtures as helper functions and structs that tests call explicitly. The result is more verbose than Python's magic-injection approach but also more transparent — you can always see exactly what setup code runs by reading the test.

## Workspace Fixtures

The most common fixture in agent testing is a temporary workspace with a predefined file structure. Build a factory that makes this one line:

```rust
use tempfile::TempDir;
use std::path::{Path, PathBuf};

pub struct TestWorkspace {
    dir: TempDir,
}

impl TestWorkspace {
    /// Create an empty workspace.
    pub fn empty() -> Self {
        Self {
            dir: TempDir::new().unwrap(),
        }
    }

    /// Create a workspace with a minimal Rust project.
    pub fn rust_project() -> Self {
        let ws = Self::empty();
        ws.write_file(
            "Cargo.toml",
            r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#,
        );
        ws.write_file("src/main.rs", "fn main() {\n    println!(\"Hello\");\n}\n");
        ws
    }

    /// Create a workspace with a Rust project that has a failing test.
    pub fn rust_project_with_failing_test() -> Self {
        let ws = Self::rust_project();
        ws.write_file(
            "src/lib.rs",
            r#"pub fn add(a: i32, b: i32) -> i32 {
    a - b  // Bug: should be a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }
}
"#,
        );
        ws
    }

    /// Create a workspace with multiple source files.
    pub fn multi_file_project() -> Self {
        let ws = Self::rust_project();
        ws.write_file(
            "src/lib.rs",
            "pub mod utils;\npub mod config;\n",
        );
        ws.write_file(
            "src/utils.rs",
            "pub fn greet(name: &str) -> String {\n    format!(\"Hello, {}!\", name)\n}\n",
        );
        ws.write_file(
            "src/config.rs",
            "pub const MAX_RETRIES: u32 = 3;\n",
        );
        ws
    }

    /// Write a file relative to the workspace root.
    pub fn write_file(&self, relative_path: &str, content: &str) {
        let path = self.dir.path().join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    /// Read a file relative to the workspace root.
    pub fn read_file(&self, relative_path: &str) -> String {
        let path = self.dir.path().join(relative_path);
        std::fs::read_to_string(path).unwrap()
    }

    /// Check if a file exists.
    pub fn file_exists(&self, relative_path: &str) -> bool {
        self.dir.path().join(relative_path).exists()
    }

    /// Get the workspace root path.
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Get the path as a string.
    pub fn path_str(&self) -> &str {
        self.dir.path().to_str().unwrap()
    }
}
```

Now your tests are clean:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_tool_finds_file_in_workspace() {
        let ws = TestWorkspace::rust_project();
        let tool = ReadFileTool::new(ws.path_str());

        let result = tool.execute("src/main.rs").unwrap();
        assert!(result.contains("println!"));
    }

    #[test]
    fn write_tool_creates_new_file() {
        let ws = TestWorkspace::empty();
        let tool = WriteFileTool::new(ws.path_str());

        tool.execute("hello.txt", "Hello!").unwrap();
        assert_eq!(ws.read_file("hello.txt"), "Hello!");
    }
}
```

::: tip Coming from Python
In pytest, fixtures are functions decorated with `@pytest.fixture` and injected by name:
```python
@pytest.fixture
def rust_workspace(tmp_path):
    (tmp_path / "Cargo.toml").write_text('[package]\nname = "test"\n')
    (tmp_path / "src").mkdir()
    (tmp_path / "src/main.rs").write_text('fn main() {}')
    return tmp_path

def test_read_tool(rust_workspace):
    tool = ReadFileTool(str(rust_workspace))
    result = tool.execute("src/main.rs")
    assert "fn main" in result
```
Rust's approach is explicit: you call `TestWorkspace::rust_project()` instead of declaring a parameter. This means you can see the fixture creation at the call site, which makes tests easier to understand without jumping to fixture definitions. The trade-off is slightly more typing, but the readability payoff is worth it.
:::

## Git Repository Fixtures

Some tools need a real git repository. Build a fixture that creates one:

```rust
impl TestWorkspace {
    /// Create a workspace with an initialized git repo.
    pub fn git_repo() -> Self {
        let ws = Self::rust_project();
        ws.run_git(&["init"]);
        ws.run_git(&["add", "."]);
        ws.run_git(&["commit", "-m", "Initial commit"]);
        ws
    }

    /// Create a git repo with multiple commits for testing git log, diff, etc.
    pub fn git_repo_with_history() -> Self {
        let ws = Self::git_repo();

        // Second commit
        ws.write_file("src/lib.rs", "pub fn hello() -> &'static str {\n    \"hello\"\n}\n");
        ws.run_git(&["add", "."]);
        ws.run_git(&["commit", "-m", "Add lib.rs"]);

        // Third commit
        ws.write_file("README.md", "# Test Project\n");
        ws.run_git(&["add", "."]);
        ws.run_git(&["commit", "-m", "Add README"]);

        ws
    }

    fn run_git(&self, args: &[&str]) {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(self.dir.path())
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@test.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@test.com")
            .output()
            .expect("git command failed");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[cfg(test)]
mod git_tests {
    use super::*;

    #[test]
    fn git_repo_has_clean_status() {
        let ws = TestWorkspace::git_repo();
        let output = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(ws.path())
            .output()
            .unwrap();
        assert!(output.stdout.is_empty(), "Repo should be clean after initial commit");
    }

    #[test]
    fn git_repo_with_history_has_three_commits() {
        let ws = TestWorkspace::git_repo_with_history();
        let output = std::process::Command::new("git")
            .args(["rev-list", "--count", "HEAD"])
            .current_dir(ws.path())
            .output()
            .unwrap();
        let count: usize = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .unwrap();
        assert_eq!(count, 3);
    }
}
```

## Conversation Fixtures

For integration tests, you need pre-built conversation histories. A builder makes this ergonomic:

```rust
pub struct ConversationFixture {
    pub messages: Vec<Message>,
    pub responses: Vec<LlmResponse>,
}

pub struct ConversationBuilder {
    messages: Vec<Message>,
    responses: Vec<LlmResponse>,
}

impl ConversationBuilder {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            responses: Vec::new(),
        }
    }

    pub fn user_says(mut self, text: &str) -> Self {
        self.messages.push(Message {
            role: "user".to_string(),
            content: MessageContent::Text(text.to_string()),
        });
        self
    }

    pub fn assistant_responds_with_text(mut self, text: &str) -> Self {
        self.responses.push(
            ResponseBuilder::new()
                .text(text)
                .stop_reason(StopReason::EndTurn)
                .build(),
        );
        self
    }

    pub fn assistant_calls_tool(mut self, name: &str, input: serde_json::Value) -> Self {
        self.responses.push(
            ResponseBuilder::new()
                .tool_use(name, input)
                .build(),
        );
        self
    }

    pub fn tool_returns(mut self, tool_use_id: &str, result: &str) -> Self {
        self.messages.push(Message {
            role: "user".to_string(),
            content: MessageContent::ToolResult {
                tool_use_id: tool_use_id.to_string(),
                content: result.to_string(),
            },
        });
        self
    }

    pub fn build(self) -> ConversationFixture {
        ConversationFixture {
            messages: self.messages,
            responses: self.responses,
        }
    }
}

/// Pre-built conversation fixtures for common scenarios.
pub mod conversations {
    use super::*;
    use serde_json::json;

    pub fn simple_question_and_answer() -> ConversationFixture {
        ConversationBuilder::new()
            .user_says("What is Rust?")
            .assistant_responds_with_text(
                "Rust is a systems programming language focused on safety and performance.",
            )
            .build()
    }

    pub fn read_file_then_answer() -> ConversationFixture {
        ConversationBuilder::new()
            .user_says("What does main.rs do?")
            .assistant_calls_tool("read_file", json!({"path": "src/main.rs"}))
            .tool_returns("tool_0", "fn main() {\n    println!(\"Hello\");\n}\n")
            .assistant_responds_with_text("It prints Hello to the console.")
            .build()
    }

    pub fn multi_step_edit() -> ConversationFixture {
        ConversationBuilder::new()
            .user_says("Add a greet function to lib.rs")
            .assistant_calls_tool("read_file", json!({"path": "src/lib.rs"}))
            .tool_returns("tool_0", "// empty lib\n")
            .assistant_calls_tool("write_file", json!({
                "path": "src/lib.rs",
                "content": "pub fn greet(name: &str) -> String {\n    format!(\"Hello, {}!\", name)\n}\n"
            }))
            .tool_returns("tool_1", "File written successfully")
            .assistant_responds_with_text("Done! I added a greet function to lib.rs.")
            .build()
    }
}
```

Usage in tests becomes one line:

```rust
#[tokio::test]
async fn test_read_file_conversation() {
    let fixture = conversations::read_file_then_answer();
    let provider = MockProvider::new(fixture.responses);
    let ws = TestWorkspace::rust_project();
    let tools = create_tools(ws.path());

    let agent = AgentLoop::new(std::sync::Arc::new(provider), tools, 10);
    let actions = agent.run("What does main.rs do?").await;

    assert!(actions.iter().any(|a| matches!(&a.kind, ActionKind::Finished(_))));
}
```

## Managing Fixture Data Files

For recorded replay fixtures and sample codebases, keep a clear directory structure:

```
tests/
  fixtures/
    recordings/
      read_file_conversation.json
      fix_bug_conversation.json
      multi_step_edit.json
    workspaces/
      simple_rust_project/
        Cargo.toml
        src/
          main.rs
      buggy_project/
        Cargo.toml
        src/
          lib.rs
    expected_outputs/
      formatted_tool_result.txt
      api_request.json
```

Load fixture files with a helper:

```rust
pub fn load_fixture(relative_path: &str) -> String {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = std::path::Path::new(&manifest_dir)
        .join("tests")
        .join("fixtures")
        .join(relative_path);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to load fixture {}: {}", relative_path, e))
}

pub fn load_json_fixture<T: serde::de::DeserializeOwned>(relative_path: &str) -> T {
    let content = load_fixture(relative_path);
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse fixture {}: {}", relative_path, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_recording_fixture() {
        // This test verifies the fixture loading mechanism itself
        let fixture_path = "recordings/read_file_conversation.json";
        // Only run if the fixture file exists
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let full_path = std::path::Path::new(&manifest_dir)
            .join("tests/fixtures")
            .join(fixture_path);
        if full_path.exists() {
            let content = load_fixture(fixture_path);
            assert!(!content.is_empty());
        }
    }
}
```

::: info In the Wild
Production coding agents like Claude Code use a combination of fixture approaches: code-based fixtures (builder functions) for mock providers and conversation histories, and file-based fixtures (JSON recordings) for replay tests. The code-based fixtures are preferred when the fixture is simple enough to express in a few lines of code. File-based fixtures are used when the data is complex (real API responses with many fields) or when the fixture was generated by a recording system rather than authored by hand.
:::

## Keeping Fixtures Maintainable

As the test suite grows, fixtures accumulate. Keep them maintainable with these practices:

1. **Name fixtures descriptively.** `rust_project_with_failing_test()` is better than `workspace_v2()`.
2. **Compose fixtures from smaller pieces.** `git_repo()` builds on `rust_project()` which builds on `empty()`. Adding a new variant means combining existing ones.
3. **Version fixture data files.** If you change the recording format, update all fixture files in a single commit.
4. **Delete unused fixtures.** Dead fixtures confuse future developers. If no test uses a fixture, remove it.
5. **Document complex fixtures.** If a fixture sets up a specific scenario (a merge conflict, a partially completed refactoring), document why.

```rust
impl TestWorkspace {
    /// Create a workspace that simulates a merge conflict.
    ///
    /// The workspace has a file `src/lib.rs` with conflict markers,
    /// as if `git merge` left unresolved conflicts. This is used
    /// to test the agent's ability to resolve merge conflicts.
    pub fn merge_conflict() -> Self {
        let ws = Self::git_repo();
        ws.write_file("src/lib.rs", r#"<<<<<<< HEAD
pub fn hello() -> &'static str {
    "hello from main"
}
=======
pub fn hello() -> &'static str {
    "hello from feature"
}
>>>>>>> feature
"#);
        ws
    }
}
```

## Key Takeaways

- Build workspace fixtures as factory methods on a `TestWorkspace` struct that wraps `TempDir`, providing pre-built scenarios like empty workspaces, Rust projects, git repos, and projects with specific bugs
- Use conversation builders to create reusable multi-turn conversation fixtures with tool calls and results, making integration test setup a single line
- Store fixture data files (recordings, sample codebases, expected outputs) in a `tests/fixtures/` directory with a clear organizational structure
- Load fixture files using `CARGO_MANIFEST_DIR` to build absolute paths, ensuring tests work regardless of the working directory
- Keep fixtures maintainable by naming them descriptively, composing complex fixtures from simpler ones, and removing unused fixtures
