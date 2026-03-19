# Cross-Agent Comparison: Context Management Strategies

## 1. Introduction

This document provides a comprehensive side-by-side comparison of context management strategies across **17 coding agents**: OpenHands, Codex, Gemini CLI, OpenCode, Goose, Claude Code, Aider, ForgeCode, mini-SWE-agent, Warp, Ante, Droid, Junie CLI, Pi Coding Agent, Sage Agent, TongAgents, and Capy.

Context management — how an agent tracks, compresses, persists, and retrieves conversational and code context — is arguably the single most important architectural decision in agent design. It determines how long an agent can work autonomously, how much codebase it can reason about, and how gracefully it degrades as tasks grow complex.

Use this file as a reference for understanding the landscape. Each section examines one dimension of context management, comparing all 17 agents in tabular form with analysis.

The agents span the full spectrum: from mini-SWE-agent (zero context management) to OpenHands (10-condenser pipeline with 6-layer architecture). Understanding where each agent sits on this spectrum — and *why* — reveals the fundamental tradeoffs in agent design.

---

## 2. Context Window Sizes and Models

The foundation of context management is the raw capacity available. Agents vary dramatically in how much context they can hold and how aggressively they use it. Window size alone does not determine effectiveness — what matters is how intelligently the agent *uses* the available space.

| Agent | Primary Model(s) | Context Window | How They Use It |
|-------|------------------|----------------|-----------------|
| OpenHands | Claude 3.5/4, GPT-4o | 128K–200K | Aggressive — fills then compacts via configurable pipeline |
| Codex | GPT-4o, o3-mini | 128K | Monitors at 90% capacity, auto-compacts with truncation |
| Gemini CLI | Gemini 2.5 Pro/Flash | 1M–2M | Leverages full window; compaction rarely needed |
| OpenCode | Claude, GPT-4o, Gemini | Varies by model | Uses server-reported usage; compacts at 95% threshold |
| Goose | Multiple providers | Varies | Proactive compaction at 80%, reactive on token-limit errors |
| Claude Code | Claude 3.5/4 | 200K | Auto-compaction + manual `/compact` command |
| Aider | Multiple models | Varies | Repo-map + token budgets prevent overflow proactively |
| ForgeCode | Multiple | 200K typical | Bounded context per sub-agent; no single agent sees full history |
| mini-SWE-agent | Various | 128K–200K | No compaction — designed to fit entirely in window |
| Warp | Multiple | Varies | User-initiated compaction via `/fork-and-compact` |
| Ante | Multiple | Varies | Distributed across meta-agent + specialized sub-agents |
| Droid | Multiple | Large | Incremental compression with anchor points |
| Junie CLI | JetBrains AI models | Varies | Token-budget allocation across multi-model routing |
| Pi Coding Agent | Multiple | Varies | Tree-structured history with branching |
| Sage Agent | Multiple | Varies | Basic context management, minimal sophistication |
| TongAgents | Multiple | Varies | Multi-agent distribution across specialized roles |
| Capy | Multiple | Varies | Spec document serves as context boundary between agents |

**Key Observation:** Gemini CLI's 1M–2M window fundamentally changes the game — it can hold entire codebases in context, making compaction largely unnecessary. Most other agents must actively manage 128K–200K windows.

---

## 3. Compaction Strategies Per Agent

Compaction is how agents deal with context overflow. The approaches range from "do nothing" to sophisticated multi-stage pipelines. This is where the most innovation is happening across the ecosystem.

