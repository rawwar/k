---
title: Response Generation
description: How the agent decides between continuing the tool loop and presenting a final response to the user.
---

# Response Generation

> **What you'll learn:**
> - How stop reasons in the LLM response determine whether the loop continues or presents output to the user
> - The difference between end_turn (final response) and tool_use (continue loop) stop reasons
> - How to format and present the agent's final response including summaries of actions taken

Response generation is the transition from the Done state back to Idle -- the moment when the agent has completed its work and presents results to the user. But it also encompasses the decision that leads to Done in the first place: after each LLM call, your code inspects the stop reason and decides whether to continue the inner loop or stop.

This subchapter covers both sides: how the model signals it is ready to respond, and how your code formats and delivers that response to the user.

## The Stop Reason Decision

Every LLM response includes a `stop_reason` field (sometimes called `finish_reason` in other APIs). This field is the primary control signal for the agentic loop:

```rust
enum StopReason {
    EndTurn,        // Model has finished its response
    ToolUse,        // Model wants to execute tools
    MaxTokens,      // Response hit the max_tokens limit
    StopSequence,   // Response hit a stop sequence
}

fn decide_next_state(response: &LlmResponse) -> AgentState {
    match response.stop_reason {
        StopReason::EndTurn => {
            AgentState::Done {
                response: response.text.clone(),
            }
        }
        StopReason::ToolUse => {
            AgentState::ToolDetected {
                tool_calls: response.tool_calls.clone(),
            }
        }
        StopReason::MaxTokens => {
            // The model was cut off mid-response
            // This needs special handling
            handle_max_tokens(response)
        }
        StopReason::StopSequence => {
            // Treated like end_turn for most agents
            AgentState::Done {
                response: response.text.clone(),
            }
        }
    }
}
```

The `EndTurn` and `ToolUse` cases are straightforward -- they map directly to the two branches of the inner loop. The interesting cases are `MaxTokens` and how the model decides when it is "done."

## The MaxTokens Edge Case

When the model hits the `max_tokens` limit, its response is truncated. The model did not choose to stop -- it was forced to stop. This can happen in the middle of a sentence, the middle of a code block, or even the middle of a tool call's JSON parameters.

How you handle this depends on what was truncated:

```rust
fn handle_max_tokens(response: &LlmResponse) -> AgentState {
    // Check if there are incomplete tool calls
    if !response.tool_calls.is_empty() {
        // The model was trying to make tool calls but got cut off
        // The last tool call might have incomplete JSON parameters
        let valid_calls: Vec<ToolCall> = response.tool_calls
            .iter()
            .filter(|tc| is_valid_json(&tc.input))
            .cloned()
            .collect();

        if valid_calls.is_empty() {
            // No valid tool calls -- treat as an incomplete text response
            AgentState::Done {
                response: format!(
                    "{}\n\n[Response was truncated due to length limit]",
                    response.text
                ),
            }
        } else {
            // Execute the valid tool calls and continue
            AgentState::ToolDetected {
                tool_calls: valid_calls,
            }
        }
    } else {
        // Pure text response that was truncated
        // Option 1: Show what we have with a truncation notice
        AgentState::Done {
            response: format!(
                "{}\n\n[Response was truncated due to length limit. \
                 You can ask the agent to continue.]",
                response.text
            ),
        }
        // Option 2: Automatically continue by sending the partial
        // response back and asking the model to continue
        // (more complex but better UX)
    }
}

fn is_valid_json(value: &serde_json::Value) -> bool {
    // Check that the value is a non-null object
    // (incomplete JSON from truncation often fails to parse at all,
    // but serde might parse partial objects)
    value.is_object()
}
```

::: python Coming from Python
Python's Anthropic SDK handles the same `stop_reason` values. In Python, you would check `response.stop_reason == "max_tokens"` and handle it similarly. The logic is identical -- the difference is that Rust's match exhaustiveness check ensures you handle every stop reason variant, while Python lets you silently ignore unknown stop reasons if you forget an `elif` branch.
:::

## What the Model Considers "Done"

The model decides to produce an `end_turn` stop reason when it believes it has completed the user's request. This is a judgment call by the model, not a hard rule. Several factors influence this decision:

**The system prompt** can instruct the model about when to stop. For example: "After making code changes, always verify them by running the tests before providing your final response." This encourages the model to do more tool calls before concluding.

**The task complexity** matters. For a simple question ("What does this function do?"), the model might read one file and respond. For a complex task ("Refactor this module to use async/await"), the model might go through many iterations of reading, editing, testing, and fixing before it is satisfied.

**The tool results** affect the model's decision. If a test fails after a code change, the model will typically loop back to fix the issue rather than presenting a broken result. If all tests pass, it will conclude.

You cannot directly control when the model stops. But you can influence it through the system prompt and through the information you provide in tool results. This is why formatting tool results well (as we discussed in the previous subchapter) matters -- it gives the model the information it needs to decide whether more work is required.

## Formatting the Final Response

When the agent reaches the Done state, it has accumulated text from the model's final response. This text might reference actions taken during the inner loop ("I've fixed the compilation error in src/main.rs by adding the missing import"). Your code presents this to the user:

```rust
struct FinalResponse {
    text: String,
    tool_calls_made: usize,
    tokens_used: TokenUsage,
    duration: std::time::Duration,
}

fn present_response(response: &FinalResponse) {
    // Print the model's response text
    // (if streaming was used, the text was already printed in real-time
    //  during the LLM invocation phase)
    if !response.text.is_empty() {
        println!("\n{}", response.text);
    }

    // Optionally show a summary of what happened
    if response.tool_calls_made > 0 {
        println!(
            "\n[{} tool calls | {} input tokens, {} output tokens | {:.1}s]",
            response.tool_calls_made,
            response.tokens_used.input_tokens,
            response.tokens_used.output_tokens,
            response.duration.as_secs_f64()
        );
    }
}
```

