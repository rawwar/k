---
title: Claude Code
status: complete
---

# Claude Code

> Anthropic's first-party agentic coding assistant — a terminal-native CLI agent built in TypeScript with an Ink-based TUI.

## What It Is

Claude Code is Anthropic's official CLI coding agent. It lives in the terminal, understands entire codebases, and executes multi-step coding tasks autonomously through natural language. Unlike inline autocomplete tools, Claude Code operates as a full agentic system: it reads files, runs commands, edits code, searches the web, and verifies its own work — all within a multi-turn conversation loop.

- **First released**: February 2025 (research preview), GA mid-2025
- **Distribution**: npm package (`@anthropic-ai/claude-code`), native installers, Homebrew, WinGet
- **Platforms**: macOS, Linux, Windows (WSL), plus VS Code / JetBrains extensions, Desktop app, Web UI
- **License**: Closed-source (the GitHub repo at `anthropics/claude-code` hosts the npm package, plugins, and issue tracker — not the full source)
- **Runtime**: Node.js 18+, TypeScript, Ink (React for CLI)
- **Models**: Claude Sonnet, Opus, and Haiku (user-selectable per session)

## Terminal-Bench 2.0 Rankings

| Rank | Agent + Model | Score |
|------|--------------|-------|
| #39 | Claude Code (Claude Opus 4.6) | 58.0% |
| #48 | Claude Code (Claude Opus 4.5) | 52.1% |
| #69 | Claude Code (Claude Sonnet 4.5) | 40.1% |

## Key Capabilities

- **Agentic coding loop**: Gather context → take action → verify results, repeated until task complete
- **Full codebase awareness**: Reads across files, understands project structure, makes coordinated multi-file edits
- **Permission model**: Graduated trust levels (default → auto-accept edits → plan mode → bypass) with per-tool allow/ask/deny rules
- **CLAUDE.md project memory**: Persistent instructions loaded every session, plus auto-memory for learned patterns
- **Sub-agent spawning**: Built-in Explore, Plan, and general-purpose sub-agents; custom sub-agents via `.claude/agents/`
- **MCP server support**: Connect to external tools (GitHub, Sentry, databases, Figma) via Model Context Protocol
- **Context compaction**: Automatic compaction when approaching limits, plus manual `/compact` command
- **Git integration**: Branch-aware sessions, worktree support for parallel sessions, PR creation
- **Multi-surface**: Same engine across terminal, VS Code, JetBrains, Desktop app, Web, Slack, GitHub Actions

## Architecture at a Glance

```
┌─────────────────────────────────────────────┐
│              Ink TUI (React for CLI)         │
│  ┌────────┐ ┌──────────┐ ┌───────────────┐  │
│  │ Input  │ │ Renderer │ │ Permission UI │  │
│  └────────┘ └──────────┘ └───────────────┘  │
├─────────────────────────────────────────────┤
│            Agentic Harness                   │
│  ┌─────────────┐  ┌────────────────────┐    │
│  │ Agent Loop   │  │ Context Manager   │    │
│  │ (multi-turn) │  │ (compaction,      │    │
│  │              │  │  CLAUDE.md, etc.) │    │
│  └─────────────┘  └────────────────────┘    │
├─────────────────────────────────────────────┤
│              Tool System                     │
│  Read │ Edit │ Bash │ Grep │ Glob │ Web*   │
│  Agent │ Write │ WebFetch │ MCP tools      │
├─────────────────────────────────────────────┤
│         Permission / Safety Layer            │
│  allow/ask/deny rules │ sandboxing │ hooks  │
├─────────────────────────────────────────────┤
│         Claude API (Sonnet/Opus/Haiku)       │
└─────────────────────────────────────────────┘
```

## Research Files

| File | Contents |
|------|----------|
| [architecture.md](architecture.md) | TypeScript + Ink stack, tool system, permission model, session management |
| [agentic-loop.md](agentic-loop.md) | The gather → act → verify loop, model selection, tool chaining |
| [tool-system.md](tool-system.md) | All known tools, permission levels, MCP integration |
| [context-management.md](context-management.md) | /compact, auto-compaction, CLAUDE.md, auto-memory, sub-agents for context isolation |
| [unique-patterns.md](unique-patterns.md) | Key differentiators vs. other coding agents |
| [benchmarks.md](benchmarks.md) | Terminal-Bench 2.0, SWE-bench scores, model comparisons |
| [references.md](references.md) | Official docs, blog posts, public analyses |

## Key Takeaways for Agent Builders

1. **Permission model as a first-class feature**: Claude Code's graduated trust system (default → acceptEdits → plan → bypassPermissions) is the most sophisticated permission model in any coding agent. It balances autonomy with safety.
2. **Context is the bottleneck**: The entire best-practices guide centers around one constraint — the context window fills up fast. Auto-compaction, `/compact`, sub-agents, and skills all exist to manage this.
3. **CLAUDE.md as project memory**: A simple but powerful pattern — a markdown file loaded every session that gives the agent persistent instructions. Other agents could adopt this.
4. **Sub-agent architecture**: Spawning child agents with separate context windows for exploration keeps the main conversation clean. The Explore sub-agent uses Haiku for speed.
5. **Hooks system**: Deterministic shell scripts at lifecycle points (PreToolUse, PostToolUse, etc.) complement the advisory CLAUDE.md with hard enforcement.
6. **Skills as on-demand knowledge**: Unlike CLAUDE.md (always loaded), skills load only when relevant, reducing context waste.
