---
title: State Machine Model
description: Modeling the agentic loop as a finite state machine with explicit states, transitions, and terminal conditions.
---

# State Machine Model

> **What you'll learn:**
> - How to represent the agentic loop as a state machine with states like Idle, Processing, ToolExecution, and Done
> - The valid transitions between states and the events that trigger them
> - Why the state machine model makes it easier to reason about edge cases, error recovery, and testing

In the previous subchapter, you saw the agentic loop as nested `loop` blocks with `match` statements. That works, but as the loop grows more complex -- handling errors, timeouts, parallel tool calls, and context overflow -- the control flow becomes difficult to reason about. What you need is a formal model that makes every possible state and every valid transition explicit.

That model is a finite state machine. In this subchapter, we define the states of the agentic loop, the events that cause transitions, and the terminal conditions that end the loop. This state machine becomes the blueprint for the Rust implementation you will build later.

## Why a State Machine?

When you write an agentic loop as a simple `loop` with conditionals, you implicitly encode a set of states and transitions. The problem is that these states are hidden in the control flow. Consider what happens when you need to answer questions like:

- Can the agent receive user input while a tool is executing?
- What happens if the LLM returns an error during a tool execution sequence?
- Is it valid to go from "processing tool result" directly to "waiting for user input"?

With a `loop` and `match`, you answer these by tracing through the code. With a state machine, you answer them by looking at the transition table. The state machine makes illegal states unrepresentable and forces you to handle every case.

::: python Coming from Python
Python developers often model state machines informally -- maybe an `if/elif` chain on a `status` variable, or a dictionary mapping states to handler functions. Rust's `enum` type makes state machines first-class: each state is a variant, and the compiler forces you to handle every variant in a `match`. If you add a new state and forget to handle it somewhere, the code will not compile. This is a significant advantage over Python's runtime-only checking.
:::

## The States

The agentic loop has six primary states:

**1. Idle** -- The agent is waiting for user input. The outer REPL is at the prompt. No LLM call is in progress, no tools are executing. This is the resting state.

**2. Processing** -- The agent has received user input and is calling the LLM. The API request is in flight. The agent is waiting for the model's response.

**3. ToolDetected** -- The LLM has responded with one or more tool use requests. The agent has parsed the tool calls and is preparing to execute them.

**4. ToolExecuting** -- One or more tools are running. The agent is waiting for tool execution to complete.

**5. ObservationReady** -- Tool execution has completed. The results have been formatted and added to the conversation history. The agent is ready to call the LLM again with the updated history.

**6. Done** -- The LLM has produced a final text response (stop reason: `end_turn`). The agent is ready to display this to the user and return to Idle.

And two terminal/error states:

**7. Error** -- An unrecoverable error has occurred (API failure after retries, context overflow, iteration limit exceeded). The agent must report the error and return to Idle or exit.

**8. Cancelled** -- The user has interrupted the agent (e.g., Ctrl+C). All in-progress work is abandoned and the agent returns to Idle.

In Rust, this maps directly to an enum:

```rust
enum AgentState {
    Idle,
    Processing,
    ToolDetected { tool_calls: Vec<ToolCall> },
    ToolExecuting { pending: Vec<ToolCall> },
    ObservationReady { results: Vec<ToolResult> },
    Done { response: String },
    Error { error: AgentError },
    Cancelled,
}

struct ToolCall {
    id: String,
    name: String,
    input: serde_json::Value,
}

struct ToolResult {
    tool_use_id: String,
    content: String,
    is_error: bool,
}

enum AgentError {
    ApiError(String),
    ContextOverflow,
    IterationLimit,
    ToolError(String),
}
```

Notice that some states carry data. `ToolDetected` holds the parsed tool calls. `Done` holds the response text. This is Rust's algebraic type system at work -- the state and its associated data are bundled together, and you cannot access the data without first matching the state.

## The Transition Diagram

Here are all valid transitions between states, described as a directed graph:

```text
                  User sends message
    [Idle] ─────────────────────────────> [Processing]
      ^                                       |
      |                                       | LLM responds
      |                                       v
      |                        ┌──── stop_reason = end_turn ────> [Done]
      |                        |                                    |
      |                   [Processing]                              |
      |                        |                                    |
      |                        └──── stop_reason = tool_use ──> [ToolDetected]
      |                                                             |
      |                                                     Execute tools
      |                                                             |
      |                                                             v
      |                                                      [ToolExecuting]
      |                                                             |
      |                                                    Tools complete
      |                                                             |
      |                                                             v
      |                                                   [ObservationReady]
      |                                                             |
      |                                               Call LLM with results
      |                                                             |
      |                                                             v
      |                                                       [Processing]
      |                                                        (re-enters)
      |
      |     Display response
      +──── <────────────── [Done]
      |
      |     Report error
      +──── <────────────── [Error]
      |
      |     Acknowledge cancellation
      +──── <────────────── [Cancelled]

  Any state ──── API/tool error ──────> [Error]
  Any state ──── User interrupt ──────> [Cancelled]
```

