---
title: The Agentic Loop
status: complete
---

# The Agentic Loop

Every coding agent — regardless of language, framework, or architecture — is built around the same primitive: a loop that alternates between **thinking** (LLM inference) and **acting** (tool execution). This document synthesizes the patterns, variations, and design decisions observed across 17+ real-world implementations.

---

## Core Pattern

The agentic loop is a **ReAct (Reasoning + Acting)** cycle. The model receives context, decides what to do, the system executes that action, and the result feeds back as new context. This repeats until the model has nothing left to do.

### The Universal Pseudocode

```
messages = [system_prompt, user_task]

while true:
    response = llm.generate(messages)
    messages.append(response)

    tool_calls = parse_tool_calls(response)
    if tool_calls is empty:
        break                          # model is done

    for call in tool_calls:
        result = execute(call)
        messages.append(observation(result))
```

This is not a simplification — it is the **actual architecture** of the simplest agents. mini-SWE-agent's entire step method is two lines:

```python
def step(self):
    return self.execute_actions(self.query())
```

Query the model, execute whatever it says. The entire agent is `~100 lines`. No planning step, no reflection step, no tool selection logic — just query → execute → append → repeat. And it scores competitively on SWE-bench.

### Why This Pattern is Universal

Every agent we studied — from mini-SWE-agent's 100-line Python script to Codex CLI's thousands of lines of Rust — implements this same fundamental cycle. The reason is that it maps directly to how LLM APIs work:

1. **LLMs are stateless functions**: They take messages in and produce a response. The loop provides the statefulness.
2. **Tool use is the bridge**: The model can't read files or run commands directly. Tool calls are the only way to affect the world.
3. **Observations close the loop**: Without feeding results back, the model is flying blind. The loop is the feedback mechanism.
4. **Termination is natural**: When the model has enough information and has completed its work, it simply responds with text instead of tool calls. No explicit "stop" signal is needed.

The differences between agents — and they are substantial — emerge from what they layer **around** this core: how they manage state, when they stream, how they orchestrate multiple agents, and when they decide to stop.

---

## Variations

The agentic loop exists on a spectrum from minimal to highly orchestrated. Each variation adds capabilities but also complexity.

### The Spectrum

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
                                                              │
                                                    ┌─────────┴─────────┐
                                                    │  Edit-Apply-      │
                                                    │  Verify           │
                                                    │  (Aider, Junie)   │
                                                    │  [Orthogonal]     │
                                                    └───────────────────┘
```

### 1. Simple Linear Loop

**Representatives**: mini-SWE-agent, Pi

The purest form. A `while True` loop that calls the LLM, executes tool calls, appends results, and repeats. No streaming, no state machine, no message queues.

**Characteristics**:
- Entire message history passed to LLM on every call
- Synchronous execution — one tool call at a time
- Termination by checking if the last message is an exit signal
- No context management (messages go in, they never come out)

**Strengths**: Trivially debuggable. The trajectory saved to disk IS what the model saw. Perfect for research, fine-tuning, and RL — the trajectory is the training data.

**Limitations**: Context window fills up on long tasks. No way to interrupt mid-generation. No real-time feedback to the user.

Pi takes this same minimal pattern and makes it extensible — the core loop stays simple, but extensions can hook into events (`tool:start`, `tool:complete`, `response:complete`) to add lint/test cycles, permission gates, or sub-agent delegation without touching the loop itself.

### 2. Streaming Loop

**Representatives**: OpenCode (Go), Goose (Rust), Gemini CLI (TypeScript)

Adds token-by-token streaming to the basic loop. The LLM response is processed incrementally rather than waiting for completion, enabling real-time TUI rendering.

**Key difference from simple loop**: The "generate" step becomes a stream processing pipeline:

```
for event in provider.stream(messages):
    match event:
        TextDelta    → update display, accumulate text
        ToolUseStart → begin tracking new tool call
        ToolUseStop  → mark tool call complete
        Complete     → finalize message, track usage
        Error        → handle failure
