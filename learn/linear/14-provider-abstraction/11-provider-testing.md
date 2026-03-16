---
title: Provider Testing
description: Test provider adapters thoroughly using mock servers, recorded responses, and contract tests to ensure correctness without hitting live APIs.
---

# Provider Testing

> **What you'll learn:**
> - How to build mock HTTP servers that simulate each provider's API behavior for fast, deterministic unit tests
> - Techniques for recording real API interactions and replaying them in tests to catch serialization regressions
> - How to write contract tests that verify all provider adapters satisfy the same behavioral expectations defined by the trait

Testing provider adapters is tricky. The code under test makes HTTP calls to external APIs that cost money, are slow, and can fail nondeterministically. You need testing strategies that give confidence without hitting live APIs. This subchapter covers three complementary approaches: mock providers, mock HTTP servers, and contract tests.

## Strategy 1: Mock Provider

The simplest testing approach is to build a mock that implements the `Provider` trait directly. This tests the code that uses providers without testing the adapters themselves:

```rust
use std::sync::{Arc, Mutex};

/// A mock provider that returns predetermined responses.
pub struct MockProvider {
    name: String,
    model: String,
    capabilities: ModelCapabilities,
    /// Queue of responses to return, in order.
    responses: Arc<Mutex<Vec<Result<ChatResponse, ProviderError>>>>,
    /// Record of all requests received.
    requests: Arc<Mutex<Vec<ChatRequest>>>,
}

impl MockProvider {
    pub fn new() -> Self {
        Self {
            name: "mock".into(),
            model: "mock-model".into(),
            capabilities: ModelCapabilities {
                supports_tools: true,
                supports_streaming: true,
                supports_vision: false,
                supports_extended_thinking: false,
                supports_prompt_caching: false,
                max_context_tokens: 100_000,
                max_output_tokens: 4_096,
            },
            responses: Arc::new(Mutex::new(Vec::new())),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Queue a successful response.
    pub fn with_response(self, response: ChatResponse) -> Self {
        self.responses.lock().unwrap().push(Ok(response));
        self
    }

    /// Queue an error response.
    pub fn with_error(self, error: ProviderError) -> Self {
        self.responses.lock().unwrap().push(Err(error));
        self
    }

    /// Get all requests that were sent to this mock.
    pub fn received_requests(&self) -> Vec<ChatRequest> {
        self.requests.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl Provider for MockProvider {
    async fn send_message(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        self.requests.lock().unwrap().push(request);

        self.responses
            .lock()
            .unwrap()
            .pop()
            .unwrap_or(Err(ProviderError::Other(
                "MockProvider: no more queued responses".into(),
            )))
    }

    async fn stream_message(&self, request: ChatRequest) -> Result<StreamResult, ProviderError> {
        // For mock streaming, convert a regular response into a stream
        let response = self.send_message(request).await?;

        let stream = async_stream::stream! {
            for block in &response.content {
                match block {
                    ContentBlock::Text { text } => {
                        yield Ok(StreamEvent::TextDelta(text.clone()));
                    }
                    ContentBlock::ToolUse { id, name, input } => {
                        yield Ok(StreamEvent::ToolUseStart {
                            id: id.clone(),
                            name: name.clone(),
                        });
                        yield Ok(StreamEvent::ToolInputDelta(
                            serde_json::to_string(input).unwrap_or_default()
                        ));
                    }
                    _ => {}
                }
            }
            yield Ok(StreamEvent::Done {
                usage: response.usage.clone(),
                stop_reason: response.stop_reason.clone(),
            });
        };

        Ok(Box::pin(stream))
    }

    fn capabilities(&self) -> &ModelCapabilities {
        &self.capabilities
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn model(&self) -> &str {
        &self.model
    }
}
```

Use the mock to test your agentic loop:

```rust
#[tokio::test]
async fn test_agent_sends_user_message() {
    let mock = MockProvider::new()
        .with_response(ChatResponse {
            content: vec![ContentBlock::Text {
                text: "Hello! How can I help?".into(),
            }],
            model: "mock-model".into(),
            usage: Usage {
                input_tokens: 10,
                output_tokens: 8,
                ..Default::default()
            },
            stop_reason: StopReason::EndTurn,
        });

    let requests = mock.requests.clone();
    let mut agent = Agent::new(Box::new(mock));
    let response = agent.chat("Hi there").await.unwrap();

    assert_eq!(response, "Hello! How can I help?");

    let sent_requests = requests.lock().unwrap();
    assert_eq!(sent_requests.len(), 1);
    assert_eq!(sent_requests[0].messages.len(), 1);
}
```

::: python Coming from Python
Python developers often use `unittest.mock.Mock` or `MagicMock` to stub out provider clients. The mock auto-generates methods based on what is called on it. Rust's approach is more structured: you write an explicit `MockProvider` that implements the trait, and the compiler ensures it has every required method. This is more work upfront, but you never discover at test runtime that your mock is missing a method.
:::

## Strategy 2: Mock HTTP Server

Mock providers test the code that uses the `Provider` trait, but they skip the adapter's HTTP and serialization logic entirely. To test the adapters themselves, spin up a local HTTP server that mimics the real API:

```rust
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path, header};

#[tokio::test]
async fn test_anthropic_adapter_sends_correct_headers() {
    // Start a mock HTTP server
    let mock_server = MockServer::start().await;

    // Define what the mock server should expect and respond with
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-key"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "content": [{"type": "text", "text": "Test response"}],
                    "model": "claude-sonnet-4-20250514",
                    "stop_reason": "end_turn",
                    "usage": {
                        "input_tokens": 25,
                        "output_tokens": 10
                    }
                }))
        )
        .expect(1)  // Expect exactly one matching request
        .mount(&mock_server)
        .await;

    // Create an Anthropic provider pointed at the mock server
    let mut provider = AnthropicProvider::new(
        "test-key".into(),
        "claude-sonnet-4-20250514".into(),
    );
    provider.base_url = mock_server.uri();

    // Send a request
    let request = ChatRequest {
        model: "claude-sonnet-4-20250514".into(),
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentBlock::Text { text: "Hello".into() }],
        }],
        system_prompt: Some("Be helpful".into()),
        max_tokens: 1024,
        temperature: None,
        tools: None,
        extensions: HashMap::new(),
    };

    let response = provider.send_message(request).await.unwrap();

    assert_eq!(response.usage.input_tokens, 25);
    assert_eq!(response.usage.output_tokens, 10);
    assert_eq!(response.stop_reason, StopReason::EndTurn);

    // Verify the text was extracted correctly
    let text = match &response.content[0] {
        ContentBlock::Text { text } => text.as_str(),
        _ => panic!("Expected text block"),
    };
    assert_eq!(text, "Test response");
}
```

Test error handling with different HTTP status codes:

```rust
#[tokio::test]
async fn test_anthropic_adapter_handles_rate_limit() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(429)
                .set_body_json(serde_json::json!({
                    "error": {
                        "type": "rate_limit_error",
                        "message": "Rate limit exceeded"
                    }
                }))
        )
        .mount(&mock_server)
        .await;

    let mut provider = AnthropicProvider::new("test-key".into(), "claude-sonnet-4-20250514".into());
    provider.base_url = mock_server.uri();

    let request = ChatRequest {
        model: "claude-sonnet-4-20250514".into(),
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentBlock::Text { text: "Hello".into() }],
        }],
        system_prompt: None,
        max_tokens: 1024,
        temperature: None,
        tools: None,
        extensions: HashMap::new(),
    };

    let result = provider.send_message(request).await;
    assert!(matches!(result, Err(ProviderError::RateLimited { .. })));
}

#[tokio::test]
async fn test_anthropic_adapter_handles_auth_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&mock_server)
        .await;

    let mut provider = AnthropicProvider::new("bad-key".into(), "claude-sonnet-4-20250514".into());
    provider.base_url = mock_server.uri();

    let request = ChatRequest {
        model: "claude-sonnet-4-20250514".into(),
        messages: vec![],
        system_prompt: None,
        max_tokens: 1024,
        temperature: None,
        tools: None,
        extensions: HashMap::new(),
    };

    let result = provider.send_message(request).await;
    assert!(matches!(result, Err(ProviderError::Auth(_))));
}
```

