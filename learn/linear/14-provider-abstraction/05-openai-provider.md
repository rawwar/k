---
title: OpenAI Provider
description: Implement an OpenAI provider adapter covering the Chat Completions API, function calling, and streaming differences from Anthropic.
---

# OpenAI Provider

> **What you'll learn:**
> - How to map OpenAI's Chat Completions API (roles, function_call, tool_calls) onto your unified provider interface
> - The key structural differences between OpenAI and Anthropic message formats and how the adapter normalizes them
> - How to handle OpenAI-specific streaming chunks, including partial function call arguments and finish_reason signals

With the Anthropic adapter complete, you now build the OpenAI adapter. OpenAI's Chat Completions API differs from Anthropic's Messages API in several structural ways — message roles work differently, tool calls are a separate field rather than content blocks, and the streaming chunk format has its own shape. The adapter's job is to hide all of these differences behind the same `Provider` trait.

## Key Differences from Anthropic

Before writing code, understand the major structural differences you need to bridge:

| Aspect | Anthropic | OpenAI |
|--------|-----------|--------|
| System prompt | Separate `system` parameter | A message with `role: "system"` |
| Content format | Array of content blocks | String or array of parts |
| Tool calls | Content blocks with `type: "tool_use"` | Separate `tool_calls` array on assistant message |
| Tool results | Content block with `type: "tool_result"` | Message with `role: "tool"` |
| Stop signal | `stop_reason` field | `finish_reason` field |
| Streaming | SSE with typed events | SSE with `delta` objects |

These are exactly the kinds of differences your adapter must absorb.

## The OpenAiProvider Struct

```rust
use reqwest::Client;

pub struct OpenAiProvider {
    client: Client,
    api_key: String,
    model: String,
    capabilities: ModelCapabilities,
    base_url: String,
}

impl OpenAiProvider {
    pub fn new(api_key: String, model: String) -> Self {
        let capabilities = Self::capabilities_for_model(&model);
        Self {
            client: Client::new(),
            api_key,
            model,
            capabilities,
            base_url: "https://api.openai.com".to_string(),
        }
    }

    /// Allow custom base URL for Azure OpenAI or compatible APIs.
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }

    fn capabilities_for_model(model: &str) -> ModelCapabilities {
        match model {
            m if m.starts_with("gpt-4o") => ModelCapabilities {
                supports_tools: true,
                supports_streaming: true,
                supports_vision: true,
                supports_extended_thinking: false,
                supports_prompt_caching: false,
                max_context_tokens: 128_000,
                max_output_tokens: 16_384,
            },
            m if m.starts_with("gpt-4-turbo") => ModelCapabilities {
                supports_tools: true,
                supports_streaming: true,
                supports_vision: true,
                supports_extended_thinking: false,
                supports_prompt_caching: false,
                max_context_tokens: 128_000,
                max_output_tokens: 4_096,
            },
            m if m.starts_with("o1") || m.starts_with("o3") => ModelCapabilities {
                supports_tools: true,
                supports_streaming: true,
                supports_vision: true,
                supports_extended_thinking: false,
                supports_prompt_caching: false,
                max_context_tokens: 200_000,
                max_output_tokens: 100_000,
            },
            _ => ModelCapabilities {
                supports_tools: true,
                supports_streaming: true,
                supports_vision: false,
                supports_extended_thinking: false,
                supports_prompt_caching: false,
                max_context_tokens: 16_000,
                max_output_tokens: 4_096,
            },
        }
    }
}
```

Note the `with_base_url` builder method. OpenAI-compatible APIs (Azure OpenAI, Together AI, Groq) use the same request format but different endpoints. By making the base URL configurable, your OpenAI adapter doubles as an adapter for any OpenAI-compatible service.

## OpenAI-Specific Request Types

