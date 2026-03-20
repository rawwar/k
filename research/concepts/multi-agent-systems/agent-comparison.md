---
title: Cross-Agent Comparison
status: complete
---

# Cross-Agent Comparison

This document provides a systematic comparison of all 17 coding agents studied in this
research library across their multi-agent capabilities. We compare orchestration patterns,
specialist agent types, communication methods, and key architectural decisions. The goal
is to provide a reference for understanding which agents use which patterns and why.

---

## Multi-Agent Support Overview

### The Multi-Agent Spectrum

Not all agents implement multi-agent patterns. The spectrum ranges from purely
single-agent systems to fully specialized ensembles:

```
Level 0               Level 1                Level 2              Level 3
Single Agent          Sub-Agent              Pipeline             Specialized
                      Delegation             Orchestration        Ensemble
    │                     │                      │                    │
 mini-SWE             Claude Code             SageAgent           ForgeCode
 Aider (basic)        OpenHands               Junie CLI           Capy
 OpenCode             Codex CLI                                   TongAgents
 Pi Coding Agent      Goose
 Warp                 Ante
 Droid                Gemini CLI
 Gemini CLI*
```

*Gemini CLI has multi-agent scaffolding but limited documentation of its use.

---

## Comprehensive Comparison Table

### Multi-Agent Architecture

| Agent | Multi-Agent Level | Pattern | # of Agent Types | Spawnable Sub-Agents |
|-------|------------------|---------|-------------------|---------------------|
| **ForgeCode** | 3 (Ensemble) | 3 bounded specialists | 3 (Forge/Muse/Sage) | Yes |
| **Capy** | 3 (Ensemble) | 2 hard-split specialists | 2 (Captain/Build) | No (fixed pair) |
| **TongAgents** | 3 (Ensemble) | Multi-agent team (speculative) | Unknown | Unknown |
| **SageAgent** | 2 (Pipeline) | 5-stage pipeline | 5 pipeline agents | No (fixed pipeline) |
| **Junie CLI** | 2 (Pipeline) | 3-stage pipeline | 3 (Plan/Execute/Verify) | No (fixed pipeline) |
| **Claude Code** | 1 (Sub-Agent) | Orchestrator + spawnable workers | 3 built-in + custom | Yes (no nesting) |
| **Codex CLI** | 1 (Sub-Agent) | Resource-controlled sub-agents | 3 roles | Yes |
| **OpenHands** | 1 (Sub-Agent) | Event-sourced + delegation | Main + delegates + micro | Yes (AgentDelegate) |
| **Goose** | 1 (Sub-Agent) | MCP-based + Summon + ACP | Dynamic via extensions | Yes (Summon) |
| **Ante** | 1 (Sub-Agent) | Meta-Agent + dynamic pool | Dynamic | Yes (concurrent) |
| **Gemini CLI** | 1 (Sub-Agent) | Agent scaffolding | Not fully documented | Possibly |
| **OpenCode** | 1 (Sub-Agent) | Coder + task agent | 2 (coder/task) | Yes (task sub-agent) |
| **Aider** | 0.5 (Two-Model) | Architect mode | 2 models (not agents) | No |
| **Droid** | 0 (Single) | Single agent, multi-interface | 1 | No |
| **Warp** | 0 (Single) | Local + cloud agent modes | 1 per mode | No |
| **Pi Coding Agent** | 0 (Single) | Minimal core | 1 | No |
| **mini-SWE-agent** | 0 (Single) | Deliberately minimal | 1 | No |

### Specialist Agent Types

