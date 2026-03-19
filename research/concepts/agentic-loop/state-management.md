# State Management in Agentic Loops

## Overview

How agents maintain context across loop iterations is a critical architectural decision—arguably
the single most consequential design choice after model selection. Every message in the conversation
history is replayed on every future LLM call, so the cost of state management is not O(n) but
O(n²) in the worst case: each new turn pays for every previous turn.

The approaches form a spectrum from trivially simple to highly structured:

```
Linear List ──→ Wrapped Messages ──→ Event Sourcing ──→ Checkpointed State
 (append)        (metadata)           (projections)      (time-travel)
```

State management directly impacts:
- **Reproducibility**: Can you replay a session and get the same result?
- **Context window usage**: How efficiently do you use the model's finite context?
- **Multi-agent support**: Can multiple agents share or partition state?
- **Debuggability**: Can you inspect what the model saw at each step?
- **Crash recovery**: If the process dies, how much work is lost?
- **Cost**: Every token in the history is billed on every subsequent call.

The right choice depends on task duration. A 20-step SWE-bench fix has different needs than
a 200-turn interactive coding session.

---

## 1. Linear Message History (Simplest)

### Used by: mini-SWE-agent, Pi

The most minimal approach: a Python list that only grows.

```python
class Agent:
    def __init__(self, system_prompt: str, model: str):
        self.messages: list[dict] = [
            {"role": "system", "content": system_prompt}
        ]
        self.model = model

    def add_messages(self, *messages):
        """Append messages. Never remove, never modify."""
        self.messages.extend(messages)

    def run_step(self):
        response = client.chat.completions.create(
            model=self.model,
            messages=self.messages,  # Send EVERYTHING every time
            tools=self.tools,
        )
        assistant_msg = response.choices[0].message
        self.add_messages(assistant_msg)

        if assistant_msg.tool_calls:
            for tc in assistant_msg.tool_calls:
                result = self.execute_tool(tc)
                self.add_messages({
                    "role": "tool",
                    "tool_call_id": tc.id,
                    "content": result,
                })
```

**Messages go in, they never come out.** The trajectory IS the state.

### Characteristics

| Property | Value |
|----------|-------|
| Write pattern | Append-only |
| Read pattern | Full scan (send all to model) |
| Mutation | Never |
| Deletion | Never |
| Persistence | Dump list to JSON |

### Why Linear Works

- **Perfect reproducibility**: replay the same messages → same behavior (modulo temperature)
- **No information loss**: the model sees exactly what happened
- **Simple debugging**: `print(json.dumps(self.messages, indent=2))` — that's the complete state
- **Training data**: trajectory files load directly as fine-tuning/RL data with no transformation
- **No risk of losing critical context during summarization**: a subtle but important bug that
  plagues more sophisticated approaches

```
Step 0: [system]
Step 1: [system, user]
Step 2: [system, user, assistant]
Step 3: [system, user, assistant, tool_result]
Step 4: [system, user, assistant, tool_result, assistant]
...
Step N: [system, user, assistant, tool_result, ..., assistant]  ← entire history every call
```

### Limitation: Context Window Saturation

The fatal flaw is arithmetic. If each step adds ~1,000 tokens (assistant message + tool result),
then after 100 steps you're sending 100K tokens on every call. At GPT-4o prices, step 100 alone
costs as much as steps 1-10 combined.

```
Tokens sent per step (cumulative):
Step  1:   1,000 tokens
Step 10:  10,000 tokens
Step 50:  50,000 tokens
Step 100: 100,000 tokens  ← approaching context limits
Step 200: 200,000 tokens  ← exceeds most models

Total tokens across all steps = n(n+1)/2 × avg_step_size
For 100 steps: ~5,050,000 tokens total API usage
```

**Works for SWE-bench** (typically 20-40 steps) but **not for long interactive sessions** (100+ turns).

This is precisely why more sophisticated approaches exist—they're all attempts to break this
quadratic cost curve while preserving as much of the linear model's simplicity as possible.

---

