---
title: ForgeCode
category: agents
---

# ForgeCode

> ZSH-native terminal coding agent; **#2 on Terminal-Bench 2.0** (81.8% with GPT-5.4, 2026-03-12) — held #1 from late-2025 through April 2026, when OpenAI's Codex + GPT-5.5 took the top spot at 82.0%.

## Overview

ForgeCode is a terminal-first AI coding agent built by [Antinomy HQ](https://github.com/antinomyhq). The open-source core lives at [github.com/antinomyhq/forge](https://github.com/antinomyhq/forge), written in Rust. A proprietary runtime layer called **ForgeCode Services** adds a context engine, tool-call corrections, and skill routing on top.

What makes ForgeCode architecturally distinct from other terminal agents (Claude Code, Aider, etc.) is the combination of:

1. **Multi-agent architecture** — three specialized sub-agents (Forge, Muse, Sage) with different access levels and purposes, rather than a single monolithic loop.
2. **ZSH-native integration** — the `:` sentinel character lets users send prompts from their native shell without entering a separate REPL. Aliases, Oh My Zsh plugins, and custom functions keep working.
3. **Model routing** — users can assign different models to different task phases (thinking model for planning, fast model for execution, large-context model for big files) and switch mid-session.
4. **ForgeCode Services** — a proprietary runtime layer providing semantic entry-point discovery, dynamic skill loading, tool-call correction, todo enforcement, and progressive reasoning budget control.

## Sub-Agents

| Agent | Access | Purpose |
|-------|--------|---------|
| **Forge** | Read + Write | Implementation — modifies files, creates code, executes commands. Active by default. |
| **Muse** | Read-only | Planning & analysis — creates detailed plans, reviews impact, analyzes architecture. |
| **Sage** | Read-only | Research & investigation — traces bugs, maps dependencies, understands codebases. Used internally by Forge and Muse. |

**Typical workflow**: Muse plans → Forge implements. Both delegate research to Sage transparently.

## Key Differentiators

- **ZSH-native**: Type `:` + space + prompt. No environment switch. File tagging with `@` + Tab.
- **Multi-agent, not single loop**: Each agent operates on bounded, minimal context for its role.
- **ForgeCode Services context engine**: Semantic search, entry-point discovery, up to 93% fewer tokens than naive approaches.
- **Tool corrections**: Heuristic + static analysis layer auto-corrects tool-call arguments before dispatch — critical for local/open-weight models.
- **Progressive thinking policy**: High reasoning budget for first 10 messages (planning), low for execution, high again at verification checkpoints.
- **Enforced verification skill**: Before completing a task, the runtime programmatically requires a reviewer-mode pass — "what evidence proves this is actually done?"

## Benchmark Results (Terminal-Bench 2.0)

| Model | Score | Rank | Date |
|-------|-------|------|------|
| GPT-5.4 | 81.8% ±2.0 | #2 | 2026-03-12 |
| Claude Opus 4.6 | 79.8% ±1.6 | #4 | 2026-03-12 |
| Gemini 3.1 Pro | 78.4% ±1.8 | #6 | 2026-03-02 |

ForgeCode held the #1 position from late-2025 through April 2026. It was overtaken by OpenAI's **Codex + GPT-5.5** (82.0%, 2026-04-23). ForgeCode is still the highest-ranked agent that runs *across* multiple frontier models within ~2 points of each other.

For comparison, Google reports Gemini 3.1 Pro at 68.5% on TermBench running natively. ForgeCode's runtime harness added ~10 percentage points on the same model weights.

## Architecture at a Glance

```
┌─────────────────────────────────────────────┐
│  ZSH Shell (native)                         │
│  `:` sentinel → ForgeCode Plugin            │
└──────────────┬──────────────────────────────┘
               │
┌──────────────▼──────────────────────────────┐
│  ForgeCode Runtime                          │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐       │
│  │  FORGE  │ │  MUSE   │ │  SAGE   │       │
│  │ (impl)  │ │ (plan)  │ │(research)│      │
│  └────┬────┘ └────┬────┘ └────┬────┘       │
│       └───────────┼───────────┘             │
│                   │                         │
│  ┌────────────────▼────────────────────┐    │
│  │  ForgeCode Services (proprietary)   │    │
│  │  • Context engine (semantic search) │    │
│  │  • Tool-call correction layer       │    │
│  │  • Skill engine (dynamic loading)   │    │
│  │  • Todo enforcement                 │    │
│  │  • Progressive thinking policy      │    │
│  └────────────────┬────────────────────┘    │
│                   │                         │
│  ┌────────────────▼────────────────────┐    │
│  │  LLM Providers                      │    │
│  │  Anthropic · OpenAI · Google ·      │    │
│  │  DeepSeek · Mistral · Meta · etc.   │    │
│  └─────────────────────────────────────┘    │
└─────────────────────────────────────────────┘
```

## Installation

```bash
curl -fsSL https://forgecode.dev/cli | sh
forge setup    # configure ZSH plugin
forge login    # authenticate with an AI provider
: hello world  # first prompt from native shell
```

Works on macOS, Linux, Android, and Windows (WSL/Git Bash). Requires ZSH and a Nerd Font.

## Open Source vs. Proprietary

- **Open source** ([github.com/antinomyhq/forge](https://github.com/antinomyhq/forge)): Core agent, multi-agent framework, tool definitions, ZSH plugin, all configuration.
- **Proprietary** (ForgeCode Services): Context engine, tool-call correction layer, skill routing, semantic search indexing, todo enforcement, reasoning budget control. Currently free to use.

## Deep-Dive Files

| File | Topic |
|------|-------|
| [architecture.md](architecture.md) | Multi-agent architecture — Forge, Muse, Sage, bounded context |
| [agentic-loop.md](agentic-loop.md) | How the multi-agent loop orchestrates tasks |
| [tool-system.md](tool-system.md) | Tool system, tool corrections for local models |
| [context-management.md](context-management.md) | Context engine, bounded context across sub-agents |
| [unique-patterns.md](unique-patterns.md) | Key differentiators and novel patterns |
| [benchmarks.md](benchmarks.md) | Terminal-Bench 2.0 scores and analysis |
| [references.md](references.md) | All source links |

## References

- Website: https://forgecode.dev
- Docs: https://forgecode.dev/docs
- GitHub: https://github.com/antinomyhq/forge
- Blog (Part 1): https://forgecode.dev/blog/benchmarks-dont-matter/
- Blog (Part 2): https://forgecode.dev/blog/gpt-5-4-agent-improvements/
- Terminal-Bench 2.0 leaderboard: https://www.tbench.ai/leaderboard/terminal-bench/2.0

## Recent Updates (April 2026)

Highlights from the `v2.12.7` → `v2.12.10` release stream (late April 2026):

- **GPT-5.5 / GPT-5.5-pro support** — added to the provider layer ([#3158](https://github.com/antinomyhq/forge/pull/3158)). (The TB2 #2 result of 81.8% was set on GPT-5.4; GPT-5.5 support landed as Codex moved into #1 with the same model family.)
- **NVIDIA provider** — new OpenAI-compatible NVIDIA provider support ([#2847](https://github.com/antinomyhq/forge/pull/2847)).
- **DeepSeek improvements** — DeepSeek V4 models added to the OpenCode providers list; `reasoning_effort` now propagated to the DeepSeek API ([#3159](https://github.com/antinomyhq/forge/pull/3159)).
- **MiMo V2.5 Pro** added to the OpenCode Go provider list ([#3185](https://github.com/antinomyhq/forge/pull/3185)).
- **Sub-agent / tool-registry refactor** — research sub-agent filtering moved into the tool registry ([#3114](https://github.com/antinomyhq/forge/pull/3114)).
- **MCP race-condition fix** — concurrent MCP initialisation race resolved ([#3181](https://github.com/antinomyhq/forge/pull/3181)).
- **Ergonomics** — custom terminal-safe spinner replaces `indicatif` ([#3178](https://github.com/antinomyhq/forge/pull/3178)); reasoning-effort rendered next to model in the CLI prompt; `forge logs` command added; local provider config renames `URL` → `HOST` (with backward compatibility); `use_forge_committer` git config option to control commit author identity.
- **Reliability** — Codex 503 responses now retried; OpenAI Responses strict-mode disabled for "open" tool schemas to avoid spurious rejections; `InvalidContentType` SSE errors now include the response body for easier debugging.
