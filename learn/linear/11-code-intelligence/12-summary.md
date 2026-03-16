---
title: Summary
description: Review of code intelligence tools and techniques with connections to version control integration in the next chapter.
---

# Summary

> **What you'll learn:**
> - How tree-sitter, ripgrep, glob patterns, and LSP form a comprehensive code intelligence stack for a coding agent
> - Which code intelligence capabilities are essential for the version control integration features in Chapter 12
> - Key architectural decisions for code intelligence and their impact on agent accuracy and performance across languages

This chapter took you from the limitations of text search to a complete code intelligence stack. Let's consolidate what you have learned and see how these tools work together in practice.

## The Code Intelligence Stack

You now have four layers of code understanding, each serving a different purpose:

**Layer 1: File Discovery (glob + ignore crate).** Before you can analyze code, you need to find the right files. The `ignore` crate provides gitignore-aware, parallel directory walking that efficiently enumerates files matching glob patterns like `**/*.rs` or `src/**/*.{ts,tsx}`. This layer respects `.gitignore` to avoid searching generated code and dependencies.

**Layer 2: Text Search (ripgrep).** Fast, language-agnostic text search across files. Ripgrep combines parallel directory traversal, memory-mapped I/O, and SIMD-accelerated regex matching to search large codebases in milliseconds. It finds exact strings, regex patterns, and text occurrences — but cannot distinguish definitions from references or code from comments.

**Layer 3: Structural Analysis (tree-sitter).** Parses source code into concrete syntax trees that capture the full grammatical structure. Tree-sitter queries let you match patterns like "all public functions returning Result" or "every struct that derives Serialize." The incremental parser handles edits in microseconds, and error recovery produces useful trees even for broken code.

**Layer 4: Semantic Analysis (LSP).** Full language-specific understanding including type inference, cross-file reference resolution, trait dispatch, and macro expansion. LSP provides this through a standard JSON-RPC protocol that works with rust-analyzer, pyright, gopls, and dozens of other language servers.

Each layer is more powerful but more expensive. A well-designed agent starts at Layer 1, moves to Layer 2 for text search, uses Layer 3 for structural questions, and only reaches for Layer 4 when deep semantic understanding is required.

## How the Layers Work Together

Consider a realistic agent task: "Add error handling to all database functions that currently use `.unwrap()`."

Here is how the layers combine to handle this:

```rust
// Step 1: File Discovery — find all Rust files in the project
// Layer 1: glob + ignore crate
// Result: ["src/db.rs", "src/db/queries.rs", "src/db/connection.rs", ...]

// Step 2: Text Search — narrow to files containing ".unwrap()"
// Layer 2: ripgrep with --files-with-matches
// Result: ["src/db/queries.rs", "src/db/connection.rs"]

// Step 3: Structural Analysis — find functions containing unwrap calls
// Layer 3: tree-sitter query for function_item nodes containing
//          call expressions on .unwrap()
// Result: [
//   ("src/db/queries.rs", "fetch_user", line 15),
//   ("src/db/queries.rs", "save_record", line 42),
//   ("src/db/connection.rs", "connect", line 8),
// ]

// Step 4: Semantic Analysis — determine return types to choose
//         the right error handling strategy
// Layer 4: LSP hover on each function to get its resolved return type
// Result: [
//   ("fetch_user" returns "User" — needs to change to "Result<User, DbError>"),
//   ("save_record" returns "()" — needs to change to "Result<(), DbError>"),
//   ("connect" returns "Connection" — needs to change to "Result<Connection, DbError>"),
// ]
```

Each layer reduces the search space for the next. File discovery finds 200 Rust files. Ripgrep narrows to 2 files containing `.unwrap()`. Tree-sitter identifies 3 specific functions. LSP provides the type information needed to generate the correct fix. Without this pipeline, the agent would need to read all 200 files in their entirety — consuming the context window and slowing everything down.

::: python Coming from Python
The layered approach mirrors what experienced Python developers do manually: `find . -name "*.py"` to locate files, `grep` to find candidates, `ast.parse()` to understand structure, and `pyright` or `mypy` for type analysis. The difference is that in a Rust agent, all four layers are programmatically accessible and can be composed into automated pipelines.
:::

## Key Concepts Reviewed

**Tree-sitter fundamentals.** Tree-sitter is an incremental GLR parser that produces concrete syntax trees. It handles error recovery gracefully, works across 150+ languages with a uniform API, and supports a declarative query language for pattern matching. The Rust bindings (`tree-sitter` crate) provide `Parser`, `Tree`, `Node`, `TreeCursor`, `Query`, and `QueryCursor` as the core types.

**Parsing and navigation.** `Parser::parse()` takes source bytes and returns a `Tree`. Navigate with `Node::child()`, `Node::child_by_field_name()`, or `TreeCursor` for efficient depth-first traversal. Nodes expose `kind()` for the type name, byte ranges for text extraction, and positions for line/column information.

**Query language.** S-expression patterns match tree structures: `(function_item name: (identifier) @name)`. Predicates like `#eq?` and `#match?` add text constraints. Multi-pattern queries extract multiple symbol types in one pass. Scoped queries run against subtrees for targeted extraction.

**Semantic extraction.** Type annotations, function signatures, and scope chains can be extracted from syntax trees. This provides useful semantic signal without a full type checker. The gap — inferred types, trait resolution, macro expansion — requires LSP.

