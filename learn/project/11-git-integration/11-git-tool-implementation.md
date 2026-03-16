---
title: Git Tool Implementation
description: Bringing all git capabilities together into a cohesive tool implementation with proper error handling, consistent output formatting, and integration with the agent's tool registry.
---

# Git Tool Implementation

> **What you'll learn:**
> - How to structure a unified git tool that exposes subcommands through the tool trait
> - Techniques for consistent error handling across different git operations
> - How to register git tools in the agent's tool system and wire them to the conversation loop

Throughout this chapter, you have built individual git capabilities: status parsing, branch management, commit creation, worktree isolation, diff generation, safety checks, conflict detection, and repository analysis. Now it is time to assemble them into a cohesive tool that the LLM can invoke through the agent's tool system. This subchapter brings everything together.

## Designing the Tool Interface

The LLM needs a clear interface to interact with git. You have two design options:

1. **One tool per operation** -- `git_status`, `git_diff`, `git_commit`, etc. This gives the LLM many small, focused tools.
2. **One tool with subcommands** -- a single `git` tool that accepts a `command` parameter. This keeps the tool count manageable.

Production coding agents typically use option 2. A single `git` tool with subcommands reduces the number of tools the LLM needs to reason about while still providing full functionality:

```rust
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The input schema for the git tool -- sent by the LLM
#[derive(Debug, Deserialize)]
pub struct GitToolInput {
    pub command: GitCommand,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum GitCommand {
    Status,
    Diff {
        #[serde(default)]
        cached: bool,
        #[serde(default)]
        paths: Vec<String>,
    },
    Log {
        #[serde(default = "default_log_count")]
        count: usize,
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        grep: Option<String>,
    },
    Blame {
        path: String,
        #[serde(default)]
        line_start: Option<usize>,
        #[serde(default)]
        line_end: Option<usize>,
    },
    Commit {
        message: String,
        #[serde(default)]
        files: Vec<String>,
    },
    Branch {
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        switch: bool,
        #[serde(default)]
        list: bool,
    },
    DiffStat {
        #[serde(default)]
        from: Option<String>,
        #[serde(default)]
        to: Option<String>,
    },
}

fn default_log_count() -> usize {
    10
}

/// The output returned to the LLM
#[derive(Debug, Serialize)]
pub struct GitToolOutput {
    pub success: bool,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
```

::: python Coming from Python
In Python, you might use a dictionary for the tool input: `{"action": "status"}` or `{"action": "commit", "message": "fix bug", "files": ["src/main.rs"]}`. Rust's enum with `serde(tag = "action")` gives you the same JSON structure but with compile-time guarantees that every variant is handled. When you add a new subcommand to `GitCommand`, the compiler forces you to add a handler for it -- you cannot accidentally forget.
:::

## Implementing the Tool Trait

Let's implement the `Tool` trait (from Chapter 4) for our git tool. The key design decisions are: where does the repository path come from, and how do we enforce safety?

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

// Simplified versions of our types for this example
struct GitToolInput {
    action: String,
    args: std::collections::HashMap<String, String>,
}

struct GitToolOutput {
    success: bool,
    output: String,
    error: Option<String>,
}

