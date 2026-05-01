# Deep Agents — Context Management

## Two Strategies

Deep Agents handles long-running conversations with two layered
strategies, both automatic:

1. **Auto-summarisation** — older turns get summarised when the
   conversation grows long.
2. **Files as overflow** — tool outputs that would blow context get
   written to disk; the model sees a path / preview instead of the raw
   content.

The user doesn't trigger either — the harness does it.

## Why Files-as-Overflow Matters

Most coding agent context bloat comes from one place: tool outputs.
A `cat` of a generated file, a verbose `pytest -v`, a `find` over a
node_modules tree — any of these can dump tens of thousands of tokens
into the conversation in one tool call.

Deep Agents' answer: the harness intercepts large outputs, writes them
to disk, and replaces the in-context payload with a path plus enough
preview to be useful. The agent can then `grep`/`read_file` the parts
it actually needs.

This is a much cheaper pattern than summarising-everything, and it
matches how humans use shell pipelines (`> output.log`, then grep).

## Auto-Summarisation

The README says only that "auto-summarization when conversations get
long" is included. Specifics (threshold, summarisation prompt,
which messages get rewritten) live in the source and the LangChain
docs. The summarisation step is built into the LangGraph node that
manages the chat history, so it benefits from the runtime's
checkpointing — summarised state survives restarts.

## LangGraph Checkpointing

Because the loop is a LangGraph, conversation state can be persisted to
any LangGraph-supported checkpointer (in-memory, SQLite, Postgres,
custom). For long agent runs, this means restart-safety: the harness
can be resumed without losing context.

The CLI ships with persistent memory enabled by default.

## Per-Sub-Agent Context

Each `task`-spawned sub-agent has its own context window. The sub-agent
can burn through 50K tokens of exploration without any of it leaking
into the parent's context — only the returned summary does. This is
critical for the "deep dive" pattern.

## ACP / Editor Surface

When driven from an editor via ACP, the editor itself often becomes
the user's view of context — file edits applied through `edit_file`
show up in the editor, and the conversation is the side-panel. Context
remains server-side regardless of surface.

## Token Counting

LangGraph / LangChain handle tokenisation per provider. Deep Agents
inherits this — different models with different tokenisers Just Work.

## What's Not Documented

- Exact summarisation threshold
- Whether summarisation is per-turn or per-N-turns
- How the file-overflow heuristic decides "too big"

Likely all configurable via the customisation API but not surfaced in
the README.

---

*From the README and `docs.langchain.com/oss/python/deepagents/`.*
