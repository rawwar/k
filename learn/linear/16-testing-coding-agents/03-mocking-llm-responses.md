---
title: Mocking LLM Responses
description: Build LLM mock infrastructure that returns predetermined responses, enabling fast and deterministic testing of the agentic loop.
---

# Mocking LLM Responses

> **What you'll learn:**
> - How to implement a mock provider that returns scripted responses, supporting multi-turn conversations with tool use sequences
> - Techniques for designing mock response builders that make it easy to construct realistic LLM outputs including text, tool calls, and stop reasons
> - How to use the mock provider to test specific agent behaviors like tool selection, error recovery, and conversation flow without calling real APIs

Your agentic loop calls the LLM on every turn. If your integration tests hit the real API, they become slow (seconds per turn), expensive (tokens cost money), and non-deterministic (different responses each run). Mocking the LLM provider solves all three problems. You build a fake provider that returns scripted responses, and your loop cannot tell the difference.

The key design decision you made back in the agentic loop chapter — putting the LLM provider behind a trait — now pays dividends. You can swap in any implementation that satisfies the trait, including one that returns hardcoded responses from a queue.

## The Provider Trait

Let's start with the trait that your real and mock providers both implement. This is the boundary that separates deterministic from non-deterministic code:

```rust
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: MessageContent,
}

#[derive(Debug, Clone)]
pub enum MessageContent {
    Text(String),
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: String },
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: Vec<MessageContent>,
    pub stop_reason: StopReason,
    pub usage: Usage,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
}

#[derive(Debug, Clone, Default)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn send(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse, ProviderError>;
}

#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug)]
pub enum ProviderError {
    RateLimited,
    ServerError(String),
    NetworkError(String),
}
```

## Building the Mock Provider

The mock provider holds a queue of responses. Each time the agentic loop calls `send()`, the mock pops the next response off the queue. If the queue is empty, it returns an error — this also catches bugs where the loop makes more LLM calls than you expected:

```rust
use std::sync::Mutex;

pub struct MockProvider {
    responses: Mutex<Vec<LlmResponse>>,
    recorded_calls: Mutex<Vec<Vec<Message>>>,
}

impl MockProvider {
    pub fn new(responses: Vec<LlmResponse>) -> Self {
        Self {
            responses: Mutex::new(responses),
            recorded_calls: Mutex::new(Vec::new()),
        }
    }

    /// Returns all the message histories that were sent to this provider.
    pub fn calls(&self) -> Vec<Vec<Message>> {
        self.recorded_calls.lock().unwrap().clone()
    }

    /// Returns how many calls were made to this provider.
    pub fn call_count(&self) -> usize {
        self.recorded_calls.lock().unwrap().len()
    }
}

#[async_trait]
impl LlmProvider for MockProvider {
    async fn send(
        &self,
        messages: &[Message],
        _tools: &[ToolDefinition],
    ) -> Result<LlmResponse, ProviderError> {
        // Record the call for later assertion
        self.recorded_calls
            .lock()
            .unwrap()
            .push(messages.to_vec());

        // Pop the next scripted response
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            panic!(
                "MockProvider exhausted: no more scripted responses. \
                 The agent made more LLM calls than expected."
            );
        }
        Ok(responses.remove(0))
    }
}
```

The mock records every call for later assertion. This lets you verify not just the final result but the entire conversation that led to it.

::: python Coming from Python
In Python, you might use `unittest.mock.MagicMock` or a library like `responses` to mock HTTP calls:
```python
from unittest.mock import AsyncMock
provider = AsyncMock()
provider.send.side_effect = [response1, response2]
```
The Rust approach is more explicit: you implement the trait yourself, gaining full control over the mock's behavior. There is no magic — just a struct that satisfies the trait. This is more code upfront but makes the mock's behavior completely transparent and easy to debug.
:::

## Response Builders

Constructing `LlmResponse` values by hand is verbose. A builder pattern makes tests readable:

```rust
pub struct ResponseBuilder {
    content: Vec<MessageContent>,
    stop_reason: StopReason,
    usage: Usage,
}

impl ResponseBuilder {
    pub fn new() -> Self {
        Self {
            content: Vec::new(),
            stop_reason: StopReason::EndTurn,
            usage: Usage {
                input_tokens: 100,
                output_tokens: 50,
            },
        }
    }

    pub fn text(mut self, text: &str) -> Self {
        self.content.push(MessageContent::Text(text.to_string()));
        self
    }

    pub fn tool_use(mut self, name: &str, input: serde_json::Value) -> Self {
        let id = format!("tool_{}", self.content.len());
        self.content.push(MessageContent::ToolUse {
            id,
            name: name.to_string(),
            input,
        });
        self.stop_reason = StopReason::ToolUse;
        self
    }

    pub fn stop_reason(mut self, reason: StopReason) -> Self {
        self.stop_reason = reason;
        self
    }

    pub fn build(self) -> LlmResponse {
        LlmResponse {
            content: self.content,
            stop_reason: self.stop_reason,
            usage: self.usage,
        }
    }
}

// Usage in tests becomes clean and readable:
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builder_creates_text_response() {
        let response = ResponseBuilder::new()
            .text("I'll read that file for you.")
            .build();

        assert_eq!(response.stop_reason, StopReason::EndTurn);
        assert_eq!(response.content.len(), 1);
    }

    #[test]
    fn builder_creates_tool_use_response() {
        let response = ResponseBuilder::new()
            .text("Let me read the file.")
            .tool_use("read_file", json!({"path": "src/main.rs"}))
            .build();

        assert_eq!(response.stop_reason, StopReason::ToolUse);
        assert_eq!(response.content.len(), 2);
    }
}
```