## 2. Conversation Objects with Metadata

### Used by: OpenCode, Goose, Gemini CLI

The next step up: wrap each message in a structured object that carries metadata beyond
role/content.

```go
// OpenCode's message structure (Go)
type Message struct {
    ID          string       `json:"id"`
    Role        Role         `json:"role"`           // user, assistant, tool
    Parts       []ContentPart `json:"parts"`          // text, tool_call, tool_result, image
    Model       string       `json:"model,omitempty"` // which model generated this
    FinishReason FinishReason `json:"finish_reason,omitempty"`
    TokenUsage  Usage        `json:"token_usage"`
    CreatedAt   time.Time    `json:"created_at"`
    Visible     bool         `json:"visible"`        // KEY: controls summarization
}

type Usage struct {
    InputTokens  int `json:"input_tokens"`
    OutputTokens int `json:"output_tokens"`
    CacheRead    int `json:"cache_read_tokens"`    // Anthropic prompt caching
    CacheWrite   int `json:"cache_write_tokens"`
}
```

The `Visible` field is the crucial addition. Messages can be **present in the session record**
but **excluded from the LLM call**. This decouples the audit trail from the active context.

### OpenCode's Database-Backed Messages

OpenCode persists every message to a SQLite database immediately upon creation:

```go
// Simplified from OpenCode's session store
func (s *Store) AddMessage(sessionID string, msg Message) error {
    tx, _ := s.db.Begin()
    defer tx.Rollback()

    _, err := tx.Exec(`
        INSERT INTO messages (id, session_id, role, parts, model, token_usage, created_at, visible)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
        msg.ID, sessionID, msg.Role, marshal(msg.Parts),
        msg.Model, marshal(msg.TokenUsage), msg.CreatedAt, msg.Visible,
    )
    if err != nil {
        return err
    }

    tx.Commit()
    s.publish(SessionUpdated{ID: sessionID})  // Pub/sub → TUI re-renders
    return nil
}
```

**Design benefits**:
- **Crash recovery**: every message written before the next LLM call
- **Real-time UI**: pub/sub notifies the TUI of new messages as they arrive
- **Multi-process safe**: SQLite handles concurrent access
- **Query flexibility**: find all messages by model, by token count, by time range

**Summarization flow**:
1. Token count exceeds threshold
2. Summarize old messages via LLM
3. Mark old messages as `Visible: false`
4. Insert summary message as `Role: "user"` (re-roled for compatibility)
5. Future LLM calls see: `[system, summary_as_user, recent_messages...]`

### Goose's Visibility-Based Management

Goose takes a similar approach but with more explicit cut-off tracking:

```python
# Goose's summarization strategy (conceptual)
class ConversationManager:
    def __init__(self):
        self.messages: list[Message] = []
        self.tool_call_cut_off: int = 0  # Index of oldest non-summarized tool call

    def should_summarize(self) -> bool:
        visible_tokens = sum(m.token_count for m in self.messages if m.visible)
        return visible_tokens > self.context_limit * 0.80

    def summarize(self):
        # Find old tool-call pairs to summarize
        old_pairs = self.messages[self.tool_call_cut_off : self.find_recent_boundary()]

        summary = self.llm.summarize(old_pairs)
        for msg in old_pairs:
            msg.visible = False  # Still in session, but excluded from LLM calls

        self.insert_summary(summary)
        self.tool_call_cut_off = self.find_recent_boundary()
```

**Key insight**: tool call/result pairs are the primary candidates for summarization because
they're often verbose (file contents, command output) but their semantic content can be
compressed. A 500-line file read becomes "Read config.py, which defines database connection
settings and migration paths."

### Goose Background Summarization

Goose performs summarization in the background, not blocking the main loop:

```
Main Thread                    Background Thread
───────────                    ─────────────────
step() → LLM call             
  ← response                  
check token count              
  if > 80%: trigger ──────────→ summarize(old_messages)
