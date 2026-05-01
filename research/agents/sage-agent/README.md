---
title: SageAgent Architecture Analysis
status: complete
---

# SageAgent

> A Python-based multi-agent system framework by OpenSage for building AI agents with deep research and rapid execution modes.

## Overview

SageAgent (repo: `OpenSageAI/Sage`) is an open-source, MIT-licensed framework that orchestrates
multiple specialized agents in a pipeline to accomplish complex tasks. Built by Zhang Zheng
(zhangzheng-thu, likely Tsinghua-affiliated) under the OpenSageAI GitHub organization.

Key characteristics:
- **Multi-agent pipeline**: Five specialized agents collaborate in sequence with a feedback loop
- **Dual execution modes**: Deep Research (comprehensive) and Rapid Execution (lightweight)
- **MCP-native tooling**: First-class Model Context Protocol support for tool integration
- **Streamlit demo UI**: Web-based interface via `sage_demo.py`

## 2026 Q2 Update — Active Development Surge

The active development repo is now **`ZHangZHengEric/Sage`** (the `OpenSageAI/Sage` mirror
has not received commits since May 2025; the `ZHangZHengEric` fork has been the canonical
trunk through 2026). Highlights from the **2026-03 → 2026-04** window relevant to this
analysis:

- **Sage Terminal (TUI)** — a new terminal UI client landed across Apr 25-30 (commits
  `add29b9` "Add Sage Terminal TUI preview", `4232168` "Refactor Sage Terminal modules",
  `2713790` "Polish Sage Terminal composer and overlays"), adding a first-class TUI
  alongside the existing Streamlit web demo. A `Sage Terminal` runtime distribution is
  now built and shipped (`58f74b1`).
- **Pipeline simplified — TaskRouterAgent removed.** PR #110/#111/#112 (Apr 24,
  `c25a7fa` "refactor: remove TaskRouterAgent and simplify default flow"; `bfdf679`
  "Remove task router; refactor plan agent; add lint tool and structured tool errors")
  removed an intermediate routing agent that had been added between releases and pulled
  the default flow back closer to the canonical 5-agent pipeline.
- **`turn_status` protocol replaces `finish_turn`** (Apr 26, `2d80b89`; Apr 27 follow-ups
  `7b373cf`, `de8abb1`, `03a3ca8`) — the previous `finish_turn` signal was replaced with
  a richer `turn_status` protocol, with the status pair stripped from SSE output but kept
  visible to the LLM and tagged with `metadata.coerced_from` when coerced.
- **Desktop app** — Windows NSIS-only bundle and macOS desktop builds (`867c977`,
  `d50bdf3`, `5f10f04` across Apr 26-27).
- **Self-check agent** — added Apr 29 (`260bd1b`) alongside passthrough sandbox config
  and chat markdown workspace.
- **Bedrock / multi-agent solution docs** — Apr 29 (`b5c89be` "docs: add Bedrock primer
  and multi-agent solution guides").
- **`SAGE_FORCE_TOOL_CHOICE_REQUIRED` env var** — Apr 29 (`24c4e27`) gates forced
  `tool_choice=required`; a tool-call loop bug under that flag was fixed Apr 30
  (`5e656d3`, PR #143 merged 2026-05-01).

## Architecture (High-Level)

```
User Input
    │
    ▼
┌─────────────────┐
│ AgentController  │  (entry point)
└────────┬────────┘
         ▼
┌─────────────────┐
│TaskAnalysisAgent │  Understand requirements
└────────┬────────┘
         ▼
┌─────────────────┐
│  PlanningAgent   │◄─────────────┐  Plan subtasks
└────────┬────────┘               │
         ▼                        │
┌─────────────────┐               │
│  ExecutorAgent   │               │  Execute via ToolManager
│  ┌─────────────┐│               │
│  │ ToolManager  ││               │
│  │ Local │ MCP  ││               │
│  └─────────────┘│               │
└────────┬────────┘               │
         ▼                        │
┌─────────────────┐               │
│ObservationAgent  │───(incomplete)┘  Monitor progress
└────────┬────────┘
         │ (complete)
         ▼
┌─────────────────┐
│TaskSummaryAgent  │  Generate final output
└────────┬────────┘
         ▼
    Final Output
```

## Benchmark Highlights

| Benchmark | Model | Score | Rank |
|---|---|---|---|
| Terminal-Bench 2.0 | GPT-5.3-Codex | 78.4% ±2.2 | #5 |

Score ties with ForgeCode (Gemini 3.1 Pro) at 78.4%. Result dated 2026-03-13.

## File Index

| File | Description |
|---|---|
| [architecture.md](architecture.md) | Multi-agent pipeline, directory layout, execution modes |
| [agentic-loop.md](agentic-loop.md) | Agent pipeline flow and feedback mechanism |
| [tool-system.md](tool-system.md) | ToolBase, ToolManager, MCP integration |
| [context-management.md](context-management.md) | Message format, inter-agent context flow |
| [unique-patterns.md](unique-patterns.md) | Distinctive design patterns and decisions |
| [benchmarks.md](benchmarks.md) | Terminal-Bench 2.0 results |
| [references.md](references.md) | Links and resources |

## Roadmap (from README)

1. Tool System Enhancements — more comprehensive MCP server support
2. Logger Optimization
3. Supported Models — expand tested model coverage
4. Infinite Context — ultra-long and complex task support
5. Professional Agents — domain-specialized agent modules

---

*This is a Tier 3 (lighter treatment) analysis. Some internal details are inferred from
public README and directory structure rather than deep source review.*