| Agent | Primary Strategy | Secondary Strategy | LLM-Based? | Trigger |
|-------|-----------------|-------------------|-------------|---------|
| OpenHands | Configurable condenser pipeline | 10 condenser types available | Yes (optional) | Event count threshold (default: 100) |
| Codex | Per-item truncation (oldest first) | Global truncation pass | No | 90% of context window |
| Gemini CLI | Rarely needed at 1M+ | Checkpoint-based via shadow git | Minimal | Manual or on very long sessions |
| OpenCode | LLM summarization of history | Full conversation replacement | Yes | 95% of context window |
| Goose | Tool-pair background summarization | 3-level compaction hierarchy | Yes | 80% proactive threshold |
| Claude Code | LLM summarization | Auto-trigger + `/compact` command | Yes | Automatic threshold detection |
| Aider | Repo-map (proactive prevention) | Token budgets per category | No | Budget-based, not reactive |
| ForgeCode | Sub-agent partitioning | Semantic entry-point analysis | No | By architectural design |
| mini-SWE-agent | None — no compaction | Output truncation only | No | Never triggers |
| Warp | `/fork-and-compact` | User-initiated only | Yes | Manual user command |
| Ante | Sub-agent distribution | Meta-agent summarization | Minimal | Architectural — by design |
| Droid | Incremental compression | Anchor point preservation | Yes | Continuous, ongoing |
| Junie CLI | Token-budget allocation | Context routing across models | Minimal | Budget threshold |
| Pi Coding Agent | Tree branching (fork history) | `/fork` command | No | Manual user decision |
| Sage Agent | Basic truncation | None | No | Window limit |
| TongAgents | Agent-level distribution | Role-based partitioning | No | By design |
| Capy | Spec document boundary | Captain/Build agent split | Yes | Architectural boundary |

**Key Observation:** There is a clear divide between LLM-based compaction (OpenHands, OpenCode, Goose, Claude Code, Droid) and structural compaction (ForgeCode, Ante, Capy). LLM-based approaches preserve semantic content but cost tokens; structural approaches avoid the problem entirely through architecture.

---

## 4. Token Counting Approaches

Accurate token counting is critical for knowing *when* to compact. Agents vary widely in precision.

| Agent | Method | Accuracy | Client Library | Cost Tracking |
|-------|--------|----------|----------------|---------------|
| Aider | Exact tokenizer + sampling fallback | High | tiktoken | Basic per-session |
| Codex | Byte heuristic (4 bytes/token) | Medium (~15% variance) | None (custom) | Cumulative across session |
| OpenCode | Server-reported usage only | Exact (post-hoc) | None | Per-turn cost display |
| Goose | Provider-specific tokenizers | Varies by provider | Varies | Yes, per-provider |
| Claude Code | Internal Anthropic counting | N/A (integrated) | Integrated SDK | `/context` command |
| Gemini CLI | Gemini API reporting | High | Google SDK | Built-in |
| Junie CLI | Pre-computed token budgets | Medium | Varies by model | Yes |
| Warp | Pre-computed budgets | Medium | Varies | Yes |
| OpenHands | Model-specific estimation | Medium-High | Varies | Per-event tracking |
| ForgeCode | Per-agent budget tracking | Medium | Varies | Per-agent |
| Droid | Server-reported + estimation | Medium-High | Varies | Cross-session |
| Pi Coding Agent | Basic estimation | Low-Medium | Varies | Minimal |
| mini-SWE-agent | Basic or none | Low | None | Trajectory-level |
| Ante | Per-agent estimation | Medium | Varies | Distributed |
| TongAgents | Per-agent estimation | Medium | Varies | Per-role |
| Sage Agent | Basic estimation | Low | Varies | Minimal |
| Capy | Per-agent budget | Medium | Varies | Per-phase |

**Key Observation:** Only Aider invests in client-side exact tokenization (via tiktoken). Codex takes the opposite extreme with a simple byte-heuristic. Most agents rely on server-reported usage, which is accurate but only available *after* the API call — too late to prevent overflow on the current turn.

---

## 5. Memory and Persistence Systems

How agents remember across sessions and provide persistent context.

| Agent | Memory File | Auto-Memory | Session Storage | Resume Mechanism |
|-------|-------------|-------------|-----------------|------------------|
| Claude Code | CLAUDE.md (hierarchical) | Yes (auto-writes MEMORY.md) | Session store | `--resume`, `--name` |
| Gemini CLI | GEMINI.md (hierarchical) | No | Shadow git repo | `/restore` command |
| OpenCode | OpenCode.md | No | SQLite database | Session selection UI |
| Goose | .goosehints + AGENTS.md | No | File-based sessions | Session management |
| Warp | AGENTS.md | No | Warp Drive (cloud) | Cross-device sync |
| ForgeCode | AGENTS.md | No | N/A | N/A |
| Aider | None built-in | No | diskcache (SQLite) | `--restore-chat-history` |
| OpenHands | None | No | EventStream (JSON) | Event replay |
| Codex | None | No | In-memory only | GhostSnapshot rollback |
| Pi Coding Agent | None | No | JSONL tree structure | `/tree`, `/fork` |
| Droid | None (reported auto-memory) | Reported | Cloud storage | Cross-interface resume |
| Junie CLI | None | No | IDE-integrated | IDE session management |
| mini-SWE-agent | None | No | Full trajectory JSON | Direct trajectory replay |
| Ante | None | No | Distributed state | Meta-agent state |
| TongAgents | None | No | Per-agent state | N/A |
| Sage Agent | None | No | Minimal | N/A |
| Capy | Spec document | No | Phase-based | Spec continuity |

