---
title: Local Models Ollama
description: Integrating local LLM inference through Ollama's API, handling model availability checks, managing reduced capabilities, and optimizing prompts for smaller models.
---

# Local Models Ollama

> **What you'll learn:**
> - How to connect to Ollama's local API and detect which models are available
> - How to handle the reduced tool-calling capability of smaller local models gracefully
> - Techniques for adapting prompts and expectations when running on less capable models

Cloud providers are not always available or appropriate. Sometimes you are on an airplane, behind a restrictive firewall, or working with sensitive code that cannot leave your machine. Ollama lets you run open-source LLMs locally, and your provider abstraction makes integrating it straightforward. The challenge is not the API -- Ollama follows the OpenAI chat completions format -- but rather gracefully handling models that are less capable than their cloud counterparts.

## Ollama's API

Ollama exposes a REST API on `localhost:11434` by default. It supports the OpenAI-compatible `/v1/chat/completions` endpoint, which means you could technically reuse your OpenAI adapter with a different base URL. However, building a dedicated Ollama adapter gives you better control over local-specific concerns: checking which models are pulled, handling models that lack tool calling, and managing the slower response times of local inference.

Ollama also has its own native API at `/api/chat` with additional features like model loading status and GPU utilization. You will use a mix of both.

## Checking Model Availability

Before sending requests, your adapter should verify that the requested model is actually available locally. Unlike cloud APIs that return an error for unknown models, Ollama will attempt to pull a model it does not have, which can stall a request for minutes or hours.

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::provider::{
    Provider, ProviderError, StreamHandle,
    types::*,
};

pub struct OllamaProvider {
    client: Client,
    model: String,
    base_url: String,
}

/// Response from Ollama's /api/tags endpoint listing local models.
#[derive(Debug, Deserialize)]
struct OllamaModelsResponse {
    models: Vec<OllamaModelInfo>,
}

#[derive(Debug, Deserialize)]
struct OllamaModelInfo {
    name: String,
    size: u64,
    #[serde(default)]
    details: Option<OllamaModelDetails>,
}

#[derive(Debug, Deserialize)]
struct OllamaModelDetails {
    parameter_size: Option<String>,
    family: Option<String>,
}

impl OllamaProvider {
    pub fn new(model: String) -> Self {
        Self {
            client: Client::new(),
            model,
            base_url: "http://localhost:11434".to_string(),
        }
    }

    pub fn with_base_url(model: String, base_url: String) -> Self {
        Self {
            client: Client::new(),
            model,
            base_url,
        }
    }

    /// Check if the Ollama server is running and the model is available.
    pub async fn check_availability(&self) -> Result<bool, ProviderError> {
        let response = self.client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .map_err(|e| ProviderError::Api {
                status: 0,
                message: format!("Cannot reach Ollama server: {}", e),
                retryable: false,
            })?;

        let models: OllamaModelsResponse = response.json().await
            .map_err(|e| ProviderError::Serialization(e.to_string()))?;

        // Ollama model names may include tags like ":latest"
        let available = models.models.iter().any(|m| {
            m.name == self.model
                || m.name == format!("{}:latest", self.model)
                || m.name.starts_with(&format!("{}:", self.model))
        });

        Ok(available)
    }

    /// List all locally available models.
    pub async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        let response = self.client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .map_err(|e| ProviderError::Api {
                status: 0,
                message: format!("Cannot reach Ollama server: {}", e),
                retryable: false,
            })?;

        let models: OllamaModelsResponse = response.json().await
            .map_err(|e| ProviderError::Serialization(e.to_string()))?;

        Ok(models.models.into_iter().map(|m| m.name).collect())
    }
}
```

## The Trait Implementation

Since Ollama supports the OpenAI-compatible format, the request and response structures are similar. The key differences are: no API key required, a different base URL, and the need for fallback behavior when tool calling fails.

```rust
/// Ollama uses OpenAI-compatible request format.
#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<serde_json::Value>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    num_predict: u32,      // Equivalent to max_tokens
    temperature: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OllamaMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<serde_json::Value>>,
}

fn to_ollama_messages(system: &str, messages: &[Message]) -> Vec<OllamaMessage> {
    let mut result = Vec::new();

    if !system.is_empty() {
        result.push(OllamaMessage {
            role: "system".to_string(),
            content: system.to_string(),
            tool_calls: None,
        });
    }

    for msg in messages {
        let role = match msg.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
        };

        // For local models, flatten all content into a single string.
        // Most local models handle structured content poorly.
        let content = msg.content.iter().map(|block| {
            match block {
                ContentBlock::Text { text } => text.clone(),
                ContentBlock::ToolUse { name, input, .. } => {
                    format!("[Tool call: {} with input: {}]",
                        name,
                        serde_json::to_string_pretty(input).unwrap_or_default()
                    )
                }
                ContentBlock::ToolResult { content, is_error, .. } => {
                    if *is_error {
                        format!("[Tool error: {}]", content)
                    } else {
                        format!("[Tool result: {}]", content)
                    }
                }
            }
        }).collect::<Vec<_>>().join("\n");

        result.push(OllamaMessage {
            role: role.to_string(),
            content,
            tool_calls: None,
        });
    }

    result
}

