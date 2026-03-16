---
title: Working with Git in Rust
description: Two approaches to Git integration in Rust — spawning the git CLI as a child process versus using the git2 (libgit2) library for direct repository access.
---

# Working with Git in Rust

> **What you'll learn:**
> - The tradeoffs between spawning git commands (simple, full feature set, requires git installed) and using git2 (no dependency, API access, incomplete features)
> - How to use the git2 crate to open repositories, read objects, and perform common operations without subprocess overhead
> - When to use each approach: git2 for read-heavy operations and git CLI for complex operations like rebase, stash, and push

Now that you understand Git's object model, you need a way to interact with it from Rust. You have two main options: spawn the `git` command-line tool as a child process, or use the `git2` crate which wraps `libgit2`, a C library that reimplements Git's core operations. Most production agents use a mix of both, and understanding when to reach for each one is a key design decision.

## Approach 1: Spawning the Git CLI

The simplest approach is the one you already know from Chapter 7: spawn `git` as a child process using `std::process::Command`. This gives you access to every Git feature, since you are running the same `git` binary that users run on the command line.

```rust
use std::process::Command;
use std::path::Path;

/// Run a git command in the given repository directory and return stdout.
fn git_command(repo_dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to spawn git: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("git {} failed: {}", args.join(" "), stderr))
    }
}

fn main() {
    let repo = Path::new(".");

    // Get the current branch
    match git_command(repo, &["branch", "--show-current"]) {
        Ok(branch) => println!("Current branch: {}", branch.trim()),
        Err(e) => eprintln!("Error: {}", e),
    }

    // Get a short log
    match git_command(repo, &["log", "--oneline", "-5"]) {
        Ok(log) => println!("Recent commits:\n{}", log),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

This approach has clear advantages: it is straightforward, works with every Git feature, and produces the same output users expect from the command line. The downsides are subprocess overhead (each call forks a process), a dependency on `git` being installed, and the need to parse text output.

### Building a Typed Wrapper

Raw string output is fragile. Let's build a typed layer over the CLI approach so the rest of your agent works with structured data:

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct GitCli {
    repo_dir: PathBuf,
}

#[derive(Debug)]
pub struct CommitInfo {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

impl GitCli {
    pub fn new(repo_dir: impl Into<PathBuf>) -> Self {
        Self { repo_dir: repo_dir.into() }
    }

    fn run(&self, args: &[&str]) -> Result<String, String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_dir)
            .output()
            .map_err(|e| format!("Failed to spawn git: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("git {} failed: {}", args.join(" "), stderr))
        }
    }

    pub fn current_branch(&self) -> Result<String, String> {
        self.run(&["branch", "--show-current"])
            .map(|s| s.trim().to_string())
    }

    pub fn recent_commits(&self, count: usize) -> Result<Vec<CommitInfo>, String> {
        let count_str = format!("-{}", count);
        let output = self.run(&[
            "log", &count_str,
            "--format=%H%n%an%n%ai%n%s%n---"
        ])?;

        let mut commits = Vec::new();
        let mut lines = output.lines().peekable();

        while lines.peek().is_some() {
            let hash = match lines.next() {
                Some(h) if !h.is_empty() => h.to_string(),
                _ => break,
            };
            let author = lines.next().unwrap_or("").to_string();
            let date = lines.next().unwrap_or("").to_string();
            let message = lines.next().unwrap_or("").to_string();
            let _separator = lines.next(); // consume "---"

            commits.push(CommitInfo { hash, author, date, message });
        }

        Ok(commits)
    }

    pub fn is_clean(&self) -> Result<bool, String> {
        let output = self.run(&["status", "--porcelain"])?;
        Ok(output.trim().is_empty())
    }
}
```

## Approach 2: The git2 Crate

The `git2` crate provides Rust bindings to `libgit2`, giving you direct API access to the Git object database without spawning processes. Add it to your `Cargo.toml`:

```toml
[dependencies]
git2 = "0.19"
```

Opening a repository and reading basic information looks like this:

```rust
use git2::Repository;
use std::path::Path;

fn explore_repo(path: &Path) -> Result<(), git2::Error> {
    // Open the repository -- this finds .git by walking up
    let repo = Repository::discover(path)?;

    // Check if the working directory is clean
    let statuses = repo.statuses(None)?;
    println!("Modified files: {}", statuses.len());

    // Read HEAD
    let head = repo.head()?;
    if let Some(name) = head.shorthand() {
        println!("Current branch: {}", name);
    }

    // Get the commit HEAD points to
    let commit = head.peel_to_commit()?;
    println!("HEAD commit: {}", commit.id());
    println!("Author: {}", commit.author());
    println!("Message: {}", commit.message().unwrap_or("(no message)"));

    // Walk the commit history
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TIME)?;

    println!("\nRecent history:");
    for (i, oid) in revwalk.enumerate().take(5) {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        println!("  {} {}",
            &commit.id().to_string()[..8],
            commit.message().unwrap_or("").lines().next().unwrap_or(""));
        if i >= 4 { break; }
    }

    Ok(())
}
```

::: python Coming from Python
Python developers often use `GitPython` (`import git`) for similar functionality. The `git2` crate is more like Python's `pygit2` -- it wraps the same C library (`libgit2`) and provides low-level access to Git objects. The key difference in Rust is that `git2` returns `Result` types everywhere, forcing you to handle errors like "repository not found" or "HEAD is unborn" explicitly rather than catching exceptions.
:::

### Reading Trees and Blobs with git2

The `git2` crate gives you direct access to the object model you learned about in the previous subchapter:

```rust
use git2::Repository;
use std::path::Path;

fn read_file_at_commit(
    repo_path: &Path,
    commit_hash: &str,
    file_path: &str,
) -> Result<String, git2::Error> {
    let repo = Repository::discover(repo_path)?;
    let oid = git2::Oid::from_str(commit_hash)?;
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;

    // Navigate the tree to find our file
    let entry = tree.get_path(Path::new(file_path))?;
    let blob = repo.find_blob(entry.id())?;

    String::from_utf8(blob.content().to_vec())
        .map_err(|_| git2::Error::from_str("File is not valid UTF-8"))
}

fn list_files_in_commit(
    repo_path: &Path,
    commit_hash: &str,
) -> Result<Vec<String>, git2::Error> {
    let repo = Repository::discover(repo_path)?;
    let oid = git2::Oid::from_str(commit_hash)?;
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;

    let mut files = Vec::new();
    tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
        if let Some(name) = entry.name() {
            let path = if dir.is_empty() {
                name.to_string()
            } else {
                format!("{}{}", dir, name)
            };
            files.push(path);
        }
        git2::TreeWalkResult::Ok
    })?;

    Ok(files)
}
```

## When to Use Each Approach

The practical answer is: use both. Here is a decision framework for your agent:

**Use `git2` when:**
- You need to read repository state frequently (status checks, file lookups)
- You want to avoid subprocess overhead in hot paths
- You need to traverse the commit graph or object database
- You are running on a system where `git` might not be installed
- You need atomic operations with proper error handling

**Use the `git` CLI when:**
- You need features `libgit2` does not fully support (rebase, stash, push/pull with authentication)
- You need the exact output format users expect (for display purposes)
- You are prototyping and want the simplest implementation
- You need to run hooks (libgit2 does not run Git hooks)
- You are performing operations that involve remote communication (fetch, push, clone with SSH)

::: wild In the Wild
Claude Code primarily uses the Git CLI approach, spawning `git` commands through its shell execution tool. This is a pragmatic choice: the CLI gives access to every Git feature, and the output parsing overhead is acceptable for the frequency of Git operations in a typical agent session. The shell-based approach also means Git hooks run normally, which is important for repositories that use pre-commit hooks for linting or formatting. Some operations like reading repository status are frequent enough that a direct library approach would reduce latency, but the simplicity of a single integration path usually wins.
:::

## A Hybrid Approach

In practice, production agents benefit from a hybrid wrapper that uses `git2` for fast read operations and shells out for writes and complex operations:

```rust
use git2::Repository;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct GitRepo {
    path: PathBuf,
    repo: Repository,
}

impl GitRepo {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, String> {
        let path = path.into();
        let repo = Repository::discover(&path)
            .map_err(|e| format!("Not a git repository: {}", e))?;
        Ok(Self { path, repo })
    }

    /// Fast read via git2 -- no subprocess overhead
    pub fn current_branch(&self) -> Result<String, String> {
        let head = self.repo.head()
            .map_err(|e| format!("Failed to read HEAD: {}", e))?;
        Ok(head.shorthand().unwrap_or("HEAD").to_string())
    }

    /// Fast status check via git2
    pub fn has_changes(&self) -> Result<bool, String> {
        let statuses = self.repo.statuses(None)
            .map_err(|e| format!("Failed to get status: {}", e))?;
        Ok(!statuses.is_empty())
    }

    /// Shell out for commit -- runs hooks, handles edge cases
    pub fn commit(&self, message: &str) -> Result<String, String> {
        self.git_cli(&["commit", "-m", message])
    }

    /// Shell out for push -- needs SSH/HTTPS auth handling
    pub fn push(&self, remote: &str, branch: &str) -> Result<String, String> {
        self.git_cli(&["push", remote, branch])
    }

    /// Shell out for stash -- not well supported in libgit2
    pub fn stash_push(&self, message: &str) -> Result<String, String> {
        self.git_cli(&["stash", "push", "-m", message])
    }

    fn git_cli(&self, args: &[&str]) -> Result<String, String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .output()
            .map_err(|e| format!("Failed to spawn git: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("git {} failed: {}", args.join(" "), stderr))
        }
    }
}
```

This hybrid pattern gives you the best of both worlds: low-latency reads for operations your agent performs frequently (checking status, reading files) and full feature coverage for operations that need it (commit with hooks, push with authentication).

## Error Handling Patterns

Both approaches produce errors that your agent needs to handle gracefully. The most common failure modes are:

```rust
use std::path::Path;

fn safe_git_operation(repo_path: &Path) {
    // Common error: not in a git repository
    match git2::Repository::discover(repo_path) {
        Ok(repo) => {
            // Common error: HEAD doesn't exist (empty repo)
            match repo.head() {
                Ok(head) => println!("On branch: {}", head.shorthand().unwrap_or("?")),
                Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
                    println!("Repository has no commits yet");
                }
                Err(e) => println!("Error reading HEAD: {}", e),
            }
        }
        Err(_) => println!("Not inside a git repository"),
    }
}
```

Your agent should gracefully degrade when Git operations fail. A missing repository should not crash the agent -- it should inform the user and continue with reduced functionality.

## Key Takeaways

- The `git` CLI gives you access to every Git feature and produces familiar output, but incurs subprocess overhead and requires parsing text output.
- The `git2` crate provides direct API access to the Git object database, making it fast for read-heavy operations like status checks and history traversal.
- Production agents typically use a hybrid approach: `git2` for frequent reads (status, file content, branch info) and the CLI for writes and complex operations (commit with hooks, push, rebase).
- Always handle the common failure modes gracefully: not in a repository, empty repository with no commits, and detached HEAD state.
- `git2` does not run Git hooks or support all remote transport protocols -- when hooks or remote operations matter, shell out to the CLI.
