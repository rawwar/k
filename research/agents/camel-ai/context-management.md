# CAMEL-AI — Context Management

## Statefulness as a Design Principle

One of CAMEL's four stated design principles:

> *"Agent context is managed as a state transition process, supporting
> rich, dynamic memory management over time."*

Concretely, `Memories` is a first-class stack component (separate from
Models, Tools, Storage, Retrievers). The framework treats memory not as
a feature you opt into but as a layer in the system.

## Memory Component

Memory in CAMEL pulls from `Storage` (configurable backends — JSON, KV,
vector store) and is owned by individual `ChatAgent` instances or
shared across an agent society.

The CAMEL website (<https://www.camel-ai.org/framework>) describes
agents that maintain stateful memory enabling them to perform
multi-step interactions and tackle sophisticated tasks — a positioning
similar to Letta, but with a wider research agenda (data generation,
world simulation) rather than purpose-built memory products.

## Retrieval

Separate `Retrievers` component — RAG over the agent's available
storage. Useful when you have a large knowledge base that doesn't fit
in context. The framework distinguishes retrieval from memory:
memories are the agent's own experiences, retrievers query external
knowledge.

## Multi-Agent Context

In a Workforce setup, each agent has its own context. The
manager/coordinator agent sees its own conversation; sub-agents see
theirs. Context-sharing across agents is explicit — typically via
message-passing or a shared blackboard, not a single pooled context
window.

## Long-Horizon Support

CAMEL emphasises long-horizon multi-step tasks as a use case. The
combination of stateful memory + retrievers + multi-agent context
isolation is the framework's answer to the "context window explosion"
problem.

## What This Means for the Coding Mode

For the TB2 coding configuration (Sonnet 4.5, 46.5%), context
management is presumably handled by:

- Per-agent memory + storage,
- Optional retrievers over the project files,
- Workforce-style context isolation across roles.

The exact configuration is not publicly documented.

## What's Not Documented Here

- Specific memory backends used by the TB2 submission
- Compaction / summarisation strategies (the framework supports them,
  but the choice depends on configuration)
- Token-budget allocation across society members

These would need a dedicated read of the source.

---

*From `camel-ai/camel` README, the framework principles page at
<https://www.camel-ai.org/framework>, and the tech stack page.*
