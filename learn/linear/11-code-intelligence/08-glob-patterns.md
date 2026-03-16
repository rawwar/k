---
title: Glob Patterns
description: File discovery using glob patterns — syntax rules, recursive matching, brace expansion, and efficient implementation for finding files in large directory trees.
---

# Glob Patterns

> **What you'll learn:**
> - Glob pattern syntax including wildcards (*), recursive descent (**), character classes ([abc]), and brace expansion ({a,b})
> - How glob matching is implemented efficiently using the ignore crate's gitignore-aware directory walker
> - Designing a file discovery tool for agents that combines glob patterns with gitignore rules and custom exclusion lists

Before you can search or parse a file, you need to *find* it. When an agent needs "all Rust source files in the project" or "every TOML file in the config directory," it uses glob patterns. Globs are the shell-style wildcard patterns you already know from commands like `ls *.rs` — but implementing them efficiently for a coding agent requires understanding the matching rules, the performance implications, and how to integrate with gitignore for smart file discovery.

## Glob Pattern Syntax

Glob patterns match file paths using wildcard characters. The core syntax elements are:

| Pattern | Matches | Example |
|---------|---------|---------|
| `*` | Any sequence of non-separator characters | `*.rs` matches `main.rs`, `lib.rs` |
| `**` | Any sequence of characters including path separators | `src/**/*.rs` matches `src/lib.rs`, `src/tools/shell.rs` |
| `?` | Any single character | `test?.rs` matches `test1.rs`, `testA.rs` |
| `[abc]` | Any single character in the set | `[ml]*.rs` matches `main.rs`, `lib.rs` |
| `[a-z]` | Any single character in the range | `[a-z]*.toml` matches files starting with a lowercase letter |
| `[!abc]` | Any single character NOT in the set | `[!.]*.rs` matches files not starting with a dot |
| `{a,b}` | Either alternative (brace expansion) | `*.{rs,toml}` matches `main.rs`, `Cargo.toml` |

The most important distinction is between `*` and `**`. A single `*` never crosses directory boundaries: `src/*.rs` matches `src/main.rs` but not `src/tools/shell.rs`. The double `**` crosses directory boundaries: `src/**/*.rs` matches both.

```rust
use glob::glob;

fn demonstrate_glob_patterns() {
    // Single wildcard — only top-level .rs files in src/
    println!("=== src/*.rs ===");
    for entry in glob("src/*.rs").expect("Invalid glob pattern") {
        if let Ok(path) = entry {
            println!("  {}", path.display());
        }
    }

    // Double wildcard — all .rs files recursively
    println!("\n=== src/**/*.rs ===");
    for entry in glob("src/**/*.rs").expect("Invalid glob pattern") {
        if let Ok(path) = entry {
            println!("  {}", path.display());
        }
    }

    // Brace expansion — multiple extensions
    println!("\n=== **/*.{{rs,toml}} ===");
    for entry in glob("**/*.{rs,toml}").expect("Invalid glob pattern") {
        if let Ok(path) = entry {
            println!("  {}", path.display());
        }
    }
}

fn main() {
    demonstrate_glob_patterns();
}
```

::: python Coming from Python
Python has both `glob.glob()` and `pathlib.Path.glob()`:
```python
from pathlib import Path
# Python's ** requires recursive=True in glob.glob
# But pathlib handles it naturally
for path in Path("src").glob("**/*.py"):
    print(path)
```
Rust's `glob` crate works the same way syntactically. The key difference is that neither Python's `glob` nor Rust's `glob` crate respects `.gitignore` by default — for that you need the `ignore` crate in Rust, which we cover below.
:::

## The `ignore` Crate: Gitignore-Aware File Walking

The `glob` crate works but has a significant limitation: it walks every file in the directory tree, including files in `.git/`, `node_modules/`, `target/`, and other directories that should be skipped. For large projects, this makes it impractically slow.

The `ignore` crate (maintained by the ripgrep author) solves this. It provides a directory walker that automatically reads and respects `.gitignore`, `.ignore`, `.rgignore`, and `.git/info/exclude` files. This is the same walker that powers ripgrep's file discovery.

```toml
[dependencies]
ignore = "0.4"
```

```rust
use ignore::WalkBuilder;
use std::path::Path;

fn find_files_with_ignore(root: &str, extension: &str) -> Vec<String> {
    let mut files = Vec::new();

    let walker = WalkBuilder::new(root)
        .hidden(false)       // Include hidden files (but .gitignore still applies)
        .git_ignore(true)    // Respect .gitignore
        .git_global(true)    // Respect global gitignore
        .git_exclude(true)   // Respect .git/info/exclude
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext == extension {
                    files.push(path.display().to_string());
                }
            }
        }
    }

    files
}

fn main() {
    let rust_files = find_files_with_ignore(".", "rs");
    println!("Found {} Rust files:", rust_files.len());
    for f in &rust_files {
        println!("  {}", f);
    }
}
```

The `WalkBuilder` is highly configurable. For an agent, the most useful options are:

