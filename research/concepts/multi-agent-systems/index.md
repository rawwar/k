# Multi-Agent Systems for Coding Agents

## Overview

Multi-agent systems (MAS) represent one of the most significant architectural patterns
emerging in AI-powered software engineering. Rather than relying on a single monolithic
agent to handle every aspect of code generation, analysis, and modification, multi-agent
architectures decompose complex coding tasks across multiple specialized agents — each
with distinct capabilities, context windows, and permission boundaries.

This research folder provides a comprehensive analysis of multi-agent patterns as they
apply specifically to **coding agents** — AI systems that read, write, debug, and reason
about source code. We draw from real-world implementations (ForgeCode, SageAgent,
Claude Code, OpenHands, Codex CLI, Capy, TongAgents, Junie CLI) as well as foundational
frameworks (OpenAI Swarm/Agents SDK, Google A2A Protocol, Anthropic's agentic patterns).

---

## Why Multi-Agent for Coding?

Software engineering is inherently a multi-role discipline. Even a single developer
switches between roles throughout the day:

- **Researcher**: Understanding existing code, tracing dependencies, reading docs
- **Architect**: Planning changes, considering system-wide implications
- **Implementer**: Writing code, making modifications
- **Reviewer**: Checking work for correctness, style, edge cases
- **Tester**: Verifying that changes work as intended

Multi-agent systems formalize this natural division. Instead of asking one LLM to
simultaneously plan, implement, and verify — which strains context windows, creates
conflicting objectives, and makes it hard to enforce quality gates — dedicated agents
handle each phase with purpose-built prompts, tools, and constraints.

### The Core Tension

Multi-agent systems introduce complexity. Every agent boundary creates:

1. **Communication overhead** — Agents must share context, and information is lost at boundaries
2. **Coordination costs** — Someone (or something) must decide which agent runs when
3. **Latency** — Multiple LLM calls take more time than one
4. **Cost** — More API calls means more tokens consumed

The question is never "should we use multi-agent?" but rather "does the task complexity
justify the coordination overhead?" For simple file edits, a single agent suffices. For
multi-file refactors requiring planning, implementation, and verification — multi-agent
architectures consistently outperform monolithic approaches.

---

## Key Patterns

This folder explores the following multi-agent patterns in depth:

### Architecture Patterns

| File | Pattern | Key Insight |
|------|---------|-------------|
| [orchestrator-worker.md](./orchestrator-worker.md) | Orchestrator-Worker | Central brain delegates; Anthropic's most-recommended pattern for coding |
| [peer-to-peer.md](./peer-to-peer.md) | Peer-to-Peer | Agents as equals; debate, review, and consensus patterns |
| [specialist-agents.md](./specialist-agents.md) | Specialist Agents | Purpose-built roles with hard boundaries; ForgeCode's 3-agent model |
| [swarm-patterns.md](./swarm-patterns.md) | Swarm Patterns | Lightweight handoffs; OpenAI's approach to agent coordination |

### Mechanism Patterns

| File | Pattern | Key Insight |
|------|---------|-------------|
| [communication-protocols.md](./communication-protocols.md) | Communication Protocols | How agents talk: shared state, message passing, A2A, MCP |
| [context-sharing.md](./context-sharing.md) | Context Sharing | The hardest problem: passing knowledge without blowing context windows |
| [evaluation-agent.md](./evaluation-agent.md) | Evaluator-Optimizer | Generator + evaluator loops; enforced verification patterns |

### Analysis

| File | Pattern | Key Insight |
|------|---------|-------------|
| [real-world-examples.md](./real-world-examples.md) | Real-World Examples | Deep dives into ForgeCode, SageAgent, Claude Code, OpenHands, and more |
| [agent-comparison.md](./agent-comparison.md) | Multi vs Single Agent | When to use which; trade-off analysis |

---

## The Multi-Agent Spectrum

Not all multi-agent systems are created equal. Implementations fall on a spectrum:

```
Single Agent ──────────────────────────────────────────── Full Multi-Agent
     │                    │                    │                    │
  Monolithic         Sub-Agent            Pipeline           Specialized
   Loop             Delegation           Orchestration        Ensemble
     │                    │                    │                    │
  mini-SWE          Claude Code            SageAgent          ForgeCode
  Aider basic       OpenHands              Junie CLI          Capy
  OpenCode          Codex CLI              (5 agents)         (Captain/Build)
                    Goose                                     TongAgents
```

### Level 0: Single Agent
One LLM, one loop, one context window. Tools extend capability but there's no
delegation. Examples: mini-SWE-agent, basic Aider, OpenCode.

### Level 1: Sub-Agent Delegation
A primary agent spawns child agents for specific tasks. The children run in isolated
context windows and return summaries. The parent remains in control. This is primarily
a **context management** strategy. Examples: Claude Code (explore/plan/general sub-agents),
OpenHands (AgentDelegateAction), Codex CLI (explorer/worker roles), Goose (summon).

### Level 2: Pipeline Orchestration
Multiple agents execute in a defined sequence, each transforming the output of the
previous. May include feedback loops. Examples: SageAgent (5-agent pipeline with
ObservationAgent feedback), Junie CLI (Planner→Executor→Verifier).

### Level 3: Specialized Ensemble
Distinct agents with hard role boundaries and non-overlapping capabilities, coordinated
by an orchestration layer. Each agent has purpose-built prompts, tools, and permissions.
Examples: ForgeCode (Forge/Muse/Sage with read-write vs read-only boundaries),
Capy (Captain/Build with no-code vs no-questions constraints).

---

## Key Findings from Research

### 1. Context Isolation is the Primary Motivation
Across all implementations we studied, the #1 reason for adopting multi-agent architecture
is **context window management**, not task specialization. Claude Code's documentation
makes this explicit: sub-agents exist to keep the main context clean. ForgeCode's bounded
context model ensures only summaries — not raw exploration data — flow between agents.

### 2. Enforcement Beats Prompting
ForgeCode's programmatic verification enforcement (the agent literally cannot mark a task
as complete without running verification) produced their "biggest single improvement."
Compare this to Claude Code's prompt-based approach where the agent *decides* whether
to run tests. Dedicated evaluator agents (SageAgent's ObservationAgent) provide a middle
ground between full programmatic enforcement and pure prompting.

### 3. Hard Boundaries Create Better Outputs
The most effective multi-agent systems impose **hard constraints** on agent capabilities:
- ForgeCode: Muse (planner) literally cannot write code; Sage (researcher) is never user-facing
- Capy: Captain cannot write code; Build cannot ask clarifying questions
- Claude Code: Explore sub-agents get a cheaper, faster model (Haiku) and read-only tools

These constraints aren't limitations — they're features. They prevent role confusion
and force each agent to excel within its domain.

### 4. No Production System Uses Peer-to-Peer
Every multi-agent coding system we studied uses hierarchical patterns (parent→child,
pipeline, orchestrator→worker). No production system uses true peer-to-peer negotiation,
voting, or consensus. This may change as the field matures, but currently the overhead
of peer coordination exceeds its benefits for structured coding tasks.

### 5. Communication Patterns Cluster into Three Types
- **Summary-based handoff**: Agent A explores, summarizes, passes summary to Agent B (ForgeCode, Claude Code)
- **Event stream/pub-sub**: Centralized event bus connects all agents (OpenHands)
- **Spec/document-based**: Written specification is the interface between agents (Capy)

---

## Industry Frameworks

Several frameworks provide building blocks for multi-agent systems:

| Framework | Creator | Key Concept | Production Ready |
|-----------|---------|-------------|-----------------|
| **Agents SDK** | OpenAI | Handoffs, guardrails, tracing | Yes |
| **Swarm** | OpenAI | Lightweight agent primitives (educational) | No (deprecated) |
| **A2A Protocol** | Google (Linux Foundation) | Agent-to-agent communication standard | Yes (v1.0) |
| **Claude Agent SDK** | Anthropic | Orchestrator-workers, evaluator-optimizer | Yes |
| **MCP** | Anthropic | Tool/context protocol (not agent-to-agent) | Yes |
| **LangGraph** | LangChain | Graph-based agent orchestration | Yes |
| **CrewAI** | CrewAI | Role-based multi-agent framework | Yes |
| **AutoGen** | Microsoft | Multi-agent conversation framework | Yes |

---

## How to Read This Folder

**Start here** if you're new to multi-agent systems:
1. This README for overview
2. [orchestrator-worker.md](./orchestrator-worker.md) for the most common pattern
3. [specialist-agents.md](./specialist-agents.md) for role design
4. [real-world-examples.md](./real-world-examples.md) for concrete implementations

**Deep dive** into specific mechanisms:
- [communication-protocols.md](./communication-protocols.md) for how agents talk
- [context-sharing.md](./context-sharing.md) for the hardest problem in multi-agent
- [evaluation-agent.md](./evaluation-agent.md) for quality assurance patterns

**Explore alternative patterns**:
- [peer-to-peer.md](./peer-to-peer.md) for non-hierarchical approaches
- [swarm-patterns.md](./swarm-patterns.md) for lightweight coordination
- [agent-comparison.md](./agent-comparison.md) for the multi vs single agent decision

---

## Terminology

| Term | Definition |
|------|-----------|
| **Agent** | An LLM with instructions, tools, and an execution loop |
| **Sub-agent** | A child agent spawned by a parent agent for a specific task |
| **Orchestrator** | A central agent that decomposes tasks and delegates to workers |
| **Worker** | An agent that executes a specific subtask assigned by an orchestrator |
| **Handoff** | The transfer of control from one agent to another |
| **Context window** | The maximum token capacity of an LLM's input |
| **Bounded context** | A pattern where only summaries (not raw data) cross agent boundaries |
| **Agent card** | A metadata document describing an agent's capabilities (A2A term) |
| **Specialist agent** | An agent designed for a single, well-defined role |
| **Agentic loop** | The core cycle: observe → think → act → observe |
| **Event stream** | A pub/sub message bus connecting agents (OpenHands pattern) |
| **Verification skill** | A programmatic check that runs before task completion |

---

## References

- Anthropic. "Building Effective Agents." 2024. https://www.anthropic.com/research/building-effective-agents
- OpenAI. "Swarm (experimental)." 2024. https://github.com/openai/swarm
- OpenAI. "Agents SDK." 2025. https://github.com/openai/openai-agents-python
- Google. "Agent2Agent Protocol." 2025. https://github.com/a2aproject/A2A
- Anthropic. "Model Context Protocol." 2024. https://modelcontextprotocol.io
- Research files in `/research/agents/*/` — ForgeCode, SageAgent, Claude Code, OpenHands, TongAgents, Capy, Codex, Goose, Junie CLI
