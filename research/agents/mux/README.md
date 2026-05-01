---
title: Mux
status: complete
---

# Mux

> Coder Technologies' desktop & browser app for running multiple coding
> agents in parallel, each in its own isolated workspace. Open-source
> (AGPL-3.0). **#12 on Terminal-Bench 2.0** with GPT-5.3-Codex (74.6%).

## Overview

Mux (`coder/mux`) is a UI for managing many concurrent agent sessions,
not a new model harness. Each "workspace" pairs a coding agent loop with an
isolated runtime — local directory, git worktree, or remote SSH host — so
several agents can chip at the same repo without stomping on each other.

Key characteristics:

- **Parallel-first UX** — sidebar of workspaces, central git divergence view
- **Three runtimes**: local, git worktree, SSH
- **Multi-model**: `sonnet-4-*`, `grok-*`, `gpt-5-*`, `opus-4-*`,
  Ollama (local), OpenRouter (long tail)
- **VS Code extension** for jumping into a workspace from your IDE
- **Custom agent loop** "but much of the core UX is inspired by Claude Code"
  — Plan/Exec mode, vim inputs, `/compact`
- **Opportunistic compaction** as a Mux-original
- **Mode prompts** scoped via `AGENTS.md` headings (`Model:`, `Tool:`)

## File Index

| File | Description |
|---|---|
| [architecture.md](architecture.md) | Workspaces, runtimes, instruction layering |
| [agentic-loop.md](agentic-loop.md) | Plan/Exec mode, opportunistic compaction |
| [tool-system.md](tool-system.md) | Bash, file edit, propose_plan, todo, web |
| [context-management.md](context-management.md) | `/compact`, opportunistic compaction |
| [unique-patterns.md](unique-patterns.md) | Parallelism, scoped instructions |
| [benchmarks.md](benchmarks.md) | TB2 results across three model rows |
| [references.md](references.md) | Links |

## Benchmark Highlights

| Model | Rank | Score |
|---|---|---|
| GPT-5.3-Codex | **#12** | 74.6% ±2.5 |
| Claude Opus 4.6 | #22 | 66.5% ±2.5 |
| GPT-5.2 | #34 | 60.7% |
| Claude Opus 4.5 | #38 | 58.4% |

---

*Tier 2 analysis — sourced from `coder/mux` README and the public
documentation at <https://mux.coder.com>. Internal loop details are not
fully open-source-visible despite the AGPL repo being public; the agent
loop module structure and tool list are inferred from docs.*