```rust
use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;

fn build_agent_walker(root: &str, glob_pattern: Option<&str>) -> ignore::Walk {
    let mut builder = WalkBuilder::new(root);

    builder
        .hidden(false)        // Search hidden files like .env (agent often needs these)
        .git_ignore(true)     // Always respect .gitignore
        .git_global(true)     // Respect global gitignore
        .git_exclude(true)    // Respect .git/info/exclude
        .max_depth(Some(20))  // Prevent infinite recursion in pathological cases
        .follow_links(false)  // Don't follow symlinks (avoids cycles)
        .threads(4);          // Parallel walking with 4 threads

    // Add custom ignore patterns
    builder.add_custom_ignore_filename(".agentignore");

    // Apply glob filter if provided
    if let Some(pattern) = glob_pattern {
        let mut overrides = OverrideBuilder::new(root);
        overrides.add(pattern).expect("Invalid glob pattern");
        builder.overrides(overrides.build().expect("Failed to build overrides"));
    }

    builder.build()
}

fn main() {
    // Find all Rust files, respecting gitignore
    let walker = build_agent_walker(".", Some("*.rs"));

    for entry in walker {
        if let Ok(entry) = entry {
            if entry.path().is_file() {
                println!("{}", entry.path().display());
            }
        }
    }
}
```

## Parallel Directory Walking

For large codebases, the `ignore` crate supports parallel walking through its `WalkParallel` type. This distributes directory entries across multiple threads:

```rust
use ignore::WalkBuilder;
use std::sync::{Arc, Mutex};

fn parallel_file_discovery(root: &str, extension: &str) -> Vec<String> {
    let files = Arc::new(Mutex::new(Vec::new()));

    let walker = WalkBuilder::new(root)
        .git_ignore(true)
        .threads(num_cpus::get())  // Use all available cores
        .build_parallel();

    let files_clone = Arc::clone(&files);
    let ext = extension.to_string();

    walker.run(|| {
        let files = Arc::clone(&files_clone);
        let ext = ext.clone();

        Box::new(move |result| {
            if let Ok(entry) = result {
                let path = entry.path();
                if path.is_file() {
                    if let Some(file_ext) = path.extension().and_then(|e| e.to_str()) {
                        if file_ext == ext {
                            files.lock().unwrap().push(path.display().to_string());
                        }
                    }
                }
            }
            ignore::WalkState::Continue
        })
    });

    let mut result = Arc::try_unwrap(files).unwrap().into_inner().unwrap();
    result.sort(); // Parallel walk doesn't guarantee order
    result
}

fn main() {
    let files = parallel_file_discovery(".", "rs");
    println!("Found {} Rust files (parallel):", files.len());
    for f in &files {
        println!("  {}", f);
    }
}
```

The parallel walker is the same engine that ripgrep uses for file discovery. On a project with 50,000 files, it can enumerate all files in under 100ms on modern hardware. The sequential walker takes 300-500ms for the same task.

::: tip In the Wild
Claude Code's file discovery tool (Glob) uses gitignore-aware walking to avoid returning results from `node_modules/`, `target/`, `.git/`, and other directories that would pollute search results. It applies the same filtering that ripgrep uses, ensuring consistency between file listing and content search. OpenCode follows the same approach, using Go's `gitignore` library for the same purpose. The lesson is clear: always respect gitignore rules in agent tools — searching generated or vendored code wastes tokens and confuses the LLM.
:::

## Designing a Glob Tool for Agents

Putting it all together, here is a file discovery function designed for agent use. It supports glob patterns, respects gitignore, and returns results in a format suitable for LLM consumption:

```rust
use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct FileEntry {
    path: PathBuf,
    size_bytes: u64,
}

fn glob_search(
    root: &Path,
    pattern: &str,
    max_results: usize,
) -> Result<Vec<FileEntry>, String> {
    let mut builder = WalkBuilder::new(root);
    builder
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .hidden(false)
        .max_depth(Some(20));

    // Build glob override
    let mut overrides = OverrideBuilder::new(root);
    overrides.add(pattern).map_err(|e| format!("Invalid pattern: {}", e))?;
    builder.overrides(overrides.build().map_err(|e| format!("Override error: {}", e))?);

    let mut results = Vec::new();

    for entry in builder.build() {
        let entry = entry.map_err(|e| format!("Walk error: {}", e))?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let size = path.metadata()
            .map(|m| m.len())
            .unwrap_or(0);

        results.push(FileEntry {
            path: path.to_path_buf(),
            size_bytes: size,
        });

        if results.len() >= max_results {
            break;
        }
    }

    // Sort by path for deterministic output
    results.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(results)
}

fn main() {
    let root = Path::new(".");

    match glob_search(root, "**/*.rs", 100) {
        Ok(files) => {
            println!("Found {} files:", files.len());
            for entry in &files {
                println!("  {} ({} bytes)", entry.path.display(), entry.size_bytes);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

The `max_results` parameter is crucial for agent use. A glob like `**/*` in a large monorepo could return millions of files. By capping results, you prevent the agent from generating a response so large it overflows the context window. If the limit is hit, the agent can refine the pattern or ask the user to narrow the scope.

## Key Takeaways

- Glob patterns use `*` for single-directory wildcards, `**` for recursive descent, `?` for single characters, `[...]` for character classes, and `{a,b}` for alternatives
- The `ignore` crate provides gitignore-aware directory walking that automatically skips `.git/`, `node_modules/`, `target/`, and other ignored paths — always prefer it over the basic `glob` crate
- Parallel walking with `WalkBuilder::build_parallel()` leverages multiple cores for large codebases, matching the performance of ripgrep's file discovery
- Agent glob tools should cap results with `max_results` to prevent context window overflow and sort results for deterministic output
- Combining glob-based file discovery with content search (ripgrep) and structural analysis (tree-sitter) gives you a complete file-finding pipeline
