---
title: Testing Providers
description: Testing provider adapters with recorded HTTP responses, mock servers, and contract tests that verify each adapter conforms to the provider trait correctly.
---

# Testing Providers

> **What you'll learn:**
> - How to record and replay HTTP responses for deterministic provider adapter testing
> - How to build mock provider servers that simulate API behavior including errors and rate limits
> - Techniques for contract testing that ensures all adapters satisfy the provider trait invariants

Provider adapters translate between your generic types and provider-specific API formats. If a translation is wrong -- a field name misspelled, a JSON structure mismatched, a streaming event misinterpreted -- the agent breaks in ways that are hard to diagnose. Testing the adapters thoroughly is not optional. This subchapter covers three testing strategies: recorded response tests, mock server tests, and contract tests.

## Strategy 1: Recorded Response Tests

The simplest testing approach uses real API responses captured once and replayed during tests. You save the JSON body from a real API call and parse it in a test to verify your adapter handles it correctly.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// A real Anthropic API response captured from a test request.
    const ANTHROPIC_RESPONSE_JSON: &str = r#"{
        "id": "msg_01XfDUDYJgAACzvnptvVoYEL",
        "type": "message",
        "role": "assistant",
        "content": [
            {
                "type": "text",
                "text": "Hello! How can I help you today?"
            }
        ],
        "model": "claude-sonnet-4-20250514",
        "stop_reason": "end_turn",
        "usage": {
            "input_tokens": 25,
            "output_tokens": 12
        }
    }"#;

    #[test]
    fn test_parse_anthropic_text_response() {
        let body: AnthropicResponse = serde_json::from_str(ANTHROPIC_RESPONSE_JSON)
            .expect("Failed to parse recorded response");

        let response = parse_anthropic_response(body);

        assert_eq!(response.content.len(), 1);
        match &response.content[0] {
            ContentBlock::Text { text } => {
                assert_eq!(text, "Hello! How can I help you today?");
            }
            _ => panic!("Expected text block"),
        }
        assert_eq!(response.stop_reason, StopReason::EndTurn);
        assert_eq!(response.usage.input_tokens, 25);
        assert_eq!(response.usage.output_tokens, 12);
        assert_eq!(response.model, "claude-sonnet-4-20250514");
    }

    /// A response that includes a tool use block.
    const ANTHROPIC_TOOL_RESPONSE_JSON: &str = r#"{
        "id": "msg_01ABC123",
        "type": "message",
        "role": "assistant",
        "content": [
            {
                "type": "text",
                "text": "I'll read that file for you."
            },
            {
                "type": "tool_use",
                "id": "toolu_01DEF456",
                "name": "read_file",
                "input": {
                    "path": "src/main.rs"
                }
            }
        ],
        "model": "claude-sonnet-4-20250514",
        "stop_reason": "tool_use",
        "usage": {
            "input_tokens": 150,
            "output_tokens": 45
        }
    }"#;

    #[test]
    fn test_parse_anthropic_tool_response() {
        let body: AnthropicResponse = serde_json::from_str(ANTHROPIC_TOOL_RESPONSE_JSON)
            .expect("Failed to parse recorded response");

        let response = parse_anthropic_response(body);

        assert_eq!(response.content.len(), 2);
        assert_eq!(response.stop_reason, StopReason::ToolUse);

        match &response.content[1] {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "toolu_01DEF456");
                assert_eq!(name, "read_file");
                assert_eq!(input["path"], "src/main.rs");
            }
            _ => panic!("Expected tool use block"),
        }
    }
}
```

Do the same for OpenAI responses:

```rust
#[cfg(test)]
mod openai_tests {
    use super::*;