```

**OpenCode** uses Go channels and context cancellation. Each streaming event immediately updates the message in the database and publishes via pub/sub, allowing the TUI to render in real time. Cancellation propagates through `context.Context` — the user presses Ctrl+X, the context cancels, and every layer (stream, tool execution, loop) checks `ctx.Done()`.

**Goose** wraps its loop in `reply()` → `reply_internal()`, yielding an `AgentEvent` stream. Before each LLM call, it injects MOIM (Model-Oriented Information Management) context from extensions — dynamic per-turn information that keeps persistent instructions always present. Tool results are collected via `tokio::select!` that merges tool result streams with elicitation messages (polled every 100ms).

**Gemini CLI** adds a **tool scheduler** between the model response and execution. When the model returns multiple tool calls, the scheduler batches read-only tools for parallel execution while sequencing mutating tools with confirmation gates. It also implements **token caching** — system instructions and tool declarations are cached server-side to reduce costs on long conversations.

### 3. Event-Driven State Machine

**Representative**: OpenHands

Replaces the simple loop with a formal state machine. An `AgentController` manages transitions between states (`INIT`, `RUNNING`, `PAUSED`, `AWAITING_USER_INPUT`, `FINISHED`, `ERROR`, `STOPPED`), and an `EventStream` acts as the central bus connecting controller, agent, runtime, and observers.

```
Controller._step():
    1. Check state, budget, iteration limits
    2. action = agent.step(state)
    3. Route action by type:
       - CondensationAction  → apply directly, re-step
       - AgentFinishAction   → transition to FINISHED
       - AgentDelegateAction → spawn child controller
       - MessageAction(wait) → transition to AWAITING_USER_INPUT
    4. Publish action to EventStream
    5. Run StuckDetector
    6. Runtime picks up action → executes → publishes Observation
    7. Next _step() sees Observation in state.history
```

**What makes this different**: Every action and every observation is a first-class `Event` in a persistent, append-only stream. Subscribers (runtime, security analyzer, logger, UI) independently react to events. The agent itself only sees conversation history and returns an `Action` — the controller handles all orchestration.

**Parallel tool calls** are handled via a `pending_actions` queue: if the LLM returns 3 tool calls, they're queued and drained one per `_step()` call, without additional LLM calls until the queue is empty.

**Agent delegation** uses a `NestedEventStore` — the child agent gets a filtered view of the parent's event stream, providing isolation while results flow back via `AgentDelegateObservation`.

### 4. Message-Passing (SQ/EQ)

**Representative**: Codex CLI

Implements the loop as a Tokio task processing submissions from a **Submission Queue (SQ)** and emitting events to an **Event Queue (EQ)**. This decouples the loop from its callers entirely.

```rust
while let Some(submission) = rx_sub.recv().await {
    match submission.op {
        Op::UserTurn { .. }    => { /* build prompt, stream, process items */ }
        Op::ExecApproval { .. } => { /* resolve blocked approval */ }
        Op::Compact { .. }      => { /* run context compaction */ }
        Op::Interrupt           => { /* cancel stream, abort tools */ }
        Op::Undo { .. }         => { /* rollback last N turns */ }
        Op::Shutdown            => break,
    }
}
```

**What makes this different**: The loop processes not just "run the next turn" but a vocabulary of operations — approvals, compactions, interrupts, undos, sub-agent spawns. Each operation type is a distinct message. The UI is completely decoupled; it submits `Op`s and reads `Event`s.

**Tool execution** is parallel by default when the model supports it (`futures::future::join_all`). The `ToolOrchestrator` handles the critical path: check approval → select sandbox → execute → handle sandbox denial with escalation retry.

**Auto-compaction** triggers when estimated token usage hits 90% of the context window (default ~272K tokens), using a remote compaction endpoint that summarizes the conversation while preserving `GhostSnapshot` items for undo support.

### 5. Multi-Agent Orchestration

**Representatives**: ForgeCode (Forge/Muse/Sage), Ante, Claude Code, Capy

Instead of one agent running one loop, multiple specialized agents coordinate to complete a task. The variations differ in how they orchestrate:

**ForgeCode** — Three named agents with distinct roles:
- **Muse**: Read-only planning agent. High thinking budget. Produces implementation plans.
- **Forge**: Read-write execution agent. Low thinking budget during execution. Follows plans.
- **Sage**: Read-only research agent. Delegated to by Forge/Muse for deep codebase analysis.

ForgeCode Services wraps these agents with: semantic entry-point discovery (before any agent runs), dynamic skill loading, tool-call correction (intercept and auto-fix before dispatch), progressive thinking policy (high budget for messages 1–10, low for 11+), and **verification enforcement** — the runtime programmatically requires a verification pass before marking any task complete. This was the key insight: prompting "please verify" doesn't reliably produce verification. Enforcement does.

**Ante** — Two-tier fan-out/fan-in: a meta-agent decomposes tasks into sub-tasks, dispatches them to concurrent sub-agents (each running its own inner plan-act-observe loop), then synthesizes results. Built in Rust with a lock-free scheduler for genuine concurrency. Self-organizing philosophy: sub-agents adapt autonomously rather than requiring the meta-agent to micromanage.

**Claude Code** — Three-phase loop (gather context → take action → verify) with sub-agent delegation. Sub-agents (Explore/Haiku for search, General-purpose/Sonnet for complex tasks) run in separate context windows. Key constraint: sub-agents cannot spawn sub-agents, preventing infinite nesting. The loop is not fixed-step — a simple question may need only one read; a complex bug fix cycles through all phases multiple times.

**Capy** — Three-phase handoff: Captain (planning, can ask user clarifying questions, cannot write code) → Build (execution, cannot ask questions, works autonomously). Hard constraints on each phase create natural quality gates.

### 6. Edit-Apply-Verify Loop

**Representatives**: Aider, Junie CLI

An orthogonal pattern that can be layered onto any loop: after the LLM produces edits, the agent **applies** them, then **verifies** via lint and test, and iterates on failures.

**Aider** — Human-directed single-turn edit cycle:
```
User message → Context assembly → LLM → Parse edits → Apply →
Git commit → Lint (if --auto-lint) → Test (if --auto-test) →
  └─ If errors: send to LLM → re-edit → re-apply → re-test
