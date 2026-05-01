# Letta Code — Unique Patterns

## 1. Persisted Agent, Not Session

Every other agent on the TB2 leaderboard treats a CLI invocation as a new
context. Letta Code treats it as a continuation of the same agent. This
is the central organising claim of the project, and the README states it
explicitly:

> *"Every conversation is like meeting a new contractor"* (sessions)
> vs.
> *"Like having a coworker or mentee that learns and remembers"* (Letta).

## 2. Memory Blocks as Editable Always-On Context

Memory blocks are labelled, agent-editable text that lives outside the
thread but in the prompt. The agent owns its own context. Closest analog
in other tools is `CLAUDE.md` / `AGENTS.md`, but those are user-edited,
static, and not part of the model's tool surface.

## 3. Skill Learning (`/skill`)

A first-party verb for *self-distillation*: the agent reflects on what
it just did and writes a reusable skill file. Closest analog elsewhere
is "ask Claude Code to write a skill for this" — but Letta builds it
into the workflow.

## 4. Model-Portable Memory

`/model` swaps the LLM but keeps the agent. Memory blocks, thread, and
skills are model-neutral. This makes "GPT for cheap exploration, Opus for
hard edits" trivial — you don't lose anything when switching.

## 5. Same Agent Across Surfaces

CLI, desktop, mobile, Slack, Telegram, Discord all see the same agent.
The CLI is one of many faces; the agent doesn't know which surface is
talking to it.

## 6. MemGPT Lineage

Letta is the rebrand of MemGPT (Packer et al., 2023, "MemGPT: Towards
LLMs as Operating Systems"). The hierarchical-memory ideas from that
paper are embedded in the framework. Letta Code is the
coding-specifically-skinned product on top of those ideas.

## 7. Server-Client Split

The CLI is a client; the server holds the state. Most coding agents are
self-contained processes. Letta's split is closer to LSPs or Jupyter
kernels — and is what enables the multi-surface story.

## Comparison

| Pattern | Letta Code | Claude Code | Codex CLI | Mux |
|---|---|---|---|---|
| State lifetime | persistent | session | session | session per workspace |
| `/clear` resets | thread only | everything | everything | everything |
| Memory editing | agent tool calls | human-edited file | human-edited | human-edited |
| Multi-surface same agent | ✅ | ❌ | ❌ | ❌ |
| Skill learning verb | ✅ | ❌ | ❌ | ❌ |
| Model swap preserves state | ✅ | n/a | n/a | per workspace |

---

*Patterns from Letta Code README and the broader Letta / MemGPT
literature.*
