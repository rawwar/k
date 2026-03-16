---
title: OpenAI Adapter
description: Implementing the provider trait for OpenAI's Chat Completions API, translating tool definitions, handling function calling, and adapting the streaming delta format.
---

# OpenAI Adapter

> **What you'll learn:**
> - How to translate the agent's tool definitions into OpenAI's function calling schema
> - How to map between the generic message format and OpenAI's role-based message structure
> - Techniques for reassembling OpenAI's chunked streaming deltas into complete tool calls

The OpenAI adapter is your second `Provider` trait implementation, and it immediately reveals why you need the adapter pattern. OpenAI's Chat Completions API differs from Anthropic's Messages API in nearly every structural detail: messages carry a single string or array content, tool calls live in a separate field on assistant messages, and streaming deltas use a fundamentally different chunking scheme. The adapter smooths over all of these differences.

## OpenAI's Request Format

OpenAI's chat completions API structures requests differently from Anthropic. The system prompt is a message with `role: "system"`. Tool definitions use a "function" wrapper. Tool calls and tool results have their own message roles.

Define the OpenAI-specific types in `src/provider/openai.rs`:

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::provider::{
    Provider, ProviderError, StreamHandle,
    types::*,
};

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OpenAIToolDef>,
    max_tokens: u32,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAIMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OpenAIFunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAIFunctionCall {
    name: String,
    arguments: String, // JSON string, not parsed object
}

#[derive(Debug, Serialize)]
struct OpenAIToolDef {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunctionDef,
}

