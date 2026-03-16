---
title: Anthropic Provider
description: Implement a complete Anthropic provider adapter supporting the Messages API, streaming, tool use, and extended thinking.
---

# Anthropic Provider

> **What you'll learn:**
> - How to implement the provider trait for Anthropic's Messages API, handling content blocks, tool use blocks, and multi-turn conversations
> - Techniques for supporting Anthropic-specific features like extended thinking, prompt caching, and beta headers through the adapter layer
> - How to handle Anthropic's streaming format (server-sent events with content_block_delta) and map it to your unified stream type

With the adapter pattern established, let's build the first concrete provider: Anthropic. This is likely the provider your agent already talks to, so the adapter needs to be thorough — covering not just basic message sending but streaming, tool use, and Anthropic-specific features like extended thinking.

## The AnthropicProvider Struct

The provider struct holds everything needed to make API calls:

```rust
use reqwest::Client;

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    model: String,
    capabilities: ModelCapabilities,
    base_url: String,
    /// Additional headers for beta features.
    beta_headers: Vec<String>,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String) -> Self {
        let capabilities = Self::capabilities_for_model(&model);
        let beta_headers = Self::beta_headers_for_model(&model);

        Self {
            client: Client::new(),
            api_key,
            model,
            capabilities,
            base_url: "https://api.anthropic.com".to_string(),
            beta_headers,
        }
    }

    fn capabilities_for_model(model: &str) -> ModelCapabilities {
        match model {
            m if m.starts_with("claude-sonnet-4") => ModelCapabilities {
                supports_tools: true,
                supports_streaming: true,
                supports_vision: true,
                supports_extended_thinking: true,
                supports_prompt_caching: true,
                max_context_tokens: 200_000,
                max_output_tokens: 16_384,
            },
            m if m.starts_with("claude-3-5-haiku") => ModelCapabilities {
                supports_tools: true,
                supports_streaming: true,
                supports_vision: false,
                supports_extended_thinking: false,
                supports_prompt_caching: true,
                max_context_tokens: 200_000,
                max_output_tokens: 8_192,
            },
            _ => ModelCapabilities {
                supports_tools: true,
                supports_streaming: true,
                supports_vision: true,
                supports_extended_thinking: false,
                supports_prompt_caching: false,
                max_context_tokens: 200_000,
                max_output_tokens: 4_096,
            },
        }
    }

    fn beta_headers_for_model(model: &str) -> Vec<String> {
        let mut headers = Vec::new();
        // Extended thinking requires a beta header
        if model.starts_with("claude-sonnet-4") || model.starts_with("claude-3-7") {
            headers.push("interleaved-thinking-2025-05-14".to_string());
        }
        headers
    }
}
```

The constructor inspects the model name to determine capabilities and required beta headers. This is pragmatic — the model name is the only information available at construction time, and different Claude models have different feature sets.

## Implementing send_message

The non-streaming implementation translates your canonical request, sends it to Anthropic, and translates the response back:

```rust
#[async_trait::async_trait]
impl Provider for AnthropicProvider {
    async fn send_message(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let api_request = self.build_request_body(&request, false);

        let mut http_request = self.client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json");

        // Add beta headers if needed
        for beta in &self.beta_headers {
            http_request = http_request.header("anthropic-beta", beta);
        }

        let http_response = http_request
            .json(&api_request)
            .send()
            .await?;

        let status = http_response.status();
        if !status.is_success() {
            return Err(self.handle_error_response(status, http_response).await);
        }

        let api_response: AnthropicResponse = http_response.json().await?;
        Ok(self.translate_response(api_response))
    }

    fn capabilities(&self) -> &ModelCapabilities {
        &self.capabilities
    }

    fn name(&self) -> &str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.model
    }

    // stream_message shown below
    # async fn stream_message(&self, _: ChatRequest) -> Result<StreamResult, ProviderError> { todo!() }
}
```

The error handling method classifies HTTP status codes into your error enum:

```rust
impl AnthropicProvider {
    async fn handle_error_response(
        &self,
        status: reqwest::StatusCode,
        response: reqwest::Response,
    ) -> ProviderError {
        let body = response.text().await.unwrap_or_default();

        match status.as_u16() {
            401 => ProviderError::Auth("Invalid Anthropic API key".into()),
            429 => {
                // Parse retry-after if available
                let retry_after_ms = 1000; // Default to 1 second
                ProviderError::RateLimited { retry_after_ms }
            }
            529 => ProviderError::Api {
                status: 529,
                message: "Anthropic API is overloaded".into(),
            },
            _ => ProviderError::Api {
                status: status.as_u16(),
                message: body,
            },
        }
    }
}
```

