---
title: Ollama Local Provider
description: Build an Ollama provider adapter that enables your coding agent to work with locally-hosted open-source models.
---

# Ollama Local Provider

> **What you'll learn:**
> - How to implement a provider adapter for Ollama's REST API, handling model management, generation, and chat endpoints
> - The capability limitations of local models compared to cloud providers and how to degrade gracefully when features are unsupported
> - How to handle Ollama-specific concerns like model pulling, GPU memory management, and performance tuning for agent workloads

Anthropic and OpenAI are cloud providers — you send HTTP requests over the internet and pay per token. Ollama is different. It runs models locally on your machine, talking to a REST API on `localhost:11434`. The adapter for Ollama bridges the same `Provider` trait, but the implementation must handle concerns that cloud providers do not: checking whether a model is downloaded, gracefully degrading when features like tool use are unsupported, and managing the realities of running large models on consumer hardware.

## Why Support Local Models?

Local models matter for several reasons in a coding agent:

- **Privacy**: code never leaves the developer's machine.
- **Cost**: no per-token charges, just electricity.
- **Offline usage**: works without an internet connection.
- **Experimentation**: try new open-source models the moment they release.

The trade-off is capability. Local models are generally less capable than frontier cloud models, especially for complex coding tasks. Your adapter must handle this gap gracefully.

## The OllamaProvider Struct

```rust
use reqwest::Client;

pub struct OllamaProvider {
    client: Client,
    base_url: String,
    model: String,
    capabilities: ModelCapabilities,
}

impl OllamaProvider {
    pub fn new(base_url: String, model: String) -> Self {
        let capabilities = Self::capabilities_for_model(&model);
        Self {
            client: Client::new(),
            base_url,
            model,
            capabilities,
        }
    }

    fn capabilities_for_model(model: &str) -> ModelCapabilities {
        // Local models have varying capabilities. These are reasonable defaults.
        // Tool use support depends on the specific model.
        let supports_tools = model.contains("qwen")
            || model.contains("llama3.1")
            || model.contains("llama3.2")
            || model.contains("mistral")
            || model.contains("command-r");

        let max_context = if model.contains("128k") || model.contains("qwen2.5-coder") {
            131_072
        } else if model.contains("32k") {
            32_768
        } else {
            8_192  // Conservative default for local models
        };

        ModelCapabilities {
            supports_tools,
            supports_streaming: true,
            supports_vision: model.contains("llava") || model.contains("vision"),
            supports_extended_thinking: false,
            supports_prompt_caching: false,
            max_context_tokens: max_context,
            max_output_tokens: 4_096,
        }
    }
}
```

The capability detection is heuristic-based — you infer what a model can do from its name. This is imperfect but practical. Ollama does not expose a structured capabilities endpoint, so the best you can do is maintain a mapping of known model families to their features.

## Ollama's API Format

Ollama exposes an OpenAI-compatible `/api/chat` endpoint, but its native format has some differences. Let's define the serde types:

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(crate) struct OllamaChatRequest {
    pub model: String,
    pub messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OllamaTool>>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<OllamaOptions>,
}

#[derive(Serialize)]
pub(crate) struct OllamaMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Serialize)]
pub(crate) struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_predict: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_ctx: Option<u32>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct OllamaTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OllamaFunctionDef,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct OllamaFunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct OllamaToolCall {
    pub function: OllamaFunctionCall,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct OllamaFunctionCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Deserialize)]
pub(crate) struct OllamaChatResponse {
    pub message: OllamaResponseMessage,
    pub model: String,
    pub done: bool,
    #[serde(default)]
    pub prompt_eval_count: Option<u32>,
    #[serde(default)]
    pub eval_count: Option<u32>,
}

