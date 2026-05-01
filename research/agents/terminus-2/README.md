---
title: Terminus 2
status: complete
---

# Terminus 2

> AfterQuery / Laude Institute's reference agent scaffold bundled with the
> Terminal-Bench harness. Used as the canonical baseline to evaluate language
> models on Terminal-Bench 2.0 — appears 20+ times on the leaderboard.

## Overview

Terminus 2 is *not* a product — it is the **reference agent** ships inside the
[`harbor-framework/terminal-bench`](https://github.com/harbor-framework/terminal-bench)
repository (formerly `laude-institute/terminal-bench`) at
`terminal_bench/agents/terminus_2/`. It exists primarily to give every
language model a fair, common scaffold against which to measure raw model
ability on real terminal tasks.

Key characteristics:

- **Single-LLM loop**: one model, one tmux session, one prompt template
- **Pluggable parser**: emits either JSON or XML structured commands
- **LiteLLM backend**: works with any provider LiteLLM supports (OpenAI,
  Anthropic, Google, xAI, Z-AI, Moonshot, MiniMax, DeepSeek, Alibaba, …)
- **Effectively unlimited episodes** by default (`max_episodes = 1_000_000`)
- **No sub-agents, no MCP, no skills** — deliberately minimal

## Why it matters

Terminus 2 is the closest thing the leaderboard has to a "naked model" run.
Every entry tagged `Terminus 2 / <Model>` is essentially asking: *how well does
this model drive a terminal when given the simplest reasonable harness?* That
makes its ~25 leaderboard rows the most useful **cross-model** comparison on
TB2 — even if no individual row is the best score.

| Entry | Model | Rank | Score |
|---|---|---|---|
| Best | GPT-5.3-Codex | **#25** | 64.7% ±2.7 |
| | Claude Opus 4.6 | #28 | 62.9% |
| | Claude Opus 4.5 | #42 | 57.8% |
| | Gemini 3 Pro | #44 | 56.9% |
| | GPT-5.2 | #47 | 54.0% |
| | GLM 5 | #49 | 52.4% |
| Worst | GPT-OSS-20B | #124 | 3.1% |

(See `benchmarks.md` for the full ladder of 25 entries.)

## File Index

| File | Description |
|---|---|
| [architecture.md](architecture.md) | Class layout, parsers, tmux integration |
| [agentic-loop.md](agentic-loop.md) | Episode loop, completion signal |
| [tool-system.md](tool-system.md) | Single-tool design (keystrokes + duration) |
| [context-management.md](context-management.md) | Chat history, context-length handling |
| [unique-patterns.md](unique-patterns.md) | Reference-scaffold philosophy |
| [benchmarks.md](benchmarks.md) | All 25 TB2 entries |
| [references.md](references.md) | Source links |

---

*Tier 2/3 analysis — based on direct read of `terminus_2.py` source and the
TB2 leaderboard (snapshot 2026-05-01). Terminus 2 has no marketing site of
its own; it lives inside the benchmark repo.*
