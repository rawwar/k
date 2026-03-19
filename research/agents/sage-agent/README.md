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
