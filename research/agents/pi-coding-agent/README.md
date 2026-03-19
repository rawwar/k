---
title: Pi — The Radically Extensible Terminal Coding Agent
status: complete
---

# Pi Coding Agent

> "If I don't need it, it won't be built." — A minimal, radically extensible terminal coding agent that ships four tools and lets extensions handle everything else.

## Overview

Pi (`@mariozechner/pi-coding-agent`) is a terminal-based coding agent created by **Mario Zechner** (@badlogic), the well-known open-source developer behind the libGDX game framework. Pi was born from frustration with Claude Code's growing complexity — Mario described it as a tool that "turned into a spaceship with 80% of functionality I have no use for." Pi takes the opposite approach: an aggressively minimal core with a powerful extension system that lets users build exactly the features they need.

The project lives in a monorepo (`pi-mono`) organized as seven packages, each independently publishable to npm. The coding agent itself is just one package that composes the others. The website URL — shittycodingagent.ai — captures the tongue-in-cheek philosophy: this is intentionally not a polished product, it's a set of primitives for people who want control.

Pi's core philosophy is **primitives over features**. Where other agents build in MCP support, sub-agents, plan modes, permission systems, and background tasks, Pi deliberately omits all of these from the core. Instead, it provides an extension API powerful enough that each of these can be (and has been) implemented as a community package. This keeps the core simple, the prompt cache stable, and the behavior predictable.

### What Makes Pi Special

1. **Four tools, infinite extensibility** — Ships with only `read`, `write`, `edit`, and `bash`. Everything else is an extension.
2. **Monorepo architecture** — Seven packages that separate concerns cleanly: LLM API, agent runtime, TUI, coding agent, web UI, Slack bot, GPU pod management.
3. **Cross-provider LLM API** — `pi-ai` unifies 15+ providers behind a single interface with context handoff between providers.
4. **Tree-structured sessions** — JSONL files with parent IDs enabling in-place branching, navigation, and fork operations.
5. **Skills system** — On-demand capability packages following the Agent Skills standard (agentskills.io) for progressive context disclosure.
6. **Message queue** — Submit steering messages or follow-ups while the agent works, with configurable delivery modes.
7. **Anti-feature-creep philosophy** — Deliberate omissions (no MCP, no sub-agents, no plan mode, no permissions) are a feature, not a limitation.

## Key Stats

- **Language**: TypeScript (monorepo, 7 packages)
- **License**: MIT
- **Creator**: Mario Zechner (@badlogic)
- **Repository**: github.com/badlogic/pi-mono
- **Website**: shittycodingagent.ai
- **Default tools**: 4 (read, write, edit, bash)
- **LLM providers**: 15+ via pi-ai
- **Modes**: 4 (Interactive, Print/JSON, RPC, SDK)
- **Package ecosystem**: npm keyword `pi-package`

## Architecture at a Glance

```
pi-mono/
┌─────────────────────────────────────────────────────────────────┐
│                         packages/                                │
│                                                                  │
│  ┌──────────────┐  ┌──────────────────┐  ┌──────────────────┐  │
│  │    pi-ai     │  │  pi-agent-core   │  │     pi-tui       │  │
│  │  Unified LLM │  │  Agent runtime,  │  │  Terminal UI,    │  │
│  │  API, 15+    │──│  tool calling,   │──│  differential    │  │
│  │  providers   │  │  state, events   │  │  rendering       │  │
│  └──────────────┘  └──────────────────┘  └──────────────────┘  │
│          │                   │                     │             │
│          ▼                   ▼                     ▼             │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                  pi-coding-agent (CLI)                     │  │
│  │  4 tools: read, write, edit, bash                         │  │
│  │  Extensions · Skills · Packages · 4 Modes                 │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                  │
│  ┌──────────────┐  ┌──────────────────┐  ┌──────────────────┐  │
│  │   pi-web-ui  │  │      pi-mom      │  │     pi-pods      │  │
│  │  Web chat    │  │  Slack bot that   │  │  CLI for vLLM    │  │
│  │  components  │  │  delegates to pi  │  │  on GPU pods     │  │
│  └──────────────┘  └──────────────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## What Pi Deliberately Does NOT Have Built-in

| Feature | Pi's Alternative |
|---------|-----------------|
| MCP support | Use skills with CLI tools and READMEs, or build via extension |
| Sub-agents | Spawn pi instances via tmux, or build via extension |
| Permission popups | Run in container, or build via extension |
| Plan mode | Write plans to files, or build via extension |
| Built-in to-dos | Use TODO.md, or build via extension |
| Background bash | Use tmux |

Every deliberate omission has a simple workaround and can be fully implemented via the extension API. This is the core design insight: **the extension system IS the feature set**.

## Ecosystem Highlights

- **Active Discord community** with real-time discussion and package sharing
- **Third-party packages**: pi-skills, pi-messenger, pi-mcp-adapter, pi-web-access
- **Comparison projects**: pi-vs-claude-code benchmarking repos
- **Curated list**: awesome-pi-agent
- **Multi-agent integration**: Works with orchestrators like Overstory and Agent of Empires
- **Install from anywhere**: `pi install npm:@foo/pi-tools` or `pi install git:github.com/user/repo`

## Files in This Research

| File | Description |
|------|-------------|
| [architecture.md](architecture.md) | Monorepo structure, pi-ai provider system, pi-agent-core, pi-tui, the 4 modes |
| [agentic-loop.md](agentic-loop.md) | Minimal agent loop, message queue, steering vs follow-up, compaction |
| [tool-system.md](tool-system.md) | The 4 default tools, extension API, skills system, packages, why no MCP |
| [context-management.md](context-management.md) | AGENTS.md/SYSTEM.md, compaction, skills, prompt templates, dynamic context, sessions |
| [unique-patterns.md](unique-patterns.md) | Radical extensibility, deliberate omissions, cross-provider handoff, tree sessions |
| [benchmarks.md](benchmarks.md) | Community adoption metrics, ecosystem growth, comparison projects |
| [references.md](references.md) | Source repos, blog posts, Discord, npm packages, documentation |
