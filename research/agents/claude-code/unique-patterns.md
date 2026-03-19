# Claude Code — Unique Patterns & Key Differentiators

> What makes Claude Code architecturally distinct from other coding agents (Copilot CLI, Cursor, Aider, Cline, etc.).

## 1. Graduated Permission Model

**What**: A multi-level trust system with five permission modes and per-tool allow/ask/deny rules.

**How it works**:
- `default` → prompts for every risky action
- `acceptEdits` → auto-approves file edits, still asks for Bash commands
- `plan` → read-only mode, Claude can only analyze
- `dontAsk` → auto-denies everything unless pre-approved
- `bypassPermissions` → skips all prompts (for CI/automation)

Users cycle through modes with `Shift+Tab` during a session. Rules can be configured at managed, project, local, and user scopes with deny-wins-always precedence.

**Why it matters**: Most coding agents have binary permission models (on/off). Claude Code's graduated system allows users to ratchet trust up or down based on the task. Combined with glob-pattern tool matching and OS-level sandboxing, this is the most sophisticated permission model in any coding agent.

**Key detail**: Permission rules for Bash support wildcards at any position (`Bash(npm run *)`, `Bash(* --version)`), Read/Edit rules follow gitignore spec, and WebFetch supports domain patterns.

## 2. Ink-Based TUI (React for the Terminal)

**What**: The CLI interface is built with Ink, a React renderer for terminal output.

**How it works**: The UI is a React component tree rendered to the terminal via Ink. Input handling, diff rendering, spinners, progress bars, and permission dialogs are all React components.

**Why it matters**: This is architecturally novel for a coding agent. Benefits:
- Component-based UI with familiar React patterns
- Potentially easier to build complex interactive TUI features
- Shared mental model with the web development ecosystem
- Enables features like inline diff rendering, progress bars, and interactive permission dialogs

**Competitors**: Most coding agents use plain terminal output or simpler TUI libraries. Cursor has a full IDE (Electron). Aider uses standard Python terminal output. This makes Claude Code's approach unique in the space.

## 3. CLAUDE.md — Project Memory Files

**What**: A markdown file that Claude reads at the start of every session, providing persistent project instructions.

**How it works**:
- `CLAUDE.md` or `.claude/CLAUDE.md` in project root — shared with team via git
- `~/.claude/CLAUDE.md` — personal instructions for all projects
- Parent/child directory CLAUDE.md files — hierarchical inheritance
- Managed policy CLAUDE.md — organization-wide, cannot be excluded
- `@path/to/import` syntax for file imports
- `.claude/rules/` for modular, path-scoped instructions

**Why it matters**: Other agents have configuration files, but CLAUDE.md is uniquely designed as a human-readable instruction file that shapes agent behavior. It's treated as context (loaded into the conversation), not configuration (parsed by the harness). This means:
- Natural language instructions, not YAML/JSON schemas
- Team-shared via git, building collective knowledge
- Survives compaction (re-read from disk after `/compact`)
- Hierarchical (monorepo-friendly)

**Comparison**: Cursor has `.cursorrules`. GitHub Copilot has custom instructions. But Claude Code's implementation is more complete — imports, path-scoped rules, hierarchical loading, managed policy level, and the auto-memory complement.

## 4. Auto Memory — Claude Learns from Corrections

**What**: Claude automatically saves notes for itself as it works, building project knowledge across sessions.

**How it works**:
- Stored in `~/.claude/projects/<project>/memory/MEMORY.md` + topic files
- Claude decides what's worth remembering (build commands, debugging insights, patterns)
- First 200 lines of MEMORY.md loaded at session start
- Topic files loaded on demand
- Toggle with `/memory` command
- Machine-local, not shared

**Why it matters**: This is a unique bidirectional memory system. CLAUDE.md = human → agent instructions. Auto-memory = agent → agent notes. Combined, they create a persistent knowledge base that improves over time without manual effort.

## 5. `/compact` — Manual Context Compaction

**What**: A user-triggered command to summarize and compress conversation history.

**How it works**:
```
/compact                            # General compaction
/compact Focus on the API changes   # Directed — prioritize specific content
/compact Keep all test results      # Selective — preserve certain information
```

**Why it matters**: Most agents either auto-compact silently or let context overflow. Claude Code gives users explicit control with focus instructions. Combined with auto-compaction, selective rewind summarization, `/clear`, and `/btw`, this provides five distinct granularities of context management.

## 6. Sub-Agent Spawning with Context Isolation

**What**: Claude Code spawns child agents that run in separate context windows with independent permissions, tools, and models.

**How it works**:
- **Built-in agents**: Explore (Haiku, read-only), Plan (inherits, read-only), General-purpose (inherits, all tools)
- **Custom agents**: `.claude/agents/` markdown files with YAML frontmatter
- **Isolation**: Each sub-agent gets its own context window — work doesn't pollute main conversation
- **No nesting**: Sub-agents cannot spawn other sub-agents (prevents infinite recursion)
- **Worktree isolation**: Sub-agents can run in temporary git worktrees (`isolation: worktree`)
- **Persistent memory**: Sub-agents can maintain their own auto-memory across sessions

**Custom sub-agent example**:
```yaml
---
name: security-reviewer
description: Reviews code for security vulnerabilities
tools: Read, Grep, Glob, Bash
model: opus
memory: project
---
You are a senior security engineer. Review code for:
- Injection vulnerabilities (SQL, XSS, command injection)
- Authentication and authorization flaws
- Secrets or credentials in code
```

