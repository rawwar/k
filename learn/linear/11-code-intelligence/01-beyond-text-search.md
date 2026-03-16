---
title: Beyond Text Search
description: Why text-based search is insufficient for code understanding and how structural and semantic approaches unlock more powerful agent capabilities.
---

# Beyond Text Search

> **What you'll learn:**
> - The limitations of text search for code tasks: false positives from comments and strings, inability to understand scope, and lack of type awareness
> - How structural search using syntax trees eliminates entire categories of false matches by understanding grammar
> - The spectrum from text search to structural search to semantic analysis and when each level is appropriate

Imagine your coding agent receives a request: "Rename the function `process_data` to `transform_data` across the codebase." The naive approach is straightforward — search for the string `process_data` and replace it everywhere. But within seconds, this falls apart. The string appears in a comment explaining the old API. It appears in a log message: `"calling process_data with {} items"`. It appears as part of a longer name: `process_data_batch`. And crucially, there is a completely different `process_data` function in a test helper module that should not be renamed at all.

Text search treats source code as a flat sequence of characters. Code is not flat — it has structure, scope, meaning. A coding agent that cannot see that structure is working blindfolded. This subchapter explores why text search breaks down and what the alternatives look like.

## Where Text Search Fails

Let's start with a concrete example. Suppose you want to find all definitions of a function called `connect` in a Rust project. A naive grep gives you this:

```rust
// Here's what grep "connect" returns:

// 1. The actual function definition — this is what we want
fn connect(addr: &str) -> Result<TcpStream> { ... }

// 2. A function CALL — not a definition
let stream = connect("127.0.0.1:8080");

// 3. A comment mentioning the concept
// TODO: connect to the database before starting the server

// 4. A string literal
let msg = "Failed to connect to remote host";

// 5. A method on a different type entirely
impl WebSocket {
    fn connect(&self) -> Result<()> { ... }
}

// 6. An import statement
use crate::network::connect;
```

Out of six matches, only two are function definitions (items 1 and 5), and you might only want item 1 if you are looking for the free function. Text search gives you a 33% hit rate at best, and the noise gets worse as codebases grow.

The problems fall into distinct categories:

**False positives from non-code content.** Comments and string literals are not executable code, but text search cannot tell them apart from real identifiers. In a well-commented codebase, comments might account for 30% or more of matches for common terms.

**No understanding of grammatical role.** The string `connect` plays different roles depending on where it appears. It is a function definition in one place, a function call in another, a type method somewhere else, and an import in yet another. Text search sees them all as identical byte sequences.

**No scope awareness.** Two functions named `connect` in different modules are completely separate symbols. Text search merges them into one result set, and any operation on "the `connect` function" becomes ambiguous.

**No structural boundaries.** Text search cannot answer questions like "find functions that take more than three parameters" or "find all match arms in this function." These require understanding the tree structure of the code.

::: python Coming from Python
Python developers often use `ast.parse()` from the standard library to get structural understanding:
```python
import ast

source = open("app.py").read()
tree = ast.parse(source)

# Find all function definitions
for node in ast.walk(tree):
    if isinstance(node, ast.FunctionDef):
        print(f"Function: {node.name} at line {node.lineno}")
```
Rust does not have a built-in multi-language parser. Instead, the ecosystem relies on tree-sitter, which serves the same purpose as `ast.parse()` but works across dozens of languages with a single, uniform API. If you have used Python's `ast` module, the tree-sitter concepts will feel familiar.
:::

## The Structural Search Advantage

Structural search understands the grammar of the source language. Instead of matching character sequences, it matches patterns in the syntax tree. Here is the same "find function definitions named `connect`" task using a tree-sitter query:

```rust
// Tree-sitter query (S-expression syntax)
// This matches ONLY function definitions, never calls, comments, or strings
let query_source = r#"
    (function_item
        name: (identifier) @func_name
        (#eq? @func_name "connect"))
"#;
```

This query says: "Find nodes of type `function_item` that have a child named `name` which is an `identifier` node whose text equals `connect`." It will never match a comment, a string literal, or a function call — those are different node types in the syntax tree. The hit rate goes from 33% to 100%.

