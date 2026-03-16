---
title: Stop Conditions
description: Define the conditions under which the agentic loop should terminate and return control to the user.
---

# Stop Conditions

> **What you'll learn:**
> - The three primary stop conditions: end_turn from the model, maximum turns reached, and explicit user interrupt
> - How to read the `stop_reason` field from the API response to detect when the model considers its task complete
> - How to handle edge cases like the model getting stuck in a loop or producing empty responses

The agentic loop needs to know when to stop. This sounds simple -- stop when the model says it is done -- but in practice there are several conditions that should terminate the loop, and some of them involve edge cases that can silently break your agent if you do not handle them.

## The Primary Stop Conditions

There are five conditions that should cause the agentic loop to exit:

### 1. The Model Signals End of Turn

The most common stop condition. The API response contains `"stop_reason": "end_turn"`, meaning the model has finished its work and is responding to the user. The response content contains the text of the model's final answer.

```rust
match response.stop_reason.as_deref() {
    Some("end_turn") => {
        // Model is done -- extract text and return to user
        let text = extract_text(&response.content);
        return Ok(LoopResult::Complete(text));
    }
    // ...
}
```

### 2. Maximum Turns Reached

You implemented this in the previous subchapter. When the inner turn counter hits the configured limit, the loop exits before making another API call.

```rust
if turn_state.check_inner_limit().is_err() {
    let partial = last_assistant_text(&state.messages);
    return Ok(LoopResult::TurnLimitReached(partial));
}
```

### 3. Maximum Tokens Reached

The API response might contain `"stop_reason": "max_tokens"`, meaning the model ran out of output space before finishing its response. This is different from `end_turn` -- the model was *cut off*, not *finished*. You have a few options: treat it as an error, try to continue the conversation, or return the partial response.

```rust
Some("max_tokens") => {
    // Model was cut off -- its response is incomplete.
    // We can try to continue by prompting it to keep going,
    // or we can return the partial result.
    let partial = extract_text(&response.content);
    return Ok(LoopResult::MaxTokens(partial));
}
```

### 4. Context Window Overflow

If the conversation state has grown so large that it approaches the model's context window limit, continuing would either fail (the API rejects the request) or degrade quality (the model struggles with very long contexts). You should check this before each API call:

```rust
if state.is_approaching_limit(MAX_CONTEXT_TOKENS) {
    return Ok(LoopResult::ContextOverflow);
}
```

### 5. Error Conditions

Network errors, API errors (rate limits, server errors), malformed responses, or tool execution failures can all terminate the loop. These are not "normal" stop conditions -- they are exceptions that require error handling:

```rust
let response = client
    .send_message(&request)
    .await
    .map_err(|e| AgentError::ApiError(e.to_string()))?;
```

The `?` operator propagates the error up to the caller, terminating the loop.

## Modeling Stop Conditions as an Enum

Let's define a result type that captures all the ways the loop can end:

```rust
/// The outcome of running the agentic loop for a single user request.
#[derive(Debug)]
pub enum LoopResult {
    /// The model completed its response normally (stop_reason: end_turn).
    Complete(String),

    /// The model was cut off by the token limit (stop_reason: max_tokens).
    /// Contains whatever partial text was generated.
    MaxTokens(String),

    /// The inner turn limit was reached before the model finished.
    /// Contains the last assistant text, if any.
    TurnLimitReached(String),

    /// The conversation state grew too large for the context window.
    ContextOverflow,
}

impl LoopResult {
    /// Get the text content from the result, regardless of how the loop ended.
    pub fn text(&self) -> &str {
        match self {
            LoopResult::Complete(text) => text,
            LoopResult::MaxTokens(text) => text,
            LoopResult::TurnLimitReached(text) => text,
            LoopResult::ContextOverflow => {
                "The conversation has grown too long. Please start a new session."
            }
        }
    }

    /// Returns true if the model completed its work normally.
    pub fn is_complete(&self) -> bool {
        matches!(self, LoopResult::Complete(_))
    }
}
```

By making `LoopResult` an enum, you force every caller to consider all the ways the loop can end. The `text()` method provides a convenient fallback for callers that just want to display something.

::: python Coming from Python
In Python, you might represent different exit conditions with a dictionary or a simple tuple:
```python
def agent_loop(messages):
    # ...
    return {"status": "complete", "text": response_text}
    # or
    return {"status": "max_turns", "text": partial_text}
```
The problem is that nothing enforces the caller checking the `status` field. With Rust's `LoopResult` enum and pattern matching, the compiler tells you if you forget to handle one of the variants:
```rust
match result {
    LoopResult::Complete(text) => println!("{text}"),
    LoopResult::TurnLimitReached(text) => {
        println!("{text}");
        println!("(Turn limit reached -- send another message to continue)");
    }
    // Compiler error if you forget MaxTokens or ContextOverflow!
}
```
:::

## Handling the stop_reason Field

The `stop_reason` field from the Anthropic API has a few possible values, and you should handle all of them:

