---
title: TongAgents
status: research-limited
---

# TongAgents

> A coding agent built by BIGAI (Beijing Institute for General Artificial Intelligence) that achieved top-3 performance on Terminal-Bench 2.0, though with very limited public documentation.

## Overview

TongAgents is a coding agent system developed by **BIGAI** (北京通用人工智能研究院, Beijing Institute for General Artificial Intelligence), a leading Chinese AI research institute focused on artificial general intelligence. The agent appeared on the Terminal-Bench 2.0 leaderboard in March 2026 and quickly demonstrated strong performance across multiple foundation models.

**⚠️ Research Limitation:** As of this writing, no public GitHub repository, academic paper, or detailed technical documentation has been found for TongAgents. This analysis is based on benchmark results, BIGAI's broader research portfolio, and reasonable inferences from the agent's naming conventions and performance patterns. Claims are clearly marked as known facts vs. informed speculation.

## What We Know (Confirmed)

- **Terminal-Bench 2.0 Rank #3** with Gemini 3.1 Pro: **80.2% ±2.6** (submitted 2026-03-13)
- **Terminal-Bench 2.0 Rank #13** with Claude Opus 4.6: **~71.9%** (estimated from leaderboard position)
- Built by BIGAI, the same institute behind TongSIM, tong-geometry, and other "Tong"-prefixed projects
- The name follows BIGAI's established naming convention where "Tong" (通, meaning "general/universal") is a prefix for their tools and platforms

## BIGAI (The Organization)

BIGAI is a prominent Chinese AI research institute founded by **Professor Zhu Songchun (朱松纯)**, a well-known computer vision and AI researcher with a background at UCLA. The institute focuses on:

- **AGI research** — cognitive architectures, reasoning, planning
- **Embodied intelligence** — robotics, humanoid control (LIFT, ECO)
- **Simulation platforms** — TongSIM (physics simulation, open-sourced, trending on HuggingFace)
- **Game AI** — CivRealm (strategy game environment)
- **Computer vision** — extensive publication history at CVPR, ICLR, CoRL

BIGAI has published in top venues including Nature Machine Intelligence and Science. Their GitHub organization (`bigai-ai`) has 34+ public repositories, predominantly in robotics and embodied AI. The move into coding agents represents an expansion of their agent research portfolio.

## Key Results

| Benchmark | Model | Score | Rank | Date |
|-----------|-------|-------|------|------|
| Terminal-Bench 2.0 | Gemini 3.1 Pro | 80.2% ±2.6 | #3 | 2026-03-13 |
| Terminal-Bench 2.0 | Claude Opus 4.6 | ~71.9% | #13 | 2026-03 |

The ~8 percentage point gap between models is notable and discussed further in [context-management.md](context-management.md).

## What Makes It Interesting

1. **Institutional backing** — BIGAI is a well-funded AGI-focused research institute, not a startup or solo project
2. **Multi-model support** — tested with both Gemini and Claude, suggesting model-agnostic design
3. **Rapid entry** — appeared on leaderboards without prior public announcements or papers
4. **"Tong" ecosystem** — part of a broader family of BIGAI tools and platforms
5. **Chinese AI research context** — represents growing Chinese institutional investment in coding agents

## File Index

| File | Contents |
|------|----------|
| [architecture.md](architecture.md) | Inferred architecture based on naming and performance patterns |
| [agentic-loop.md](agentic-loop.md) | What can be inferred about the execution loop |
| [tool-system.md](tool-system.md) | Likely tool capabilities based on Terminal-Bench requirements |
| [context-management.md](context-management.md) | Inferences from model performance gaps |
| [unique-patterns.md](unique-patterns.md) | Distinctive aspects and institutional context |
| [benchmarks.md](benchmarks.md) | Detailed Terminal-Bench 2.0 results and comparisons |
| [references.md](references.md) | Links to BIGAI, Terminal-Bench, and related projects |
