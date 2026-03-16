---
title: Summary
description: Review the complete search and code intelligence toolkit and reflect on how these capabilities transform agent effectiveness.
---

# Summary

> **What you'll learn:**
> - How grep, glob, and tree-sitter tools work together to give the agent comprehensive code navigation
> - Which search strategies are most effective for different coding tasks the agent encounters
> - What future enhancements like LSP integration and embedding-based search could add to the toolkit

You started this chapter with an agent that could read and write files but had no way to *find* anything. Now your agent has a complete search and code intelligence toolkit: grep for content search, glob for file discovery, regex for flexible pattern matching, tree-sitter for structural code understanding, and a ranking system that presents the most relevant results first. Let's review what you built and how the pieces fit together.

## The Search Tool Stack

Here is the complete stack of search capabilities, from lowest level (most general) to highest level (most specific):

### Layer 1: File Discovery (Glob)

The glob tool answers the fundamental question: "what files exist?" It walks the directory tree, matches file paths against patterns, and returns results sorted by modification time. The file filter layer ensures it respects `.gitignore` rules, skips binary files, and honors custom `.agentignore` patterns.

**When the agent uses it:** At the start of a task to understand project structure. When looking for configuration files, test files, or files of a specific type. When the agent does not know which files to search.

### Layer 2: Content Search (Grep)

The grep tool answers: "which files contain this text?" It compiles regex patterns, walks the directory tree (reusing the same file filter), and returns matching lines with surrounding context. The regex crate's linear-time guarantee means it is safe for LLM-generated patterns.

**When the agent uses it:** To find function calls, error messages, import statements, and any text pattern. This is the agent's workhorse search tool -- it gets used more than any other.

### Layer 3: Structural Analysis (Tree-sitter)

Tree-sitter parsing answers: "what is the structure of this code?" It builds concrete syntax trees that represent every construct in the source. The AST navigation and query systems extract function definitions, struct declarations, and impl blocks with their exact positions.

**When the agent uses it:** To find definitions (not just usages), list methods on a type, understand the structure of a file, and provide context for search results. It adds precision that text search cannot achieve.

### Layer 4: Semantic Search (Symbol Index)

The symbol index answers: "where is this symbol defined?" It maps names to definition locations across the entire project, with type information and parent context. Prefix matching enables fuzzy lookup.

**When the agent uses it:** When it needs to understand a type, find the definition of a function, or list all methods available on a struct. This is faster than grep for definition lookups because the index is pre-built.

### Layer 5: Code-Aware Search

Code-aware search answers: "where does this pattern appear in a specific code context?" By combining regex with AST node type checking, it restricts searches to function bodies, comments, string literals, or any other syntactic context.

**When the agent uses it:** To find TODO comments (not mentions of "TODO" in variable names), locate hardcoded strings, or search within a specific function's body. It eliminates false positives that text-only search produces.

### Layer 6: Ranking and Presentation

The ranking system transforms raw results into ordered, budgeted output. It scores results on match quality, path relevance, structural importance, recency, and proximity to the user's focus area. Token-aware truncation ensures results fit the context window.

**When the agent uses it:** Every time, invisibly. All search tools feed their results through the ranking system before returning output to the LLM.

## The Search Decision Flow

When the LLM receives a task that requires finding code, it follows this decision flow:

1. **Do I know the file?** If yes, use the read tool directly.
2. **Do I know the file pattern?** If yes, use glob to find matching files.
3. **Do I know the symbol name?** If yes, use symbol_search for the definition.
4. **Do I know a text pattern?** If yes, use grep to find content matches.
5. **Do I need structural context?** If yes, use code-aware search to filter by AST context.

Good tool descriptions guide the LLM through this flow. The cross-references in each tool's description ("for file names, use glob instead") act as decision edges.

::: python Coming from Python
The search pipeline maps to familiar Python tools:
```python
# Layer 1: File discovery
from pathlib import Path
list(Path(".").rglob("*.py"))

# Layer 2: Content search
import subprocess
subprocess.run(["grep", "-rn", "pattern", "."])

# Layer 3: Structural analysis
import ast
tree = ast.parse(source)

# Layer 4: Semantic search
import jedi
script = jedi.Script(source, path="main.py")
script.goto(line=10, column=5)

# Layer 5: Code-aware search
# Python's ast module + grep -- no built-in equivalent

# Layer 6: Ranking
results.sort(key=lambda r: r.score, reverse=True)
```
The Rust implementation is more unified -- all layers share the same file filter, use the same result types, and feed through the same ranking system.
:::