fn run_git_checked(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// The git tool with its configuration
pub struct GitTool {
    repo_path: PathBuf,
    max_diff_lines: usize,
    allow_commits: bool,
}

impl GitTool {
    pub fn new(repo_path: PathBuf) -> Self {
        Self {
            repo_path,
            max_diff_lines: 500,
            allow_commits: true,
        }
    }

    /// The tool's JSON schema description for the LLM
    pub fn schema() -> serde_json::Value {
        serde_json::json!({
            "name": "git",
            "description": "Interact with the git repository. Supports status, diff, log, blame, commit, and branch operations. Use this to understand code history, track changes, and create commits.",
            "input_schema": {
                "type": "object",
                "required": ["action"],
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["status", "diff", "log", "blame", "commit", "branch", "diff_stat"],
                        "description": "The git operation to perform"
                    },
                    "cached": {
                        "type": "boolean",
                        "description": "For diff: show staged changes instead of unstaged"
                    },
                    "paths": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "File paths to limit the operation to"
                    },
                    "count": {
                        "type": "integer",
                        "description": "For log: number of entries to show (default 10)"
                    },
                    "path": {
                        "type": "string",
                        "description": "For log/blame: specific file path"
                    },
                    "grep": {
                        "type": "string",
                        "description": "For log: filter commits by message pattern"
                    },
                    "line_start": {
                        "type": "integer",
                        "description": "For blame: starting line number"
                    },
                    "line_end": {
                        "type": "integer",
                        "description": "For blame: ending line number"
                    },
                    "message": {
                        "type": "string",
                        "description": "For commit: the commit message"
                    },
                    "files": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "For commit: files to stage and commit"
                    },
                    "name": {
                        "type": "string",
                        "description": "For branch: branch name to create or switch to"
                    },
                    "switch": {
                        "type": "boolean",
                        "description": "For branch: switch to the branch after creating"
                    },
                    "list": {
                        "type": "boolean",
                        "description": "For branch: list all branches"
                    }
                }
            }
        })
    }

    pub fn execute(&self, action: &str, args: &serde_json::Value) -> GitToolOutput {
        let result = match action {
            "status" => self.handle_status(),
            "diff" => self.handle_diff(args),
            "log" => self.handle_log(args),
            "blame" => self.handle_blame(args),
            "commit" => self.handle_commit(args),
            "branch" => self.handle_branch(args),
            "diff_stat" => self.handle_diff_stat(args),
            _ => Err(format!("Unknown git action: '{}'", action)),
        };

        match result {
            Ok(output) => GitToolOutput {
                success: true,
                output,
                error: None,
            },
            Err(e) => GitToolOutput {
                success: false,
                output: String::new(),
                error: Some(e),
            },
        }
    }

    fn handle_status(&self) -> Result<String, String> {
        let branch = run_git_checked(&self.repo_path, &["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap_or_else(|_| "(detached)".to_string());

        let status = run_git_checked(&self.repo_path, &["status", "--short"])?;

        if status.is_empty() {
            Ok(format!("On branch {}. Working tree is clean.", branch))
        } else {
            let file_count = status.lines().count();
            Ok(format!(
                "On branch {}. {} changed files:\n{}",
                branch, file_count, status
            ))
        }
    }

    fn handle_diff(&self, args: &serde_json::Value) -> Result<String, String> {
        let cached = args.get("cached").and_then(|v| v.as_bool()).unwrap_or(false);

        let mut git_args = vec!["diff", "-U3"];
        if cached {
            git_args.push("--cached");
        }

        // Add path filters
        if let Some(paths) = args.get("paths").and_then(|v| v.as_array()) {
            git_args.push("--");
            // We need to collect the paths as owned Strings to keep them alive
            let path_strings: Vec<String> = paths
                .iter()
                .filter_map(|p| p.as_str().map(String::from))
                .collect();

            let output = {
                let mut cmd = Command::new("git");
                cmd.args(&git_args).current_dir(&self.repo_path);
                for p in &path_strings {
                    cmd.arg(p);
                }
                cmd.output().map_err(|e| format!("Failed to run git: {}", e))?
            };

            let stdout = String::from_utf8_lossy(&output.stdout);
            return self.truncate_output(&stdout);
        }

        let output = run_git_checked(&self.repo_path, &git_args)?;
        self.truncate_output(&output)
    }

    fn handle_log(&self, args: &serde_json::Value) -> Result<String, String> {
        let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(10);
        let count_str = format!("-{}", count);

        let mut git_args = vec!["log", &count_str, "--format=%h %ci %s", "--no-merges"];

        let grep_arg;
        if let Some(grep) = args.get("grep").and_then(|v| v.as_str()) {
            grep_arg = format!("--grep={}", grep);
            git_args.push(&grep_arg);
            git_args.push("-i");
        }

        if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
            git_args.push("--");
            git_args.push(path);
        }

        run_git_checked(&self.repo_path, &git_args)
    }

    fn handle_blame(&self, args: &serde_json::Value) -> Result<String, String> {
        let path = args.get("path").and_then(|v| v.as_str())
            .ok_or_else(|| "blame requires a 'path' argument".to_string())?;

        let mut git_args = vec!["blame", "--no-pager"];

        let range_arg;
        if let (Some(start), Some(end)) = (
            args.get("line_start").and_then(|v| v.as_u64()),
            args.get("line_end").and_then(|v| v.as_u64()),
        ) {
            range_arg = format!("-L{},{}", start, end);
            git_args.push(&range_arg);
        }

        git_args.push(path);

        let output = run_git_checked(&self.repo_path, &git_args)?;
        self.truncate_output(&output)
    }

    fn handle_commit(&self, args: &serde_json::Value) -> Result<String, String> {
        if !self.allow_commits {
            return Err("Commits are disabled for this session.".to_string());
        }

        let message = args.get("message").and_then(|v| v.as_str())
            .ok_or_else(|| "commit requires a 'message' argument".to_string())?;

        // Stage specified files
        if let Some(files) = args.get("files").and_then(|v| v.as_array()) {
            for file in files {
                if let Some(path) = file.as_str() {
                    run_git_checked(&self.repo_path, &["add", "--", path])?;
                }
            }
        }

        // Verify something is staged
        let staged = run_git_checked(&self.repo_path, &["diff", "--cached", "--name-only"])?;
        if staged.is_empty() {
            return Err("Nothing to commit. Stage files first or specify files to commit.".to_string());
        }

        // Create the commit
        run_git_checked(&self.repo_path, &["commit", "-m", message])?;
        let hash = run_git_checked(&self.repo_path, &["rev-parse", "--short", "HEAD"])?;
        let file_count = staged.lines().count();

        Ok(format!(
            "Created commit {} with {} files.\nMessage: {}",
            hash, file_count, message
        ))
    }

    fn handle_branch(&self, args: &serde_json::Value) -> Result<String, String> {
        let list = args.get("list").and_then(|v| v.as_bool()).unwrap_or(false);

        if list {
            return run_git_checked(&self.repo_path, &["branch", "--list", "-v"]);
        }

        let name = args.get("name").and_then(|v| v.as_str())
            .ok_or_else(|| "branch requires a 'name' argument (or use 'list': true)".to_string())?;

        let switch = args.get("switch").and_then(|v| v.as_bool()).unwrap_or(false);

        if switch {
            // Try to switch to existing branch first, create if not exists
            match run_git_checked(&self.repo_path, &["switch", name]) {
                Ok(_) => Ok(format!("Switched to branch '{}'", name)),
                Err(_) => {
                    run_git_checked(&self.repo_path, &["switch", "-c", name])?;
                    Ok(format!("Created and switched to branch '{}'", name))
                }
            }
        } else {
            run_git_checked(&self.repo_path, &["branch", name])?;
            Ok(format!("Created branch '{}' (not switched)", name))
        }
    }

    fn handle_diff_stat(&self, args: &serde_json::Value) -> Result<String, String> {
        let mut git_args = vec!["diff", "--stat"];

        if let Some(from) = args.get("from").and_then(|v| v.as_str()) {
            git_args.push(from);
        }
        if let Some(to) = args.get("to").and_then(|v| v.as_str()) {
            git_args.push(to);
        }

        run_git_checked(&self.repo_path, &git_args)
    }

    /// Truncate output to stay within the configured line limit
    fn truncate_output(&self, output: &str) -> Result<String, String> {
        let lines: Vec<&str> = output.lines().collect();
        if lines.len() <= self.max_diff_lines {
            Ok(output.to_string())
        } else {
            let truncated: Vec<&str> = lines[..self.max_diff_lines].to_vec();
            Ok(format!(
                "{}\n\n[Output truncated: showing {} of {} lines]",
                truncated.join("\n"),
                self.max_diff_lines,
                lines.len()
            ))
        }
    }
}

