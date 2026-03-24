---
title: Context Management
status: complete
---


# Context Management

> Synthesized from studying 17 coding agents: OpenHands, Codex, Gemini CLI, OpenCode, Goose, Claude Code, Aider, ForgeCode, mini-SWE-agent, Warp, Ante, Droid, Junie CLI, Pi Coding Agent, Sage Agent, TongAgents, and Capy.

## The Problem

Context management is the central design challenge of coding agents. Nearly every architectural decision — sub-agents, compaction, memory files, skill systems — exists to address it.

### The Fundamental Mismatch

Real codebases are vast. A medium-sized project might contain 500K–5M tokens of source code, plus documentation, configuration, test suites, and build artifacts. Meanwhile, LLM context windows range from 32K to 1M tokens. Even at the frontier, a single model call cannot "see" an entire non-trivial codebase. The agent must decide what to include, what to summarize, and what to discard — and it must decide correctly, because including the wrong context is worse than including none at all.

ForgeCode makes this explicit: **"Context size is a multiplier on the right entry point, not a substitute for it."** A model with 200K context starting in the right file outperforms a model with 1M context starting in the wrong directory.

### Quality Degradation Under Context Pressure

From Anthropic's Claude Code documentation: *"Most best practices are based on one constraint: Claude's context window fills up fast, and performance degrades as it fills."* This is unusually transparent about the fundamental limitation. As context fills, models start "forgetting" earlier instructions, make more errors, and lose coherence. The degradation is not linear — performance drops sharply once a model-specific threshold is exceeded.

### The Compounding Problem in Agentic Loops

Single-loop agents face compounding context pressure across turns:

1. **Exploration bloat**: Finding the right file requires reading directory listings, search results, and candidate files. Most of this is irrelevant but persists in context.
2. **History accumulation**: Each tool call and result stays in the conversation. By message 20, the context window is dominated by stale exploration results.
3. **Cost amplification**: Every token in context is re-sent on every turn. A 100K-token context over 30 turns means 3M input tokens billed — at $3/M tokens, that's $9 in input costs alone for a single task.

### The Design Space

Agents have converged on three fundamental strategies, used alone or in combination:

| Strategy | Philosophy | Example Agents |
|----------|-----------|----------------|
| **Reduce what goes in** | Selective context loading, entry-point discovery | Aider, ForgeCode, Gemini CLI |
| **Compress what's there** | Summarization, truncation, sliding windows | OpenHands, Codex, Goose, OpenCode |
| **Partition across windows** | Sub-agents with bounded context | Claude Code, ForgeCode, Ante, Capy |

The choice between these strategies reveals a deeper tension: **information fidelity vs. token efficiency**. Every compression loses something. Every partition creates a communication boundary. The art of context management is choosing what to lose.

---

## Token Counting

Before you can manage context, you must measure it. Agents take surprisingly different approaches to this seemingly straightforward problem.

### Approach 1: Exact Tokenizer Counting

The gold standard. Use the model's actual tokenizer (e.g., tiktoken for OpenAI models) to count tokens precisely. Aider uses this for short texts, falling back to sampling for longer content:

```python
# Aider's hybrid approach
def token_count(self, text):
    if len(text) < 200:
        return self.main_model.token_count(text)  # Exact
    # Sample every 100th line, extrapolate
    lines = text.splitlines(keepends=True)
    step = len(lines) // 100 or 1
    sample = lines[::step]
    return sample_tokens / len("".join(sample)) * len(text)
```

**Trade-off**: Accurate but requires loading a tokenizer model (startup cost, dependency) and is slow for large texts. The sampling fallback is a pragmatic compromise.

### Approach 2: Byte-Based Heuristic Estimation

Codex CLI's most distinctive design decision — no tokenizer at all:

```rust
const APPROX_BYTES_PER_TOKEN: usize = 4;

fn approx_token_count(text: &str) -> usize {
    text.len() / APPROX_BYTES_PER_TOKEN
}
```

This works because English text averages roughly 4 bytes per token across modern tokenizers. The estimation is combined with server-reported token counts from the most recent API response — the heuristic only covers tokens added *since* the last API call.

**Trade-off**: Fast and dependency-free, but less accurate for non-English text, code with unusual symbol density, or heavily structured output. Acceptable because the estimation only needs to be "close enough" for compaction threshold decisions, not exact.