continue stepping                ← summary ready
next LLM call uses summary ←── inject summary, mark old invisible
```

This avoids the latency hit of synchronous summarization, though it means one or two steps
may run with a nearly-full context window before the summary is ready.

---

## 3. Event Sourcing (Append-Only Log)

### Used by: OpenHands

Event sourcing is an architectural pattern from distributed systems applied to agent state.
Instead of storing "current state," you store the sequence of events that produced it.

```
EventStream: [UserMsg₀, CmdRun₁, CmdOutput₂, FileEdit₃, FileEditObs₄, ...]
                ↓           ↓          ↓           ↓            ↓
              Event       Event      Event       Event        Event
              id=0        id=1       id=2        id=3         id=4
```

Each event is a discrete, immutable record:

```python
# OpenHands event types (simplified)
@dataclass
class Event:
    id: int                    # Monotonically increasing
    timestamp: datetime
    source: EventSource        # AGENT, USER, ENVIRONMENT
    cause: int | None          # ID of event that caused this one

@dataclass
class CmdRunAction(Event):     # Agent decides to run a command
    command: str
    thought: str               # Agent's reasoning
    blocking: bool

@dataclass
class CmdOutputObservation(Event):  # Environment returns output
    content: str
    exit_code: int
    command: str

@dataclass
class FileEditAction(Event):   # Agent decides to edit a file
    path: str
    content: str               # Or diff/replacement spec

@dataclass
class AgentCondenseAction(Event):  # Agent decides to condense history
    pass                       # Triggers condensation of older events
```

### Event Sourcing Principles Applied

**Append-only**: events are never modified or deleted. Once CmdRun₁ exists, it exists forever.
If a command is "undone," that's a new UndoAction event, not a deletion of the original.

**Projection**: the current state is derived (projected) from the event sequence. The agent
never reads the raw event stream; it reads a `State` object computed from events:

```python
class State:
    """Projected view of the event stream for the agent."""
    history: list[Event]           # May be condensed
    plan: Plan                     # Extracted from events
    iteration: int                 # Count of agent actions
    max_iterations: int
    token_usage: TokenUsage        # Accumulated across events
    metrics: Metrics

    @classmethod
    def from_events(cls, events: list[Event]) -> "State":
        state = cls()
        for event in events:
            state.apply(event)     # Each event updates the projection
        return state
```

**Temporal queries**: because all events are preserved, you can answer questions like:
- "What was the state at step 15?" → replay events 0-15
- "What commands ran between steps 10 and 20?" → filter events by type and range
- "How many tokens were used before condensation?" → sum token events up to condensation point

**Separation of concerns**: writing events (the agent loop and environment) is decoupled
from reading state (the agent's prompt construction and the UI).

### Event Stream Capabilities

The event-sourced design enables several powerful patterns:

```
                    ┌─────────────────────────────┐
                    │        EventStream           │
                    │  [E₀, E₁, E₂, ..., Eₙ]     │
                    └──────┬──────┬───────┬────────┘
                           │      │       │
              ┌────────────┘      │       └────────────┐
              ▼                   ▼                     ▼
     ┌─────────────┐    ┌──────────────┐    ┌──────────────────┐
     │ Condensation │    │  Delegation  │    │  External        │
     │ (compress    │    │ (filtered    │    │  Monitoring      │
     │  old events) │    │  child view) │    │  (subscriptions) │
     └─────────────┘    └──────────────┘    └──────────────────┘
```

1. **Condensation without destroying originals**: older events are summarized into a
   single condensation event. The originals remain in the stream but are excluded from
   the projected state.

2. **Delegation via NestedEventStore**: a child agent receives a filtered view of the
   parent's event stream, containing only events relevant to its delegated task. The
   child's actions are appended to a sub-stream that flows back to the parent.

3. **Replay for debugging**: re-run the agent from any point by replaying events.
   Combined with deterministic tool execution, this enables exact reproduction of bugs.

4. **External monitoring via subscriptions**: the UI, logging systems, and metrics
   collectors subscribe to the event stream and react in real time.

---

## 4. Checkpointing and Time-Travel

### Used by: Codex CLI, Claude Code, LangGraph

Checkpointing goes beyond append-only logs by capturing snapshots of the complete state
at specific points, enabling efficient restoration without replaying the entire history.

### Codex CLI's ContextManager

Codex CLI maintains a structured item list with explicit operations for manipulation:

```rust
// Codex CLI's context management (Rust, simplified)
struct ContextManager {
    items: Vec<ResponseItem>,       // The conversation items
    ghost_snapshots: Vec<Snapshot>, // Preserved for redo after undo
    session_id: String,
    rollout_path: PathBuf,          // JSONL file for persistence
}

