# Terminus 2 — Context Management

## The Chat Buffer

Context is a single `Chat` object that grows monotonically:

- system prompt (from `terminus-{json,xml}-plain.txt`),
- the task instruction,
- model response → tmux output → model response → tmux output → …

There is **no compaction, summarisation, or sliding window**. The buffer
grows until either the task completes or `ContextLengthExceededError` is
raised by `LiteLLM`.

## Context-Length Handling

When the LLM raises `ContextLengthExceededError` mid-episode, the agent
records a `FailureMode` and terminates the run for that task. The harness
attributes the failure to the model, not the harness.

This is why TB2 entries for small-context models (`GPT-5-Nano`,
`GPT-OSS-20B`, `Claude Haiku 4.5`) score so much lower as Terminus 2: the
scaffold has no escape valve for context overflow.

## Output-Length Handling

`OutputLengthExceededError` is treated the same way — recorded as a failure
mode. There is no automatic continuation prompt.

## Tmux Output Truncation

The agent does not trim or filter tmux pane captures before appending them
to the chat. A noisy build log, a `cat` of a large file, or an interactive
TUI redraw all land verbatim in context. The result is that long-running
tasks (large test suites, multi-stage builds) blow context budgets very
quickly on smaller models — which is, again, intended scaffold behaviour
for benchmarking.

## Timestamped Markers

```python
self._timestamped_markers: list[tuple[float, str]] = []
```

This buffer captures `(time, marker)` tuples used for timeline reconstruction
when a run is replayed in the TB2 dashboard. It is not part of the LLM's
context — it is recording metadata.

## What's Missing

- No retrieval (RAG) over the repo
- No embeddings, no repo map
- No memory blocks à la Letta
- No `AGENTS.md` / `CLAUDE.md` ingestion
- No prior-turn summarisation
- No tool-output filtering

---

*Behaviour read from the chat construction in `terminus_2.py`. Where the
public source is silent on a knob, that's noted explicitly above.*
