---
title: "Ante Agentic Loop"
status: complete
---

# Ante Agentic Loop

> Ante, by Antigma Labs, is a Rust-built terminal coding agent featuring a self-organizing
> multi-agent architecture. Its agentic loop diverges significantly from the single-agent
> paradigm common in most coding assistants. Instead of one agent iterating in a plan-act-observe
> cycle, Ante introduces a **meta-agent orchestrator** that decomposes tasks and delegates to
> concurrent sub-agents — each running its own inner loop. This document reconstructs the
> agentic loop from publicly available descriptions; Ante is closed-source, so specifics are
> inferred rather than confirmed from code.

## Overview

Most coding agents follow a straightforward loop: receive a prompt, call an LLM, execute
tools, observe results, repeat. Ante layers a **two-tier loop** on top of this pattern:

1. An **outer loop** driven by the meta-agent, responsible for task decomposition, delegation,
   and result synthesis.
2. An **inner loop** within each sub-agent, following a plan-act-observe-decide cycle for
   its assigned sub-task.

The combination produces a fan-out / fan-in execution model where multiple sub-agents work
concurrently, coordinated loosely by the meta-agent rather than managed step-by-step.

## Meta-Agent Orchestration Loop

The top-level loop is the meta-agent's responsibility. When a user submits a request, the
meta-agent does not directly start editing files or running commands. Instead, it operates
as an orchestrator — what Antigma Labs describes as an "organization of agents to scale."

The meta-agent loop proceeds roughly as follows:

1. **Receive** the user request (natural language prompt, potentially with file context).
2. **Decompose** the request into discrete sub-tasks. For a feature request, this might mean
   separating the data-layer changes from the API changes from the test updates.
3. **Delegate** each sub-task to a sub-agent, providing it with the necessary context
   (relevant files, constraints, dependencies on other sub-tasks).
4. **Monitor** sub-agent progress. The meta-agent may receive intermediate status or wait
   for completion signals.
5. **Collect** results from all sub-agents (fan-in).
6. **Synthesize** the combined output — resolving conflicts, verifying coherence across
   sub-task results, and assembling a unified response.
7. **Present** the final result to the user.

```
User Request
     │
     ▼
┌──────────────┐
│  Meta-Agent  │
│  Decompose   │──── Break into sub-tasks
│  & Delegate  │
└──────┬───────┘
       │
  ┌────▼────┐
  │ Fan-Out │──── Dispatch to sub-agents (concurrent)
  └────┬────┘
       │
  ┌────▼────────┐
  │ Sub-Agents   │──── Each runs inner loop
  │ Execute      │
  └────┬────────┘
       │
  ┌────▼────┐
  │ Fan-In  │──── Collect results
  └────┬────┘
       │
  ┌────▼──────────┐
  │  Meta-Agent    │
  │  Synthesize    │──── Combine, verify, present
  └────┬──────────┘
       │
       ▼
  User Response
```

The meta-agent itself likely relies on an LLM call to perform decomposition and synthesis —
meaning the outer loop contains its own LLM interaction steps. The decomposition quality
directly determines how well the system parallelizes work and whether sub-agents receive
coherent, independent sub-tasks.

## Sub-Agent Execution Loop

Each sub-agent, once it receives a delegated sub-task, runs a self-contained agentic loop
that mirrors the classic plan-act-observe pattern:

1. **Plan**: The sub-agent's LLM reasons about the approach — which files to read, what
   edits to make, what commands to run.
2. **Act**: Tool calls are executed — file reads, file writes, shell commands, code search,
   and other operations.
3. **Observe**: Tool outputs are collected and fed back into the conversation context. The
   sub-agent evaluates whether the action succeeded or produced errors.
4. **Decide**: The sub-agent determines whether the sub-task is complete. If not, it loops
   back to the Plan step with updated context. If done, it reports its results back to the
   meta-agent.

```
Receive Task from Meta-Agent
     │
     ▼
┌──────────┐
│  Plan    │──── LLM reasons about approach
└────┬─────┘
     │
     ▼
┌──────────┐
│  Act     │──── Execute tool calls (file I/O, shell, etc.)
└────┬─────┘
     │
     ▼
┌──────────┐
│  Observe │──── Collect tool output, check results
└────┬─────┘
     │
     ▼
┌──────────┐     ┌────────────────┐
│  Decide  │────►│ Report to      │
│  (done?) │ yes │ Meta-Agent     │
└────┬─────┘     └────────────────┘
     │ no
     ▼
  (loop back to Plan)
```

Sub-agents are expected to be narrowly scoped. A sub-agent assigned to "update the database
migration" should not also start modifying API handlers — that would be a different sub-agent's
concern. This scoping is what enables concurrency: independent sub-tasks on independent file
regions can proceed in parallel without conflict.

