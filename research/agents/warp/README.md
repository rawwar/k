---
title: Warp — AI-Native Terminal with Integrated Coding Agent
status: complete
---

# Warp

> The terminal reimagined from scratch in Rust: a GPU-accelerated, block-based terminal
> emulator with a deeply integrated AI agent platform ("Oz") that can attach to live
> interactive processes, execute in the cloud, and unify local and remote agent workflows.

## Overview

**Warp** is an AI-native terminal application built entirely in **Rust** with **Metal**
GPU rendering. Unlike every other AI coding agent — which wraps an existing terminal
(Claude Code, Codex CLI, aider) — Warp **is** the terminal itself. This architectural
distinction is fundamental: because Warp controls the entire rendering pipeline, PTY
management, and input handling, its agent has capabilities no wrapper-based agent can
match.

Founded by **Zach Lloyd** (former Google Docs engineering lead), Warp launched publicly
in 2022 as a modern terminal replacement and has since evolved into a full agent platform.
The company has raised $170M+ in funding, reflecting the scale of ambition: replace the
50-year-old terminal paradigm with something AI-first.

### What Makes Warp Special

1. **Terminal IS the Agent Environment**: Warp's agent doesn't shell out — it reads the
   live terminal buffer, writes to the PTY, and can attach to interactive processes
   (psql, vim, python REPL, gdb). No other coding agent can "see" a running `npm run dev`
   server's output and interact with it in real time.

2. **GPU-Accelerated Rendering**: Custom Rust → Metal pipeline achieving 400+ fps with
   ~1.9ms average redraw time. ~250 lines of Metal shader code render three primitive
   types (rects, images, glyphs). This isn't cosmetic — it enables the rich block-based
   UI that makes agent interaction feel native.

3. **Block-Based Data Model**: Every command+output pair is a discrete "block" with its
   own grid (forked from Alacritty's grid model). Blocks can be individually selected,
   shared, referenced by the agent, and composed into context. This replaces the
   traditional terminal's single scrollback buffer.

4. **Oz Agent Platform**: Unifies local agents (running in the Warp app) and cloud agents
   (running on Warp infrastructure or self-hosted) under a single orchestration platform.
   Cloud agents can be triggered by Slack, Linear, GitHub events, webhooks, or schedules.

5. **Agent Modality**: Two distinct interaction modes — a clean terminal for command
   execution and a dedicated conversation view for multi-turn agent workflows. The agent
   can seamlessly transition between executing commands and conversational planning.

6. **Full Terminal Use**: The agent can start interactive tools, read their output from the
   terminal buffer, write input to them, and hand control back to the human. This
   "takeover/handback" pattern is unique to terminal-native agents.

7. **Skills & Rules System**: Reusable markdown instruction sets (SKILL.md files) with
   parameter support, plus hierarchical project rules via AGENTS.md files — giving teams
   fine-grained control over agent behavior per directory.

### Key Stats

| Metric | Value |
|--------|-------|
| **Language** | Rust (client), with Metal shaders (macOS) |
| **Rendering** | 400+ fps, ~1.9ms avg redraw, Metal GPU |
| **Open Source** | No (issues-only GitHub repo; plans to open-source UI framework) |
| **Platform** | macOS (primary), Linux, Windows (coming) |
| **Models** | GPT-5.x, Claude Opus/Sonnet 4.x, Gemini 3 Pro, Gemini 2.5 Pro |
| **Auto Modes** | Cost-efficient, Responsive, Genius |
| **Terminal-Bench 2.0** | Rank #31 (61.2%), #36 (59.1%), #52 (50.1%) |
| **Terminal-Bench 1.0** | Rank #11 (52.0%) |
| **Funding** | $170M+ raised |
| **SOC 2** | Compliant; Zero Data Retention with all LLM providers |

## Architecture at a Glance

```
┌───────────────────────────────────────────────────────────────────┐
│                        Warp Terminal App                          │
│                     (Rust + Metal GPU Rendering)                  │
│                                                                   │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────────────────────┐ │
│  │   Terminal    │  │    Agent     │  │     Oz Orchestration    │ │
│  │    View       │  │ Conversation │  │       Platform          │ │
│  │              │  │    View      │  │                         │ │
│  │  ┌────────┐  │  │  Multi-turn  │  │  ┌───────┐ ┌────────┐  │ │
│  │  │ Block  │  │  │  planning &  │  │  │ Local │ │ Cloud  │  │ │
│  │  │ Block  │  │  │  execution   │  │  │ Agent │ │ Agents │  │ │
│  │  │ Block  │  │  │              │  │  │       │ │        │  │ │
│  │  └────────┘  │  └──────┬───────┘  │  └───┬───┘ └───┬────┘  │ │
│  └──────┬───────┘         │          │      │         │        │ │
│         │                 │          └──────┼─────────┼────────┘ │
│         ▼                 ▼                 ▼         ▼          │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    PTY / Shell Hooks                         │ │
│  │         (precmd/preexec → Block boundaries)                 │ │
│  │    ┌──────────┐  ┌──────────┐  ┌──────────────────┐        │ │
│  │    │ Grid per │  │ Grid per │  │ Grid per block   │        │ │
│  │    │ block    │  │ block    │  │ (Alacritty fork)  │        │ │
│  │    └──────────┘  └──────────┘  └──────────────────┘        │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                   │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │              Rust → Element Tree → GPU Primitives           │  │
│  │         (rect, image, glyph) → Metal Shaders (~250 LOC)    │  │
│  └────────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────────┘
         │                                          │
         ▼                                          ▼
┌──────────────────┐                    ┌──────────────────────┐
│   Codebase       │                    │   Cloud / Triggers   │
│   Context        │                    │                      │
│  (Semantic Index, │                    │  Slack, Linear,      │
│   Embeddings)    │                    │  GitHub, Webhooks,   │
│                  │                    │  Schedules           │
└──────────────────┘                    └──────────────────────┘
```

## Comparison with Wrapper-Based Agents

| Aspect | Warp (Terminal-Native) | Wrapper Agents (Claude Code, etc.) |
|--------|------------------------|------------------------------------|
| Terminal control | Full PTY ownership | Spawns subprocesses |
| Interactive apps | Attach, read buffer, write input | Cannot interact with running apps |
| Rendering | Custom GPU pipeline | Inherits host terminal |
| Command context | Block-level isolation | Raw text parsing |
| Output awareness | Reads live terminal buffer | Captures stdout/stderr |
| Cloud execution | Native cloud agent support | Separate infrastructure needed |

## Files in This Research

| File | Contents |
|------|----------|
| [architecture.md](architecture.md) | Rust+Metal GPU rendering, PTY/block system, data model, UI framework, Oz platform |
| [agentic-loop.md](agentic-loop.md) | Oz agent request processing, Full Terminal Use, permissions, planning, takeover/handback |
| [tool-system.md](tool-system.md) | File ops, shell execution, web search, MCP, Computer Use, LSP, code review, voice |
| [context-management.md](context-management.md) | Semantic indexing, multi-modal context, Rules, Skills, Warp Drive, forking/compaction |
| [unique-patterns.md](unique-patterns.md) | Terminal-native patterns, GPU pipeline, block model, Full Terminal Use, cloud+local unification |
| [benchmarks.md](benchmarks.md) | Terminal-Bench 2.0 and 1.0 results, model configurations, performance analysis |
| [references.md](references.md) | Comprehensive URL reference: docs, blog posts, GitHub, benchmarks |
