# Letta Code — Context Management

## Three Layers

A Letta Code agent's effective context is composed of three layers:

```
┌──────────────────────────────────────┐
│ Memory Blocks  (always-on, editable) │  persistent
├──────────────────────────────────────┤
│ Thread / Conversation messages       │  per-session
├──────────────────────────────────────┤
│ Tool outputs in flight               │  per-turn
└──────────────────────────────────────┘
```

The memory-blocks layer is what differentiates Letta from session-based
agents — it survives `/clear`, model swaps, surface swaps (CLI ↔ desktop
↔ mobile), and time.

## Memory Blocks

Each block is a labelled piece of text with a maximum size. Standard
labels include `human` and `persona`; users and agents can add custom
ones (e.g. `project-conventions`, `current-task`, `dependencies`).

The agent can rewrite blocks with tool calls. Rewrites persist. Over
time, the agent "learns" by curating its own blocks — the original
MemGPT premise, productised.

## Thread Resets

`/clear` resets only the thread. The next turn sees:

- the same memory blocks,
- the same skills directory,
- the same subagent states,
- but no prior conversational messages.

Practically this is closer to "new chat with a coworker who knows you"
than "start over with a stranger".

## Paging / Long-Context Strategy

Letta's underlying framework handles long-context by paging older thread
content out of the live context window into "recall" storage that the
agent can retrieve from. The Letta Code README does not detail the
specific strategy, but the parent framework's MemGPT roots imply
hierarchical memory with main-context and external-context tiers.

## Multi-Surface Consistency

Because state is server-side, opening the same agent from the CLI, the
desktop app, the mobile interface, or a Slack DM all produce the same
context. There is no per-surface context divergence to manage.

## Token Counting

Not explicitly documented per provider. The framework abstracts model
calls behind its own normalised representation, so tokenisation differences
across providers don't surface to the user.

## Limits

- Maximum memory block size is configurable but bounded
- Total agent state can grow large; cleanup / archiving strategies are
  framework-level concerns documented in `docs.letta.com`
- The Letta Code README does not publish a hard turn cap

---

*Context model from the MemGPT/Letta papers and `docs.letta.com`. CLI
specifics from `letta-ai/letta-code` README.*