    const OPENAI_RESPONSE_JSON: &str = r#"{
        "id": "chatcmpl-abc123",
        "object": "chat.completion",
        "model": "gpt-4o-2024-08-06",
        "choices": [
            {
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I assist you?"
                },
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 20,
            "completion_tokens": 8,
            "total_tokens": 28
        }
    }"#;

    #[test]
    fn test_parse_openai_text_response() {
        let body: OpenAIResponse = serde_json::from_str(OPENAI_RESPONSE_JSON)
            .expect("Failed to parse recorded response");

        let response = parse_openai_response(body);

        assert_eq!(response.content.len(), 1);
        match &response.content[0] {
            ContentBlock::Text { text } => {
                assert_eq!(text, "Hello! How can I assist you?");
            }
            _ => panic!("Expected text block"),
        }
        assert_eq!(response.stop_reason, StopReason::EndTurn);
        assert_eq!(response.usage.input_tokens, 20);
        assert_eq!(response.usage.output_tokens, 8);
    }

    const OPENAI_TOOL_RESPONSE_JSON: &str = r#"{
        "id": "chatcmpl-xyz789",
        "object": "chat.completion",
        "model": "gpt-4o-2024-08-06",
        "choices": [
            {
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [
                        {
                            "id": "call_abc123",
                            "type": "function",
                            "function": {
                                "name": "read_file",
                                "arguments": "{\"path\": \"src/main.rs\"}"
                            }
                        }
                    ]
                },
                "finish_reason": "tool_calls"
            }
        ],
        "usage": {
            "prompt_tokens": 120,
            "completion_tokens": 30,
            "total_tokens": 150
        }
    }"#;

    #[test]
    fn test_parse_openai_tool_response() {
        let body: OpenAIResponse = serde_json::from_str(OPENAI_TOOL_RESPONSE_JSON)
            .expect("Failed to parse recorded response");

        let response = parse_openai_response(body);

        assert_eq!(response.stop_reason, StopReason::ToolUse);
        assert_eq!(response.content.len(), 1);

        match &response.content[0] {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "call_abc123");
                assert_eq!(name, "read_file");
                assert_eq!(input["path"], "src/main.rs");
            }
            _ => panic!("Expected tool use block"),
        }
    }
}
```

Recorded response tests are fast, deterministic, and require no network access. They are your first line of defense against parsing regressions.

::: python Coming from Python
In Python, you might use `pytest` fixtures with JSON files:
```python
def test_parse_anthropic_response():
    with open("tests/fixtures/anthropic_tool_response.json") as f:
        data = json.load(f)
    result = parse_anthropic_response(data)
    assert result.tool_calls[0].name == "read_file"
```
The Rust approach embeds the JSON as string constants using `const` or `include_str!`, which means the test data lives next to the test code. No fixture files to keep in sync. Rust's `serde_json::from_str` is the equivalent of `json.load()`.
:::

## Strategy 2: Mock Server Tests

Recorded response tests verify parsing, but they do not test the full HTTP request-response cycle. Mock server tests spin up a local HTTP server that simulates a provider's API, letting you test headers, authentication, error codes, and streaming.

```rust
use axum::{Router, routing::post, Json, http::StatusCode, http::HeaderMap};
use tokio::net::TcpListener;

/// Create a mock Anthropic server that returns a canned response.
async fn mock_anthropic_server(
    response_body: serde_json::Value,
) -> (String, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route("/v1/messages", post(move |headers: HeaderMap, body: Json<serde_json::Value>| {
            let response = response_body.clone();
            async move {
                // Verify the API key header is present
                let api_key = headers.get("x-api-key")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");

                if api_key.is_empty() {
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(serde_json::json!({
                            "error": {"message": "Missing API key"}
                        })),
                    );
                }

                // Verify required fields in the request body
                assert!(body.get("model").is_some(), "Request missing 'model' field");
                assert!(body.get("messages").is_some(), "Request missing 'messages' field");

                (StatusCode::OK, Json(response))
            }
        }));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (base_url, handle)
}