## Self-Organizing Behavior

A distinguishing design principle in Ante is **self-organization**. Traditional multi-agent
systems often use rigid orchestration: a central controller issues precise instructions,
monitors every step, and handles all coordination. Ante's philosophy, as described publicly,
leans toward emergent coordination — agents behave more like teammates than subordinates.

Key characteristics of this self-organizing approach:

- **The user is "another agent"**: Rather than the user sitting outside the system issuing
  commands, the user is modeled as a participant in the agent network. This blurs the
  boundary between human input and agent-generated context.
- **Loose coupling**: Sub-agents coordinate through shared context and results rather than
  through direct message-passing or rigid protocols. The meta-agent provides the initial
  decomposition, but sub-agents may have latitude in how they accomplish their tasks.
- **Emergent task refinement**: If a sub-agent discovers during execution that its sub-task
  is different than expected (e.g., a file doesn't exist, an API has changed), it can adapt
  its plan autonomously rather than requiring the meta-agent to re-plan.
- **Collaborative rather than hierarchical**: The mental model is less "boss and workers"
  and more "team of specialists." The meta-agent is a coordinator, not a micromanager.

This philosophy has practical implications for the loop. Error handling, for example, may
be distributed: a sub-agent that encounters a build error can attempt to fix it within its
own loop before escalating to the meta-agent. This reduces round-trips through the outer
loop and keeps the system responsive.

> **Note**: The degree to which self-organization is implemented versus aspirational is
> unclear from public descriptions alone. The architectural intent is clear, but the
> specific mechanisms (agent-to-agent communication, conflict resolution, etc.) are not
> publicly documented.

## Lock-Free Scheduling

Ante is built in Rust, and its runtime leverages a **lock-free scheduler** for concurrent
sub-agent execution. This is a critical enabler for the multi-agent loop.

In a lock-based concurrency model, agents contending for shared resources (file system
access, context memory, LLM API connections) would block each other, serializing what should
be parallel work. Ante's lock-free approach means:

- **No mutex contention**: Sub-agents do not acquire locks to read or update shared state.
  Instead, atomic operations or lock-free data structures (common in Rust's ecosystem via
  crates like `crossbeam`) enable safe concurrent access.
- **Independent progress**: Each sub-agent can make progress regardless of what other agents
  are doing. One agent waiting on an LLM API response does not block another agent from
  executing a file write.
- **Predictable latency**: Lock-free designs avoid priority inversion and convoy effects,
  which helps keep the overall loop latency predictable even as the number of concurrent
  sub-agents increases.
- **Rust's ownership model**: Rust's compile-time guarantees around data races complement
  the lock-free design. The borrow checker ensures that shared mutable state is handled
  correctly without relying on runtime locking.

From a loop perspective, the lock-free scheduler means the fan-out step in the meta-agent's
outer loop is genuinely concurrent — sub-agents are dispatched and run in parallel, with the
fan-in step collecting results as they arrive rather than polling or waiting in sequence.

## LLM Interaction Pattern

Ante supports multiple LLM backends, spanning both cloud and local providers:

- **Cloud**: Anthropic Claude, Google Gemini (and potentially others via API-compatible
  endpoints).
- **Local**: Possibly through `nanochat-rs` or similar local inference libraries, enabling
  fully offline operation.

The LLM interaction pattern within the loop must account for these differences:

### Cloud API Calls

For cloud-hosted models, each LLM interaction in the loop involves:

1. Constructing the prompt (system message, conversation history, tool definitions).
2. Sending an HTTP request to the provider's API.
3. Waiting for the response (potentially streaming).
4. Parsing the response for content and/or tool-use requests.

Cloud calls introduce network latency, which is why concurrency matters — while one sub-agent
waits for a Claude response, others can continue executing tool calls or processing their
own LLM responses.

### Local Inference

In offline or local mode, the LLM call is replaced by local model inference:

1. The same prompt construction occurs.
2. Inference runs on the local machine (CPU or GPU).
3. Latency depends on model size and hardware rather than network conditions.

Local inference puts the model in the critical path of the loop — a slow local model
directly increases each iteration's cycle time. The lock-free scheduler still helps here,
as multiple sub-agents can interleave their compute phases.

### Unified Interface

Regardless of backend, the loop logic likely uses a unified interface for LLM calls —
the sub-agent issues a "complete this conversation" request, and the backend abstraction
handles whether that goes to a cloud API or a local model. This keeps the loop structure
identical across deployment modes.

## Tool Execution Cycle

Within each sub-agent's inner loop, the Act phase involves executing tool calls. Ante, as
a terminal-native coding agent, likely supports a standard set of tools:

- **File reads**: Read file contents, list directories, search codebases.
- **File writes**: Create, edit, or delete files. Edits may be patch-based (surgical
  replacements) or full-file rewrites.
- **Shell commands**: Run build commands, tests, linters, git operations, and other CLI tools.
- **Code search**: Grep-like or AST-aware search across the codebase.

The tool execution cycle within a single inner-loop iteration:

1. The LLM response includes one or more tool-use requests (structured as tool name +
   parameters).
2. The agent runtime validates and executes each tool call.
3. Tool outputs (stdout, stderr, file contents, error messages) are captured.
4. Outputs are appended to the conversation context.
5. The updated context is sent back to the LLM for the next iteration.

When multiple tool calls are returned in a single LLM response, Ante may execute them
concurrently (leveraging the same lock-free scheduler) or sequentially depending on
dependencies between the calls.

Tool execution errors are a key part of the loop's self-correction mechanism. A failed
`cargo build` produces compiler errors that the sub-agent feeds back to the LLM, which
then plans a fix — this is the standard observe → plan → act recovery cycle.

## Offline Loop Variant

Ante advertises offline capability, meaning the entire agentic loop can run without
network access. In this mode:

- **LLM inference** is performed locally (no cloud API calls).
- **All tool execution** remains local (file I/O, shell commands are inherently local).
- **The meta-agent loop** still operates the same way — decompose, delegate, collect,
  synthesize — but every LLM call in both the outer and inner loops hits the local model.

The offline variant has distinct performance characteristics:

- **Latency profile shifts**: Network latency disappears, but local inference latency
  dominates. For small models on capable hardware, this may be faster than cloud calls.
  For larger models on limited hardware, each loop iteration slows significantly.
- **No rate limiting**: Cloud APIs impose rate limits and token quotas. Local inference
  has no such constraints — the loop can iterate as fast as hardware allows.
- **Privacy**: All code and prompts stay on the local machine, which is a key motivation
  for the offline mode.

The lock-free scheduler is equally important in offline mode. Even with local inference,
multiple sub-agents benefit from concurrent execution — while one agent's inference runs,
another can execute tool calls, and a third can be assembling its next prompt.

## Comparison to Single-Agent Loops

To contextualize Ante's approach, it helps to contrast with the single-agent loop used by
most coding agents (Claude Code, Aider, Cursor Agent, etc.):

| Aspect | Single-Agent Loop | Ante Multi-Agent Loop |
|---|---|---|
| **Structure** | One loop: prompt → LLM → tools → observe → repeat | Two-tier: outer meta-agent loop + inner sub-agent loops |
| **Concurrency** | Sequential (one LLM call at a time) | Concurrent sub-agents via lock-free scheduler |
| **Task handling** | Single agent handles entire request | Meta-agent decomposes; sub-agents handle parts |
| **Error recovery** | Same agent retries | Sub-agent retries locally; escalates if needed |
| **Context window** | One conversation grows over time | Each sub-agent has a focused, smaller context |
| **Scaling** | Bounded by single context window | Can scale to multiple parallel contexts |

The multi-agent approach trades simplicity for potential throughput gains. A complex refactor
that touches many files might complete faster with parallel sub-agents, but the decomposition
overhead and synthesis step add complexity that a single-agent loop avoids.

## Open Questions

Since Ante is closed-source, several aspects of the loop remain unclear:

- **Inter-agent communication**: Can sub-agents communicate with each other during execution,
  or only through the meta-agent? Direct agent-to-agent messaging would enable richer
  coordination but adds complexity.
- **Conflict resolution**: When two sub-agents modify the same file, how are conflicts
  detected and resolved? The meta-agent's synthesis step likely handles this, but the
  specific mechanism is unknown.
- **Dynamic re-planning**: Can the meta-agent revise its decomposition mid-execution if a
  sub-agent reports unexpected findings? This would require a feedback loop within the
  outer loop itself.
- **Context sharing**: How much context do sub-agents share? Full codebase context for each
  would be expensive; selective context based on the sub-task would be more efficient but
  risks missing relevant information.
- **Agent memory**: Whether sub-agents or the meta-agent maintain memory across requests
  (session persistence) is not documented publicly.

## Summary

Ante's agentic loop is a two-tier, multi-agent system:

- The **meta-agent outer loop** handles decomposition, delegation, collection, and synthesis.
- **Sub-agent inner loops** each run a plan-act-observe-decide cycle for their assigned
  sub-tasks.
- **Lock-free scheduling** in Rust enables genuine concurrency across sub-agents.
- **Self-organizing behavior** favors emergent coordination over rigid top-down control.
- The loop operates identically across **cloud and offline modes**, with only the LLM
  backend differing.
- The architecture trades single-agent simplicity for parallel throughput and modular
  task handling.