```

The agent becomes "agentic" only in the lint/test feedback sub-loop — it autonomously iterates on failures with bounded retries. In **architect mode**, this splits into two models: a reasoning model (o3, R1) describes the solution in prose, then an editing model (Sonnet, GPT-4) translates it into structured file edits. Each model plays to its strengths.

**Junie CLI** — Extends this with explicit phases (Understand → Plan → Implement → Verify → Diagnose) and dual execution paths. In IDE mode, implementation uses PSI-aware refactoring (semantically correct, all references updated, imports managed). In CLI mode, it falls back to text-based search-and-replace. Verification is first-class — not something the user must invoke, but an integral part of every task. Multi-model delegation routes sub-tasks to the most appropriate model (Flash for boilerplate, Sonnet for implementation, Opus for diagnosis).

### 7. Dual-Mode and Specialized Loops

Several agents implement mode-dependent loop behavior:

**Warp** — Distinguished by Full Terminal Use (FTU). Because Warp owns the PTY, the agent can interact with live interactive processes (psql, vim, Python REPL). The agent reads the terminal buffer, writes input to the PTY, and monitors long-running processes. A takeover/handback pattern transfers control between human and agent at natural breakpoints.

**Droid** — Delegation-oriented loop that works across interfaces (CLI, Slack, Linear, CI). Tracks an **autonomy ratio** (tool calls per user message, targeting 13x) as a key metric. Specification Mode separates planning (reasoning model) from execution (efficient model).

**SageAgent** — Linear multi-agent pipeline: TaskAnalysis → Planning → Executor → Observation → TaskSummary, with a single feedback loop from Observation back to Planning when the task is incomplete.

### Variation Comparison

| Variation | Complexity | Concurrency | Real-time UI | Error Recovery | Representative |
|-----------|-----------|-------------|-------------|----------------|----------------|
| Simple linear | Minimal | None | No | Feed error to LLM | mini-SWE-agent |
| Streaming | Low | Async I/O | Yes | Feed error + retry | OpenCode |
| Event-driven SM | Medium | Event-based | Yes | Stuck detection + condensation | OpenHands |
| Message-passing | High | Full async | Yes | Interrupt + undo + compaction | Codex |
| Multi-agent | High | Sub-agent parallelism | Yes | Distributed recovery | ForgeCode |
| Edit-apply-verify | Medium | None | Partial | Lint/test retry loop | Aider |

---

## State Management

How agents maintain context across loop iterations is a critical architectural decision. The approaches form a spectrum from trivially simple to highly structured.

### 1. Linear Message History

**Used by**: mini-SWE-agent, Pi

The simplest approach: a flat `list[dict]` where messages are appended and never removed.

```python
self.messages: list[dict] = []
# Messages go in, they never come out
def add_messages(self, *messages):
    self.messages.extend(messages)
