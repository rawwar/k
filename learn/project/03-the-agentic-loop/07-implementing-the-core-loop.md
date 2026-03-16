---
title: Implementing the Core Loop
description: Write the central while loop in Rust that orchestrates LLM calls, tool dispatch, and state updates.
---

# Implementing the Core Loop

> **What you'll learn:**
> - How to write the main `async fn agent_loop` that ties together the LLM client, message history, and tool execution
> - How to use pattern matching on the response's stop_reason and content blocks to decide the next action
> - How to structure the code so that adding new tool types later requires no changes to the loop itself

This is the subchapter where everything comes together. You have designed the message types, the conversation state, the turn counter, and the stop conditions. Now you will write the actual loop function that wires them all together into a working agentic loop.

## The Complete Loop Implementation

Let's build the loop in a file called `src/agent.rs`. This is the heart of your coding agent. In `src/agent.rs`, add the following:

```rust
use crate::types::{
    AgentError, ContentBlock, ConversationState, LoopAction, LoopResult,
    Message, TurnConfig, Usage,
};
use reqwest::Client as HttpClient;
use serde::Serialize;
use serde_json::Value;

/// Configuration for the agent.
pub struct AgentConfig {
    pub api_key: String,
    pub model: String,
    pub max_tokens: u32,
    pub system_prompt: String,
    pub turn_config: TurnConfig,
}

/// The main agent that runs the agentic loop.
pub struct Agent {
    http_client: HttpClient,
    config: AgentConfig,
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self {
        Agent {
            http_client: HttpClient::new(),
            config,
        }
    }

    /// Run the agentic loop for a single user message.
    ///
    /// This is the core function of the entire agent. It sends the user's
    /// message to the LLM, processes the response, executes any tool calls,
    /// feeds results back, and repeats until the model is done or a stop
    /// condition is met.
    pub async fn run(
        &self,
        state: &mut ConversationState,
        user_message: &str,
    ) -> Result<LoopResult, AgentError> {
        // Add the user's message to the conversation
        state.add_user_message(user_message);

        let mut inner_turns: usize = 0;
        let max_turns = self.config.turn_config.max_inner_turns;

        loop {
            // --- Stop condition: turn limit ---
            if max_turns > 0 && inner_turns >= max_turns {
                let partial = self.last_assistant_text(state);
                return Ok(LoopResult::TurnLimitReached(partial));
            }

            // --- Stop condition: context window ---
            if state.is_approaching_limit(180_000) {
                return Ok(LoopResult::ContextOverflow);
            }

            // --- Phase 1: Call the LLM ---
            let response = self.call_api(state).await?;
            inner_turns += 1;

            // Record token usage
            state.record_usage(&response.usage);

            // --- Phase 2: Append assistant response to history ---
            state.add_assistant_message(response.content.clone());

            // --- Stop condition: empty response ---
            if self.is_empty_response(&response.content) {
                return Err(AgentError::EmptyResponse {
                    turn: inner_turns,
                });
            }

            // --- Phase 3: Decide what to do next ---
            match self.decide_action(
                response.stop_reason.as_deref(),
                &response.content,
            ) {
                LoopAction::ReturnToUser => {
                    // The model is done. Extract the text and return.
                    let text = Self::extract_text(&response.content);
                    return Ok(LoopResult::Complete(text));
                }

                LoopAction::ExecuteTools => {
                    // The model wants to call tools. Execute them
                    // and feed results back.
                    let tool_results = self
                        .handle_tool_calls(&response.content)
                        .await;
                    state.add_tool_results(tool_results);
                    // Loop continues -- the model will see the results
                    // on the next iteration.
                }

                LoopAction::MaxTokensReached => {
                    let text = Self::extract_text(&response.content);
                    return Ok(LoopResult::MaxTokens(text));
                }

                LoopAction::UnexpectedReason(reason) => {
                    return Err(AgentError::UnexpectedStopReason(reason));
                }
            }
        }
    }

    /// Call the Anthropic Messages API with the current conversation state.
    async fn call_api(
        &self,
        state: &ConversationState,
    ) -> Result<ApiResponse, AgentError> {
        let request_body = ApiRequest {
            model: self.config.model.clone(),
            max_tokens: self.config.max_tokens,
            system: self.config.system_prompt.clone(),
            messages: state.messages.clone(),
        };

        let response = self
            .http_client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AgentError::ApiError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".to_string());
            return Err(AgentError::ApiError(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        response
            .json::<ApiResponse>()
            .await
            .map_err(|e| AgentError::ApiError(e.to_string()))
    }

    /// Determine the next action based on the API response.
    fn decide_action(
        &self,
        stop_reason: Option<&str>,
        content: &[ContentBlock],
    ) -> LoopAction {
        match stop_reason {
            Some("end_turn") => LoopAction::ReturnToUser,
            Some("tool_use") => LoopAction::ExecuteTools,
            Some("max_tokens") => LoopAction::MaxTokensReached,
            Some("stop_sequence") => LoopAction::ReturnToUser,
            Some(other) => {
                LoopAction::UnexpectedReason(other.to_string())
            }
            None => {
                // Fallback: check content blocks for tool_use
                if content
                    .iter()
                    .any(|b| matches!(b, ContentBlock::ToolUse { .. }))
                {
                    LoopAction::ExecuteTools
                } else {
                    LoopAction::ReturnToUser
                }
            }
        }
    }

    /// Handle tool calls from the assistant's response.
    /// Returns tool result content blocks to feed back into the conversation.
    ///
    /// NOTE: This is a stub implementation. Chapter 4 replaces this with
    /// a real tool registry and dispatch system.
    async fn handle_tool_calls(
        &self,
        content: &[ContentBlock],
    ) -> Vec<ContentBlock> {
        let mut results = Vec::new();

        for block in content {
            if let ContentBlock::ToolUse { id, name, input } = block {
                // Stub: return a placeholder result for any tool call
                let result_text = format!(
                    "[Tool '{}' called with input: {}. \
                     Tool execution not yet implemented.]",
                    name, input
                );
                results.push(ContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    content: result_text,
                    is_error: Some(true),
                });
            }
        }

        results
    }

    /// Extract all text content from response content blocks.
    fn extract_text(content: &[ContentBlock]) -> String {
        content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Check if a response has no meaningful content.
    fn is_empty_response(&self, content: &[ContentBlock]) -> bool {
        if content.is_empty() {
            return true;
        }
        content.iter().all(|block| match block {
            ContentBlock::Text { text } => text.trim().is_empty(),
            _ => false,
        })
    }

    /// Get the text from the last assistant message, if any.
    fn last_assistant_text(&self, state: &ConversationState) -> String {
        state
            .messages
            .iter()
            .rev()
            .find(|m| m.role == crate::types::Role::Assistant)
            .map(|m| Self::extract_text(&m.content))
            .unwrap_or_default()
    }
}

/// The request body sent to the Anthropic Messages API.
#[derive(Debug, Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<Message>,
}

/// The response body from the Anthropic Messages API.
#[derive(Debug, serde::Deserialize)]
struct ApiResponse {
    #[allow(dead_code)]
    id: String,
    content: Vec<ContentBlock>,
    #[allow(dead_code)]
    model: String,
    stop_reason: Option<String>,
    usage: Usage,
}
```

