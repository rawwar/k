# Agent Comparison

## Overview

This document synthesizes the loop patterns of 17+ coding agents into a unified comparison.
The analysis is based on source-level reading of each agent's core loop, not marketing materials.

Three guiding observations frame the entire comparison:

1. **There is no "best" loop** — the best loop depends on the model, the task, and the deployment context.
2. **Complexity is not always beneficial** — simple loops often outperform complex ones on benchmarks.
3. **Production requirements force complexity** — observability, undo, interrupt, and multi-user support all demand architectural sophistication that benchmarks don't measure.

The agents studied span from ~200-line research prototypes (mini-SWE-agent) to production systems with tens of thousands of lines of orchestration code (Codex CLI, Claude Code, OpenHands).

---

## The Complexity Spectrum

```
Simplest                                                          Most Complex
   │                                                                    │
   ▼                                                                    ▼
┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────────┐
│  Simple  │  │ Streaming│  │  Event-  │  │ Message- │  │  Multi-    │
│  Linear  │  │   Loop   │  │  Driven  │  │ Passing  │  │  Agent     │
│          │  │          │  │  State   │  │  (SQ/EQ) │  │ Orchestr.  │
│mini-SWE  │  │ OpenCode │  │  Machine │  │          │  │            │
│  Pi      │  │  Goose   │  │          │  │  Codex   │  │ ForgeCode  │
│          │  │Gemini CLI│  │ OpenHands│  │          │  │ Ante       │
│          │  │          │  │          │  │          │  │ Claude Code│
└──────────┘  └──────────┘  └──────────┘  └──────────┘  └────────────┘
```

### Why This Spectrum Exists

Each step to the right on the spectrum adds capabilities at the cost of complexity, debugging difficulty, and cognitive load for contributors.

- **Simple loops** (mini-SWE-agent, Pi): Fast to build, easy to debug, sufficient when the underlying model is strong. The entire loop fits in a single function. Ideal for research because the trajectory *is* the training data — no hidden state transformations.
- **Streaming loops** (OpenCode, Goose, Gemini CLI): Add real-time UX via chunked token delivery. The loop now manages partial state: a token buffer, tool-call detection mid-stream, and backpressure. In Go (OpenCode) this means goroutines and channels; in Rust (Goose) it means async streams with `tokio`; in TypeScript (Gemini CLI) it means async iterators.
- **Event-driven state machines** (OpenHands): The loop is replaced by an event bus. Actions and observations are first-class objects with types, metadata, and serialization. This enables replay, condensation, and delegation — but the control flow is now implicit in event handlers rather than explicit in a `while` loop.
- **Message-passing with queues** (Codex CLI): Submit and execution queues decouple the request from the execution. This enables undo (drop from SQ before execution), interrupt (drain EQ mid-turn), and compaction (rewrite SQ entries). The cost: debugging requires tracing messages across queues, and ordering guarantees become non-trivial.
- **Multi-agent orchestration** (ForgeCode, Ante, Claude Code): A meta-agent decomposes tasks and delegates to specialized sub-agents. Each sub-agent may itself run any of the simpler loop patterns. The orchestrator manages context windows across agents, handles fan-out/fan-in, and enforces verification. The cost: emergent behavior is hard to predict, and failures cascade across agent boundaries.

### The Fundamental Trade-off

```
                    Debuggability
                         ▲
                         │
            mini-SWE ●   │
                         │   ● Pi
                         │
              OpenCode ● │        ● Goose
                         │
                         │   ● Gemini CLI
                         │
            OpenHands ●  │
                         │
                Codex ●  │        ● Claude Code
                         │
                         └──────────────────────► Capabilities
```

Every agent occupies a point on this curve. There is no free lunch — more capabilities always cost debuggability unless you invest heavily in observability tooling (which itself adds complexity).

---

## Comprehensive Comparison Table

