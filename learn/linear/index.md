---
title: Linear Tutorial Track
description: Understand the theory behind coding agents, then implement each component
---

# Linear Tutorial Track

A concept-first approach to building CLI coding agents. Each chapter explores a key topic in depth — the theory, the trade-offs, and the implementation patterns — before you write code.

This track is ideal if you want to understand *why* things work the way they do. You'll study real-world agents (Claude Code, OpenCode, Pi), learn the underlying concepts, and then implement each component with a deep understanding.

## Learning Objectives

- Understand the architecture and design patterns of modern coding agents
- Learn Rust from a Python developer's perspective
- Master each component: LLM APIs, tool systems, TUI, streaming, and more
- Study real-world implementations and extract common patterns
- Build a coding agent grounded in solid theoretical understanding

## Chapters

1. [What Is a Coding Agent?](/linear/01-what-is-a-coding-agent/) — The AI coding revolution
2. [Rust for Python Developers](/linear/02-rust-for-python-developers/) — Bridging the language gap
3. [Understanding LLMs](/linear/03-understanding-llms/) — How language models work for agents
4. [Anatomy of an Agentic Loop](/linear/04-anatomy-of-an-agentic-loop/) — The core pattern
5. [Tool Systems Deep Dive](/linear/05-tool-systems-deep-dive/) — How agents interact with the world
6. [File System Operations](/linear/06-file-system-operations/) — Reading, writing, and editing code
7. [Process Management and Shell](/linear/07-process-management-and-shell/) — Running commands safely
8. [Streaming and Realtime](/linear/08-streaming-and-realtime/) — Server-sent events and incremental rendering
9. [Terminal User Interfaces](/linear/09-terminal-user-interfaces/) — Building beautiful TUIs
10. [Conversation State Machines](/linear/10-conversation-state-machines/) — Managing context and history
11. [Code Intelligence](/linear/11-code-intelligence/) — Tree-sitter, grep, and semantic search
12. [Version Control Integration](/linear/12-version-control-integration/) — Git as infrastructure
13. [Safety and Permissions](/linear/13-safety-and-permissions/) — Threat models and guardrails
14. [Provider Abstraction](/linear/14-provider-abstraction/) — Supporting multiple LLM providers
15. [Extensibility Patterns](/linear/15-extensibility-patterns/) — Plugins, MCP, and hooks
16. [Testing Coding Agents](/linear/16-testing-coding-agents/) — How to test non-deterministic systems
17. [Packaging and Distribution](/linear/17-packaging-and-distribution/) — Shipping Rust binaries
18. [Case Study: Building It All](/linear/18-case-study-building-it-all/) — Putting it all together

## Cross-Reference: Project Track

Each linear chapter has a corresponding project track chapter where you implement the same concepts hands-on.

| Linear Chapter | Project Chapter |
|---------------|----------------|
| 1. What Is a Coding Agent? | — (conceptual overview) |
| 2. Rust for Python Developers | 1. Hello, Rust CLI |
| 3. Understanding LLMs | 2. First LLM Call |
| 4. Anatomy of an Agentic Loop | 3. The Agentic Loop |
| 5. Tool Systems Deep Dive | 4. Building a Tool System |
| 6. File System Operations | 5. File Operations Tools |
| 7. Process Management and Shell | 6. Shell Execution |
| 8. Streaming and Realtime | 7. Streaming Responses |
| 9. Terminal User Interfaces | 8. Terminal UI with Ratatui |
| 10. Conversation State Machines | 9. Conversation Context Management |
| 11. Code Intelligence | 10. Search and Code Intelligence |
| 12. Version Control Integration | 11. Git Integration |
| 13. Safety and Permissions | 12. Permission and Safety |
| 14. Provider Abstraction | 13. Multi-Provider Support |
| 15. Extensibility Patterns | 14. Extensibility and Plugins |
| 16. Testing Coding Agents | 15. Production Polish |
| 17. Packaging and Distribution | 15. Production Polish |
| 18. Case Study: Building It All | — (capstone) |

## Prerequisites

- Basic programming experience (Python is perfect)
- Willingness to think before coding
- Interest in understanding how AI tools work under the hood