```

**Advantage**: The trajectory IS the state. Perfect reproducibility — replay the same messages and you get the same behavior. Ideal for research and fine-tuning.

**Limitation**: No context management. Long tasks exhaust the context window. mini-SWE-agent's typical SWE-bench trajectory is ~20-40 steps — short enough that this works, but it doesn't scale to longer sessions.

### 2. Conversation Objects with Metadata

**Used by**: OpenCode, Goose, Gemini CLI, most production agents

Messages are wrapped in structured objects with metadata (model, finish reason, token usage, timestamps). The conversation is persisted to a database or file system.

OpenCode persists every message to a database and publishes updates via pub/sub. When the session has a summary, it truncates history to the summary message and re-roles it as a "user" message — effectively restarting the conversation from the summary while preserving context.

Goose tracks messages with visibility flags — messages marked invisible by summarization are excluded from LLM calls but preserved in the session. A `tool_call_cut_off` determines where old tool-call pairs should be summarized in the background.

### 3. Event Streams with Action/Observation Pairs

**Used by**: OpenHands

State is an append-only `EventStream` of typed events. Each action (what the agent wants to do) and observation (what happened) is a discrete event with metadata (source, timestamp, cause).

```
EventStream: [UserMsg₀, CmdRun₁, CmdOutput₂, FileEdit₃, FileEditObs₄, ...]
```

The agent sees a **projected view** of this stream (the `State` object), not the raw events. This enables:
- **Condensation**: Old events can be summarized without destroying the original stream
- **Delegation**: Child agents get a filtered view via `NestedEventStore`
- **Replay**: The stream can be replayed to reconstruct state
- **Observation**: External systems can subscribe to the stream for monitoring

### 4. Structured Session State with Checkpointing

**Used by**: Codex CLI, Claude Code

Codex's `ContextManager` maintains a structured item list (`Vec<ResponseItem>`) with operations for compaction, undo, and rollback. `GhostSnapshot` items are preserved across compaction and undo, enabling redo. Sessions are persisted as JSONL rollouts that can be resumed across terminal restarts.

Claude Code adds **checkpoints** that capture both conversation state and code state. Users can rewind to any checkpoint, restoring conversation, files, or both. Checkpoints persist across sessions.

### 5. Multi-Agent Shared State

**Used by**: ForgeCode, Ante, Capy

When multiple agents collaborate, state management must handle cross-agent coordination:

ForgeCode preserves context across agent switches (Muse → Forge → Sage) while keeping each agent's internal working context bounded. The `todo_write` tool provides explicit shared state — each agent reads and updates the same task list.

Ante's sub-agents have independent contexts (each with a focused, smaller window) but share results through the meta-agent's fan-in step. This prevents context window exhaustion that would occur if all sub-task context were in one window.

Capy enforces strict isolation: Captain's output (the spec) is the sole interface to Build. No shared mutable state, no mid-execution communication.

### State Management Comparison

| Approach | Context Control | Reproducibility | Multi-Agent | Persistence |
|----------|----------------|-----------------|-------------|-------------|
| Linear list | None (grow only) | Perfect | N/A | Trajectory file |
| Conversation + metadata | Summarization | Good | Shared DB | Database |
| Event stream | Condensation | Perfect (replay) | NestedEventStore | Append-only log |
| Structured + checkpoints | Compaction + undo | Good (rollout) | Sub-agent sessions | JSONL rollout |
| Multi-agent shared | Per-agent bounded | Partial | Explicit interfaces | Per-session |

---

## Stop Conditions

The loop must terminate. Agents employ a layered approach — multiple independent conditions, any of which can halt execution.

### 1. Model Signals Completion

The most natural termination: the LLM responds with text and **no tool calls**. This means it has nothing left to do.

```
# Every agent checks this
if response.tool_calls is empty:
    break  # model is done