| Agent | Loop Type | Language | Streaming | Parallel Tools | Multi-Agent | Error Recovery | Context Mgmt | Stop Conditions |
|-------|-----------|----------|-----------|----------------|-------------|----------------|--------------|-----------------|
| mini-SWE-agent | Simple linear | Python | No | No | No | Feed error back | Linear (grow only) | Exit command, limits |
| Pi | Simple + extensions | TypeScript | Yes | No | Via extensions | Feed error back | Compaction | No tool calls |
| OpenCode | Streaming | Go | Yes (channels) | No (sequential) | Sub-agent (read-only) | Feed back + retry (8×) | Summarization | FinishReason |
| Goose | Streaming | Rust | Yes (async stream) | No (sequential) | No | Feed back + compaction | Visibility-based | Max turns (1000) |
| Gemini CLI | Streaming | TypeScript | Yes | Batched (read-only parallel) | No | Feed back | Token budgeting + caching | Iteration limits |
| OpenHands | Event-driven SM | Python | No | Via pending_actions queue | Agent delegation | StuckDetector (4 strategies) | Event sourcing + condensation | AgentFinishAction + limits |
| Codex CLI | Message-passing | Rust | Yes (SSE/WS) | Yes (join_all) | Sub-agent spawning | Sandbox escalation | Compaction + undo | Op::Interrupt + TurnComplete |
| Claude Code | Multi-agent | TypeScript | Yes | Yes | Sub-agents (Explore, General) | Feed back + redirect | Checkpoints | User-driven + natural |
| ForgeCode | Multi-agent | — | — | Sub-agent parallelism | Forge/Muse/Sage | Tool correction + budget | Cross-agent context | Verification enforcement |
| Aider | Edit-apply-verify | Python | Partial | No | No (architect = 2 models) | Fuzzy matching + retry | Repo map + summarization | Bounded retries |
| Ante | Multi-agent | Rust | — | Concurrent sub-agents | Meta-agent + sub-agents | Per-sub-agent | Per-agent bounded | Fan-in completion |
| Capy | Multi-agent | — | — | No | Captain/Build | Phase-based | Strict isolation | Phase completion |
| SageAgent | Pipeline | — | — | No | 5-agent pipeline | Observation feedback | Pipeline handoff | Observation judgment |
| Warp | Streaming + FTU | — | Yes | No | No | Terminal observation | Context + compaction | Handback pattern |
| Droid | Delegation | — | — | — | Cross-interface | — | Cross-interface | Autonomy ratio |
| Junie CLI | Edit-apply-verify | Kotlin | — | No | Multi-model delegation | Diagnostic loop (3–5×) | Phase-based | Verify pass |
| TongAgents | Multi-agent (inferred) | — | — | — | Multi-agent (inferred) | — | — | Verification |

### Reading the Table

- **Loop Type** describes the core architectural pattern, not every feature of the agent.
- **Parallel Tools** means concurrent tool *execution*, not concurrent tool *calls in a single LLM response* (which most agents support at the API level).
- **Error Recovery** ranges from simple (feed error text back) to sophisticated (stuck detection with multiple recovery strategies).
- **Context Mgmt** describes how the agent handles growing conversation history as it approaches the model's context window limit.

---

## Performance Correlation with Loop Complexity

### The Surprising Finding

The data from SWE-bench Verified (and similar benchmarks) reveals a counterintuitive pattern:

- **mini-SWE-agent** (simplest possible loop, ~200 lines) scores **65% on SWE-bench Verified** with Claude 3.5 Sonnet.
- Many architecturally complex agents score comparably or lower on the same benchmark.
- The "roulette" experiment (randomly switching models between turns) *improves* results — suggesting that diversity of reasoning patterns matters more than loop sophistication.

This leads to a critical insight: **MODEL QUALITY > LOOP COMPLEXITY** for most benchmark tasks.

### Why Benchmarks Undercount Complexity Benefits

Benchmarks like SWE-bench measure single-issue resolution in isolated repositories. They do not measure:

1. **Long-running session management** — real tasks span hours, not minutes
2. **Multi-repository coordination** — production tasks cross repo boundaries
3. **Interrupt and resume** — users stop and restart constantly
4. **Undo and rollback** — wrong paths need reversal, not just retry
5. **Observability** — teams need to audit what the agent did and why
6. **Concurrent users** — production systems serve many users simultaneously

These are exactly the capabilities that complex architectures provide.

### When Complexity Pays Off

| Scenario | Why Complexity Helps | Example Agent |
|----------|---------------------|---------------|
| Long-running tasks (>30 min) | Context management prevents degradation | OpenCode, Goose, Codex |
| Multi-file refactoring | Parallel tool execution reduces latency | Codex, Gemini CLI |
| Unreliable environments | Error recovery prevents cascading failures | OpenHands (StuckDetector) |
| Production deployments | Observability, interrupt, undo are required | Codex, Claude Code |
| Team workflows | Multi-agent coordination enables delegation | ForgeCode, Ante |
| CI/CD pipelines | Non-interactive mode needs policy-based approval | Codex (exec mode), ForgeCode |

### When Simplicity Wins

