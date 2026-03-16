---
title: Summary
description: A complete recap of the agentic loop architecture, its state machine model, and how it prepares us for tool system implementation.
---

# Summary

> **What you'll learn:**
> - A unified view of the agentic loop from input to output including all intermediate states
> - The design decisions that will guide our Rust implementation of the loop in later chapters
> - How the loop architecture connects to the tool system deep dive in Chapter 5

You have now seen the agentic loop from every angle: its origins in the REPL pattern, its transformation from chatbot to agent through tool use, its formal model as a state machine, and its concrete implementation in production systems. Let's bring everything together into a unified picture that will serve as the blueprint for the Rust implementation ahead.

## The Complete Picture

The agentic loop is a two-level architecture:

**The outer loop** is a REPL. It reads user input, delegates to the inner loop, presents results, and waits for the next input. The user controls this loop -- they decide when to send messages, when to interrupt, and when to exit.

**The inner loop** is the agentic core. It calls the LLM, detects tool requests, executes tools, collects observations, and repeats until the model signals completion. The model controls this loop -- it decides which tools to call, how to interpret results, and when it has enough information to respond.

Here is the complete flow, combining every phase we covered in this chapter:

```text
OUTER LOOP (User-controlled):
  [Idle]
    |
    User sends message
    |
    v
  [Input Processing]
    - Capture input (readline, multi-line)
    - Detect slash commands (/help, /clear, /exit)
    - Validate input (empty check, length check)
    - Create user message, append to history
    - Assemble full context (system prompt + history + tools)
    - Manage context window (truncate or compact if needed)
    |
    v
  INNER LOOP (Model-controlled):
    [Processing]
      |
      - Construct API request
      - Send to LLM (streaming)
      - Parse streaming events, display text in real-time
      - Assemble complete response (text + tool calls + usage)
      - Handle API errors (retry with backoff for rate limits/server errors)
      |
      v
    [Tool Call Detection]
      |
      - Inspect stop_reason: end_turn or tool_use?
      - If end_turn -> [Done]
      - If tool_use -> extract and validate tool calls
      |
      v
    [Tool Dispatch]
      |
      - Look up tool handler in registry
      - Check permissions (auto-approve reads, prompt for writes)
      - Execute tool with timeout enforcement
      - Capture result (success or error)
      |
      v
    [Observation Collection]
      |
      - Format tool results (line numbers, stdout/stderr separation)
      - Truncate large outputs (head + tail strategy)
      - Format errors with self-correction hints
      - Add assistant message + tool results to history
      |
      v
    [Stop Condition Check]
      |
      - Check iteration limit
      - Check token budget
      - Check user interrupt (Ctrl+C)
      - Check error budget (consecutive failures)
      - If any limit hit -> graceful degradation with progress report
      - If all clear -> back to [Processing]
      |
      v
    (loop back to Processing)
    ...
    [Done]
      |
      v
  [Response Generation]
    - Finalize streamed text (add newline, flush buffer)
    - Display turn summary (iterations, tool calls, tokens, duration)
    - Add final assistant message to history
    - Reset turn-level state (iteration counter, turn tokens)
    |
    v
  [Idle]
    (wait for next user message)
```

## The State Machine Recap

The formal state machine has eight states and a well-defined transition table:

| State | Description | Outgoing Transitions |
|-------|-------------|---------------------|
| **Idle** | Waiting for user input | -> Processing (user sends message) |
| **Processing** | LLM API call in progress | -> Done (end_turn), -> ToolDetected (tool_use), -> Error (API failure) |
| **ToolDetected** | Tool calls parsed and validated | -> ToolExecuting (begin execution) |
| **ToolExecuting** | Tools running | -> ObservationReady (tools complete), -> Error (unrecoverable tool failure) |
| **ObservationReady** | Results formatted and added to history | -> Processing (continue loop), -> Error (limits exceeded) |
| **Done** | Final response ready | -> Idle (response displayed) |
| **Error** | Unrecoverable failure | -> Idle (error reported) |
| **Cancelled** | User interrupt | -> Idle (cancellation acknowledged) |

The inner loop is the cycle: **Processing -> ToolDetected -> ToolExecuting -> ObservationReady -> Processing**. It repeats until Processing transitions to Done instead of ToolDetected.

In Rust, this maps to an enum with data-carrying variants, a `loop` with a `match`, and the compiler's exhaustiveness checking to ensure every state is handled:

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