#[async_trait]
impl Provider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
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
        // Check if server is reachable first
        let available = self.check_availability().await?;
        if !available {
            return Err(ProviderError::ModelNotFound(format!(
                "Model '{}' is not available in Ollama. Run: ollama pull {}",
                self.model, self.model
            )));
        }

        // Try the OpenAI-compatible endpoint first (supports tool calling
        // on models that have it)
        let request = OllamaRequest {
            model: self.model.clone(),
            messages: to_ollama_messages(system, messages),
            tools: if tools.is_empty() {
                Vec::new()
            } else {
                tools.iter().map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                }).collect()
            },
            stream: false,
            options: OllamaOptions {
                num_predict: max_tokens,
                temperature: 0.1, // Lower temperature for more predictable tool use
            },
        };

        let response = self.client
            .post(format!("{}/api/chat", self.base_url))
            .json(&request)
            .timeout(std::time::Duration::from_secs(120)) // Local models can be slow
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::Timeout { timeout_ms: 120_000 }
                } else {
                    ProviderError::Http(e)
                }
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api {
                status,
                message: body,
                retryable: false,
            });
        }

        let body: serde_json::Value = response.json().await
            .map_err(|e| ProviderError::Serialization(e.to_string()))?;

        Ok(parse_ollama_response(body))
    }

    async fn stream_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
        max_tokens: u32,
    ) -> Result<StreamHandle, ProviderError> {
        let request = OllamaRequest {
            model: self.model.clone(),
            messages: to_ollama_messages(system, messages),
            tools: Vec::new(), // Streaming with tools is unreliable on local models
            stream: true,
            options: OllamaOptions {
                num_predict: max_tokens,
                temperature: 0.1,
            },
        };

        let response = self.client
            .post(format!("{}/api/chat", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api {
                status,
                message: body,
                retryable: false,
            });
        }

        let (tx, rx) = mpsc::channel(100);
        let byte_stream = response.bytes_stream();

        tokio::spawn(async move {
            if let Err(e) = process_ollama_stream(byte_stream, tx.clone()).await {
                let _ = tx.send(StreamEvent::Error(e.to_string())).await;
            }
        });

        Ok(StreamHandle { receiver: rx })
    }
}
```

::: python Coming from Python
In Python, you might use the `ollama` package:
```python
import ollama

response = ollama.chat(
    model="llama3",
    messages=[{"role": "user", "content": "Hello"}],
)
print(response["message"]["content"])
```
The simplicity is appealing, but notice what is missing: no availability check, no timeout handling, no fallback for tool calling failures. The Rust adapter handles all of these because the type system forces you to think about them upfront.
:::

## Parsing Ollama Responses

Ollama's native API response format differs from the OpenAI-compatible one:

```rust
fn parse_ollama_response(body: serde_json::Value) -> ProviderResponse {
    let message = &body["message"];
    let content_text = message["content"].as_str().unwrap_or("").to_string();

    let mut content = Vec::new();

    // Check for tool calls in the response
    if let Some(tool_calls) = message["tool_calls"].as_array() {
        for tc in tool_calls {
            if let Some(function) = tc.get("function") {
                let name = function["name"].as_str().unwrap_or("").to_string();
                let arguments = function.get("arguments")
                    .cloned()
                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

                content.push(ContentBlock::ToolUse {
                    id: format!("ollama_{}", uuid_simple()),
                    name,
                    input: arguments,
                });
            }
        }
    }

    // Add text content (even alongside tool calls)
    if !content_text.is_empty() {
        content.insert(0, ContentBlock::Text { text: content_text });
    }

    // Ollama doesn't provide detailed token counts in native API
    let eval_count = body["eval_count"].as_u64().unwrap_or(0) as u32;
    let prompt_eval_count = body["prompt_eval_count"].as_u64().unwrap_or(0) as u32;

    let stop_reason = if content.iter().any(|b| matches!(b, ContentBlock::ToolUse { .. })) {
        StopReason::ToolUse
    } else {
        StopReason::EndTurn
    };

    ProviderResponse {
        content,
        usage: TokenUsage {
            input_tokens: prompt_eval_count,
            output_tokens: eval_count,
            ..Default::default()
        },
        model: body["model"].as_str().unwrap_or("unknown").to_string(),
        stop_reason,
    }
}

