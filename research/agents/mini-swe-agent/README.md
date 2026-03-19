---
title: mini-SWE-agent
status: complete
---

# mini-SWE-agent

> **"What if our agent was 100x simpler, and still worked nearly as well?"**
>
> Minimal bash-only coding agent from Princeton/Stanford; ~100 lines of Python, >74% on SWE-bench Verified.

## Overview

mini-SWE-agent is a deliberately minimal AI software engineering agent from the Princeton & Stanford team behind [SWE-bench](https://swebench.com) and [SWE-agent](https://swe-agent.com). It is the intellectual successor to SWE-agent — born from the realization that as LLMs became more capable through 2024–2025, the elaborate tool interfaces and history processors that SWE-agent pioneered were no longer necessary to achieve state-of-the-art performance.

The core agent class is approximately **100 lines of Python**. With the environment (~80 lines), model wrapper (~130 lines), and run script (~40 lines), the entire functional system totals roughly 350 lines. There are no fancy dependencies, no custom tool implementations, and no complex state management.

Despite this radical simplicity, mini-SWE-agent achieves **>74% on SWE-bench Verified** (with Gemini 3 Pro) — competitive with the most complex commercial agents that have orders of magnitude more code.

## Design Philosophy

The project asks a provocative question: **what happens when you strip an agent down to its absolute essence?** The answer turns out to be: not much is lost. This has profound implications:

1. **Scaffold complexity has diminishing returns** — the gains from elaborate tool systems, context compaction, and history processing shrink as base models improve
2. **Put the LM in the center** — instead of building intelligence into the scaffold, let the language model figure things out with raw bash
3. **Simplicity enables scale** — stateless execution and linear history make sandboxing, parallelization, and debugging trivial

## Adoption

mini-SWE-agent is widely adopted across industry and academia:

- **Meta** — agent research and evaluation
- **NVIDIA** — coding agent development
- **Essential AI** — foundation model evaluation
- **IBM** — agent benchmarking
- **Nebius**, **Anyscale** — infrastructure and scaling
- **Princeton University**, **Stanford University** — academic research, fine-tuning, RL for agents

It serves as the official **bash-only baseline** on the [SWE-bench leaderboard](https://swebench.com), where it powers the "bash-only" comparison track that isolates LM capability from agent scaffold sophistication.

## Three Key Design Decisions

| Decision | Implementation | Why It Matters |
|----------|---------------|----------------|
| **Bash-only** | No custom tools; one `bash` tool via tool-call API or triple-backtick parsing | Works with literally any model; nothing to install in sandboxes |
| **Linear history** | Messages append-only; trajectory == LM input | Perfect for debugging, fine-tuning, and RL training data |
| **Stateless execution** | Each action is a fresh `subprocess.run` | Trivial sandboxing; swap for `docker exec` and you're done |

## Quick Start

```bash
# Try it immediately (no install)
pip install uv && uvx mini-swe-agent

# Or install
pip install mini-swe-agent
mini  # launches the CLI
```

## File Index

| File | Description |
|------|-------------|
| [architecture.md](architecture.md) | System architecture and component design |
| [agentic-loop.md](agentic-loop.md) | The ~100 line ReAct loop — annotated source code |
| [tool-system.md](tool-system.md) | Bash-only tool philosophy and implementation |
| [context-management.md](context-management.md) | Linear history and output truncation |
| [unique-patterns.md](unique-patterns.md) | **Key insights** — why minimal works |
| [benchmarks.md](benchmarks.md) | SWE-bench Verified scores, Terminal-Bench, roulette experiments |
| [references.md](references.md) | Links, papers, related projects |

## References

- GitHub: https://github.com/SWE-agent/mini-swe-agent
- Docs: https://mini-swe-agent.com
- SWE-bench Verified: >74% with Gemini 3 Pro
- Tutorial: https://minimal-agent.com
- SWE-agent paper: https://arxiv.org/abs/2405.15793