### Approach 3: Server-Reported Only

OpenCode takes the simplest approach: no client-side token counting at all. It relies entirely on the provider's reported `TokenUsage` after each response:

```go
type TokenUsage struct {
    InputTokens         int64
    OutputTokens        int64
    CacheCreationTokens int64
    CacheReadTokens     int64
}
```

**Trade-off**: Zero client complexity, but context overflow is only detected *after* it happens (the provider returns an error). This requires reactive error handling rather than proactive budget management.

### Approach 4: Pre-Computed Token Budgets

Several agents allocate fixed budgets to context components upfront:

| Agent | Budget Allocation Example |
|-------|--------------------------|
| **Aider** | Repo-map: 1K default (expandable to 8K); response reserve: 4K minimum |
| **Junie CLI** | ~2K system, ~20K active files, ~10K related files, ~2K build/test |
| **Warp** | Explicit allocation across system prompt, codebase, history, user message, tool defs, response reserve |

This approach treats context like a financial budget — each category gets an allocation, and components compete for the remainder.

### Cost Tracking

Token counting increasingly happens alongside cost tracking. OpenCode computes per-turn costs using model pricing data:

```go
cost := model.CostPer1MInCached/1e6*float64(usage.CacheCreationTokens) +
    model.CostPer1MOutCached/1e6*float64(usage.CacheReadTokens) +
    model.CostPer1MIn/1e6*float64(usage.InputTokens) +
    model.CostPer1MOut/1e6*float64(usage.OutputTokens)
```

Claude Code's `/context` command surfaces what's consuming space. Codex tracks cumulative token usage across the session. This visibility helps both the agent and the user make informed decisions about when to compact or reset.

### Comparison

| Approach | Agents | Accuracy | Speed | Dependencies |
|----------|--------|----------|-------|-------------|
| Exact tokenizer | Aider (short text) | High | Slow | Tokenizer library |
| Sampling + extrapolation | Aider (long text) | Medium-High | Fast | Tokenizer library |
| Byte heuristic (~4 B/tok) | Codex | Medium | Fastest | None |
| Server-reported only | OpenCode | Exact (post-hoc) | N/A | None |
| Pre-computed budgets | Aider, Junie, Warp | Varies | Fast | Optional |

The trend is clear: **accuracy matters less than you'd think**. Compaction thresholds are set conservatively (80-95% of window), so a ±10% estimation error rarely causes problems. Speed and simplicity win.

---

## Compaction

Compaction is the richest area of divergence across agents. When context grows too large, something must give. The strategies range from brute-force truncation to sophisticated LLM-powered summarization pipelines.

### Summarization

The most common approach: ask an LLM to summarize older conversation history, replacing verbose history with a compact summary.

**OpenCode** uses a focused summarization prompt that preserves what matters for continuation:

```
"Provide a detailed but concise summary of our conversation above.
 Focus on: what was done, what we're doing, which files we're working on,
 and what we're going to do next."
```

The summary message replaces all prior history — subsequent turns start from the summary rather than the full history.

**Claude Code** offers both auto-compaction and manual `/compact` with focus instructions:

```
/compact Focus on the API changes    # Directed compaction
/compact Keep the test results       # Selective preservation
```

Claude Code also re-reads CLAUDE.md from disk after compaction, ensuring project instructions are never lost — a key design decision that other agents don't replicate.

**OpenHands** implements two LLM-based condensers:
- **LLMSummarizingCondenser**: Free-form summary of forgotten events
- **StructuredSummaryCondenser**: Categorized sections ("Key Findings", "Files Modified", "Current Approach", "Open Issues") — giving the agent a more organized view than free-form text

**Droid** claims the most ambitious approach: incremental compression with anchor points, where key decisions are preserved at full fidelity while surrounding context is compressed progressively. Each context piece carries a "confidence" score determining compression aggressiveness. This reportedly enables multi-week continuous sessions.

### Truncation

Rather than summarizing, simply cap or trim content. This is faster and cheaper than LLM-based summarization.

**Per-item truncation** (Codex): Each tool output is capped at ~10KB, preserving the beginning (67%) and end (33%) with a middle elision marker. The asymmetric split keeps error messages (typically at the end) visible:

```rust
let prefix_size = limit * 2 / 3;   // ~67% from start
let suffix_size = limit / 3;        // ~33% from end
format!("{prefix}\n…{truncated_chars} chars truncated…\n{suffix}")
```

