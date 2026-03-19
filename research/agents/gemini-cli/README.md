---
title: Gemini CLI Architecture Analysis
status: complete
---

# Gemini CLI

> Google's first-party terminal coding agent — open-source, TypeScript-based,
> leveraging Gemini models with 1M token context windows and multimodal input.

## Overview

Gemini CLI is Google's official command-line AI coding agent, released as open-source
under the Apache 2.0 license. It occupies the same category as Anthropic's Claude Code
and OpenAI's Codex CLI — a terminal-native agent built by the model provider themselves,
deeply optimized for their own models.

Built in **TypeScript** as a monorepo (packages/core + packages/cli), Gemini CLI is
designed around Gemini's distinctive strengths: massive 1M token context windows,
native multimodal input (images, audio, PDFs), Google Search grounding for real-time
information, and a multi-tier sandboxing system that spans macOS Seatbelt, Docker,
Podman, gVisor, and LXC.

**Key differentiators from other agents:**
- 1M token context window (largest among terminal agents)
- Multimodal coding: paste screenshots, audio, PDFs into the conversation
- Google Search grounding: real-time web information via `google_web_search`
- Progressive skill disclosure: on-demand expertise loaded only when needed
- Multi-tier sandboxing: 4+ sandbox backends for different security needs
- Token caching: automatic API-level optimization for repeated system instructions
- Checkpointing: shadow git repos for conversation + tool call history
- Free tier: 60 req/min, 1000 req/day with Google OAuth (no API key needed)

## At a Glance

| Attribute | Details |
|---|---|
| **Developer** | Google (google-gemini) |
| **License** | Apache 2.0 |
| **Language** | TypeScript (Node.js) |
| **Repository** | github.com/google-gemini/gemini-cli |
| **Package** | @anthropic-ai/claude-code → `gemini` on npm |
| **Install** | `npm install -g @anthropic-ai/gemini-cli`, Homebrew, npx |
| **Models** | Gemini 3 Flash, Gemini 2.5 Pro/Flash |
| **Context** | 1M tokens |
| **Auth** | Google OAuth (free), API Key, Vertex AI |
| **Release** | Weekly: nightly → preview (Tue) → stable (Tue) |
| **Terminal-Bench** | #55 (Gemini 3 Flash, 47.4%) |

## Architecture at a Glance

```
┌─────────────────────────────────────────────────────────────────────┐
│                         packages/cli                                │
│   CLI entry point, argument parsing, REPL, slash commands,          │
│   output rendering, interactive UI                                  │
└────────────────────────────────┬────────────────────────────────────┘
                                 │
┌────────────────────────────────▼────────────────────────────────────┐
│                        packages/core                                │
│                                                                     │
│  ┌──────────┐  ┌──────────────┐  ┌──────────────┐  ┌───────────┐  │
│  │  agent/   │  │   agents/    │  │    core/     │  │  tools/   │  │
│  │ orchestr. │  │  sub-agents  │  │ client, chat │  │ 18+ tools │  │
│  │           │  │              │  │ content gen  │  │ registry  │  │
│  └──────────┘  └──────────────┘  │ turns, token │  └───────────┘  │
│                                   │ limits, sched│                  │
│  ┌──────────┐  ┌──────────────┐  └──────────────┘  ┌───────────┐  │
│  │ sandbox/ │  │   config/    │                     │   mcp/    │  │
│  │ Seatbelt │  │  settings    │  ┌──────────────┐  │  servers  │  │
│  │ Docker   │  │  GEMINI.md   │  │   skills/    │  │  protocol │  │
│  │ gVisor   │  │              │  │  progressive │  └───────────┘  │
│  │ LXC      │  └──────────────┘  │  disclosure  │                  │
│  └──────────┘                     └──────────────┘                  │
│                                                                     │
│  ┌──────────┐  ┌──────────────┐  ┌──────────────┐  ┌───────────┐  │
│  │ policy/  │  │confirmation- │  │   safety/    │  │ routing/  │  │
│  │ security │  │    bus/      │  │   filters    │  │ model sel │  │
│  └──────────┘  │ user confirm │  └──────────────┘  └───────────┘  │
│                 └──────────────┘                                    │
│  ┌──────────┐  ┌──────────────┐  ┌──────────────┐  ┌───────────┐  │
│  │ hooks/   │  │   voice/     │  │    ide/      │  │ output/   │  │
│  │lifecycle │  │  voice input │  │  VS Code     │  │ rendering │  │
│  └──────────┘  └──────────────┘  └──────────────┘  └───────────┘  │
│                                                                     │
│  ┌──────────┐  ┌──────────────┐  ┌──────────────┐                  │
│  │telemetry/│  │  fallback/   │  │  billing/    │                  │
│  │ tracking │  │  error recov │  │  usage track │                  │
│  └──────────┘  └──────────────┘  └──────────────┘                  │
└─────────────────────────────────────────────────────────────────────┘
```