| Scenario | Why Simplicity Wins | Example Agent |
|----------|-------------------|---------------|
| Single-file edits | No context management needed | mini-SWE-agent |
| Research and fine-tuning | Trajectory = training data, minimal hidden state | mini-SWE-agent |
| Rapid prototyping | Time-to-first-result dominates | Pi, Smolagents |
| Tasks within one context window | No compaction/summarization needed | Any simple loop |
| Strong model available | Model self-manages errors and planning | mini-SWE-agent + Claude 3.5 |

---

## Language Choice Comparison

| Language | Agents | Strengths | Weaknesses |
|----------|--------|-----------|------------|
| Python | mini-SWE-agent, OpenHands, Aider | Fast development cycle, rich ML/AI ecosystem, easy prototyping, good library support for LLM APIs | Performance ceiling, GIL limits true concurrency, harder to ship as single binary |
| Rust | Codex CLI, Goose, Ante | Memory safety without GC, excellent performance, fearless concurrency with `tokio`, single-binary distribution | Steep learning curve, slower iteration speed, async Rust complexity |
| Go | OpenCode | Simple concurrency model (goroutines + channels), fast compilation, single binary, excellent stdlib | Less expressive type system, error handling verbosity, smaller AI ecosystem |
| TypeScript | Pi, Gemini CLI, Claude Code | Massive ecosystem (npm), async/await ergonomics, JSON-native, easy UI integration | Runtime overhead (Node.js), weaker type guarantees at runtime, dependency sprawl |
| Kotlin | Junie CLI | JVM ecosystem, null safety, coroutines for concurrency | JVM startup overhead, less common in AI tooling |

### Language Choice Implications for Loop Design

The language choice isn't just a preference — it fundamentally shapes the loop architecture:

- **Rust** agents (Codex, Goose) naturally gravitate toward message-passing and channel-based designs because Rust's ownership model makes shared mutable state painful. The `Op` enum in Codex and the async stream in Goose are idiomatic Rust.
- **Go** agents (OpenCode) use goroutines and channels extensively. The streaming loop in OpenCode is a goroutine that writes to a channel, consumed by the main loop — a pattern that would be more complex in any other language.
- **Python** agents (mini-SWE-agent, OpenHands) favor simplicity or event-driven patterns. Python's GIL means true parallelism requires multiprocessing or async I/O, pushing agents toward either simple sequential loops or async event systems.
- **TypeScript** agents (Pi, Gemini CLI, Claude Code) leverage async/await and the event loop. The streaming in Gemini CLI uses async iterators; Claude Code uses TypeScript's type system to define tool schemas.

---

## Key Architectural Decisions

### 1. Streaming vs Batch

**Streaming agents** deliver tokens to the user as they are generated. This is critical for UX — users want to see progress, not wait for a complete response.

| Approach | Agents | Implementation | Trade-off |
|----------|--------|----------------|-----------|
| Full streaming | OpenCode, Goose, Codex, Claude Code | Channel/stream per response | Complex buffering, partial tool-call detection |
| Partial streaming | Aider, Gemini CLI | Stream text, batch tool calls | Simpler, but tool execution feels laggy |
| No streaming | mini-SWE-agent, OpenHands | Wait for complete response | Simplest, but poor interactive UX |

**Key implementation challenge**: Detecting tool calls mid-stream. When the model starts emitting a JSON tool call, the agent must buffer it until the JSON is complete, then parse and execute it. This is trivial in batch mode but requires a state machine in streaming mode.

### 2. Sequential vs Parallel Tool Execution

| Approach | Agents | How It Works | When It Helps |
|----------|--------|--------------|---------------|
| Sequential | OpenCode, Goose, mini-SWE-agent, Aider | Execute tools one-at-a-time in order | Simple, deterministic, easy to debug |
| Parallel (all) | Codex CLI | `join_all` on all tool futures | Multi-file reads, search operations |
| Parallel (read-only) | Gemini CLI | Batch read operations, sequential writes | Safety — writes are never concurrent |

**The read/write distinction** (Gemini CLI) is elegant: reads are idempotent and safe to parallelize, while writes can conflict. This provides most of the latency benefit with none of the consistency risk.

### 3. Single vs Multi-Agent