**Key Observation:** Claude Code's CLAUDE.md has become an informal standard — Gemini CLI (GEMINI.md), OpenCode (OpenCode.md), and multiple agents (AGENTS.md) have adopted the same hierarchical memory-file pattern. Auto-memory (automatically learning preferences) remains rare; only Claude Code implements it meaningfully.

---

## 6. Architecture Comparison

The highest-level view: how each agent structures its context management. Architecture choices ripple through every other dimension — they determine what compaction strategies are possible, how memory works, and what the ceiling is for task complexity.

| Agent | Single/Multi Agent | Context Philosophy | Unique Innovation |
|-------|-------------------|-------------------|-------------------|
| OpenHands | Single (configurable) | Fill aggressively, then compact | 10-condenser pipeline with 6-layer architecture |
| Codex | Single | Conservative truncation | Byte-based estimation + GhostSnapshot rollback |
| Gemini CLI | Single | Leverage massive 1M+ window | Shadow git checkpointing for state recovery |
| OpenCode | Single | Server-reported, auto-compact | SQLite persistence with multi-provider support |
| Goose | Single | 3-level hierarchical compaction | Tool-pair background summarization + MOIM extensions |
| Claude Code | Multi (sub-agents) | Partition work across windows | Sub-agents as a context management strategy |
| Aider | Single | Proactive prevention | tree-sitter + PageRank-based repo-map |
| ForgeCode | Multi (3-agent pipeline) | Bounded context per agent | Semantic entry-point analysis — 93% fewer tokens |
| mini-SWE-agent | Single | No compaction needed | Linear history doubles as training data |
| Warp | Single | User-controlled compaction | `/fork-and-compact` with cloud persistence |
| Ante | Multi (meta + sub) | Distributed across agents | Meta-agent coordination pattern |
| Droid | Single | Incremental compression | Confidence-scored compression for multi-week sessions |
| Junie CLI | Single | Budget allocation | JetBrains IDE integration + multi-model routing |
| Pi Coding Agent | Single | Tree-structured branching | JSONL tree with `/fork` for exploration |
| Capy | Multi (2-agent) | Spec document as boundary | Captain/Build architecture with spec handoff |
| TongAgents | Multi | Multi-agent distribution | Specialized agent roles with distinct contexts |
| Sage Agent | Single | Basic management | Minimal approach — simplicity as strategy |

---

## 7. Proactive vs Reactive Strategies

A critical distinction: does the agent *prevent* context overflow or *react* to it?

| Agent | Proactive Strategies | Reactive Strategies |
|-------|---------------------|---------------------|
| OpenHands | Event-count threshold triggers compaction early | Falls back through condenser pipeline on overflow |
| Codex | Monitors usage at 90% to compact before overflow | Truncates oldest items on token-limit API errors |
| Gemini CLI | 1M+ window makes overflow rare by design | Shadow git checkpoint restore on failure |
| OpenCode | Server-reported tracking enables early warning | LLM summarization when 95% threshold hit |
| Goose | 80% threshold + background tool-pair summarization | 3-level reactive compaction + API error retry |
| Claude Code | Sub-agent delegation prevents single-window overflow | Auto-compaction + `/compact` for manual recovery |
| Aider | Repo-map keeps only relevant code in context | Token budgets drop lowest-priority content first |
| ForgeCode | 3-agent architecture bounds context by design | Semantic entry-point reduces input at parse time |
| mini-SWE-agent | Designed for short tasks that fit in window | Output truncation is the only reactive measure |
| Warp | AGENTS.md provides persistent context cheaply | `/fork-and-compact` when user notices degradation |
| Ante | Meta-agent distributes work before context fills | Sub-agent isolation prevents cross-contamination |
| Droid | Continuous incremental compression | Anchor-point preservation on aggressive compression |
| Junie CLI | Multi-model routing sends tasks to optimal model | Budget reallocation when limits approached |
| Pi Coding Agent | Tree branching lets users explore without bloating main | `/fork` discards exploration branches |
| Sage Agent | None significant | Basic truncation on overflow |
| TongAgents | Role-based distribution prevents single-agent bloat | Agent-level context clearing |
| Capy | Spec document bounds what each agent sees | Captain re-plans if Build agent context exhausted |