## Building the Request Body

The request builder handles both standard messages and Anthropic-specific features. Notice how it checks the `extensions` map for optional features:

```rust
impl AnthropicProvider {
    fn build_request_body(
        &self,
        request: &ChatRequest,
        stream: bool,
    ) -> AnthropicRequest {
        let messages: Vec<AnthropicMessage> = request.messages.iter().map(|msg| {
            AnthropicMessage {
                role: match msg.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                    Role::System => "user".to_string(),
                },
                content: msg.content.iter().map(|block| block.into()).collect(),
            }
        }).collect();

        let tools: Option<Vec<AnthropicTool>> = request.tools.as_ref().map(|tools| {
            tools.iter().map(|t| AnthropicTool {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
            }).collect()
        });

        let mut api_request = AnthropicRequest {
            model: request.model.clone(),
            max_tokens: request.max_tokens,
            messages,
            system: request.system_prompt.clone(),
            tools,
            temperature: request.temperature,
            stream,
            thinking: None,
        };

        // Handle extended thinking via extensions
        if let Some(budget) = request.extensions.get("thinking_budget") {
            if self.capabilities.supports_extended_thinking {
                if let Some(budget_val) = budget.as_u64() {
                    api_request.thinking = Some(ThinkingConfig {
                        thinking_type: "enabled".to_string(),
                        budget_tokens: budget_val as u32,
                    });
                    // Anthropic requires temperature=1 with thinking
                    api_request.temperature = Some(1.0);
                }
            }
        }

        api_request
    }
}
```

The extended thinking feature is a good example of the adapter handling provider-specific behavior. The caller sets `extensions["thinking_budget"]` without knowing whether the provider supports it. The Anthropic adapter checks its capabilities, applies the feature if supported, and also enforces Anthropic's constraint that thinking requires temperature=1.

::: python Coming from Python
In Python, you might use `**kwargs` to pass provider-specific options through a generic interface. The Rust equivalent is the `extensions: HashMap<String, Value>` field. Both approaches keep the interface clean, but Rust's version benefits from `serde_json::Value` being a strongly-typed JSON tree — you know you are working with valid JSON values, not arbitrary Python objects.
:::

## Implementing Streaming

Anthropic's streaming uses server-sent events (SSE). Each event has a `type` field that tells you what kind of data it carries. Your adapter needs to parse these events and emit your canonical `StreamEvent` values:

```rust
use futures::StreamExt;
use tokio::io::AsyncBufReadExt;

impl AnthropicProvider {
    async fn stream_message_impl(
        &self,
        request: ChatRequest,
    ) -> Result<StreamResult, ProviderError> {
        let api_request = self.build_request_body(&request, true);

        let mut http_request = self.client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json");

        for beta in &self.beta_headers {
            http_request = http_request.header("anthropic-beta", beta);
        }

        let response = http_request
            .json(&api_request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(self.handle_error_response(response.status(), response).await);
        }

        let byte_stream = response.bytes_stream();

        // Parse SSE events and map to our StreamEvent type
        let stream = async_stream::stream! {
            let mut buffer = String::new();
            let mut current_usage = Usage::default();
            let mut stop_reason = StopReason::EndTurn;

            let mut byte_stream = std::pin::pin!(byte_stream);

            while let Some(chunk_result) = byte_stream.next().await {
                let chunk = match chunk_result {
                    Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                    Err(e) => {
                        yield Err(ProviderError::Http(e));
                        return;
                    }
                };

                buffer.push_str(&chunk);

                // Process complete SSE events from the buffer
                while let Some(event) = extract_sse_event(&mut buffer) {
                    match parse_anthropic_event(&event) {
                        Some(AnthropicStreamEvent::ContentBlockStart { content_block }) => {
                            if let Some(tool) = content_block.as_tool_use() {
                                yield Ok(StreamEvent::ToolUseStart {
                                    id: tool.id.clone(),
                                    name: tool.name.clone(),
                                });
                            }
                        }
                        Some(AnthropicStreamEvent::ContentBlockDelta { delta }) => {
                            match delta {
                                Delta::TextDelta { text } => {
                                    yield Ok(StreamEvent::TextDelta(text));
                                }
                                Delta::InputJsonDelta { partial_json } => {
                                    yield Ok(StreamEvent::ToolInputDelta(partial_json));
                                }
                            }
                        }
                        Some(AnthropicStreamEvent::MessageDelta { delta, usage }) => {
                            if let Some(reason) = delta.stop_reason {
                                stop_reason = match reason.as_str() {
                                    "tool_use" => StopReason::ToolUse,
                                    "max_tokens" => StopReason::MaxTokens,
                                    _ => StopReason::EndTurn,
                                };
                            }
                            if let Some(output_tokens) = usage.output_tokens {
                                current_usage.output_tokens = output_tokens;
                            }
                        }
                        Some(AnthropicStreamEvent::MessageStart { message }) => {
                            current_usage.input_tokens = message.usage.input_tokens;
                        }
                        Some(AnthropicStreamEvent::MessageStop) => {
                            yield Ok(StreamEvent::Done {
                                usage: current_usage.clone(),
                                stop_reason: stop_reason.clone(),
                            });
                        }
                        None => {} // Unknown event type, skip
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}
```

