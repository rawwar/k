---
title: CAMEL-AI
status: complete
---

# CAMEL-AI

> Open-source multi-agent framework for "finding the scaling laws of
> agents" — research-led, with role-playing and large-scale agent
> simulation as core use cases. **#58 on Terminal-Bench 2.0** with
> Claude Sonnet 4.5 (46.5%).

## Overview

CAMEL (Communicative Agents for "Mind" Exploration of Large Language
Models, repo `camel-ai/camel`) is an Apache-2.0 framework that started
as a 2023 NeurIPS paper on role-playing multi-agent communication and
grew into a broad multi-agent research toolkit. The CAMEL-AI community
has 100+ researchers across CMU, Caltech, Tsinghua, Oxford, NUS, KAUST,
and many other institutions.

The framework's stated mission is studying agent **scaling laws** —
they emphasise being able to simulate up to ~1M agents — rather than
shipping a coding-specific product.

The CAMEL-AI **TB2 entry** is a coding-mode configuration of the
broader framework, not a separately-named product. The leaderboard org
is "CAMEL-AI" and the entry sits at #58 (Claude Sonnet 4.5, 46.5%).

Key characteristics (framework-level):

- **Multi-agent role-playing** — original CAMEL paper concept
- **ChatAgent** — base single-agent abstraction; multi-agent topologies
  built on top
- **Workforce** — agents with roles, hierarchies, long-horizon tasks
- **Stateful memory** — agents persist context across multi-step
  interactions
- **Code-as-prompt** principle — code and comments treated as
  agent-readable prompts
- **Synthetic data generation** is a major use case
- **OWL** (sister project, `camel-ai/owl`) — multi-agent collaboration
  framework built on CAMEL; #1 open-source on GAIA

## File Index

| File | Description |
|---|---|
| [architecture.md](architecture.md) | ChatAgent, Workforce, components |
| [agentic-loop.md](agentic-loop.md) | Role-play loop, multi-agent topologies |
| [tool-system.md](tool-system.md) | Toolkit, MCP, memory, retrieval |
| [context-management.md](context-management.md) | Memory, statefulness |
| [unique-patterns.md](unique-patterns.md) | Scaling laws, role-play, OWL |
| [benchmarks.md](benchmarks.md) | TB2 result + GAIA context |
| [references.md](references.md) | Links |

## Benchmark Highlights

| Benchmark | Configuration | Rank | Score |
|---|---|---|---|
| Terminal-Bench 2.0 | Claude Sonnet 4.5 | **#58** | 46.5% ±2.4 |
| GAIA (via OWL sister project) | — | #1 OSS | 69.09 avg |

---

*Tier 3 brevity. CAMEL is a large framework (the README is huge); this
page focuses on the parts relevant to using it as a coding agent. The
specific TB2 submission's agent topology is not detailed in public
materials; the Sonnet 4.5 row is the only published result.*
