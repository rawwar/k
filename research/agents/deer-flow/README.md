---
title: DeerFlow Architecture Analysis
status: complete
category: super-agent-harness
---

# DeerFlow

> An open-source **super agent harness** by ByteDance that orchestrates sub-agents, memory, and sandboxes to do almost anything — powered by extensible skills built on LangGraph.

## Overview

DeerFlow (**D**eep **E**xploration and **E**fficient **R**esearch **Flow**) is an open-source project by ByteDance released under the MIT license. It started as a deep-research framework (v1) and was rewritten from scratch as a fully-fledged **super agent harness** in v2 (launched February 2026).

Unlike the CLI coding agents in this research corpus, DeerFlow is not a coding-first tool — it is a **general-purpose runtime** that gives AI agents the infrastructure to get complex, long-horizon work done: research, report generation, slide creation, data pipelines, dashboards, and code tasks. The v2 rewrite was triggered by community usage patterns: developers were pushing v1 far beyond deep research, and it became clear the system needed to become a proper harness rather than a narrow research framework.

DeerFlow is built on **LangGraph + LangChain**, runs on **Python (backend) + Next.js (frontend)**, and ships batteries-included: filesystem, memory, skills, sandboxed execution, and sub-agent spawning are all built in.

**Key characteristics:**

- **Super agent harness** — not a framework you wire together; a batteries-included runtime
- **Skills-as-Markdown** — a "skill" is a `.md` file defining a workflow, loaded on-demand
- **Progressive skill loading** — only loads what the task needs; keeps context window lean
- **Sandboxed execution** — each task runs inside an isolated Docker container
- **Dynamic sub-agents** — lead agent spawns sub-agents on the fly; parallel when possible
- **Long-term memory** — persistent across sessions; per-user profile, preferences, knowledge
- **IM channel dispatch** — receive tasks from Telegram, Slack, Feishu/Lark
- **MCP server support** — extensible tool ecosystem via Model Context Protocol

## Repository

