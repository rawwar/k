---
title: grok-cli
status: complete
---

# grok-cli

> Community-built terminal coding agent for xAI's Grok API. Bun + OpenTUI,
> sub-agents on by default, X + web search, Telegram remote control.
> **#43 on Terminal-Bench 2.0** with Grok 4.20 Reasoning (57.3%).

## Overview

`grok-cli` (npm: `grok-dev`, repo `superagent-ai/grok-cli`) is an
**open-source, community-built** CLI that wraps the xAI Grok API. The
README is explicit:

> *"This project is community-built, open-source, and not affiliated with,
> endorsed by, or sponsored by xAI Corp."*

The TB2 leaderboard tags the agent's org as **"Vibe Kit"** — the same
team / brand publishing under `superagent-ai`.

Key characteristics:

- **TypeScript + Bun** runtime; **OpenTUI** for the terminal UI
- **Defaults tuned for Grok**: `grok-code-fast-1`,
  `grok-4-1-fast-reasoning`, `grok-4.20-multi-agent-0309`, plus flagship
  variants
- **`search_x` and `search_web`** as first-class tools (live X / web)
- **Sub-agents on by default** (`task` foreground, `delegate` background)
- **`computer` sub-agent** for macOS desktop automation via `agent-desktop`
- **`/verify`** — dedicated agent that builds, tests, boots, and runs
  browser smoke checks
- **Telegram remote control** — pair once, drive the agent from your phone
- **Schedules** — recurring or one-off prompts via background daemon
- **MCP** support via `mcpServers` in `.grok/settings.json`
- **Image / video generation** as in-chat tools
- **`--batch-api`** mode for cheaper unattended runs via xAI Batch API

## File Index

| File | Description |
|---|---|
| [architecture.md](architecture.md) | TUI, sub-agents, daemon, settings |
| [agentic-loop.md](agentic-loop.md) | Interactive vs. headless, sub-agent dispatch |
| [tool-system.md](tool-system.md) | Built-ins, sub-agents, MCP, skills |
| [context-management.md](context-management.md) | Sessions, batch API |
| [unique-patterns.md](unique-patterns.md) | Telegram, computer use, scheduling |
| [benchmarks.md](benchmarks.md) | TB2 result |
| [references.md](references.md) | Links |

## Benchmark Highlights

| Model | Rank | Score |
|---|---|---|
| Grok 4.20 Reasoning | **#43** | 57.3% |

---

*Tier 2 analysis — README is detailed and the source is open. Loop
internals are inferred from the public README; the actual TypeScript
source has not been reviewed file-by-file here.*
