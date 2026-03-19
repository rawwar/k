# Event-Driven Loops

## Overview

Event-driven loops replace the simple `while True` pattern with a **formal state machine**
backed by an **append-only event log**. Instead of a tight loop that calls the LLM and
executes the result in-line, the system decouples *decision-making* from *execution* by
routing every interaction through a typed event stream.

The canonical example is **OpenHands** (formerly OpenDevin). Its architecture is built
around four pillars:

| Pillar | Role |
|--------|------|
| **AgentController** | State machine that drives the loop via `_step()` |
| **EventStream** | Append-only bus carrying Actions and Observations |
| **Agent** (e.g. CodeActAgent) | The LLM-powered brain that produces Actions |
| **Runtime** | Sandboxed executor that consumes Actions and emits Observations |

The key insight: **every action the agent wants to take and every result it receives
is a first-class Event**. This turns an opaque agent loop into an observable, replayable,
debuggable system.

```
┌────────────┐     step()      ┌────────────┐
│            │ ──────────────▶ │            │
│  Agent     │   Action        │ Controller │
│  (Brain)   │ ◀────────────── │ (State     │
│            │                 │  Machine)  │
└────────────┘                 └─────┬──────┘
                                     │ publish(Action)
                                     ▼
                              ┌──────────────┐
                              │              │
                              │ EventStream  │
                              │ (append-only │
                              │  log)        │
                              │              │
                              └──────┬───────┘
                    ┌────────────────┼────────────────┐
                    ▼                ▼                 ▼
            ┌──────────┐    ┌──────────────┐   ┌───────────┐
            │ Runtime  │    │  Security    │   │ Logger /  │
            │ (sandbox)│    │  Analyzer    │   │ UI        │
            └────┬─────┘    └──────────────┘   └───────────┘
                 │ publish(Observation)
                 ▼
          ┌──────────────┐
          │ EventStream  │  (observation appended)
          └──────────────┘
```

---

## The Event Stream Concept

The EventStream is the **single source of truth** for everything that has happened
during an agent session. It is an append-only, ordered log of typed events.

### Core Properties

1. **Append-only** — events are never modified or deleted; new events are always
   appended at the end. This makes the log a reliable audit trail.
2. **Typed** — every event is an instance of a concrete class (e.g. `CmdRunAction`,
   `CmdOutputObservation`). This enables type-safe dispatch and serialisation.
3. **Causally linked** — each event carries a `cause` field pointing at the event ID
   that triggered it. An `Observation` always points back to the `Action` that caused it.
4. **Timestamped** — wall-clock time is recorded for profiling and debugging.
5. **Source-tagged** — the `source` field identifies the producer:
   `EventSource.AGENT`, `EventSource.USER`, `EventSource.ENVIRONMENT`.

### Conceptual Model

```python
@dataclass
class Event:
    id: int                     # monotonically increasing
    timestamp: datetime         # when the event was created
    source: EventSource         # AGENT | USER | ENVIRONMENT
    cause: int | None           # id of the event that triggered this
    message: str                # human-readable summary
    # ... subclass-specific fields

class Action(Event):
    """Something the agent (or user) wants to happen."""
    timeout: int = 120          # max seconds for execution

class Observation(Event):
    """The result of an action being executed."""
    content: str                # primary output
```

### Subscribers

The EventStream supports a publish/subscribe model. Multiple independent consumers
react to events as they are appended:

| Subscriber | Reacts to | Purpose |
|------------|-----------|---------|
| `AGENT_CONTROLLER` | Observations | Feed results back to the agent for the next step |
| `RUNTIME` | Actions | Execute actions in the sandbox |
| `SECURITY_ANALYZER` | All events | Audit actions for policy violations |
| `LOGGER / UI` | All events | Display progress, persist to disk |

This fan-out architecture means adding a new consumer (e.g. a cost tracker) requires
**zero changes** to the core loop — just subscribe to the stream.

---

### Event Types

OpenHands defines a rich taxonomy of event types. Every action the agent can take has
a corresponding observation type for the result.

#### Actions (Agent → Environment)

