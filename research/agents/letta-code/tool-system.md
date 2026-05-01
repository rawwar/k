# Letta Code — Tool System

## Three Tool Sources

Letta Code's tool surface comes from three places:

1. **Built-in tools** — bundled with the framework (`web_search`,
   `fetch_webpage`, file/edit primitives, shell, plus the memory-edit
   tools that operate on memory blocks).
2. **Skills** — `.skills/<name>/SKILL.md` directories, each effectively
   a packaged prompt + tool recipe. Compatible with the Claude Code
   skills convention.
3. **Subagents** — invocable as tools via the `task`-style delegation
   pattern; each subagent has its own toolset and memory.

## Memory-Edit Tools

The defining built-in tools are the ones that mutate memory blocks.
Whenever the agent decides "I should remember this", it invokes a tool
that edits a labelled memory block; the change persists immediately to
the server-side agent state. This is what lets the agent "learn" between
sessions without any RAG or fine-tuning step.

## Skills

A skill in Letta Code follows the Claude-Code pattern:

```
.skills/
  bug-triage/
    SKILL.md      # instructions, tool list, examples
    ...           # supporting files / scripts
```

The agent can opt into a skill at runtime; instructions are appended to
the active prompt and any skill-specific tools are added to the available
set.

### Skill Learning

`/skill` derives a new skill from the agent's current trajectory. The
agent reflects on what it just did, extracts a generalisable recipe, and
writes it as a new `SKILL.md`. This closes the loop: the agent does
something once, gets to keep the recipe forever.

## Subagents as Tools

Subagents are invoked like tools but each has its own conversation thread
and memory blocks. A common pattern: a "code-search" subagent invoked
many times accumulates specialist knowledge of the codebase, while the
main agent stays focused on the user-facing task.

## MCP

Letta supports MCP through the framework's standard tool registration —
not enumerated in the Letta Code README excerpt itself. The framework
docs describe attaching MCP servers as additional tool providers.

## Provider Coverage

Tools work uniformly across providers because the loop is server-side
and provider-neutral. The same tool calls produced by Claude Opus 4.5
work when the user runs `/model` and switches to Gemini 3 Pro or
GPT-5.1-Codex — at the cost of having to re-render prompts in each
provider's preferred function-calling format.

---

*Tool surface from `letta-ai/letta-code` README and the Letta SDK
examples. Specific tool schemas are documented at `docs.letta.com`.*