```

Variants:
- mini-SWE-agent: checks `messages[-1].role == "exit"` (the model outputs a special submit command)
- OpenCode: checks `FinishReason == EndTurn` (not `ToolUse`)
- Goose: text-only response → exit the loop
- Codex: `!session.has_pending_tool_results()` → emit `TurnComplete`
- OpenHands: agent returns `AgentFinishAction` → controller transitions to `FINISHED`

This is the **only universal** stop condition — every agent implements it. All others are safety nets.

### 2. Iteration / Token Budget Limits

Hard caps to prevent runaway loops:

| Agent | Iteration Limit | Budget Limit |
|-------|----------------|--------------|
| mini-SWE-agent | `step_limit` (configurable) | `cost_limit` ($3.00 default) |
| OpenHands | `max_iterations` per run | Per-task budget |
| Goose | `max_turns` (1000 default, via `GOOSE_MAX_TURNS`) | — |
| Gemini CLI | Max iterations per turn + timeout | Token budget exhaustion |
| Junie | 3–5 implement-verify cycles | Token + time budget |
| ForgeCode | `max_requests_per_turn` | `FORGE_TOOL_TIMEOUT` (300s) |
| Codex | — (no explicit turn limit) | Auto-compaction at 90% window |

OpenCode notably has **no explicit turn limit** — it trusts the model to know when to stop. This is a deliberate design choice with trade-offs.

### 3. User Interrupt

Every interactive agent supports interruption, but the mechanisms differ:

| Agent | Mechanism | Behavior |
|-------|-----------|----------|
| OpenCode | `Cancel(sessionID)` → context cancellation | Clean abort at next check point |
| Claude Code | `Esc` key | Stop mid-action, context preserved, redirect |
| Goose | `CancellationToken` | Checked at loop top and during tool collection |
| Codex | `Op::Interrupt` → cancel stream + abort tools | Double Ctrl+C exits program |
| Warp | Handback pattern | Agent signals completion at natural breakpoints |

Claude Code's approach is notable: the user can **type while Claude works** — Claude sees the message and adjusts approach. No need to wait for completion.

### 4. Error Accumulation

Rather than failing on a single error, agents can tolerate errors up to a threshold:

- **ForgeCode**: `max_tool_failure_per_turn` — limits consecutive tool failures before forcing completion, preventing infinite retry loops
- **OpenHands**: Error loop detection in StuckDetector — if last K observations are all `ErrorObservation`, the agent is stuck
- **Goose**: `ContextLengthExceeded` triggers compaction (up to 2 attempts), then gives up

### 5. Stuck Detection

**OpenHands' StuckDetector** is the most sophisticated approach, running after every step:

| Strategy | Pattern | Detection |
|----------|---------|-----------|
| Identical repetition | `action[n] == action[n-1] == action[n-2]` | Same action 3x in a row |
| Alternating pattern | `action[n] == action[n-2] && action[n-1] == action[n-3]` | Ping-ponging between two actions |
| Error loop | Last K observations are all errors | Repeated errors without recovery |
| Empty response | Last K actions have empty content | Degenerate LLM output |

Recovery options: raise `AgentStuckInLoopError` (terminate), inject `LoopRecoveryAction` (nudge), or force condensation (compress history to break the pattern).

**Goose's RepetitionInspector** serves a similar function — it detects tools called repeatedly without progress and can block further calls.

### 6. Verification Pass

Some agents require explicit verification before allowing completion:

**ForgeCode**: The runtime **programmatically enforces** a verification pass. Before a task is marked complete, it checks whether the agent called the verification skill. If not, the runtime injects a reminder and requires it — no opt-out. The verification generates a checklist: what was requested, what was done, what evidence exists, what's missing. This was the key insight from TermBench evaluation — models under pressure skip optional verification.

**Aider**: The lint/test cycle after applying edits serves as implicit verification. If `--auto-test` is enabled and tests fail, the agent iterates (with bounded retries) before presenting results.

**Junie**: Verification is first-class. After implementation, the agent runs tests, checks inspections, validates compilation. If failures are found, it enters a diagnostic loop (analyze → fix → re-verify) for up to 3–5 iterations before escalating.

### Stop Condition Layering

In practice, agents use multiple conditions simultaneously:

```
while true:
    if cancelled:           break    # User interrupt
    if iterations > max:    break    # Safety cap
    if budget_exceeded:     break    # Cost control
    if stuck_detected:      break    # Pattern detection

    response = llm.generate(messages)

    if no_tool_calls(response):
        if verification_required and not verified:
            inject_verification_prompt()
            continue
        break                        # Natural completion

    execute_tools(response)
