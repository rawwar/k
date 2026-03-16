---
title: "Chapter 6: File System Operations"
description: Implementing safe, reliable file reading, writing, and editing tools — the most fundamental operations a coding agent performs.
---

# File System Operations

Reading and writing files is the most fundamental thing a coding agent does. Every other capability -- code generation, refactoring, bug fixing, test writing -- ultimately reduces to reading source files, reasoning about them, and writing changes back to disk. Getting file operations right means getting them safe, fast, and resilient to the many edge cases that arise in real-world codebases.

This chapter covers the full spectrum of file system operations. We start with reading strategies that handle everything from small configuration files to massive log files. We then tackle writing with an emphasis on safety: atomic operations that prevent data corruption, backup strategies, and encoding-aware output. The core of the chapter addresses the editing problem -- how to modify existing files precisely using string replacement, patch-based approaches, and diff algorithms.

By the end of this chapter, you will have implemented a complete set of file operation tools for your agent. You will understand the trade-offs between different editing strategies, know how to handle large files without consuming excessive memory, and have cross-platform path handling that works on macOS, Linux, and Windows.

## Learning Objectives
- Implement file reading tools that handle encoding detection, line-range selection, and large files efficiently
- Build safe file writing operations using atomic writes and temporary files to prevent data corruption
- Design and implement a string-replacement-based editing tool that works reliably with LLM-generated inputs
- Understand diff algorithms and when to use patch-based editing versus string replacement
- Handle cross-platform path differences and Unicode edge cases in file content
- Implement file watching for live-reload development workflows

## Subchapters
1. [File Reading Strategies](/linear/06-file-system-operations/01-file-reading-strategies)
2. [Writing Safely](/linear/06-file-system-operations/02-writing-safely)
3. [Editing Approaches](/linear/06-file-system-operations/03-editing-approaches)
4. [String Replace vs Patch](/linear/06-file-system-operations/04-string-replace-vs-patch)
5. [Diff Algorithms](/linear/06-file-system-operations/05-diff-algorithms)
6. [Atomic Operations](/linear/06-file-system-operations/06-atomic-operations)
7. [Large File Handling](/linear/06-file-system-operations/07-large-file-handling)
8. [Encoding and Unicode](/linear/06-file-system-operations/08-encoding-and-unicode)
9. [File Watching](/linear/06-file-system-operations/09-file-watching)
10. [Temporary Files](/linear/06-file-system-operations/10-temporary-files)
11. [Cross Platform Paths](/linear/06-file-system-operations/11-cross-platform-paths)
12. [Summary](/linear/06-file-system-operations/12-summary)

## Prerequisites
- Chapter 5 (tool system concepts)