**Key Observation:** The most effective agents combine proactive *and* reactive strategies. Goose exemplifies this: 80% proactive threshold + background summarization + 3-level reactive fallback. Pure reactive approaches (Sage Agent, mini-SWE-agent) only work for short tasks.

### Strategy Effectiveness Matrix

| Strategy Type | Best For | Worst For | Example Agents |
|--------------|----------|-----------|----------------|
| Pure proactive (prevention) | Large codebases, known structure | Exploratory tasks | Aider, ForgeCode |
| Pure reactive (compression) | Short-medium tasks | Long autonomous runs | Codex, Sage Agent |
| Hybrid (proactive + reactive) | General purpose, long sessions | Simple quick fixes (overkill) | Goose, Claude Code, OpenHands |
| Architectural (multi-agent) | Complex multi-step workflows | Simple single-file edits | ForgeCode, Ante, Capy |
| Avoidance (big window) | Everything up to window limit | Tasks exceeding even 1M tokens | Gemini CLI |

---

## 8. The Spectrum: Simplest to Most Sophisticated

Ranking all 17 agents from simplest to most complex context management:

| Rank | Agent | Sophistication | Why This Ranking |
|------|-------|---------------|------------------|
| 1 | mini-SWE-agent | None | No compaction at all — relies on fitting in window |
| 2 | Sage Agent | Basic | Minimal truncation, no persistence, no memory files |
| 3 | Pi Coding Agent | Low | Tree branching is clever but manual; no auto-compaction |
| 4 | TongAgents | Low-Medium | Multi-agent distribution but basic per-agent management |
| 5 | Codex | Medium | Truncation + byte heuristic — simple but effective |
| 6 | OpenCode | Medium | LLM summarization + SQLite — solid single-agent approach |
| 7 | Warp | Medium | User-controlled + AGENTS.md + cloud persistence |
| 8 | Gemini CLI | Medium-High | 1M window + shadow git — sidesteps the problem elegantly |
| 9 | Goose | High | 3-level compaction + MOIM + tool-pair summarization |
| 10 | Aider | High | Repo-map (tree-sitter + PageRank) + token budgets |
| 11 | Claude Code | High | Sub-agents + auto-memory + hierarchical CLAUDE.md |
| 12 | Junie CLI | High | Multi-model routing + budget allocation + IDE integration |
| 13 | Capy | High | Captain/Build + spec-document boundary |
| 14 | Ante | High | Meta-agent coordination + distributed context |
| 15 | ForgeCode | Very High | 3-agent pipeline + semantic entry-point — 93% token savings |
| 16 | Droid | Very High | Incremental compression + confidence scoring + multi-week |
| 17 | OpenHands | Very High | 10-condenser pipeline + 6-layer architecture + full config |

**Note:** Higher sophistication does not always mean better. mini-SWE-agent's "no compaction" approach works perfectly for short, well-scoped tasks. Gemini CLI's strategy of "just use a bigger window" is arguably the most practical. The right level of sophistication depends on task duration, codebase size, and autonomy requirements.

---

## 9. Cross-Cutting Dimensions

### 9a. Multi-Agent vs Single-Agent Context Distribution

| Approach | Agents | Tradeoffs |
|----------|--------|-----------|
| Single-agent, no compaction | mini-SWE-agent, Sage Agent | Simple but limited to short tasks |
| Single-agent, reactive compaction | Codex, OpenCode, Warp | Works for medium tasks; quality degrades on long ones |
| Single-agent, proactive management | Aider, Goose, Droid, Junie CLI | Best single-agent approaches; Aider's repo-map is standout |
| Multi-agent, partitioned context | Claude Code, ForgeCode, Capy, Ante, TongAgents | Highest token efficiency; most architectural complexity |

