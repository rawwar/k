# Agent Design Patterns

> A comprehensive taxonomy of agentic system architectures, based on Anthropic's
> ["Building Effective Agents"](https://www.anthropic.com/research/building-effective-agents)
> framework and observed across 17 CLI coding agents.

---

## Overview

The landscape of AI-powered coding agents is vast, but the underlying
architectural patterns are surprisingly finite. Anthropic's "Building Effective
Agents" blog post provides the clearest taxonomy to date, organizing agentic
system designs along a **spectrum of complexity** — from simple augmented LLMs
to fully autonomous agents.

This document serves as the central reference for understanding these patterns,
how they relate to one another, and how the 17 CLI coding agents in our study
implement them in practice.

### The Core Insight

Anthropic draws a critical distinction between two categories:

| Term         | Definition                                                        |
|--------------|-------------------------------------------------------------------|
| **Workflow** | Systems where LLMs are orchestrated through **predefined code paths** — the developer controls the flow. |
| **Agent**    | Systems where LLMs **dynamically direct their own processes** and tool usage — the model controls the flow. |

This is not a binary classification. Real systems exist on a continuum, and most
production CLI coding agents combine multiple patterns. The key insight is that
**workflows are deterministic orchestrations** while **agents are autonomous
loops** — and the best systems know when to use each.

### Why This Matters for CLI Coding Agents

CLI coding agents face a unique set of constraints:

- **Interactive terminal environment** — streaming output, user interrupts
- **File system as primary workspace** — reads, writes, diffs, patches
- **Tool-heavy workloads** — shell commands, LSP, search, git operations
- **Long-running tasks** — multi-file refactors, debugging sessions
- **Cost sensitivity** — token usage directly impacts user experience

These constraints push agent designers toward specific pattern choices. A chat
assistant might get away with a simple augmented LLM, but a coding agent that
needs to autonomously fix a bug across 12 files requires orchestration.

---

## The Pattern Spectrum

Anthropic presents these patterns as a progression from simple to complex. Each
pattern builds on the ones before it:

```
                        The Agentic System Spectrum

  Simple                                                          Complex
    |                                                                |
    v                                                                v

 +----------+   +--------+   +-------+   +---------------+   +---------+
 |Augmented |-->|Prompt  |-->|Routing|-->|Parallelization|-->|Orchestr.|
 |   LLM    |   |Chaining|   |       |   |               |   |-Workers |
 +----------+   +--------+   +-------+   +---------------+   +---------+
                                                                   |
                                                                   v
                                               +-----------+  +--------+
                                               |Evaluator- |  |  Full  |
                                               | Optimizer  |  | Agents |
                                               +-----------+  +--------+

  <--- Building Block ---><---- Workflows (predefined paths) --><-Agents->
```

**Key:** Each step to the right adds autonomy and capability — but also adds
latency, cost, and failure modes. Anthropic's core advice: **start at the left
and move right only when the task demands it.**

### Zoomed-In View

```
 +---------------------------------------------------------------------+
 |                     AUGMENTED LLM (Building Block)                   |
 |                                                                      |
 |  Every pattern below uses this as its atomic unit:                   |
 |  LLM + Retrieval + Tools + Memory                                   |
 +---------------------------+-----------------------------------------+
                             |
                +------------+------------+
                v            v            v
        +----------+  +----------+  +--------------+
        |  Prompt  |  | Routing  |  |Parallelization|
        | Chaining |  |          |  |              |
        | (serial) |  |(dispatch)|  | (concurrent) |
        +----+-----+  +----------+  +--------------+
             |
             v
     +---------------+     +----------------+
     | Orchestrator- |     |   Evaluator-   |
     |   Workers     |     |   Optimizer    |
     |  (delegate)   |     |   (iterate)    |
     +-------+-------+     +--------+-------+
             |                       |
             +-----------+-----------+
                         v
                +--------------+
                |  Autonomous  |
                |    Agents    |
                |   (loop)     |
                +--------------+
```

---

## Pattern Catalog

Each pattern has a dedicated deep-dive document. Below is a summary of each
pattern, its classification, and when to use it.

### 1. Augmented LLM (Building Block)

**Deep dive:** [augmented-llm.md](augmented-llm.md)

The foundational unit of all agentic systems. An LLM enhanced with:
- **Retrieval** — pulling relevant context (RAG, codebase search)
- **Tools** — executing actions (file I/O, shell, LSP)
- **Memory** — persisting state across interactions (CLAUDE.md, etc.)

This is not a pattern *per se* — it is the **building block** from which all
other patterns are composed. Every node in an orchestrator-workers system,
every step in a prompt chain, is an augmented LLM.

**When sufficient:** Single-turn code generation, well-scoped questions,
simple file edits where one inference call is enough.

---

### 2. Prompt Chaining (Workflow)

**Deep dive:** [prompt-chaining.md](prompt-chaining.md)

A sequence of LLM calls where each step's output becomes the next step's input.
The flow is **predefined by the developer**, not decided by the model.

```
  Input --> [LLM Step 1] --> [Gate] --> [LLM Step 2] --> [Gate] --> Output
```

**Examples in coding agents:**
- Generate code -> validate syntax -> apply diff
- Analyze error -> propose fix -> verify fix compiles
- Plan changes -> implement each change -> run tests

**When to use:** Tasks that are naturally decomposable into fixed, sequential
subtasks with clear handoff points.

---

### 3. Routing (Workflow)

**Deep dive:** [routing.md](routing.md)

A classification step that directs the input to a specialized handler. The
router examines the request and dispatches it to the appropriate downstream
process.

```
                    +---> [Handler A: Code Edit]
  Input --> [Router]+---> [Handler B: Explanation]
                    +---> [Handler C: Debug Session]
```

**Examples in coding agents:**
- Classifying user intent (edit vs. explain vs. search)
- Selecting the right tool for the job
- Choosing model tier based on task complexity

**When to use:** When you have distinct categories of inputs that benefit from
specialized handling.

---

### 4. Parallelization (Workflow)

**Deep dive:** [parallelization.md](parallelization.md)

Running multiple LLM calls **concurrently** and aggregating results. Two
sub-patterns:
- **Sectioning:** Splitting a task into independent subtasks
- **Voting:** Running the same task multiple times for consensus

```
              +---> [LLM Call A] ---+
  Input --->  +---> [LLM Call B] ---+--> Aggregator --> Output
              +---> [LLM Call C] ---+
```

**Examples in coding agents:**
- Searching multiple files simultaneously
- Running parallel tool calls (Claude Code, Codex CLI)
- Multi-model voting for code review

**When to use:** When subtasks are independent and latency matters, or when
you need redundancy for reliability.

---

### 5. Orchestrator-Workers (Workflow)

**Deep dive:** [orchestrator-workers.md](orchestrator-workers.md)

A central orchestrator LLM that dynamically breaks down tasks and delegates
to worker LLMs. Unlike prompt chaining, the decomposition is **not predefined**
— the orchestrator decides what subtasks to create at runtime.

```
              +--------------+
              | Orchestrator |
              |    (LLM)     |
              +--+---+---+---+
                 |   |   |
         +-------+   |   +-------+
         v           v           v
   +----------+ +----------+ +----------+
   | Worker 1 | | Worker 2 | | Worker N |
   |  (LLM)   | |  (LLM)   | |  (LLM)   |
   +----------+ +----------+ +----------+
```

**Examples in coding agents:**
- Claude Code's sub-agent spawning for parallel tasks
- OpenHands's multi-agent architecture
- Junie CLI's plan -> implement -> verify pipeline

**When to use:** Complex tasks where the subtask breakdown is not known in
advance and requires LLM-level reasoning to decompose.

---

### 6. Evaluator-Optimizer (Workflow)

**Deep dive:** [evaluator-optimizer.md](evaluator-optimizer.md)

A two-LLM loop where one generates output and another evaluates it, providing
feedback for iterative refinement.

```
  Input --> [Generator LLM] --> [Evaluator LLM] ---+
                 ^                                   |
                 +-------- feedback <----------------+
                          (iterate until satisfied)
```

**Examples in coding agents:**
- Generate code -> run tests -> fix failures (test-driven loop)
- Aider's lint-fix cycle
- Code review -> revision loops

**When to use:** When there is a clear evaluation criterion and iterative
refinement demonstrably improves output quality.

---

### 7. Autonomous Agents (Full Pattern)

**Deep dive:** [when-to-use-agents.md](when-to-use-agents.md)

The most complex pattern. The LLM operates in a **loop**, dynamically deciding
which tools to call, what to do next, and when to stop. The human sets the
goal; the agent decides the path.

```
  +------------------------------------------------+
  |                  Agent Loop                     |
  |                                                 |
  |  +----------+    +-----------+    +---------+  |
  |  | Observe  |--->|  Think    |--->|   Act   |--+--> [Environment]
  |  |(read env)|    |(reason)   |    |(tools)  |  |         |
  |  +----------+    +-----------+    +---------+  |         |
  |       ^                                         |         |
  |       +-----------------------------------------+---------+
  |                  (loop until done)              |
  +------------------------------------------------+
```

**Examples in coding agents:**
- Claude Code's agentic loop with tool use
- Codex CLI's autonomous task execution
- ForgeCode's ReAct-style agent loop
- OpenHands's sandboxed autonomous execution

**When to use:** Open-ended tasks where the solution path cannot be
predetermined, and the agent needs to react to intermediate results.

---

## Patterns in CLI Coding Agents

The following table maps each of the 17 agents to their **primary** architectural
pattern(s). Most agents combine multiple patterns, but the primary pattern
reflects the dominant control flow.

### Agent-to-Pattern Mapping

| Agent              | Tier | Primary Pattern        | Secondary Patterns                  | Notes                                      |
|--------------------|------|------------------------|-------------------------------------|--------------------------------------------|
| **ForgeCode**      | 1    | Autonomous Agent       | Augmented LLM, Parallelization      | ReAct loop with multi-tool dispatch        |
| **Claude Code**    | 1    | Autonomous Agent       | Orchestrator-Workers, Parallelization| Sub-agent spawning, parallel tool calls    |
| **Codex CLI**      | 1    | Autonomous Agent       | Evaluator-Optimizer                 | Sandboxed execution with auto-apply        |
| **Droid**          | 1    | Autonomous Agent       | Orchestrator-Workers                | Background task execution                  |
| **Ante**           | 1    | Autonomous Agent       | Routing, Parallelization            | Context-aware task routing                 |
| **OpenCode**       | 1    | Autonomous Agent       | Augmented LLM                       | Lean tool-use loop                         |
| **OpenHands**      | 1    | Orchestrator-Workers   | Autonomous Agent, Parallelization   | Multi-agent sandboxed architecture         |
| **Warp**           | 2    | Augmented LLM          | Routing                             | Terminal-integrated, command-focused        |
| **Gemini CLI**     | 2    | Autonomous Agent       | Augmented LLM                       | Google ecosystem integration               |
| **Goose**          | 2    | Autonomous Agent       | Routing, Parallelization            | Extension-based tool routing               |
| **Junie CLI**      | 2    | Prompt Chaining        | Evaluator-Optimizer                 | Plan -> implement -> verify pipeline       |
| **mini-SWE-agent** | 2    | Autonomous Agent       | Augmented LLM                       | Minimal ReAct loop                         |
| **Pi Coding Agent**| 2    | Autonomous Agent       | Augmented LLM                       | Lightweight agent loop                     |
| **Aider**          | 2    | Evaluator-Optimizer    | Augmented LLM, Prompt Chaining      | Edit -> lint -> fix cycle                  |
| **Sage Agent**     | 3    | Autonomous Agent       | Augmented LLM                       | Research-focused agent                     |
| **TongAgents**     | 3    | Orchestrator-Workers   | Parallelization                     | Multi-agent collaboration                  |
| **Capy**           | 3    | Autonomous Agent       | Augmented LLM                       | Minimal agent implementation               |

### Pattern Distribution

```
  Autonomous Agent  ====================================  12 agents (71%)
  Augmented LLM     ========                               1 primary (6%)
  Orchestrator-Wkr  ========                               2 primary (12%)
  Evaluator-Optim   ====                                   1 primary (6%)
  Prompt Chaining    ====                                   1 primary (6%)
  Routing            (secondary only)
  Parallelization    (secondary only)
```

The dominance of the Autonomous Agent pattern is unsurprising — CLI coding
agents are, by nature, open-ended problem solvers. However, the **secondary**
patterns reveal the real architectural diversity. No agent uses a single
pattern in isolation.

---

## The Simplicity Principle

**Deep dive:** [simplicity-principle.md](simplicity-principle.md)

Anthropic's most important piece of advice is deceptively simple:

> *"Start with the simplest solution possible and only increase complexity when
> needed. This might mean not building agentic systems at all."*

This principle manifests in several ways across CLI coding agents:

### 1. Not Everything Needs an Agent

Many coding tasks are single-turn: "explain this function," "generate a unit
test," "convert this to TypeScript." These are perfectly served by an augmented
LLM — no loop required.

Agents like **Warp** lean into this insight, providing powerful augmented LLM
capabilities without a full agent loop for every interaction.

### 2. Workflows Before Agents

When a task *does* require multiple steps, a predefined workflow (prompt chain)
is often more reliable than an autonomous agent. **Junie CLI** exemplifies
this with its plan -> implement -> verify pipeline — the steps are fixed, even
though LLMs execute each step.

### 3. Agent Loop as Last Resort

The full autonomous agent loop — observe, think, act, repeat — should be
reserved for genuinely open-ended tasks. Debugging a complex multi-file issue,
performing a large refactor, or migrating a codebase: these justify the cost
and latency of an agent loop.

### The Cost Equation

```
  Simplicity Tradeoff:

  Pattern             Latency    Cost    Reliability    Flexibility
  -------------------------------------------------------------------
  Augmented LLM       Low        Low     High           Low
  Prompt Chaining     Medium     Medium  High           Medium
  Routing             Low        Low     High           Medium
  Parallelization     Low*       Medium  Medium         Medium
  Orchestrator-Wkrs   High       High    Medium         High
  Evaluator-Optimizer  High       High    High**         Medium
  Autonomous Agent    Variable   High    Variable       Very High

  * Parallelization trades cost for latency
  ** High reliability when evaluation criteria are clear
```

---

## Decision Framework

When designing or evaluating a CLI coding agent, use this decision tree:

```
  Is the task single-turn (one inference call)?
  |
  +-- YES --> Use Augmented LLM
  |
  +-- NO --> Is the task decomposable into fixed steps?
              |
              +-- YES --> Use Prompt Chaining
              |           (add gates between steps for quality control)
              |
              +-- NO --> Does the task have distinct categories?
                          |
                          +-- YES --> Use Routing to dispatch
                          |
                          +-- NO --> Are subtasks independent?
                                      |
                                      +-- YES --> Use Parallelization
                                      |
                                      +-- NO --> Is the decomposition dynamic?
                                                  |
                                                  +-- YES --> Use Orchestrator-Workers
                                                  |
                                                  +-- NO --> Is there a clear eval criterion?
                                                              |
                                                              +-- YES --> Use Evaluator-Optimizer
                                                              |
                                                              +-- NO --> Use Autonomous Agent
```

### Practical Heuristics

1. **If you can write the control flow in a script, it's a workflow** — don't
   use an agent for something a bash script could orchestrate.

2. **If the number of steps is unknown, you probably need an agent** — but
   consider whether an orchestrator-workers pattern could bound the scope.

3. **If quality matters more than speed, add an evaluator** — the
   evaluator-optimizer pattern is the most reliable way to improve output.

4. **If latency matters, parallelize** — but only when subtasks are truly
   independent.

5. **If the task is truly open-ended, use an autonomous agent** — but invest
   heavily in guardrails, tool quality, and stopping conditions.

---

## How Agents Combine Patterns

Real-world CLI coding agents rarely use a single pattern. Instead, they compose
patterns at different levels of their architecture. Here are concrete examples:

### Claude Code: Layered Composition

```
  +-----------------------------------------+
  |         Autonomous Agent (outer loop)    |
  |                                          |
  |  +----------------------------------+   |
  |  | Routing: intent classification   |   |
  |  |  +-- code edit                   |   |
  |  |  +-- search/explain              |   |
  |  |  +-- multi-file task --------+   |   |
  |  +----------------------------------+   |
  |                                 |        |
  |                                 v        |
  |  +----------------------------------+   |
  |  | Orchestrator-Workers: sub-agents |   |
  |  |  +-- Worker 1 (file A)          |   |
  |  |  +-- Worker 2 (file B)          |   |
  |  |  +-- Worker 3 (file C)          |   |
  |  +----------------------------------+   |
  |                                          |
  |  Parallelization: concurrent tool calls  |
  |  Augmented LLM: every node above         |
  +-----------------------------------------+
```

### Aider: Evaluator-Driven Loop

```
  +---------------------------------------+
  |  Evaluator-Optimizer (outer loop)     |
  |                                        |
  |  +---------------------------------+  |
  |  | Prompt Chain:                   |  |
  |  |  1. Analyze request             |  |
  |  |  2. Generate edit (Augmented LLM)  |
  |  |  3. Apply diff                  |  |
  |  |  4. Lint check --> fix cycle    |  |
  |  |  5. Test run --> fix cycle      |  |
  |  +---------------------------------+  |
  |                                        |
  |  Augmented LLM: repo-map RAG          |
  +---------------------------------------+
```

### OpenHands: Multi-Agent Orchestration

```
  +-----------------------------------------+
  |  Orchestrator-Workers (outer structure) |
  |                                          |
  |  +----------------------------------+   |
  |  | CodeAct Agent (autonomous loop)  |   |
  |  |  +-- Plans and reasons           |   |
  |  |  +-- Executes code in sandbox    |   |
  |  |  +-- Observes results            |   |
  |  +----------------------------------+   |
  |                                          |
  |  +----------------------------------+   |
  |  | BrowsingAgent (specialized)      |   |
  |  +----------------------------------+   |
  |                                          |
  |  Sandboxed execution environment         |
  +-----------------------------------------+
```

---

## Cross-References

This directory contains deep dives on each pattern and related concepts:

| File                                                        | Topic                                |
|-------------------------------------------------------------|--------------------------------------|
| [augmented-llm.md](augmented-llm.md)                       | The Augmented LLM building block     |
| [prompt-chaining.md](prompt-chaining.md)                    | Prompt Chaining workflow pattern      |
| [routing.md](routing.md)                                    | Routing workflow pattern              |
| [parallelization.md](parallelization.md)                    | Parallelization workflow pattern      |
| [orchestrator-workers.md](orchestrator-workers.md)          | Orchestrator-Workers workflow pattern |
| [evaluator-optimizer.md](evaluator-optimizer.md)            | Evaluator-Optimizer workflow pattern  |
| [when-to-use-agents.md](when-to-use-agents.md)             | Autonomous Agents and when to use them|
| [simplicity-principle.md](simplicity-principle.md)          | The case for starting simple          |
| [agent-comparison.md](agent-comparison.md)                  | Comparative analysis of all 17 agents |

---

## Key Takeaways

1. **Patterns form a spectrum, not a hierarchy.** Moving from augmented LLM to
   autonomous agent is not "upgrading" — it's trading simplicity for flexibility.
   Each point on the spectrum is the right choice for some set of tasks.

2. **The augmented LLM is the universal building block.** Every pattern above it
   is composed of augmented LLMs. Investing in better retrieval, better tools,
   and better memory improves *every* pattern. See [augmented-llm.md](augmented-llm.md).

3. **Workflows are underrated.** The industry's focus on "agents" often
   overshadows the power of well-designed workflows. Prompt chaining and
   routing are cheaper, faster, and more reliable for structured tasks.

4. **Real agents combine patterns.** No production system uses a single pattern.
   Claude Code combines autonomous agents with orchestrator-workers and
   parallelization. Aider combines evaluator-optimizer with prompt chaining.
   The art is in the composition.

5. **Start simple, measure, then add complexity.** Anthropic's simplicity
   principle is not just philosophical advice — it's engineering wisdom. Every
   layer of orchestration adds latency, cost, and failure modes. Add it only
   when you can measure the improvement.

6. **Tool quality matters more than orchestration.** A well-designed augmented
   LLM with excellent tools will outperform a sophisticated agent with poor
   tools. The Agent-Computer Interface (ACI) is as important as the UI.
   See Anthropic's tool design principles in [augmented-llm.md](augmented-llm.md).

7. **The 17 agents converge on patterns.** Despite different languages,
   different LLM providers, and different design philosophies, the 17 agents
   in our study independently converge on the same small set of patterns.
   This suggests these patterns are fundamental to the problem space.

---

*This document is part of a research library studying CLI coding agents.
For the full agent catalog, see the [research directory](../../). For
methodology, see the top-level [README](../../../README.md).*