| Pattern | Agents | Architecture | Strengths | Weaknesses |
|---------|--------|-------------|-----------|------------|
| Single agent | mini-SWE-agent, OpenCode, Goose, Aider | One model, one loop | Simple, predictable, debuggable | Context limits, no specialization |
| Architect + coder | Aider (dual model) | One model plans, another edits | Separates reasoning from execution | Brittle handoff, doubled latency |
| Meta-agent + sub-agents | ForgeCode, Ante | Orchestrator decomposes and delegates | Task decomposition, parallel work | Emergent failures, complex coordination |
| Typed sub-agents | Claude Code | Named agents (Explore, General) with specific capabilities | Clear roles, focused context | Agent selection heuristics can misfire |
| Pipeline | SageAgent | Fixed sequence of specialized agents | Predictable flow, clear handoff points | Rigid, can't adapt to novel task shapes |

### 4. State Management

This is perhaps the most consequential architectural decision, as it determines what operations are possible:

| Strategy | Agents | Data Structure | Enables | Prevents |
|----------|--------|---------------|---------|----------|
| Linear append | mini-SWE-agent | `list[dict]` (messages) | Perfect replay, simple trajectories | Undo, compaction, branching |
| Structured metadata | OpenCode, Goose | DB-backed session with metadata fields | Session management, search | True event sourcing |
| Event sourcing | OpenHands | Append-only event stream with condensation | Full replay, delegation, auditing | Nothing (most powerful, most complex) |
| Checkpoint/undo | Codex CLI, Claude Code | Snapshots with rollback capability | Undo, branching, resume from checkpoint | Requires significant storage |

**The event sourcing trade-off** (OpenHands): Every action and observation is an immutable event. This enables perfect replay and auditing — you can reconstruct the exact state of the agent at any point. But it also means the event stream grows without bound, requiring a condensation strategy (summarize old events to reclaim context window space).

**The checkpoint trade-off** (Codex, Claude Code): Periodic snapshots of file system + conversation state. This enables undo ("go back to before that refactoring") and resume ("pick up where I left off"). But each checkpoint is expensive to create and store.

### 5. Stop Strategy

How an agent decides to stop is a surprisingly deep design decision:

```
┌─────────────────────────────────────────────────────────┐
│                    Stop Strategies                        │
├─────────────────┬───────────────────────────────────────┤
│ Trust the model │ The model emits a "done" signal.      │
│                 │ No explicit turn limit.                │
│                 │ Risk: infinite loops if model confused.│
│                 │ Agents: OpenCode, Codex               │
├─────────────────┼───────────────────────────────────────┤
│ Hard limits     │ Fixed max turns / max tokens.         │
│                 │ Always terminates.                     │
│                 │ Risk: stops mid-task on hard problems. │
│                 │ Agents: mini-SWE-agent, OpenHands,    │
│                 │         Goose (1000 turns)             │
├─────────────────┼───────────────────────────────────────┤
│ Verification    │ Agent must prove the task is done      │
│ gate            │ (run tests, check output, etc).        │
│                 │ Risk: false confidence if tests pass   │
│                 │ but behavior is wrong.                 │
│                 │ Agents: ForgeCode, Junie               │
├─────────────────┼───────────────────────────────────────┤
│ User-driven     │ Human decides when to stop.            │
│                 │ Best UX, worst automation.             │
│                 │ Agents: Claude Code (interactive mode)  │
├─────────────────┼───────────────────────────────────────┤
│ Composite       │ Combines multiple strategies.          │
│                 │ E.g., trust model OR hard limit OR     │
│                 │ user interrupt.                        │
│                 │ Agents: Most production agents          │
└─────────────────┴───────────────────────────────────────┘
```

---

## Cross-Cutting Concerns

### Context Window Management

The context window is the fundamental constraint of every agent loop. As the conversation grows, the agent must decide what to keep and what to discard.

| Strategy | Used By | Mechanism | When Triggered | What's Lost |
|----------|---------|-----------|----------------|-------------|
| No management | mini-SWE-agent | Grow until limit, then fail | Never (short tasks only) | Nothing (until crash) |
| Summarization | OpenCode, Goose | LLM summarizes old messages | At configurable % threshold | Detail from early turns |
| Condensation | OpenHands | Replace event ranges with summaries | On context overflow | Individual event details |
| Compaction | Codex CLI | Rewrite submit queue entries | At ~90% of context window | Old tool outputs, verbose errors |
| Checkpointing | Claude Code, LangGraph | Snapshot full state, reset conversation | On demand or at thresholds | Nothing (state preserved externally) |
| Token budgeting | Gemini CLI | Pre-allocate token budgets per section | Before each turn | Proactively limits verbosity |
| Repo map | Aider | Compressed representation of codebase structure | Always present in context | File contents (only structure kept) |