### 9b. Memory File Adoption

The CLAUDE.md pattern has spawned an ecosystem:
- **CLAUDE.md** → Claude Code (the originator)
- **GEMINI.md** → Gemini CLI (direct adaptation)
- **OpenCode.md** → OpenCode (same pattern)
- **AGENTS.md** → Goose, Warp, ForgeCode (standardized variant)
- **.goosehints** → Goose (additional layer)
- **Spec documents** → Capy (functional equivalent)
- **None** → OpenHands, Codex, Aider, mini-SWE-agent, Pi Coding Agent

### 9c. Session Persistence Approaches

| Approach | Agents | Durability |
|----------|--------|------------|
| In-memory only | Codex | Lost on exit (GhostSnapshot for rollback only) |
| File-based JSON/JSONL | OpenHands, Goose, Pi Coding Agent, mini-SWE-agent | Durable, human-readable |
| SQLite database | OpenCode, Aider | Durable, queryable |
| Cloud storage | Warp, Droid | Cross-device, highest durability |
| Shadow git repo | Gemini CLI | Durable + versioned state |
| IDE-integrated | Junie CLI | Tied to IDE session lifecycle |

---

## 10. Key Insights

### No Single Approach Dominates
The landscape reveals no universal winner. Each strategy excels in specific contexts:
- **Short tasks** → mini-SWE-agent's simplicity is ideal
- **Large codebases** → Aider's repo-map or Gemini CLI's 1M window
- **Long autonomous sessions** → Droid's incremental compression or OpenHands' pipeline
- **Multi-step workflows** → ForgeCode's 3-agent pipeline or Capy's Captain/Build

### Proactive Beats Reactive
Agents that prevent context bloat consistently outperform those that compress after the fact. Aider's repo-map (only loading relevant code) and ForgeCode's semantic entry-point (93% fewer tokens) demonstrate that *not putting tokens in context* is better than *removing them later*.

### Multi-Agent = Largest Token Savings, Most Complexity
ForgeCode's 93% token reduction is the standout metric. But multi-agent architectures introduce coordination overhead, potential information loss at boundaries, and debugging complexity. The tradeoff is worth it for complex, long-running tasks but overkill for quick fixes.

### Simple Approaches Work Surprisingly Well
mini-SWE-agent achieves competitive benchmark scores with zero context management. For tasks that fit in a single context window (and many real-world tasks do), sophisticated compaction adds complexity without benefit.

### The Trend: Smarter Context, Not Just Bigger Windows
While Gemini CLI proves bigger windows help, the broader trend is toward *smarter* context management:
- **Structural prevention** (ForgeCode, Aider) over reactive compression
- **Hierarchical memory** (CLAUDE.md pattern) for cross-session persistence
- **Multi-agent distribution** for complex tasks
- **Confidence-scored compression** (Droid) for quality-aware compaction

### The Emerging Standard Stack
A "best practices" context management stack is emerging:
1. **Memory file** (CLAUDE.md / AGENTS.md) for persistent preferences
2. **Proactive filtering** (repo-map or semantic analysis) to minimize input
3. **LLM-based summarization** as the primary compaction strategy
4. **Token budget system** to allocate context across categories
5. **Session persistence** (SQLite or file-based) for resume capability
6. **Sub-agent delegation** for tasks that exceed single-window capacity

No single agent implements all six perfectly, but Claude Code and Goose come closest.

### What's Missing From All Agents
Despite rapid progress, several gaps remain across the entire landscape:
- **Semantic deduplication** — no agent detects when the same information appears multiple times in context
- **User-adaptive compaction** — compaction aggressiveness should vary by user expertise and task type
- **Cross-session learning** — only Claude Code attempts to learn from past sessions; most start fresh
- **Context quality metrics** — no agent measures whether compacted context actually preserves decision-relevant information
- **Collaborative context** — no agent handles multi-user shared context for team workflows

---

*This comparison is based on analysis of open-source repositories, documentation, and published research as of 2025. Agent capabilities evolve rapidly — verify against current versions.*