| Agent | Planner | Implementer | Researcher | Reviewer | Tester | Observer |
|-------|---------|-------------|------------|----------|--------|----------|
| **ForgeCode** | Muse ✓ | Forge ✓ | Sage ✓ | Verification skill ✓ | — | — |
| **Capy** | Captain ✓ | Build ✓ | Captain ✓ | — | Build ✓ | — |
| **SageAgent** | PlanningAgent ✓ | ExecutorAgent ✓ | TaskAnalysisAgent ✓ | — | — | ObservationAgent ✓ |
| **Junie CLI** | Planner ✓ | Executor ✓ | — | — | Verifier ✓ | — |
| **Claude Code** | Plan sub-agent ✓ | General sub-agent ✓ | Explore sub-agent ✓ | Custom agent ✓ | Custom agent ✓ | — |
| **Codex CLI** | Default ✓ | Worker ✓ | Explorer ✓ | — | — | — |
| **OpenHands** | Main agent ✓ | Main agent ✓ | Micro-agents ✓ | — | — | StuckDetector ✓ |
| **Goose** | Main agent ✓ | Main agent ✓ | Analyze ext ✓ | — | — | — |
| **Ante** | Meta-Agent ✓ | Sub-agents ✓ | Sub-agents ✓ | — | — | — |
| **Aider** | Architect model ✓ | Editor model ✓ | Repo-map ✓ | — | — | — |
| **MetaGPT** | ProductMgr ✓ | Engineer ✓ | Architect ✓ | — | QA ✓ | — |
| **AutoGen** | Configurable ✓ | Configurable ✓ | Configurable ✓ | Configurable ✓ | Configurable ✓ | — |
| **CrewAI** | Configurable ✓ | Configurable ✓ | Configurable ✓ | Configurable ✓ | Configurable ✓ | — |

### Communication Methods

| Agent | Protocol | Direction | Serialization | Discovery |
|-------|----------|-----------|---------------|-----------|
| **ForgeCode** | Bounded context passing | Orchestrator → worker | Compressed summaries | Static 3-agent |
| **Capy** | Spec document | Captain → Build | Natural language spec | Static 2-agent |
| **SageAgent** | Pipeline handoff | Linear + 1 feedback edge | Agent output | Static 5-agent |
| **Junie CLI** | Backend proxy | Linear pipeline | HTTP/API via JetBrains | Static 3-stage |
| **Claude Code** | Tool-use (Messages API) | Parent → child | JSON tool_use/tool_result | Static + `.claude/agents/` |
| **Codex CLI** | SQ/EQ messages | Bidirectional | Rust typed enums | Static roles |
| **OpenHands** | EventStream pub/sub | Broadcast | Action/Observation types | Subscriber registration |
| **Goose** | MCP protocol | Client → server | MCP messages | Extension discovery |
| **Ante** | Lock-free scheduler | Meta → sub-agents | Atomic state | Dynamic pool |
| **OpenCode** | Go pub/sub broker | Coder → task | Typed messages | Static 2-agent |
| **Aider** | Direct function call | Architect → editor | LLM output → edit format | Hardcoded |
| **MetaGPT** | Structured artifacts | Pipeline | PRDs, designs, code | Predefined roles |
| **AutoGen** | AgentChat messages | Configurable | Python objects | Static or dynamic |
| **CrewAI** | Task outputs | Sequential/hierarchical | Task results | Role definitions |

---

## Orchestration Pattern Comparison

### Which Agents Use Which Patterns

| Pattern | Agents Using It | Key Example |
|---------|----------------|-------------|
| **Orchestrator-Worker** | Claude Code, Codex CLI, ForgeCode, Ante | Claude Code's Task tool |
| **Pipeline** | SageAgent, Junie CLI, MetaGPT, ChatDev | SageAgent's 5-stage pipeline |
| **Hard-Split Specialists** | ForgeCode, Capy | Capy's Captain/Build with capability constraints |
| **Swarm/Handoffs** | (None pure; Goose has elements) | OpenAI Agents SDK (framework) |
| **Event-Sourced** | OpenHands | EventStream with typed Action/Observation |
| **Two-Model** | Aider, Junie CLI | Aider's Architect mode |
| **Agents-as-Tools** | AutoGen, OpenAI Agents SDK | AutoGen's AgentTool |
| **MCP-Based** | Goose, SageAgent | Goose's everything-is-MCP approach |
| **Peer/Debate** | (None in production) | Research pattern only |

### Parallelism Support

