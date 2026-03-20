# Agent Design Pattern Comparison

> Mapping 17 CLI coding agents to Anthropic's design patterns

## Overview

This document provides a comprehensive cross-reference between 17 CLI coding agents and the
seven design patterns identified in Anthropic's "Building Effective Agents" blog. Rather than
treating each pattern as a binary choice, we analyze the degree to which each agent implements
each pattern, how patterns interact within a single agent, and what novel patterns emerge from
real-world implementations that extend beyond Anthropic's taxonomy.

The analysis reveals that top-performing agents (ForgeCode, Claude Code, OpenHands) implement
4-6 patterns simultaneously, while simpler agents (mini-SWE-agent, Pi Coding Agent) achieve
surprising effectiveness with 1-2 patterns. The relationship between pattern complexity and
agent performance is not linear—it depends on how well patterns are composed and whether the
complexity serves the use case.

---

## Design Pattern Taxonomy

A brief recap of Anthropic's seven patterns, ordered from simplest to most complex:

### 1. Augmented LLM (Building Block)
The foundational building block: an LLM enhanced with retrieval, tools, and memory. Not a
workflow pattern per se, but the substrate on which all other patterns build. Every agent
implements this at minimum.

### 2. Prompt Chaining (Workflow)
Sequential LLM calls where each step's output feeds the next step's input, with optional
validation gates between steps. Best for tasks with predictable, ordered subtasks.

### 3. Routing (Workflow)
A classification step that directs input to specialized handlers. Enables different
processing paths based on input characteristics, task type, or complexity level.

### 4. Parallelization (Workflow)
Two variants: **Sectioning** splits a task into independent subtasks that run concurrently.
**Voting** runs the same task multiple times and aggregates results for reliability.

### 5. Orchestrator-Workers (Workflow)
A central orchestrator dynamically decomposes tasks, delegates to worker agents, and
synthesizes their outputs. Unlike prompt chaining, the decomposition is dynamic—the
orchestrator decides what workers to spawn based on the task.

### 6. Evaluator-Optimizer (Workflow)
A generation-evaluation loop: one LLM generates output, another (or the same) evaluates it,
and the generator refines based on feedback. Continues until quality criteria are met or
iteration limits are reached.

### 7. Autonomous Agent (Architecture)
An LLM operating in a loop with tool access and environment feedback. The agent decides what
to do next based on observations, with minimal human intervention. The most flexible but
least predictable pattern.

---

## Master Comparison Table

This table maps all 17 agents against all 7 patterns. Symbols indicate the degree of
implementation:

- ✅ = Primary pattern (core to the agent's architecture)
- ◐ = Partial/implicit (present but not the primary mechanism)
- ○ = Minimal or absent

| # | Agent | Augmented LLM | Prompt Chaining | Routing | Parallelization | Orchestrator-Workers | Evaluator-Optimizer | Autonomous Agent |
|---|-------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| 1 | ForgeCode | ✅ | ◐ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 2 | Claude Code | ✅ | ◐ | ◐ | ✅ | ✅ | ✅ | ✅ |
| 3 | Codex CLI | ✅ | ○ | ○ | ◐ | ○ | ◐ | ✅ |
| 4 | Droid | ✅ | ◐ | ✅ | ◐ | ✅ | ◐ | ✅ |
| 5 | Ante | ✅ | ○ | ✅ | ✅ | ✅ | ◐ | ✅ |
| 6 | OpenCode | ✅ | ○ | ◐ | ○ | ○ | ○ | ✅ |
| 7 | OpenHands | ✅ | ○ | ○ | ○ | ◐ | ✅ | ✅ |
| 8 | Warp | ✅ | ○ | ◐ | ○ | ○ | ○ | ◐ |
| 9 | Gemini CLI | ✅ | ○ | ◐ | ○ | ○ | ◐ | ✅ |
| 10 | Goose | ✅ | ○ | ✅ | ○ | ◐ | ○ | ✅ |
| 11 | Junie CLI | ✅ | ✅ | ✅ | ○ | ◐ | ✅ | ◐ |
| 12 | mini-SWE-agent | ✅ | ○ | ○ | ○ | ○ | ○ | ✅ |
| 13 | Pi Coding Agent | ✅ | ○ | ◐ | ○ | ○ | ○ | ✅ |
| 14 | Aider | ✅ | ◐ | ○ | ○ | ○ | ✅ | ◐ |
| 15 | Sage Agent | ✅ | ✅ | ◐ | ○ | ✅ | ◐ | ◐ |
| 16 | TongAgents | ✅ | ◐ | ◐ | ◐ | ◐ | ◐ | ✅ |
| 17 | Capy | ✅ | ○ | ○ | ✅ | ✅ | ◐ | ✅ |

### Reading the Table

The table reveals several patterns:

1. **Augmented LLM is universal**: Every agent implements it. This is the foundation.
2. **Autonomous Agent is near-universal**: 13 of 17 agents use it as a primary pattern.
   The exceptions (Warp, Junie CLI, Aider, Sage Agent) use more structured approaches.
3. **Prompt Chaining is rare as primary**: Only Junie CLI and Sage Agent use explicit,
   fixed-sequence pipelines as their primary flow.
4. **Routing separates tiers**: Tier 1 agents (ForgeCode, Droid, Ante) heavily use routing.
   Tier 3 agents mostly don't.
5. **Pattern count correlates with tier**: Tier 1 averages 5.1 primary/partial patterns.
   Tier 2 averages 3.3. Tier 3 averages 3.0. But mini-SWE-agent is a notable outlier.

---

## Per-Agent Pattern Analysis

### ForgeCode

**Tier 1 — #1 Terminal-Bench 2.0 (81.8%)**

| Pattern | Implementation | Details |
|---------|:---:|---------|
| Augmented LLM | ✅ | ZSH-native shell integration, rich tool access |
| Prompt Chaining | ◐ | Implicit sequencing within sub-agent pipelines |
| Routing | ✅ | Model routing per phase — different models for planning, execution, review |
| Parallelization | ✅ | Multiple sub-agents can execute concurrently |
| Orchestrator-Workers | ✅ | Forge/Muse/Sage sub-agent architecture |
| Evaluator-Optimizer | ✅ | Sub-agent results feed back to orchestrator for refinement |
| Autonomous Agent | ✅ | Core agentic loop with environment feedback |

**Pattern interaction**: ForgeCode's architecture is a textbook example of pattern composition.
The top-level loop is an autonomous agent. Within that loop, a routing layer selects the
appropriate model for each phase. Complex tasks trigger orchestrator-workers with the three
named sub-agents (Forge for core work, Muse for creative tasks, Sage for analysis). Sub-agents
can run in parallel when their tasks are independent. The orchestrator evaluates sub-agent
outputs and can trigger refinement cycles.

**Why it works**: The multi-agent architecture allows ForgeCode to apply maximum capability
where it matters (frontier models for planning) and maximum efficiency where it doesn't
(fast models for classification). The Rust implementation ensures the orchestration overhead
is minimal.

**Key insight**: ForgeCode proves that layering many patterns can work when the orchestration
is fast and the routing is intelligent. The 81.8% Terminal-Bench score suggests that pattern
composition, not pattern simplicity, drives top performance.

---

### Claude Code

**Tier 1 — Anthropic's flagship CLI agent**

| Pattern | Implementation | Details |
|---------|:---:|---------|
| Augmented LLM | ✅ | TypeScript, CLAUDE.md memory, context compaction, rich tool suite |
| Prompt Chaining | ◐ | Implicit in how the main loop sequences operations |
| Routing | ◐ | Graduated permissions act as a form of routing (safe vs risky operations) |
| Parallelization | ✅ | Explore sub-agents can run in parallel for codebase investigation |
| Orchestrator-Workers | ✅ | Main loop as orchestrator, Explore/Plan/custom sub-agents as workers |
| Evaluator-Optimizer | ✅ | Tool feedback loops — observes command output, adjusts approach |
| Autonomous Agent | ✅ | Core architecture is a single agentic loop |

**Pattern interaction**: Claude Code's elegance lies in its single-loop architecture that
implicitly implements multiple patterns. The main agentic loop handles most tasks directly
(augmented LLM + autonomous agent). When tasks are complex, it spawns sub-agents—this is
orchestrator-workers but triggered dynamically rather than architecturally mandated. The
tool feedback loop (run command → observe output → adjust) is a natural evaluator-optimizer.

**Graduated permissions as routing**: Claude Code's permission system is an underappreciated
form of routing. Operations are classified by risk level and routed through different
approval paths. This is not traditional input-type routing but operational-risk routing.

**Context compaction as enabler**: Context compaction allows the autonomous agent loop to run
longer without hitting context limits. This extends the viable duration of the agent pattern,
making it practical for larger tasks that would otherwise exceed context windows.

---

### Codex CLI

**Tier 1 — OpenAI's sandboxed agent**

| Pattern | Implementation | Details |
|---------|:---:|---------|
| Augmented LLM | ✅ | Rust implementation, Ratatui TUI, tool access within sandbox |
| Prompt Chaining | ○ | No explicit sequential pipeline |
| Routing | ○ | No explicit routing mechanism |
| Parallelization | ◐ | SQ/EQ architecture implies some concurrent processing |
| Orchestrator-Workers | ○ | Single agent, no sub-agent delegation |
| Evaluator-Optimizer | ◐ | Implicit through sandbox feedback (run → observe → adjust) |
| Autonomous Agent | ✅ | Primary pattern — LLM loop with strong sandbox |

**Pattern interaction**: Codex CLI is architecturally simple: a single autonomous agent loop
with a uniquely strong sandbox. The 3-layer OS sandbox (macOS Seatbelt, Linux bubblewrap +
seccomp, Windows ACLs) is not a pattern from Anthropic's taxonomy but is arguably the most
important architectural decision in the agent.

**SQ/EQ architecture**: The SQ (System Quality) and EQ (Experience Quality) dual architecture
hints at parallelization—different quality dimensions can be evaluated independently. However,
this is more of an internal design philosophy than an explicit parallelization pattern.

**Simplicity as strategy**: Codex CLI proves that a single, well-implemented autonomous agent
with a strong sandbox can be highly effective without orchestration, routing, or explicit
evaluation loops. The sandbox provides the error tolerance that other agents achieve through
pattern complexity.

---

### Droid

**Tier 1 — Factory.ai enterprise agent**

| Pattern | Implementation | Details |
|---------|:---:|---------|
| Augmented LLM | ✅ | Multi-interface access (IDE, Web, CLI, Slack, Linear, CI/CD) |
| Prompt Chaining | ◐ | Enterprise workflows often follow structured sequences |
| Routing | ✅ | Multi-interface routing — same agent, different entry points |
| Parallelization | ◐ | Can process multiple tasks from different interfaces concurrently |
| Orchestrator-Workers | ✅ | Central agent orchestrating across interfaces and tools |
| Evaluator-Optimizer | ◐ | Proprietary compaction with anchor points implies quality assessment |
| Autonomous Agent | ✅ | Core loop operates autonomously across interfaces |

**Pattern interaction**: Droid's distinctive feature is multi-interface routing. The same
agent can be invoked from an IDE, a web interface, CLI, Slack, Linear, or CI/CD pipelines.
This is routing at the interface level rather than the task level—the entry point determines
the interaction mode, available context, and response format.

**Model/vendor agnostic routing**: Droid routes not just between interfaces but between
model providers. This is a meta-routing layer: choose the best model vendor for the current
task, then route the task to the appropriate interface handler.

**Proprietary compaction with anchor points**: Droid's context compaction preserves "anchor
points"—key context elements that must be retained across compaction cycles. This enables
longer autonomous agent loops without context degradation, similar to Claude Code's
compaction but with explicit anchor management.

---

### Ante

**Tier 1 — Antigma Labs' self-organizing agent**

| Pattern | Implementation | Details |
|---------|:---:|---------|
| Augmented LLM | ✅ | Own inference stack, offline-first, Rust implementation |
| Prompt Chaining | ○ | Self-organizing contradicts fixed pipelines |
| Routing | ✅ | Meta-agent orchestrator routes tasks to appropriate sub-agents |
| Parallelization | ✅ | Lock-free scheduling enables concurrent sub-agent execution |
| Orchestrator-Workers | ✅ | Self-organizing multi-agent with dynamic worker creation |
| Evaluator-Optimizer | ◐ | Meta-agent evaluates sub-agent performance and adjusts |
| Autonomous Agent | ✅ | Core autonomous loop with self-organizing capability |

**Pattern interaction**: Ante represents the most dynamic form of orchestrator-workers. Rather
than having pre-defined sub-agents (like ForgeCode's Forge/Muse/Sage), Ante's meta-agent
orchestrator creates and assigns sub-agents dynamically based on the task at hand. This is
self-organizing orchestration—the worker topology adapts to the task.

**Lock-free scheduling**: Ante's lock-free scheduling is a parallelization enabler at the
systems level. Sub-agents can execute concurrently without coordination overhead, which
makes the parallelization pattern practical even for fine-grained subtasks.

**Offline-first with own inference stack**: By controlling the inference stack, Ante can
optimize model routing internally rather than relying on external API routing. This tight
integration between the routing pattern and the execution infrastructure is unique among
the 17 agents.

---

### OpenCode

**Tier 1 — Open-source Go TUI**

| Pattern | Implementation | Details |
|---------|:---:|---------|
| Augmented LLM | ✅ | Bubble Tea TUI, SQLite persistence, 10+ provider support |
| Prompt Chaining | ○ | No explicit pipeline |
| Routing | ◐ | Provider routing — choose from 10+ model providers |
| Parallelization | ○ | Single-threaded agent loop |
| Orchestrator-Workers | ○ | No sub-agent delegation |
| Evaluator-Optimizer | ○ | No explicit evaluation loop |
| Autonomous Agent | ✅ | Standard agentic loop with tool use |

**Pattern interaction**: OpenCode is architecturally straightforward: an augmented LLM in an
autonomous agent loop with excellent developer experience. The Bubble Tea TUI framework
provides a polished terminal interface, and SQLite persistence enables cross-session state.

**Provider routing as the key differentiator**: With 10+ provider support, OpenCode's routing
is at the provider/model level rather than the task level. Users choose (or the agent selects)
the most appropriate model from a diverse set of providers. This is a form of routing that
operates at the infrastructure level.

**Pub/sub events**: OpenCode's event system (pub/sub) is an architectural pattern that enables
loose coupling between components. While not directly one of Anthropic's patterns, it provides
the infrastructure for future pattern additions (e.g., parallelization through event-driven
worker coordination).

---

### OpenHands

**Tier 1 — Event-driven autonomous agent**

| Pattern | Implementation | Details |
|---------|:---:|---------|
| Augmented LLM | ✅ | CodeAct paradigm — bash + IPython as unified action space |
| Prompt Chaining | ○ | No fixed pipeline; dynamic sequencing |
| Routing | ○ | No explicit routing; single processing path |
| Parallelization | ○ | Single-agent execution |
| Orchestrator-Workers | ◐ | EventStream enables loose worker coordination |
| Evaluator-Optimizer | ✅ | StuckDetector + Condenser system for loop management |
| Autonomous Agent | ✅ | Primary pattern — event-driven autonomous loop |

**Pattern interaction**: OpenHands' architecture is built around the EventStream pub/sub
system. Every action and observation flows through the event stream, creating a natural
feedback loop. The StuckDetector monitors this stream for patterns indicating the agent is
stuck (repeated actions, circular reasoning) and intervenes—this is an evaluator-optimizer
at the meta-level, evaluating the agent's process rather than its output.

**CodeAct as augmented LLM variant**: Instead of defining tools as JSON-schema functions,
CodeAct gives the LLM direct access to bash and IPython. This is a fundamentally different
approach to tool augmentation—rather than structured tool calls, the agent writes and
executes code directly. This trades the safety of structured tools for the flexibility of
arbitrary code execution (within a Docker sandbox).

**Condenser system**: The Condenser manages context by compressing the event history. This is
not an Anthropic pattern per se, but it enables the autonomous agent pattern to run for much
longer than context limits would otherwise allow. Similar in purpose to Claude Code's context
compaction and Droid's anchor-point compaction.

---

### Warp

**Tier 2 — AI-native terminal**

| Pattern | Implementation | Details |
|---------|:---:|---------|
| Augmented LLM | ✅ | Full PTY ownership, Metal GPU rendering, block-per-command |
| Prompt Chaining | ○ | No explicit pipeline |
| Routing | ◐ | Oz agent platform may route between capabilities |
| Parallelization | ○ | Single interaction model |
| Orchestrator-Workers | ○ | No sub-agent delegation |
| Evaluator-Optimizer | ○ | No explicit evaluation loop |
| Autonomous Agent | ◐ | Oz platform enables agent-like behavior |

**Pattern interaction**: Warp is unique in the set because it's fundamentally a *terminal*
with AI capabilities rather than an AI agent with terminal access. This inverts the typical
relationship: instead of an LLM that can run shell commands, it's a shell that can invoke
LLMs.

**Terminal-as-augmentation**: By owning the full PTY and rendering pipeline, Warp has access
to terminal context that other agents don't: command history, output formatting, error
patterns, environment state. This is augmented LLM where the augmentation comes from deep
terminal integration rather than explicit tool definitions.

**Block-per-command model**: Warp's block model (each command and its output form a discrete
block) provides natural segmentation for context. This is an implicit form of prompt chaining
where each block can inform the next interaction, but it's driven by user behavior rather
than agent orchestration.

**Oz agent platform**: Warp's newer Oz platform moves toward more autonomous agent patterns,
but the primary value proposition remains the AI-enhanced terminal experience.

---

### Gemini CLI

**Tier 2 — Google's million-token agent**

| Pattern | Implementation | Details |
|---------|:---:|---------|
| Augmented LLM | ✅ | 1M token context, GEMINI.md memory, multimodal input |
| Prompt Chaining | ○ | No explicit pipeline |
| Routing | ◐ | Progressive skill disclosure acts as capability routing |
| Parallelization | ○ | Single-agent execution |
| Orchestrator-Workers | ○ | No sub-agent delegation |
| Evaluator-Optimizer | ◐ | Git checkpoint shadow repos enable rollback-and-retry |
| Autonomous Agent | ✅ | Standard agentic loop with massive context |

**Pattern interaction**: Gemini CLI's primary innovation is using massive context (1M tokens)
to reduce the need for complex patterns. Where other agents need orchestrator-workers to
break down large tasks (because the context can't hold everything at once), Gemini CLI can
potentially hold the entire relevant codebase in context and reason over it directly.

**Progressive skill disclosure as routing**: Gemini CLI reveals capabilities progressively
based on user expertise. This is a user-facing routing mechanism: novice users see simpler
tools, expert users get advanced capabilities. The routing happens at the interaction level
rather than the task level.

**Git checkpoint shadow repos**: By maintaining shadow git repositories, Gemini CLI can
checkpoint state at any point and rollback on failure. This is an implicit evaluator-optimizer
pattern: try an approach, evaluate the result, and if it fails, rollback and try differently.
The evaluation is done by the user or the agent observing test results rather than by a
dedicated evaluator component.

---

### Goose

**Tier 2 — Block's MCP-native agent**

| Pattern | Implementation | Details |
|---------|:---:|---------|
| Augmented LLM | ✅ | MCP-native with 7 transport types, extensive tool access |
| Prompt Chaining | ○ | No fixed pipeline |
| Routing | ✅ | Multi-layered tool inspection pipeline |
| Parallelization | ○ | Single-agent primary loop |
| Orchestrator-Workers | ◐ | Summon sub-agents for specialized tasks |
| Evaluator-Optimizer | ○ | No explicit generation-evaluation loop |
| Autonomous Agent | ✅ | Core agentic loop with MCP tool use |

**Pattern interaction**: Goose's most distinctive pattern is its multi-layered tool inspection
pipeline: Security → Adversary → Permission → Repetition. Every tool call passes through four
inspection layers before execution. This is routing applied to tool safety rather than task
classification—each layer can block, modify, or approve the tool call.

**MCP-native composition**: Goose supports 7 MCP transport types, making it the most
MCP-integrated agent in the set. This means tool access is not hard-coded but dynamically
composed through MCP servers. The augmented LLM pattern is implemented through protocol-
level composition rather than code-level integration.

**MOIM (Model-Oriented Instruction Mapping)**: Goose uses MOIM to map user instructions to
model-appropriate formats. This is a form of input routing that adapts the prompt based on
the target model's strengths and expected formats.

**Summon sub-agents**: Goose can spawn ("summon") specialized sub-agents for tasks that
require different capabilities. This is a lightweight orchestrator-workers pattern where
sub-agents are created on demand rather than being persistent architectural components.

---

### Junie CLI

**Tier 2 — JetBrains' test-driven agent**

| Pattern | Implementation | Details |
|---------|:---:|---------|
| Augmented LLM | ✅ | PSI-based language understanding, dual IDE/CLI operation |
| Prompt Chaining | ✅ | Explicit 6-step pipeline: understand→plan→implement→verify→iterate→present |
| Routing | ✅ | Dynamic per-task model routing selects optimal model |
| Parallelization | ○ | Sequential pipeline execution |
| Orchestrator-Workers | ◐ | Pipeline stages can be seen as specialized workers |
| Evaluator-Optimizer | ✅ | Verify→iterate loop is explicit evaluation-optimization |
| Autonomous Agent | ◐ | Operates autonomously within the pipeline structure |

**Pattern interaction**: Junie CLI is the clearest example of prompt chaining among the 17
agents. The 6-step pipeline (understand → plan → implement → verify → iterate → present) is
an explicit, ordered sequence with clear handoff points between stages.

**Pipeline as structured autonomy**: Within the pipeline structure, each stage operates with
some autonomy, but the overall flow is deterministic. This is a compromise between the
predictability of prompt chaining and the flexibility of autonomous agents.

**Test-driven evaluation**: The verify → iterate loop is a natural evaluator-optimizer where
tests serve as the evaluator. If tests fail, the iterate step refines the implementation.
This is particularly effective because the evaluation criterion is objective (tests pass or
fail) rather than subjective (is this code "good"?).

**PSI-based language understanding**: JetBrains' PSI (Program Structure Interface) gives
Junie CLI deep, parser-level understanding of code structure. This augments the LLM with
structural information that other agents can only get through tree-sitter or similar tools.

---

### mini-SWE-agent

**Tier 2 — Radical simplicity**

| Pattern | Implementation | Details |
|---------|:---:|---------|
| Augmented LLM | ✅ | ~100 lines Python, bash-only tool access |
| Prompt Chaining | ○ | No pipeline |
| Routing | ○ | No routing |
| Parallelization | ○ | No parallelization |
| Orchestrator-Workers | ○ | No orchestration |
| Evaluator-Optimizer | ○ | No explicit evaluation |
| Autonomous Agent | ✅ | Minimal but complete autonomous loop |

**Pattern interaction**: mini-SWE-agent is the control case. With approximately 100 lines of
Python, it implements only two patterns: augmented LLM (bash as the sole tool) and autonomous
agent (the LLM loop). Everything else is left to the model's inherent capability.

**The scaffold complexity thesis**: mini-SWE-agent's creator argues that "scaffold complexity
has diminishing returns." The agent's competitive performance on benchmarks supports this—
much of what complex scaffolding provides can be replicated by a sufficiently capable model
with basic tool access.

**Implications for pattern selection**: mini-SWE-agent suggests that the augmented LLM +
autonomous agent combination is the irreducible core of a coding agent. All other patterns
are optimizations that improve performance, reliability, or efficiency—but they're not
strictly necessary. This is a powerful baseline for evaluating whether additional patterns
justify their complexity cost.

---

### Pi Coding Agent

**Tier 2 — Primitives over features**

| Pattern | Implementation | Details |
|---------|:---:|---------|
| Augmented LLM | ✅ | 4 tools only: read, write, edit, bash |
| Prompt Chaining | ○ | No pipeline |
| Routing | ◐ | Extension API enables capability routing |
| Parallelization | ○ | No parallelization |
| Orchestrator-Workers | ○ | No orchestration |
| Evaluator-Optimizer | ○ | No explicit evaluation |
| Autonomous Agent | ✅ | Standard agentic loop with minimal tools |

**Pattern interaction**: Pi Coding Agent takes a "primitives over features" philosophy,
providing only four tools (read, write, edit, bash) and relying on the model to compose
them into higher-level operations. The extension API is the mechanism for adding capability
—but the core agent remains minimal.

**Extension API as routing**: While Pi Coding Agent doesn't have explicit routing, its
extension API allows external systems to add capabilities. This is a form of deferred
routing—the routing decisions are made by the extension ecosystem rather than the agent
itself.

**Cross-provider context handoff**: Pi Coding Agent supports handing off context between
different model providers. This is a unique form of routing at the session level: start a
task with one model, hand off to another when the task characteristics change.

---

### Aider

**Tier 2 — Python pair programmer**

| Pattern | Implementation | Details |
|---------|:---:|---------|
| Augmented LLM | ✅ | Repo-map (tree-sitter + PageRank), git-native, multiple edit formats |
| Prompt Chaining | ◐ | Architect mode uses plan→implement two-step chain |
| Routing | ○ | No explicit routing |
| Parallelization | ○ | Single-model execution |
| Orchestrator-Workers | ○ | No sub-agent delegation |
| Evaluator-Optimizer | ✅ | Edit→test→fix loop, benchmark-driven development |
| Autonomous Agent | ◐ | Can loop autonomously but typically more interactive |

**Pattern interaction**: Aider's most significant contribution is **edit format innovation**.
It supports multiple edit formats (whole file, search/replace, diff, architect), each
optimized for different model capabilities and task types. This is not routing in Anthropic's
sense but is a form of output format optimization that significantly impacts quality.

**Repo-map as augmented LLM**: Aider's repo-map (built with tree-sitter + PageRank) provides
the LLM with a structural understanding of the entire repository. This is one of the most
sophisticated augmentation strategies—it doesn't just give the LLM files, it gives it a
ranked understanding of which files and symbols are most relevant.

**Architect mode as prompt chaining**: In architect mode, a planning model designs the
approach and a coding model implements it. This is a two-step prompt chain with an implicit
quality gate between planning and implementation.

**Benchmark-driven evaluator-optimizer**: Aider's development process uses benchmarks
(SWE-bench, etc.) as the evaluator for optimizing edit formats, prompts, and model
configurations. This is evaluator-optimizer applied to the agent's own development, not just
to individual tasks.

---

### Sage Agent

**Tier 3 — Pipeline-based agent**

| Pattern | Implementation | Details |
|---------|:---:|---------|
| Augmented LLM | ✅ | MCP-native tool access |
| Prompt Chaining | ✅ | Explicit 5-agent pipeline: TaskAnalysis→Planning→Executor→Observation→TaskSummary |
| Routing | ◐ | Dual modes (Deep Research/Rapid Execution) act as top-level routing |
| Parallelization | ○ | Sequential pipeline execution |
| Orchestrator-Workers | ✅ | Each pipeline stage is a specialized agent/worker |
| Evaluator-Optimizer | ◐ | Observation stage evaluates Executor output |
| Autonomous Agent | ◐ | Pipeline stages have limited autonomy |

**Pattern interaction**: Sage Agent is the most explicit implementation of prompt chaining in
the set, with five named agents forming a strict pipeline. Each agent specializes in one
phase: TaskAnalysis understands the problem, Planning creates the approach, Executor
implements, Observation evaluates, and TaskSummary synthesizes.

**Dual modes as routing**: The choice between Deep Research mode and Rapid Execution mode is
a top-level routing decision. Deep Research mode runs the full pipeline with thorough analysis
at each stage. Rapid Execution mode shortcuts or abbreviates pipeline stages for faster
results. This is routing applied to the pipeline's thoroughness level.

**Pipeline as orchestrator-workers**: While the pipeline is sequential (prompt chaining), each
stage is a specialized agent (orchestrator-workers). The distinction is that the
"orchestration" is static—the pipeline order is fixed, not dynamically determined. This makes
Sage Agent a hybrid: prompt chaining for the flow, orchestrator-workers for the execution
of each stage.

---

### TongAgents

**Tier 3 — Closed/unpublished**

| Pattern | Implementation | Details |
|---------|:---:|---------|
| Augmented LLM | ✅ | Inferred from Terminal-Bench performance |
| Prompt Chaining | ◐ | Likely structured workflow given benchmark performance |
| Routing | ◐ | Possible multi-model routing (inferred) |
| Parallelization | ◐ | Possible concurrent execution (inferred) |
| Orchestrator-Workers | ◐ | Likely multi-agent given the team (BIGAI) |
| Evaluator-Optimizer | ◐ | Likely present given benchmark ranking |
| Autonomous Agent | ✅ | Core pattern for Terminal-Bench performance |

**Pattern interaction**: TongAgents from BIGAI ranks top-3 on Terminal-Bench 2.0 (80.2%) but
remains closed and unpublished. Analysis is necessarily speculative, based on the
performance characteristics and the research group's known expertise.

**Inferred architecture**: Top-3 Terminal-Bench performance typically requires sophisticated
multi-pattern composition. Given BIGAI's research focus on multi-agent systems, TongAgents
likely implements orchestrator-workers with some form of model routing. The high benchmark
score suggests effective evaluation and iteration capabilities.

**Research implications**: TongAgents' closed nature makes it a reminder that some of the most
effective agent architectures may not be publicly documented. The patterns we can study in
open-source agents may not represent the full design space.

---

### Capy

**Tier 3 — Cloud IDE agent**

| Pattern | Implementation | Details |
|---------|:---:|---------|
| Augmented LLM | ✅ | Cloud IDE integration, sandboxed Ubuntu VMs |
| Prompt Chaining | ○ | No fixed pipeline |
| Routing | ○ | No explicit routing beyond Captain/Build split |
| Parallelization | ✅ | 25+ concurrent tasks, git worktrees |
| Orchestrator-Workers | ✅ | Captain (plans) / Build (executes) two-agent split |
| Evaluator-Optimizer | ◐ | Captain can evaluate Build's output and iterate |
| Autonomous Agent | ✅ | Both Captain and Build operate autonomously |

**Pattern interaction**: Capy's architecture is a clean implementation of orchestrator-workers
with the Captain/Build split. Captain handles planning and decomposition; Build handles
execution. This separation of concerns is explicit and architectural.

**Parallelization at scale**: Capy supports 25+ concurrent tasks using git worktrees for
isolation. This is the most aggressive parallelization in the set—not just parallel sub-agents
within a task, but parallel tasks at the workspace level. Each concurrent task gets its own
git worktree, preventing interference.

**Sandboxed Ubuntu VMs**: Each task runs in a sandboxed Ubuntu VM, providing strong isolation.
This enables aggressive autonomy (the agent can do anything within the VM) without risk to the
host system. Similar philosophy to Codex CLI's sandbox-first approach but at the VM level
rather than the OS sandbox level.

---

## Pattern Adoption Trends

### Most Common Patterns

Ranked by adoption across all 17 agents (counting ✅ and ◐):

| Pattern | ✅ Primary | ◐ Partial | Total | Adoption Rate |
|---------|:---------:|:--------:|:-----:|:------------:|
| Augmented LLM | 17 | 0 | 17 | 100% |
| Autonomous Agent | 13 | 4 | 17 | 100% |
| Routing | 5 | 7 | 12 | 71% |
| Evaluator-Optimizer | 4 | 7 | 11 | 65% |
| Orchestrator-Workers | 5 | 4 | 9 | 53% |
| Prompt Chaining | 2 | 5 | 7 | 41% |
| Parallelization | 4 | 3 | 7 | 41% |

### Pattern Combination Frequency

The most common pattern combinations among the 17 agents:

1. **Augmented LLM + Autonomous Agent** (17/17) — The universal base
2. **+ Evaluator-Optimizer** (11/17) — Adding quality iteration
3. **+ Routing** (12/17) — Adding input/task classification
4. **+ Orchestrator-Workers** (9/17) — Adding delegation capability
5. **+ Parallelization** (7/17) — Adding concurrent execution

### Emerging Patterns Not in Anthropic's Framework

Several patterns observed across the 17 agents don't map cleanly to Anthropic's seven:

1. **Context management as pattern**: Compaction (Claude Code, Droid), Condensing (OpenHands),
   massive context (Gemini CLI) — managing the agent's memory is a cross-cutting concern
2. **Safety-as-architecture**: Sandboxing (Codex CLI, Capy), inspection pipelines (Goose),
   graduated permissions (Claude Code) — safety mechanisms shape the entire architecture
3. **Tool composition via protocol**: MCP-native design (Goose) enables dynamic tool
   composition that transcends hard-coded tool definitions
4. **Edit format optimization**: Aider's multiple edit formats are a unique pattern for
   optimizing model-to-code translation

---

## Complexity vs Performance

### Do More Patterns Equal Better Performance?

| Agent | Pattern Count (✅) | Pattern Count (✅+◐) | Terminal-Bench | Tier |
|-------|:-----------------:|:-------------------:|:-------------:|:----:|
| ForgeCode | 6 | 7 | 81.8% | 1 |
| Claude Code | 5 | 7 | — | 1 |
| Codex CLI | 2 | 4 | — | 1 |
| Ante | 5 | 6 | — | 1 |
| Droid | 4 | 7 | — | 1 |
| OpenHands | 3 | 4 | — | 1 |
| mini-SWE-agent | 2 | 2 | — | 2 |
| TongAgents | 2 | 7 | 80.2% | 3 |
| Capy | 4 | 5 | — | 3 |

**Observations:**

1. **High pattern count correlates with Tier 1** — but it's not sufficient. Pattern
   implementation quality matters more than pattern count.
2. **mini-SWE-agent is the outlier**: Only 2 patterns, yet competitive. This validates
   the "scaffold complexity has diminishing returns" thesis for well-defined tasks.
3. **Codex CLI (2 primary patterns) is Tier 1**: Strong sandbox + simple loop can be as
   effective as complex multi-agent architectures for the right use cases.
4. **ForgeCode (6 primary patterns) tops benchmarks**: Maximum pattern composition works
   when the orchestration is fast and the routing is intelligent.

**Conclusion**: Pattern count is neither necessary nor sufficient for performance. What
matters is whether the patterns address the specific bottlenecks the agent faces. ForgeCode
benefits from routing because it uses multiple models. mini-SWE-agent doesn't need routing
because it uses one model and minimal scaffold. Both are optimal for their design philosophy.

---

## Patterns by Architecture

### Single-Agent Architectures

Agents that use a single LLM loop without sub-agent delegation:

| Agent | Key Patterns | Architecture Style |
|-------|-------------|-------------------|
| Codex CLI | Augmented LLM + Autonomous Agent | Sandbox-first single loop |
| OpenCode | Augmented LLM + Autonomous Agent | Provider-agnostic single loop |
| mini-SWE-agent | Augmented LLM + Autonomous Agent | Minimal single loop |
| Pi Coding Agent | Augmented LLM + Autonomous Agent | Extension-based single loop |
| Gemini CLI | Augmented LLM + Autonomous Agent | Massive-context single loop |
| Warp | Augmented LLM | Terminal-integrated |

**Common thread**: Single-agent architectures rely on model capability and tool access rather
than architectural complexity. They scale by improving the model or expanding tool access,
not by adding more agents.

### Multi-Agent Architectures

Agents that delegate to sub-agents:

| Agent | Key Patterns | Architecture Style |
|-------|-------------|-------------------|
| ForgeCode | Orchestrator-Workers + Routing + Parallelization | Named sub-agents (Forge/Muse/Sage) |
| Claude Code | Orchestrator-Workers + Evaluator-Optimizer | Dynamic sub-agent spawning |
| Ante | Orchestrator-Workers + Routing + Parallelization | Self-organizing sub-agents |
| Droid | Orchestrator-Workers + Routing | Multi-interface sub-agents |
| Capy | Orchestrator-Workers + Parallelization | Captain/Build split |
| Goose | Augmented LLM + Routing | Summon-based sub-agents |

**Common thread**: Multi-agent architectures add complexity to handle tasks that exceed a
single agent's effective scope. They sacrifice simplicity for capability.

### Event-Driven Architectures

| Agent | Key Patterns | Architecture Style |
|-------|-------------|-------------------|
| OpenHands | Autonomous Agent + Evaluator-Optimizer | EventStream pub/sub |
| OpenCode | Augmented LLM + Autonomous Agent | Pub/sub events |

**Common thread**: Event-driven architectures decouple action from observation, enabling
flexible composition and monitoring (e.g., StuckDetector subscribing to the event stream).

### Pipeline Architectures

| Agent | Key Patterns | Architecture Style |
|-------|-------------|-------------------|
| Junie CLI | Prompt Chaining + Evaluator-Optimizer | 6-step fixed pipeline |
| Sage Agent | Prompt Chaining + Orchestrator-Workers | 5-agent fixed pipeline |
| Aider | Augmented LLM + Evaluator-Optimizer | Architect two-step pipeline |

**Common thread**: Pipeline architectures trade flexibility for predictability. They work
best when the task structure is known in advance and the pipeline stages are well-matched
to the task decomposition.

---

## Novel Patterns Not in Anthropic's Framework

### CodeAct (OpenHands): Unified Action Space

Instead of defining tools as structured function calls, CodeAct gives the LLM direct access
to bash and IPython. The "tool" is the ability to write and execute arbitrary code. This
collapses the distinction between "thinking" and "acting"—the model's output IS the action.

**Why it's novel**: Anthropic's augmented LLM pattern assumes structured tool calls. CodeAct
removes the structure, trading safety for flexibility. The Docker sandbox provides the safety
that structured tools normally provide.

**Implication**: As models become more capable at code generation, the structured tool call
pattern may become unnecessary overhead. CodeAct points toward a future where the model's
code IS the tool interface.

### Edit Format Innovation (Aider): Output Format as Pattern

Aider supports four edit formats: whole file, search/replace, diff, and architect. Each
format represents a different tradeoff between model capability, output reliability, and
token efficiency.

**Why it's novel**: Anthropic's patterns focus on *how* the LLM is orchestrated, not *what
format* its output takes. Aider demonstrates that the output format is itself a design
decision with significant performance implications.

**Implication**: Agent builders should treat output format as a first-class design dimension,
not just a serialization detail.

### Terminal-as-Agent (Warp): Environment as Architecture

Warp doesn't add AI to a terminal—it rebuilds the terminal with AI as a first-class citizen.
The terminal's PTY ownership, GPU rendering, and block-per-command model are all AI-informed
design decisions.

**Why it's novel**: Other agents treat the terminal as a tool to be called. Warp treats it
as the agent's native environment. This inversion changes what's possible: the agent has
access to terminal state, rendering context, and interaction patterns that tool-calling
agents can't access.

### Sandbox-as-Pattern (Codex CLI): Safety Through Containment

Codex CLI's 3-layer OS sandbox is not just a safety feature—it's an architectural pattern
that enables the agent to be maximally autonomous. By containing all side effects at the OS
level, the sandbox removes the need for application-level safety patterns (permission prompts,
operation classification, tool inspection).

**Why it's novel**: Anthropic's framework treats safety as a consideration within patterns,
not as a pattern itself. Codex CLI demonstrates that the safety mechanism can be the primary
architectural decision that shapes all other pattern choices.

### MCP-Native Composition (Goose): Protocol as Pattern

Goose's MCP-native design means tools are not hard-coded but discovered and composed through
the MCP protocol. This enables dynamic capability expansion without agent modification.

**Why it's novel**: Traditional augmented LLM defines tools at build time. MCP-native design
defines tools at runtime through protocol negotiation. This is a meta-pattern that affects
how the augmented LLM pattern itself is implemented.

### Context Management as Pattern

Multiple agents implement sophisticated context management:
- **Claude Code**: Context compaction to extend session length
- **Droid**: Anchor-point compaction to preserve critical context
- **OpenHands**: Condenser system for event history compression
- **Gemini CLI**: 1M token context to avoid compaction entirely

**Why it's novel**: Anthropic's framework doesn't explicitly address context window management
as a pattern. In practice, context management is one of the most impactful architectural
decisions, affecting how long an agent can run, how much it can "remember," and how reliably
it maintains coherence across long sessions.

---

## Key Takeaways

1. **Every agent is an augmented LLM**: The augmented LLM pattern is the universal
   foundation. The differences between agents come from what they layer on top.

2. **Autonomous agent is the default mode**: 13 of 17 agents use the autonomous agent
   pattern as primary. The agentic loop (observe → think → act → observe) is the
   dominant architecture for CLI coding agents.

3. **Top performers compose many patterns**: ForgeCode (7 patterns), Claude Code (7
   patterns), and Droid (7 patterns) all layer multiple patterns. But composition must
   be deliberate—more patterns without good orchestration just adds overhead.

4. **Simplicity can win**: mini-SWE-agent (2 patterns) and Codex CLI (4 patterns) prove
   that fewer, well-implemented patterns can be highly effective. The choice between
   simplicity and complexity depends on the target use case.

5. **Safety mechanisms are architectural decisions**: Sandbox-first (Codex CLI),
   inspection pipelines (Goose), and graduated permissions (Claude Code) are not bolted-on
   safety features—they fundamentally shape the agent's pattern choices.

6. **Novel patterns are emerging**: CodeAct, edit format innovation, terminal-as-agent,
   sandbox-as-pattern, MCP-native composition, and context management represent patterns
   that extend beyond Anthropic's framework. The field is actively evolving.

7. **The comparison table is a snapshot**: Agent architectures evolve rapidly. The patterns
   analyzed here reflect current implementations, but several agents (Warp with Oz,
   OpenCode with its pub/sub system, Goose with MCP expansion) have architectural
   foundations that enable future pattern additions.

8. **Pattern selection should be evidence-based**: Rather than choosing patterns based on
   theoretical appeal, study how successful agents implement them. The 17-agent comparison
   provides empirical evidence for which patterns work, which combine well, and which are
   essential versus optional.
