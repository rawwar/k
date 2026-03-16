---
title: Git Basics for Agents
description: Foundational git concepts that coding agents need to understand, including the object model, working tree, staging area, and how agents interact with repositories differently than humans.
---

# Git Basics for Agents

> **What you'll learn:**
> - How the git object model (blobs, trees, commits, refs) maps to agent operations
> - Why the working tree, staging area, and HEAD distinction matters for automated workflows
> - How agent-driven git usage differs from interactive human usage

Before your agent runs a single `git` command, you need to understand what git actually manages and why it matters for automated code modification. Humans interact with git through muscle memory -- staging files, writing commit messages, resolving conflicts in an editor. An agent does none of that interactively. Every operation must be expressed as an explicit command with predictable output.

This subchapter establishes the mental model you need for the rest of the chapter. If you already know git well, you will still benefit from thinking about it through the lens of automation.

## The Git Object Model

Git stores everything as objects in a content-addressed database. There are four types:

- **Blobs** store file contents. A blob has no name or path -- it is just raw bytes identified by a SHA-1 hash.
- **Trees** store directory listings. Each entry maps a filename to a blob (for files) or another tree (for subdirectories).
- **Commits** point to a tree (the snapshot) plus metadata: author, timestamp, message, and one or more parent commits.
- **Refs** are human-readable names (`main`, `feature/add-search`, `HEAD`) that point to commit hashes.

For agent operations, the key insight is this: every commit is a complete snapshot of the entire project, not a diff. When your agent creates a commit, it captures the full state of the working tree at that moment. This means you can always return to any commit and get back a complete, working project -- which makes commits the perfect checkpoint mechanism for an agent that is experimenting with code changes.

```rust
use std::process::Command;

/// Inspect the git object model -- demonstrate that a commit points to a tree
fn show_commit_structure(repo_path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["cat-file", "-p", "HEAD"])
        .current_dir(repo_path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git cat-file failed: {}", stderr).into());
    }

    let commit_info = String::from_utf8(output.stdout)?;
    // Output looks like:
    // tree 8a7b3f...
    // parent 5c1d2e...
    // author Name <email> timestamp
    // committer Name <email> timestamp
    //
    // Commit message here
    Ok(commit_info)
}

fn main() {
    match show_commit_structure(".") {
        Ok(info) => println!("HEAD commit structure:\n{}", info),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

::: python Coming from Python
In Python, you might use the `gitpython` library (`import git; repo = git.Repo('.')`) which gives you object-oriented access to commits, trees, and blobs. In Rust, we are going to drive the `git` CLI directly via `std::process::Command`. This is the approach most production coding agents take -- it avoids linking against `libgit2` (which the `git2` crate wraps) and works with whatever git version the user has installed. The tradeoff is that you parse text output rather than calling a typed API, but for an agent that already processes natural language, parsing git output is straightforward.
:::

## Working Tree, Staging Area, and HEAD

Git tracks three distinct states for every file, and your agent needs to reason about all three:

**Working tree** is the actual files on disk. When your agent edits a file using its file-write tool, the change appears in the working tree immediately.

**Staging area** (also called the "index") is a snapshot of what will go into the next commit. Files must be explicitly added to the staging area with `git add`. This two-step process -- edit then stage -- gives the agent fine-grained control over which changes become part of a commit.

**HEAD** points to the current commit on the current branch. The difference between HEAD and the staging area is what `git diff --cached` shows. The difference between the staging area and the working tree is what `git diff` shows (with no flags).

```rust
use std::process::Command;

/// Check which of the three states a file is in
fn file_state(repo_path: &str, file_path: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Check working tree vs staging area (unstaged changes)
    let unstaged = Command::new("git")
        .args(["diff", "--name-only", "--", file_path])
        .current_dir(repo_path)
        .output()?;

    // Check staging area vs HEAD (staged changes)
    let staged = Command::new("git")
        .args(["diff", "--cached", "--name-only", "--", file_path])
        .current_dir(repo_path)
        .output()?;

    // Check if file is untracked
    let untracked = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard", "--", file_path])
        .current_dir(repo_path)
        .output()?;

    let has_unstaged = !unstaged.stdout.is_empty();
    let has_staged = !staged.stdout.is_empty();
    let is_untracked = !untracked.stdout.is_empty();

    let state = match (is_untracked, has_staged, has_unstaged) {
        (true, _, _) => "untracked (new file not yet added to git)",
        (false, true, true) => "partially staged (some changes staged, some not)",
        (false, true, false) => "fully staged (ready to commit)",
        (false, false, true) => "modified but unstaged",
        (false, false, false) => "clean (matches HEAD)",
    };

    Ok(format!("{}: {}", file_path, state))
}

