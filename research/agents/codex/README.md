---
title: Codex CLI Architecture Analysis
status: complete
---

# Codex CLI

> OpenAI's official terminal-native coding agent — open-source, Rust-native, with OS-level sandboxing. **#1 on Terminal-Bench 2.0** (82.0% with GPT-5.5, 2026-04-23).

## Overview

**Codex CLI** is OpenAI's locally-running coding agent for the terminal. It reads,
modifies, and executes code on your machine inside a sandboxed environment.
Originally built in TypeScript (the `codex-cli/` directory, now legacy), the
production implementation is written entirely in **Rust** (`codex-rs/`), compiled
to a single static binary with zero runtime dependencies.

| Property | Value |
|---|---|
| **Repository** | [github.com/openai/codex](https://github.com/openai/codex) |
| **License** | Apache-2.0 |
| **Language** | Rust (70+ crates in a Cargo workspace) |
| **UI Framework** | [Ratatui](https://ratatui.rs/) (full-screen TUI) |
| **Build System** | Cargo + Bazel (dual build support) |
| **Install** | `npm i -g @openai/codex` / `brew install --cask codex` |
| **Platforms** | macOS, Linux, Windows (WSL + experimental native) |
| **API** | OpenAI Responses API (streaming SSE + WebSocket) |
| **MCP Support** | Client (connects to MCP servers) + Server (`codex mcp-server`) |

## OpenAI's Vision

Codex CLI is part of OpenAI's "Codex" family that spans three form factors:

1. **Codex CLI** — local terminal agent (this project, open-source)
2. **Codex IDE** — VS Code / Cursor / Windsurf extension
3. **Codex Cloud** — cloud-hosted agent at chatgpt.com/codex

The CLI is designed for developers who "live in the terminal" and want ChatGPT-level
reasoning with the ability to actually execute code, manipulate files, and iterate —
all under version control and OS-level sandboxing.

## Language & Stack

The codebase is a **Cargo workspace with 70+ crates**, organized as a monorepo:

| Crate | Role |
|---|---|
| `codex-core` | Central orchestration — agent loop, session management, model client |
| `codex-protocol` | Shared types: SQ/EQ protocol, config enums, model metadata |
| `codex-cli` | Binary entry point + subcommand routing via `clap` |
| `codex-tui` | Full-screen terminal UI built with Ratatui |
| `codex-exec` | Non-interactive `codex exec` mode for automation/CI |
| `codex-exec-server` | JSON-RPC exec server (WebSocket/stdio transport) |
| `codex-config` | Layered configuration system (TOML-based) |
| `codex-execpolicy` | Command approval policy engine with prefix-rule matching |
| `codex-linux-sandbox` | Linux sandbox: bubblewrap + seccomp + optional Landlock |
| `codex-api` | Typed client for OpenAI Responses, Compaction, Memory APIs |
| `codex-mcp-server` | Codex as an MCP server for other agents |
| `windows-sandbox-rs` | Windows sandbox: ACLs + firewall + restricted tokens |
| `process-hardening` | Pre-main security: disable ptrace, core dumps, LD_PRELOAD |

Key ecosystem dependencies: `tokio`, `ratatui`, `clap`, `reqwest`, `serde`,
`landlock`, `seccompiler`, and vendored forks of `crossterm`/`ratatui`.

## Sandboxing Model (Key Differentiator)

Codex CLI's **defining feature** is its multi-layered OS-level sandboxing that runs
entirely locally — no containers, VMs, or cloud isolation needed:

### Three Sandbox Modes

| Mode | Filesystem | Network |
|---|---|---|
| `read-only` | Entire FS read-only | Blocked |
| `workspace-write` (default) | CWD writable, rest read-only | Blocked |
| `danger-full-access` | No restrictions | Allowed |

### Platform-Specific Enforcement

- **macOS**: Apple Seatbelt (`sandbox-exec`) with custom profiles. Read-only jail
  with explicit writable roots (`$PWD`, `$TMPDIR`, `~/.codex`). All network blocked.
- **Linux**: Bubblewrap (mount/user/PID/network namespaces) + seccomp syscall
  filtering + optional Landlock. Blocks `ptrace`, `io_uring`, and all network
  syscalls except `AF_UNIX`.
- **Windows**: Dedicated sandbox user accounts + Windows ACL Deny ACEs +
  Windows Firewall per-SID outbound block rules + alternate desktop isolation +
  restricted security tokens.

### Protected Paths

Even in `workspace-write` mode, sensitive paths remain read-only:
- `.git/` (and resolved `gitdir:` targets)
- `.codex/` and `.agents/` directories
- Protection is recursive

### Process Hardening (Pre-Main)

Applied via `#[ctor::ctor]` before `main()`:
- **Linux**: `PR_SET_DUMPABLE=0`, `RLIMIT_CORE=0`, strip all `LD_*` env vars
- **macOS**: `PT_DENY_ATTACH`, `RLIMIT_CORE=0`, strip all `DYLD_*` env vars
- Prevents debugger attachment, core dumps, and shared-library injection

## Terminal-Bench 2.0 Scores

| Rank | Model | Score | Date |
|---|---|---|---|
| **#1** | **GPT-5.5** | **82.0% ±2.2** | 2026-04-23 |
| #9 | GPT-5.3-Codex (as "Simple Codex") | 75.1% ±2.4 | 2026-02-06 |
| #29 | GPT-5.2 | 62.9% ±3.0 | 2025-12-18 |
| #35 | GPT-5.1-Codex-Max | 60.4% ±2.7 | 2025-11-24 |
| #55 | GPT-5 | 49.6% ±2.9 | 2025-11-04 |
| #59 | GPT-5-Codex | 44.3% ±2.7 | 2025-11-04 |
| #88 | GPT-5-Mini | 31.9% ±3.0 | 2025-11-04 |
| #119 | GPT-5-Nano | 11.5% ±2.3 | 2025-11-04 |

The GPT-5.5 entry (submitted 2026-04-23) is currently the top result on the entire Terminal-Bench 2.0 leaderboard, dethroning ForgeCode + GPT-5.4 (#2 at 81.8%) and TongAgents + Gemini 3.1 Pro (#3 at 80.2%).

## Key Features

- **Interactive TUI**: Full-screen terminal UI with syntax highlighting, diff
  rendering, theme support (`/theme`), and image input
- **Non-interactive mode**: `codex exec PROMPT` for CI/scripting
- **Resume sessions**: `codex resume` picks up prior transcript and context
- **Sub-agents**: Spawn parallel agent threads for complex multi-file tasks
- **Web search**: Built-in tool with cached/live/disabled modes
- **Code review**: Dedicated `/review` command with branch diff, uncommitted
  changes, and commit review modes
- **MCP client + server**: Connect to external tools; run Codex as a tool for
  other agents
- **Multi-model**: GPT-5.4 (recommended), GPT-5.3-Codex, GPT-5.2-Codex,
  Ollama, LM Studio, and any OpenAI-compatible provider
- **Approval policies**: `on-request` (default), `untrusted`, `never`,
  granular per-category
- **Enterprise features**: OTel telemetry, managed config requirements (MDM),
  custom CA certificates, SQLite state persistence
- **Slash commands**: `/review`, `/fork`, `/model`, `/permissions`, `/clear`,
  `/copy`, `/status`, custom user-defined commands

## Architecture at a Glance

```
┌─────────────┐    SQ/EQ     ┌──────────────┐
│  TUI / CLI  │◄────────────►│  codex-core   │
│  (Ratatui)  │  (channels)  │  (Session)    │
└─────────────┘              │               │
                             │  ┌──────────┐ │     ┌─────────────────┐
                             │  │ToolRouter│─┼────►│ ExecPolicy      │
                             │  └──────────┘ │     │ (rule matching) │
                             │               │     └─────────────────┘
                             │  ┌──────────┐ │     ┌─────────────────┐
                             │  │ToolOrch. │─┼────►│ SandboxManager  │
                             │  └──────────┘ │     │ (OS-level)      │
                             │               │     └─────────────────┘
                             │  ┌──────────┐ │     ┌─────────────────┐
                             │  │ContextMgr│─┼────►│ codex-api       │
                             │  └──────────┘ │     │ (Responses API) │
                             └──────────────┘     └─────────────────┘
```

The SQ/EQ (Submission Queue / Event Queue) pattern decouples the UI from the
agent core. The UI submits `Op` variants (user input, approvals, config changes)
and receives `EventMsg` variants (agent messages, approval requests, token usage).

## Recent Updates (March–April 2026)

Highlights from `rust-v0.125.0` → `rust-v0.128.0` (released through late April 2026):

- **Persisted `/goal` workflows** — long-running goals are now first-class objects with create/pause/resume/clear, app-server APIs, model tools, and TUI controls.
- **`codex update`** — built-in self-update command; the long-standing `--full-auto` flag is now deprecated in favor of explicit permission profiles.
- **Expanded permission profiles** — built-in defaults, sandbox CLI profile selection, cwd controls, and active-profile metadata exposed to clients.
- **Plugin marketplace** — installation from a remote marketplace, remote bundle caching, plugin-bundled hooks, hook enablement state, and external-agent config import.
- **External agent session import** — Codex can import sessions originating from other agents (background imports, imported-session title handling).
- **MultiAgentV2 hardening** — explicit thread caps, wait-time controls, root/sub-agent hints, and v2-specific depth handling.
- **GPT-5.5 / `gpt-image-2`** — bundled OpenAI Docs skill refreshed; Bedrock GPT-5.4 endpoint/model metadata corrected; Bedrock GPT-5.4 `apply_patch` regression fixed.

## See Also

- [Architecture](architecture.md) — Full architecture deep-dive
- [Agentic Loop](agentic-loop.md) — Agent loop implementation details
- [Tool System](tool-system.md) — Tools, sandboxing, and execution policy
- [Context Management](context-management.md) — Token management and compaction
- [Unique Patterns](unique-patterns.md) — Key differentiators vs other agents
- [Benchmarks](benchmarks.md) — Performance data
- [References](references.md) — Links and resources