/// Generate a simple unique ID for tool calls (Ollama doesn't provide them).
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}", nanos)
}
```

Notice `uuid_simple()` -- Ollama does not always provide tool call IDs, so you generate them. This is a common pattern when adapting APIs with missing fields: fill in sensible defaults rather than propagating `Option`s throughout your codebase.

## Streaming from Ollama

Ollama's streaming format sends one JSON object per line (newline-delimited JSON, or NDJSON), not SSE events. Each object contains a `message` field with a `content` delta:

```rust
use futures_util::StreamExt;

async fn process_ollama_stream(
    mut byte_stream: impl futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
    tx: mpsc::Sender<StreamEvent>,
) -> Result<(), ProviderError> {
    let mut buffer = String::new();

    while let Some(chunk) = byte_stream.next().await {
        let chunk = chunk.map_err(|e| ProviderError::StreamError(e.to_string()))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(newline_pos) = buffer.find('\n') {
            let line: String = buffer.drain(..newline_pos + 1).collect();
            let line = line.trim();

            if line.is_empty() {
                continue;
            }

            let parsed: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            // Check if stream is done
            if parsed["done"].as_bool().unwrap_or(false) {
                // Extract final token counts
                let usage = TokenUsage {
                    input_tokens: parsed["prompt_eval_count"]
                        .as_u64().unwrap_or(0) as u32,
                    output_tokens: parsed["eval_count"]
                        .as_u64().unwrap_or(0) as u32,
                    ..Default::default()
                };
                let _ = tx.send(StreamEvent::Usage(usage)).await;
                let _ = tx.send(StreamEvent::Done {
                    stop_reason: StopReason::EndTurn,
                }).await;
                return Ok(());
            }

            // Text content delta
            if let Some(content) = parsed["message"]["content"].as_str() {
                if !content.is_empty() {
                    let _ = tx.send(StreamEvent::TextDelta(
                        content.to_string()
                    )).await;
                }
            }
        }
    }

    Ok(())
}
```

## Handling Reduced Capabilities

The hardest part of the Ollama adapter is not the API integration -- it is managing expectations. Local models are typically 7B to 70B parameters, compared to the hundreds of billions in cloud models. They make mistakes more often, especially with structured output like tool calls.

One practical strategy is to use a text-based fallback for tool calling. If the model does not support native tool calling, you can embed tool descriptions in the system prompt and parse tool calls from the text output:

```rust
/// Build a system prompt that includes tool descriptions for models
/// without native tool calling support.
fn build_tool_prompt(base_system: &str, tools: &[ToolDefinition]) -> String {
    if tools.is_empty() {
        return base_system.to_string();
    }

    let tool_descriptions: Vec<String> = tools.iter().map(|t| {
        format!(
            "- {}: {}\n  Parameters: {}",
            t.name,
            t.description,
            serde_json::to_string_pretty(&t.parameters).unwrap_or_default()
        )
    }).collect();

    format!(
        "{}\n\n## Available Tools\n\
         You can call tools by responding with a JSON block:\n\
         ```json\n\
         {{\"tool\": \"tool_name\", \"input\": {{...}}}}\n\
         ```\n\n\
         Tools:\n{}",
        base_system,
        tool_descriptions.join("\n")
    )
}
```

This text-based tool calling is less reliable than native support, but it lets smaller models participate in the agent's workflow. The capabilities system (covered in the next subchapter) tells the agent which approach to use for each model.

::: wild In the Wild
Several production agents support Ollama or similar local inference servers. The approach varies: some treat local models as first-class citizens with full tool support, while others limit them to simpler tasks like summarization and explanation. Claude Code focuses on Anthropic's cloud models, but OpenCode supports Ollama through its OpenAI-compatible adapter, demonstrating the practical value of the `with_base_url` pattern.
:::

## Key Takeaways

- Ollama provides local LLM inference with an API that is partially OpenAI-compatible, but a dedicated adapter handles local-specific concerns like availability checking and timeout management
- Always verify model availability before sending requests -- Ollama will attempt to download missing models, which can hang the request indefinitely
- Local models often lack native tool calling; a text-based fallback embeds tool descriptions in the system prompt and parses structured responses from the model's text output
- Generate synthetic IDs and fill in default values for fields that local APIs do not provide, rather than propagating `Option`s throughout the codebase
- Ollama streams responses as newline-delimited JSON (NDJSON), not SSE, requiring a different parsing approach from the cloud adapters