## Strategy 3: Contract Tests

Contract tests verify that every provider adapter satisfies the same behavioral expectations. You write the test once using the trait, then run it against each adapter:

```rust
/// Contract tests that every Provider implementation must pass.
/// Run against each adapter to ensure consistent behavior.
async fn provider_contract_basic_message(provider: &dyn Provider) {
    let request = ChatRequest {
        model: provider.model().to_string(),
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: "Say exactly: hello world".into(),
            }],
        }],
        system_prompt: None,
        max_tokens: 100,
        temperature: Some(0.0),
        tools: None,
        extensions: HashMap::new(),
    };

    let response = provider.send_message(request).await.unwrap();

    // Every provider must return at least one content block
    assert!(!response.content.is_empty(), "Response must have content");

    // Every provider must return a valid stop reason
    assert!(
        matches!(response.stop_reason, StopReason::EndTurn | StopReason::MaxTokens),
        "Stop reason must be EndTurn or MaxTokens"
    );

    // Every provider must return non-zero usage
    assert!(response.usage.output_tokens > 0, "Must report output tokens");
}

async fn provider_contract_tool_use(provider: &dyn Provider) {
    if !provider.capabilities().supports_tools {
        return; // Skip for providers without tool support
    }

    let request = ChatRequest {
        model: provider.model().to_string(),
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: "What is the weather in San Francisco? Use the get_weather tool.".into(),
            }],
        }],
        system_prompt: None,
        max_tokens: 1024,
        temperature: Some(0.0),
        tools: Some(vec![ToolDefinition {
            name: "get_weather".into(),
            description: "Get the current weather for a location".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": { "type": "string" }
                },
                "required": ["location"]
            }),
        }]),
        extensions: HashMap::new(),
    };

    let response = provider.send_message(request).await.unwrap();

    // Must contain a tool use block
    let has_tool_use = response.content.iter().any(|block| {
        matches!(block, ContentBlock::ToolUse { name, .. } if name == "get_weather")
    });

    assert!(has_tool_use, "Provider must return a tool use block when tools are available and the prompt requests one");

    // Stop reason must be ToolUse
    assert_eq!(response.stop_reason, StopReason::ToolUse);
}

async fn provider_contract_streaming(provider: &dyn Provider) {
    use futures::StreamExt;

    let request = ChatRequest {
        model: provider.model().to_string(),
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: "Count from 1 to 5.".into(),
            }],
        }],
        system_prompt: None,
        max_tokens: 200,
        temperature: Some(0.0),
        tools: None,
        extensions: HashMap::new(),
    };

    let mut stream = provider.stream_message(request).await.unwrap();

    let mut got_text = false;
    let mut got_done = false;

    while let Some(event) = stream.next().await {
        match event.unwrap() {
            StreamEvent::TextDelta(text) => {
                assert!(!text.is_empty(), "Text deltas must not be empty");
                got_text = true;
            }
            StreamEvent::Done { usage, .. } => {
                assert!(usage.output_tokens > 0, "Done event must include output token count");
                got_done = true;
            }
            _ => {}
        }
    }

    assert!(got_text, "Stream must emit at least one TextDelta");
    assert!(got_done, "Stream must emit a Done event");
}
```

Run these contract tests against mock servers for each adapter:

```rust
#[tokio::test]
async fn anthropic_satisfies_basic_message_contract() {
    let server = setup_anthropic_mock_server().await;
    let provider = AnthropicProvider::new("test-key".into(), "claude-sonnet-4-20250514".into());
    // Point provider at mock server...
    provider_contract_basic_message(&provider).await;
}

#[tokio::test]
async fn openai_satisfies_basic_message_contract() {
    let server = setup_openai_mock_server().await;
    let provider = OpenAiProvider::new("test-key".into(), "gpt-4o".into());
    // Point provider at mock server...
    provider_contract_basic_message(&provider).await;
}
```

## Testing the Decorator Stack

Test the retry and tracking decorators to ensure they compose correctly:

```rust
#[tokio::test]
async fn test_retry_retries_on_rate_limit() {
    let mock = MockProvider::new()
        .with_error(ProviderError::RateLimited { retry_after_ms: 10 })
        .with_error(ProviderError::RateLimited { retry_after_ms: 10 })
        .with_response(ChatResponse {
            content: vec![ContentBlock::Text { text: "Success after retries".into() }],
            model: "mock".into(),
            usage: Usage::default(),
            stop_reason: StopReason::EndTurn,
        });

    // Note: responses are popped (LIFO), so queue them in reverse order
    let retry = RetryProvider::new(
        Box::new(mock),
        RetryConfig {
            max_retries: 3,
            base_delay: Duration::from_millis(1), // Fast for tests
            max_delay: Duration::from_millis(10),
            jitter: 0.0,
        },
    );

    let request = ChatRequest {
        model: "mock".into(),
        messages: vec![],
        system_prompt: None,
        max_tokens: 100,
        temperature: None,
        tools: None,
        extensions: HashMap::new(),
    };

    let result = retry.send_message(request).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_tracking_records_usage() {
    let mock = MockProvider::new()
        .with_response(ChatResponse {
            content: vec![ContentBlock::Text { text: "Response".into() }],
            model: "mock".into(),
            usage: Usage {
                input_tokens: 100,
                output_tokens: 50,
                ..Default::default()
            },
            stop_reason: StopReason::EndTurn,
        });

    let usage = Arc::new(Mutex::new(SessionUsage::default()));
    let tracking = TrackingProvider::new(
        Box::new(mock),
        usage.clone(),
        ModelPricing {
            input_per_million: 3.0,
            output_per_million: 15.0,
            cached_input_per_million: None,
        },
    );

    let request = ChatRequest {
        model: "mock".into(),
        messages: vec![],
        system_prompt: None,
        max_tokens: 100,
        temperature: None,
        tools: None,
        extensions: HashMap::new(),
    };

    tracking.send_message(request).await.unwrap();

    let session = usage.lock().unwrap();
    assert_eq!(session.request_count, 1);
    assert_eq!(session.total_input_tokens, 100);
    assert_eq!(session.total_output_tokens, 50);
    assert!(session.total_cost > 0.0);
}
```

::: wild In the Wild
Claude Code tests its provider interactions using recorded API responses, replaying them deterministically in CI. This approach catches serialization regressions — if Anthropic adds a new field to their response format, the tests surface any deserialization failures before they hit production. OpenCode uses Go's `httptest` package for similar mock server testing of its provider adapters.
:::

## Key Takeaways

- Use three complementary testing strategies: mock providers (for agent logic), mock HTTP servers (for adapter serialization and error handling), and contract tests (for cross-adapter consistency).
- The `MockProvider` implements the `Provider` trait with queued responses and request recording, giving tests fine-grained control over provider behavior.
- The `wiremock` crate provides a mock HTTP server that validates request headers, paths, and bodies while returning controlled responses — perfect for testing that adapters send correctly formatted API requests.
- Contract tests define behavioral expectations once and run them against every adapter, ensuring that switching providers does not change the agent's observable behavior.
- Test the decorator stack (retry, tracking, fallback) with fast mock providers to verify that composition works correctly without network delays.