Read this diagram carefully. The key cycle is: **Processing -> ToolDetected -> ToolExecuting -> ObservationReady -> Processing**. This is the inner loop. It repeats until the LLM decides to produce a final response (`end_turn`), at which point the transition goes to Done instead of ToolDetected.

## Transitions as Events

Each transition is triggered by a specific event. Let's catalog them:

| From | To | Trigger Event |
|------|----|---------------|
| Idle | Processing | User submits a message |
| Processing | Done | LLM responds with stop_reason = end_turn |
| Processing | ToolDetected | LLM responds with stop_reason = tool_use |
| Processing | Error | API call fails (after retries) |
| ToolDetected | ToolExecuting | Agent begins tool execution |
| ToolExecuting | ObservationReady | All tools complete (success or failure) |
| ToolExecuting | Error | Tool execution causes unrecoverable error |
| ObservationReady | Processing | Tool results added to history, LLM called again |
| ObservationReady | Error | Iteration limit or token budget exceeded |
| Done | Idle | Response displayed to user |
| Error | Idle | Error reported to user |
| Cancelled | Idle | Cancellation acknowledged |
| Any | Cancelled | User sends interrupt signal (Ctrl+C) |

This table is your specification. Every transition in this table must have corresponding code in your implementation. Any transition *not* in this table is illegal -- if your code somehow goes from `Idle` directly to `ToolExecuting`, you have a bug.

## Implementing the State Machine in Rust

The state machine translates cleanly into a `loop` with a `match`:

```rust
fn run_agent_turn(state: &mut AgentContext) -> Result<String, AgentError> {
    let mut current = AgentState::Processing;

    loop {
        current = match current {
            AgentState::Processing => {
                match call_llm(&state.history, &state.tools) {
                    Ok(response) => {
                        match response.stop_reason {
                            StopReason::EndTurn => {
                                AgentState::Done { response: response.text }
                            }
                            StopReason::ToolUse => {
                                AgentState::ToolDetected {
                                    tool_calls: response.tool_calls,
                                }
                            }
                        }
                    }
                    Err(e) => AgentState::Error { error: e },
                }
            }

            AgentState::ToolDetected { tool_calls } => {
                AgentState::ToolExecuting { pending: tool_calls }
            }

            AgentState::ToolExecuting { pending } => {
                let results: Vec<ToolResult> = pending
                    .iter()
                    .map(|tc| execute_tool(&tc.name, &tc.input))
                    .collect();

                // Add tool calls and results to history
                for (call, result) in pending.iter().zip(results.iter()) {
                    state.history.push_tool_call(call);
                    state.history.push_tool_result(result);
                }

                state.iteration_count += 1;
                if state.iteration_count >= state.max_iterations {
                    AgentState::Error { error: AgentError::IterationLimit }
                } else {
                    AgentState::ObservationReady { results }
                }
            }

            AgentState::ObservationReady { .. } => {
                // Results are already in history, go back to Processing
                AgentState::Processing
            }

            AgentState::Done { response } => {
                return Ok(response);
            }

            AgentState::Error { error } => {
                return Err(error);
            }

            _ => unreachable!("Invalid state in agent turn"),
        };
    }
}
```

Every arm of the `match` produces the next state. The compiler ensures you handle every variant. If you add a new state to the enum, every `match` in your code will demand a new arm. This is the state machine pattern in Rust: the `enum` defines the states, the `match` defines the transitions, and the compiler enforces completeness.

::: tip In the Wild
OpenCode explicitly models its agent loop as a state machine in Go. Its `agent.go` file contains a `for` loop with a `switch` on the response type, transitioning between "calling the model," "executing tools," and "returning results." Claude Code takes a similar approach in TypeScript, using a loop that checks `response.stop_reason` to decide whether to continue tool execution or return to the user. The state machine is implicit in both implementations, but the pattern is identical to what we have described here.
:::

## Benefits of the State Machine Model

Modeling the loop as a state machine gives you three concrete benefits:

**Exhaustive handling.** The Rust compiler will not let you forget a state. If you add a `Paused` state for user approval of dangerous tools, every `match` must handle it.

**Testability.** You can unit-test individual transitions: "Given state `ToolDetected` with these tool calls, verify the next state is `ToolExecuting` with the same calls." No need to set up a full REPL or mock an LLM to test state logic.

**Debuggability.** When something goes wrong, you can log state transitions: `Processing -> ToolDetected -> ToolExecuting -> Error(timeout)`. This trace tells you exactly where and why the loop failed.

## Key Takeaways

- The agentic loop has six primary states (Idle, Processing, ToolDetected, ToolExecuting, ObservationReady, Done) plus Error and Cancelled
- The inner loop is the cycle: Processing -> ToolDetected -> ToolExecuting -> ObservationReady -> Processing, repeating until the LLM produces a final response
- Rust's enum type maps naturally to state machine states, and `match` expressions enforce exhaustive transition handling at compile time
- Every valid state transition should be enumerated in a transition table -- any transition not in the table is a bug
- The state machine model makes the loop testable (test individual transitions), debuggable (log state traces), and safe (the compiler catches missing cases)
