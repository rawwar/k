---
title: Letta Code
status: complete
---

# Letta Code

> Memory-first coding harness from Letta (formerly MemGPT). Built around
> long-lived agents that persist across sessions and learn from
> experience. **#36 on Terminal-Bench 2.0** with Claude Opus 4.5 (59.1%).

## Overview

Letta Code (`@letta-ai/letta-code`, repo `letta-ai/letta-code`) is the CLI
front-end on top of the [Letta](https://github.com/letta-ai/letta) agent
framework. Where Claude Code, Codex, and Gemini CLI all treat each
invocation as an independent session, Letta Code ties every session to a
**persisted agent** with portable memory.

Key characteristics:

- **Persistent agent** — same agent across sessions; `/clear` only resets
  the thread, not memory
- **Portable across models** — Claude, GPT, Gemini, GLM, Kimi
- **Skills** at `.skills/`, plus **skill learning** (`/skill` to derive a
  skill from the current trajectory)
- **Subagents** as a first-class building block
- **Multi-surface**: CLI, desktop app (macOS / Windows / Linux), mobile,
  Slack / Telegram / Discord channels
- Built on the Letta framework, originally **MemGPT** (Charles Packer et al.)

## File Index

| File | Description |
|---|---|
| [architecture.md](architecture.md) | Letta server, memory blocks, Letta Code CLI |
| [agentic-loop.md](agentic-loop.md) | Stateful loop, subagents |
| [tool-system.md](tool-system.md) | Built-in tools, skills, MCP |
| [context-management.md](context-management.md) | Memory blocks, paged history |
| [unique-patterns.md](unique-patterns.md) | Skill learning, model-portable memory |
| [benchmarks.md](benchmarks.md) | TB2 results across three models |
| [references.md](references.md) | Links |

## Benchmark Highlights

| Model | Rank | Score |
|---|---|---|
| Claude Opus 4.5 | **#36** | 59.1% ±2.4 |
| Gemini 3 Pro | #45 | 56.0% ±3.0 |
| GPT-5.1-Codex | #48 | 53.5% ±2.8 |

---

*Tier 2 analysis — Letta is well-documented in academic and product
contexts; this page focuses on the Letta Code CLI specifically. Some loop
internals are abstracted from the framework docs at docs.letta.com.*