#[tokio::test]
async fn test_anthropic_provider_sends_correct_request() {
    let response = serde_json::json!({
        "id": "msg_test",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "text", "text": "test response"}],
        "model": "claude-sonnet-4-20250514",
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 10, "output_tokens": 5}
    });

    let (base_url, _handle) = mock_anthropic_server(response).await;

    let provider = AnthropicProvider::with_base_url(
        "test-key".to_string(),
        "claude-sonnet-4-20250514".to_string(),
        base_url,
    );

    let result = provider.send_message(
        "You are a test assistant.",
        &[Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: "Hello".to_string(),
            }],
        }],
        &[],
        1024,
    ).await;

    let response = result.expect("Request should succeed");
    assert_eq!(response.content.len(), 1);
    assert_eq!(response.usage.input_tokens, 10);
}
```

### Testing Error Handling

Mock servers are especially valuable for testing error paths that are hard to trigger with real APIs:

```rust
/// Mock server that simulates rate limiting.
async fn mock_rate_limited_server() -> (String, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route("/v1/messages", post(|| async {
            (
                StatusCode::TOO_MANY_REQUESTS,
                [("retry-after", "5")],
                Json(serde_json::json!({
                    "error": {"message": "Rate limit exceeded"}
                })),
            )
        }));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (format!("http://{}", addr), handle)
}

#[tokio::test]
async fn test_rate_limit_produces_correct_error() {
    let (base_url, _handle) = mock_rate_limited_server().await;

    let provider = AnthropicProvider::with_base_url(
        "test-key".to_string(),
        "claude-sonnet-4-20250514".to_string(),
        base_url,
    );

    let result = provider.send_message(
        "test",
        &[Message {
            role: Role::User,
            content: vec![ContentBlock::Text { text: "hello".into() }],
        }],
        &[],
        1024,
    ).await;

    match result {
        Err(ProviderError::RateLimited { retry_after_ms }) => {
            assert_eq!(retry_after_ms, Some(5000));
        }
        other => panic!("Expected RateLimited error, got: {:?}", other),
    }
}
```

## Strategy 3: Contract Tests

Contract tests verify that all adapters satisfy the same behavioral contract. You write one test that works with any `Provider` implementation, then run it against each adapter:

```rust
/// Contract test: every provider must handle a basic text request.
async fn contract_text_request(provider: &dyn Provider) {
    let response = provider.send_message(
        "You are a test assistant. Respond with exactly: OK",
        &[Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: "Respond with OK".to_string(),
            }],
        }],
        &[],
        100,
    ).await
    .expect("Provider should handle basic text request");

    // Every provider must return at least one content block
    assert!(!response.content.is_empty(), "Response must have content");

    // The first block should be text
    assert!(
        matches!(&response.content[0], ContentBlock::Text { .. }),
        "First content block should be text"
    );

    // Usage must be populated
    assert!(response.usage.input_tokens > 0, "Input tokens must be > 0");
    assert!(response.usage.output_tokens > 0, "Output tokens must be > 0");

    // Stop reason must be EndTurn for a simple text request
    assert_eq!(response.stop_reason, StopReason::EndTurn);
}