| Action Class | Trigger | Purpose |
|-------------|---------|---------|
| `CmdRunAction` | `execute_bash` tool | Run a bash command in the sandbox |
| `IPythonRunCellAction` | `execute_ipython_cell` tool | Execute Python code in a Jupyter kernel |
| `FileEditAction` | `str_replace_editor` (edit) | Apply a str_replace or insert to a file |
| `FileReadAction` | `str_replace_editor` (view) | Read file contents (whole or range) |
| `BrowseInteractiveAction` | `browser` tool | Execute browser automation commands |
| `AgentFinishAction` | `finish` tool | Signal task completion |
| `AgentDelegateAction` | Internal routing | Spawn a child agent for a sub-task |
| `AgentThinkAction` | `think` tool | Record internal reasoning (no side effects) |
| `MessageAction` | User or agent message | Communicate with the user |
| `CondensationAction` | Memory manager | Compress history to fit context window |
| `MCPAction` | MCP tool call | Invoke an external MCP server tool |

#### Observations (Environment → Agent)

| Observation Class | Source Action | Content |
|-------------------|--------------|---------|
| `CmdOutputObservation` | `CmdRunAction` | stdout + stderr + exit code |
| `IPythonRunCellObservation` | `IPythonRunCellAction` | Cell output / display data |
| `FileEditObservation` | `FileEditAction` | Diff or confirmation of edit |
| `FileReadObservation` | `FileReadAction` | File content (possibly truncated) |
| `BrowserOutputObservation` | `BrowseInteractiveAction` | Accessibility tree / screenshot |
| `MCPObservation` | `MCPAction` | MCP tool result |
| `ErrorObservation` | Any action | Error message when execution fails |
| `AgentDelegateObservation` | `AgentDelegateAction` | Result from child agent |
| `NullObservation` | `AgentThinkAction` | No-op acknowledgement |

#### Example: A Single Round Trip

```python
# 1. Agent decides to run a command
action = CmdRunAction(command="git diff --stat", thought="Check what changed")
# → published to EventStream with id=42, source=AGENT

# 2. Runtime picks up the action from the stream
#    Executes inside sandbox container
#    Publishes result:
obs = CmdOutputObservation(
    content=" src/main.py | 12 ++++++------\n 1 file changed",
    command_id=42,
    exit_code=0,
    command="git diff --stat"
)
# → published to EventStream with id=43, cause=42, source=ENVIRONMENT

# 3. Controller sees the observation, feeds it to the agent on next step()
```

---

## State Machine: AgentState Transitions

The `AgentController` operates as a finite state machine. The current state determines
what happens in each tick of the loop.

### States

```python
class AgentState(str, Enum):
    LOADING = "loading"
    INIT = "init"
    RUNNING = "running"
    AWAITING_USER_INPUT = "awaiting_user_input"
    AWAITING_USER_CONFIRMATION = "awaiting_user_confirmation"
    PAUSED = "paused"
    STOPPED = "stopped"
    FINISHED = "finished"
    REJECTED = "rejected"
    ERROR = "error"
```

### Transition Diagram

```
                    ┌─────────────────────────────────────────┐
                    │                                         │
                    ▼                                         │
              ┌──────────┐                                    │
              │  LOADING  │                                   │
              └─────┬─────┘                                   │
                    │ resources ready                          │
                    ▼                                          │
              ┌──────────┐                                    │
              │   INIT   │                                    │
              └─────┬─────┘                                   │
                    │ controller.start()                       │
                    ▼                                          │
              ┌──────────┐◀──────────────────────────┐        │
              │ RUNNING  │                           │        │
              └──┬──┬──┬─┘                           │        │
                 │  │  │                             │        │
    ┌────────────┘  │  └────────────┐                │        │
    │               │               │                │        │
    ▼               ▼               ▼                │        │
┌────────┐  ┌────────────────┐  ┌────────┐           │        │
│FINISHED│  │AWAITING_USER   │  │ PAUSED │           │        │
│        │  │_INPUT          │  │        │           │        │
└────────┘  └───────┬────────┘  └───┬────┘           │        │
                    │ user responds  │ resume         │        │
                    └────────────────┴────────────────┘        │
                                                               │
              ┌──────────┐    ┌──────────┐                     │
              │  ERROR   │    │ STOPPED  │◀── external cancel ─┘
              └──────────┘    └──────────┘
                  ▲               ▲
                  │               │
                  └───── from RUNNING (budget exceeded,
                         unrecoverable error, iteration limit)
```

