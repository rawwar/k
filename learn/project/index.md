---
title: Project-Based Track
description: Build a CLI coding agent from scratch, one feature at a time
---

# Project-Based Track

Build a fully functional CLI coding agent in Rust, chapter by chapter. Each chapter adds a new capability to your agent — by the end, you'll have built something comparable to tools like Claude Code and OpenCode.

This track is ideal if you learn best by doing. Every concept is introduced in the context of a real feature you're building. You'll start with a simple REPL and finish with a production-ready agent.

## Learning Objectives

- Build a working CLI coding agent from scratch in Rust
- Understand the architecture of modern AI coding assistants
- Implement core capabilities: tool use, streaming, TUI, and more
- Learn Rust idioms through practical, incremental development
- Ship a polished, extensible binary

## Chapters

1. [Hello, Rust CLI](/project/01-hello-rust-cli/) — Your first Rust binary and REPL
2. [First LLM Call](/project/02-first-llm-call/) — Connecting to the Anthropic API
3. [The Agentic Loop](/project/03-the-agentic-loop/) — The core loop that makes it an agent
4. [Building a Tool System](/project/04-building-a-tool-system/) — Extensible tool architecture
5. [File Operations Tools](/project/05-file-operations-tools/) — Read, write, and edit files
6. [Shell Execution](/project/06-shell-execution/) — Running commands safely
7. [Streaming Responses](/project/07-streaming-responses/) — Real-time token streaming
8. [Terminal UI with Ratatui](/project/08-terminal-ui-with-ratatui/) — Beautiful terminal interface
9. [Conversation Context Management](/project/09-conversation-context-management/) — Managing the context window
10. [Search and Code Intelligence](/project/10-search-and-code-intelligence/) — Grep, glob, and tree-sitter
11. [Git Integration](/project/11-git-integration/) — Version control as a safety net
12. [Permission and Safety](/project/12-permission-and-safety/) — Keeping the agent safe
13. [Multi-Provider Support](/project/13-multi-provider-support/) — Anthropic, OpenAI, Ollama
14. [Extensibility and Plugins](/project/14-extensibility-and-plugins/) — MCP, hooks, and plugins
15. [Production Polish](/project/15-production-polish/) — Shipping a real tool

## Cross-Reference: Linear Track

Each project chapter has a corresponding linear track chapter that covers the same concepts from a theory-first perspective.

| Project Chapter | Linear Chapter |
|----------------|---------------|
| 1. Hello, Rust CLI | 2. Rust for Python Developers |
| 2. First LLM Call | 3. Understanding LLMs |
| 3. The Agentic Loop | 4. Anatomy of an Agentic Loop |
| 4. Building a Tool System | 5. Tool Systems Deep Dive |
| 5. File Operations Tools | 6. File System Operations |
| 6. Shell Execution | 7. Process Management and Shell |
| 7. Streaming Responses | 8. Streaming and Realtime |
| 8. Terminal UI with Ratatui | 9. Terminal User Interfaces |
| 9. Conversation Context Management | 10. Conversation State Machines |
| 10. Search and Code Intelligence | 11. Code Intelligence |
| 11. Git Integration | 12. Version Control Integration |
| 12. Permission and Safety | 13. Safety and Permissions |
| 13. Multi-Provider Support | 14. Provider Abstraction |
| 14. Extensibility and Plugins | 15. Extensibility Patterns |
| 15. Production Polish | 16–18. Testing, Packaging, Case Study |

## Prerequisites

- Basic programming experience (Python is perfect)
- A terminal and text editor
- Curiosity about how AI coding tools work