- **URL**: https://github.com/bytedance/deer-flow
- **Language**: Python (backend), TypeScript/Next.js (frontend)
- **License**: MIT
- **Version**: 2.0 (ground-up rewrite; v1 was deep-research only)
- **Stars**: ~20 K+ (reached #1 GitHub Trending on Feb 28, 2026 at v2 launch)
- **Organization**: ByteDance
- **Website**: https://deerflow.tech

## Architecture Summary

DeerFlow is split into three runtime layers with a unified proxy front:

```
┌─────────────────────────────────────────────────────┐
│  Client (Next.js, port 3000) / IM Channels           │
│  Telegram · Slack · Feishu (no public IP needed)     │
└───────────────────┬─────────────────────────────────┘
                    │ Unified proxy (port 2026)
          ┌─────────┴──────────┐
          │                    │
┌─────────▼──────┐   ┌─────────▼──────────────────────┐
│  Gateway API   │   │  LangGraph Server (port 2024)   │
│  (port 8001)   │   │  Lead Agent (graph-based)        │
│  Skill install │   │  Sub-agent spawning              │
│  Follow-up gen │   │  Session management              │
│  IM dispatch   │   │  Streaming responses             │
└────────────────┘   └──────────────┬─────────────────┘
                                    │
                     ┌──────────────▼──────────────┐
                     │  Sandbox (Docker container) │
                     │  /mnt/skills/public         │
                     │  /mnt/skills/custom         │
                     │  /mnt/user-data/uploads     │
                     │  /mnt/user-data/workspace   │
                     │  /mnt/user-data/outputs     │
                     └─────────────────────────────┘
```

## Execution Modes

DeerFlow exposes four execution modes that balance speed vs. capability:

| Mode | Description | Sub-Agents | Use When |
|------|-------------|------------|----------|
| **flash** | Fast, minimal reasoning | No | Quick queries, simple lookups |
| **standard** | Normal agentic loop | No | Most tasks |
| **pro** | Includes planning phase | No | Complex multi-step tasks |
| **ultra** | Full sub-agent fan-out | Yes | Long-horizon, parallel research |

## Key Components

### Skills System
Skills are structured Markdown files (`.md`) that define a workflow, best practices, and references to supporting resources. DeerFlow ships built-in skills for research, report generation, slide creation, web pages, and image/video generation. Custom skills drop into `/mnt/skills/custom`. Skills load **progressively** — only when the task needs them — keeping the context window lean.

### Sub-Agent Orchestration
The lead agent can spawn sub-agents dynamically. Each sub-agent gets its own scoped context, tools, and termination conditions. Sub-agents run in parallel when possible and return structured results. The lead synthesizes all results into a coherent output. See [agentic-loop.md](agentic-loop.md).

### Sandbox & Filesystem
Every task runs inside an isolated Docker container with a full filesystem. The agent reads, writes, and edits files; executes bash commands; and views images. All state is auditable. Zero contamination between sessions. See [tool-system.md](tool-system.md).

### Long-Term Memory
Persistent cross-session memory: user profile, preferences, writing style, technical stack, recurring workflows. Stored locally; user-controlled.

### MCP Server Support
Tools are extensible via MCP servers (HTTP/SSE with OAuth). Swap or add tools without modifying the core harness.

## Interesting Patterns

1. **Skills-as-Markdown** — a capability module is a `.md` file, not code. Declarative workflow specification that the LLM reads directly.
2. **Progressive skill loading** — skills load on demand, not all at once. Keeps context lean for token-sensitive models.
3. **LangGraph native** — orchestration is a typed state graph with checkpointing, time-travel debugging, and durable execution.
4. **Super agent harness concept** — the distinction between "framework you wire together" vs. "runtime with everything built in."
5. **Claude Code integration** — a skill (`claude-to-deerflow`) lets Claude Code send tasks to DeerFlow; bridges CLI agents with the harness.
6. **IM channel dispatch without public IP** — uses long-polling (Telegram), Socket Mode (Slack), WebSocket (Feishu). No ngrok required.
7. **v1→v2 ground-up rewrite** — v1 (deep research only) was extended by the community far beyond its intended scope, prompting a full redesign.

## Benchmark / Reception

DeerFlow is not measured on traditional coding benchmarks (SWE-bench, Terminal-Bench). Its community traction is the primary signal:

- **#1 GitHub Trending** globally on launch day of v2 (Feb 28, 2026)
- Trendshift #1 spot for repositories
- Active community of contributors extending v1 far beyond deep research (the catalyst for v2)

## Research Files

| File | Contents |
|------|----------|
| [README.md](README.md) | This file — overview, architecture, key patterns |
| [architecture.md](architecture.md) | LangGraph stack, Python/Node.js split, sandbox modes, IM channels |
| [agentic-loop.md](agentic-loop.md) | Lead agent loop, sub-agent spawning, parallel execution, result synthesis |
| [tool-system.md](tool-system.md) | Skills-as-Markdown, progressive loading, MCP, core tools, skill archives |
| [context-management.md](context-management.md) | Sub-agent context isolation, summarization, filesystem offload |
| [unique-patterns.md](unique-patterns.md) | Super agent harness concept, skills, v1→v2, Claude Code bridge, InfoQuest |
| [benchmarks.md](benchmarks.md) | Community reception, GitHub trending, star trajectory |
| [references.md](references.md) | All links — GitHub, docs, website, guides |

## References

- [GitHub Repository](https://github.com/bytedance/deer-flow)
- [Official Website](https://deerflow.tech)
- [Architecture Deep Dive](architecture.md)
- [Agent Loop Analysis](agentic-loop.md)
- [Tool System / Skills](tool-system.md)
- [Context Management](context-management.md)
- [Unique Patterns](unique-patterns.md)
- [All References](references.md)
