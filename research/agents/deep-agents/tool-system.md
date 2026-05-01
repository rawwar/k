# Deep Agents — Tool System

## Built-In Tools

From the README:

| Category | Tools |
|---|---|
| Planning | `write_todos` |
| Filesystem | `read_file`, `write_file`, `edit_file`, `ls`, `glob`, `grep` |
| Shell | `execute` (sandboxed) |
| Delegation | `task` (sub-agent spawning) |

These constitute the "harness" — every Deep Agent gets them by default.
They are paired with smart-default prompts that teach the model when to
use which.

## `execute` Sandboxing

The README mentions `execute` runs "with sandboxing" but does not detail
the mechanism. Likely candidates from the LangChain ecosystem are
container-based sandboxes (Modal, Daytona, E2B, Coder, Daytona) or local
process isolation. The CLI documentation expands on this with "remote
sandboxes" as a feature.

## Custom Tools

Adding tools is one keyword arg:

```python
agent = create_deep_agent(
    model="openai:gpt-5.4",
    tools=[my_custom_tool],
)
```

Tools follow the standard LangChain tool interface — any
`@tool`-decorated function or `BaseTool` subclass works.

## MCP via Adapters

MCP is supported via the
[`langchain-mcp-adapters`](https://github.com/langchain-ai/langchain-mcp-adapters)
package. Install it alongside `deepagents` and pass MCP server tools to
`tools=`. This is a clean separation: the Deep Agents harness doesn't
itself speak MCP, but it inherits MCP support from the LangChain
ecosystem.

## Sub-Agent Tool Surface

Sub-agents spawned via `task` get the same tool surface by default. You
can configure restricted sub-agents (e.g. read-only research agents)
via the customization API; the docs at
`docs.langchain.com/oss/python/deepagents/` go into specifics.

## File-Based Overflow

Mentioned in the README under context management but worth re-stating
in the tool context: when a tool produces a giant blob, the harness
writes it to a file and hands the agent a path. This means the
filesystem tools (`read_file`, `grep`, `glob`) double as the agent's
memory for things that wouldn't fit in chat context.

This is a deliberate idiom borrowed from how human developers work — if
the build output is huge, you redirect it to a file and grep it later.
Deep Agents bakes it in.

## Web Search (CLI only)

The Deep Agents CLI bundles a web search tool out of the box. The SDK
does not — you'd add a search tool yourself. Useful split: the SDK
stays minimal, the CLI ships a turnkey product.

## Tool Description Style

Default prompts include guidance on *when* to use each tool — not just
the schemas. This is consistent with the harness positioning: opinions
about good agent behaviour, not just plumbing.

---

*Tool surface from `langchain-ai/deepagents` README and the customisation
example in `docs.langchain.com/oss/python/deepagents/overview`.*