fn main() {
    match file_state(".", "src/main.rs") {
        Ok(state) => println!("{}", state),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

The three-state model matters because an agent that blindly runs `git add -A` before every commit might accidentally stage files it did not intend to commit -- temporary debug output, generated files, or half-finished changes. Throughout this chapter, you will learn to stage selectively and verify state before committing.

## How Agent Git Usage Differs from Human Git Usage

When a human developer uses git, the workflow is interactive and judgment-driven. They look at `git diff`, decide what to stage, write a commit message by hand, and resolve merge conflicts in an editor. An agent must replace every one of these interactive steps with programmatic equivalents:

| Human Workflow | Agent Equivalent |
|---|---|
| Eyeball `git diff` output | Parse diff, count changed lines, summarize for LLM context |
| Decide what to stage | Stage only files the agent modified in this task |
| Write commit message | Generate message from change analysis or LLM |
| Resolve merge conflicts | Parse conflict markers, present to LLM, apply resolution |
| Interactive rebase | Never -- agents should not rewrite history |
| Force push | Never -- agents should only do non-destructive operations |

The fundamental principle is: **agents should only perform non-destructive git operations**. A human can recover from a bad `git reset --hard` because they remember what they were doing. An agent cannot. Your git integration layer should make it impossible to lose work.

::: wild In the Wild
Claude Code creates a git checkpoint (a stash or commit) before performing multi-file edits. If the agent's changes break something, the user can roll back to the checkpoint instantly. This "save before you edit" pattern is one of the most important safety mechanisms in production coding agents. OpenCode takes a similar approach, creating automatic savepoints that the user can restore with a single command.
:::

## Running Git Commands from Rust

Every git operation in this chapter follows the same pattern: build a `Command`, execute it, check the exit status, and parse stdout. Here is the foundational helper you will use throughout:

```rust
use std::path::Path;
use std::process::Command;

#[derive(Debug)]
pub struct GitOutput {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

/// Execute a git command in the given repository directory
pub fn run_git(repo_path: &Path, args: &[&str]) -> Result<GitOutput, std::io::Error> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()?;

    Ok(GitOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

/// Execute a git command and return an error if it fails
pub fn run_git_checked(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let result = run_git(repo_path, args).map_err(|e| format!("Failed to run git: {}", e))?;

    if result.success {
        Ok(result.stdout)
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            result.stderr.trim()
        ))
    }
}

fn main() {
    let repo = Path::new(".");

    // Check if we are in a git repository
    match run_git_checked(repo, &["rev-parse", "--is-inside-work-tree"]) {
        Ok(_) => println!("Inside a git repository"),
        Err(e) => println!("Not a git repo: {}", e),
    }

    // Get current branch name
    match run_git_checked(repo, &["rev-parse", "--abbrev-ref", "HEAD"]) {
        Ok(branch) => println!("Current branch: {}", branch.trim()),
        Err(e) => println!("Error: {}", e),
    }
}
```

Notice that `run_git` returns both stdout and stderr and lets the caller decide what to do with failures, while `run_git_checked` is a convenience wrapper for when you only care about success. You will see both patterns throughout the chapter.

## Detecting Repository Context

Before any git operation, the agent needs to know: "Am I inside a git repository?" and "Where is the repository root?" These two questions avoid confusing errors when the user opens the agent in a non-git directory.

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

/// Find the root of the git repository containing the given path
pub fn find_repo_root(working_dir: &Path) -> Result<PathBuf, String> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(working_dir)
        .output()
        .map_err(|e| format!("Failed to execute git: {}", e))?;

    if output.status.success() {
        let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(PathBuf::from(root))
    } else {
        Err("Not inside a git repository".to_string())
    }
}

/// Check if a path is inside a git repository
pub fn is_git_repo(path: &Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn main() {
    let cwd = Path::new(".");

    if is_git_repo(cwd) {
        match find_repo_root(cwd) {
            Ok(root) => println!("Repository root: {}", root.display()),
            Err(e) => eprintln!("{}", e),
        }
    } else {
        println!("Not inside a git repository -- git tools will be unavailable");
    }
}
```

Your agent's tool dispatcher should call `is_git_repo` at startup and conditionally enable or disable git tools based on the result. There is no point offering a "git status" tool if the user is not working in a repository.

## Key Takeaways

- Git stores complete snapshots (not diffs) in commits, making every commit a reliable checkpoint for agent rollback.
- The working tree, staging area, and HEAD are three distinct states -- your agent must reason about all three to stage and commit selectively.
- Agent git usage must be exclusively non-destructive: no force pushes, no hard resets, no history rewriting.
- Wrap all git commands in a `run_git` helper that captures stdout, stderr, and exit status for reliable error handling.
- Always detect repository context before offering git tools -- check `is_git_repo` and `find_repo_root` at agent startup.
