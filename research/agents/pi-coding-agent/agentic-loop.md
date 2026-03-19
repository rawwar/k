# Pi — The Minimal Agent Loop

## Overview

Pi's agentic loop is deliberately simple. Where other coding agents (Claude Code, Aider, Goose) build complex multi-phase loops with planning, validation, lint-test-fix cycles, and sub-agent delegation, Pi implements a straightforward message → LLM → tool calls → results cycle. Complexity is pushed to extensions rather than baked into the core loop.

This simplicity is a conscious design choice. Mario Zechner's philosophy is that a complex core loop means complex debugging, unpredictable behavior, and prompt cache instability. By keeping the loop minimal, Pi ensures that the agent's behavior is transparent and that extensions can modify it at well-defined points.

## The Core Loop

```
┌─────────────────────────────────────────────────────────────┐
│                      Pi Agent Loop                           │
│                                                              │
│  ┌──────────────┐                                           │
│  │  User Input   │ ◄── Interactive / RPC / SDK / Print      │
│  └──────┬───────┘                                           │
│         │                                                    │
│         ▼                                                    │
│  ┌──────────────┐                                           │
│  │   Message     │ Check message queue for steering          │
│  │   Assembly    │ messages, append to conversation           │
│  └──────┬───────┘                                           │
│         │                                                    │
│         ▼                                                    │
│  ┌──────────────┐                                           │
│  │   LLM Call    │ Send conversation to provider via pi-ai   │
│  │  (streaming)  │ Stream tokens to TUI as they arrive       │
│  └──────┬───────┘                                           │
│         │                                                    │
│         ├──── No tool calls ──── ▶ Done (wait for input)    │
│         │                                                    │
│         ▼                                                    │
│  ┌──────────────┐                                           │
│  │  Tool Calls   │ Execute read/write/edit/bash              │
│  │  (sequential) │ (or extension-registered tools)           │
│  └──────┬───────┘                                           │
│         │                                                    │
│         ▼                                                    │
│  ┌──────────────┐                                           │
│  │  Tool Results │ Append results to conversation            │
│  └──────┬───────┘                                           │
│         │                                                    │
│         ▼                                                    │
│  ┌──────────────┐                                           │
│  │  Context      │ Check token count; trigger compaction     │
│  │  Check        │ if approaching limit                      │
│  └──────┬───────┘                                           │
│         │                                                    │
│         └──────── Loop back to LLM Call ─────────────────►  │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Phase 1: Message Assembly

The loop begins with a user message arriving through one of the four modes (Interactive, Print/JSON, RPC, SDK). Before sending to the LLM, Pi checks the **message queue** for any pending steering messages that should be injected.

Context is assembled from:
- The conversation history (stored in the tree-structured JSONL session)
- System prompt (default + SYSTEM.md overrides)
- AGENTS.md project instructions
- Any dynamically injected context from extensions

### Phase 2: LLM Call

Pi sends the assembled conversation to the configured provider via `pi-ai`. The response streams token-by-token, rendered to the TUI in real-time via differential rendering. During streaming, the user can:
- Press Enter to submit a **steering message** (delivered after current tool call)
- Press Alt+Enter to queue a **follow-up** (waits until agent finishes)

### Phase 3: Tool Execution

If the LLM response includes tool calls, Pi executes them against the registered tool handlers. The default tools are `read`, `write`, `edit`, and `bash`, but extensions can register additional tools or replace the defaults entirely.

Tool calls are executed and their results are appended to the conversation history.

### Phase 4: Context Check & Compaction

After tool results are appended, Pi checks whether the conversation is approaching the model's context window limit. If it is, **compaction** is triggered — older messages are summarized to free space while preserving essential context (see [context-management.md](context-management.md)).

### Phase 5: Loop or Stop

If the LLM made tool calls, the loop continues back to Phase 2 with the updated conversation. If the LLM responded with just text (no tool calls), the loop pauses and waits for the next user input.

## The Message Queue

Pi's message queue is one of its more interesting interaction patterns. While the agent is working (executing tool calls, waiting for LLM responses), the user can submit messages that are queued for delivery at specific points.

### Steering Messages (Enter)

Pressing Enter while the agent works submits a **steering message**. This message is delivered to the LLM after the current tool call completes but before the next LLM inference. This allows the user to redirect the agent mid-task:

```
Agent is running bash tests...
User presses Enter: "Actually, skip the integration tests, just fix the unit tests"
→ Message delivered after bash completes
→ LLM sees the steering message in its next inference
→ Agent adjusts course
```

**Delivery timing**: After the current tool call finishes, before the next LLM call.

### Follow-up Messages (Alt+Enter)

Pressing Alt+Enter queues a **follow-up message**. This waits until the agent completely finishes its current task (all tool calls done, final text response given) before being delivered:

```
Agent is working on feature A...
User presses Alt+Enter: "After you're done, also update the README"
→ Message waits until agent finishes feature A
→ Delivered as a new user message
→ Agent starts working on README
```

**Delivery timing**: After the agent's complete turn (text response with no tool calls).

### Configurable Delivery Modes

Extensions can customize message queue behavior — changing delivery timing, adding message types, or implementing priority queues. This flexibility means the message queue can be adapted to different workflows without modifying the core loop.

## How the Loop Stays Simple

The key insight is what the loop does NOT do:

| Feature | Other Agents | Pi |
|---------|-------------|-----|
| Plan-then-execute | Built into loop | Extension or file-based |
| Lint-test-fix cycle | Automatic retry | Extension or manual |
| Sub-agent delegation | Built-in orchestration | Spawn via tmux or extension |
| Permission gates | Interrupt loop for approval | Extension or container isolation |
| Background tasks | Built-in task management | Use tmux |
| Automatic retries | Retry logic in loop | Extension |

Each of these features can be added via extensions, but none of them complicate the core loop. This means:

1. **Debugging is straightforward** — the loop has one path, not branching logic
2. **Prompt cache stays stable** — no conditional system prompt sections
3. **Behavior is predictable** — no hidden retry logic or automatic fixes
4. **Extensions have clear hooks** — they modify behavior at well-defined points

## Compaction Triggering

Compaction is the only "complex" behavior in the core loop. When the conversation approaches the context window limit:

1. Pi detects the token count is above a configurable threshold
2. Older messages (not the most recent exchanges) are summarized
3. The summary replaces the original messages in the conversation
4. The loop continues with the compacted context

Extensions can fully customize compaction behavior — changing the summarization prompt, the threshold, which messages are compacted, or replacing the entire compaction strategy.

## Event System

Throughout the loop, Pi emits events that extensions can subscribe to:

- `message:received` — User message arrived
- `message:streaming` — LLM tokens arriving
- `tool:start` — Tool call about to execute
- `tool:complete` — Tool call finished with result
- `response:complete` — LLM turn finished
- `compaction:triggered` — Context compaction starting
- `session:updated` — Session state changed

This event-driven architecture lets extensions observe and modify loop behavior without touching the loop itself. An extension could log all tool calls, gate certain operations behind approval prompts, implement automatic rollback on errors, or inject additional context between turns — all through event handlers.

## Comparison to Other Agent Loops

| Agent | Loop Complexity | Built-in Phases |
|-------|----------------|-----------------|
| Aider | Medium | Edit → Apply → Lint → Test → Fix |
| Claude Code | High | Plan → Execute → Validate → Sub-agents |
| Goose | Medium | Think → Act → Observe + Extensions |
| **Pi** | **Low** | **Message → LLM → Tools → Loop** |

Pi's loop is the simplest of the major terminal coding agents. This is not a limitation — it's the core architectural decision that enables everything else. The loop provides a stable foundation; extensions provide the features.