fn main() {
    let tool = GitTool::new(PathBuf::from("."));

    // Print the tool schema
    println!("Tool schema:\n{}\n", serde_json::to_string_pretty(&GitTool::schema()).unwrap());

    // Simulate LLM tool calls
    let status_result = tool.execute("status", &serde_json::json!({}));
    println!("Status: {}\n", status_result.output);

    let log_result = tool.execute("log", &serde_json::json!({"count": 5}));
    println!("Log:\n{}\n", log_result.output);

    let branch_result = tool.execute("branch", &serde_json::json!({"list": true}));
    println!("Branches:\n{}\n", branch_result.output);
}
```

## Registering the Tool

In Chapter 4, you built a tool registry. Here is how the git tool plugs into it:

```rust
use std::path::PathBuf;

// Simplified tool trait from Chapter 4
trait Tool {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn execute(&self, input: &serde_json::Value) -> serde_json::Value;
}

struct GitTool {
    repo_path: PathBuf,
}

impl GitTool {
    fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }

    fn run_action(&self, action: &str, args: &serde_json::Value) -> Result<String, String> {
        // Dispatch to the appropriate handler (shown in detail above)
        match action {
            "status" => Ok("On branch main. Working tree clean.".to_string()),
            _ => Err(format!("Unknown action: {}", action)),
        }
    }
}

impl Tool for GitTool {
    fn name(&self) -> &str {
        "git"
    }

    fn description(&self) -> &str {
        "Interact with the git repository for version control operations"
    }

    fn execute(&self, input: &serde_json::Value) -> serde_json::Value {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("status");

        match self.run_action(action, input) {
            Ok(output) => serde_json::json!({
                "success": true,
                "output": output
            }),
            Err(e) => serde_json::json!({
                "success": false,
                "error": e
            }),
        }
    }
}

struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    fn new() -> Self {
        Self { tools: Vec::new() }
    }

    fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.iter().find(|t| t.name() == name).map(|t| t.as_ref())
    }
}