| Agent | Parallel Agent Execution | Mechanism | Max Concurrency |
|-------|------------------------|-----------|-----------------|
| **Capy** | ✓ Full | Independent VMs per task | 25 concurrent jams |
| **Codex CLI** | ✓ Full | `join_all` + atomic guards | Resource-limited |
| **Ante** | ✓ Full | Lock-free scheduler | Dynamic pool size |
| **Claude Code** | ✓ Sub-agents | Separate context windows | Not documented |
| **ForgeCode** | ✓ Limited | Low-complexity parallelization | Not documented |
| **OpenHands** | ✓ Event-based | ThreadPoolExecutor per subscriber | Per-subscriber |
| **Goose** | ✓ Via Summon | Sub-agent isolation | Not documented |
| **SageAgent** | ✗ Sequential | Pipeline architecture | 1 (sequential) |
| **Junie CLI** | ✗ Sequential | Pipeline architecture | 1 (sequential) |
| **Aider** | ✗ Sequential | Two-model chain | 1 (sequential) |

---

## Verification and Quality Enforcement

| Agent | Verification Type | Enforcement Level | Mechanism |
|-------|------------------|-------------------|-----------|
| **ForgeCode** | Programmatic + LLM review | **Architectural** (runtime blocks completion) | Verification Skill |
| **SageAgent** | Continuous observation | **Pipeline** (ObservationAgent in loop) | Feedback to PlanningAgent |
| **Junie CLI** | Test-driven | **Iterative** (test loop, 3-5 max iterations) | Verifier stage |
| **Capy** | Build-internal tests | **Worker** (Build runs tests autonomously) | Test suite execution |
| **Claude Code** | Prompt-suggested | **Advisory** (agent decides whether to test) | System prompt guidance |
| **Codex CLI** | Optional | **Advisory** | User-configured |
| **OpenHands** | StuckDetector | **Reactive** (detects loops, not quality) | Loop monitoring |
| **Goose** | Recipe retry + reset | **Retry-based** (clears slate on failure) | Conversation reset |
| **Aider** | Lint + test integration | **External** (relies on pre-commit hooks) | Git-native workflow |
| **MetaGPT** | QA Engineer | **Pipeline** (QA stage generates tests) | Role-based |
| **AutoGen** | Configurable | **Framework** (user-defined termination) | Custom conditions |
| **CrewAI** | Reviewer agent | **Role** (reviewer task in crew) | Sequential task |

---

## Model Strategy

| Agent | Model Tiering | Strategy | Impact |
|-------|--------------|----------|--------|
| **Claude Code** | ✓ Haiku for explore, parent for general | Cost optimization | Cheap research, expensive reasoning |
| **Junie CLI** | ✓ Multi-model router | Dynamic routing per task type | +6.7pp improvement |
| **ForgeCode** | ✓ Progressive reasoning budget | Budget varies by conversation stage | Resource-adaptive |
| **Aider** | ✓ Architect mode | Reasoning model + editing model | +5.3pp improvement |
| **Codex CLI** | ✓ Explorer may use faster model | Role-based model selection | Cost optimization |
| **Goose** | ✓ ACP + model selection | Agent Communication Protocol | Agent-of-agents |
| **SageAgent** | ✗ Single model | Same model for all 5 agents | Simplicity |
| **Capy** | Not documented | — | — |

---

## Context Management Strategy

| Agent | Strategy | Compression | Key Mechanism |
|-------|----------|-------------|---------------|
| **ForgeCode** | Bounded context | ~93% token reduction | sem_search + summary handoff |
| **Claude Code** | Isolated windows | Per sub-agent isolation | Separate context per agent |
| **Capy** | Spec document | High (full plan → concise spec) | Document as interface |
| **SageAgent** | Pipeline handoff | Per-stage output | Linear data flow |
| **Codex CLI** | Role-based filtering | Per-role context | JSONL persistence |
| **OpenHands** | Event replay | Full history available | EventStream + file store |
| **Goose** | MOIM + summarization | Per-turn injection + background | Extension context injection |
| **Junie CLI** | Backend proxy | Server-side management | JetBrains backend |

---

## Unique Innovations Per Agent