## Scripting Multi-Turn Conversations

Most interesting agent interactions involve multiple turns: the model requests a tool, you execute it, the model sees the result, and it either requests another tool or gives a final answer. Here is how you script a two-turn conversation:

```rust
#[cfg(test)]
mod multi_turn_tests {
    use super::*;
    use serde_json::json;

    fn setup_read_then_respond() -> MockProvider {
        let responses = vec![
            // Turn 1: model requests to read a file
            ResponseBuilder::new()
                .text("Let me check the file.")
                .tool_use("read_file", json!({"path": "src/main.rs"}))
                .build(),
            // Turn 2: model sees the file content and responds
            ResponseBuilder::new()
                .text("The file contains a simple main function that prints hello world.")
                .stop_reason(StopReason::EndTurn)
                .build(),
        ];
        MockProvider::new(responses)
    }

    #[tokio::test]
    async fn mock_supports_multi_turn() {
        let provider = setup_read_then_respond();

        // First call — model requests a tool
        let first = provider
            .send(&[], &[])
            .await
            .unwrap();
        assert_eq!(first.stop_reason, StopReason::ToolUse);

        // Second call — model gives final answer
        let second = provider
            .send(&[], &[])
            .await
            .unwrap();
        assert_eq!(second.stop_reason, StopReason::EndTurn);

        // Verify two calls were made
        assert_eq!(provider.call_count(), 2);
    }
}
```

## Mocking Error Conditions

Your mock should also simulate failure modes — rate limits, server errors, network timeouts. Build a variant that returns errors:

```rust
pub struct FailingProvider {
    error: ProviderError,
}

impl FailingProvider {
    pub fn rate_limited() -> Self {
        Self {
            error: ProviderError::RateLimited,
        }
    }

    pub fn server_error(msg: &str) -> Self {
        Self {
            error: ProviderError::ServerError(msg.to_string()),
        }
    }
}

#[async_trait]
impl LlmProvider for FailingProvider {
    async fn send(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
    ) -> Result<LlmResponse, ProviderError> {
        Err(match &self.error {
            ProviderError::RateLimited => ProviderError::RateLimited,
            ProviderError::ServerError(msg) => ProviderError::ServerError(msg.clone()),
            ProviderError::NetworkError(msg) => ProviderError::NetworkError(msg.clone()),
        })
    }
}

#[cfg(test)]
mod error_tests {
    use super::*;

    #[tokio::test]
    async fn handles_rate_limit() {
        let provider = FailingProvider::rate_limited();
        let result = provider.send(&[], &[]).await;
        assert!(matches!(result, Err(ProviderError::RateLimited)));
    }
}
```

This lets you test that your agentic loop retries on rate limits, reports server errors to the user, and handles network failures gracefully.

::: wild In the Wild
OpenCode uses a mock provider approach in its test suite where scripted responses are loaded from JSON files. This keeps the test code clean — the test function specifies which fixture file to load, and the mock provider serves those responses in order. Claude Code takes a similar approach but generates mock responses inline using builder functions. Both approaches work well; the choice comes down to whether you prefer fixtures-as-data (JSON files) or fixtures-as-code (builder functions).
:::

## Asserting on Recorded Calls

The mock provider records every call, letting you verify that the agentic loop sent the right messages:

```rust
#[cfg(test)]
mod call_assertion_tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn records_conversation_history() {
        let provider = MockProvider::new(vec![
            ResponseBuilder::new()
                .text("Done!")
                .stop_reason(StopReason::EndTurn)
                .build(),
        ]);

        let messages = vec![Message {
            role: "user".to_string(),
            content: MessageContent::Text("Hello".to_string()),
        }];

        provider.send(&messages, &[]).await.unwrap();

        let calls = provider.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].len(), 1);
    }
}
```

## Key Takeaways

- Put the LLM provider behind a trait so you can swap in a mock implementation that returns scripted responses from a queue
- Use a builder pattern to construct mock responses — this keeps test code readable even for multi-turn conversations with tool calls
- Record every call the mock receives so you can assert on the full conversation history, not just the final result
- Build separate mock variants for error conditions (rate limits, server errors, network failures) to test your retry and error handling logic
- The mock provider is the single most important testing tool in your agent's test suite — it turns non-deterministic integration tests into fast, repeatable ones