fn main() {
    let mut registry = ToolRegistry::new();

    // Only register git tool if we are in a git repository
    let repo_path = PathBuf::from(".");
    let is_git_repo = std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(&repo_path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if is_git_repo {
        registry.register(Box::new(GitTool::new(repo_path)));
        println!("Git tool registered");
    } else {
        println!("Not in a git repo -- git tool not available");
    }

    // Simulate a tool call from the LLM
    if let Some(git) = registry.get("git") {
        let result = git.execute(&serde_json::json!({"action": "status"}));
        println!("Result: {}", result);
    }
}
```

::: wild In the Wild
Claude Code exposes a single `bash` tool that can run any command, including git commands. However, it layers safety checks on top -- certain git operations are intercepted and blocked before they reach the shell. This approach gives maximum flexibility while maintaining safety. Other agents like OpenCode have dedicated git tools with explicit subcommands, which gives the LLM better guidance about what operations are available.
:::

## Error Handling Patterns

Git operations can fail for many reasons: the repository is in a bad state, a file does not exist, a branch name is invalid, the index is locked. Your tool needs consistent error handling that gives the LLM enough information to recover:

```rust
use std::path::Path;

/// Categorize git errors so the LLM can decide how to respond
#[derive(Debug)]
pub enum GitError {
    /// Not in a git repository
    NotARepo,
    /// The operation requires a clean working tree
    DirtyWorkingTree(String),
    /// A branch or ref does not exist
    RefNotFound(String),
    /// The git index is locked by another process
    IndexLocked,
    /// A merge conflict prevents the operation
    MergeConflict(Vec<String>),
    /// Permission or filesystem error
    IoError(String),
    /// Any other git error
    Other(String),
}

impl GitError {
    /// Classify a git error message into a specific variant
    pub fn classify(stderr: &str) -> Self {
        if stderr.contains("not a git repository") {
            GitError::NotARepo
        } else if stderr.contains("index.lock") {
            GitError::IndexLocked
        } else if stderr.contains("Please commit your changes or stash them") {
            GitError::DirtyWorkingTree(stderr.to_string())
        } else if stderr.contains("pathspec") && stderr.contains("did not match") {
            GitError::RefNotFound(stderr.to_string())
        } else if stderr.contains("CONFLICT") || stderr.contains("merge conflict") {
            GitError::MergeConflict(Vec::new())
        } else if stderr.contains("Permission denied") || stderr.contains("No such file") {
            GitError::IoError(stderr.to_string())
        } else {
            GitError::Other(stderr.to_string())
        }
    }

    /// Format the error for the LLM with recovery suggestions
    pub fn for_llm(&self) -> String {
        match self {
            GitError::NotARepo => {
                "Error: Not inside a git repository. Git operations are not available \
                 in this directory."
                    .to_string()
            }
            GitError::DirtyWorkingTree(_) => {
                "Error: Working tree has uncommitted changes that would be overwritten. \
                 Either commit the current changes first, or use the stash to save them \
                 temporarily."
                    .to_string()
            }
            GitError::RefNotFound(detail) => {
                format!(
                    "Error: The specified branch or reference was not found. \
                     Use 'git branch --list' to see available branches. Detail: {}",
                    detail
                )
            }
            GitError::IndexLocked => {
                "Error: The git index is locked, likely by another git process. \
                 Wait a moment and try again, or check for hung git processes."
                    .to_string()
            }
            GitError::MergeConflict(files) => {
                if files.is_empty() {
                    "Error: Merge conflict detected. Resolve conflicts before proceeding."
                        .to_string()
                } else {
                    format!(
                        "Error: Merge conflicts in {} files: {}. \
                         Resolve each conflict before proceeding.",
                        files.len(),
                        files.join(", ")
                    )
                }
            }
            GitError::IoError(detail) => {
                format!("Error: Filesystem error: {}", detail)
            }
            GitError::Other(detail) => {
                format!("Error: Git operation failed: {}", detail)
            }
        }
    }
}

fn main() {
    // Demonstrate error classification
    let errors = vec![
        "fatal: not a git repository (or any of the parent directories): .git",
        "error: Your local changes would be overwritten. Please commit your changes or stash them.",
        "fatal: Unable to create '/path/.git/index.lock': File exists.",
        "error: pathspec 'nonexistent-branch' did not match any file(s) known to git",
    ];

    for err_msg in errors {
        let classified = GitError::classify(err_msg);
        println!("Input: {}", err_msg);
        println!("LLM output: {}\n", classified.for_llm());
    }
}
```

## Key Takeaways

- Design the git tool as a single tool with subcommands (`action` field) rather than many separate tools -- this keeps the LLM's tool list manageable.
- Use serde's tagged enum (`#[serde(tag = "action")]`) to parse the LLM's JSON input directly into typed Rust variants with compile-time safety.
- Register the git tool conditionally -- only when the agent is running inside a git repository.
- Classify git errors into specific categories with recovery suggestions so the LLM can take corrective action instead of giving up.
- Always truncate large outputs (diffs, logs, blame) to stay within the LLM's context window while preserving the most useful information.