fn run_turn(context: &mut AgentContext) -> Result<String, AgentError> {
    let mut state = AgentState::Processing;

    loop {
        state = match state {
            AgentState::Processing => { /* call LLM, return next state */ }
            AgentState::ToolDetected { tool_calls } => { /* begin execution */ }
            AgentState::ToolExecuting { pending } => { /* run tools */ }
            AgentState::ObservationReady { results } => { /* format and continue */ }
            AgentState::Done { response } => return Ok(response),
            AgentState::Error { error } => return Err(error),
            AgentState::Cancelled => return Ok("Cancelled.".to_string()),
            AgentState::Idle => unreachable!("Idle during turn execution"),
        };
    }
}
```

## The Design Decisions

Throughout this chapter, we identified several design decisions that will guide our Rust implementation. Here is a summary:

### Architecture
- **Two-loop structure**: outer REPL for user interaction, inner loop for model-driven tool execution
- **State machine model**: explicit states and transitions, enforced by Rust's enum and match
- **History as single source of truth**: all state flows through the conversation message array

### Input Processing
- **Slash command interception**: handled in the outer REPL, before the inner loop
- **Context management**: sliding window with compaction for long conversations
- **Token estimation**: character-based heuristic for decisions, precise counting for limits

### LLM Invocation
- **Streaming by default**: SSE parsing with real-time text display
- **Retry with backoff**: for rate limits (429) and server errors (5xx)
- **Response assembly**: accumulate partial JSON for tool call parameters

### Tool System
- **Registry pattern**: map tool names to handlers at startup
- **Permission tiers**: auto-approve reads, prompt for writes
- **Timeout enforcement**: kill tools that exceed time limits
- **Parallel execution**: for independent read-only tools

### Error Handling
- **Model-driven recovery**: feed errors back as tool results for self-correction
- **Error budgets**: limit consecutive and total errors to prevent infinite error loops
- **Graceful degradation**: report progress when stop conditions fire mid-task

::: python Coming from Python
If you have built agents in Python using frameworks like LangChain or direct API calls, the architecture we have described will be familiar at the conceptual level. The key differences in Rust are: (1) the state machine is enforced by the compiler via enums and exhaustive matching, not just by convention; (2) streaming is more explicit because Rust requires you to handle the async stream processing manually; and (3) the type system catches many error-handling omissions at compile time that Python would only catch at runtime. These differences add upfront development cost but dramatically reduce the "works in testing, breaks in production" failure mode.
:::

## What Comes Next

This chapter was about understanding the loop. The next chapters are about building the components that plug into it:

**Chapter 5: Tool System Deep Dive** covers the tool registry, tool definitions, parameter validation, and the trait-based tool abstraction in Rust. You will build a complete tool system that your agentic loop can dispatch to.

**Chapter 6: Conversation Management** covers the conversation history data structure, token counting, context window management, and compaction strategies. You will build the memory system that feeds the agentic loop.

**Chapter 7: Process Management and Shell** covers spawning child processes, capturing output, handling timeouts, and sandboxing. You will build the `run_command` tool that lets your agent execute shell commands.

Each of these chapters builds a component that slots into the agentic loop architecture described here. The loop is the skeleton; the tools, conversation management, and process handling are the muscles and organs.

::: wild In the Wild
Both Claude Code and OpenCode evolved their loop implementations over time. The initial versions were simpler -- fewer error states, no parallel tool execution, basic permission models. As real users encountered edge cases, the loops grew more robust. This is the natural progression: build the basic loop first, get it working end-to-end, then layer on sophistication as real-world usage reveals what matters. Our Rust implementation will follow the same trajectory.
:::

## The Agentic Loop in One Paragraph

An agentic loop is a two-level control structure where an outer REPL handles user interaction and an inner loop handles model-driven tool execution. On each turn, user input is processed into a message, added to the conversation history, and sent to the LLM along with the system prompt and tool definitions. The LLM responds with either a final text response (ending the turn) or tool use requests (continuing the loop). Tool calls are validated, dispatched through a registry, executed with permission checks and timeouts, and their results are formatted and fed back into the conversation history. The inner loop continues until the model produces a final response, an iteration or token limit is reached, or the user interrupts. Errors at every stage are either retried (API failures), fed back to the model for self-correction (tool failures), or reported to the user (unrecoverable failures). The entire system is modeled as a state machine with eight states and deterministic transitions, making it testable, debuggable, and extensible.

## Exercises

These exercises focus on analyzing and designing aspects of the agentic loop. They emphasize reasoning about edge cases, failure modes, and architectural trade-offs.

### Exercise 1: Loop Termination Scenario Analysis (Easy)

For each of these scenarios, identify which stop condition should fire and what the agent should report to the user:

1. The model has made 25 tool calls without producing a final response (iteration limit is 30)
2. The conversation history has reached 180K tokens in a 200K context window
3. The model has attempted the same file edit 4 times and failed each time (error budget is 5)
4. The user presses Ctrl+C while the model is mid-stream generating a response
5. The model returns `end_turn` but its response is empty

**Deliverable:** For each scenario, state which condition fires, what the agent should communicate to the user, and whether the agent should save its partial progress.

### Exercise 2: Error Recovery Strategy Comparison (Medium)

Compare three approaches to handling tool execution failures in the agentic loop: (a) feed the error back to the model as a tool result and let it retry, (b) retry the tool call automatically with the same arguments after a delay, and (c) ask the user to decide what to do. For each approach, identify two scenarios where it works well and two where it fails.

**What to consider:** Think about transient vs. permanent errors, the cost of wasted API calls, user experience during long autonomous tasks, and what happens when the model enters an error loop. Consider how the error budget interacts with each strategy.

**Deliverable:** A comparison table with the three approaches, their strengths, weaknesses, and the two good/two bad scenarios for each. Include a recommendation for which approach to use as the default and when to switch to alternatives.

### Exercise 3: State Machine Extension Design (Medium)

The chapter describes an 8-state machine for the agentic loop. Design an extension that adds support for "planning steps" -- a mode where the model thinks about what to do before taking action, similar to chain-of-thought reasoning. Define: what new states are needed, what transitions connect them to the existing states, what data each new state carries, and how the user can see the plan.

**What to consider:** Should planning be a separate state or a modifier on existing states? Should the plan be shown to the user before execution? Can the user reject a plan? How does planning interact with the iteration limit -- does a planning step count as an iteration?

**Deliverable:** An updated state transition table showing the new states and transitions, the Rust enum variants with their data, and a paragraph explaining the user experience.

### Exercise 4: Comparing Loop Implementations (Hard)

The chapter describes loops in Claude Code (TypeScript), OpenCode (Go), and Pi (Rust). Analyze how each would handle this sequence of events: the model requests two tool calls in parallel, the first succeeds but the second times out, and the model then requests a third tool call that depends on the result of the timed-out second call.

**What to consider:** How does each agent handle parallel tool execution? What does the model see when a tool times out -- an error result, a timeout notification, or nothing? How does the model handle a dependency on a failed tool? What should the ideal behavior be, and which agent's architecture best supports it?

**Deliverable:** A step-by-step trace through each agent's loop for this scenario, identifying where they diverge. Include a proposed "ideal" handling and an analysis of which architectural decisions enable or prevent it.

### Exercise 5: Graceful Degradation Design (Hard)

Design a graceful degradation strategy for when the agent hits a stop condition mid-task. The agent should: summarize what it has accomplished so far, identify what remains to be done, save enough state that the user can resume the task, and present this information clearly. Define what data needs to be captured during loop execution to enable this report, and how the agent constructs the summary.

**What to consider:** The agent cannot ask the model to summarize (it may have hit a token limit). Consider what metadata the loop should track during execution: which files were read, which were modified, which tool calls succeeded, which failed, and what the model's last stated intent was. Think about how this interacts with conversation history persistence.

**Deliverable:** A design specifying the tracking data structure, the summary generation algorithm, the user-facing output format, and how resume would work in the next session.

## Key Takeaways

- The agentic loop is a two-level architecture: an outer REPL (user-controlled) containing an inner tool loop (model-controlled), connected by the conversation history as the single source of truth
- The inner loop follows the cycle Processing -> ToolDetected -> ToolExecuting -> ObservationReady -> Processing, repeating until the model signals end_turn or a stop condition fires
- Rust's enum type and exhaustive match make the state machine model a natural fit for implementation, with the compiler enforcing that every state and transition is handled
- Production agents (Claude Code, OpenCode) implement this same pattern with the same states, validating that the model we have described captures the essential structure of real agent loops
- The agentic loop is the skeleton of the agent -- the tool system, conversation management, and process handling that we build in subsequent chapters are the components that slot into this structure