Let's walk through the key design decisions.

## Why an `Agent` Struct?

The loop could be a standalone function, but wrapping it in an `Agent` struct gives you a natural place to put configuration and shared resources:

- The HTTP client is created once and reused across all API calls (connection pooling)
- The API key and model name are configured once, not passed to every function
- Later chapters will add more state: a tool registry, a permission system, a UI handle

This is the builder pattern in practice. You construct an `Agent`, configure it, and then call `.run()` for each user message.

::: python Coming from Python
In Python, you would likely use a class with a similar structure:
```python
class Agent:
    def __init__(self, api_key, model, system_prompt, max_turns=25):
        self.client = anthropic.Anthropic(api_key=api_key)
        self.model = model
        self.system_prompt = system_prompt
        self.max_turns = max_turns

    def run(self, messages, user_message):
        messages.append({"role": "user", "content": user_message})
        for turn in range(self.max_turns):
            response = self.client.messages.create(
                model=self.model,
                system=self.system_prompt,
                messages=messages,
                max_tokens=4096,
            )
            messages.append({"role": "assistant", "content": response.content})
            if response.stop_reason == "end_turn":
                return response.content[0].text
            # handle tool calls...
```
The structures are nearly identical. The main difference is that Rust's version uses `async/await` and returns a `Result`, making error handling explicit rather than relying on exception propagation.
:::

