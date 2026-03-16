---
title: Conversation as State
description: Modeling an agent conversation as a finite state machine with defined states, transitions, and invariants that govern message flow and context management.
---

# Conversation as State

> **What you'll learn:**
> - How to model a conversation as a state machine with states like Idle, AwaitingResponse, ToolExecution, and Compacting
> - The invariants that must hold at each state transition, such as alternating user/assistant roles and valid tool call sequences
> - Why treating conversation as explicit state prevents subtle bugs in context management and message ordering

When you build a Python chatbot, you typically store messages in a list and append new ones as they arrive. This works fine for short conversations, but a coding agent's conversation is far more complex. The agent cycles through distinct phases -- waiting for user input, calling the LLM, executing tools, compacting history -- and each phase has different rules about what messages can be added, modified, or removed. If you treat the conversation as a dumb list, you will eventually hit subtle bugs: a tool result appearing without a matching tool call, a compaction running mid-response, or messages arriving in an impossible order.

The solution is to model your conversation as a **state machine**. Each conversation exists in exactly one state at any moment, and only specific transitions are allowed from each state. This makes illegal states unrepresentable -- a core Rust philosophy that Python developers find transformative once they experience it.

## Defining Conversation States

Let's start by identifying the states a coding agent conversation moves through during a typical interaction:

```rust
#[derive(Debug, Clone, PartialEq)]
enum ConversationState {
    /// Waiting for user input. No LLM call in progress.
    Idle,
    /// User message received, preparing to send to LLM.
    Preparing,
    /// Request sent to LLM, waiting for response stream.
    AwaitingResponse,
    /// LLM has requested tool execution(s).
    ToolExecution {
        pending_calls: Vec<ToolCallId>,
        completed_calls: Vec<ToolCallId>,
    },
    /// Context window limit approached, running compaction.
    Compacting,
    /// Conversation has been terminated (error or user exit).
    Terminated { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ToolCallId(String);
```

Each variant captures not just the state name but the data relevant to that state. `ToolExecution` tracks which tool calls are still pending and which have completed -- you need both to know when to send results back to the LLM. `Terminated` carries a reason so you can distinguish between a normal exit and an error.

::: python Coming from Python
In Python, you would likely model this with a string variable (`self.state = "idle"`) or maybe a string enum. The problem is nothing stops you from setting `self.state = "awaiting_rsponse"` (notice the typo) -- Python happily assigns it. Rust's enum system means the compiler rejects any state that isn't in the defined set. You literally cannot create an invalid state.
:::

## Defining Valid Transitions

The state machine only has meaning if you enforce which transitions are legal. Not every state can transition to every other state -- for example, you should never go directly from `Idle` to `ToolExecution` without first receiving an LLM response.

```rust
#[derive(Debug, Clone)]
enum ConversationEvent {
    UserMessage(String),
    LlmRequestSent,
    LlmResponseText(String),
    LlmToolCallRequested(Vec<ToolCallId>),
    ToolCallCompleted(ToolCallId),
    AllToolCallsCompleted,
    CompactionTriggered,
    CompactionCompleted,
    SessionEnded(String),
}

struct Conversation {
    state: ConversationState,
    messages: Vec<Message>,
}

impl Conversation {
    fn transition(&mut self, event: ConversationEvent) -> Result<(), ConversationError> {
        use ConversationState::*;
        use ConversationEvent::*;

        let new_state = match (&self.state, event) {
            // From Idle: user sends a message
            (Idle, UserMessage(text)) => {
                self.messages.push(Message::user(text));
                Preparing
            }

            // From Preparing: request sent to LLM
            (Preparing, LlmRequestSent) => AwaitingResponse,

            // From AwaitingResponse: LLM responds with text
            (AwaitingResponse, LlmResponseText(text)) => {
                self.messages.push(Message::assistant(text));
                Idle
            }

            // From AwaitingResponse: LLM requests tool calls
            (AwaitingResponse, LlmToolCallRequested(calls)) => {
                let pending = calls.clone();
                self.messages.push(Message::tool_calls(calls));
                ToolExecution {
                    pending_calls: pending,
                    completed_calls: vec![],
                }
            }

            // From ToolExecution: a tool call finishes
            (ToolExecution { pending_calls, completed_calls }, ToolCallCompleted(id)) => {
                let mut pending = pending_calls.clone();
                let mut completed = completed_calls.clone();
                pending.retain(|c| c != &id);
                completed.push(id);

                if pending.is_empty() {
                    Preparing // All done, re-send to LLM with results
                } else {
                    ToolExecution {
                        pending_calls: pending,
                        completed_calls: completed,
                    }
                }
            }

            // Compaction can happen from Idle or Preparing
            (Idle, CompactionTriggered) | (Preparing, CompactionTriggered) => Compacting,

            // Compaction finishes, return to Idle
            (Compacting, CompactionCompleted) => Idle,

            // Terminal state from anywhere except Terminated
            (state, SessionEnded(reason)) if *state != Terminated { reason: reason.clone() } => {
                Terminated { reason }
            }

            // Any other transition is invalid
            (state, event) => {
                return Err(ConversationError::InvalidTransition {
                    from: format!("{:?}", state),
                    event: format!("{:?}", event),
                });
            }
        };

        self.state = new_state;
        Ok(())
    }
}
```

