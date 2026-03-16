---
title: "Chapter 5: File Operations Tools"
description: Implement the read, write, and edit tools that give your coding agent the ability to work with files on disk.
---

# File Operations Tools

This chapter builds the three most important tools a coding agent can have: the ability to read files, write new files, and edit existing files. These are the tools that transform your agent from a conversationalist into something that can actually modify code. Every real-world coding agent -- Claude Code, Cursor, Aider -- has some version of these three tools at its core.

You will implement each tool as a struct that satisfies the Tool trait from Chapter 4, with proper JSON schemas, input validation, and error handling. But the engineering challenges go beyond just reading and writing bytes. You need to handle path resolution and safety checks to prevent the agent from escaping its allowed directories. You need atomic writes so a crash mid-write does not corrupt a file. You need the edit tool to perform precise string replacements without accidentally modifying the wrong section of a file.

By the end of this chapter your agent can read any file in the project, create new files, and make targeted edits to existing code. Combined with the agentic loop from Chapter 3, this means the model can now read a file, reason about what to change, make the edit, read the file again to verify, and iterate -- exactly the workflow a human developer follows.

## Learning Objectives
- Implement a ReadFile tool that returns file contents with line numbers
- Implement a WriteFile tool with atomic writes and backup support
- Implement an EditFile tool that performs exact string replacement
- Handle path resolution, canonicalization, and directory traversal prevention
- Test file tools thoroughly using temporary directories and fixtures
- Understand the security and safety considerations of giving an agent file system access

## Subchapters
1. [Read Tool](/project/05-file-operations-tools/01-read-tool)
2. [Write Tool](/project/05-file-operations-tools/02-write-tool)
3. [Edit Tool String Replace](/project/05-file-operations-tools/03-edit-tool-string-replace)
4. [Path Handling](/project/05-file-operations-tools/04-path-handling)
5. [Safety Checks](/project/05-file-operations-tools/05-safety-checks)
6. [File Permissions](/project/05-file-operations-tools/06-file-permissions)
7. [Diffing Strategies](/project/05-file-operations-tools/07-diffing-strategies)
8. [Atomic Writes](/project/05-file-operations-tools/08-atomic-writes)
9. [Glob Patterns](/project/05-file-operations-tools/09-glob-patterns)
10. [Handling Large Files](/project/05-file-operations-tools/10-handling-large-files)
11. [Testing File Tools](/project/05-file-operations-tools/11-testing-file-tools)
12. [Summary](/project/05-file-operations-tools/12-summary)

## Prerequisites
- Chapter 4 completed (working tool system with trait, registry, and dispatch)
- Understanding of Rust's std::fs module and path handling