## The Loop Is Tool-Agnostic

Look at the `handle_tool_calls` method. Right now it is a stub that returns an error for every tool call. But the loop does not care -- it just takes whatever `Vec<ContentBlock>` comes back and feeds it into the conversation state. When you replace this stub with a real tool registry in Chapter 4, the loop itself does not change.

This is the key architectural insight: the loop orchestrates, but it does not implement tool logic. It asks "are there tool calls?", delegates to a handler, and feeds results back. What happens inside the handler is not the loop's concern.

## Pattern Matching as Control Flow

The `match` on `LoopAction` is the decision point of the entire loop. Each arm leads to a different outcome:

- `ReturnToUser` breaks the loop and returns the model's text
- `ExecuteTools` processes tool calls and lets the loop continue
- `MaxTokensReached` returns the partial response
- `UnexpectedReason` returns an error

Because Rust's `match` is exhaustive, adding a new `LoopAction` variant in the future (say, `HumanApprovalRequired` for a permission system) will cause a compile error everywhere the match is used. The compiler guides you to every place that needs updating.

## Integrating with the REPL

To use this loop from your REPL (built in Chapter 1), you call `agent.run()` for each user input:

```rust
use std::io::{self, BufRead, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AgentConfig {
        api_key: std::env::var("ANTHROPIC_API_KEY")?,
        model: "claude-sonnet-4-20250514".to_string(),
        max_tokens: 4096,
        system_prompt: "You are a helpful coding assistant.".to_string(),
        turn_config: TurnConfig::default(),
    };

    let agent = Agent::new(config);
    let mut state = ConversationState::new("You are a helpful coding assistant.");

    let stdin = io::stdin();
    print!("> ");
    io::stdout().flush()?;

    for line in stdin.lock().lines() {
        let input = line?;
        if input.trim().is_empty() {
            print!("> ");
            io::stdout().flush()?;
            continue;
        }

        match agent.run(&mut state, &input).await {
            Ok(result) => {
                println!("\n{}", result.text());
                if !result.is_complete() {
                    println!("(Note: response may be incomplete)");
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }

        print!("\n> ");
        io::stdout().flush()?;
    }

    Ok(())
}
```

Each call to `agent.run()` enters the agentic loop, which may iterate multiple times for tool calls before returning. The `state` persists between calls, so the model has full context of the entire session.

::: wild In the Wild
Claude Code's core loop follows this same pattern -- an outer REPL that reads user input and an inner agentic loop that processes each request. The outer loop handles command parsing (slash commands like `/help` and `/clear`), while the inner loop handles the LLM interaction. OpenCode separates these into distinct goroutines in Go, with the REPL running on the main thread and the agent loop running asynchronously. The separation between "user interaction" and "agent processing" is consistent across all production agents.
:::

## What This Does Not Handle Yet

The loop is functional but incomplete. Here is what is missing -- each gap will be filled in a later chapter:

| Missing piece | Chapter |
|---|---|
| Real tool execution (file read, write, shell) | Chapter 4-6 |
| Streaming responses (see tokens as they arrive) | Chapter 7 |
| Terminal UI (rich display, progress indicators) | Chapter 8 |
| Context compaction (smart truncation) | Chapter 9 |
| Permission checks before tool execution | Chapter 12 |

The beauty of this architecture is that each of these additions plugs into the existing loop without restructuring it. The tool registry replaces `handle_tool_calls`. Streaming replaces `call_api`. The UI wraps the output. Context management wraps the state. Permissions add a check before tool dispatch. The loop's `match` on `LoopAction` stays the same.

## Key Takeaways

- The core loop is an `async fn` on an `Agent` struct that calls the LLM, inspects the response, optionally executes tools, feeds results back, and repeats
- Pattern matching on `LoopAction` provides exhaustive, compiler-verified control flow -- every possible response from the API is handled explicitly
- The loop is tool-agnostic: `handle_tool_calls` is a stub that Chapter 4 replaces with a real tool registry, without changing the loop itself
- An `Agent` struct holds shared resources (HTTP client, config) that persist across all loop iterations, similar to how a Python class would hold a client instance
- The loop integrates with the REPL by being called once per user message, while the conversation state carries context across messages