enum Op {
    /// Add a new item to the context
    Push(ResponseItem),

    /// Compact: replace items with model-generated summary
    Compact {
        model: String,
        summary_instructions: String,
    },

    /// Undo: remove the last N turns
    Undo {
        n_turns: usize,
        snapshot: GhostSnapshot,  // Save removed items for redo
    },

    /// Rollback: deep reset to a previous state
    Rollback {
        to_index: usize,
    },
}
```

**GhostSnapshot**: When items are removed via Undo or Compact, they're preserved as
"ghost" snapshots. This enables redo—undone items can be restored because they were
never actually deleted, just moved to a shadow list.

```
Before Undo:
  items: [A, B, C, D, E]

After Undo(n=2):
  items: [A, B, C]
  ghost_snapshots: [{items: [D, E], at_index: 3}]

After Redo:
  items: [A, B, C, D, E]     ← restored from ghost
  ghost_snapshots: []
```

**JSONL Rollouts**: Sessions are persisted as newline-delimited JSON, one event per line:

```jsonl
{"type":"session_start","id":"abc123","model":"o4-mini","timestamp":"2025-01-15T10:00:00Z"}
{"type":"user_message","content":"Fix the login bug in auth.py"}
{"type":"assistant_message","content":"I'll look at auth.py...","tool_calls":[...]}
{"type":"tool_result","tool_call_id":"tc_1","content":"...file contents..."}
{"type":"compaction","summary":"Investigated auth.py, found session token not refreshed..."}
```

This enables session resumption across terminal restarts: load the JSONL, replay to current state.

### Claude Code's Checkpoints

Claude Code takes checkpointing further by capturing both **conversation state** and **code state**:

```
Checkpoint at step 15:
┌──────────────────────────────────┐
│ Conversation State               │
│  - Messages 0-15                 │
│  - Current plan                  │
│  - Tool results cache            │
├──────────────────────────────────┤
│ Code State                       │
│  - Git stash or shadow copy      │
│  - Modified files snapshot       │
│  - Working directory state       │
└──────────────────────────────────┘
```

**User rewind**: the user can jump back to any checkpoint, restoring:
- Just the conversation (keep current code, replay from earlier context)
- Just the code (keep current conversation, restore files)
- Both (full time-travel)

This is particularly valuable for exploratory coding where the user wants to try approach A,
then rewind and try approach B, comparing results.

### LangGraph's Checkpointing System

LangGraph implements the most formalized checkpointing, drawing on its graph-based execution model:

```python
# LangGraph checkpoint configuration
from langgraph.checkpoint.sqlite import SqliteSaver

# Automatic checkpointing at every graph node
with SqliteSaver.from_conn_string("checkpoints.db") as saver:
    graph = builder.compile(checkpointer=saver)

    # Run the graph
    config = {"configurable": {"thread_id": "session-1"}}
    result = graph.invoke(input_state, config)

    # Time-travel: get state at any checkpoint
    history = list(graph.get_state_history(config))
    step_5_state = history[5]

    # Rollback and replay from step 5 with modifications
    graph.update_state(config, {"messages": modified_messages}, as_node="agent")
    result = graph.invoke(None, config)  # Continues from modified state
