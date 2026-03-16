---
title: Git Object Model
description: The four Git object types — blobs, trees, commits, and tags — and how they form a content-addressable directed acyclic graph that represents repository history.
---

# Git Object Model

> **What you'll learn:**
> - How blobs store file contents, trees represent directory structures, and commits link snapshots to parent history
> - The content-addressable storage model where SHA-1 hashes identify objects and enable deduplication and integrity verification
> - How refs (branches, tags, HEAD) provide mutable pointers into the immutable commit graph

Before you write a single line of Git integration code, you need a mental model of what Git actually stores. Most developers interact with Git through commands like `git add`, `git commit`, and `git push` without understanding the data structures underneath. But when you are building an agent that manipulates repositories programmatically, understanding the object model is the difference between code that works and code that corrupts.

Git is, at its core, a content-addressable filesystem with a version control system built on top. Every piece of data Git stores is identified by the SHA-1 hash of its content. This simple idea has profound consequences for how an agent can reason about repository state.

## The Four Object Types

Git's object database contains exactly four types of objects: **blobs**, **trees**, **commits**, and **tags**. Everything else in Git -- branches, HEAD, the staging area -- is built on top of these four primitives.

### Blobs: File Contents

A blob is the simplest object. It stores the raw contents of a file with no metadata -- no filename, no permissions, no timestamps. Just bytes.

```rust
use std::process::Command;

fn inspect_blob() {
    // Create a temporary file and hash it to see a blob
    let output = Command::new("git")
        .args(["hash-object", "-w", "--stdin"])
        .arg("--stdin")
        .output()
        .expect("failed to run git hash-object");

    // You can also inspect an existing blob:
    // git cat-file -p <sha1> prints the raw content
    // git cat-file -t <sha1> prints "blob"
    let output = Command::new("git")
        .args(["cat-file", "-t", "HEAD:src/main.rs"])
        .output()
        .expect("failed to run git cat-file");

    let object_type = String::from_utf8_lossy(&output.stdout);
    println!("Object type: {}", object_type.trim()); // prints "blob"
}
```

The critical insight for agent developers: two files with identical content share the same blob, regardless of their filename or location. If your agent creates a file that already exists elsewhere in the repo with the same content, Git stores it only once. This deduplication is automatic and makes Git extremely space-efficient.

### Trees: Directory Snapshots

A tree object represents a directory. It contains a list of entries, where each entry has a mode (file permissions), a type (blob or tree), a SHA-1 hash, and a filename. Trees are recursive -- a tree can contain other trees, which is how Git represents nested directory structures.

```rust
use std::process::Command;

fn inspect_tree() {
    // List the top-level tree of the current commit
    let output = Command::new("git")
        .args(["cat-file", "-p", "HEAD^{tree}"])
        .output()
        .expect("failed to run git cat-file");

    let tree_contents = String::from_utf8_lossy(&output.stdout);
    // Output looks like:
    // 100644 blob a1b2c3d4...  Cargo.toml
    // 040000 tree e5f6a7b8...  src
    // 100644 blob c9d0e1f2...  README.md
    for line in tree_contents.lines() {
        let parts: Vec<&str> = line.splitn(4, |c| c == ' ' || c == '\t').collect();
        if parts.len() >= 4 {
            println!("mode={} type={} hash={} name={}",
                parts[0], parts[1], parts[2], parts[3]);
        }
    }
}
```

For an agent, trees are essential because they represent the complete state of the project at a given point. When you create a commit, you are actually creating a tree object that captures every file and directory, and then pointing a commit at that tree.

### Commits: History Links

A commit object ties everything together. It contains:
- A pointer to a tree (the project snapshot)
- Zero or more parent commit hashes (zero for the initial commit, one for normal commits, two or more for merges)
- Author information (name, email, timestamp)
- Committer information (can differ from author)
- The commit message

```rust
use std::process::Command;

fn inspect_commit() {
    let output = Command::new("git")
        .args(["cat-file", "-p", "HEAD"])
        .output()
        .expect("failed to run git cat-file");

    let commit_raw = String::from_utf8_lossy(&output.stdout);
    // Output looks like:
    // tree 4b825dc6...
    // parent a1b2c3d4...
    // author Jane Dev <jane@example.com> 1710000000 -0800
    // committer Jane Dev <jane@example.com> 1710000000 -0800
    //
    // Add error handling to shell tool

    for line in commit_raw.lines() {
        if line.starts_with("tree ") {
            println!("Points to tree: {}", &line[5..]);
        } else if line.starts_with("parent ") {
            println!("Parent commit: {}", &line[7..]);
        }
    }
}
```

The parent chain is what gives Git its history. Every commit points back to its predecessor, forming a directed acyclic graph (DAG). This is not a linear list -- merge commits have multiple parents, creating a graph structure that your agent needs to navigate when analyzing history.

