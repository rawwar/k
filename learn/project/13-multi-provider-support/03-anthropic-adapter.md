---
title: Anthropic Adapter
description: Implementing the provider trait for Anthropic's Messages API, handling system prompts, tool use blocks, streaming events, and Claude-specific features like cache control.
---

# Anthropic Adapter

> **What you'll learn:**
> - How to map the generic provider trait to Anthropic's Messages API request and response format
> - How to handle Anthropic-specific features like system prompts, tool_use content blocks, and cache control
> - Techniques for parsing Anthropic's SSE streaming events into the common streaming interface

The Anthropic adapter is the first concrete implementation of your `Provider` trait. If you have been building the agent since Chapter 2, you already have much of this code -- but it was hardwired into the agent. Now you will restructure it as a clean adapter that translates between your provider-neutral types and Anthropic's Messages API format.

## The Anthropic Request Format

Anthropic's Messages API has a distinctive structure that differs from other providers in several ways. The system prompt is a top-level field, not a message in the conversation. Tool definitions use a specific schema. Content blocks are typed objects within each message.

Start by defining the Anthropic-specific request types in `src/provider/anthropic.rs`:

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::provider::{
    Provider, ProviderError, StreamHandle,
    types::*,
};

/// Anthropic Messages API request body.
#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<AnthropicTool>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: Vec<AnthropicContent>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum AnthropicContent {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

#[derive(Debug, Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}
```

These types mirror Anthropic's API specification. The key thing to notice is how similar they look to the provider-neutral types you defined earlier -- that is intentional. Anthropic's content block model inspired the generic types. The translation is straightforward but necessary to keep the adapter boundary clean.

## Translation Layer: Generic to Anthropic

The adapter needs functions to convert your provider-neutral types into Anthropic's format:

```rust
fn to_anthropic_messages(messages: &[Message]) -> Vec<AnthropicMessage> {
    messages.iter().map(|msg| {
        let role = match msg.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "user", // Anthropic doesn't allow system role in messages
        };

        let content = msg.content.iter().map(|block| {
            match block {
                ContentBlock::Text { text } => {
                    AnthropicContent::Text { text: text.clone() }
                }
                ContentBlock::ToolUse { id, name, input } => {
                    AnthropicContent::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    }
                }
                ContentBlock::ToolResult { tool_use_id, content, is_error } => {
                    AnthropicContent::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        content: content.clone(),
                        is_error: if *is_error { Some(true) } else { None },
                    }
                }
            }
        }).collect();

        AnthropicMessage {
            role: role.to_string(),
            content,
        }
    }).collect()
}

fn to_anthropic_tools(tools: &[ToolDefinition]) -> Vec<AnthropicTool> {
    tools.iter().map(|tool| {
        AnthropicTool {
            name: tool.name.clone(),
            description: tool.description.clone(),
            input_schema: tool.parameters.clone(),
        }
    }).collect()
}
```

Notice the `System` role handling -- Anthropic does not allow a "system" role in the messages array. The system prompt goes in the top-level `system` field of the request. If your generic messages include system-role entries, the adapter maps them to user messages. This is the kind of provider-specific quirk that the adapter layer absorbs.

## The Adapter Struct and Trait Implementation

```rust
pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            base_url: "https://api.anthropic.com".to_string(),
        }
    }

    /// Create with a custom base URL (useful for testing with mock servers).
    pub fn with_base_url(api_key: String, model: String, base_url: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            base_url,
        }
    }
}
```

Now the trait implementation:

```rust
#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn send_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
        max_tokens: u32,
    ) -> Result<ProviderResponse, ProviderError> {
        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens,
            system: system.to_string(),
            messages: to_anthropic_messages(messages),
            tools: to_anthropic_tools(tools),
            stream: false,
        };

        let response = self.client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status().as_u16();
        if status == 429 {
            let retry_after = response.headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .map(|s| s * 1000);
            return Err(ProviderError::RateLimited {
                retry_after_ms: retry_after,
            });
        }

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api {
                status,
                message: body,
                retryable: status >= 500,
            });
        }

        let body: AnthropicResponse = response.json().await
            .map_err(|e| ProviderError::Serialization(e.to_string()))?;

        Ok(parse_anthropic_response(body))
    }

    async fn stream_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
        max_tokens: u32,
    ) -> Result<StreamHandle, ProviderError> {
        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens,
            system: system.to_string(),
            messages: to_anthropic_messages(messages),
            tools: to_anthropic_tools(tools),
            stream: true,
        };

        let response = self.client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api {
                status,
                message: body,
                retryable: status >= 500,
            });
        }

        let (tx, rx) = mpsc::channel(100);

        // Spawn a task to read the SSE stream and send events
        let byte_stream = response.bytes_stream();
        tokio::spawn(async move {
            if let Err(e) = process_anthropic_stream(byte_stream, tx.clone()).await {
                let _ = tx.send(StreamEvent::Error(e.to_string())).await;
            }
        });

        Ok(StreamHandle { receiver: rx })
    }
}
```

::: python Coming from Python
In Python, you might use the official `anthropic` SDK:
```python
import anthropic