/// Contract test: every provider must handle tool definitions.
async fn contract_tool_request(provider: &dyn Provider) {
    let tools = vec![ToolDefinition {
        name: "get_weather".to_string(),
        description: "Get the weather for a location.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "The city name"
                }
            },
            "required": ["location"]
        }),
    }];

    let response = provider.send_message(
        "You are a test assistant. Use the get_weather tool for any weather question.",
        &[Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: "What's the weather in Paris?".to_string(),
            }],
        }],
        &tools,
        500,
    ).await
    .expect("Provider should handle tool definitions");

    // Response should contain a tool use (if model supports it)
    let has_tool_use = response.content.iter()
        .any(|b| matches!(b, ContentBlock::ToolUse { .. }));

    if has_tool_use {
        assert_eq!(response.stop_reason, StopReason::ToolUse);

        // Verify tool use has required fields
        for block in &response.content {
            if let ContentBlock::ToolUse { id, name, input } = block {
                assert!(!id.is_empty(), "Tool use must have an ID");
                assert_eq!(name, "get_weather");
                assert!(input.get("location").is_some(), "Tool input must have location");
            }
        }
    }
}
```

Run the contract tests against mock-backed providers:

```rust
#[tokio::test]
async fn test_anthropic_contract_text() {
    let response = serde_json::json!({
        "id": "msg_test",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "text", "text": "OK"}],
        "model": "claude-sonnet-4-20250514",
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 30, "output_tokens": 1}
    });

    let (base_url, _handle) = mock_anthropic_server(response).await;
    let provider = AnthropicProvider::with_base_url(
        "test-key".into(), "claude-sonnet-4-20250514".into(), base_url,
    );

    contract_text_request(&provider).await;
}

#[tokio::test]
async fn test_openai_contract_text() {
    let response = serde_json::json!({
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "OK"},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 25, "completion_tokens": 1, "total_tokens": 26}
    });

    let (base_url, _handle) = mock_openai_server(response).await;
    let provider = OpenAIProvider::with_base_url(
        "test-key".into(), "gpt-4o".into(), base_url,
    );

    contract_text_request(&provider).await;
}
```

Contract tests catch a subtle but dangerous class of bugs: when two adapters behave differently for the same logical operation. If the Anthropic adapter sets `StopReason::EndTurn` but the OpenAI adapter sets `StopReason::Unknown("stop")`, the contract test catches the discrepancy.

::: wild In the Wild
Testing LLM integrations is a challenge every agent faces. Claude Code uses extensive test fixtures and mocked responses. OpenCode tests its provider adapters with recorded HTTP exchanges, replaying them during CI to verify that API format changes have not broken the adapters. The contract test pattern is borrowed from microservice testing, where it is used to verify that service implementations conform to an API specification.
:::

## Testing the Fallback Chain

The fallback chain deserves its own tests to verify that failures in one provider trigger fallback to the next:

```rust
#[tokio::test]
async fn test_fallback_chain_retries_on_error() {
    // First provider returns 500
    let (url1, _h1) = mock_error_server(500, "Internal error").await;
    let provider1 = AnthropicProvider::with_base_url(
        "key1".into(), "claude-sonnet-4-20250514".into(), url1,
    );

    // Second provider succeeds
    let success_response = serde_json::json!({
        "id": "chatcmpl-ok",
        "object": "chat.completion",
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "Fallback worked!"},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 10, "completion_tokens": 3, "total_tokens": 13}
    });
    let (url2, _h2) = mock_openai_server(success_response).await;
    let provider2 = OpenAIProvider::with_base_url(
        "key2".into(), "gpt-4o".into(), url2,
    );

    let chain = FallbackChain::builder()
        .add_with_retries(Arc::new(provider1), 0, 100)  // No retries
        .add(Arc::new(provider2))
        .build();

    let response = chain.send_message(
        "test",
        &[Message {
            role: Role::User,
            content: vec![ContentBlock::Text { text: "hello".into() }],
        }],
        &[],
        100,
    ).await
    .expect("Fallback chain should succeed");

    // Response should come from the second provider
    assert_eq!(response.model, "gpt-4o");
}
```

## Key Takeaways

- Recorded response tests parse saved JSON responses to verify adapter translation logic -- they are fast, deterministic, and require no network access
- Mock server tests use `axum` to simulate provider APIs including authentication, error codes, and rate limits, testing the full HTTP request-response cycle
- Contract tests define behavioral expectations that all adapters must satisfy, catching inconsistencies between adapter implementations
- Every adapter should be tested against both success responses (text, tool use) and error responses (rate limits, server errors, auth failures)
- The `with_base_url` constructor on each adapter is not just for production flexibility -- it is essential for pointing adapters at mock servers during testing
