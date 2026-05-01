# grok-cli — Context Management

## Sessions

Conversations persist across CLI invocations. `--session latest` or
`-s <id>` resumes; works in both interactive and headless modes.
Session storage lives under the `.grok/` project state.

## Tool Round Cap

`--max-tool-rounds N` caps how many tool-execution rounds a headless
run will take. The README example uses 30; the default isn't explicitly
stated for interactive mode (which is effectively user-bounded by Ctrl-C).

## Batch API Mode

`--batch-api` routes the run through xAI's Batch API. This trades
latency for cost — good for "review the repo overnight" or scheduled
unattended runs. Schedules running under the daemon often pair with
`--batch-api` for low-priority recurring work.

## Sub-Agent Context Isolation

Sub-agents run with their own context windows. The main loop sees only
the result that comes back — the sub-agent's working context (file
reads, search results, intermediate reasoning) doesn't leak upward. This
is the standard reason sub-agents exist in modern agents: you can let a
"deep dive" subagent burn 50K tokens exploring a codebase and the parent
loop only pays the cost of the summary.

The `delegate` background variant takes this further — it runs
asynchronously without blocking the main loop, and returns its result
when ready.

## NDJSON Event Stream

`--format json` exposes the loop as a stream of typed events:

```json
{"type":"step_start","step":3}
{"type":"text","content":"Looking at the failing test..."}
{"type":"tool_use","name":"bash","args":{...}}
{"type":"step_finish","step":3}
{"type":"error","message":"..."}
```

Useful for piping into other tools, observability, or building UIs on
top of grok-cli's loop.

## Telegram Channel as Side Channel

Telegram messages are inputs to the same conversation, not a separate
context. This means context budget is shared — a long Telegram-driven
session can fill the model's window just like an interactive one.

## What's Not Documented

- Compaction strategy (does grok-cli compact, or does it just stop?)
- Token budget policy across sub-agents
- Per-provider tokenisation
- Session storage format / size limits

---

*From `superagent-ai/grok-cli` README. Internal compaction details are
not foregrounded in public docs.*