**Observation-level truncation** (mini-SWE-agent): If command output exceeds 10,000 characters, show head (5K) + tail (5K) with a warning. This is transparent to the model and encourages it to use more targeted commands.

**Search result limits** (ForgeCode): `FORGE_MAX_SEARCH_RESULT_BYTES` (default 10KB) prevents a single grep from flooding context with thousands of matches.

**Global tool output pass** (Codex): When total context is too large, a global pass truncates *all* function outputs with the configured policy — a blunt but effective last resort.

### Sliding Window

Drop older messages entirely, keeping only recent history.

**OpenHands' ConversationWindowCondenser** preserves initial context (setup/instructions) and recent events, forgetting everything in between:

```python
class ConversationWindowCondenser(RollingCondenser):
    max_events: int = 100
    keep_first: int = 5   # Preserve initial setup/instructions
    # Keep first K events + last (max - K) events; forget the middle
```

**RecentEventsCondenser** is even simpler: keep the last N events (default 50), discard everything else.

**AmortizedForgettingCondenser** takes a gentler approach: gradual forgetting that preserves progressively more recent events, avoiding the harsh cliff of a fixed window.

The key insight across all sliding-window approaches: **initial context matters**. The system prompt, first user message, and setup instructions should be preserved even as the middle of the conversation is discarded.

### Condenser Chains (OpenHands)

OpenHands' most powerful contribution is the **PipelineCondenser** — chaining multiple condensers in sequence:

```toml
[condenser]
type = "pipeline"

[[condenser.stages]]
type = "observation_masking"     # First: hide verbose observation content

[[condenser.stages]]
type = "conversation_window"     # Then: apply sliding window
max_events = 100
keep_first = 5

[[condenser.stages]]
type = "llm_summarizing"         # Finally: summarize if still too large
max_events = 50
```

This enables strategies like: "First mask observation content, then apply a sliding window, then summarize if still too large." Each stage operates on the output of the previous, and any stage can short-circuit by returning a `CondensationAction` that requires controller intervention before proceeding.

OpenHands provides 10 condenser implementations that can be composed:

| Condenser | Strategy | LLM Cost |
|-----------|----------|----------|
| NoOp | Pass-through | None |
| RecentEvents | Keep last N | None |
| ConversationWindow | First K + last N | None |
| AmortizedForgetting | Gradual progressive | None |
| ObservationMasking | Replace content with placeholders | None |
| BrowserOutput | Browser-specific masking | None |
| LLMAttention | LLM selects events to keep | 1 call |
| LLMSummarizing | LLM summarizes forgotten events | 1 call |
| StructuredSummary | Categorized section summary | 1 call |
| Pipeline | Chain multiple condensers | Varies |

The agent can even request condensation itself via the `CondensationRequestTool` — a form of **self-directed memory management** where the agent decides when to forget.

### Tool-Pair Summarization (Goose)

Goose's most distinctive innovation: **background summarization of tool_call + tool_result pairs**. Each turn, a background task identifies old tool pairs and generates concise summaries:

```rust
// Mark old messages invisible (kept for UI/persistence)
for (idx, _) in &summaries {
    conversation.messages[*idx].set_agent_visible(false);
}
// Insert summaries in their place
for (_, summary) in summaries {
    conversation.messages.push(summary);
}
```

The originals are preserved for UI display and session persistence but marked `agent_visible: false` so the LLM never sees them. This is elegant — the complete history exists for debugging, but the LLM's view is progressively compressed.

Goose operates at three levels:
1. **Proactive** (pre-loop): Compact at 80% of context limit
2. **Reactive** (error recovery): Compact on `ContextLengthExceeded`, max 2 attempts
3. **Background** (per-turn): Asynchronous tool-pair summarization

### MOIM — Model-Oriented Information Management (Goose)

Each turn, before calling the LLM, Goose injects dynamic context from extensions via `inject_moim()`. Extensions provide per-turn context that's relevant *right now* — similar to RAG but driven by the extension ecosystem rather than vector search. The "Top of Mind" extension uses this to inject persistent user instructions.

### Bounded Context Across Sub-Agents

Rather than compressing one window, partition the work across multiple windows.

**Claude Code** makes this philosophy explicit: *"Sub-agents are the most powerful context management tool."* Exploration happens in a separate context window; only the summary returns to the main conversation:

```
Main Context Window                    Sub-Agent Context Window
┌──────────────────────┐              ┌──────────────────────┐
│ Your conversation    │              │ Exploration task     │
│ (preserved)          │   ──spawn──▶ │ Reads 20 files       │
│                      │              │ Searches codebase    │
│                      │   ◀─summary─ │ Analyzes patterns    │
│ + compact summary    │              │ (all discarded)      │
└──────────────────────┘              └──────────────────────┘
```

**ForgeCode** takes this further with a three-agent pipeline (Sage → Muse → Forge) where each agent receives only distilled output from the previous stage — not raw exploration context.

**Ante** distributes context across a meta-agent and sub-agents, with effective total context equal to the sum of all agent windows. **Capy** uses a spec document as the compression boundary: Captain distills exploration into a structured spec; Build receives only the spec plus codebase access.

### Repo-Map: Structural Context Without Full Files (Aider)

Aider's repo-map is the most sophisticated approach to *preventing* context bloat rather than compressing it after the fact. The pipeline:

1. **Tree-sitter parsing**: Extract definitions and references from every source file (100+ languages)
2. **Dependency graph**: Build file-level reference graph (A references symbols defined in B)
3. **Personalized PageRank**: Rank files by importance, biased toward files currently in the chat and mentioned identifiers
4. **Token-budgeted selection**: Include highest-ranked symbols until the budget (default 1024 tokens, expandable to 8K) is consumed

The result is a compact structural map showing class definitions, function signatures, and import relationships — without function bodies, comments, or implementation details. The map dynamically resizes: expanding when no files are added (maximum codebase awareness), shrinking when files are present (leaving room for full content).

### Semantic Entry-Point Discovery (ForgeCode)

ForgeCode's context engine performs a semantic pass before any agent starts exploring:

1. Index the project into vector embeddings
2. Analyze the task description semantically
3. Return ranked entry-point files and functions
4. Agent starts in the right place, using context to reason deeply rather than explore broadly

ForgeCode claims "up to 93% fewer tokens" compared to naive exploration.

### No Compaction (mini-SWE-agent)

The deliberate anti-pattern: append everything, truncate nothing, summarize nothing.

This works because: (1) modern context windows are enormous (128K-200K+), (2) typical SWE-bench trajectories generate only 30K-60K tokens, (3) linear history means zero information loss, (4) trajectories are directly usable as fine-tuning data with no distribution shift.

mini-SWE-agent's position: **"Start with linear history, and only add compaction if you empirically need it."** For benchmark-focused agents, this is surprisingly effective.

### Compaction Strategy Comparison

| Strategy | Information Loss | LLM Cost | Latency | Best For |
|----------|-----------------|----------|---------|----------|
| **LLM summarization** | Medium (summary quality varies) | 1 LLM call per compaction | High | Long multi-topic sessions |
| **Structured summary** | Low-Medium (categorized) | 1 LLM call | High | Complex multi-step tasks |
| **Per-item truncation** | Low (head+tail preserved) | None | Negligible | Large tool outputs |
| **Sliding window** | High (old context lost) | None | Negligible | Simple short-lived sessions |
| **Condenser chains** | Configurable | Varies | Varies | Sophisticated long-running agents |
| **Tool-pair summarization** | Medium | 1 call per pair | Background | Tool-heavy workflows |
| **Sub-agent partitioning** | Low (only summaries cross) | Extra agent calls | Medium | Exploration-heavy tasks |
| **Repo-map** | None (proactive selection) | None | Build cost | Codebase-aware editing |
| **Semantic entry-point** | None (proactive selection) | Embedding cost | Index cost | Large unfamiliar codebases |
| **No compaction** | None | None | None | Short tasks, fine-tuning data |

### Compaction Triggers

When does compaction fire? The thresholds reveal different risk tolerances:

| Agent | Trigger | Philosophy |
|-------|---------|-----------|
| **Goose** | 80% of context window | Conservative — large buffer for response + tools |
| **Codex** | 90% of context window | Moderate — smaller buffer |
| **OpenCode** | 95% of context window | Aggressive — maximize history retention |
| **OpenHands** | Configurable event count (default 100) | Event-based, not token-based |
| **Claude Code** | Auto (configurable) + manual `/compact` | Dual: system + user control |
| **Warp** | `/fork-and-compact` | User-initiated with conversation forking |