OpenAI's message format differs significantly from your canonical types. Here are the serde types that match the Chat Completions API:

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(crate) struct OpenAiRequest {
    pub model: String,
    pub messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OpenAiToolDef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

#[derive(Serialize)]
pub(crate) struct OpenAiMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct OpenAiToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: OpenAiFunctionCall,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct OpenAiFunctionCall {
    pub name: String,
    pub arguments: String,  // JSON string, not parsed object
}

#[derive(Serialize)]
pub(crate) struct OpenAiToolDef {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OpenAiFunctionDef,
}

#[derive(Serialize)]
pub(crate) struct OpenAiFunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}
```

Notice the critical difference: OpenAI's `tool_calls` use `arguments` as a JSON **string**, not a parsed JSON object. When OpenAI streams tool calls, it sends the arguments as partial JSON strings that you must concatenate. Anthropic sends parsed JSON objects. This is a subtle but important distinction that the adapter must handle.

::: python Coming from Python
In Python's `openai` library, tool call arguments come as a string that you must `json.loads()` yourself. Anthropic's Python library gives you a parsed dict. Both behaviors exist in Rust too — OpenAI's adapter must deserialize the arguments string with `serde_json::from_str`, while Anthropic's adapter receives a `serde_json::Value` directly.
:::

## Request Translation

The biggest translation challenge is converting your canonical `Message` types into OpenAI's format, where system prompts are messages, tool results are separate messages, and tool calls are a field on assistant messages:

```rust
impl OpenAiProvider {
    fn build_request_body(&self, request: &ChatRequest, stream: bool) -> OpenAiRequest {
        let mut messages = Vec::new();

        // OpenAI uses a system message, not a separate parameter
        if let Some(system) = &request.system_prompt {
            messages.push(OpenAiMessage {
                role: "system".to_string(),
                content: Some(system.clone()),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        // Translate each canonical message
        for msg in &request.messages {
            match msg.role {
                Role::User => {
                    // Collect text content from the message
                    let text: String = msg.content.iter()
                        .filter_map(|b| match b {
                            ContentBlock::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    messages.push(OpenAiMessage {
                        role: "user".to_string(),
                        content: Some(text),
                        tool_calls: None,
                        tool_call_id: None,
                    });

                    // Tool results in user messages become separate "tool" messages
                    for block in &msg.content {
                        if let ContentBlock::ToolResult { tool_use_id, content, .. } = block {
                            messages.push(OpenAiMessage {
                                role: "tool".to_string(),
                                content: Some(content.clone()),
                                tool_calls: None,
                                tool_call_id: Some(tool_use_id.clone()),
                            });
                        }
                    }
                }
                Role::Assistant => {
                    // Collect text content
                    let text: String = msg.content.iter()
                        .filter_map(|b| match b {
                            ContentBlock::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("");

                    // Collect tool use blocks into OpenAI's tool_calls format
                    let tool_calls: Vec<OpenAiToolCall> = msg.content.iter()
                        .filter_map(|b| match b {
                            ContentBlock::ToolUse { id, name, input } => {
                                Some(OpenAiToolCall {
                                    id: id.clone(),
                                    call_type: "function".to_string(),
                                    function: OpenAiFunctionCall {
                                        name: name.clone(),
                                        arguments: serde_json::to_string(input)
                                            .unwrap_or_default(),
                                    },
                                })
                            }
                            _ => None,
                        })
                        .collect();

                    messages.push(OpenAiMessage {
                        role: "assistant".to_string(),
                        content: if text.is_empty() { None } else { Some(text) },
                        tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
                        tool_call_id: None,
                    });
                }
                Role::System => {
                    // Already handled above
                }
            }
        }

        let tools = request.tools.as_ref().map(|tools| {
            tools.iter().map(|t| OpenAiToolDef {
                tool_type: "function".to_string(),
                function: OpenAiFunctionDef {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.input_schema.clone(),
                },
            }).collect()
        });

        OpenAiRequest {
            model: request.model.clone(),
            messages,
            max_tokens: Some(request.max_tokens),
            temperature: request.temperature,
            tools,
            stream: if stream { Some(true) } else { None },
        }
    }
}
```

The translation has several interesting aspects. Tool use blocks, which are inline content blocks in your canonical format and in Anthropic's format, become a separate `tool_calls` field on the OpenAI assistant message. Tool results, which are content blocks inside user messages in your format, become standalone messages with `role: "tool"` in OpenAI's format. And the system prompt, a parameter in Anthropic's API, becomes the first message with `role: "system"`.

## Response Translation

Translating back from OpenAI's response format requires handling the `tool_calls` array:

```rust
#[derive(Deserialize)]
pub(crate) struct OpenAiResponse {
    pub choices: Vec<OpenAiChoice>,
    pub model: String,
    pub usage: Option<OpenAiUsage>,
}

#[derive(Deserialize)]
pub(crate) struct OpenAiChoice {
    pub message: OpenAiResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct OpenAiResponseMessage {
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Deserialize)]
pub(crate) struct OpenAiUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl OpenAiProvider {
    fn translate_response(&self, response: OpenAiResponse) -> ChatResponse {
        let choice = &response.choices[0];
        let mut content = Vec::new();

        // Add text content if present
        if let Some(text) = &choice.message.content {
            if !text.is_empty() {
                content.push(ContentBlock::Text { text: text.clone() });
            }
        }

        // Convert tool_calls to ContentBlock::ToolUse
        if let Some(tool_calls) = &choice.message.tool_calls {
            for tc in tool_calls {
                let input: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                content.push(ContentBlock::ToolUse {
                    id: tc.id.clone(),
                    name: tc.function.name.clone(),
                    input,
                });
            }
        }

        let stop_reason = match choice.finish_reason.as_deref() {
            Some("tool_calls") => StopReason::ToolUse,
            Some("length") => StopReason::MaxTokens,
            Some("stop") => StopReason::EndTurn,
            _ => StopReason::EndTurn,
        };

        let usage = response.usage.map(|u| Usage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
            cache_read_tokens: None,
            cache_write_tokens: None,
        }).unwrap_or_default();

        ChatResponse { content, model: response.model, usage, stop_reason }
    }
}
```

Pay attention to the `serde_json::from_str(&tc.function.arguments)` call — this is where the OpenAI adapter parses the JSON arguments string into a `serde_json::Value` to match your canonical `ToolUse` format. If the JSON is malformed, it falls back to an empty object rather than failing the entire response.

## Streaming Implementation

OpenAI's streaming format uses SSE events where each `data:` line contains a JSON chunk with a `delta` object. The delta accumulates tool call arguments piece by piece:

```rust
impl OpenAiProvider {
    async fn stream_message_impl(
        &self,
        request: ChatRequest,
    ) -> Result<StreamResult, ProviderError> {
        let api_request = self.build_request_body(&request, true);

        let response = self.client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&api_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api {
                status: status.as_u16(),
                message: body,
            });
        }

        let byte_stream = response.bytes_stream();

        let stream = async_stream::stream! {
            let mut buffer = String::new();
            let mut current_tool_calls: Vec<PartialToolCall> = Vec::new();
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

                while let Some(line_end) = buffer.find('\n') {
                    let line = buffer[..line_end].trim().to_string();
                    buffer = buffer[line_end + 1..].to_string();

                    if line == "data: [DONE]" {
                        // Emit final tool calls if any were accumulated
                        for tc in &current_tool_calls {
                            yield Ok(StreamEvent::ToolInputDelta(String::new()));
                        }
                        yield Ok(StreamEvent::Done {
                            usage: Usage::default(),
                            stop_reason: StopReason::EndTurn,
                        });
                        return;
                    }

                    if let Some(json_str) = line.strip_prefix("data: ") {
                        if let Ok(chunk) = serde_json::from_str::<OpenAiStreamChunk>(json_str) {
                            if let Some(delta) = &chunk.choices.first()
                                .and_then(|c| c.delta.as_ref())
                            {
                                // Handle text content
                                if let Some(content) = &delta.content {
                                    yield Ok(StreamEvent::TextDelta(content.clone()));
                                }

                                // Handle tool calls
                                if let Some(tool_calls) = &delta.tool_calls {
                                    for tc_delta in tool_calls {
                                        let idx = tc_delta.index as usize;
                                        // New tool call starting
                                        if let Some(id) = &tc_delta.id {
                                            let name = tc_delta.function.as_ref()
                                                .map(|f| f.name.clone().unwrap_or_default())
                                                .unwrap_or_default();
                                            yield Ok(StreamEvent::ToolUseStart {
                                                id: id.clone(),
                                                name,
                                            });
                                        }
                                        // Accumulate arguments
                                        if let Some(func) = &tc_delta.function {
                                            if let Some(args) = &func.arguments {
                                                yield Ok(StreamEvent::ToolInputDelta(
                                                    args.clone()
                                                ));
                                            }
                                        }
                                    }
                                }
                            }

                            // Check for finish reason
                            if let Some(reason) = chunk.choices.first()
                                .and_then(|c| c.finish_reason.as_deref())
                            {
                                let stop = match reason {
                                    "tool_calls" => StopReason::ToolUse,
                                    "length" => StopReason::MaxTokens,
                                    _ => StopReason::EndTurn,
                                };
                                let usage = chunk.usage.map(|u| Usage {
                                    input_tokens: u.prompt_tokens.unwrap_or(0),
                                    output_tokens: u.completion_tokens.unwrap_or(0),
                                    ..Default::default()
                                }).unwrap_or_default();
                                yield Ok(StreamEvent::Done {
                                    usage,
                                    stop_reason: stop,
                                });
                            }
                        }
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}
```

The streaming delta types that match OpenAI's format:

```rust
#[derive(Deserialize)]
pub(crate) struct OpenAiStreamChunk {
    pub choices: Vec<OpenAiStreamChoice>,
    pub usage: Option<OpenAiStreamUsage>,
}

#[derive(Deserialize)]
pub(crate) struct OpenAiStreamChoice {
    pub delta: Option<OpenAiStreamDelta>,
    pub finish_reason: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct OpenAiStreamDelta {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<OpenAiStreamToolCall>>,
}

#[derive(Deserialize)]
pub(crate) struct OpenAiStreamToolCall {
    pub index: u32,
    pub id: Option<String>,
    pub function: Option<OpenAiStreamFunction>,
}

#[derive(Deserialize)]
pub(crate) struct OpenAiStreamFunction {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct OpenAiStreamUsage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
}
```

::: wild In the Wild
OpenCode, being written in Go, implements both Anthropic and OpenAI providers behind a common interface. Its OpenAI adapter handles the same structural differences — tool calls as a separate array, arguments as JSON strings, system messages instead of a system parameter. The key insight from production agents is that the adapter layer needs thorough test coverage, because subtle format differences cause failures that only appear with specific conversation patterns (like multi-tool-call responses).
:::

## Key Takeaways

- OpenAI's Chat Completions API structures messages differently from Anthropic: system prompts are messages, tool calls are a separate field, and tool results are standalone messages with `role: "tool"`.
- The adapter converts between canonical content blocks and OpenAI's `tool_calls` array, including parsing the `arguments` JSON string into `serde_json::Value` for your unified format.
- OpenAI streaming sends partial function arguments as incremental strings that must be concatenated — the adapter maps these to `StreamEvent::ToolInputDelta` events.
- The configurable `base_url` lets the same adapter work with OpenAI, Azure OpenAI, and any OpenAI-compatible API endpoint.
- Both stop reason naming (`stop` vs `end_turn`, `length` vs `max_tokens`) and token count field names (`prompt_tokens` vs `input_tokens`) differ between providers. The adapter normalizes everything to your canonical enums and structs.