**The compaction hierarchy** (from least aggressive to most):

1. **Truncate tool outputs** — keep first/last N lines of long outputs
2. **Summarize old turns** — replace detailed turns with one-line summaries
3. **Drop system messages** — remove non-essential system context
4. **Reset with checkpoint** — nuke the conversation, restore from snapshot

### Permission Models

Agents must balance autonomy with safety. The permission model determines how much the agent can do without human approval.

| Model | Used By | Approach | UX Impact |
|-------|---------|----------|-----------|
| Trust-based (no checks) | mini-SWE-agent, Aider | Agent executes anything in its tool set | Fast, but unsafe for production |
| Per-action approval | OpenCode, Goose | Each dangerous tool invocation asks the user | Safe, but interrupts flow constantly |
| Allowlist | Claude Code | Pre-approved commands bypass approval | Good balance — common ops are fast |
| Per-session approval | OpenCode | "Allow X for this session" | Reduces interrupt frequency over time |
| Policy-based | Codex CLI | Configurable approval levels (suggest/auto-edit/full-auto) | Most flexible, hardest to configure |
| Sandbox-first | Codex CLI | Execute in sandbox, escalate on failure | Safe by default, complexity in sandbox setup |

### Error Recovery Sophistication

Error recovery is where architectural complexity most directly translates to user-visible quality. A table of escalating sophistication:

```
Level 0: No recovery
         Crash on error. Agent stops.
         No agents use this in practice.

Level 1: Feed error back to LLM
         Append error text to conversation. Let model figure it out.
         Used by: mini-SWE-agent, Pi, most simple agents.
         Works surprisingly well with strong models.

Level 2: Retry with backoff
         On transient errors (rate limits, network), retry up to N times.
         Used by: OpenCode (8× retry), Goose (with compaction on retry).
         Essential for production reliability.

Level 3: Tool correction before execution
         Validate and fix tool calls before executing them.
         Used by: ForgeCode (Muse corrects Forge's tool calls).
         Prevents errors rather than recovering from them.

Level 4: Stuck detection + multi-strategy recovery
         Detect when agent is looping or making no progress.
         Apply escalating recovery strategies.
         Used by: OpenHands StuckDetector:
           Strategy 1: Detect identical action repetition
           Strategy 2: Detect alternating action pairs
           Strategy 3: Detect monologue (agent talking to itself)
           Strategy 4: Force context condensation and redirect

Level 5: Sandbox escalation + state rollback
         Execute in sandbox. On failure, roll back state and retry
         with elevated permissions or different strategy.
         Used by: Codex CLI (sandbox → escalate → compact → retry).
```

---

## Deep Dive: Loop Internals

### The Simplest Loop (mini-SWE-agent)

```python
# Pseudocode — the ENTIRE loop
messages = [system_prompt]
while True:
    response = llm.chat(messages)
    messages.append(response)
    if response.has_tool_calls():
        for tool_call in response.tool_calls:
            result = execute(tool_call)
            messages.append(result)
            if tool_call.name == "exit":
                return
    else:
        break  # no tool calls = done
```

This is ~20 lines. Everything is explicit. The trajectory (messages list) is the complete state. No hidden queues, no event buses, no state machines. Research groups love this because the trajectory *is* the training data for fine-tuning.

### The Most Complex Loop (Codex CLI, simplified)

```rust
// Pseudocode — heavily simplified from actual Codex implementation
loop {
    // Phase 1: Check submit queue
    let pending = submit_queue.drain();
    if pending.is_empty() && execution_queue.is_empty() {
        break; // nothing left to do
    }

    // Phase 2: Maybe compact
    if token_count(&pending) > 0.9 * context_window {
        pending = compact(pending); // rewrite history
    }

    // Phase 3: Send to model (streaming)
    let stream = model.stream(pending);
    let mut tool_calls = vec![];

    // Phase 4: Process stream
    while let Some(chunk) = stream.next().await {
        match chunk {
            Token(t) => emit_to_ui(t),
            ToolCall(tc) => tool_calls.push(tc),
            Interrupt => { cancel_stream(); break; }
        }
    }

    // Phase 5: Execute tools (parallel)
    let results = join_all(tool_calls.map(|tc| {
        sandbox.execute(tc).or_else(|e| escalate(tc, e))
    })).await;

    // Phase 6: Enqueue results
    for result in results {
        execution_queue.push(result);
    }
}
```