client = anthropic.Anthropic(api_key="...")
response = client.messages.create(
    model="claude-sonnet-4-20250514",
    max_tokens=4096,
    messages=[{"role": "user", "content": "Hello"}],
)
```
The SDK handles serialization and error handling for you. In Rust, you build the HTTP requests directly with `reqwest`, giving you full control over headers, timeouts, and error classification -- which you need for the fallback chain system.
:::

## Parsing the Response

The non-streaming response needs to be translated back from Anthropic's format to your provider-neutral types:

```rust
#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    model: String,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
    #[serde(default)]
    cache_read_input_tokens: Option<u32>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u32>,
}

fn parse_anthropic_response(resp: AnthropicResponse) -> ProviderResponse {
    let content = resp.content.into_iter().map(|block| {
        match block {
            AnthropicContent::Text { text } => ContentBlock::Text { text },
            AnthropicContent::ToolUse { id, name, input } => {
                ContentBlock::ToolUse { id, name, input }
            }
            AnthropicContent::ToolResult { tool_use_id, content, is_error } => {
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error: is_error.unwrap_or(false),
                }
            }
        }
    }).collect();

    let stop_reason = match resp.stop_reason.as_deref() {
        Some("end_turn") => StopReason::EndTurn,
        Some("tool_use") => StopReason::ToolUse,
        Some("max_tokens") => StopReason::MaxTokens,
        Some("stop_sequence") => StopReason::StopSequence,
        Some(other) => StopReason::Unknown(other.to_string()),
        None => StopReason::Unknown("none".to_string()),
    };

    ProviderResponse {
        content,
        usage: TokenUsage {
            input_tokens: resp.usage.input_tokens,
            output_tokens: resp.usage.output_tokens,
            cache_read_tokens: resp.usage.cache_read_input_tokens,
            cache_creation_tokens: resp.usage.cache_creation_input_tokens,
        },
        model: resp.model,
        stop_reason,
    }
}
```

## Streaming: Processing SSE Events

Anthropic's streaming API sends Server-Sent Events (SSE). Each event has a type and a JSON data payload. The key events are:

- `message_start` -- contains the initial message metadata
- `content_block_start` -- signals a new text or tool_use block
- `content_block_delta` -- carries incremental content (text deltas or tool input deltas)
- `content_block_stop` -- marks the end of a content block
- `message_delta` -- contains the stop reason and final usage
- `message_stop` -- signals the end of the stream

```rust
use futures_util::StreamExt;

async fn process_anthropic_stream(
    mut byte_stream: impl futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
    tx: mpsc::Sender<StreamEvent>,
) -> Result<(), ProviderError> {
    let mut buffer = String::new();

    while let Some(chunk) = byte_stream.next().await {
        let chunk = chunk.map_err(|e| ProviderError::StreamError(e.to_string()))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        // Process complete SSE events from the buffer
        while let Some(event) = extract_sse_event(&mut buffer) {
            if let Some(stream_event) = parse_sse_to_stream_event(&event)? {
                if tx.send(stream_event).await.is_err() {
                    return Ok(()); // Receiver dropped, stop processing
                }
            }
        }
    }

    Ok(())
}

