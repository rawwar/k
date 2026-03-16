---
title: Grep at Scale
description: High-performance text search with ripgrep — regex patterns, file type filtering, parallel directory walking, and optimizations for searching large codebases.
---

# Grep at Scale

> **What you'll learn:**
> - How ripgrep achieves high performance through parallel directory traversal, memory-mapped I/O, and SIMD-accelerated regex matching
> - Using ripgrep's file type definitions, glob filters, and ignore file integration to narrow search scope efficiently
> - Combining text search with structural queries: using grep to find candidates and tree-sitter to validate structural matches

Tree-sitter gives you structural understanding of individual files. But before you can analyze a file's structure, you need to *find* the right files. In a codebase with thousands of files, the question "which files contain the string `DatabaseConnection`?" needs a fast answer. This is where text search — specifically, high-performance text search — remains essential.

Ripgrep (the `rg` command) has become the standard for fast code search. Written in Rust, it combines several algorithmic techniques to search orders of magnitude faster than traditional grep. Understanding how ripgrep works helps you use it effectively in your agent and understand the trade-offs between text search and structural search.

## Why Ripgrep Is Fast

Ripgrep's speed comes from multiple layers of optimization, each addressing a different bottleneck:

**Parallel directory traversal.** Ripgrep walks the directory tree using multiple threads. On modern SSDs, a single thread can be bottlenecked by system call latency — each `readdir()` call takes microseconds, and a large codebase might have tens of thousands of directories. Parallel traversal keeps all CPU cores busy issuing system calls concurrently.

**Gitignore-aware filtering.** Before even opening a file, ripgrep checks `.gitignore`, `.rgignore`, and `.ignore` patterns. In a typical project, this eliminates `node_modules/`, `target/`, `.git/`, and other directories that contain thousands of files you never want to search. Skipping these directories is ripgrep's single biggest performance win — it often reduces the search space by 90% or more.

**Memory-mapped I/O.** For large files, ripgrep uses `mmap()` instead of `read()`. Memory mapping lets the operating system's virtual memory system handle buffering and page management, avoiding redundant copying between kernel and user space. This is faster than reading the file into a heap buffer, especially for files larger than the OS page cache.

**SIMD-accelerated string matching.** Ripgrep uses the `memchr` crate, which leverages SIMD (Single Instruction, Multiple Data) instructions to scan bytes 16 or 32 at a time. For a simple literal string search, this means processing 32 bytes per CPU cycle instead of one.

**The Aho-Corasick and regex engines.** For multi-pattern searches, ripgrep uses the Aho-Corasick algorithm, which matches multiple strings simultaneously in a single pass. For regex patterns, it uses the `regex` crate's Thompson NFA engine, which guarantees linear time — no catastrophic backtracking.

::: python Coming from Python
Python developers often use `subprocess` to call `grep` or `rg`, or use `os.walk()` with `re.search()` for in-process search. The in-process approach is dramatically slower: Python's `re` module uses a backtracking regex engine (exponential worst case), `os.walk()` is single-threaded, and Python's GIL prevents true parallelism. Ripgrep, called from Rust or via subprocess, is typically 10-100x faster than a Python-native search. The `grep` crate brings ripgrep's matching engine into your Rust process.
:::

## Using Ripgrep from Rust

There are two ways to use ripgrep from a Rust agent: calling the `rg` binary as a subprocess, or using the underlying `grep` crate family directly. The subprocess approach is simpler and what most agents use:

```rust
use std::process::Command;

#[derive(Debug)]
struct GrepMatch {
    file: String,
    line_number: usize,
    line_text: String,
}

fn ripgrep_search(
    pattern: &str,
    directory: &str,
    file_glob: Option<&str>,
) -> Result<Vec<GrepMatch>, String> {
    let mut cmd = Command::new("rg");

    // Core flags
    cmd.arg("--line-number")   // Include line numbers
       .arg("--no-heading")     // File path on every line (easier to parse)
       .arg("--color=never");   // No ANSI escape codes

    // Optional file type filter
    if let Some(glob) = file_glob {
        cmd.arg("--glob").arg(glob);
    }

    cmd.arg(pattern).arg(directory);

    let output = cmd.output().map_err(|e| format!("Failed to run rg: {}", e))?;

    if !output.status.success() && output.status.code() != Some(1) {
        // Exit code 1 means "no matches" — that's not an error
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("rg failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut matches = Vec::new();

    for line in stdout.lines() {
        // Format: file:line_number:line_text
        let mut parts = line.splitn(3, ':');
        if let (Some(file), Some(line_num), Some(text)) =
            (parts.next(), parts.next(), parts.next())
        {
            if let Ok(num) = line_num.parse::<usize>() {
                matches.push(GrepMatch {
                    file: file.to_string(),
                    line_number: num,
                    line_text: text.to_string(),
                });
            }
        }
    }

    Ok(matches)
}

fn main() {
    match ripgrep_search("fn.*->.*Result", "src/", Some("*.rs")) {
        Ok(matches) => {
            println!("Found {} matches:", matches.len());
            for m in &matches {
                println!("  {}:{} — {}", m.file, m.line_number, m.line_text.trim());
            }
        }
        Err(e) => eprintln!("Search failed: {}", e),
    }
}
```

### The `grep` Crate for In-Process Search

For tighter integration, the `grep` family of crates provides ripgrep's functionality as a library. The main crates are `grep-regex` for regex matching, `grep-searcher` for file searching, and `grep-matcher` for the trait abstraction:

```rust
use grep_regex::RegexMatcher;
use grep_searcher::Searcher;
use grep_searcher::sinks::UTF8;
use std::path::Path;

fn search_file(path: &Path, pattern: &str) -> Result<Vec<(usize, String)>, Box<dyn std::error::Error>> {
    let matcher = RegexMatcher::new(pattern)?;
    let mut matches = Vec::new();

    Searcher::new().search_path(
        &matcher,
        path,
        UTF8(|line_number, line| {
            matches.push((line_number as usize, line.to_string()));
            Ok(true) // Continue searching
        }),
    )?;

    Ok(matches)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let results = search_file(Path::new("src/main.rs"), r"fn\s+\w+")?;
    for (line_num, text) in &results {
        println!("Line {}: {}", line_num, text.trim());
    }
    Ok(())
}
```

The in-process approach avoids subprocess overhead and gives you programmatic access to search results. The trade-off is more complex setup — you need to handle directory walking, ignore files, and parallelism yourself (or use the `ignore` crate, which we cover in the Glob Patterns subchapter).

## Smart Search Strategies for Agents

An agent does not just search blindly. It applies strategies to narrow the search space and rank results:

**File type filtering.** When searching for Rust function definitions, filter to `*.rs` files. When searching for configuration, filter to `*.toml`, `*.yaml`, `*.json`. This eliminates noise from irrelevant file types.

**Directory scoping.** If the agent knows the task involves the `server` module, search only in `src/server/` rather than the entire project. Ripgrep's directory argument naturally scopes the search.

**Context lines.** Include a few lines before and after each match (`-C 3` flag) to give the LLM enough context to understand the match without reading the full file. This is a token-efficient strategy — 7 lines of context is often enough to determine if a match is relevant.

**Result limiting.** Cap the number of results to prevent flooding the context window. If a search for `error` returns 500 matches, the agent should refine the query rather than dumping all 500 into the prompt.

```rust
use std::process::Command;

fn smart_search(
    pattern: &str,
    directory: &str,
    file_types: &[&str],
    context_lines: usize,
    max_results: usize,
) -> Result<String, String> {
    let mut cmd = Command::new("rg");

    cmd.arg("--line-number")
       .arg("--no-heading")
       .arg("--color=never")
       .arg(format!("-C{}", context_lines))
       .arg(format!("--max-count={}", max_results));

    for ft in file_types {
        cmd.arg("--glob").arg(ft);
    }

    // Respect common ignore patterns
    cmd.arg("--hidden")        // Search hidden files
       .arg("--glob=!.git/");  // But skip .git directory

    cmd.arg(pattern).arg(directory);

    let output = cmd.output().map_err(|e| format!("Failed to run rg: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.to_string())
}

fn main() {
    match smart_search(
        r"impl\s+\w+\s+for",
        "src/",
        &["*.rs"],
        3,   // 3 lines of context
        20,  // Max 20 matches per file
    ) {
        Ok(results) => println!("{}", results),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

::: tip In the Wild
Claude Code wraps ripgrep behind its Grep tool, adding several agent-specific behaviors: it always respects `.gitignore`, it limits results to prevent context window overflow, and it strips ANSI color codes from output. OpenCode similarly shells out to `rg` and post-processes results. The key insight is that agents use grep not for human-readable output, but as a structured data source — each match has a file path, line number, and text that feeds into the next step of the agent's reasoning.
:::

## Combining Text Search with Structural Search

The most powerful pattern is using text search as a fast first pass and tree-sitter as a precise second pass. Text search is O(n) in file size with a tiny constant factor. Tree-sitter parsing is more expensive but gives you structural precision. Combining them gives you the best of both worlds:

```rust
use std::process::Command;
use tree_sitter::{Parser, Query, QueryCursor};

fn find_async_functions_returning_result(directory: &str) -> Vec<String> {
    // Step 1: Fast text search to find candidate files
    let output = Command::new("rg")
        .args(["--files-with-matches", "async fn", directory, "--glob", "*.rs"])
        .output()
        .expect("Failed to run rg");

    let candidates = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();

    // Step 2: Parse each candidate with tree-sitter for structural validation
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    parser.set_language(&language).unwrap();

    let query = Query::new(&language, r#"
        (function_item
            name: (identifier) @func_name
            return_type: (generic_type
                type: (type_identifier) @ret_type
                (#eq? @ret_type "Result")))
    "#).unwrap();

    let name_idx = query.capture_index_for_name("func_name").unwrap();

    for file_path in candidates.lines() {
        if let Ok(source) = std::fs::read_to_string(file_path) {
            if let Some(tree) = parser.parse(&source, None) {
                let mut cursor = QueryCursor::new();
                for m in cursor.matches(&query, tree.root_node(), source.as_bytes()) {
                    for capture in m.captures {
                        if capture.index == name_idx {
                            let name = &source[capture.node.start_byte()..capture.node.end_byte()];
                            results.push(format!("{}:{} — {}", file_path, capture.node.start_position().row + 1, name));
                        }
                    }
                }
            }
        }
    }

    results
}
```

Text search narrows thousands of files down to a handful of candidates. Tree-sitter then applies precise structural matching to those candidates. The result is both fast and accurate.

## Key Takeaways

- Ripgrep achieves high performance through parallel directory walking, gitignore-aware filtering, memory-mapped I/O, and SIMD-accelerated string matching
- Calling ripgrep as a subprocess with `--line-number --no-heading --color=never` produces easily parseable output for agent consumption
- Smart search strategies — file type filtering, directory scoping, context lines, and result limiting — prevent context window overflow
- The `grep` crate family provides ripgrep's functionality as a Rust library for in-process search without subprocess overhead
- The most powerful pattern combines text search as a fast first pass (finding candidate files) with tree-sitter as a precise second pass (structural validation)
