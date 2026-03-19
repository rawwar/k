# SageAgent — Context Management

> Note: Context management internals are limited in the public documentation. This analysis
> is inferred from the message format and pipeline architecture.

## Message Format

SageAgent uses a structured message format with typed messages:

| Type | Purpose |
|---|---|
| `normal` | Regular conversation messages — user input, agent output, final results |
| `thinking` | Intermediate thought process — reasoning steps, internal deliberation |

This two-type system is simpler than frameworks that use many message roles (e.g., system,
user, assistant, tool, function). The `thinking` type likely maps to chain-of-thought
traces that can be displayed in the UI or logged for debugging.

## Inter-Agent Context Flow

Context flows through the pipeline sequentially. Each agent receives output from the
previous agent and produces structured output for the next:

```
User Input (text)
    │
    ▼
TaskAnalysisAgent
    │  Output: Structured task description
    ▼
PlanningAgent
    │  Output: Ordered subtask list with dependencies
    ▼
ExecutorAgent
    │  Output: Execution results per subtask
    ▼
ObservationAgent
    │  Output: Completion assessment + feedback (if incomplete)
    ▼
TaskSummaryAgent
    │  Output: Human-readable summary
    ▼
Final Output (text)
```

## Context Accumulation

As the pipeline progresses, each agent likely has access to:
- The original user request
- Outputs from all preceding agents in the current iteration
- (In feedback loops) Results from previous iterations

This accumulation pattern means context grows with each pipeline stage and each
feedback iteration. The "Infinite Context" roadmap item suggests that context window
limits are a known challenge for complex multi-iteration tasks.

## Feedback Loop Context

When the ObservationAgent triggers a re-plan, the PlanningAgent receives:
- Original task analysis (from TaskAnalysisAgent — stable across iterations)
- Previous execution results (what was accomplished)
- Observation feedback (what remains to be done)

This gives the PlanningAgent enough context to create a revised plan without
re-analyzing the original request.

## Execution Mode Impact

- **Deep Research Mode**: Full context accumulation across multiple iterations.
  Context can grow significantly for complex tasks.
- **Rapid Execution Mode**: Likely single-pass, so context stays bounded to one
  pipeline traversal.

## Streamlit UI Context

The `sage_demo.py` Streamlit interface provides a web-based view of the agent
pipeline execution. The `thinking` message type likely drives a collapsible
"reasoning" panel in the UI, while `normal` messages appear as primary output.

## Limitations

The public documentation does not detail:
- Context window management strategies (truncation, summarization)
- Whether agents share a single conversation thread or maintain separate contexts
- Memory or persistence mechanisms across sessions
- Token budget allocation across pipeline stages

---

*Tier 3 analysis — context management is the least documented aspect. Most details
inferred from architecture and message type descriptions.*