```

**Checkpoint storage backends**:

| Backend | Use Case | Durability | Performance |
|---------|----------|------------|-------------|
| MemorySaver | Testing, prototyping | None (in-memory) | Fastest |
| SqliteSaver | Local development | File-based | Fast |
| PostgresSaver | Production | Full ACID | Good |
| RedisSaver | High-throughput | Configurable | Fastest persistent |

**Human-in-the-loop via checkpoints**: LangGraph uses checkpoints as natural interrupt points.
When a graph node is marked as requiring human approval:

```
Node A ──→ [CHECKPOINT] ──→ Human reviews ──→ [RESUME] ──→ Node B
              │                                    │
              └── State saved to DB                └── State loaded, possibly modified
```

The graph pauses, persists its complete state, and waits. Hours or days later, a human
reviews the state, optionally modifies it, and signals resumption. The graph loads the
checkpoint and continues as if no time passed.

---

## 5. Multi-Turn State: Tracking Progress

Beyond conversation history, agents need to track higher-level progress: what have they
accomplished, what remains, and what knowledge have they accumulated?

### Plan State

**ForgeCode's todo_write**: an explicit shared task list that agents read and update:

```python
# ForgeCode's todo tracking (conceptual)
todos = [
    {"id": "1", "title": "Understand the codebase structure", "status": "done"},
    {"id": "2", "title": "Identify the bug in auth.py",      "status": "done"},
    {"id": "3", "title": "Write fix for session refresh",     "status": "in_progress"},
    {"id": "4", "title": "Add tests for the fix",             "status": "pending"},
    {"id": "5", "title": "Run test suite",                    "status": "pending"},
]
```

This provides **observability** into agent progress. Both the agent and the user can see:
- What's been attempted (and whether it succeeded)
- What the agent plans to do next
- Where the agent is stuck

Without explicit plan state, the only way to understand agent progress is to read through
the entire conversation—impractical for long sessions.

### Accumulated Knowledge

Different agents handle knowledge accumulation differently:

**Capy's strict handoff**:
```
Captain Agent                    Build Agent
─────────────                    ───────────
Analyze task                     
Write specification ────────────→ Receive spec (SOLE input)
                                  Execute plan
                                  Return result
```
The Captain's output (a structured specification) is the Build agent's entire input.
No shared mutable state, no ambient knowledge—everything the Build agent needs must be
explicitly written in the specification.

**Claude Code's auto-memory (CLAUDE.md)**:
```markdown
# CLAUDE.md (auto-updated)
## Project Context
- Python 3.11, FastAPI backend
- Tests: pytest, run with `make test`
- Auth uses JWT with refresh tokens

## Learned Preferences
- User prefers type hints on all functions
- Use black for formatting (line length 100)
- Commit messages: conventional commits style
```

This file persists across sessions, accumulating project-specific knowledge that would
otherwise need to be re-discovered each time.

**SageAgent's pipeline carry-forward**:
```
TaskAnalysis → PlanGeneration → CodeImplementation → TestGeneration
     │              │                   │                   │
     └──────────────┴───────────────────┴───────────────────┘
                    Output of each stage feeds into next
```

Each pipeline stage produces structured output that's consumed by the next stage,
with earlier outputs available to later stages for reference.

### Session Metadata

Every agent tracks operational metadata alongside conversation state:

```python
@dataclass
class SessionMetadata:
    # Budget tracking
    total_input_tokens: int = 0
    total_output_tokens: int = 0
    estimated_cost_usd: float = 0.0

    # Timeout detection
    session_start: datetime
    last_activity: datetime
    idle_timeout_seconds: int = 300

    # Limit checking
    iteration_count: int = 0
    max_iterations: int = 100

    # Model routing
    active_model: str = "claude-sonnet-4-20250514"
    fallback_model: str = "claude-haiku"
    model_switch_count: int = 0