```rust
/// Process the stop_reason from an API response and determine the loop action.
fn decide_action(
    stop_reason: Option<&str>,
    content: &[ContentBlock],
) -> LoopAction {
    match stop_reason {
        Some("end_turn") => LoopAction::ReturnToUser,
        Some("tool_use") => LoopAction::ExecuteTools,
        Some("max_tokens") => LoopAction::MaxTokensReached,
        Some("stop_sequence") => {
            // A custom stop sequence was hit. Treat as end_turn.
            LoopAction::ReturnToUser
        }
        Some(other) => LoopAction::UnexpectedReason(other.to_string()),
        None => {
            // No stop_reason -- check if there are tool_use blocks in content.
            // Some API edge cases can omit stop_reason.
            if content.iter().any(|b| matches!(b, ContentBlock::ToolUse { .. })) {
                LoopAction::ExecuteTools
            } else {
                LoopAction::ReturnToUser
            }
        }
    }
}

#[derive(Debug)]
enum LoopAction {
    ReturnToUser,
    ExecuteTools,
    MaxTokensReached,
    UnexpectedReason(String),
}
```

The `None` case is worth noting. While the API should always include a `stop_reason`, defensive coding means handling the case where it does not. Looking at the content blocks as a fallback is a safe heuristic.

## Edge Case: Empty Responses

Sometimes the model returns a response with no meaningful content -- an empty text block, or a response where `content` is an empty array. This can happen when the model is confused or when the context is corrupted. You should detect this and break the loop to avoid an infinite cycle of empty exchanges:

```rust
fn is_empty_response(content: &[ContentBlock]) -> bool {
    if content.is_empty() {
        return true;
    }
    // Check if all text blocks are empty or whitespace-only
    content.iter().all(|block| match block {
        ContentBlock::Text { text } => text.trim().is_empty(),
        _ => false,
    })
}

// In the loop:
if is_empty_response(&response.content) {
    return Err(AgentError::EmptyResponse {
        turn: turn_state.inner_turns(),
    });
}
```

## Edge Case: Repetitive Tool Calls

A subtler problem is when the model gets stuck calling the same tool with the same arguments repeatedly. This can happen if a tool returns an error and the model keeps retrying without changing its approach. You can detect this by tracking recent tool calls:

```rust
/// Track recent tool calls to detect repetitive behavior.
pub struct RepetitionDetector {
    recent_calls: Vec<(String, String)>, // (tool_name, input_hash)
    max_repeats: usize,
}

impl RepetitionDetector {
    pub fn new(max_repeats: usize) -> Self {
        RepetitionDetector {
            recent_calls: Vec::new(),
            max_repeats,
        }
    }

    /// Record a tool call and return true if it has been repeated
    /// too many times.
    pub fn record_and_check(&mut self, tool_name: &str, input: &str) -> bool {
        let key = (tool_name.to_string(), input.to_string());
        self.recent_calls.push(key.clone());

        let count = self
            .recent_calls
            .iter()
            .filter(|c| **c == key)
            .count();

        count >= self.max_repeats
    }

    /// Clear the history when a new user message arrives.
    pub fn reset(&mut self) {
        self.recent_calls.clear();
    }
}
```

::: wild In the Wild
Production agents like Claude Code handle repetition detection at multiple levels. The loop itself has a turn limit, but there are also checks for specific patterns like repeated identical tool calls, tool calls that always return errors, and sequences where the model alternates between two states without making progress. When repetition is detected, Claude Code can inject a hint into the conversation telling the model to try a different approach.
:::

## Combining All Stop Conditions

Here is how all the conditions come together in the loop:

```rust
pub async fn agent_loop(
    client: &Client,
    state: &mut ConversationState,
    config: &TurnConfig,
) -> Result<LoopResult, AgentError> {
    let mut turn_state = AgentTurnState::new(config.clone());
    let mut repetition = RepetitionDetector::new(3);

    loop {
        // Stop condition: turn limit
        turn_state.check_inner_limit()?;

        // Stop condition: context overflow
        if state.is_approaching_limit(MAX_CONTEXT_TOKENS) {
            return Ok(LoopResult::ContextOverflow);
        }

        // Call the LLM (stop condition: API error propagated by ?)
        let response = client.send_message(&state.to_api_request()).await?;
        turn_state.increment_inner();
        state.add_assistant_message(response.content.clone());

        // Stop condition: empty response
        if is_empty_response(&response.content) {
            return Err(AgentError::EmptyResponse {
                turn: turn_state.inner_turns(),
            });
        }

        // Decide what to do based on stop_reason
        match decide_action(response.stop_reason.as_deref(), &response.content) {
            LoopAction::ReturnToUser => {
                let text = extract_text(&response.content);
                turn_state.complete_outer_turn();
                return Ok(LoopResult::Complete(text));
            }
            LoopAction::ExecuteTools => {
                let results = execute_tool_calls(
                    &response.content,
                    &mut repetition,
                ).await?;
                state.add_tool_results(results);
            }
            LoopAction::MaxTokensReached => {
                let text = extract_text(&response.content);
                return Ok(LoopResult::MaxTokens(text));
            }
            LoopAction::UnexpectedReason(reason) => {
                return Err(AgentError::UnexpectedStopReason(reason));
            }
        }
    }
}
```

Every exit path is explicit. The compiler ensures you handle every `LoopAction` variant. And every stop condition is checked at the appropriate point in the loop -- before the API call (turn limit, context overflow) or after (stop reason, empty response, repetition).

## Key Takeaways

- Five conditions can stop the loop: model end_turn, max turns, max tokens, context overflow, and errors -- each requires a different response
- Model `LoopResult` as an enum so the compiler forces callers to handle every exit path, not just the happy path
- Always check `stop_reason` defensively: handle `None`, handle unknown values, and look at content blocks as a fallback
- Detect edge cases that cause infinite loops: empty responses and repetitive tool calls are the two most common culprits
- Check stop conditions at the right time: turn limits and context overflow *before* the API call, response-based conditions *after*
