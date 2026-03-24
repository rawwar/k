---
title: Capy — Parallel-Native Cloud IDE with Captain/Build Agent Architecture
status: complete
---

# Capy

> Cloud-based AI coding IDE built for parallel agent orchestration; uses a distinctive two-agent split (Captain for planning, Build for execution) to ship entire sprints concurrently.

## Overview

**Capy** is a commercial, cloud-based IDE/platform built by **Lowercase** (Lowercase Labs). Named after capybaras, Capy positions itself as "the IDE for the parallel age" — a platform where developers orchestrate multiple autonomous coding agents from a single dashboard, executing entire sprints in parallel rather than working tasks sequentially.

Unlike terminal-first agents (Claude Code, Aider) or IDE extensions (Cursor, Copilot), Capy is a **standalone cloud IDE** where the agent infrastructure is the product. Every task runs in its own sandboxed Ubuntu VM, uses git worktrees for branch isolation, and can execute concurrently with up to 25 parallel "jams" on the Pro plan.

Capy's most distinctive architectural decision is the **Captain/Build split** — a two-agent architecture where planning and execution are handled by separate agents with hard-enforced capability boundaries. Captain plans but cannot write code; Build executes but cannot ask clarifying questions. This forced separation produces real specs before any code is written, reducing the wasted iterations common in single-agent systems.

### What Makes Capy Special

1. **Captain + Build Split**: Two-agent architecture with hard capability boundaries — planning agent can never write code, execution agent can never ask questions mid-task.

2. **Parallel-Native Design**: Built from the ground up for concurrent execution. Run 25+ coding tasks simultaneously, each in isolated environments.

3. **Task-Based Workflow**: Think in tasks, not tabs. Each task groups its chat, branch, environment, and PR as a single unit.

4. **Model Agnostic**: Supports Claude Opus 4.6, GPT-5.3 Codex, Gemini 3 Pro, Grok 4 Fast, Kimi K2, GLM 4.7, Qwen 3 Coder, and more.

5. **Multiplayer Collaboration**: Tag teammates, resume issues, collaborative by default — designed for teams, not solo use.

## Terminal-Bench Scores

| Benchmark | Model Config | Rank | Score |
|-----------|-------------|------|-------|
| Terminal-Bench 2.0 | Capy + Claude Opus 4.6 | #7 | 75.3% ±2.4 |

## Key Stats

- **Type**: Closed-source, commercial SaaS platform (cloud IDE)
- **Website**: [capy.ai](https://capy.ai)
- **Company**: Lowercase (Lowercase Labs)
- **Pricing**: $20/month Pro (3 seats included), custom Enterprise; free for open source
- **Trusted by**: 50,000+ engineers
- **Security**: SOC 2 Type II certified (March 2026)
- **Execution**: Sandboxed Ubuntu VMs per task, git worktrees for isolation
- **Parallel capacity**: Up to 25 concurrent jams (Pro plan)
- **Model support**: Claude Opus 4.6, GPT-5.3 Codex, Gemini 3 Pro, Grok 4 Fast, Kimi K2, GLM 4.7, Qwen 3 Coder
- **Source code**: Not publicly available (closed-source commercial product)

## Architecture at a Glance

```
User (describes task in Capy IDE)
    │
    ▼
┌─────────────────────────────────────┐
│            Captain Agent            │
│  • Reads codebase, researches       │
│  • Asks clarifying questions        │
│  • Writes exhaustive spec           │
│  • CANNOT write code or run cmds    │
└──────────────┬──────────────────────┘
               │ (spec handoff)
               ▼
┌─────────────────────────────────────┐
│             Build Agent             │
│  • Receives spec + codebase access  │
│  • Full Ubuntu VM with sudo         │
│  • Edits files, runs tests          │
│  • Opens PRs on GitHub              │
│  • CANNOT ask questions mid-task    │
└─────────────────────────────────────┘
```

## Files in This Research

| File | Contents |
|------|----------|
| [architecture.md](architecture.md) | Captain/Build two-agent split, cloud execution, VM sandboxing |
| [agentic-loop.md](agentic-loop.md) | Three-phase handoff: user → Captain → Build, feedback mechanisms |
| [tool-system.md](tool-system.md) | Model-agnostic design, VM execution, GitHub integration |
| [context-management.md](context-management.md) | Spec as context handoff, codebase exploration, Build isolation |
| [unique-patterns.md](unique-patterns.md) | Planning/execution split, parallel-native, task workflow, OSS policy |
| [benchmarks.md](benchmarks.md) | Terminal-Bench 2.0 results and context |
| [references.md](references.md) | Links to capy.ai, blog posts, resources |

## References

- Website: https://capy.ai
- Blog: "Captain vs Build: Why We Split the AI Agent in Two" (Feb 2026)
- Terminal-Bench 2.0: rank #7 (Claude Opus 4.6, 75.3% ±2.4)
