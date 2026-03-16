---
title: "Chapter 10: Search and Code Intelligence"
description: Implementing grep, glob, regex, and tree-sitter powered tools that give your agent the ability to understand and navigate codebases.
---

# Search and Code Intelligence

A coding agent that cannot search and understand code is like a developer without an IDE. In previous chapters you built tools that read, write, and edit files -- but those tools require the agent to already know *which* file to open and *where* the relevant code lives. This chapter builds the search and code intelligence tools that transform your agent from a general-purpose assistant into a true coding partner. You will implement grep, glob, and regex-powered search tools, then go deeper with tree-sitter for AST-level code understanding.

Starting with the fundamentals, you will build a grep tool that searches file contents with regex support and a glob tool that finds files by name pattern. You will learn how to write correct and performant regular expressions in Rust, and how to match file patterns efficiently across large directory trees. These tools give your agent the ability to find relevant code quickly -- the same way a human developer uses `Ctrl+Shift+F` in their editor or `rg` from the terminal.

The second half of the chapter introduces tree-sitter, a powerful incremental parsing library that builds concrete syntax trees for source code. You will use tree-sitter to implement AST navigation, semantic search that understands code structure, and language-aware search that can find function definitions, type declarations, and call sites. By the end, your agent will have a suite of search tools that rival what a human developer gets from their editor.

## Learning Objectives
- Build a grep tool with regex support, file filtering, and context line display
- Implement a glob tool for fast file pattern matching across directory trees
- Write correct and performant regular expressions using the `regex` crate
- Combine gitignore rules, glob patterns, and binary detection into a unified file filter
- Integrate tree-sitter for language-aware parsing and AST navigation
- Implement semantic search that understands code structure beyond text matching
- Detect programming languages and select the correct parser automatically
- Rank and present search results effectively within context window constraints
- Register all search tools with the agent's tool system using descriptive JSON schemas

## Subchapters
1. [Grep Tool](/project/10-search-and-code-intelligence/01-grep-tool)
2. [Glob Tool](/project/10-search-and-code-intelligence/02-glob-tool)
3. [Regex In Rust](/project/10-search-and-code-intelligence/03-regex-in-rust)
4. [File Pattern Matching](/project/10-search-and-code-intelligence/04-file-pattern-matching)
5. [Tree Sitter Intro](/project/10-search-and-code-intelligence/05-tree-sitter-intro)
6. [AST Navigation](/project/10-search-and-code-intelligence/06-ast-navigation)
7. [Semantic Search](/project/10-search-and-code-intelligence/07-semantic-search)
8. [Code Aware Search](/project/10-search-and-code-intelligence/08-code-aware-search)
9. [Language Detection](/project/10-search-and-code-intelligence/09-language-detection)
10. [Search Result Ranking](/project/10-search-and-code-intelligence/10-search-result-ranking)
11. [Integrating Search Tools](/project/10-search-and-code-intelligence/11-integrating-search-tools)
12. [Summary](/project/10-search-and-code-intelligence/12-summary)

## Prerequisites
- Chapter 4: Tool system architecture and the `Tool` trait
- Chapter 5: File operation tools and path handling patterns
- Chapter 9: Context management (understanding token budgets for result truncation)
- Familiarity with regular expressions at a basic level (helpful but not required)