## What You Built in This Chapter

Let's take inventory of the concrete components:

| Component | Crate(s) | Purpose |
|-----------|----------|---------|
| `GrepTool` | `regex`, `walkdir` | Content search with regex, context lines |
| `GlobTool` | `globset`, `walkdir` | File discovery by name pattern |
| `FileFilter` | `ignore` | Unified gitignore, binary, and custom filtering |
| `RegexCache` | `regex` | Compiled pattern caching for repeated searches |
| `QueryEngine` | `tree-sitter` | Reusable tree-sitter parser and query runner |
| `SymbolIndex` | `tree-sitter` | Project-wide symbol definition index |
| `Language` enum | `tree-sitter-*` | Language detection and grammar selection |
| Ranking system | (built-in) | Multi-signal scoring and token-aware truncation |

These components total around 1,000 lines of Rust code and give your agent search capabilities comparable to a modern code editor.

## Performance Considerations

Search tools interact with the filesystem and potentially parse thousands of files. Here are the key performance insights from this chapter:

**File filtering dominates performance.** The difference between searching 50 files (with `.gitignore` filtering) and 10,000 files (without) is a 200x speedup. Always use the `ignore` crate.

**Regex compilation is not free.** Cache compiled patterns when the same pattern is used multiple times. The `RegexCache` with `Mutex<HashMap>` handles this.

**Tree-sitter parsing is fast.** Parsing a typical source file takes microseconds. Indexing a 100-file project takes under a second. Incremental re-parsing after edits is even faster.

**Ranking is cheap.** Scoring and sorting results is negligible compared to the I/O cost of reading files. Do not skip ranking to "save time."

## Future Enhancements

The search toolkit you built covers the fundamental use cases, but there are several directions you could extend it:

**LSP Integration.** The Language Server Protocol provides precise go-to-definition, find-references, and rename capabilities for any language with an LSP server. Wrapping LSP calls in a tool would give the agent IDE-quality navigation. The trade-off is that LSP servers are external processes that need to be installed and configured.

**Embedding-Based Semantic Search.** Instead of matching symbol names, you could embed code snippets as vectors and search by semantic similarity. This would let the agent find "functions that handle authentication" without knowing the exact names. This requires an embedding model and a vector store, which adds complexity.

**Incremental Index Updates.** The current symbol index is built once at startup. Watching for file changes (using `notify` crate) and incrementally updating the index would keep it current as the agent makes edits.

**Cross-File Reference Resolution.** The current symbol index finds definitions but does not track references (call sites, type usages). Adding reference tracking would enable "find all usages" queries.

::: wild In the Wild
Production coding agents are continuously expanding their search capabilities. Claude Code recently added support for tree-sitter-based code understanding, complementing its existing grep and glob tools. Cursor and Continue integrate with LSP servers for precise navigation. The trend is clear: more structural understanding leads to better agent performance, because the agent spends less time searching and more time solving the actual problem.
:::

## Exercises

1. **(Easy)** Add a `--type` parameter to the grep tool that accepts language names (e.g., "rust", "python") and automatically sets the include glob pattern. For example, `type: "rust"` should set `include: "*.rs"`.

2. **(Medium)** Implement a "file outline" tool that takes a file path and returns a structured summary of its contents: function signatures, struct definitions, and impl blocks. Use tree-sitter to extract the outline and format it compactly.

3. **(Hard)** Build an incremental symbol index that watches for file changes using the `notify` crate. When a file is saved, re-parse only that file and update the index. Verify that the index stays consistent when files are added, deleted, and renamed.

4. **(Hard)** Add cross-file reference tracking to the symbol index. For each symbol definition, track where it is imported and used across the project. This enables a "find all usages" query that the agent can use to understand the impact of a change before making it.

## Key Takeaways

- The complete search toolkit has six layers: file discovery (glob), content search (grep), structural analysis (tree-sitter), semantic search (symbol index), code-aware search (AST + regex), and ranking/presentation.
- File filtering with the `ignore` crate is the single biggest performance optimization -- it reduces the number of files searched by 100x or more in typical projects.
- Tool descriptions and cross-references guide the LLM to select the right search tool for each task, preventing wasted tool calls and improving agent efficiency.
- Tree-sitter's incremental parsing and error recovery make it robust enough for use in a coding agent, where files are frequently in-progress and changing.
- The search toolkit you built in this chapter is the foundation for everything that follows: git integration, code refactoring, and production-quality agent features all depend on the ability to find and understand code.