### Transition Rules

```python
# Valid transitions enforced by the controller:
_TRANSITIONS: dict[AgentState, list[AgentState]] = {
    AgentState.LOADING:    [AgentState.INIT],
    AgentState.INIT:       [AgentState.RUNNING, AgentState.STOPPED],
    AgentState.RUNNING:    [
        AgentState.AWAITING_USER_INPUT,
        AgentState.AWAITING_USER_CONFIRMATION,
        AgentState.FINISHED,
        AgentState.ERROR,
        AgentState.STOPPED,
        AgentState.PAUSED,
    ],
    AgentState.AWAITING_USER_INPUT:        [AgentState.RUNNING, AgentState.STOPPED],
    AgentState.AWAITING_USER_CONFIRMATION: [AgentState.RUNNING, AgentState.STOPPED,
                                            AgentState.REJECTED],
    AgentState.PAUSED:     [AgentState.RUNNING, AgentState.STOPPED],
    AgentState.FINISHED:   [],  # terminal
    AgentState.ERROR:      [],  # terminal
    AgentState.STOPPED:    [],  # terminal
    AgentState.REJECTED:   [],  # terminal
}
```

Invalid transitions raise `ValueError`, making the state machine **self-enforcing**.

---

## AgentController._step() — The Heartbeat

The `_step()` method is the **single most important method** in the entire system. It
is called repeatedly while the agent is in the `RUNNING` state. Each call is one
"heartbeat" of the agent.

### Phase-by-Phase Breakdown

```python
async def _step(self) -> None:
    """Execute one step of the agent loop."""

    # ──────────────────────────────────────────────────
    # PHASE 1: Pre-step Validation
    # ──────────────────────────────────────────────────
    # Check that we're still in a valid state to step
    if self.state.agent_state != AgentState.RUNNING:
        raise AgentNotRunningError(f"Cannot step in state {self.state.agent_state}")

    # Budget guard: check token usage, iteration count, cost
    if self.state.metrics.accumulated_cost > self.config.max_budget_per_task:
        raise AgentBudgetExceededError(
            f"Budget ${self.state.metrics.accumulated_cost:.2f} "
            f"exceeds limit ${self.config.max_budget_per_task:.2f}"
        )

    if self.state.iteration >= self.config.max_iterations:
        raise AgentIterationLimitError(
            f"Iteration {self.state.iteration} >= limit {self.config.max_iterations}"
        )

    # ──────────────────────────────────────────────────
    # PHASE 2: Invoke the Agent Brain
    # ──────────────────────────────────────────────────
    # This is where the LLM is called (unless pending_actions has items)
    action: Action = self.agent.step(self.state)
    # The agent returns a single Action representing its decision

    # ──────────────────────────────────────────────────
    # PHASE 3: Route by Action Type
    # ──────────────────────────────────────────────────

    # 3a. Condensation — apply directly, don't publish
    if isinstance(action, CondensationAction):
        self.state.history.apply_condensation(action)
        return  # re-step immediately; no event published

    # 3b. Finish — transition to terminal state
    if isinstance(action, AgentFinishAction):
        self.state.outputs = action.outputs
        self.set_agent_state(AgentState.FINISHED)
        self.event_stream.add_event(action, EventSource.AGENT)
        return

    # 3c. Delegation — spawn a child controller
    if isinstance(action, AgentDelegateAction):
        self._start_delegation(action)
        return

    # 3d. User-input request — pause and wait
    if isinstance(action, MessageAction) and action.wait_for_response:
        self.set_agent_state(AgentState.AWAITING_USER_INPUT)
        self.event_stream.add_event(action, EventSource.AGENT)
        return

    # 3e. Normal action — publish to stream for runtime
    self.event_stream.add_event(action, EventSource.AGENT)

    # ──────────────────────────────────────────────────
    # PHASE 4: Post-step Evaluation
    # ──────────────────────────────────────────────────
    # The StuckDetector checks if the agent is in a loop
    self.stuck_detector.check(action, self.state.history)

    # ──────────────────────────────────────────────────
    # PHASE 5: Wait for Observation (async)
    # ──────────────────────────────────────────────────
    # The Runtime picks up the action from the EventStream,
    # executes it in the sandbox, and publishes an Observation.
    # The controller waits for the observation event before
    # calling _step() again.

    self.state.iteration += 1
```

