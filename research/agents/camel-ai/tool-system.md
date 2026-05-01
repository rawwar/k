# CAMEL-AI — Tool System

## CAMEL Toolkit

CAMEL ships a "batteries-included" toolkit — listed on the project's
tech-stack page as covering messaging, planning, evaluation, and
observability. Tools are a first-class component (separate from Models,
Memories, Storage, Retrievers, etc.).

Without re-enumerating the entire toolkit (it's extensive), the broad
categories are:

- **Code execution** — Interpreters component
- **Web / search** — search tools
- **Retrieval** — Retrievers component (RAG over storage)
- **Memory tools** — for managing the Memories component
- **MCP** — first-class MCP support listed on tech stack
- **Human-in-the-loop** — explicit gating tools
- **Observability** — Observe component for tracing agent runs

## Tool Registration

Tools are registered on a `ChatAgent` at construction time:

```python
agent = ChatAgent(
    system_message=...,
    model=...,
    tools=[my_tool, search_tool, ...],
)
```

This is the standard LangChain-style pattern.

## MCP

CAMEL lists MCP on its core component list (alongside Tools, Memories,
Models). This makes it natural to plug existing MCP servers into CAMEL
agents — the framework treats MCP as a peer protocol rather than a
bolted-on adapter.

## Verifiers

A distinctive component: `Verifier` is a separate stack-component
(not a tool). Verifiers check agent outputs against criteria — useful
for the framework's RL / synthetic-data use cases where you need
verified correctness.

## Environments

CAMEL has explicit `Environments` as components — abstract execution
contexts (sandboxes, simulators, browsers). This is more general than
"sandbox the bash tool" and reflects the framework's research use
cases (e.g. world simulation).

## Toolkit Sharing Across Agents

In multi-agent setups, tools can be shared (all agents see the same
toolkit) or partitioned (each role gets its own subset). This is
configurable at the Workforce / society level.

## OWL Tool Surface

OWL inherits CAMEL's toolkit and adds collaboration-specific tools for
agent-to-agent coordination. The GAIA-leading score is partly
attributable to OWL's tool integration breadth.

## Documentation

The full toolkit is too large to enumerate here. The authoritative
list is at <https://camel-ai.github.io/camel> (framework docs).

---

*Tool surface from the CAMEL README, the camel-ai.org tech stack page,
and the OWL README.*