| Agent | Unique Innovation | Why It Matters |
|-------|------------------|---------------|
| **ForgeCode** | Enforced verification + tool-call correction | Highest Terminal-Bench score (81.8%) |
| **Claude Code** | Custom agents via markdown + worktree isolation | Most extensible sub-agent system |
| **Codex CLI** | SQ/EQ + RAII resource management | Most robust sub-agent lifecycle |
| **Capy** | Forcing functions (constraints as features) | Cleanest separation of concerns |
| **SageAgent** | Single feedback edge in pipeline | Simplest effective feedback loop |
| **OpenHands** | Keyword-triggered microagents | Lightweight RAG without embeddings |
| **Goose** | Agent Communication Protocol (ACP) | Agent-of-agents: use any agent as backend |
| **Junie CLI** | Dynamic multi-model routing | Best cost/quality trade-off via model selection |
| **Ante** | Zero-mutex lock-free scheduling | Highest theoretical concurrency |
| **Aider** | Repo-map via tree-sitter + PageRank | Best single-agent context management |
| **MetaGPT** | SOP as code (`Code = SOP(Team)`) | Most structured multi-agent workflow |
| **AutoGen** | Magentic-One + AutoGen Studio | Most mature framework ecosystem |
| **CrewAI** | Crews + Flows hybrid | Best balance of autonomy and control |

---

## Decision Guide: When to Use What

### For Simple Tasks (Single File Edit, Quick Fix)

**Recommendation:** Single agent (Aider, OpenCode, mini-SWE-agent)

No multi-agent overhead needed. The coordination cost exceeds the benefit.

### For Medium Tasks (Multi-File Changes, Feature Implementation)

**Recommendation:** Sub-agent delegation (Claude Code, Codex CLI)

Spawn explore agents for research, use the main agent for implementation.
Context isolation keeps the main window clean.

### For Complex Tasks (System Refactors, Architecture Changes)

**Recommendation:** Specialized ensemble (ForgeCode, Capy)

Hard role boundaries ensure thorough planning (Muse/Captain) and reliable
execution (Forge/Build). Verification enforcement catches errors.

### For Autonomous Execution (Background Tasks, CI/CD)

**Recommendation:** Pipeline with verification (SageAgent, Junie CLI)

Defined stages with feedback loops ensure quality without human oversight.
Test-driven verification provides objective quality signals.

### For Multi-Model Cost Optimization

**Recommendation:** Model-tiered systems (Claude Code, Junie CLI)

Use cheap models for research, expensive models for reasoning. Junie CLI's
dynamic router demonstrates a 6.7pp improvement from model selection alone.

### For Framework-Based Development

**Recommendation:** AutoGen or CrewAI

When building custom multi-agent systems rather than using pre-built agents.
AutoGen for flexibility, CrewAI for role-based simplicity.

---

## Terminal-Bench 2.0 Rankings (Multi-Agent Relevant)

| Rank | Agent | Score | Multi-Agent Level | Key Pattern |
|------|-------|-------|-------------------|-------------|
| 1 | **ForgeCode** (Opus 4.6) | 81.8% | Level 3 (Ensemble) | Enforced verification |
| 1 | **ForgeCode** (GPT 5.4) | 81.8% | Level 3 (Ensemble) | Enforced verification |
| 3 | **TongAgents** (Gemini Pro) | 80.2% | Level 3 (Ensemble) | Unknown |
| 4 | **Claude Code** (Opus 4.6) | 78.5% | Level 1 (Sub-Agent) | Task tool |
| — | **Capy** | Not ranked | Level 3 (Ensemble) | Hard boundaries |
| — | **SageAgent** | ~70% est | Level 2 (Pipeline) | 5-stage pipeline |
| 14 | **Junie CLI** (multi-model) | 71.0% | Level 2 (Pipeline) | Multi-model routing |
| 25 | **Junie CLI** (Flash only) | 64.3% | Level 2 (Pipeline) | Single model |

**Observation:** The top-ranked agents all use Level 2+ multi-agent patterns.
However, correlation is not causation — these agents also have other advantages
(better prompts, more tools, enforced verification) that contribute to their scores.

---

## Summary Matrix: All 17 Agents at a Glance