### The Step Lifecycle (Visual)

```
_step() call N
│
├─▶ [1] Pre-checks (budget? iteration limit? state == RUNNING?)
│       │
│       ├── FAIL → raise budget/iteration error → state = STOPPED
│       └── PASS ↓
│
├─▶ [2] action = agent.step(state)
│       │
│       ├── LLM called (if pending_actions empty)
│       └── Action returned from queue (if pending_actions has items)
│
├─▶ [3] Route action
│       │
│       ├── CondensationAction   → apply to history, return (no event)
│       ├── AgentFinishAction    → state = FINISHED, publish, return
│       ├── AgentDelegateAction  → spawn child controller, return
│       ├── MessageAction(wait)  → state = AWAITING_USER_INPUT, return
│       └── Other                → publish to EventStream
│
├─▶ [4] StuckDetector.check(action)
│       │
│       └── stuck? → raise AgentStuckInLoopError
│
└─▶ [5] Runtime executes action → publishes Observation
        │
        └── Controller sees Observation → calls _step() again
```

---

## CodeActAgent.step() — The Agent Brain

The `CodeActAgent` is the default agent implementation. Its `step()` method is the
bridge between the controller's state machine and the LLM.

### Full Flow

```python
def step(self, state: State) -> Action:
    """Produce the next action for the controller to execute."""

    # ──────────────────────────────────────────────────
    # 1. Drain pending actions (parallel tool calls)
    # ──────────────────────────────────────────────────
    if self.pending_actions:
        return self.pending_actions.popleft()

    # ──────────────────────────────────────────────────
    # 2. Check for /exit command from user
    # ──────────────────────────────────────────────────
    latest_user_msg = state.get_last_user_message()
    if latest_user_msg and latest_user_msg.strip() == "/exit":
        return AgentFinishAction()

    # ──────────────────────────────────────────────────
    # 3. History condensation (if needed)
    # ──────────────────────────────────────────────────
    condensation = self.condenser.maybe_condense(state.history)
    if condensation is not None:
        return condensation  # CondensationAction

    # ──────────────────────────────────────────────────
    # 4. Build LLM messages from history
    # ──────────────────────────────────────────────────
    messages: list[Message] = self._build_messages(state)
    #   - System prompt (role, tools, instructions)
    #   - Condensed history (events → messages)
    #   - Most recent observation

    # ──────────────────────────────────────────────────
    # 5. Call the LLM
    # ──────────────────────────────────────────────────
    response = self.llm.completion(
        messages=messages,
        tools=self.tools,              # function schemas
        stop=self.action_parser.stop_tokens,
    )

    # ──────────────────────────────────────────────────
    # 6. Parse response into Action(s)
    # ──────────────────────────────────────────────────
    actions: list[Action] = self.action_parser.parse(response)

    # ──────────────────────────────────────────────────
    # 7. Queue and return
    # ──────────────────────────────────────────────────
    for action in actions:
        self.pending_actions.append(action)

    return self.pending_actions.popleft()
```

### The pending_actions Queue

Modern LLMs support **parallel tool calling** — a single response can contain multiple
tool calls. OpenHands handles this with a `deque`:

```
LLM response contains 3 tool calls:
  tool_call_1: execute_bash("make build")
  tool_call_2: execute_bash("make test")
  tool_call_3: str_replace_editor(view, "README.md")

Parser produces: [CmdRunAction, CmdRunAction, FileReadAction]

pending_actions = deque([action_1, action_2, action_3])

Timeline:
  step() call 1 → pending_actions not empty → return action_1 (popleft)
     ↓ controller publishes action_1, runtime executes, observation returned
  step() call 2 → pending_actions not empty → return action_2 (no LLM call!)
     ↓ controller publishes action_2, runtime executes, observation returned
  step() call 3 → pending_actions not empty → return action_3 (no LLM call!)
     ↓ controller publishes action_3, runtime executes, observation returned
  step() call 4 → pending_actions empty → full LLM call → new actions
```

This is elegant: **N tool calls require only 1 LLM invocation** but still go through
the full event-stream lifecycle individually. Each action gets its own observation,
each is visible in the event log, and each passes through the security analyzer.