**Grammar ecosystem.** Grammars are JavaScript DSL files compiled to C parsers. The ecosystem covers all major languages with varying quality. A multi-language parser maps file extensions to grammars, with content detection as fallback.

**Ripgrep.** High-performance text search using parallel traversal, gitignore filtering, mmap, and SIMD regex. Use as a subprocess for simplicity or via the `grep` crate family for in-process search. Combine with tree-sitter for structural validation of text matches.

**Glob patterns and file discovery.** The `ignore` crate provides gitignore-aware directory walking with parallel support. Glob syntax (`*`, `**`, `?`, `[...]`, `{a,b}`) selects files by name pattern. Always cap results and sort for deterministic agent behavior.

**Code navigation.** File outlines, go-to-definition, and find-references built on tree-sitter queries. A navigation index pre-computes symbol locations for fast repeated queries. These features are heuristic but cover common cases accurately.

**Symbol resolution.** Import statement extraction, module path resolution, and cross-file symbol indexing. Each language has its own resolution rules; heuristic approaches handle 90% of cases.

**LSP.** JSON-RPC protocol providing full semantic analysis. Core requests include definition, references, hover, and completion. More powerful than tree-sitter but heavier — use on demand for deep analysis, not as a default for every file.

::: wild In the Wild
The trend in production agents is toward richer code intelligence. Early agents like the first versions of GitHub Copilot relied almost entirely on the LLM's ability to understand code from context. Newer agents increasingly pre-process code before sending it to the model — extracting outlines, resolving imports, and providing type information as structured context. This reduces hallucination, improves accuracy on refactoring tasks, and makes the agent more token-efficient. The tools in this chapter are the building blocks of that pre-processing pipeline.
:::

## Looking Ahead: Chapter 12

Chapter 12 covers version control integration — working with Git repositories, understanding diffs, staging changes, and managing branches. The code intelligence skills from this chapter directly feed into version control features:

- **File discovery** identifies which files are tracked, modified, or untracked in the repository.
- **Structural analysis** helps the agent understand what changed in a diff — not just which lines were modified, but which functions, structs, or modules were affected.
- **Symbol resolution** connects changes across files — when a function signature changes in one file, the agent can find and update all callers.
- **Code navigation** powers intelligent commit message generation — the agent can describe changes in terms of their semantic impact rather than raw line counts.

The code intelligence stack you built here becomes the foundation for making the agent a thoughtful collaborator on version-controlled codebases.

## Exercises

### Exercise 1: Grep vs. AST-Based Search Trade-offs (Easy)

A user asks the agent to "find all functions that accept a `Config` parameter." Compare how you would solve this with ripgrep alone versus tree-sitter queries. For each approach, describe the query you would write, the false positives or negatives you would expect, and the performance characteristics. Under what circumstances is the simpler ripgrep approach good enough, and when does the tree-sitter approach become necessary?

### Exercise 2: Write a Tree-sitter Query for Error Handling Patterns (Medium)

Design a tree-sitter S-expression query that matches Rust functions containing `unwrap()` calls that are not inside test modules. Think through the node types involved: `function_item`, `call_expression`, `method_call`, `identifier`, and `mod_item` with `#[cfg(test)]`. Write out the query pattern and explain which parts are straightforward to match structurally and which would require post-processing in Rust code. Consider how your approach would differ for Python's bare `except:` clauses.

### Exercise 3: Code Navigation Strategy Design (Medium)

You are building a "go to definition" feature for an agent that must work across five languages (Rust, Python, TypeScript, Go, Java) without requiring language servers to be installed. Design a heuristic strategy using only tree-sitter and ripgrep. For each language, identify the definition patterns you would match (function declarations, class definitions, type definitions) and the resolution rules for imports. Where would your heuristic approach fail compared to LSP, and how would you communicate those limitations to the model?

### Exercise 4: Symbol Resolution Across a Monorepo (Hard)

Consider a monorepo with 50 packages across Rust, TypeScript, and Python. A user asks the agent to "rename the `process_event` function everywhere it is used." Design a multi-layer resolution strategy that identifies: (a) the canonical definition, (b) all direct call sites, (c) re-exports and aliases, and (d) dynamic references (strings containing the function name in configs or tests). For each category, specify which code intelligence layer (glob, ripgrep, tree-sitter, LSP) you would use and why. Discuss the trade-off between completeness (finding every reference) and precision (avoiding false renames).

## Key Takeaways

- The code intelligence stack has four layers — file discovery (glob/ignore), text search (ripgrep), structural analysis (tree-sitter), and semantic analysis (LSP) — each more powerful but more expensive than the last
- A well-designed agent pipelines these layers: glob to find files, ripgrep to narrow candidates, tree-sitter for structural precision, LSP for deep semantic questions
- Tree-sitter is the workhorse: instant startup, error-tolerant parsing, language-agnostic API, and a declarative query language make it the default tool for most code intelligence tasks
- LSP fills the gaps that tree-sitter cannot: type inference, cross-file resolution, trait dispatch, and macro expansion — use it on demand, not as a default
- These code intelligence capabilities directly enable the version control features in the next chapter, where understanding code structure improves diff analysis, commit generation, and cross-file change tracking
