---
title: Goose Architecture Analysis
status: complete
---

# Goose

> An open-source, on-machine AI coding agent by Block (formerly Square) that automates engineering tasks end-to-end, built on MCP (Model Context Protocol) for extensibility.

## Overview

Goose is a local-first AI agent developed by **Block, Inc.** (formerly Square) and released as open source under the Apache 2.0 license. Unlike cloud-hosted coding assistants, Goose runs entirely on the developer's machine, executing shell commands, editing files, and orchestrating complex workflows autonomously.

The project is written primarily in **Rust** (core agent, server, CLI) with a **TypeScript/Electron** desktop UI. It ships as both a CLI tool and a desktop application, with shared configuration between the two interfaces.

**Key characteristics:**

- **MCP-native architecture**: Extensions are MCP (Model Context Protocol) servers, making Goose a first-class MCP client that can connect to any MCP-compatible tool ecosystem
- **Provider-agnostic**: Supports 30+ LLM providers (Anthropic, OpenAI, Google, Azure, Bedrock, Ollama, and many more)
- **Multi-model**: Supports configuring different models for primary reasoning vs. fast operations
- **Enterprise-oriented**: Built by Block with enterprise needs in mind — permission controls, access restrictions, custom distributions, and CI/CD support
- **Extensible by design**: Plugin system based entirely on MCP, with built-in extensions, platform extensions, external stdio/HTTP extensions, and even inline Python extensions

## Repository

- **URL**: https://github.com/block/goose
- **Language**: Rust (core), TypeScript (desktop UI)
- **License**: Apache 2.0
- **Current version**: 1.28.0 (as of research date)
- **Stars**: ~15k+
- **Organization**: Block, Inc.

## Architecture Summary

Goose is organized as a Rust workspace with 8 crates:

| Crate | Purpose |
|-------|---------|
| `goose` | Core library — agent loop, providers, config, context management, MCP client |
| `goose-cli` | CLI binary (`goose` command) |
| `goose-server` | HTTP/API server (Axum-based) |
| `goose-mcp` | Built-in MCP server implementations |
| `goose-acp` | Agent Communication Protocol support |
| `goose-acp-macros` | Proc macros for ACP |
| `goose-test` | Test utilities |
| `goose-test-support` | Test support infrastructure |

```
┌─────────────────────────────────────────┐
│  UI Layer (Desktop / CLI / Text TUI)    │
└──────────────┬──────────────────────────┘
               │ HTTP API
┌──────────────▼──────────────────────────┐
│  goose-server (Axum)                    │
│  Routes → Session → Agent               │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│  Agent (core loop)                      │
│  • Stream LLM responses                 │
│  • Parse tool calls                     │
│  • Permission & security inspection     │
│  • Dispatch to ExtensionManager         │
│  • Context management & compaction      │
└──────┬──────────────┬───────────────────┘
       │              │
┌──────▼──────┐ ┌─────▼──────────────────┐
│  Provider   │ │  ExtensionManager      │
│  (LLM API)  │ │  (MCP client router)   │
│  30+ impls  │ │  Platform | Builtin |  │
└─────────────┘ │  Stdio | HTTP | Python │
                └────────────────────────┘
```

## Key Components

### Agent Loop
The core agent loop lives in `crates/goose/src/agents/agent.rs` (~97KB). It implements a streaming reply loop: prepare context → call LLM → parse tool calls → inspect permissions → dispatch tools → collect results → loop. See [agentic-loop.md](agentic-loop.md) for details.

### Extension System (MCP)
All tool functionality is provided through extensions, which are MCP servers. Goose supports 7 extension transport types: Platform (in-process), Builtin (DuplexStream MCP), Stdio (child process), StreamableHttp (remote), InlinePython, Frontend, and SSE (deprecated). See [tool-system.md](tool-system.md) for details.

### Provider System
Goose abstracts LLM access behind a `Provider` trait with 30+ implementations. It supports multi-model configuration, prompt caching (for Claude), and a "toolshim" mode for models without native tool-calling. See [architecture.md](architecture.md) for details.

### Context Management
Automatic compaction at 80% of context window, background tool-pair summarization, and emergency compaction on context overflow errors. See [context-management.md](context-management.md) for details.

## Benchmark Performance

### Terminal-Bench 2.0
| Model | Rank | Score |
|-------|------|-------|
| Claude Opus 4.5 | #44 | 54.3% |
| Claude Sonnet 4.5 | #61 | 43.1% |

### Terminal-Bench 1.0
| Model | Rank | Score |
|-------|------|-------|
| claude-opus-4 | #17 | 45.3% |

## Interesting Patterns

1. **MCP as the universal extension interface**: Everything is an MCP server — built-in tools, platform features, external plugins, even inline Python scripts
2. **Multi-layered tool inspection**: Security → Adversary → Permission → Repetition inspection pipeline before any tool executes
3. **MOIM (Model-Oriented Information Management)**: Dynamic context injection from extensions into the conversation each turn
4. **Tool-pair summarization**: Background task that summarizes old tool request/response pairs to save context tokens
5. **Recipe system**: Automated task execution with success criteria, retry logic, and conversation reset
6. **Subagent delegation**: The "Summon" extension can delegate tasks to subagents with isolated contexts
7. **Enterprise permission modes**: Autonomous, Manual Approval, Smart Approval, and Chat-only modes

## References

- [GitHub Repository](https://github.com/block/goose)
- [Documentation](https://block.github.io/goose/)
- [Architecture Deep Dive](architecture.md)
- [Agent Loop Analysis](agentic-loop.md)
- [Tool System](tool-system.md)
- [Context Management](context-management.md)
- [Unique Patterns](unique-patterns.md)
- [Benchmarks](benchmarks.md)
- [All References](references.md)