### Tags: Named Anchors

Tags are named references to specific objects (usually commits). Lightweight tags are simply refs that point to a commit. Annotated tags are full objects with a tagger, date, message, and optional GPG signature. For agent work, you will mostly use lightweight tags or refs directly, as they are simpler to create programmatically.

## Content-Addressable Storage

Every object's identity is its SHA-1 hash, computed from the content prefixed with a type header. This means:

1. **Identical content produces identical hashes** -- automatic deduplication
2. **Any modification changes the hash** -- built-in integrity verification
3. **Objects are immutable** -- once created, they never change

This immutability is what makes Git safe for agents. When your agent creates a commit, it is adding new objects to the database without modifying existing ones. The old state is always there, reachable through parent pointers and the reflog.

::: python Coming from Python
In Python, you might model repository state with mutable dictionaries and lists. Git's approach is more like a functional data structure -- every "change" creates new objects rather than mutating existing ones. Think of it like how Python's `frozenset` or tuple types work: you create new values rather than modifying in place. This immutability is why Git can safely support undo operations, which is critical for agent safety.
:::

## Refs: The Mutable Layer

All Git objects are immutable, but you need mutable pointers to track "where are we now?" That is what refs provide. A ref is simply a file containing a SHA-1 hash.

- **Branches** are refs in `.git/refs/heads/`. The file `.git/refs/heads/main` contains the SHA-1 of the latest commit on `main`.
- **Tags** are refs in `.git/refs/tags/`.
- **HEAD** is a special ref (`.git/HEAD`) that usually contains a symbolic reference like `ref: refs/heads/main`, indicating which branch is checked out.
- **Remote-tracking branches** live in `.git/refs/remotes/`.

```rust
use std::fs;
use std::path::Path;

fn read_current_branch(repo_path: &Path) -> Option<String> {
    let head_path = repo_path.join(".git/HEAD");
    let content = fs::read_to_string(&head_path).ok()?;

    // HEAD is usually "ref: refs/heads/branch-name\n"
    if let Some(ref_path) = content.strip_prefix("ref: ") {
        let branch = ref_path.trim()
            .strip_prefix("refs/heads/")
            .unwrap_or(ref_path.trim());
        Some(branch.to_string())
    } else {
        // Detached HEAD -- content is a raw SHA-1
        Some(content.trim()[..8].to_string())
    }
}
```

When you run `git commit`, Git creates a new commit object, then updates the current branch ref to point to that new commit. The ref is the only mutable part of the equation. This two-layer design -- immutable objects plus mutable refs -- is what makes Git both safe and flexible.

## The DAG in Practice

The commit graph forms a DAG (directed acyclic graph) where each commit points to its parents. Understanding this structure helps your agent reason about operations like merge-base detection, branch divergence, and history traversal.

```rust
use std::process::Command;

fn find_merge_base(branch_a: &str, branch_b: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["merge-base", branch_a, branch_b])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None // No common ancestor
    }
}

fn count_commits_between(base: &str, tip: &str) -> usize {
    let range = format!("{}..{}", base, tip);
    let output = Command::new("git")
        .args(["rev-list", "--count", &range])
        .output()
        .expect("failed to count commits");

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}
```

::: tip In the Wild
Claude Code leverages its understanding of Git's object model to provide intelligent diff displays and commit message generation. When it reads the current repository state, it does not just check which files changed -- it understands the relationship between the working tree, the index (staging area), and the HEAD commit. This three-tree architecture (HEAD, index, working tree) is central to how Git status and diff work, and it is the foundation for the status and diff operations you will implement in subchapter 3.
:::

## The Index (Staging Area)

Between the last commit (HEAD) and the working directory sits the **index**, also called the staging area. The index is a binary file (`.git/index`) that represents the next tree to be committed. When you `git add` a file, you copy it from the working directory into the index. When you `git commit`, Git writes the index as a tree object and creates a commit pointing to that tree.

This three-way structure -- HEAD tree, index, and working tree -- is what makes `git status` possible. Git compares HEAD to the index to find staged changes, and the index to the working tree to find unstaged changes. Your agent needs to understand this to accurately report what has changed and what will be committed.

## Key Takeaways

- Git stores exactly four object types: blobs (file contents), trees (directories), commits (history), and tags (named anchors) -- everything else is built on these primitives.
- Content-addressable storage means every object is identified by its SHA-1 hash, providing automatic deduplication and integrity verification without any extra effort from your agent code.
- Objects are immutable; only refs (branches, tags, HEAD) are mutable -- this two-layer design is what makes Git operations safe and reversible for agent workflows.
- The index (staging area) sits between HEAD and the working tree, creating the three-way comparison that powers `git status` and `git diff`.
- Understanding the DAG structure of commits is essential for agent operations like merge-base detection, branch divergence analysis, and history traversal.