**Why it matters**: Sub-agents serve dual purposes — task specialization AND context isolation. The Explore agent using Haiku is a deliberate cost/speed optimization. The ability to give each sub-agent its own tools, model, permission mode, MCP servers, and persistent memory makes this the most complete sub-agent system in any coding agent.

## 7. MCP Server Support (Model Context Protocol)

**What**: Native integration with MCP, an open standard for AI-tool integrations, connecting Claude Code to hundreds of external services.

**How it works**:
- Supports HTTP, SSE, stdio, and WebSocket transports
- Scoped at local, project, and user levels
- OAuth 2.0 authentication for remote servers
- Tool Search: dynamically loads MCP tools on demand when too many are configured
- Claude Code itself can serve as an MCP server (`claude mcp serve`)
- MCP resources accessible via `@` mentions
- MCP prompts available as commands

**Why it matters**: Claude Code was one of the first coding agents to adopt MCP natively. The integration is deep — scoped configuration, auth, tool search for context efficiency, elicitation dialogs, and bidirectional (Claude Code can both consume and serve MCP).

## 8. Hooks — Deterministic Lifecycle Scripts

**What**: Shell scripts that run at specific lifecycle events, providing hard enforcement (unlike advisory CLAUDE.md).

**Events**: PreToolUse, PostToolUse, SessionStart, Notification, WorktreeCreate, WorktreeRemove, Elicitation

**Example use cases**:
- Run eslint after every file edit
- Block writes to the migrations folder
- Send desktop notifications when Claude needs input
- Validate that Bash commands are read-only SQL queries

**Why it matters**: Hooks bridge the gap between "Claude should do X" (CLAUDE.md) and "Claude MUST do X" (hooks). They're deterministic — a PreToolUse hook that exits with code 2 blocks the tool call regardless of what Claude decides. This is enterprise-grade enforcement.

## 9. Skills — On-Demand Knowledge Packages

**What**: Modular knowledge units in `.claude/skills/` that load only when relevant, saving context.

**How it works**:
```yaml
---
name: api-conventions
description: REST API design conventions for our services
---
# API Conventions
- Use kebab-case for URL paths
- Use camelCase for JSON properties
```

- Claude sees skill descriptions at session start (cheap)
- Full content loads only when the skill is invoked or matched
- `disable-model-invocation: true` for manual-only skills
- Skills can define repeatable workflows (e.g., `/fix-issue 1234`)

**Why it matters**: Skills solve the context budget problem for project knowledge. Instead of putting everything in CLAUDE.md (always loaded, expensive), skills load on demand. This is a deliberate design tension — CLAUDE.md for universal rules, skills for situational knowledge.

## 10. Plan Mode — Analysis Before Action

**What**: A read-only mode where Claude can analyze the codebase and create a plan before any modifications.

**How it works**:
- Activated via `Shift+Tab` (cycle twice) or `--permission-mode plan`
- Claude uses only read-only tools — no edits, no writes, no risky Bash commands
- Uses `AskUserQuestion` to gather requirements interactively
- Plan sub-agent does codebase research
- Plan can be refined through conversation before execution

**Why it matters**: Separating analysis from execution is a safety and quality pattern. It prevents Claude from making premature changes and gives users confidence that exploration won't have side effects.

## 11. Checkpointing and Rewind

**What**: Every file edit is snapshotted, enabling rewind to any previous state.

**How it works**:
- Snapshots taken before every edit (not git-based, separate system)
- `Esc + Esc` or `/rewind` opens the rewind menu
- Can restore: conversation only, code only, or both
- Can summarize from a checkpoint (selective compaction)
- Checkpoints persist across sessions

**Why it matters**: Combined with the `Esc` interrupt, this enables a "try and rewind" workflow. Users can let Claude attempt risky changes knowing they can always roll back. This lowers the cost of experimentation.

## 12. Multi-Surface, Single Engine

**What**: The same Claude Code engine runs across terminal, VS Code, JetBrains, Desktop app, Web, Slack, GitHub Actions, and Chrome.

**How it works**: CLAUDE.md, settings, MCP servers, and skills work identically across all surfaces. The interface changes (terminal vs. IDE sidebar vs. web UI) but the underlying agentic loop is the same.

**Why it matters**: This is a platform strategy, not just a CLI tool. Users can start in the terminal, continue on mobile (web/iOS), automate in CI (GitHub Actions), and integrate into chat (Slack). No other coding agent has this breadth.

## Summary Comparison

| Pattern | Claude Code | Most Competitors |
|---------|------------|-----------------|
| Permission model | 5 modes + per-tool rules + sandboxing | Binary (on/off) or basic allow/deny |
| TUI framework | React (Ink) | Plain terminal output |
| Project memory | CLAUDE.md + auto-memory + skills + rules | Config file or system prompt |
| Context compaction | 5 granularities (auto, /compact, rewind-summarize, /clear, /btw) | Auto-compact or none |
| Sub-agents | Built-in + custom, own context/tools/model/memory | None or basic |
| External tools | MCP (native, multi-transport, tool search) | Custom integrations |
| Lifecycle hooks | Deterministic shell scripts at 7 events | None or limited |
| Analysis mode | Full plan mode with sub-agent research | None |
| Checkpointing | Every edit, persistent, rewind menu | Git-based or none |
