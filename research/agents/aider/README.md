---
title: Aider — AI Pair Programming in the Terminal
status: complete
---

# Aider

> Pioneering AI pair programming tool that lets you collaboratively edit code with LLMs directly from the terminal, and home to the most comprehensive code-editing model benchmark.

## Overview

**Aider** is an open-source, terminal-based AI pair programming tool created by **Paul Gauthier** ([@paul-gauthier](https://github.com/paul-gauthier)). First released in mid-2023, it rapidly became one of the most influential tools in the AI-assisted coding space, accumulating 30k+ GitHub stars, 5.7M+ pip installs, and processing ~15B tokens per week across its user base.

Aider occupies a distinctive position in the AI coding landscape. It is **not** a fully autonomous agent like Claude Code or Devin — it doesn't browse the web, spawn sub-processes, or orchestrate multi-step plans independently. Instead, it is a **deeply interactive pair programmer**: the human stays in the loop, directing the conversation, adding files to context, and approving changes. This human-in-the-loop design philosophy makes it more of a power tool than an autonomous agent.

Despite this, Aider has been profoundly influential on the entire category. Its innovations — particularly the **edit format system**, **repo-map**, **architect mode**, and the **benchmarking leaderboard** — have shaped how the entire industry thinks about LLM code editing.

### What Makes Aider Special

1. **Edit Format Innovation**: Aider pioneered multiple strategies for getting LLMs to reliably edit files — whole-file replacement, search/replace blocks, unified diffs, and the architect/editor split. Each format is tuned to different model strengths.

2. **Repo-Map**: A tree-sitter-powered, graph-ranked summary of the entire codebase that gives the LLM structural awareness without consuming the full context window.

3. **Architect Mode**: A dual-model approach where a reasoning model plans changes and a code-editing model applies them — achieving SOTA benchmark results.

4. **The Leaderboard**: Aider maintains the most comprehensive public benchmark for code-editing LLM performance, testing 70+ model configurations across 225 Exercism exercises. This has become a de facto industry reference.

5. **Git-Native Workflow**: Every AI edit is automatically committed with a descriptive message, making undo trivial and change review natural.

## Top Benchmark Scores (as of July 2025)

| Model | Score | Cost | Edit Format |
|-------|-------|------|-------------|
| GPT-5 (high) | 88.0% | $29.08 | diff |
| GPT-5 (medium) | 86.7% | $17.69 | diff |
| o3-pro (high) | 84.9% | $146.32 | diff |
| Gemini 2.5 Pro Preview (32k think) | 83.1% | $49.88 | diff-fenced |
| GPT-5 (low) | 81.3% | $10.37 | diff |
| o3 (high) | 81.3% | $21.23 | diff |
| Grok-4 (high) | 79.6% | $59.62 | diff |

> **Update — April 2026 (post-v0.86.1 main):** Aider's `main` branch (last commit
> 2026-04-25) adds settings for the **GPT-5.1 / 5.2 / 5.3 / 5.4** and **GPT-5-pro**
> families across OpenAI, Azure, and OpenRouter; **Claude Sonnet/Opus 4.5 and 4.6**
> with updated `sonnet`/`haiku`/`opus` aliases; the **Gemini 3 Pro preview** (now the
> default `gemini` alias); **DeepSeek Reasoner**; and **GPT-4.1-nano**. New UX:
> the `/ok` shortcut to greenlight a proposed change with optional extra instructions,
> and the ability to add files outside the git repo or promote read-only files to
> editable when auto-commits are disabled. The repo-map gained tree-sitter tags for
> **Fortran, Haskell, Julia, and Zig** and is compatible with newer tree-sitter
> Python APIs. The deprecated `google-generativeai` dependency was removed.

## Key Stats

- **Language**: Python (~88% self-written by aider itself in recent releases)
- **License**: Apache 2.0
- **5.7M+ installs** via PyPI
- **100+ programming languages** supported via tree-sitter
- **Works with virtually any LLM**: OpenAI, Anthropic, Google, DeepSeek, Ollama, local models, and more

## Architecture at a Glance

```
┌─────────────────────────────────────────┐
│              User (Terminal)             │
│         /add, /ask, /code, /arch        │
└────────────────┬────────────────────────┘
                 │
       ┌─────────▼──────────┐
       │    Coder (Core)     │
       │  - base_coder.py    │
       │  - EditFormat       │
       │  - ChatHistory      │
       └──┬──────┬───────┬──┘
          │      │       │
    ┌─────▼┐  ┌──▼───┐ ┌─▼────────┐
    │Repo  │  │Model │ │Git       │
    │Map   │  │Router│ │Integration│
    │(tree │  │      │ │(auto-    │
    │sitter)│ │      │ │commits)  │
    └──────┘  └──────┘ └──────────┘
```

## Files in This Research

| File | Contents |
|------|----------|
| [architecture.md](architecture.md) | Core architecture, module layout, class hierarchy |
| [agentic-loop.md](agentic-loop.md) | The edit → apply → lint → test loop |
| [tool-system.md](tool-system.md) | Edit formats: diff, whole, diff-fenced, architect |
| [context-management.md](context-management.md) | Repo-map, token budgeting, file selection |
| [unique-patterns.md](unique-patterns.md) | Key differentiators and innovations |
| [benchmarks.md](benchmarks.md) | The Aider leaderboard — methodology and data |
| [references.md](references.md) | Links to source, docs, blog posts |