The summary line at the bottom provides transparency. The user sees not just the final answer, but how much work went into it: how many tool calls, how many tokens, how long it took. This helps the user calibrate their expectations and understand the cost of different kinds of requests.

## Streaming and Response Generation

When you use streaming, the response generation flow changes significantly. With streaming, the model's text appears in real-time as it is generated -- the user sees it token by token. By the time the `end_turn` stop reason arrives, most of the response has already been displayed.

This means your "present response" step is less about printing text and more about finalization:

```rust
fn finalize_streamed_response(
    assembler: ResponseAssembler,
    tool_calls_made: usize,
    cumulative_usage: &TokenUsage,
    start_time: std::time::Instant,
) -> FinalResponse {
    let response = assembler.finish();

    // The text was already streamed to the terminal during LLM invocation
    // We just need to add a newline and the summary

    println!(); // End the streaming line

    let final_response = FinalResponse {
        text: response.text,
        tool_calls_made,
        tokens_used: cumulative_usage.clone(),
        duration: start_time.elapsed(),
    };

    present_response(&final_response);

    final_response
}
```

## Accumulating Results Across Iterations

The final response is not just the text from the last LLM call. The agent has been doing work throughout the inner loop -- reading files, running commands, editing code. A good agent response acknowledges this work. The model typically does this naturally ("I've read the file, found two issues, and fixed both"), but your code should track the work for the summary:

```rust
struct TurnTracker {
    iterations: usize,
    tool_calls: Vec<ToolCallRecord>,
    total_input_tokens: u32,
    total_output_tokens: u32,
    start_time: std::time::Instant,
}

struct ToolCallRecord {
    name: String,
    duration: std::time::Duration,
    success: bool,
}

impl TurnTracker {
    fn new() -> Self {
        Self {
            iterations: 0,
            tool_calls: Vec::new(),
            total_input_tokens: 0,
            total_output_tokens: 0,
            start_time: std::time::Instant::now(),
        }
    }

    fn record_iteration(&mut self, usage: &TokenUsage) {
        self.iterations += 1;
        self.total_input_tokens += usage.input_tokens;
        self.total_output_tokens += usage.output_tokens;
    }

    fn record_tool_call(&mut self, name: &str, duration: std::time::Duration, success: bool) {
        self.tool_calls.push(ToolCallRecord {
            name: name.to_string(),
            duration,
            success,
        });
    }

    fn summary(&self) -> String {
        let total_duration = self.start_time.elapsed();
        let tool_summary = if self.tool_calls.is_empty() {
            String::new()
        } else {
            let tool_names: Vec<&str> = self.tool_calls
                .iter()
                .map(|tc| tc.name.as_str())
                .collect();
            format!(" | tools: {}", tool_names.join(", "))
        };

        format!(
            "[{} iterations | {} tool calls{} | {} in, {} out tokens | {:.1}s]",
            self.iterations,
            self.tool_calls.len(),
            tool_summary,
            self.total_input_tokens,
            self.total_output_tokens,
            total_duration.as_secs_f64()
        )
    }
}
```

::: tip In the Wild
Claude Code displays a summary after each turn showing the number of tool calls made and the tokens consumed. It also provides a running cost estimate based on the model's token pricing. OpenCode tracks similar metrics and displays them in its TUI status bar. Both agents accumulate token usage across all inner loop iterations so the user sees the total cost of the turn, not just the last API call.
:::

## The Return to Idle

After the response is presented, the agent returns to the Idle state -- the outer REPL's prompt reappears, and the user can type their next message. But the transition is not just about printing text. The agent also needs to:

**Update the conversation history** -- The model's final text response must be added to the history so the next turn has full context:

```rust
fn complete_turn(
    history: &mut ConversationHistory,
    final_response: &LlmResponse,
) {
    // Add the assistant's final response to history
    // (tool call responses were already added during the inner loop)
    if !final_response.text.is_empty() {
        history.add_assistant_response(final_response);
    }
}
```

**Reset turn-level state** -- Iteration counters, turn-specific token budgets, and other per-turn tracking should be reset:

```rust
fn reset_turn_state(agent: &mut AgentContext) {
    agent.current_turn_iterations = 0;
    agent.current_turn_tool_calls = 0;
    // Session-level state (total tokens, total cost) persists
}
```

**Check session-level limits** -- Even though the turn is complete, the agent should check whether the overall session is approaching limits (total token budget, session time, etc.) and warn the user if so.

The agent is now back in Idle, with an updated conversation history, ready for the next user message. The full cycle -- from the user's input through the inner loop to the final response -- is complete.

## Key Takeaways

- The `stop_reason` field is the primary signal: `end_turn` means present the response, `tool_use` means continue the loop, and `max_tokens` means the response was truncated and may need special handling
- The model decides when it is "done" based on the task, tool results, and system prompt instructions -- your code cannot force this, but can influence it through system prompt design
- With streaming, the response text appears in real-time during LLM invocation, so response generation is mainly about finalization, summary display, and state cleanup
- Turn-level metrics (iterations, tool calls, tokens, duration) should be tracked and displayed to give the user transparency into the agent's work
- After presenting the response, the agent updates conversation history, resets turn-level state, and returns to Idle -- maintaining a clean separation between turns