#[derive(Deserialize)]
pub(crate) struct OllamaResponseMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Option<Vec<OllamaToolCall>>,
}
```

Notice a few Ollama-specific details: the `options` object holds runtime parameters like `num_predict` (max output tokens) and `num_ctx` (context window size). Token usage comes through `prompt_eval_count` and `eval_count` fields instead of a structured `usage` object.

::: python Coming from Python
In Python, you might use the `ollama` pip package that provides a high-level client. In Rust, you talk to Ollama's REST API directly with `reqwest` — there is no official Rust SDK. This is common in Rust development: the ecosystem is smaller, so you often write HTTP client code against JSON APIs rather than relying on provider-maintained SDKs.
:::

## Model Availability Check

Before sending a request, it is helpful to verify the model is actually downloaded. Ollama can pull models on demand, but that takes minutes for large models — not something you want to discover mid-conversation:

```rust
impl OllamaProvider {
    /// Check if the model is available locally.
    pub async fn ensure_model_available(&self) -> Result<(), ProviderError> {
        let response = self.client
            .post(format!("{}/api/show", self.base_url))
            .json(&serde_json::json!({ "name": &self.model }))
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => Ok(()),
            Ok(resp) if resp.status().as_u16() == 404 => {
                Err(ProviderError::Other(format!(
                    "Model '{}' is not downloaded. Run: ollama pull {}",
                    self.model, self.model
                )))
            }
            Ok(resp) => {
                let body = resp.text().await.unwrap_or_default();
                Err(ProviderError::Api {
                    status: 500,
                    message: format!("Ollama error: {body}"),
                })
            }
            Err(_) => {
                Err(ProviderError::Other(
                    "Cannot connect to Ollama. Is it running? Start with: ollama serve".into()
                ))
            }
        }
    }
}
```

This provides clear, actionable error messages. If Ollama is not running, the user sees "Is it running? Start with: ollama serve" instead of a cryptic connection refused error.

## Implementing the Provider Trait

```rust
#[async_trait::async_trait]
impl Provider for OllamaProvider {
    async fn send_message(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let api_request = self.build_request_body(&request, false);

        let http_response = self.client
            .post(format!("{}/api/chat", self.base_url))
            .json(&api_request)
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    ProviderError::Other(
                        "Cannot connect to Ollama. Is it running? Start with: ollama serve"
                            .into(),
                    )
                } else {
                    ProviderError::Http(e)
                }
            })?;

        if !http_response.status().is_success() {
            let body = http_response.text().await.unwrap_or_default();
            return Err(ProviderError::Api {
                status: 500,
                message: body,
            });
        }

        let api_response: OllamaChatResponse = http_response.json().await?;
        Ok(self.translate_response(api_response))
    }

    async fn stream_message(&self, request: ChatRequest) -> Result<StreamResult, ProviderError> {
        let api_request = self.build_request_body(&request, true);

        let response = self.client
            .post(format!("{}/api/chat", self.base_url))
            .json(&api_request)
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    ProviderError::Other(
                        "Cannot connect to Ollama. Is it running?".into()
                    )
                } else {
                    ProviderError::Http(e)
                }
            })?;

        let byte_stream = response.bytes_stream();

        let stream = async_stream::stream! {
            let mut byte_stream = std::pin::pin!(byte_stream);
            let mut buffer = String::new();

            while let Some(chunk_result) = byte_stream.next().await {
                let chunk = match chunk_result {
                    Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                    Err(e) => {
                        yield Err(ProviderError::Http(e));
                        return;
                    }
                };

                buffer.push_str(&chunk);

                // Ollama streams newline-delimited JSON
                while let Some(newline_pos) = buffer.find('\n') {
                    let line = buffer[..newline_pos].to_string();
                    buffer = buffer[newline_pos + 1..].to_string();

                    if line.trim().is_empty() {
                        continue;
                    }

                    if let Ok(chunk) = serde_json::from_str::<OllamaChatResponse>(&line) {
                        if !chunk.message.content.is_empty() {
                            yield Ok(StreamEvent::TextDelta(chunk.message.content));
                        }

                        if chunk.done {
                            let usage = Usage {
                                input_tokens: chunk.prompt_eval_count.unwrap_or(0),
                                output_tokens: chunk.eval_count.unwrap_or(0),
                                cache_read_tokens: None,
                                cache_write_tokens: None,
                            };
                            yield Ok(StreamEvent::Done {
                                usage,
                                stop_reason: StopReason::EndTurn,
                            });
                        }
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }

    fn capabilities(&self) -> &ModelCapabilities {
        &self.capabilities
    }

    fn name(&self) -> &str {
        "ollama"
    }

    fn model(&self) -> &str {
        &self.model
    }
}
```

Ollama's streaming format is simpler than both Anthropic and OpenAI. Instead of SSE events, it sends newline-delimited JSON (NDJSON). Each line is a complete JSON object with a `message` field and a `done` boolean. The final chunk has `done: true` and includes token counts.

## Request Building and Graceful Degradation

The request builder handles the case where tools are requested but the model does not support them:

```rust
impl OllamaProvider {
    fn build_request_body(&self, request: &ChatRequest, stream: bool) -> OllamaChatRequest {
        let mut messages = Vec::new();

        // Ollama supports system messages directly
        if let Some(system) = &request.system_prompt {
            messages.push(OllamaMessage {
                role: "system".to_string(),
                content: system.clone(),
                tool_calls: None,
            });
        }

        for msg in &request.messages {
            let text: String = msg.content.iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");

            let tool_calls: Option<Vec<OllamaToolCall>> = msg.content.iter()
                .filter_map(|b| match b {
                    ContentBlock::ToolUse { name, input, .. } => {
                        Some(OllamaToolCall {
                            function: OllamaFunctionCall {
                                name: name.clone(),
                                arguments: input.clone(),
                            },
                        })
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
                .into();

            messages.push(OllamaMessage {
                role: match msg.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                    Role::System => "system".to_string(),
                },
                content: text,
                tool_calls: if tool_calls.as_ref().map_or(true, |t| t.is_empty()) {
                    None
                } else {
                    tool_calls
                },
            });
        }

        // Only include tools if the model supports them
        let tools = if self.capabilities.supports_tools {
            request.tools.as_ref().map(|tools| {
                tools.iter().map(|t| OllamaTool {
                    tool_type: "function".to_string(),
                    function: OllamaFunctionDef {
                        name: t.name.clone(),
                        description: t.description.clone(),
                        parameters: t.input_schema.clone(),
                    },
                }).collect()
            })
        } else {
            None // Silently drop tools for models that don't support them
        };

        OllamaChatRequest {
            model: self.model.clone(),
            messages,
            tools,
            stream,
            options: Some(OllamaOptions {
                temperature: request.temperature,
                num_predict: Some(request.max_tokens),
                num_ctx: Some(self.capabilities.max_context_tokens),
            }),
        }
    }

    fn translate_response(&self, response: OllamaChatResponse) -> ChatResponse {
        let mut content = Vec::new();

        if !response.message.content.is_empty() {
            content.push(ContentBlock::Text {
                text: response.message.content,
            });
        }

        if let Some(tool_calls) = response.message.tool_calls {
            for tc in tool_calls {
                content.push(ContentBlock::ToolUse {
                    id: format!("ollama_{}", uuid::Uuid::new_v4()),
                    name: tc.function.name,
                    input: tc.function.arguments,
                });
            }
        }

        let stop_reason = if content.iter().any(|b| matches!(b, ContentBlock::ToolUse { .. })) {
            StopReason::ToolUse
        } else {
            StopReason::EndTurn
        };

        ChatResponse {
            content,
            model: response.model,
            usage: Usage {
                input_tokens: response.prompt_eval_count.unwrap_or(0),
                output_tokens: response.eval_count.unwrap_or(0),
                cache_read_tokens: None,
                cache_write_tokens: None,
            },
            stop_reason,
        }
    }
}
```

Note the tool call ID generation: Ollama does not return tool call IDs, so the adapter generates them using UUIDs. This ensures the agentic loop can track which tool result corresponds to which tool call, maintaining the contract expected by the rest of the system.

::: wild In the Wild
The Codex CLI agent supports local models through an OpenAI-compatible interface, effectively treating any local server that speaks the OpenAI API format as a valid provider. This pragmatic approach leverages the fact that many local model servers (Ollama, llama.cpp, vLLM) offer OpenAI-compatible endpoints. Your Ollama adapter uses Ollama's native API for richer feature access, but you could also point the OpenAI adapter at Ollama's OpenAI-compatible endpoint as a quick fallback.
:::

## Key Takeaways

- The Ollama adapter talks to a local REST API, handling connection errors with actionable messages ("Is Ollama running?") rather than generic HTTP errors.
- Capability detection for local models is heuristic — the adapter infers features from model names because Ollama does not expose a structured capabilities API.
- Graceful degradation means silently dropping tools for models that do not support them, and generating synthetic tool call IDs when Ollama does not provide them.
- Ollama streams newline-delimited JSON (NDJSON) rather than SSE, so the streaming parser is simpler than Anthropic's or OpenAI's.
- The `ensure_model_available` method lets you verify model presence before starting a conversation, avoiding long waits from accidental model pulls mid-session.