```

The order matters: user interrupt takes highest priority, then resource limits, then stuck detection, then natural completion with optional verification enforcement.

## Tools & Projects

This section surveys the key frameworks, protocols, benchmarks, and observability tools relevant to building and evaluating agentic loops in coding agents. For deep dives on each tool, see the [full research file](../../.copilot/session-state/d0354d87-4da1-4c93-a043-006f84879a18/files/agentic-loop-tools.md).

### Agent Frameworks

| Framework | Language | Paradigm | Key Differentiator |
|-----------|----------|----------|--------------------|
| **LangGraph** | Python, JS/TS | Graph-based state machines | Checkpointing, time-travel debugging, durable execution |
| **CrewAI** | Python | Role-based crews + flows | YAML-configured agent teams with sequential/hierarchical processes |
| **AutoGen** | Python, .NET | Multi-agent conversation | Agents as tools, group chat, AutoGen Studio GUI |
| **Smolagents** | Python | Code-as-action ReAct | LLM writes Python as actions — 30% fewer steps than JSON tool-calling |
| **PydanticAI** | Python | Type-safe agents | `Agent[DepsType, OutputType]` generics, dependency injection, Pydantic validation |
| **Mastra** | TypeScript | Workflow + agents | `.then()/.branch()/.parallel()` chainable syntax, React/Next.js integration |
| **OpenAI Agents SDK** | Python | Handoff chains | Minimal abstractions, guardrails, built-in tracing; production successor to Swarm |
| **Google ADK** | Python, Java, Go | Hierarchical multi-agent | Session rewind, `adk eval` CLI, code execution sandbox, A2A native |

- **LangGraph** — Low-level orchestration modeling agents as cyclic graphs (nodes = steps, edges = transitions). Supports checkpointing, time-travel debugging, and subgraph composition. https://github.com/langchain-ai/langgraph — Ideal for coding agents needing plan→code→test→debug loops with failure recovery.
- **CrewAI** — Multi-agent framework where "crews" of role-based agents collaborate on tasks. Crews handle autonomous collaboration; Flows add deterministic, event-driven control. https://github.com/crewAIInc/crewAI — Maps naturally to planner/coder/reviewer/tester agent teams.
- **AutoGen** — Microsoft's layered multi-agent framework with Core, AgentChat, and Extensions APIs. The `AgentTool` pattern wraps agents as callable tools for hierarchical setups. https://github.com/microsoft/autogen — Enables coding agents that discuss and refine code through multi-turn dialogue.
- **Smolagents** — Hugging Face's minimal library (~1,000 LOC) where the CodeAgent writes executable Python as its action rather than JSON tool specs. https://github.com/huggingface/smolagents — The most directly relevant paradigm for coding agents; code-as-action mirrors what coding agents actually do.
- **PydanticAI** — Type-safe agent framework bringing the "FastAPI feeling" to GenAI. Agents are generic types with dependency injection and automatic output validation. https://github.com/pydantic/pydantic-ai — Excellent for agents needing reliable structured outputs (diffs, AST edits, test results).
- **Mastra** — TypeScript-first framework for the JS/Node ecosystem with intuitive workflow chaining and suspend/resume. https://github.com/mastra-ai/mastra — Primary option for building coding agents in TypeScript.
- **OpenAI Agents SDK** — Lightweight, production-ready evolution of Swarm. Core loop: get completion → execute tools → handle handoffs → repeat. https://github.com/openai/openai-agents-python — Clean handoff pattern routes coding tasks to specialist sub-agents.
- **Google ADK** — Code-first framework optimized for Gemini but model-agnostic. Hierarchical multi-agent with parent→child routing and session rewind. https://github.com/google/adk-python — Session rewind is uniquely valuable for debugging agent trajectories.

### Multi-Agent Protocols

- **Swarm** — OpenAI's experimental (now deprecated) framework that introduced the **handoff** pattern: an agent delegates by returning another agent from a function. Stateless between calls. https://github.com/openai/swarm — Established the handoff pattern now standard in the OpenAI Agents SDK.
- **A2A (Agent-to-Agent)** — Google-contributed open protocol (under Linux Foundation) for communication between opaque agents. Uses Agent Cards for discovery and JSON-RPC 2.0 over HTTP(S). https://github.com/a2aproject/A2A — Enables heterogeneous agent ecosystems (e.g., a LangGraph coding agent delegating to a CrewAI testing agent). Complements MCP: MCP = agent-to-tool, A2A = agent-to-agent.

### Benchmarks & Evaluation

| Benchmark | Focus | Scale | Why It Matters |
|-----------|-------|-------|----------------|
| **SWE-bench** | Real GitHub issue resolution | 2,294 tasks (500 verified) | Gold-standard for coding agents — tests the full agentic loop |
| **Terminal-Bench** | Terminal/CLI agent tasks | — | Tests command-line navigation and system administration |
| **AgentBench** | Multi-environment (OS, DB, web) | 8 environments | Broadest coverage — OS interaction and DB tasks are coding-adjacent |

- **SWE-bench** — The definitive coding agent benchmark. Agents receive a real codebase + issue description and must produce a working patch. Variants include Verified (500 human-vetted problems), Multimodal, and Multilingual. Current SOTA: ~74% on Verified. https://swebench.com — Tests understanding, navigation, code generation, and test-passing in one benchmark.
- **Terminal-Bench** — Benchmark focused on terminal and CLI tasks, testing an agent's ability to navigate file systems, run commands, and solve system-level problems. Relevant for coding agents that operate primarily through shell interfaces.
- **AgentBench** — First multi-environment agent benchmark (Tsinghua). Tests LLM-as-Agent across OS interaction, databases, knowledge graphs, web browsing, and more. https://github.com/THUDM/AgentBench — The OS and DB tasks directly test capabilities coding agents need beyond pure code generation.

### Observability & Debugging

| Tool | Open Source | OTel-Based | Multi-Framework | Best For |
|------|------------|------------|-----------------|----------|
| **LangSmith** | SDK only | No | LangChain/LangGraph | Full-lifecycle LLM observability with evals |
| **Phoenix (Arize)** | Yes | Yes | 15+ integrations | Vendor-agnostic tracing and evaluation |
| **Logfire (Pydantic)** | No | Yes | PydanticAI-focused | Real-time debugging with Pydantic integration |

- **LangSmith** — Full-lifecycle observability platform: trace, debug, evaluate, and monitor agents. Uses `@traceable` decorator for automatic instrumentation. https://github.com/langchain-ai/langsmith-sdk — Essential for understanding agent decisions in LangGraph-based coding agents.
- **Phoenix (Arize)** — Open-source AI observability built on OpenTelemetry via OpenInference. Supports OpenAI Agents SDK, LangGraph, CrewAI, Mastra, and many more. https://github.com/Arize-ai/phoenix — The leading open-source alternative to LangSmith; vendor-agnostic approach works with any agent framework.
- **Logfire (Pydantic)** — OpenTelemetry-based observability with deep PydanticAI integration. Real-time debugging, behavior tracing, and cost tracking. https://pydantic.dev/logfire — Tight integration makes it the natural choice for PydanticAI-based coding agents.

## Real-World Implementations

| Agent | Loop Style | Reference |
|-------|-----------|-----------|
| **mini-SWE-agent** | Simplest loop (~100 lines), ReAct with bash-only tooling | [`../agents/mini-swe-agent/agentic-loop.md`](../agents/mini-swe-agent/agentic-loop.md) |
| **opencode** | Go agentic loop with streaming support and cancellation | [`../agents/opencode/agentic-loop.md`](../agents/opencode/agentic-loop.md) |
| **OpenHands** | Event-driven state machine loop | [`../agents/openhands/agentic-loop.md`](../agents/openhands/agentic-loop.md) |
| **Codex** | SQ/EQ message-passing loop implemented in Rust | [`../agents/codex/agentic-loop.md`](../agents/codex/agentic-loop.md) |
| **Claude Code** | Three-phase loop with sub-agent delegation | [`../agents/claude-code/agentic-loop.md`](../agents/claude-code/agentic-loop.md) |
| **ForgeCode** | Multi-agent orchestration (Forge/Muse/Sage) | [`../agents/forgecode/agentic-loop.md`](../agents/forgecode/agentic-loop.md) |
| **Goose** | `reply()` → `reply_internal()` streaming loop | [`../agents/goose/agentic-loop.md`](../agents/goose/agentic-loop.md) |
| **Aider** | Edit → apply → lint → test loop | [`../agents/aider/agentic-loop.md`](../agents/aider/agentic-loop.md) |
