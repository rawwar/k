# Mux — Unique Patterns

## 1. Parallel Workspaces as the Primary UX

Mux's defining choice is that **the unit of work is a workspace, not a
session**. The sidebar lets you spin up many at once, each with its own
runtime, model, and conversation. Most coding agents are mono-session;
Mux is built around the assumption that you want N agents at once.

## 2. Three Runtime Backends

Local / git-worktree / SSH gives you a clean ladder: try changes in your
working directory, parallelise across branches, or push the whole loop
onto a remote box — without switching tools.

## 3. Opportunistic Compaction

Most agents either compact never, on `/compact`, or at hard token
thresholds. Mux looks for *natural pauses* (between tool batches, after
todo completion) and compacts there. The cost is hidden from the user.

## 4. Scoped Instructions with `Model:` and `Tool:`

The instruction file format borrows the `AGENTS.md` convention but extends
it with regex-scoped sections. A single file can encode model-specific
tone, model-specific guardrails, and per-tool conventions, without
duplicating the global preamble for each model.

This is more expressive than Claude Code's `CLAUDE.md` (which is global)
and is a thoughtful answer to multi-model environments.

## 5. AGENTS.md Compatibility Cascade

Mux looks for `AGENTS.md → AGENT.md → CLAUDE.md` in order. This means a
repo configured for Claude Code works in Mux without changes, and a repo
configured for Mux degrades cleanly back to Claude Code. Friendly to
mixed-tool teams.

## 6. Todo as Dual-Purpose State

`todo_write` is both the LLM's working scratchpad and the source of truth
for the sidebar's progress UI. Tying user-visible state to a tool the
model is told to maintain is unusual; it's why the example AGENTS.md
includes "keep the TODO list current".

## 7. VS Code Extension

A small but pragmatic feature: jump from Mux into the editor at the file
the agent is currently mutating. Lower-friction human-in-the-loop than
"alt-tab to your editor and find the file".

## Comparison

| Pattern | Mux | Claude Code | Codex CLI | OpenHands |
|---|---|---|---|---|
| Parallel workspaces | ✅ first-class | manual (multiple terminals) | manual | server-managed |
| Worktree runtime | ✅ first-class | ❌ | ❌ | ❌ |
| Opportunistic compaction | ✅ | ❌ (manual `/compact`) | hidden | varies |
| Scoped Model/Tool prompts | ✅ regex | ❌ | ❌ | ❌ |
| AGENTS.md → CLAUDE.md fallback | ✅ | (CLAUDE.md only) | (AGENTS.md) | varies |

---

*Patterns from public README and `mux.coder.com` docs.*