---

## Function Calling Resolution

The `FunctionCallingActionParser` maps LLM tool call names to OpenHands Action classes.
This is the bridge between the LLM's function-calling API and the typed event system.

### Mapping Table

```python
TOOL_NAME_TO_ACTION = {
    "execute_bash":          CmdRunAction,
    "execute_ipython_cell":  IPythonRunCellAction,
    "str_replace_editor":    _resolve_editor_action,  # polymorphic
    "browser":               BrowseInteractiveAction,
    "finish":                AgentFinishAction,
    "think":                 AgentThinkAction,
}
```

### The Polymorphic Editor Tool

The `str_replace_editor` tool is special — it maps to **different Action types**
depending on the `command` argument:

```python
def _resolve_editor_action(tool_call: dict) -> Action:
    command = tool_call["arguments"]["command"]

    if command == "view":
        return FileReadAction(
            path=tool_call["arguments"]["path"],
            start=tool_call["arguments"].get("view_range", [1])[0],
            end=tool_call["arguments"].get("view_range", [None])[1],
        )
    elif command == "str_replace":
        return FileEditAction(
            path=tool_call["arguments"]["path"],
            old_str=tool_call["arguments"]["old_str"],
            new_str=tool_call["arguments"].get("new_str", ""),
        )
    elif command == "insert":
        return FileEditAction(
            path=tool_call["arguments"]["path"],
            new_str=tool_call["arguments"]["new_str"],
            insert_line=tool_call["arguments"]["insert_line"],
        )
    elif command == "create":
        return FileEditAction(
            path=tool_call["arguments"]["path"],
            new_str=tool_call["arguments"]["file_text"],
            is_create=True,
        )
```

This means one LLM tool definition fans out to multiple internal action types —
keeping the LLM's tool surface simple while the internal system remains precise.

---

## Runtime: Event Processing and Sandbox Execution

The Runtime is the **execution engine**. It subscribes to the EventStream, picks up
Actions, executes them in an isolated sandbox, and publishes Observations back.

### Architecture

```
Host Machine                          Docker Container (Sandbox)
┌─────────────────────┐              ┌──────────────────────────┐
│                     │              │                          │
│  ActionExecution    │  HTTP POST   │   ActionExecution        │
│  Client             │ ───────────▶ │   Server                 │
│                     │              │                          │
│  - subscribes to    │              │   - bash shell           │
│    EventStream      │  HTTP resp   │   - IPython kernel       │
│  - sends actions    │ ◀─────────── │   - file system          │
│    to sandbox       │              │   - browser (Playwright) │
│  - publishes obs    │              │                          │
│    to EventStream   │              │                          │
└─────────────────────┘              └──────────────────────────┘
```

### Dispatch by Action Type

```python
class ActionExecutionServer:
    """Runs inside the sandbox container."""

    async def handle_action(self, action: Action) -> Observation:
        if isinstance(action, CmdRunAction):
            return await self._run_bash(action.command, timeout=action.timeout)

        elif isinstance(action, IPythonRunCellAction):
            return await self._run_ipython(action.code)

        elif isinstance(action, FileEditAction):
            return await self._edit_file(action)

        elif isinstance(action, FileReadAction):
            return await self._read_file(action.path, action.start, action.end)

        elif isinstance(action, BrowseInteractiveAction):
            return await self._browser_action(action)

        elif isinstance(action, MCPAction):
            return await self._call_mcp_tool(action)

        else:
            return ErrorObservation(f"Unknown action type: {type(action)}")
```

### Isolation Guarantees

| Resource | Isolation Method |
|----------|-----------------|
| File system | Docker volume mount (workspace only) |
| Bash | Separate shell process per session |
| Python | Jupyter kernel with resource limits |
| Network | Configurable network policies |
| Browser | Headless Chromium in container |
| Secrets | Environment variables injected at container start |

---

## StuckDetector — 4 Strategies

The `StuckDetector` is a post-step hook that analyses recent history to detect when
the agent has fallen into an unproductive loop.

### Strategy 1: Identical Repetition

```
action[n] == action[n-1] == action[n-2]

Example:
  step 10: CmdRunAction("cat /etc/passwd")
  step 11: CmdRunAction("cat /etc/passwd")   ← same
  step 12: CmdRunAction("cat /etc/passwd")   ← same → STUCK
```

