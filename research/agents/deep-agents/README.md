---
title: Deep Agents
status: complete
---

# Deep Agents

> LangChain's "batteries-included agent harness" — an opinionated default
> agent on top of LangGraph, with a coding-CLI variant.
> **#21 on Terminal-Bench 2.0** with GPT-5.2-Codex (66.5%).

## Overview

`deepagents` (PyPI: [`deepagents`](https://pypi.org/project/deepagents/),
repo `langchain-ai/deepagents`) is a Python package that gives you a
working agent in one call: `create_deep_agent()`. It wraps the
LangGraph runtime with a curated set of built-in tools, smart prompts,
sub-agent dispatch, and context management — so you don't have to wire
that up yourself.

The package ships in two parts:

- **Deep Agents SDK** — `pip install deepagents`, library API
- **Deep Agents CLI** — pre-built terminal coding agent, the harness
  that produced the TB2 #21 score

Plus an **ACP integration** (Agent Client Protocol) for using deep
agents inside code editors like Zed.

Key characteristics:

- **LangGraph-native** — the returned object is a compiled LangGraph,
  usable with streaming, Studio, checkpointers, etc.
- **Provider-agnostic** — Google, OpenAI, Anthropic, OpenRouter,
  Fireworks, Baseten, Ollama
- **Built-in tools**: `write_todos`, `read_file`, `write_file`,
  `edit_file`, `ls`, `glob`, `grep`, `execute` (sandboxed shell), `task`
  (sub-agent)
- **Auto-summarisation** when conversations get long; large outputs
  saved to files
- **MCP** via `langchain-mcp-adapters`

## File Index

| File | Description |
|---|---|
| [architecture.md](architecture.md) | SDK, CLI, ACP, LangGraph runtime |
| [agentic-loop.md](agentic-loop.md) | LangGraph loop, todos, sub-agents |
| [tool-system.md](tool-system.md) | Built-in tools, MCP, custom tools |
| [context-management.md](context-management.md) | Auto-summarise, files-as-overflow |
| [unique-patterns.md](unique-patterns.md) | Harness vs. framework, batteries-included |
| [benchmarks.md](benchmarks.md) | TB2 result |
| [references.md](references.md) | Links |

## Benchmark Highlights

| Model | Rank | Score |
|---|---|---|
| GPT-5.2-Codex | **#21** | 66.5% ±3.1 |

---

*Tier 2 analysis — README + LangChain docs are the primary source.
Internal LangGraph mechanics are documented in `docs.langchain.com`.*
