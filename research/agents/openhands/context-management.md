# OpenHands Context Management

## Overview

OpenHands implements a sophisticated multi-layer context management system that separates
the concerns of event persistence, view construction, message formatting, and intelligent
condensation. This architecture allows the agent to operate on long-running tasks that
far exceed any single LLM context window, while maintaining coherent understanding of
the full task history.

The core insight: rather than treating context as a monolithic buffer, OpenHands
decomposes it into six distinct layers, each with a clear responsibility.

```
┌─────────────────────────────────────────────────────────┐
│                    LLM Context Window                   │
│  (System prompt + condensed messages + tool responses)  │
└────────────────────────┬────────────────────────────────┘
                         │ Message[]
┌────────────────────────┴────────────────────────────────┐
│          Layer 3: ConversationMemory                    │
│   (Event → Message conversion, role alternation,        │
│    tool call pairing, prompt caching)                   │
└────────────────────────┬────────────────────────────────┘
                         │ View (condensed events)
┌────────────────────────┴────────────────────────────────┐
│          Layer 4: Condenser System                      │
│   (10+ strategies: sliding window, LLM summarization,   │
│    observation masking, pipeline chaining)               │
└────────────────────────┬────────────────────────────────┘
                         │ View (full or filtered)
┌────────────────────────┴────────────────────────────────┐
│          Layer 2: State.view                            │
│   (Condensed subset of events the agent sees,           │
│    tracks forgotten_event_ids)                          │
└────────────────────────┬────────────────────────────────┘
                         │ all events
┌────────────────────────┴────────────────────────────────┐
│          Layer 1: EventStream                           │
│   (Complete audit trail, persisted to FileStore,        │
│    JSON with pagination/caching, never truncated)       │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│          Layer 5: Memory                                │
│   (Workspace context, microagent knowledge,             │
│    repo/runtime info via RecallAction)                  │
├─────────────────────────────────────────────────────────┤
│          Layer 6: Microagent Knowledge Injection        │
│   (Keyword-triggered skills, repo-specific guidance,    │
│    task commands)                                       │
└─────────────────────────────────────────────────────────┘
```

---

## Layer 1: EventStream — The Immutable History

The EventStream is the foundational data structure. Every action the agent takes and
every observation it receives is appended to this stream.

**Key characteristics:**

- **Append-only**: Events are never deleted or modified once written
- **Persisted**: Backed by a FileStore, serialized as JSON
- **Paginated**: Supports efficient retrieval of event ranges
- **Cached**: Recently accessed events are cached in memory
- **Subscriber model**: Components subscribe to the stream for real-time event notification

```python
class EventStream:
    def add_event(self, event: Event, source: EventSource) -> None:
        """Append an event, assign monotonic ID, notify subscribers."""

    def get_events(self, start_id=0, end_id=None, reverse=False,
                   filter_out_type=None) -> Iterable[Event]:
        """Retrieve events within a range with optional type filtering."""

    def subscribe(self, subscriber: EventStreamSubscriber, id: str) -> None:
        """Register a subscriber for real-time event notification."""
```

Events carry a monotonic `id` (their position in the stream), a `source` (agent,
user, environment), a `timestamp`, and a `cause` field linking observations to the
action that triggered them.

This layer is the **source of truth**. Even when the agent's view is condensed or
summarized, the full history remains available for debugging, replay, and analysis.

---

## Layer 2: State.view — The Agent's Window

The `State` object holds runtime state for the agent, including the `view` — the
subset of events the agent currently "sees."

```python
@dataclass
class State:
    view: View                    # Condensed event list
    history: EventStream          # Full event stream (reference)
    extra_data: dict              # Condenser metadata, etc.
    # ... other fields
```

The `View` is the critical abstraction:

```python
@dataclass
class View:
    events: list[Event]           # Events the agent can see
    forgotten_event_ids: set[int] # IDs of events removed from view
```

The `forgotten_event_ids` set enables condensers to track what has been removed,
which is essential for summary-based condensers that need to know what was previously
visible. When a `CondensationAction` is applied by the controller, it updates the
view by removing the specified events and recording their IDs.

---

## Layer 3: ConversationMemory — Message Construction

The `ConversationMemory` class (~42KB in `openhands/memory/conversation_memory.py`)
bridges the gap between the event-oriented internal model and the message-oriented
LLM API.

### Responsibilities

1. **Event-to-message conversion**: Each event type maps to a specific message format
2. **Role alternation**: Ensures valid user/assistant turn-taking for the LLM
3. **Tool call pairing**: Matches tool call actions with their corresponding observations
4. **Content truncation**: Respects `max_message_chars` to prevent oversized messages
5. **Prompt caching**: Applies Anthropic-specific cache breakpoints for efficiency
6. **Vision support**: Handles image content in observations (screenshots, etc.)