```

This metadata is often excluded from the LLM context (the model doesn't need to know its
own token count) but is critical for the orchestration layer's decisions about when to
compact, when to switch models, and when to terminate.

---

## 6. Multi-Agent Shared State

When multiple agents collaborate, state management becomes a distributed systems problem:
how do agents share information without corrupting each other's context?

### ForgeCode: Shared Context with Agent Switching

```
┌──────────┐     ┌──────────┐     ┌──────────┐
│   Muse   │ ←─→ │  Forge   │ ←─→ │   Sage   │
│ (design) │     │ (build)  │     │ (review) │
└────┬─────┘     └────┬─────┘     └────┬─────┘
     │                │                │
     └────────────────┴────────────────┘
                      │
              ┌───────┴───────┐
              │  Shared State │
              │  - todo_write │
              │  - context    │
              └───────────────┘
```

- Context preserved across agent switches (Muse → Forge → Sage)
- Each agent's internal working context is bounded (prevents any single agent from exhausting the window)
- `todo_write` serves as the explicit shared state interface—agents communicate through task status updates

### Ante: Independent Contexts

```
                  ┌─────────────┐
                  │ Meta-Agent  │
                  │ (orchestr.) │
                  └──────┬──────┘
                         │ fan-out
            ┌────────────┼────────────┐
            ▼            ▼            ▼
     ┌────────────┐ ┌────────────┐ ┌────────────┐
     │ Sub-Agent₁ │ │ Sub-Agent₂ │ │ Sub-Agent₃ │
     │ (own ctx)  │ │ (own ctx)  │ │ (own ctx)  │
     └─────┬──────┘ └─────┬──────┘ └─────┬──────┘
           │               │               │
           └───────────────┴───────────────┘
                           │ fan-in
                    ┌──────┴──────┐
                    │ Meta-Agent  │
                    │ (aggregate) │
                    └─────────────┘
```

- Independent contexts per sub-agent: each has its own message history
- Share results through meta-agent's fan-in: only the final output flows back
- Prevents context exhaustion: no single agent sees all the work
- Trade-off: sub-agents can't see each other's intermediate work

### Capy: Strict Isolation

```
Captain ──spec──→ Build
   │                │
   │ No shared      │ No communication
   │ mutable state  │ mid-execution
   │                │
   └────────────────┘
         Boundary
```

- The Captain's specification is the **sole interface** to the Build agent
- No shared mutable state between agents
- No mid-execution communication: Build cannot ask Captain for clarification
- Maximum isolation, maximum predictability
- Cost: the specification must be complete upfront (no iterative refinement between agents)

### OpenHands: NestedEventStore

```python
# OpenHands delegation via filtered event view
class NestedEventStore:
    """A filtered view of a parent EventStream for a child agent."""

    def __init__(self, parent_stream: EventStream, filter_fn):
        self.parent = parent_stream
        self.filter_fn = filter_fn
        self.child_events: list[Event] = []

    def get_events(self) -> list[Event]:
        # Child sees: filtered parent events + own events
        parent_relevant = [e for e in self.parent if self.filter_fn(e)]
        return parent_relevant + self.child_events

    def add_event(self, event: Event):
        self.child_events.append(event)
        # Also flows back to parent stream
        self.parent.add_event(event, source="child")
```

- Child agents receive a **filtered view** of the parent's event stream
- Child isolation: they only see events relevant to their delegated task
- Results flow back: child events are appended to the parent stream
- Parent maintains complete history; child maintains bounded context

---

## 7. Context Compaction Strategies

All compaction strategies face the same fundamental tension: **reducing token count** vs.
**preserving information fidelity**. Every compaction is lossy; the question is whether the
lost information matters for future steps.

### Summarization (Goose, OpenCode)

The most intuitive approach: ask an LLM to summarize old messages.

```python
def summarize_context(messages: list[Message], threshold: float = 0.80) -> list[Message]:
    total_tokens = sum(m.token_count for m in messages)
    limit = MODEL_CONTEXT_LIMIT * threshold

    if total_tokens <= limit:
        return messages  # No compaction needed

    # Split into old (to summarize) and recent (to keep)
    split_point = find_split_point(messages, target_keep_tokens=limit * 0.5)
    old_messages = messages[:split_point]
    recent_messages = messages[split_point:]

    summary = llm.generate(
        system="Summarize this conversation history. Preserve: file paths mentioned, "
               "key decisions made, errors encountered, current plan. Be concise.",
        messages=old_messages,
    )

    return [
        Message(role="user", content=f"[Previous conversation summary]\n{summary}"),
        *recent_messages,
    ]
