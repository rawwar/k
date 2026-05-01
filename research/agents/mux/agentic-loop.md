# Mux — Agentic Loop

## Loop Lineage

Mux's README says it directly: *"Mux has a custom agent loop but much of the
core UX is inspired by Claude Code. You'll find familiar features like
Plan/Exec mode, vim inputs, `/compact` and new ones like opportunistic
compaction and mode prompts."*

So the loop is Claude-Code-shaped but not source-derived.

## Plan / Exec Mode

Plan mode is the read-only proposal phase: the agent inspects the repo,
proposes a plan via the `propose_plan` tool, and waits for user approval
before any mutations. Exec mode is the after-approval state where edit
tools become available. This is the standard Claude Code split.

In Mux it lives at the workspace level — each workspace has its own
plan/exec state — so you can have one agent in plan and three others in
exec at the same time.

## `/compact` and Opportunistic Compaction

Two compaction surfaces:

- **`/compact`** — manual command, same semantics as Claude Code
- **Opportunistic compaction** — Mux-original; the loop looks for natural
  break points (after tool batches, between todos) and offers/triggers
  compaction without waiting for the user.

This is documented at <https://mux.coder.com/workspaces/compaction> and is
the loop-level mechanism that lets long-running multi-agent sessions
survive without the user babysitting context windows.

## Vim Input Mode

The composer supports vim keybindings — useful because Mux is genuinely
keyboard-driven (sidebar navigation, workspace switching, plan toggle).

## Multi-Model Within One Loop

The same loop is used regardless of model. Models registered include
`sonnet-4-*`, `opus-4-*`, `gpt-5-*`, `grok-*`, plus Ollama and OpenRouter
adapters. The Mux release on the leaderboard ran the same loop under
GPT-5.3-Codex (74.6%, #12), Claude Opus 4.6 (66.5%, #22), GPT-5.2 (60.7%),
and Claude Opus 4.5 (58.4%) — a bigger spread than most agents publish.

## Mode Prompts

`Model: <regex>` blocks in `AGENTS.md` tune the loop's behaviour per
model — for example *"keep the todo list current every few minutes while a
task is in flight"* applied only to OpenAI Codex models. The loop reads
these at session start and injects them into a `<model-...>` tag rather
than the general custom instructions block.

## What's Public, What Isn't

The README confirms: a custom loop, plan/exec, opportunistic compaction,
todo write, vim composer. The actual loop driver code is in `coder/mux`
(AGPL-3.0) but is not enumerated in this Tier 2 page — the public docs
talk about features, not state machines.

---

*Loop description from `coder/mux` README and `mux.coder.com` docs.*