The SSE parsing helpers break the byte stream into discrete events:

```rust
/// Extract a single SSE event from the buffer, returning None if incomplete.
fn extract_sse_event(buffer: &mut String) -> Option<String> {
    // SSE events are separated by double newlines
    if let Some(pos) = buffer.find("\n\n") {
        let event = buffer[..pos].to_string();
        *buffer = buffer[pos + 2..].to_string();
        Some(event)
    } else {
        None
    }
}

/// Parse an SSE event string into a typed Anthropic event.
fn parse_anthropic_event(raw: &str) -> Option<AnthropicStreamEvent> {
    let mut event_type = None;
    let mut data = None;

    for line in raw.lines() {
        if let Some(value) = line.strip_prefix("event: ") {
            event_type = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("data: ") {
            data = Some(value.to_string());
        }
    }

    let event_type = event_type?;
    let data = data?;

    match event_type.as_str() {
        "message_start" => serde_json::from_str(&data).ok(),
        "content_block_start" => serde_json::from_str(&data).ok(),
        "content_block_delta" => serde_json::from_str(&data).ok(),
        "message_delta" => serde_json::from_str(&data).ok(),
        "message_stop" => Some(AnthropicStreamEvent::MessageStop),
        _ => None,
    }
}
```

## Handling Tool Use in Responses

When the model decides to call a tool, Anthropic returns a `tool_use` content block with an `id`, `name`, and `input` object. Your adapter translates this into a `ContentBlock::ToolUse` that the agentic loop understands:

```rust
impl AnthropicProvider {
    fn translate_response(&self, response: AnthropicResponse) -> ChatResponse {
        let content = response.content.into_iter().map(|block| {
            match block {
                AnthropicContentBlock::Text { text } => {
                    ContentBlock::Text { text }
                }
                AnthropicContentBlock::ToolUse { id, name, input } => {
                    ContentBlock::ToolUse { id, name, input }
                }
                AnthropicContentBlock::ToolResult { tool_use_id, content, is_error } => {
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error: is_error.unwrap_or(false),
                    }
                }
            }
        }).collect();

        ChatResponse {
            content,
            model: response.model,
            usage: Usage {
                input_tokens: response.usage.input_tokens,
                output_tokens: response.usage.output_tokens,
                cache_read_tokens: response.usage.cache_read_input_tokens,
                cache_write_tokens: response.usage.cache_creation_input_tokens,
            },
            stop_reason: match response.stop_reason.as_deref() {
                Some("tool_use") => StopReason::ToolUse,
                Some("max_tokens") => StopReason::MaxTokens,
                Some("stop_sequence") => StopReason::StopSequence,
                _ => StopReason::EndTurn,
            },
        }
    }
}
```

The translation between `tool_use` block shapes is nearly one-to-one here because your canonical types were designed with Anthropic's format in mind. When we build the OpenAI adapter in the next subchapter, you will see a more complex translation where OpenAI's `tool_calls` array needs to be reshaped into content blocks.

::: wild In the Wild
Claude Code's Anthropic adapter handles several edge cases beyond basic tool use: it tracks thinking blocks separately, handles multi-turn conversations where the assistant's response contains both text and tool calls interleaved, and manages prompt caching by marking large system prompts with cache breakpoints. The Pi coding agent takes a simpler approach, focusing on text-only interactions and adding tool support as a layer above the provider.
:::

## Key Takeaways

- The Anthropic adapter is a struct holding an HTTP client, API key, model name, and pre-computed capabilities. Construction inspects the model name to determine which features are available.
- Request building translates canonical types to Anthropic-specific serde types, applying provider-specific logic like the temperature=1 constraint for extended thinking.
- Streaming parses Anthropic's SSE format into a buffered stream, mapping `content_block_delta`, `message_delta`, and other event types to your unified `StreamEvent` enum.
- Error handling classifies HTTP status codes (401, 429, 529) into your `ProviderError` variants, enabling the retry and fallback logic you will build later in this chapter.
- Provider-specific features like extended thinking and prompt caching are activated through the `extensions` map, keeping the core trait clean while still exposing advanced capabilities.