## Research Files

| File | Description |
|---|---|
| [architecture.md](architecture.md) | TypeScript monorepo structure, core modules, sandbox architecture, MCP integration |
| [agentic-loop.md](agentic-loop.md) | Turn processing, content generation, tool scheduling, plan mode, sub-agents |
| [tool-system.md](tool-system.md) | All 18+ tools, confirmation model, sandbox integration, MCP extension |
| [context-management.md](context-management.md) | GEMINI.md hierarchy, JIT context, token caching, checkpointing, skills |
| [unique-patterns.md](unique-patterns.md) | 1M context strategy, multimodal coding, Google ecosystem, progressive skills |
| [benchmarks.md](benchmarks.md) | Terminal-Bench 2.0 results, model comparison analysis |
| [references.md](references.md) | GitHub repo, docs, npm package, related tools |

## Key Findings

### What Gemini CLI Does Well
1. **Largest context window** — 1M tokens lets it ingest entire codebases without chunking
2. **Free tier is generous** — Google OAuth gives 60 req/min, 1000 req/day at no cost
3. **Multi-tier sandboxing** — Most flexible isolation system of any terminal agent
4. **Token caching** — Automatic API-level optimization reduces costs for API key users
5. **Multimodal native** — Images, audio, PDFs as first-class inputs to coding tasks
6. **Google Search grounding** — Real-time web information integrated into agentic loop
7. **Progressive skill disclosure** — Keeps context lean, loads expertise on demand
8. **Checkpointing** — Shadow git repos let you restore any conversation state

### Where It Falls Short
1. **Benchmark performance** — 47.4% on Terminal-Bench 2.0 (Gemini 3 Flash) is mid-tier
2. **Model quality gap** — Gemini 2.5 Pro scores only 19.6%, suggesting the agent layer
   matters less than the underlying model for difficult tasks
3. **Ecosystem maturity** — Newer than Claude Code, fewer community integrations
4. **Complexity** — Multi-tier sandboxing adds configuration burden

### Architectural Insights
- The **confirmation bus** pattern is a clean abstraction for managing user consent
  across diverse tool types and sandbox environments
- **GEMINI.md hierarchy** (global → workspace → JIT) mirrors Claude Code's
  CLAUDE.md pattern — this is becoming a de facto standard for agent configuration
- The **skills system** with progressive disclosure is unique — no other terminal agent
  has this level of on-demand expertise management
- **Sub-agent support** via `complete_task` enables decomposition of complex tasks,
  similar to Claude Code's sub-agent system but with explicit tool-level support

### Comparison with Claude Code
| Dimension | Gemini CLI | Claude Code |
|---|---|---|
| Context window | 1M tokens | 200K tokens |
| Multimodal | Images, audio, PDF | Images only |
| Sandboxing | 4+ backends | macOS Seatbelt only |
| Free tier | Yes (Google OAuth) | No |
| Token caching | Automatic | Prompt caching |
| Web search | Google Search grounding | web_fetch only |
| Skills system | Progressive disclosure | N/A |
| Checkpointing | Shadow git repos | Conversation resume |
| Benchmark | 47.4% (TB 2.0) | Higher tier |
| Maturity | Newer | More established |

## How to Read This Research

Start with this README for the high-level picture. Then:
- **Architecture deep-dive** → `architecture.md`
- **How the agent loop works** → `agentic-loop.md`
- **What tools are available** → `tool-system.md`
- **How context is managed** → `context-management.md`
- **What makes it unique** → `unique-patterns.md`
- **Performance data** → `benchmarks.md`
- **Source links** → `references.md`
