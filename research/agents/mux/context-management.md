# Mux — Context Management

## Two Compaction Surfaces

Mux exposes two compaction mechanisms:

| Mechanism | When | Trigger |
|---|---|---|
| `/compact` | User-initiated | Slash command in composer |
| Opportunistic compaction | Auto | Workspace-managed, between tool batches / todos |

Documented at <https://mux.coder.com/workspaces/compaction>. Opportunistic
compaction is the Mux-original of the two — it tries to find low-cost
moments to summarise so the user doesn't have to.

## Per-Workspace Context

Each workspace owns its own conversation, todo list, and compaction state.
Switching workspaces in the sidebar swaps in a different conversation; the
others continue running asynchronously.

This means a "context budget" is a workspace concept, not a global one. If
you have five workspaces open against the same repo, each is consuming its
own model's context window separately.

## Instruction Stripping

Scoped instruction sections (`Model: ...`, `Tool: ...`) are **stripped
from the general `<custom-instructions>` block** and re-injected only
where they apply. This keeps the always-on prompt slim while still letting
authors write everything in one `AGENTS.md`.

## Todo as Sidebar State

The `todo_write` / `todo_read` tools store state that the Mux sidebar
reads directly to render progress. This makes the todo list dual-purpose:
LLM working memory and user-facing UI element. Practically, this means
prompts often instruct the model to keep the todo list current
(see the example in `tool-system.md`).

## Models with Long vs. Short Contexts

Mux runs the same loop under models with very different windows
(Sonnet ~200K, Opus ~200K, GPT-5.x ~256K, Grok variable). Compaction —
especially opportunistic — is what lets the same UI behave consistently
across them.

## What's Not Documented

- Token-counting strategy (which encoder? per provider?)
- Whether compaction summarises tool outputs, file contents, or both
- Maximum turns / messages before forced compaction
- Persistence across app restart — implied (sessions resume) but not
  detailed

---

*From the public docs at `mux.coder.com`. The internal compaction
algorithm is in the AGPL repo but not detailed here.*