```
Agent              │ Multi │ Orch │ Spec │ Parallel │ Verify  │ Context
                   │ Agent │ Type │ ists │          │         │ Strategy
───────────────────┼───────┼──────┼──────┼──────────┼─────────┼──────────
ForgeCode          │  L3   │ Ens  │  3   │ Limited  │ENFORCED │ Bounded
Capy               │  L3   │ Ens  │  2   │ 25 VMs   │ Tests   │ Spec doc
TongAgents         │  L3   │ Ens  │  ?   │ Unknown  │ Unknown │ Unknown
SageAgent          │  L2   │ Pipe │  5   │ None     │Observer │ Pipeline
Junie CLI          │  L2   │ Pipe │  3   │ None     │TestLoop │ Proxy
Claude Code        │  L1   │ O-W  │  3+  │ SubAgent │Advisory │ Isolated
Codex CLI          │  L1   │ O-W  │  3   │ join_all │Optional │ Role-filt
OpenHands          │  L1   │ Evt  │  1+μ │ ThreadP  │StuckDet │ EventRply
Goose              │  L1   │ MCP  │  Dyn │ Summon   │ Retry   │ MOIM
Ante               │  L1   │ Meta │  Dyn │ LockFree │ —       │ Atomic
Gemini CLI         │  L1   │  ?   │  ?   │ ToolSch  │ —       │ —
OpenCode           │  L1   │ O-W  │  2   │ None     │ —       │ PubSub
Aider              │  L0.5 │ 2Mdl │  2   │ None     │External │ RepoMap
Droid              │  L0   │  —   │  1   │ None     │ —       │ Compact
Warp               │  L0   │  —   │  1   │ None     │ —       │ Auto-rte
Pi Coding Agent    │  L0   │  —   │  1   │ None     │ —       │ Cross-prv
mini-SWE-agent     │  L0   │  —   │  1   │ None     │ —       │ Minimal
───────────────────┴───────┴──────┴──────┴──────────┴─────────┴──────────

Legend:
  L0-L3: Multi-agent level (0=single, 1=sub-agent, 2=pipeline, 3=ensemble)
  O-W: Orchestrator-Worker    Ens: Ensemble    Pipe: Pipeline
  Evt: Event-sourced    Meta: Meta-Agent    2Mdl: Two-Model
  μ: microagents    Dyn: Dynamic
```

---

## Detailed Agent Profiles: Multi-Agent Capabilities

### Tier 1: Specialized Ensembles (Level 3)

**ForgeCode** — The highest-scoring agent on Terminal-Bench 2.0 (81.8%). Three agents
with hard-enforced boundaries: Forge (read-write implementer), Muse (read-only planner),
Sage (internal-only researcher). The bounded context model ensures only summaries — not
raw exploration data — cross agent boundaries. The context engine achieves 93% token
reduction through semantic entry-point discovery. Enforced verification (the runtime
blocks task completion until verification passes) is their "biggest single improvement."
Tool-call correction layer auto-corrects malformed tool calls using heuristics and static
analysis, making any model more reliable. Progressive reasoning budget allocates high
thinking for initial analysis and verification, low thinking for routine implementation.

**Capy** — Two-agent architecture with the strongest forcing functions in any coding agent.
Captain (planning) literally cannot write code; Build (execution) literally cannot ask
clarifying questions. These platform-enforced constraints create a forcing function:
Captain must write thorough specs because Build cannot request clarification. The spec
document is the sole communication interface. Supports 25 concurrent "jams," each with
its own sandboxed Ubuntu VM and git worktree. SOC 2 Type II certified. Task-based
workflow bundles conversation + branch + VM + PR into a single unit.