The detector compares the **full action content** (not just the type).

### Strategy 2: Alternating Pattern (ABAB)

```
action[n] == action[n-2]  AND  action[n-1] == action[n-3]

Example:
  step 10: FileEditAction(old="foo", new="bar")
  step 11: FileEditAction(old="bar", new="foo")   ← undoes step 10
  step 12: FileEditAction(old="foo", new="bar")   ← same as step 10
  step 13: FileEditAction(old="bar", new="foo")   ← same as step 11 → STUCK
```

This catches the common pattern where the agent edits a file, sees an error,
reverts, and repeats.

### Strategy 3: Error Loop

```
last K observations are ALL ErrorObservation

Example:
  obs 8:  ErrorObservation("Permission denied")
  obs 9:  ErrorObservation("Permission denied")
  obs 10: ErrorObservation("Permission denied")
  → STUCK (agent keeps trying something that always fails)
```

### Strategy 4: Empty Response

```
last K actions have empty/null content

Example:
  step 15: AgentThinkAction(thought="")
  step 16: AgentThinkAction(thought="")
  step 17: AgentThinkAction(thought="")
  → STUCK (LLM returning empty responses)
```

### Recovery Mechanisms

When the StuckDetector fires, recovery depends on configuration:

```python
def handle_stuck(self, state: State, strategy: str) -> None:
    if self.config.stuck_recovery == "condensation":
        # Force history compression — new context may break the loop
        condensation = self.condenser.force_condense(state.history)
        state.history.apply_condensation(condensation)

    elif self.config.stuck_recovery == "injection":
        # Inject a hint: "You seem to be repeating yourself..."
        hint = ErrorObservation(
            "You appear to be stuck in a loop. "
            "Try a completely different approach."
        )
        state.history.append(hint)

    elif self.config.stuck_recovery == "terminate":
        raise AgentStuckInLoopError(
            f"Agent stuck: {strategy} detected after "
            f"{self.config.stuck_threshold} repetitions"
        )
```

---

## Agent Delegation

OpenHands supports **hierarchical agents** through delegation. A parent agent can
spawn a child agent to handle a sub-task, then resume when the child finishes.

### How It Works

```
Parent Controller                    Child Controller
     │                                    │
     │ AgentDelegateAction               │
     │ (agent="BrowsingAgent",           │
     │  inputs={"task": "..."})          │
     │                                    │
     ├──▶ _start_delegation() ───────────▶│ INIT
     │                                    │
     │    [parent PAUSED]                 │ RUNNING
     │                                    │   ├── step()
     │                                    │   ├── action → obs
     │                                    │   ├── step()
     │                                    │   └── AgentFinishAction
     │                                    │
     │    AgentDelegateObservation ◀──────│ FINISHED
     │    (outputs={"result": "..."})     │
     │                                    │
     │ RUNNING (resumed)                  │ [destroyed]
     │
```

### NestedEventStore

The child agent does **not** see the parent's full event history. Instead, it operates
on a `NestedEventStore` — a filtered, isolated view:

```python
class NestedEventStore:
    """A virtual event store scoped to a delegation session."""

    def __init__(self, parent_stream: EventStream, start_id: int):
        self.parent = parent_stream
        self.start_id = start_id  # only see events after this point

    def get_events(self) -> list[Event]:
        # Only return events from start_id onwards
        # Filter to only events relevant to this child session
        return [
            e for e in self.parent.get_events()
            if e.id >= self.start_id
            and e.source in (EventSource.AGENT, EventSource.ENVIRONMENT)
        ]
```

This ensures:
- **Clean context**: child agent isn't confused by parent's history
- **Result flow**: child's `AgentFinishAction.outputs` flow back as
  `AgentDelegateObservation.outputs` to the parent
- **Event persistence**: all child events are still in the parent's EventStream
  (they're just filtered out of the child's view)

---

## Error Handling Hierarchy

OpenHands implements a comprehensive error handling strategy where different exception
types trigger different recovery behaviours:

| Exception | Where Caught | Handler | Result |
|-----------|-------------|---------|--------|
| `ContextWindowExceededError` | `agent.step()` | Trigger condensation | Retry with compressed history |
| `RateLimitError` | `llm.completion()` | Exponential backoff | Retry after 1s, 2s, 4s, ... |
| `FunctionCallValidationError` | `action_parser.parse()` | Convert to ErrorObservation | Agent sees error, self-corrects |
| `AgentStuckInLoopError` | `stuck_detector.check()` | Recovery action or terminate | Break repetition pattern |
| `AgentBudgetExceededError` | `_step()` pre-check | Set state = STOPPED | Graceful termination |
| `AgentIterationLimitError` | `_step()` pre-check | Set state = STOPPED | Graceful termination |
| `SandboxTimeoutError` | Runtime execution | Convert to ErrorObservation | Agent can retry command |
| `SandboxCrashError` | Runtime execution | Restart sandbox + ErrorObservation | Agent retries in fresh env |

### Error Flow Example

```python
# Context window exceeded during LLM call:
try:
    response = self.llm.completion(messages=messages, tools=tools)
except ContextWindowExceededError:
    # Force condensation — compress history
    condensation = self.condenser.force_condense(state.history)
    return condensation  # CondensationAction will be applied by controller

# On next step(), history is shorter → LLM call succeeds
```

```python
# Function call validation failure:
try:
    actions = self.action_parser.parse(response)
except FunctionCallValidationError as e:
    # Convert to observation so the agent sees what went wrong
    error_obs = ErrorObservation(
        f"Your tool call was malformed: {e}. "
        f"Please fix the arguments and try again."
    )
    self.event_stream.add_event(error_obs, EventSource.ENVIRONMENT)
    # Agent will see this error on next step and correct itself
```

---

## Advantages of Event-Driven Architecture

### 1. Observability

Every action and observation is persisted in the EventStream. You can query:
- What commands did the agent run?
- How many LLM calls were made?
- What errors occurred?
- What was the agent's reasoning at step N?

```python
# Find all commands the agent ran
commands = [
    e for e in event_stream.get_events()
    if isinstance(e, CmdRunAction)
]

# Find all errors
errors = [
    e for e in event_stream.get_events()
    if isinstance(e, ErrorObservation)
]
```

### 2. Testability

Replay an event stream to reproduce exact agent behaviour:

```python
def test_agent_handles_permission_error():
    # Create a stream with known events
    stream = EventStream()
    stream.add(CmdRunAction("rm -rf /protected"))
    stream.add(ErrorObservation("Permission denied"))

    # Step the agent and verify it tries a different approach
    action = agent.step(State(history=stream))
    assert not isinstance(action, CmdRunAction) or "sudo" not in action.command
```

### 3. Time-Travel Debugging

Rewind to any point in the stream:

```python
def replay_to_step(stream: EventStream, step: int) -> State:
    """Reconstruct state at a specific step."""
    events = stream.get_events()[:step]
    state = State()
    for event in events:
        state.apply(event)
    return state
```

### 4. Decoupled Components

The agent, controller, runtime, and UI are **completely independent**:
- Agent only knows about `State → Action`
- Controller only knows about state transitions
- Runtime only knows about `Action → Observation`
- UI only knows about reading events from the stream

Change any one component without touching the others.

### 5. Security

The `SecurityAnalyzer` subscribes to the EventStream independently:

```python
class SecurityAnalyzer:
    def on_event(self, event: Event) -> None:
        if isinstance(event, CmdRunAction):
            if self._is_dangerous(event.command):
                # Block execution by injecting rejection
                self.stream.add(ErrorObservation(
                    "Command blocked by security policy"
                ))
```

### 6. Extensibility

Adding a new capability requires:
1. Define new `Action` and `Observation` subclasses
2. Add handler in the Runtime
3. Add tool definition for the LLM

**No changes to AgentController, EventStream, or existing handlers.**

---

## Comparison with Traditional Loops