Structural search also handles questions that text search simply cannot answer:

```rust
// "Find all functions that return a Result type"
let query_source = r#"
    (function_item
        name: (identifier) @func_name
        return_type: (generic_type
            type: (type_identifier) @return_type
            (#eq? @return_type "Result")))
"#;

// "Find all match expressions with more than 5 arms"
// (This requires post-processing the query results to count children)

// "Find all struct definitions that derive Debug"
let query_source = r#"
    (attribute_item
        (attribute
            (identifier) @attr_name
            (#eq? @attr_name "derive")
            arguments: (token_tree) @derives))
    (struct_item
        name: (type_identifier) @struct_name)
"#;
```

None of these queries are possible with text search alone. You could approximate them with regular expressions, but regexes cannot handle nested structures (matching balanced parentheses is the classic example) and quickly become unmaintainable.

## The Code Intelligence Spectrum

Code understanding exists on a spectrum, and each level builds on the ones below it:

**Level 1: Text search (grep, ripgrep).** Treats code as plain text. Fast, simple, language-agnostic. Good for finding exact strings, log messages, error codes, and configuration values. Fails when you need to understand what the text *means* in context.

**Level 2: Structural search (tree-sitter).** Parses code into a syntax tree and matches patterns against the tree structure. Understands node types (function, class, variable), parent-child relationships, and named fields. Cannot resolve cross-file references or understand types beyond what is written in the source.

**Level 3: Semantic analysis (LSP, type checkers).** Understands the full meaning of code — resolved types, cross-file references, trait implementations, generic instantiations. Requires a language-specific analyzer (rust-analyzer, pyright, typescript language server). Slower to set up but provides the deepest understanding.

A well-designed coding agent uses all three levels, picking the right tool for each task:

| Task | Best Tool | Why |
|------|-----------|-----|
| Find a specific error message | grep | Exact text match, no structure needed |
| List all functions in a file | tree-sitter | Structural query, single-file scope |
| Find all callers of a function | LSP | Requires cross-file reference resolution |
| Search for a TODO comment | grep | Text search in comments is appropriate |
| Extract function signatures | tree-sitter | Structural query on function nodes |
| Find the type of a variable | LSP | Requires type inference |

::: tip In the Wild
Claude Code uses a layered approach to code search. Its Grep tool does fast text-level search using ripgrep under the hood, and it respects `.gitignore` rules to avoid searching generated files and dependencies. For structural understanding, it relies on the LLM's own ability to parse code from the text it reads — but this comes at the cost of context window tokens. Production agents increasingly integrate tree-sitter to get structural information without consuming prompt tokens on raw source code.
:::

## Why This Matters for Agents

The difference between text search and structural understanding compounds with every tool call. An agent that uses text search to find a function definition might return five candidates. The LLM then needs to read all five, figure out which one is the real definition, and spend tokens disambiguating. An agent with structural search returns exactly one result and moves on.

Over the course of a complex task — say, refactoring a module — the agent might perform dozens of searches. If each search returns 3x more noise than signal, the agent wastes context window capacity on irrelevant code, risks making changes to the wrong locations, and takes more iterations to complete the task. Structural code intelligence does not just improve accuracy — it improves token efficiency, reduces latency, and lowers the error rate of the entire agent.

The rest of this chapter teaches you how to build each level of the code intelligence stack. We start with tree-sitter, the workhorse of structural code understanding, then cover high-performance text search with ripgrep, file discovery with glob patterns, and finally the Language Server Protocol for full semantic analysis.

## Key Takeaways

- Text search treats code as flat character sequences and cannot distinguish between function definitions, calls, comments, and string literals — leading to high false-positive rates
- Structural search with tree-sitter matches patterns against the syntax tree, eliminating entire categories of false matches by understanding grammatical roles
- Code intelligence exists on a spectrum: text search (fast, imprecise) to structural search (grammar-aware) to semantic analysis (full type and reference resolution)
- A well-designed coding agent uses all three levels, selecting the appropriate tool for each task to balance speed, accuracy, and resource usage
- Structural code intelligence reduces wasted context window tokens, lowers error rates, and decreases the number of iterations needed to complete complex tasks