---

## Session Persistence

Context management doesn't end when the session closes. How agents persist and restore conversation state determines whether work survives across sessions.

### SQLite (OpenCode)

OpenCode stores all conversations in a SQLite database within the project directory:

```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL DEFAULT '',
    message_count INTEGER NOT NULL DEFAULT 0,
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    summary_message_id TEXT,
    cost REAL NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    role TEXT NOT NULL,
    parts TEXT NOT NULL,  -- JSON-serialized ContentPart array
    model TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
```

Benefits: ACID transactions, efficient queries over history, structured metadata (cost, token counts), foreign key relationships between sessions and messages. The `summary_message_id` field links to the compaction boundary — messages before the summary are excluded from future prompts.

### File-Based Session Dumps

**OpenHands** uses an append-only EventStream backed by a FileStore, with events serialized as JSON and paginated for efficient retrieval. The immutable history means context management decisions are reversible — you can reconstruct what the agent saw at any point.

**Gemini CLI** uses shadow git repositories for checkpointing:

```
~/.gemini/history/<project_hash>/
├── .git/                    # Shadow git repository
├── conversation.json        # Full conversation history
├── tool_calls.json          # All tool calls and results
├── metadata.json            # Session metadata
└── files/                   # Snapshots of modified files
```

Each turn auto-commits to the shadow repo, enabling git-based inspection of checkpoint history and restore via `/restore`.

**Pi Coding Agent** uses JSONL with `id`/`parentId` fields forming a tree structure — supporting in-place branching, `/tree` navigation, and `/fork` without ever deleting history.

### Memory Files (CLAUDE.md, GEMINI.md, OpenCode.md)

Persistent project-level instruction files that are loaded into context at session start:

| Agent | File | Hierarchy | JIT Discovery |
|-------|------|-----------|---------------|
| **Claude Code** | CLAUDE.md | User → parent dirs → project → child dirs | Yes (on file access) |
| **Gemini CLI** | GEMINI.md | Global → workspace → JIT | Yes (on file access) |
| **OpenCode** | OpenCode.md | Project-level only | No |
| **Goose** | .goosehints + AGENTS.md | Global → project → directory | No |
| **Warp** | AGENTS.md | Global → project → directory | No |
| **Claude Code** | Auto-memory (MEMORY.md) | Per-project, machine-local | Loaded at start (first 200 lines) |

Claude Code's auto-memory is distinctive: the agent writes its own notes (`~/.claude/projects/<project>/memory/MEMORY.md`) that persist across sessions. Only the first 200 lines load at startup — a practical token budget threshold.

CLAUDE.md and GEMINI.md both support modular imports via `@path/to/file` syntax, and both survive compaction — they're re-read from disk after context is compressed.

### Conversation Checkpointing and Resume

| Agent | Mechanism | Resume Command |
|-------|-----------|----------------|
| **Gemini CLI** | Shadow git repo checkpoints | `/restore` |
| **Claude Code** | Session naming + persistence | `--resume`, `--name` |
| **Pi Coding Agent** | JSONL tree-structured history | `/tree`, `/fork` |
| **Warp** | Warp Drive (cross-device, team-shared) | Conversation forking |
| **Droid** | Cross-interface persistence | Survives web ↔ CLI ↔ GitHub ↔ Slack transitions |

### The Persistence-Context Tension

A subtle design tension: persistence and compaction are at odds. Compaction discards information to free context space, but persistence wants to retain everything for future sessions. Agents resolve this differently:

- **Goose** keeps original messages with `agent_visible: false` — invisible to the LLM but preserved for persistence and UI
- **OpenHands** maintains the immutable EventStream separately from the condensed View — the full history is always available even as the agent's working view is compressed
- **Codex** preserves `GhostSnapshot` items through compaction — enabling undo/redo across compaction boundaries
- **mini-SWE-agent** sidesteps the tension entirely: no compaction means the saved trajectory exactly matches what the LLM saw, making it directly usable as training data

---

## Tools & Projects

The ecosystem of tools for context management in coding agents spans five layers — from low-level token counting to high-level memory systems. This section catalogs the key open-source projects and services at each layer.

### Tokenization & Counting

Accurate token counting is the foundation: every budget decision, compaction trigger, and cost estimate depends on it.