/// Extract a complete SSE event from the buffer, removing it from the front.
fn extract_sse_event(buffer: &mut String) -> Option<SseEvent> {
    let end = buffer.find("\n\n")?;
    let event_text: String = buffer.drain(..end + 2).collect();

    let mut event_type = String::new();
    let mut data = String::new();

    for line in event_text.lines() {
        if let Some(value) = line.strip_prefix("event: ") {
            event_type = value.to_string();
        } else if let Some(value) = line.strip_prefix("data: ") {
            data = value.to_string();
        }
    }

    Some(SseEvent { event_type, data })
}

struct SseEvent {
    event_type: String,
    data: String,
}

fn parse_sse_to_stream_event(event: &SseEvent) -> Result<Option<StreamEvent>, ProviderError> {
    match event.event_type.as_str() {
        "content_block_start" => {
            let parsed: serde_json::Value = serde_json::from_str(&event.data)
                .map_err(|e| ProviderError::Serialization(e.to_string()))?;

            if let Some(block) = parsed.get("content_block") {
                match block.get("type").and_then(|t| t.as_str()) {
                    Some("tool_use") => {
                        let id = block["id"].as_str().unwrap_or("").to_string();
                        let name = block["name"].as_str().unwrap_or("").to_string();
                        Ok(Some(StreamEvent::ToolUseStart { id, name }))
                    }
                    _ => Ok(None), // Text blocks don't need a start event
                }
            } else {
                Ok(None)
            }
        }
        "content_block_delta" => {
            let parsed: serde_json::Value = serde_json::from_str(&event.data)
                .map_err(|e| ProviderError::Serialization(e.to_string()))?;

            if let Some(delta) = parsed.get("delta") {
                match delta.get("type").and_then(|t| t.as_str()) {
                    Some("text_delta") => {
                        let text = delta["text"].as_str().unwrap_or("").to_string();
                        Ok(Some(StreamEvent::TextDelta(text)))
                    }
                    Some("input_json_delta") => {
                        let json = delta["partial_json"].as_str().unwrap_or("").to_string();
                        Ok(Some(StreamEvent::ToolInputDelta(json)))
                    }
                    _ => Ok(None),
                }
            } else {
                Ok(None)
            }
        }
        "content_block_stop" => Ok(Some(StreamEvent::ToolUseEnd)),
        "message_delta" => {
            let parsed: serde_json::Value = serde_json::from_str(&event.data)
                .map_err(|e| ProviderError::Serialization(e.to_string()))?;

            let stop_reason = parsed["delta"]["stop_reason"]
                .as_str()
                .map(|s| match s {
                    "end_turn" => StopReason::EndTurn,
                    "tool_use" => StopReason::ToolUse,
                    "max_tokens" => StopReason::MaxTokens,
                    other => StopReason::Unknown(other.to_string()),
                })
                .unwrap_or(StopReason::Unknown("none".to_string()));

            // Extract usage from message_delta if present
            if let Some(usage) = parsed.get("usage") {
                let token_usage = TokenUsage {
                    input_tokens: usage["input_tokens"].as_u64().unwrap_or(0) as u32,
                    output_tokens: usage["output_tokens"].as_u64().unwrap_or(0) as u32,
                    ..Default::default()
                };
                let _ = &tx; // usage is sent as a separate event
                // We combine stop reason and usage in the Done event
            }

            Ok(Some(StreamEvent::Done { stop_reason }))
        }
        _ => Ok(None),
    }
}
```

::: wild In the Wild
Claude Code uses Anthropic's streaming API extensively for real-time rendering of tool calls and text output. The streaming parser must handle edge cases like partial JSON in tool input deltas, where the full JSON is only valid after all chunks have been concatenated. Production agents typically buffer tool input deltas and only parse the complete JSON when the `content_block_stop` event arrives.
:::

## Key Takeaways

- The Anthropic adapter translates between your provider-neutral types and Anthropic's Messages API format, absorbing quirks like system prompts being a top-level field rather than a message
- Separate Anthropic-specific serde types from your generic types -- the translation functions between them are the adapter boundary
- Error classification during HTTP response handling feeds directly into the fallback chain: 429 becomes `RateLimited`, 5xx becomes retryable `Api` errors
- Streaming requires parsing SSE events line by line, mapping Anthropic's `content_block_delta` events into your `StreamEvent` enum
- The `with_base_url` constructor enables testing against mock servers, which you will use in the testing subchapter