| Aspect | Event-Driven (OpenHands) | Simple Loop (mini-SWE-agent) |
|--------|--------------------------|------------------------------|
| **Architecture** | State machine + event bus | `while True` with inline execution |
| **Action representation** | Typed `Action` objects | Raw text parsed with regex |
| **Observation flow** | Published to EventStream | Returned directly from function |
| **State management** | Explicit `AgentState` enum | Implicit (loop running = active) |
| **History** | Append-only EventStream | List of message dicts |
| **Parallel tool calls** | `pending_actions` queue | Not supported (serial only) |
| **Error recovery** | Per-exception-type handlers | Generic try/except |
| **Stuck detection** | 4-strategy StuckDetector | None (relies on iteration limit) |
| **Agent delegation** | Built-in with NestedEventStore | Not supported |
| **Context management** | CondensationAction (first-class) | Manual truncation |
| **Security** | SecurityAnalyzer subscriber | None |
| **Observability** | Full event replay | Print statements / logs |
| **Sandbox isolation** | Docker containers | Same-process execution |
| **Testing** | Replay event streams | Mock LLM responses |
| **Complexity** | High (~thousands of lines) | Low (~100-300 lines) |
| **Setup overhead** | Docker, servers, config | Single Python file |
| **Debugging** | Time-travel via event log | Step through with debugger |
| **Latency per step** | Higher (HTTP, serialisation) | Lower (in-process calls) |

### When to Use Which

**Simple loops** are ideal when:
- You're prototyping or learning
- The task is well-scoped (e.g., single-file edits)
- You don't need delegation, security, or replay
- Deployment simplicity matters more than observability

**Event-driven loops** are ideal when:
- You need production-grade reliability
- Multiple agents must collaborate (delegation)
- Security auditing is required
- You want full replay/debugging capabilities
- The system must be extensible by multiple teams

---

## Key Design Decisions

### 1. Event-Sourced Architecture

OpenHands chose **event sourcing** as its foundation: the EventStream is the single
source of truth, and all state is derived from replaying events.

*Trade-off*: Higher storage and complexity, but you get full replay, audit trails,
and the ability to reconstruct state at any point.

### 2. Agent Returns Action, Not Text

The agent's `step()` method returns a typed `Action` object, not raw text. This means:
- No regex parsing of LLM output
- Type-safe routing in the controller
- Clean serialisation for the event stream
- Static analysis can verify action handling is exhaustive

### 3. pending_actions Queue for Parallel Tool Calls

Rather than executing all tool calls at once, OpenHands queues them and processes
one per `_step()` call. This means each action:
- Gets its own observation
- Passes through the security analyzer
- Is visible as a separate event
- Can be individually cancelled or timed out

*Trade-off*: Slightly slower than parallel execution, but dramatically better
observability and safety.

### 4. Condensation as a First-Class Action

History compression is not a hidden side-effect — it's a `CondensationAction` that
flows through the same system as every other action. This means:
- The controller can route it specially (apply without publishing)
- It's visible in the event log
- The agent can trigger it explicitly
- The memory manager can trigger it automatically

### 5. NestedEventStore for Delegation

Child agents see a filtered view of the parent's event stream rather than getting
their own separate stream. This means:
- All events are in one place (parent stream)
- Debugging delegation is straightforward
- No cross-stream synchronisation needed
- Results flow back naturally

### 6. StuckDetector as a Post-Step Hook

The stuck detector runs *after* every step rather than being embedded in the agent.
This means:
- The agent code stays clean (separation of concerns)
- Detection logic can be updated independently
- Different agents can share the same detector
- Detection strategies are composable and configurable

---

## Summary: The Event-Driven Loop in 30 Seconds

```
┌─────────────────────────────────────────────────────┐
│                   AgentController                    │
│                                                     │
│  while state == RUNNING:                            │
│      action = agent.step(state)      # LLM call    │
│      if action is CondensationAction:               │
│          apply_condensation(action)                  │
│          continue                                   │
│      if action is AgentFinishAction:                │
│          state = FINISHED                           │
│          break                                      │
│      if action is AgentDelegateAction:              │
│          spawn_child(action)                        │
│          continue                                   │
│      event_stream.publish(action)    # → Runtime    │
│      stuck_detector.check(action)                   │
│      observation = await_observation()              │
│      event_stream.publish(observation)              │
│      # loop continues...                            │
└─────────────────────────────────────────────────────┘
```

The event-driven loop transforms an agent from an opaque black box into a
**transparent, auditable, composable system** — at the cost of additional
architectural complexity. For production systems handling real-world tasks
with security requirements, observability needs, and multi-agent coordination,
this complexity pays for itself many times over.