#[derive(Debug, Serialize)]
struct OpenAIFunctionDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}
```

A key difference jumps out immediately: OpenAI's `function.arguments` is a JSON *string*, not a parsed JSON object. When Anthropic sends tool input, it is a structured `serde_json::Value`. OpenAI sends the same data as a serialized string that you must parse yourself. This is exactly the kind of mismatch your adapter layer absorbs.

## Translation: Generic to OpenAI Format

The message translation is more involved than the Anthropic adapter because OpenAI flattens content blocks into different message structures:

```rust
fn to_openai_messages(system: &str, messages: &[Message]) -> Vec<OpenAIMessage> {
    let mut result = Vec::new();

    // System prompt is a message in OpenAI's format
    if !system.is_empty() {
        result.push(OpenAIMessage {
            role: "system".to_string(),
            content: Some(system.to_string()),
            tool_calls: None,
            tool_call_id: None,
        });
    }

    for msg in messages {
        match msg.role {
            Role::User => {
                // Collect text blocks into a single content string
                let text: String = msg.content.iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                result.push(OpenAIMessage {
                    role: "user".to_string(),
                    content: Some(text),
                    tool_calls: None,
                    tool_call_id: None,
                });

                // Tool results become separate "tool" role messages
                for block in &msg.content {
                    if let ContentBlock::ToolResult { tool_use_id, content, .. } = block {
                        result.push(OpenAIMessage {
                            role: "tool".to_string(),
                            content: Some(content.clone()),
                            tool_calls: None,
                            tool_call_id: Some(tool_use_id.clone()),
                        });
                    }
                }
            }
            Role::Assistant => {
                let text: Option<String> = {
                    let texts: Vec<&str> = msg.content.iter()
                        .filter_map(|block| match block {
                            ContentBlock::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect();
                    if texts.is_empty() { None } else { Some(texts.join("\n")) }
                };

                let tool_calls: Vec<OpenAIToolCall> = msg.content.iter()
                    .filter_map(|block| match block {
                        ContentBlock::ToolUse { id, name, input } => {
                            Some(OpenAIToolCall {
                                id: id.clone(),
                                call_type: "function".to_string(),
                                function: OpenAIFunctionCall {
                                    name: name.clone(),
                                    arguments: serde_json::to_string(input)
                                        .unwrap_or_default(),
                                },
                            })
                        }
                        _ => None,
                    })
                    .collect();

                result.push(OpenAIMessage {
                    role: "assistant".to_string(),
                    content: text,
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
                    tool_call_id: None,
                });
            }
            Role::System => {
                result.push(OpenAIMessage {
                    role: "system".to_string(),
                    content: Some(
                        msg.content.iter()
                            .filter_map(|b| match b {
                                ContentBlock::Text { text } => Some(text.as_str()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    ),
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
        }
    }

    result
}

fn to_openai_tools(tools: &[ToolDefinition]) -> Vec<OpenAIToolDef> {
    tools.iter().map(|tool| {
        OpenAIToolDef {
            tool_type: "function".to_string(),
            function: OpenAIFunctionDef {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            },
        }
    }).collect()
}
```

The most significant translation is how tool calls and tool results are represented. In your generic format (and in Anthropic's), an assistant message can contain both text and tool_use blocks in its `content` array. OpenAI splits these: text goes in `content`, tool calls go in a separate `tool_calls` field. Similarly, tool results are separate "tool" role messages in OpenAI, not content blocks within user messages.

::: python Coming from Python
If you have used OpenAI's Python SDK, you know the message structure well:
```python
response = client.chat.completions.create(
    model="gpt-4o",
    messages=[
        {"role": "system", "content": "You are a coding assistant."},
        {"role": "user", "content": "Read main.rs"},
        {"role": "assistant", "content": None, "tool_calls": [...]},
        {"role": "tool", "tool_call_id": "call_123", "content": "file contents"},
    ],
)
```
The Rust adapter does the same structural transformation, but with compile-time guarantees. If you forget to handle the `tool_calls` field, the compiler tells you -- Python would let the missing key slide until runtime.
:::

## The Adapter Struct and Trait Implementation

```rust
pub struct OpenAIProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAIProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            base_url: "https://api.openai.com".to_string(),
        }
    }

    pub fn with_base_url(api_key: String, model: String, base_url: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            base_url,
        }
    }
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
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
        let request = OpenAIRequest {
            model: self.model.clone(),
            messages: to_openai_messages(system, messages),
            tools: to_openai_tools(tools),
            max_tokens,
            stream: false,
        };

        let response = self.client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
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

        let body: OpenAIResponse = response.json().await
            .map_err(|e| ProviderError::Serialization(e.to_string()))?;

        Ok(parse_openai_response(body))
    }

    async fn stream_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
        max_tokens: u32,
    ) -> Result<StreamHandle, ProviderError> {
        let request = OpenAIRequest {
            model: self.model.clone(),
            messages: to_openai_messages(system, messages),
            tools: to_openai_tools(tools),
            max_tokens,
            stream: true,
        };

        let response = self.client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
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
        let byte_stream = response.bytes_stream();

        tokio::spawn(async move {
            if let Err(e) = process_openai_stream(byte_stream, tx.clone()).await {
                let _ = tx.send(StreamEvent::Error(e.to_string())).await;
            }
        });

        Ok(StreamHandle { receiver: rx })
    }
}
```

## Parsing OpenAI Responses

OpenAI's response structure differs significantly from Anthropic's:

```rust
#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
    model: String,
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

fn parse_openai_response(resp: OpenAIResponse) -> ProviderResponse {
    let choice = &resp.choices[0];
    let mut content = Vec::new();

    // Text content
    if let Some(text) = &choice.message.content {
        if !text.is_empty() {
            content.push(ContentBlock::Text { text: text.clone() });
        }
    }

    // Tool calls
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
        Some("stop") => StopReason::EndTurn,
        Some("tool_calls") => StopReason::ToolUse,
        Some("length") => StopReason::MaxTokens,
        Some(other) => StopReason::Unknown(other.to_string()),
        None => StopReason::Unknown("none".to_string()),
    };

    let usage = resp.usage.map(|u| TokenUsage {
        input_tokens: u.prompt_tokens,
        output_tokens: u.completion_tokens,
        ..Default::default()
    }).unwrap_or_default();

    ProviderResponse {
        content,
        usage,
        model: resp.model,
        stop_reason,
    }
}
```

Notice the `serde_json::from_str` call to parse tool arguments. OpenAI sends them as a JSON string, so you parse them into a `serde_json::Value` during translation. If the string is malformed (which can happen with smaller models), you fall back to an empty object rather than failing the entire request.

## Streaming: Reassembling Deltas

OpenAI's streaming format sends chunks as `data: {...}` lines. Each chunk contains a `delta` with partial content. For tool calls, the function name and arguments arrive in separate chunks that you must reassemble:

```rust
use futures_util::StreamExt;
use std::collections::HashMap;

async fn process_openai_stream(
    mut byte_stream: impl futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
    tx: mpsc::Sender<StreamEvent>,
) -> Result<(), ProviderError> {
    let mut buffer = String::new();
    // Track in-progress tool calls by index
    let mut active_tools: HashMap<u32, (String, String)> = HashMap::new();

    while let Some(chunk) = byte_stream.next().await {
        let chunk = chunk.map_err(|e| ProviderError::StreamError(e.to_string()))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(line_end) = buffer.find('\n') {
            let line: String = buffer.drain(..line_end + 1).collect();
            let line = line.trim();

            if line == "data: [DONE]" {
                let _ = tx.send(StreamEvent::Done {
                    stop_reason: StopReason::EndTurn,
                }).await;
                return Ok(());
            }

            if let Some(data) = line.strip_prefix("data: ") {
                let parsed: serde_json::Value = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                if let Some(choices) = parsed["choices"].as_array() {
                    if let Some(choice) = choices.first() {
                        let delta = &choice["delta"];
                        let finish = choice["finish_reason"].as_str();

                        // Text content delta
                        if let Some(text) = delta["content"].as_str() {
                            let _ = tx.send(StreamEvent::TextDelta(
                                text.to_string()
                            )).await;
                        }

                        // Tool call deltas
                        if let Some(tool_calls) = delta["tool_calls"].as_array() {
                            for tc in tool_calls {
                                let index = tc["index"].as_u64().unwrap_or(0) as u32;

                                // Start of a new tool call
                                if let Some(id) = tc["id"].as_str() {
                                    let name = tc["function"]["name"]
                                        .as_str()
                                        .unwrap_or("")
                                        .to_string();
                                    active_tools.insert(
                                        index,
                                        (id.to_string(), name.clone()),
                                    );
                                    let _ = tx.send(StreamEvent::ToolUseStart {
                                        id: id.to_string(),
                                        name,
                                    }).await;
                                }

                                // Argument delta
                                if let Some(args) = tc["function"]["arguments"].as_str() {
                                    let _ = tx.send(StreamEvent::ToolInputDelta(
                                        args.to_string()
                                    )).await;
                                }
                            }
                        }

                        // Finish reason
                        if let Some(reason) = finish {
                            let stop = match reason {
                                "stop" => StopReason::EndTurn,
                                "tool_calls" => StopReason::ToolUse,
                                "length" => StopReason::MaxTokens,
                                other => StopReason::Unknown(other.to_string()),
                            };

                            // Close any open tool calls
                            for _ in active_tools.drain() {
                                let _ = tx.send(StreamEvent::ToolUseEnd).await;
                            }

                            let _ = tx.send(StreamEvent::Done {
                                stop_reason: stop,
                            }).await;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
```

The critical difference from Anthropic's stream is tool call reassembly. OpenAI sends tool calls indexed by position, with the `id` and `name` in the first chunk and `arguments` spread across subsequent chunks. The `active_tools` map tracks which tool calls are in progress so you can emit the right `ToolUseEnd` events when the stream finishes.

::: wild In the Wild
OpenCode supports any OpenAI-compatible API through a single adapter, since many providers (Together AI, Groq, Anyscale, local vLLM servers) implement the OpenAI chat completions format. This makes the OpenAI adapter a de facto standard integration point. By supporting this format, your agent can connect to dozens of providers with no additional code.
:::

## Key Takeaways

- OpenAI's Chat Completions API uses a fundamentally different message structure than Anthropic: system prompts are messages, tool calls are a separate field, and tool results are "tool" role messages
- The biggest translation challenge is tool call arguments: OpenAI sends them as JSON strings that must be parsed, while Anthropic sends structured JSON values
- Streaming delta reassembly requires tracking active tool calls by index, since OpenAI sends the function name and arguments in separate chunks
- The `with_base_url` constructor makes this adapter work with any OpenAI-compatible API, multiplying the number of providers you support
- Despite major structural differences, both adapters expose the identical `Provider` trait interface to the agent core
