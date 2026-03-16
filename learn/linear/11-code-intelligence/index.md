---
title: "Chapter 11: Code Intelligence"
description: Building semantic code understanding with tree-sitter parsing, AST queries, grep at scale, and language server protocol fundamentals.
---

# Code Intelligence

This chapter moves beyond treating source code as plain text. A coding agent that can only do string matching is fundamentally limited — it cannot distinguish a function definition from a function call, a variable declaration from a reference, or a comment from executable code. Tree-sitter provides the foundation for semantic code understanding by parsing source files into concrete syntax trees that capture the full grammatical structure of the code.

You will learn tree-sitter from the ground up: how its incremental parsing algorithm works, how grammar definitions map source text to typed tree nodes, and how the S-expression query language lets you extract specific patterns like "all function definitions that take more than three parameters." These capabilities transform what an agent can do — instead of grepping for text patterns, it can navigate code by structure, understand scope and nesting, and make changes that respect the syntax of the language.

The chapter also covers the practical tools that complement tree-sitter in a code intelligence stack: high-performance grep with ripgrep for text-level search, glob patterns for file discovery, and the Language Server Protocol (LSP) that provides IDE-level features like go-to-definition, find-references, and diagnostics. Together, these tools give the agent a rich understanding of the codebase it operates on.

## Learning Objectives
- Understand why structural code understanding outperforms text search for agent tasks
- Parse source code with tree-sitter and navigate the resulting concrete syntax tree
- Write tree-sitter queries to extract functions, classes, imports, and other structural patterns
- Implement high-performance code search using ripgrep with glob filtering and regex patterns
- Understand the Language Server Protocol architecture and how it provides semantic code features
- Combine text search, structural queries, and LSP features for comprehensive code navigation

## Subchapters
1. [Beyond Text Search](/linear/11-code-intelligence/01-beyond-text-search)
2. [Tree Sitter Fundamentals](/linear/11-code-intelligence/02-tree-sitter-fundamentals)
3. [Parsing Source Code](/linear/11-code-intelligence/03-parsing-source-code)
4. [AST Queries](/linear/11-code-intelligence/04-ast-queries)
5. [Semantic Understanding](/linear/11-code-intelligence/05-semantic-understanding)
6. [Language Grammars](/linear/11-code-intelligence/06-language-grammars)
7. [Grep at Scale](/linear/11-code-intelligence/07-grep-at-scale)
8. [Glob Patterns](/linear/11-code-intelligence/08-glob-patterns)
9. [Code Navigation](/linear/11-code-intelligence/09-code-navigation)
10. [Symbol Resolution](/linear/11-code-intelligence/10-symbol-resolution)
11. [Language Server Protocol Basics](/linear/11-code-intelligence/11-language-server-protocol-basics)
12. [Summary](/linear/11-code-intelligence/12-summary)

## Prerequisites
- Chapter 6 (file system operations for reading and discovering source files in a codebase)
- Basic familiarity with syntax trees and parsing concepts (helpful but not required)