```

**Trigger**: typically at 80% of context window capacity.

**Risks**:
- Losing important details (a file path mentioned 50 messages ago that becomes relevant again)
- Summary quality varies (the summarizing model may miss what matters)
- Cascading summarization (summarizing a summary of a summary) degrades rapidly

### Condensation (OpenHands)

OpenHands treats condensation as a **first-class Action type** in the event stream:

```python
class AgentCondenseAction(Action):
    """Signals that older events should be condensed."""
    pass

class CondensedObservation(Observation):
    """Result of condensing older events into a summary."""
    summary: str
    condensed_event_ids: list[int]  # Which events were condensed
```

The controller applies condensation as part of the normal event processing loop:

```
Events: [E₀, E₁, E₂, ..., E₂₀, CondenseAction, E₂₂, E₂₃, ...]
                                       │
                                       ▼
Projected: [CondensedSummary(E₀..E₂₀), E₂₂, E₂₃, ...]
Original:  [E₀, E₁, E₂, ..., E₂₀, CondenseAction, E₂₂, E₂₃, ...]  ← preserved
```

**Key difference from summarization**: the original events are never destroyed. The condensed
view is a projection; the source events remain in the stream for replay and debugging.

### Remote Compaction (Codex CLI)

Codex CLI uses the model provider's native compaction endpoint:

```rust
// Codex CLI compaction (simplified)
async fn compact_context(&mut self) -> Result<()> {
    let response = self.client
        .post("/v1/responses")
        .json(&json!({
            "model": self.model,
            "input": self.items,
            "instructions": "Summarize the conversation so far, preserving key context.",
            "truncation": {
                "type": "auto",  // Model decides what to keep/trim
            }
        }))
        .send()
        .await?;

    let compacted = response.json::<CompactedResponse>().await?;

    // Preserve removed items as ghost snapshots
    let ghost = GhostSnapshot::from_items(&self.items);
    self.ghost_snapshots.push(ghost);

    self.items = compacted.items;
    Ok(())
}
```

The model provider handles the compaction logic, including intelligent trimming of function
call history. This offloads the complexity but creates a dependency on the provider's API.

### Selective Pruning (Gemini CLI)

Gemini CLI takes a budget-based approach rather than summarization:

```typescript
// Gemini CLI's token budget calculation (conceptual)
interface TokenBudget {
  systemInstruction: number;   // Fixed allocation, cached separately
  conversationHistory: number; // Flexible, trimmed to fit
  toolDefinitions: number;     // Fixed for current tool set
  currentTurn: number;         // Reserved for the current exchange
  outputReserve: number;       // Reserved for model's response
}

function allocateBudget(contextLimit: number): TokenBudget {
  const outputReserve = Math.min(contextLimit * 0.15, 8192);
  const systemInstruction = estimateTokens(systemPrompt);
  const toolDefinitions = estimateTokens(tools);
  const currentTurn = estimateTokens(currentMessages);

  // Everything left goes to conversation history
  const conversationHistory =
    contextLimit - outputReserve - systemInstruction - toolDefinitions - currentTurn;

  return { systemInstruction, conversationHistory, toolDefinitions, currentTurn, outputReserve };
}
```

Conversation history is trimmed from the oldest messages to fit within the allocated budget.
System instructions are cached separately (leveraging Gemini's context caching) to avoid
re-processing on every call.

---

## Git-Like State Management Patterns

Several agents have converged on patterns that mirror git's approach to version control,
applied to conversation state rather than code.

### Branching

**Claude Code's checkpoints function like git commits**:

```
main:     C₀ ──→ C₁ ──→ C₂ ──→ C₃ ──→ C₄  (current)
                          │