The `match` on `(&self.state, event)` is the heart of the state machine. Rust's pattern matching guarantees that every combination is either explicitly handled or falls to the catch-all error arm. You can never accidentally forget a case if you remove the catch-all -- the compiler will list every unhandled combination.

## Enforcing Invariants

Beyond state transitions, you need invariants -- properties that must always be true regardless of which state you're in. For LLM conversations, the critical invariants are:

1. **Role alternation**: User and assistant messages must alternate (with tool messages allowed between assistant and next user message).
2. **Tool call pairing**: Every `tool_result` message must reference a `tool_call` that actually exists in the history.
3. **System prompt position**: The system prompt is always the first message and never appears elsewhere.
4. **Token count consistency**: The cached total token count always equals the sum of individual message token counts.

```rust
impl Conversation {
    fn validate_invariants(&self) -> Result<(), Vec<InvariantViolation>> {
        let mut violations = Vec::new();

        // Check role alternation
        let mut expect_user = true;
        for msg in &self.messages {
            match msg.role {
                Role::System => continue, // System prompt is special
                Role::User if expect_user => expect_user = false,
                Role::Assistant if !expect_user => expect_user = true,
                Role::ToolResult => {} // Tool results don't affect alternation
                _ => violations.push(InvariantViolation::RoleAlternation {
                    expected: if expect_user { "user" } else { "assistant" }.into(),
                    got: format!("{:?}", msg.role),
                }),
            }
        }

        // Check tool call pairing
        let tool_call_ids: std::collections::HashSet<&str> = self.messages.iter()
            .filter_map(|m| m.tool_call_id.as_deref())
            .collect();

        for msg in &self.messages {
            if msg.role == Role::ToolResult {
                if let Some(ref call_id) = msg.references_call_id {
                    if !tool_call_ids.contains(call_id.as_str()) {
                        violations.push(InvariantViolation::OrphanedToolResult {
                            call_id: call_id.clone(),
                        });
                    }
                }
            }
        }

        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations)
        }
    }
}
```

You call `validate_invariants()` after every state transition during development and testing. In production, you might only validate on session save or when debugging unexpected behavior -- it's your safety net.

::: wild In the Wild
Claude Code models its conversation as a series of turns with strict ordering rules. The system prompt is assembled fresh for each API call from a template plus dynamic context, and tool results must always reference a valid `tool_use` block from the immediately preceding assistant message. OpenCode takes a similar approach in its Go implementation, using a `messages` slice with role-based validation before each API call. Both agents treat an invalid message sequence as a hard error rather than trying to fix it -- the principle is that corrupted conversation state should fail loudly rather than produce subtly wrong LLM responses.
:::

## Why State Machines Prevent Real Bugs

This might feel like over-engineering for a chat application, but consider these real scenarios that the state machine catches:

**Race condition during streaming**: The user sends a new message while the LLM is still streaming a response. Without the state machine, both the streaming response and the new user message could end up in the history simultaneously, confusing the LLM on the next call. With the state machine, the `UserMessage` event in `AwaitingResponse` state returns an `InvalidTransition` error, and your UI can queue the message or show a "please wait" indicator.

**Compaction during tool execution**: A tool returns a massive output that pushes the conversation over the context limit. Without state tracking, the compaction might run and remove the tool call that the still-pending tool result refers to. With the state machine, compaction is only allowed from `Idle` or `Preparing` states.

**Double tool completion**: A network retry causes the same tool result to arrive twice. The state machine detects that the tool call ID has already moved from `pending_calls` to `completed_calls` and rejects the duplicate.

## Conversation State vs. Application State

One important distinction: conversation state (the state machine we just built) is not the same as application state. Your agent also has configuration state (model, temperature, API keys), UI state (which panel is visible), and file system state (which files are open). Keep these separate:

```rust
struct AgentState {
    conversation: Conversation,       // The state machine
    config: AgentConfig,              // Model, temperature, etc.
    active_session: Option<SessionId>, // Persistence tracking
    tool_registry: ToolRegistry,       // Available tools
}
```

The `Conversation` struct owns the state machine and message history. Everything else lives alongside it, not inside it. This separation means you can serialize the conversation independently, swap configurations without affecting history, and test the state machine in isolation.

## Key Takeaways

- Model your conversation as a state machine with explicit states (`Idle`, `AwaitingResponse`, `ToolExecution`, `Compacting`, `Terminated`) and validated transitions between them.
- Use Rust's enum system to make illegal states unrepresentable -- the compiler enforces your state machine rules, catching bugs that would be runtime errors in Python.
- Define and validate invariants like role alternation and tool call pairing after every transition to catch corrupted conversation state early.
- Separate conversation state from application state so you can serialize, test, and reason about each independently.
- State machines prevent real concurrency and ordering bugs: race conditions during streaming, compaction during tool execution, and duplicate message handling all become compile-time or explicit runtime errors.