| Tool | Language | Key Strength | License |
|------|----------|-------------|---------|
| **[tiktoken](https://github.com/openai/tiktoken)** | Python (Rust core) | Exact counts for OpenAI models (GPT-4o, o1, o3); 3-6× faster than alternatives | MIT |
| **[tokenizers](https://github.com/huggingface/tokenizers)** | Rust (Python/JS/Ruby bindings) | Multi-model support (BPE, WordPiece, Unigram); alignment tracking maps tokens back to source spans | Apache 2.0 |
| **[gpt-tokenizer](https://github.com/niieani/gpt-tokenizer)** | TypeScript | Browser-ready; built-in `isWithinTokenLimit()` for early-exit checks and `estimateCost()` | MIT |
| **[llama-tokenizer-js](https://github.com/belladoreai/llama-tokenizer-js)** | JavaScript | Client-side LLaMA tokenization (~1 ms); critical since LLaMA/OpenAI counts differ ~20% | MIT |

> **Practical shortcut:** Most agents use `len(text) / 4` for quick estimates, then precise tokenization near context boundaries. Aider samples every 100th line and extrapolates — ~100× faster for large files.

### Code Intelligence & Repo Mapping

Understanding code structure — not just raw text — enables agents to select the *right* context.

- **[tree-sitter](https://github.com/tree-sitter/tree-sitter)** — Incremental parsing library with 100+ language grammars. Builds concrete syntax trees that update on every keystroke, even with syntax errors. Powers repo maps, symbol extraction, and AST-aware chunking across Aider, GitHub Copilot, and Cursor. MIT.

- **[Aider repo-map](https://github.com/Aider-AI/aider)** — Three-step pipeline: (1) tree-sitter extracts symbol definitions and references, (2) builds a file-level dependency graph, (3) PageRank selects the most important symbols. Default budget is 1,024 tokens; expands to 8× when no files are in chat. Apache 2.0.

- **[ast-grep](https://github.com/ast-grep/ast-grep)** — Structural code search/replace built on tree-sitter. Uses pattern syntax (`$MATCH` wildcards) instead of regex, enabling precise code-pattern queries for context selection. MIT.

- **[Sourcegraph / SCIP](https://sourcegraph.com)** — Code intelligence platform using a layered approach: precise language-server indexers → tree-sitter heuristics → text search fallback. Demonstrates production-grade code understanding at scale.

### Memory & Long-Context Systems

These projects give agents explicit control over what stays in context versus what gets paged out.

- **[Letta / MemGPT](https://github.com/letta-ai/letta)** — Treats the context window like RAM in an OS. The agent manages its own memory via tools: `core_memory_append/replace` (always in-context), `archival_memory_insert/search` (vector-backed external storage), and `conversation_search` (recall). The most radical approach — the agent itself decides what to keep. Apache 2.0.

- **[LangChain / LangGraph](https://github.com/langchain-ai/langchain)** — Provides a menu of memory strategies: buffer, sliding window, summary, summary-buffer hybrid, token-budget, entity tracking, and vector-store retrieval. LangGraph adds checkpoint-based persistence with `SqliteSaver`/`PostgresSaver` and cross-thread memory via `Store`. MIT.

- **[OpenHands condensers](https://github.com/All-Hands-AI/OpenHands)** — Pluggable compaction system with ~10 strategies including LLM summarization, attention-based selection, amortized forgetting, hierarchical condensation, and observation masking. Demonstrates that no single strategy fits all situations. MIT.

### Code RAG & Retrieval

Retrieval-Augmented Generation for code requires code-aware chunking and multi-pass search.

- **[LlamaIndex](https://github.com/run-llama/llama_index)** — Data framework with a `CodeSplitter` that respects AST structure (whole functions, not random lines), hierarchical indices for broad-to-narrow search, and hybrid retrieval combining vectors with keywords. MIT.

- **[Voyage AI (`voyage-code-3`)](https://github.com/voyage-ai/voyageai-python)** — Code-specific embedding model trained on code + documentation pairs. Outperforms general-purpose embeddings (e.g., `text-embedding-3-small`) by 15-30% on code retrieval benchmarks. Includes reranker models.

- **[Chroma](https://github.com/chroma-core/chroma)** — Embeddable vector database with a 4-function API (create, add, query, get). Handles tokenization and embedding automatically; supports metadata filtering by file path, language, or function name. Apache 2.0.

| Retrieval Approach | Speed | Semantic Accuracy | Best For |
|--------------------|-------|-------------------|----------|
| Keyword / BM25 | Fast | Low | Known identifiers, exact names |
| Embedding (general) | Medium | Medium | Natural-language queries |
| Embedding (code-specific) | Medium | High | Code search, cross-language |
| Hybrid (keyword + embedding) | Medium | High | Production systems |
| Multi-pass (lexical → embed → LLM rerank) | Slow | Highest | Complex retrieval tasks |

### Codebase Search & Understanding

End-to-end platforms that index entire repositories for AI-powered code understanding.

- **[Sweep](https://github.com/sweepai/sweep)** — Pioneered multi-pass retrieval for code: lexical search (fast) → embedding search (semantic) → LLM re-ranking. Uses AST-aware chunking and incremental indexing on git changes. BUSL-1.1.

- **[Greptile](https://greptile.com)** — Codebase understanding as a service. Indexes GitHub/GitLab repos; learns team coding standards from PR comments. Reports median time-to-merge drops from 20 hours to 1.8 hours. SOC 2 compliant; self-hosted option available.

- **[Cursor codebase indexing](https://docs.cursor.com/context/codebase-indexing)** — Embedding-based indexing with automatic re-indexing on file changes. Hybrid retrieval combines vector similarity with structural code understanding. `.cursorignore` for scoping. Arguably the most successful consumer-facing implementation.

### Persistence & Memory Stores

Long-lived agents need durable state across sessions, restarts, and crashes.

- **SQLite patterns** — The dominant choice for single-agent persistence. OpenCode/Crush stores sessions and messages; Aider uses `diskcache` (SQLite-backed) for tag caching with mtime invalidation; LangGraph's `SqliteSaver` checkpoints entire agent state. Zero-config, single-file, ACID-compliant.

- **[Mem0](https://github.com/mem0ai/mem0)** — Self-improving memory layer that automatically extracts facts from conversations, deduplicates, and resolves conflicts. Multi-level memory (user, session, agent) with vector + graph storage. Apache 2.0.

- **[Zep](https://github.com/getzep/zep)** — Long-term memory store with auto-summarization, named entity extraction, and temporal awareness. Native LangChain integration. Hybrid search (vector + metadata). Apache 2.0.

| Memory Store | Auto-Extract | Summarization | Multi-Level | Search | Integration |
|-------------|-------------|---------------|-------------|--------|-------------|
| SQLite (raw) | No | Manual | No | SQL | Universal |
| Mem0 | ✅ Facts | ✅ | ✅ User/session/agent | Vector + graph | REST API |
| Zep | ✅ Entities | ✅ | Session-level | Vector + metadata | LangChain native |
| Letta/MemGPT | Agent-driven | Agent-driven | Core/archival/recall | Vector | Built-in tools |

---

## Real-World Implementations

| Agent | Strategy | Reference |
|-------|----------|-----------|
| **OpenHands** | 10 condenser strategies, 6-layer context system | [`../agents/openhands/context-management.md`](../agents/openhands/context-management.md) |
| **opencode** | Auto-compact with SQLite persistence | [`../agents/opencode/context-management.md`](../agents/opencode/context-management.md) |
| **Claude Code** | `/compact` command, CLAUDE.md, auto-memory | [`../agents/claude-code/context-management.md`](../agents/claude-code/context-management.md) |
| **Goose** | 3-level compaction, tool-pair summarization, MOIM | [`../agents/goose/context-management.md`](../agents/goose/context-management.md) |
| **Codex** | Byte-based token estimation, auto-compaction | [`../agents/codex/context-management.md`](../agents/codex/context-management.md) |
| **Gemini CLI** | 1M token context utilization | [`../agents/gemini-cli/context-management.md`](../agents/gemini-cli/context-management.md) |
| **mini-SWE-agent** | Linear history — no compaction needed | [`../agents/mini-swe-agent/context-management.md`](../agents/mini-swe-agent/context-management.md) |
| **Aider** | Repo-map (tree-sitter → graph → PageRank → token budget) | [`../agents/aider/context-management.md`](../agents/aider/context-management.md) |
| **ForgeCode** | Bounded context across sub-agents, semantic entry-point discovery | [`../agents/forgecode/context-management.md`](../agents/forgecode/context-management.md) |