This is still simplified, but shows the key differences: dual queues, compaction, streaming with interrupt, parallel tool execution, sandbox with escalation. Each of these adds ~500–2000 lines of real implementation code.

---

## Key Insight: "The Best Loop Depends on the Model"

This is the single most important takeaway from studying 17+ agent implementations:

- **With GPT-4-class models**: Simple loops are surprisingly competitive. The model compensates for architectural simplicity with better reasoning.
- **With weaker models**: Complex loops compensate for model shortcomings — stuck detection, tool correction, and verification gates catch errors the model would miss.
- **The trend**: As models improve, loop complexity becomes *less* important for task completion. mini-SWE-agent's 65% on SWE-bench Verified with Claude 3.5 Sonnet demonstrates this clearly.
- **But**: Production requirements (observability, undo, interrupt, multi-user) *always* need architectural complexity regardless of model quality. These are engineering requirements, not intelligence requirements.
- **The sweet spot**: Start simple. Measure. Add complexity only when data shows it helps. Most teams over-engineer their agent loop relative to their model's capabilities.

### The Model-Architecture Interaction Matrix

```
                    Weak Model          Strong Model
                 ┌───────────────┬──────────────────┐
  Simple Loop    │  Poor results │  Excellent        │
                 │  (no safety   │  results          │
                 │   net)        │  (model self-     │
                 │               │   manages)        │
                 ├───────────────┼──────────────────┤
  Complex Loop   │  Decent       │  Excellent        │
                 │  results      │  results, but     │
                 │  (loop        │  unnecessary      │
                 │   compensates)│  overhead          │
                 └───────────────┴──────────────────┘
```

The asymmetry is clear: a strong model + simple loop matches a strong model + complex loop. But a weak model *needs* the complex loop. This means complex loops are future-proof but currently over-engineered for frontier models.

---

## Recommendations by Use Case

| Use Case | Recommended Pattern | Example Agent | Rationale |
|----------|-------------------|---------------|-----------|
| Research / fine-tuning | Simple linear | mini-SWE-agent | Trajectory = training data, no hidden state |
| Terminal coding agent | Streaming + compaction | OpenCode | Good UX, manageable complexity |
| Production coding tool | Multi-agent + checkpoints | Claude Code, Codex | Observability, undo, interrupt required |
| CI/CD integration | Non-interactive + policy | Codex (exec), ForgeCode | No human in loop, needs policy-based safety |
| Multi-step projects | Event-driven + delegation | OpenHands | Long-running, needs replay and recovery |
| Code editing tool | Edit-apply-verify | Aider | Tight feedback loop with user |
| Rapid prototype | Simple + framework | Smolagents, PydanticAI | Time-to-first-result matters most |
| Enterprise deployment | Message-passing + sandbox | Codex CLI | Security, audit, multi-user isolation |

---

## Emerging Patterns (2024–2025)

Several patterns are converging across agents:

1. **Compaction is universal** — Every agent hitting production adds some form of context window management. The specific strategy varies, but the need is universal.
2. **Streaming is table stakes** — No new agent launches without streaming. Users expect real-time output.
3. **Sandbox-first execution** — Codex pioneered this; others are following. Execute in isolation, escalate on failure.
4. **Model-routing** — Using different models for different sub-tasks (cheap model for search, expensive model for reasoning). Aider's architect mode was early; Claude Code's sub-agents formalize it.
5. **Verification gates** — Don't trust "I'm done." Run tests, check diffs, verify output. ForgeCode and Junie enforce this; others are adding it.
6. **Trajectory-as-data** — The agent's execution trace is increasingly treated as a first-class data product for fine-tuning, debugging, and compliance.

---

## Conclusion

The landscape of agent loop implementations reveals a clear evolutionary path from simple `while True` loops to sophisticated multi-agent orchestration systems. But evolution does not mean the earlier forms are obsolete — just as bacteria thrive alongside mammals, simple agent loops remain optimal for many use cases.

The key questions when designing an agent loop are:

1. **How strong is your model?** If frontier-class, start simple.
2. **How long are your tasks?** Long tasks need context management.
3. **Who are your users?** Developers need streaming; CI/CD needs policy.
4. **What are your safety requirements?** Production needs sandboxing and permissions.
5. **Do you need observability?** If yes, invest in event sourcing or structured logging.

Start with the simplest loop that could possibly work. Measure its failure modes. Add complexity *only* to address observed failures, never speculatively. The agents that score highest on benchmarks are often the simplest — but the agents that succeed in production are the ones that handle the messy reality of long-running, interruptible, multi-user workflows.