branch:                   └──→ C₂' ──→ C₃'  (alternative approach)
```

- Each checkpoint is a snapshot (commit) of conversation + code state
- Decision points become natural branch points
- Users can explore alternative approaches without losing prior work
- Merging branches is not yet supported (and may not be meaningful for conversations)

### Undo/Redo

**Codex CLI's operational model**:

```
items:     [A, B, C, D, E]
                              Op::Undo(n=2)
items:     [A, B, C]         ghost: [D, E]
                              Op::Redo
items:     [A, B, C, D, E]   ghost: []
                              Op::Rollback(to=1)
items:     [A, B]            ghost: []  (rollback clears ghosts—no redo past rollback)
```

- `Undo` removes turns but preserves them as ghost snapshots
- `Redo` restores from ghost snapshots
- `Rollback` is a hard reset—no recovery possible (like `git reset --hard`)

### Persistent Storage

Every serious agent persists state to survive process termination:

| Agent | Format | Location | Recovery |
|-------|--------|----------|----------|
| Codex CLI | JSONL rollouts | `~/.codex/sessions/` | Full replay |
| OpenCode | SQLite database | `~/.opencode/state.db` | Query any message |
| OpenHands | Event stream | Runtime + optional persistence | Event replay |
| Claude Code | Checkpoints | Session storage | Restore any checkpoint |
| Goose | Session files | `~/.goose/sessions/` | Load and continue |

The format choice reflects priorities:
- **JSONL**: simple, appendable, human-readable, streamable
- **SQLite**: queryable, transactional, compact, multi-process safe
- **Custom event logs**: optimized for replay and projection

---

## Comparison Table

| Approach | Context Control | Reproducibility | Multi-Agent | Persistence | Complexity |
|----------|----------------|-----------------|-------------|-------------|------------|
| Linear list | None (grow only) | Perfect | N/A | Trajectory file | Minimal |
| Conversation + metadata | Summarization | Good | Shared DB | Database | Low |
| Event stream | Condensation | Perfect (replay) | NestedEventStore | Append-only log | Medium |
| Structured + checkpoints | Compaction + undo | Good (rollout) | Sub-agent sessions | JSONL rollout | High |
| Multi-agent shared | Per-agent bounded | Partial | Explicit interfaces | Per-session | High |

### When to Use What

```
Task Duration          Recommended Approach
─────────────          ─────────────────────
< 20 steps             Linear list (don't over-engineer)
20-100 steps           Conversation + metadata with summarization
100+ steps             Event sourcing or checkpointing
Multi-agent            Shared state with explicit interfaces
Production system      Full checkpointing with persistence
Research/training      Linear list (perfect trajectories)
```

---

## Design Principles

1. **Start simple**: linear history works for most tasks. Don't add compaction machinery
   until you've measured that context window saturation is actually your bottleneck.

2. **Add compaction only when context window becomes the bottleneck**: premature optimization
   of context management introduces bugs (lost context, stale summaries) with no benefit
   for short tasks.

3. **Persist early**: save state after every step. The cost of re-doing 50 agent steps
   after a crash far exceeds the cost of writing a few KB to disk. SQLite or JSONL—either
   works, just do it before the next LLM call.

4. **Keep originals**: summarize and compact the projected view, but never destroy source
   data. You will need it for debugging, training, auditing, and replay. Disk is cheap;
   lost data is expensive.

5. **Make state observable**: if you can't inspect the state at any point in the agent's
   execution, you can't debug failures. Every agent should support "show me what the model
   saw at step N."

6. **Bound context growth**: every message costs tokens on every future turn. A 1,000-token
   tool result that's never referenced again still costs ~$0.003 per subsequent turn with
   GPT-4o. Over 100 turns, that's $0.30 for one irrelevant message. Multiply by dozens of
   tool results and the waste becomes significant.

7. **Separate the audit trail from the active context**: what the model sees (active context)
   and what you record (audit trail) should be independent. The model needs a concise,
   relevant context. The audit trail needs completeness. Trying to serve both with one
   data structure forces a compromise that satisfies neither.