**TongAgents** — Multi-agent system from Beijing Institute for General Artificial
Intelligence (BIGAI). No source code publicly available — all inferences from benchmarks.
Achieves 80.2% with Gemini 3.1 Pro on Terminal-Bench (#3). The ~8 percentage point gap
between Gemini Pro (80.2%) and Claude Opus (71.9%) suggests the architecture may leverage
specific model capabilities. Part of the "Tong" ecosystem of cognitive architecture tools.

### Tier 2: Pipeline Orchestration (Level 2)

**SageAgent** — Five-agent pipeline with exactly one feedback loop. TaskAnalysisAgent →
PlanningAgent → ExecutorAgent → ObservationAgent → TaskSummaryAgent. The single feedback
edge from ObservationAgent back to PlanningAgent enables iterative refinement without the
complexity of arbitrary agent graphs. Two execution modes: Deep Research (full pipeline,
feedback enabled) and Rapid Execution (simplified, feedback disabled). MCP-native tool
architecture supports both stdio and SSE servers. Each agent is a separate class extending
AgentBase, not just a different prompt on a generic agent.

**Junie CLI** — Three-stage pipeline (Planner → Executor → Verifier) backed by a
multi-model router that dynamically selects the optimal model per sub-task. Complex
reasoning routes to Claude Sonnet/Opus, fast edits to Gemini Flash, code generation
to the best model for the language. This dynamic routing produces a 6.7 percentage
point improvement (71.0% multi-model vs 64.3% Gemini Flash alone). All requests proxy
through JetBrains backend, enabling server-side continuous improvement without client
updates. IDE-first heritage means it has access to PSI tree, inspections, and semantic
refactoring in IDE mode with heuristic degradation in CLI mode.

### Tier 3: Sub-Agent Delegation (Level 1)

**Claude Code** — Three built-in sub-agent types (explore/plan/general-purpose) plus
user-defined custom agents via markdown files in `.claude/agents/`. Explore sub-agents
use Haiku (cheaper, faster model) with read-only tools — a deliberate cost/speed
optimization for codebase research. Critical constraint: sub-agents cannot spawn other
sub-agents (flat hierarchy). Custom agents can specify model, tools, permission mode,
MCP servers, and persistent memory. Worktree isolation gives sub-agents their own git
worktree for parallel work without merge conflicts.

**Codex CLI** — Resource-controlled sub-agents managed by AgentControl with atomic
CAS-based concurrency limits. Three roles: default, explorer (may use faster model),
worker (file ownership semantics). SQ/EQ message-passing pattern decouples agent core
from UI layer, allowing TUI, exec mode, app-server, and MCP frontends. SpawnReservation
implements Rust's Drop trait for RAII-style cleanup if agent spawn fails. JSONL rollout
persistence enables session resume for sub-agents across restarts.

**OpenHands** — Event-sourced architecture with central EventStream. AgentDelegateAction
spawns sub-agents; results return as AgentDelegateObservation. Three types of microagents:
RepoMicroagent (always active, from `.openhands/microagents/`), KnowledgeMicroagent
(keyword-triggered — mentioning "django" auto-injects Django expertise), TaskMicroagent
(`/command`-triggered workflows). Docker sandbox with HTTP-based host↔container split.
StuckDetector monitors for agents caught in loops.

**Goose** — MCP-first architecture where everything is an MCP server. Summon extension
enables sub-agent delegation with isolated contexts. Agent Communication Protocol (ACP)
allows using other agents (Claude Code, Codex, Gemini CLI) as backend LLM providers,
creating an "agent-of-agents" pattern. MOIM (Model-Oriented Information Management)
injects context from all extensions per-turn. Background tool-pair summarization
proactively compresses old tool results to free context space.

**Ante** — Meta-Agent with dynamic sub-agent pool using lock-free scheduling (atomic ops,
wait-free queues) for zero-mutex concurrency. Fully Rust-native single binary. Supports
both cloud APIs and local inference via nanochat-rs.

**OpenCode** — Go-native coder agent can spawn task sub-agents via the `agent` tool. Task
agents have read-only tools (glob, grep, ls, view, sourcegraph). Typed pub/sub broker
for internal communication. SQLite per-project persistence. LSP integration for
diagnostics provides language-aware context.

### Tier 4: Single Agent with Enhancements (Level 0-0.5)

**Aider** — Architect mode chains two models: a reasoning model (o1, o3, R1) describes
the solution in natural language, then a code-editing model (Sonnet, GPT-4o) translates
it into file edits. Not multi-agent (single Coder class, no sub-processes), but
demonstrates the value of separating reasoning from editing. Repo-map via tree-sitter +
PageRank provides effective single-agent context management. Fuzzy edit matching handles
imprecise LLM output gracefully.

**Droid (Factory)** — Single agent core that is interface-agnostic (CLI, web, Slack,
Linear, CI/CD share the same agent). Specification Mode uses reasoning models for spec
generation and execution models for implementation — similar to Capy's Captain/Build but
within one agent. Compaction Engine enables multi-week sessions by compressing history.

**Warp** — Oz Platform unifies local and cloud agent execution. Local agents run in Warp
terminal with PTY access; cloud agents run on Warp infrastructure. Auto model routing
with modes: Cost-efficient, Responsive, Genius. GPU-accelerated Metal rendering pipeline.
Block-based terminal model with per-command grid isolation.

**Pi Coding Agent** — Deliberately minimal core (no planning layer, no sub-agent
orchestration). Monorepo with 7 npm packages. Four modes: Interactive, Print/JSON, RPC,
SDK. Custom TUI framework. Cross-provider context handoff converts conversation history
between different LLM providers.

**mini-SWE-agent** — The control group: 4 components (~350 total lines), single `bash`
tool, append-only message list as state. Demonstrates that the simplest possible agent
architecture can still be effective for bounded tasks.

---

## Evolution Trajectory

Based on our research, multi-agent patterns in coding agents are evolving along
these trajectories:

### Near-Term (6-12 Months)

- **Model tiering becomes standard** — Every agent will use different models for
  different sub-tasks, following Claude Code and Junie CLI's lead.
- **Enforced verification spreads** — ForgeCode's "biggest single improvement" will
  be adopted by more agents as benchmark competition intensifies.
- **Custom agent definitions** — Claude Code's `.claude/agents/` pattern will inspire
  project-specific specialist agents across tools.

### Medium-Term (1-2 Years)

- **A2A adoption** — As agent ecosystems grow, standardized agent-to-agent communication
  will become necessary. Google's A2A protocol is the leading candidate.
- **Agent marketplaces** — Specialist agents (security reviewer, performance analyst,
  migration specialist) available as services, discoverable via Agent Cards.
- **Hybrid P2P elements** — Peer review and debate patterns will emerge within
  orchestrator-worker systems, not as standalone architectures.

### Long-Term (2+ Years)

- **Self-organizing teams** — Agent systems that dynamically assemble the right team
  for each task, drawing from a pool of available specialists.
- **Cross-tool agent collaboration** — Claude Code sub-agents collaborating with
  Codex workers on the same codebase, coordinated via A2A.
- **Learned orchestration** — Orchestrators that improve task decomposition based
  on historical success patterns, not just prompt engineering.

---

## Cross-References

- [orchestrator-worker.md](./orchestrator-worker.md) — Detailed orchestrator-worker pattern analysis
- [specialist-agents.md](./specialist-agents.md) — How specialist roles are designed
- [communication-protocols.md](./communication-protocols.md) — Communication protocol details
- [context-sharing.md](./context-sharing.md) — Context management strategies compared
- [evaluation-agent.md](./evaluation-agent.md) — Verification approaches compared
- [real-world-examples.md](./real-world-examples.md) — Deep dives into implementations
- [swarm-patterns.md](./swarm-patterns.md) — Swarm/handoff patterns
- [peer-to-peer.md](./peer-to-peer.md) — P2P patterns (theoretical)

- [orchestrator-worker.md](./orchestrator-worker.md) — Detailed orchestrator-worker pattern analysis
- [specialist-agents.md](./specialist-agents.md) — How specialist roles are designed
- [communication-protocols.md](./communication-protocols.md) — Communication protocol details
- [context-sharing.md](./context-sharing.md) — Context management strategies compared
- [evaluation-agent.md](./evaluation-agent.md) — Verification approaches compared
- [real-world-examples.md](./real-world-examples.md) — Deep dives into implementations
- [swarm-patterns.md](./swarm-patterns.md) — Swarm/handoff patterns
- [peer-to-peer.md](./peer-to-peer.md) — P2P patterns (theoretical)

---

## References

- All agent research files: `/research/agents/*/`
- Anthropic. "Building Effective Agents." 2024. https://www.anthropic.com/research/building-effective-agents
- OpenAI. "Agents SDK." 2025. https://github.com/openai/openai-agents-python
- Google. "Agent2Agent Protocol." 2025. https://github.com/a2aproject/A2A
- MetaGPT. https://github.com/geekan/MetaGPT
- Microsoft AutoGen. https://github.com/microsoft/autogen
- CrewAI. https://github.com/crewAIInc/crewAI
