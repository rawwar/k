---
title: Turn Management
description: Track and control the number of agentic turns to prevent runaway loops and manage API costs.
---

# Turn Management

> **What you'll learn:**
> - How to define a "turn" as one complete cycle of LLM call plus optional tool execution
> - How to implement a turn counter that enforces a configurable maximum turn limit
> - How to distinguish between inner turns (tool use within a single user request) and outer turns (new user inputs)

An agentic loop without a turn limit is a loop that can run forever. If the model gets confused and keeps requesting the same tool call, or if a task genuinely requires 50 tool invocations, your agent will happily burn through API credits and time until something external stops it. Turn management is the safety valve that prevents this.

## What Is a Turn?

A "turn" in an agentic loop is one complete cycle through the loop body. Specifically:

1. Call the LLM with the current message history
2. Receive the response
3. If the response includes tool calls, execute them and feed results back
4. That is one turn

If the model responds with `end_turn` (no tool calls), that is also one turn -- it is just the final turn where the loop exits instead of continuing.

This is important to distinguish from the colloquial meaning of "turn" in a conversation. In a chat, you might think of a turn as one user message plus one assistant reply. In an agentic loop, a single user message might trigger 10 turns because the model calls tools repeatedly before giving its final answer.

## Inner Turns vs Outer Turns

It helps to think about two levels of turns:

**Inner turns** are the loop iterations within a single user request. When the user says "refactor this file" and the model reads the file, writes a new version, runs the tests, and reports back, that is 4 inner turns. The user sees one interaction, but the loop ran 4 times.

**Outer turns** are the back-and-forth between the user and the agent at the REPL level. The user types a message, the agent processes it (possibly through many inner turns), and gives a final response. Then the user types another message. That is 2 outer turns.

```rust
/// Configuration for turn limits.
pub struct TurnConfig {
    /// Maximum inner turns per user request.
    /// This limits how many LLM calls the agent can make
    /// before it must respond to the user.
    pub max_inner_turns: usize,

    /// Maximum outer turns for the entire session.
    /// After this many user interactions, the session ends.
    /// Set to 0 for unlimited.
    pub max_outer_turns: usize,
}

impl Default for TurnConfig {
    fn default() -> Self {
        TurnConfig {
            max_inner_turns: 25,
            max_outer_turns: 0, // unlimited by default
        }
    }
}
```

The inner turn limit is the critical one for preventing runaway loops. A limit of 25 is generous enough for complex tasks (a 10-tool-call refactoring with some retries) but catches infinite loops before they get expensive.

## Implementing the Turn Counter

The turn counter lives in the loop itself. Here is how it integrates:

```rust
pub struct AgentTurnState {
    /// How many inner turns have elapsed in the current user request.
    inner_turns: usize,

    /// How many outer turns (user messages) have been processed this session.
    outer_turns: usize,

    /// The configured limits.
    config: TurnConfig,
}

impl AgentTurnState {
    pub fn new(config: TurnConfig) -> Self {
        AgentTurnState {
            inner_turns: 0,
            outer_turns: 0,
            config,
        }
    }

    /// Called at the start of each inner loop iteration.
    /// Returns an error if the turn limit has been reached.
    pub fn check_inner_limit(&self) -> Result<(), AgentError> {
        if self.config.max_inner_turns > 0
            && self.inner_turns >= self.config.max_inner_turns
        {
            Err(AgentError::MaxTurnsReached {
                limit: self.config.max_inner_turns,
                turn_type: "inner".to_string(),
            })
        } else {
            Ok(())
        }
    }

    /// Increment the inner turn counter after each LLM call.
    pub fn increment_inner(&mut self) {
        self.inner_turns += 1;
    }

    /// Reset inner turns and increment outer turns.
    /// Called when the agent finishes processing a user request.
    pub fn complete_outer_turn(&mut self) {
        self.inner_turns = 0;
        self.outer_turns += 1;
    }

    /// Check if the session has exceeded its outer turn limit.
    pub fn check_outer_limit(&self) -> Result<(), AgentError> {
        if self.config.max_outer_turns > 0
            && self.outer_turns >= self.config.max_outer_turns
        {
            Err(AgentError::SessionLimitReached {
                limit: self.config.max_outer_turns,
            })
        } else {
            Ok(())
        }
    }

    /// Get the current inner turn count (useful for logging).
    pub fn inner_turns(&self) -> usize {
        self.inner_turns
    }
}
```

The error type for turn limits:

```rust
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Maximum {turn_type} turns reached: {limit}")]
    MaxTurnsReached {
        limit: usize,
        turn_type: String,
    },

    #[error("Session limit reached: {limit} interactions")]
    SessionLimitReached {
        limit: usize,
    },

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Unexpected stop reason: {0}")]
    UnexpectedStopReason(String),
}
```

## Integrating Turns into the Loop

Here is how the turn counter fits into the loop structure:

```rust
pub async fn agent_loop(
    client: &Client,
    state: &mut ConversationState,
    turn_state: &mut AgentTurnState,
) -> Result<String, AgentError> {
    loop {
        // Check the turn limit before making an API call
        turn_state.check_inner_limit()?;

        // Call the LLM
        let response = client
            .send_message(&state.to_api_request("claude-sonnet-4-20250514", 4096))
            .await
            .map_err(|e| AgentError::ApiError(e.to_string()))?;

        // Record the turn
        turn_state.increment_inner();
        state.record_usage(&response.usage);

        // Append assistant message to history
        state.add_assistant_message(response.content.clone());

        match response.stop_reason.as_deref() {
            Some("end_turn") => {
                let text = extract_text(&response.content);
                turn_state.complete_outer_turn();
                return Ok(text);
            }
            Some("tool_use") => {
                let results = execute_tool_calls(&response.content).await?;
                state.add_tool_results(results);
                // Continue the loop
            }
            Some(other) => {
                return Err(AgentError::UnexpectedStopReason(
                    other.to_string(),
                ));
            }
            None => {
                return Err(AgentError::UnexpectedStopReason(
                    "no stop_reason provided".to_string(),
                ));
            }
        }
    }
}
```

The critical line is `turn_state.check_inner_limit()?;` at the top of the loop. This runs *before* the API call, so if the limit has been reached, the loop exits immediately without spending tokens on another call.

::: python Coming from Python
In Python, you would typically track turns with a simple counter variable:
```python
turns = 0
max_turns = 25
while turns < max_turns:
    response = client.messages.create(...)
    turns += 1
    if response.stop_reason == "end_turn":
        break
# If we exit the while loop without breaking, we hit the limit
if turns >= max_turns:
    print("Warning: max turns reached")
```
The Rust approach wraps this in a struct with explicit error types. The advantage is that the turn limit check returns a `Result`, so the caller *must* handle the case where turns are exhausted. In Python, it is easy to forget the post-loop check and silently return an incomplete result.
:::

## User-Configurable Limits

You should let users configure the turn limit, either through a command-line flag or environment variable. Here is how it fits into the agent's configuration:

```rust
use std::env;

impl TurnConfig {
    /// Build turn configuration from environment variables and defaults.
    pub fn from_env() -> Self {
        let max_inner = env::var("AGENT_MAX_TURNS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(25);

        let max_outer = env::var("AGENT_MAX_SESSION_TURNS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        TurnConfig {
            max_inner_turns: max_inner,
            max_outer_turns: max_outer,
        }
    }
}
```

This pattern -- environment variable with a sensible default -- is common in production agents. It lets power users adjust limits without recompiling, while keeping the defaults safe for typical use.

## When Limits Are Hit

When the agent hits the turn limit, it needs to tell the user what happened. A good agent does not just silently stop -- it explains the situation:

```rust
fn format_turn_limit_message(error: &AgentError) -> String {
    match error {
        AgentError::MaxTurnsReached { limit, .. } => {
            format!(
                "I've reached the maximum of {} tool calls for this request. \
                 Here's what I've accomplished so far. You can continue by \
                 sending another message.",
                limit
            )
        }
        AgentError::SessionLimitReached { limit } => {
            format!(
                "This session has reached its limit of {} interactions. \
                 Please start a new session to continue.",
                limit
            )
        }
        _ => "An unexpected error occurred.".to_string(),
    }
}
```

::: wild In the Wild
Claude Code defaults to a generous inner turn limit that allows complex multi-step tasks to complete. When it hits the limit, it reports what it has accomplished and invites the user to continue with another message. This is good UX -- the user never loses work, they just need to nudge the agent to keep going. Codex takes a similar approach, defaulting to a configurable limit and reporting partial progress when it is hit.
:::

## Testing Turn Limits

Turn limits are easy to test because you do not need a real LLM. You can simulate the loop with mock responses:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inner_turn_limit() {
        let config = TurnConfig {
            max_inner_turns: 3,
            max_outer_turns: 0,
        };
        let mut turn_state = AgentTurnState::new(config);

        // First three turns should succeed
        assert!(turn_state.check_inner_limit().is_ok());
        turn_state.increment_inner();
        assert!(turn_state.check_inner_limit().is_ok());
        turn_state.increment_inner();
        assert!(turn_state.check_inner_limit().is_ok());
        turn_state.increment_inner();

        // Fourth check should fail
        assert!(turn_state.check_inner_limit().is_err());
    }

    #[test]
    fn test_outer_turn_resets_inner() {
        let config = TurnConfig {
            max_inner_turns: 3,
            max_outer_turns: 0,
        };
        let mut turn_state = AgentTurnState::new(config);

        turn_state.increment_inner();
        turn_state.increment_inner();
        turn_state.increment_inner();
        assert!(turn_state.check_inner_limit().is_err());

        // Completing the outer turn resets the inner counter
        turn_state.complete_outer_turn();
        assert!(turn_state.check_inner_limit().is_ok());
    }
}
```

## Key Takeaways

- A "turn" is one complete iteration of the agentic loop: call the LLM, process the response, optionally execute tools
- Inner turns (within a single user request) are limited to prevent runaway loops and unbounded API costs; outer turns (user interactions in a session) may also be limited
- The turn check happens *before* the API call, so hitting the limit does not waste an additional API call
- Turn limits should be user-configurable via environment variables or CLI flags, with sensible defaults (25 inner turns is a reasonable starting point)
- When limits are hit, the agent should report partial progress and invite the user to continue, never silently drop work
