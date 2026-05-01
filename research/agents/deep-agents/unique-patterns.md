# Deep Agents — Unique Patterns

## 1. The "Harness" Framing

LangChain's docs draw an explicit distinction:

| Layer | Example | Role |
|---|---|---|
| Framework | LangChain | Primitives (model, tool, message) |
| Runtime | LangGraph | Durable graph execution, checkpointing |
| **Harness** | **Deep Agents** | Opinionated default agent on top |

This is a useful taxonomy. Deep Agents argues that "agent frameworks"
have been conflating two jobs — providing primitives, and providing
opinions about what a good agent looks like — and separates them
cleanly. The harness is where the opinions live.

## 2. Files-as-Overflow

Routing oversized tool outputs to disk and giving the model a path is
a borrowed-from-shell idiom that's underused in agent design. Most
agents either truncate, summarise, or just accept the bloat. Deep
Agents leans into the filesystem as working memory — which also makes
the built-in `grep`/`glob`/`read_file` tools dual-purpose: they're how
the agent navigates the project, *and* how it navigates its own
working data.

## 3. LangGraph for Free

Because the returned object is a real LangGraph, you get:

- streaming
- checkpointing (memory / SQLite / Postgres)
- human-in-the-loop (`interrupt`)
- Studio / observability
- composability with other LangGraphs

…without any of that being Deep Agents-specific. You can take the
agent and embed it as a node in a larger workflow. Most agent products
are dead-end terminals; Deep Agents is a building block.

## 4. Provider Promiscuity

Out-of-the-box support for Google, OpenAI, Anthropic, OpenRouter,
Fireworks, Baseten, Ollama. The README intro literally lists eight
provider snippets side-by-side. This is more provider coverage than
most production-grade harnesses bother with.

## 5. SDK + CLI + ACP

One project ships all three:

- **SDK** for embedding into your own apps
- **CLI** as a turnkey product (the TB2 entry)
- **ACP** for editor integration

Most agents commit to one form factor. Deep Agents covers all three
without forking, by leaning on LangGraph as the common runtime.

## 6. write_todos as a First-Class Tool

Like Mux, Codex, and Claude Code, Deep Agents treats todo tracking as
a tool the model is taught to use. The model doesn't track todos
implicitly — it writes them to a tool, the harness exposes them, the
model reads them back. Convergent design across the field as of 2026.

## 7. MIT-Licensed

100% MIT, no commercial gating. Useful baseline for anyone building
their own agent: fork it.

## Comparison

| Pattern | Deep Agents | Letta Code | Mux | Claude Code |
|---|---|---|---|---|
| Built on a runtime | ✅ LangGraph | (Letta server) | own | own |
| Multi-form-factor (SDK+CLI+ACP) | ✅ | (CLI+desktop+mobile) | (desktop+browser) | CLI |
| Files-as-overflow | ✅ explicit | implicit | implicit | implicit |
| Sub-agent isolation | ✅ | ✅ | (open another workspace) | ✅ |
| MCP | via adapter | ✅ | (limited) | ✅ |

---

*Patterns from README and `docs.langchain.com`.*