### Message Construction Flow

```python
class ConversationMemory:
    def process_events(self, condensed_events: list[Event]) -> list[Message]:
        """Convert events to LLM messages."""
        messages = []
        for event in condensed_events:
            if isinstance(event, SystemMessageAction):
                # → system message (first position)
            elif isinstance(event, MessageAction):
                if event.source == EventSource.USER:
                    # → user message
                else:
                    # → assistant message
            elif isinstance(event, Action) and event.tool_call_metadata:
                # → assistant message with tool_calls field
            elif isinstance(event, Observation):
                if event.tool_call_metadata:
                    # → tool response message (role=tool)
                else:
                    # → user message with observation content
            messages.append(msg)
        return messages
```

### Prompt Caching Strategy

For Anthropic models, `ConversationMemory` applies prompt caching by marking
specific messages with cache control breakpoints. This reduces cost and latency
for repeated prefixes in the conversation:

```python
def apply_prompt_caching(self, messages: list[Message]) -> list[Message]:
    """Mark messages for Anthropic prompt caching.

    Strategy: Cache the system message and recent conversation turns
    to maximize cache hits across consecutive steps.
    """
```

---

## Layer 4: The Condenser System

The condenser system (`openhands/memory/condenser/`) is the most architecturally
rich component. It provides a pluggable framework for reducing event history to
fit within LLM context limits.

### Base Abstractions

```python
class Condenser(ABC):
    @abstractmethod
    def condense(self, view: View) -> View | Condensation:
        """Reduce events to a smaller set.

        Returns:
            View: A condensed view the agent can use directly.
            Condensation: An action to be applied by the controller,
                          triggering a re-step after application.
        """

    def condensed_history(self, state: State) -> View | Condensation:
        """Main entry point — condense the state's current view."""
        return self.condense(state.view)
```

The dual return type is key to the architecture:

- **View** — The condenser directly produces a reduced event list. The agent
  proceeds immediately with this view.
- **Condensation** — The condenser produces a `CondensationAction` that the
  controller must apply to the state. After application, the agent re-steps
  with the updated view. This enables condensers that modify state.

### RollingCondenser — Threshold-Based Base

```python
class RollingCondenser(Condenser):
    """Base for condensers that trigger when a threshold is exceeded."""

    @abstractmethod
    def should_condense(self, view: View) -> bool:
        """Check if condensation is needed (e.g., event count > threshold)."""

    @abstractmethod
    def get_condensation(self, view: View) -> Condensation:
        """Produce a condensation action."""

    def condense(self, view: View) -> View | Condensation:
        if self.should_condense(view):
            return self.get_condensation(view)
        return view
```

### CondensationAction

When a condenser determines that history should be modified, it returns a
`CondensationAction`:

```python
@dataclass
class CondensationAction(Action):
    forgotten_event_ids: list[int] | None = None
    forgotten_events_start_id: int | None = None
    forgotten_events_end_id: int | None = None
    summary: str | None = None
    summary_offset: int | None = None
```

This action supports:
- **Explicit forgetting**: List specific event IDs to remove
- **Range forgetting**: Remove all events in an ID range
- **Summarization**: Attach a text summary that replaces forgotten events
- **Summary offset**: Control where the summary is inserted in the view

### Condenser Implementations

| Condenser | Strategy | Triggers | Preserves | Cost |
|-----------|----------|----------|-----------|------|
| **NoOpCondenser** | Pass-through | Never | Everything | None |
| **RecentEventsCondenser** | Keep last N events | Always | Recent events only | None |
| **ConversationWindowCondenser** | Sliding window | Event count > max | First K + last N events | None |
| **AmortizedForgettingCondenser** | Gradual forgetting | Threshold exceeded | Progressively recent events | None |
| **ObservationMaskingCondenser** | Content replacement | Always | Event structure, hides content | None |
| **BrowserOutputCondenser** | Browser-specific masking | Browser observations | Non-browser events | None |
| **LLMAttentionCondenser** | LLM selects events to keep | Threshold exceeded | LLM-chosen events | LLM call |
| **LLMSummarizingCondenser** | LLM summarizes forgotten events | Threshold exceeded | Summary + recent | LLM call |
| **StructuredSummaryCondenser** | Structured sections summary | Threshold exceeded | Categorized summary | LLM call |
| **PipelineCondenser** | Chain multiple condensers | Per-stage | Composition of strategies | Varies |

#### Selected Implementation Details

**RecentEventsCondenser** — The simplest non-trivial condenser:

```python
class RecentEventsCondenser(Condenser):
    keep_last: int = 50

    def condense(self, view: View) -> View:
        if len(view.events) <= self.keep_last:
            return view
        forgotten = {e.id for e in view.events[:-self.keep_last]}
        return View(
            events=view.events[-self.keep_last:],
            forgotten_event_ids=view.forgotten_event_ids | forgotten,
        )
```

**ConversationWindowCondenser** — Sliding window preserving initial context:

```python
class ConversationWindowCondenser(RollingCondenser):
    max_events: int = 100
    keep_first: int = 5   # Preserve initial setup/instructions

    def should_condense(self, view: View) -> bool:
        return len(view.events) > self.max_events

    def get_condensation(self, view: View) -> Condensation:
        # Keep first K events + last (max - K) events
        # Forget everything in between
        ...
```

**LLMSummarizingCondenser** — Uses an LLM to create a text summary of events
that are about to be forgotten. The summary is injected as a synthetic
`MessageAction` at the beginning of the view, preserving contextual awareness:

```python
class LLMSummarizingCondenser(RollingCondenser):
    llm: LLM
    max_events: int = 100

    def get_condensation(self, view: View) -> Condensation:
        events_to_forget = view.events[:-self.keep_recent]
        summary = self.llm.completion(
            messages=[{
                "role": "user",
                "content": f"Summarize these events:\n{format_events(events_to_forget)}"
            }]
        )
        return CondensationAction(
            forgotten_events_start_id=events_to_forget[0].id,
            forgotten_events_end_id=events_to_forget[-1].id,
            summary=summary,
        )
```

**StructuredSummaryCondenser** — Creates categorized summaries with sections like
"Key Findings", "Files Modified", "Current Approach", "Open Issues". This gives
the agent a more organized view of its past work than a free-form summary.

**PipelineCondenser** — Composes multiple condensers in sequence:

```python
class PipelineCondenser(Condenser):
    condensers: list[Condenser]

    def condense(self, view: View) -> View | Condensation:
        for condenser in self.condensers:
            result = condenser.condense(view)
            if isinstance(result, Condensation):
                return result  # Short-circuit: controller must apply first
            view = result
        return view
```

This enables sophisticated strategies like: "First mask observation content,
then apply a sliding window, then summarize if still too large."

---

## Layer 5: Memory — External Knowledge

The `Memory` class (`openhands/memory/memory.py`) is an EventStream subscriber that
responds to `RecallAction` events by injecting contextual knowledge.

### RecallAction Types

```python
class RecallType(Enum):
    WORKSPACE_CONTEXT = "workspace_context"  # Repo info, runtime, microagents
    KNOWLEDGE = "knowledge"                  # Triggered microagent content
```

### Knowledge Sources

| Source | Content | When Loaded |
|--------|---------|-------------|
| Repository info | File tree, README excerpts, tech stack | On workspace init |
| Runtime info | Available tools, shell environment | On runtime start |
| Global microagents | Skills from `skills/` directory | On agent init |
| User microagents | From `~/.openhands/microagents/` | On agent init |
| Repo microagents | From workspace `.openhands/microagents/` | On workspace load |

The Memory class produces `RecallObservation` events that are injected into the
event stream and become part of the agent's visible history.

---

## Layer 6: Microagent Knowledge Injection

Microagents are a knowledge injection mechanism that extends the agent's capabilities
without modifying its core prompts.

### Microagent Types

**KnowledgeMicroagent** — Triggered by keyword matching in user messages:

```python
class KnowledgeMicroagent:
    triggers: list[str]     # Keywords that activate this microagent
    content: str            # Markdown knowledge to inject

    def match(self, message: str) -> bool:
        return any(trigger in message.lower() for trigger in self.triggers)
```

Example: A microagent with `triggers: ["docker", "container"]` would inject
Docker-specific guidance whenever those words appear in user messages.

**RepoMicroagent** — Always active for the current repository. Contains
repository-specific instructions, conventions, and architectural guidance.

**TaskMicroagent** — Triggered by `/command` format in user messages. May
require additional user input before producing knowledge.

### Injection Flow

```
User message arrives
  → Memory checks KnowledgeMicroagents for keyword triggers
  → Matching microagent content injected as RecallObservation
  → RecallObservation appears in agent's event view
  → Agent sees injected knowledge as part of conversation context
```

---

## Context Flow: Complete Step Lifecycle

The following traces a single agent step through all context layers:

```
1. Agent.step() is called by the controller
   │
2. condenser.condensed_history(state)
   │
   ├─ Condenser examines state.view (current visible events)
   │
   ├─ Case A: No condensation needed
   │  └─ Returns View unchanged
   │
   └─ Case B: Condensation needed
      └─ Returns Condensation (CondensationAction)
         → Controller applies action to state
         → Controller re-invokes agent.step()
         → Back to step 2 with updated view
   │
3. ConversationMemory.process_events(condensed_view.events)
   │  → Converts events to Message[]
   │  → Applies role alternation rules
   │  → Pairs tool calls with responses
   │  → Truncates oversized messages
   │
4. ConversationMemory.apply_prompt_caching(messages)
   │  → Marks cache breakpoints for Anthropic models
   │
5. LLM.completion(messages)
   │  → System prompt + condensed history sent to LLM
   │
6. LLM response → parsed into Action
   │
7. Action added to EventStream
   │  → State.view updated
   │  → Memory subscribers notified
```

---

## Agent-Requested Condensation

A distinctive feature of OpenHands is that the agent itself can request condensation
via the `CondensationRequestTool`. This gives the agent metacognitive awareness of
its own context limits:

```python
class CondensationRequestTool:
    """Allows the agent to explicitly request context condensation.

    The agent may invoke this when it detects:
    - Repetitive patterns in its history
    - Irrelevant early context consuming token budget
    - Need to "refocus" on the current sub-task
    """
```

When the agent calls this tool, the controller triggers the configured condenser,
applies the result, and re-steps the agent with a fresh view. This is a form of
**self-directed memory management** — the agent decides when to forget.

---

## Metadata Tracking

Condensers record metadata about each condensation event in
`state.extra_data['condenser_meta']`:

```python
condenser_meta = {
    "condensation_count": 3,
    "total_events_forgotten": 142,
    "last_condensation_step": 47,
    "strategy": "llm_summarizing",
    "summaries": [
        {"step": 15, "events_forgotten": 40, "summary_length": 350},
        {"step": 31, "events_forgotten": 52, "summary_length": 420},
        {"step": 47, "events_forgotten": 50, "summary_length": 380},
    ]
}
```

This metadata enables:
- **Runtime monitoring**: Track how aggressively the condenser is operating
- **Post-hoc analysis**: Understand why an agent lost important context
- **Strategy tuning**: Adjust thresholds based on real condensation patterns
- **Benchmarking**: Compare condenser strategies across tasks

---

## Design Principles

### 1. Separation of Storage and View
The EventStream stores everything; the View is a projection. This means context
management decisions are reversible at the architectural level — you can always
reconstruct what the agent saw at any point.

### 2. Strategy as a Pluggable Component
The Condenser interface allows swapping strategies without changing the agent or
controller. A `PipelineCondenser` can compose strategies, enabling experimentation
with hybrid approaches.

### 3. Condensation as an Action
By making condensation an event in the stream (rather than a silent mutation),
the system maintains a complete audit trail of *what was forgotten and why*.
This is critical for debugging and reproducibility.

### 4. Agent Awareness of Context Limits
The `CondensationRequestTool` breaks the typical pattern where context management
is invisible to the agent. Giving the agent the ability to request condensation
enables more intelligent memory management.

### 5. Knowledge Injection via Event Stream
Rather than modifying system prompts dynamically, microagent knowledge is injected
as events. This keeps the system prompt stable and makes knowledge injection
visible in the event history.

---

## Comparison with Other Approaches

| Aspect | OpenHands | Typical Approach |
|--------|-----------|------------------|
| History storage | Immutable EventStream | In-memory message list |
| Context reduction | 10+ pluggable condensers | Simple truncation |
| Agent awareness | Can request condensation | No awareness |
| Knowledge injection | Keyword-triggered microagents | Static system prompt |
| Audit trail | Complete (events + condensation actions) | Lost on truncation |
| Composability | Pipeline condenser chains | Single strategy |
| Summary quality | Structured sections via LLM | None or basic |

---

## Configuration

Condensers are configured via the agent's config, typically specifying:

```toml
[condenser]
type = "llm_summarizing"     # or "recent_events", "pipeline", etc.
max_events = 100             # Threshold for triggering condensation
keep_first = 5               # Events to preserve at start (for window-based)
keep_recent = 20             # Events to preserve at end
```

For pipeline condensers:

```toml
[condenser]
type = "pipeline"

[[condenser.stages]]
type = "observation_masking"

[[condenser.stages]]
type = "conversation_window"
max_events = 100
keep_first = 5

[[condenser.stages]]
type = "llm_summarizing"
max_events = 50
```

---

## References

- Source: `openhands/memory/condenser/` — Condenser implementations
- Source: `openhands/memory/conversation_memory.py` — Message construction
- Source: `openhands/memory/memory.py` — Knowledge/recall system
- Source: `openhands/events/stream.py` — EventStream implementation
- Source: `openhands/controller/state/state.py` — State and View definitions
- Source: `openhands/core/config/condenser_config.py` — Condenser configuration
- Repository: [All-Hands-AI/OpenHands](https://github.com/All-Hands-AI/OpenHands)