---
title: Adapter Pattern
description: Apply the adapter pattern to wrap diverse LLM provider APIs behind a single unified interface without losing provider-specific functionality.
---

# Adapter Pattern

> **What you'll learn:**
> - How to implement the adapter pattern in Rust to translate between your canonical message format and each provider's native API types
> - Strategies for handling provider-specific features (like Anthropic's cache control or OpenAI's function calling format) without polluting the common interface
> - How to structure adapter code for maintainability, keeping provider-specific serialization/deserialization isolated in dedicated modules

You have a `Provider` trait and a set of canonical types. You have three very different APIs — Anthropic's Messages API, OpenAI's Chat Completions API, and Ollama's REST API. The adapter pattern is the bridge that connects them. Each adapter wraps a provider's native API behind your `Provider` trait, translating requests and responses between your format and theirs.

## What the Adapter Pattern Looks Like in Rust

The adapter pattern has three participants:

1. **Target**: the interface your code expects (`Provider` trait).
2. **Adaptee**: the external API with its own types and conventions (Anthropic's API, OpenAI's API).
3. **Adapter**: a struct that implements the target interface by delegating to the adaptee and translating between type systems.

In Rust, adapters are structs that hold an HTTP client and implement your trait:

```rust
use reqwest::Client;

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    model: String,
    capabilities: ModelCapabilities,
}

#[async_trait::async_trait]
impl Provider for AnthropicProvider {
    async fn send_message(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        // 1. Translate ChatRequest -> Anthropic API request body
        let api_request = self.to_anthropic_request(&request);

        // 2. Send HTTP request to Anthropic
        let http_response = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&api_request)
            .send()
            .await?;

        // 3. Translate Anthropic response -> ChatResponse
        let api_response: AnthropicResponse = http_response.json().await?;
        Ok(self.to_chat_response(api_response))
    }

    // ... other trait methods
    # async fn stream_message(&self, _: ChatRequest) -> Result<StreamResult, ProviderError> { todo!() }
    # fn capabilities(&self) -> &ModelCapabilities { &self.capabilities }
    # fn name(&self) -> &str { "anthropic" }
    # fn model(&self) -> &str { &self.model }
}
```

The translation happens in private helper methods. These are the heart of the adapter — they know about both your canonical types and the provider's native types.

## Module Structure

A clean module structure keeps provider-specific code contained. Each provider gets its own module with internal types that never leak into the rest of the codebase:

```
src/
  provider/
    mod.rs           # Re-exports Provider trait, shared types
    types.rs         # ChatRequest, ChatResponse, Message, etc.
    error.rs         # ProviderError enum
    capabilities.rs  # ModelCapabilities struct
    anthropic/
      mod.rs         # AnthropicProvider struct + Provider impl
      types.rs       # Anthropic-specific request/response types
      stream.rs      # Anthropic SSE stream parsing
    openai/
      mod.rs         # OpenAiProvider struct + Provider impl
      types.rs       # OpenAI-specific request/response types
      stream.rs      # OpenAI streaming chunk parsing
    ollama/
      mod.rs         # OllamaProvider struct + Provider impl
      types.rs       # Ollama-specific request/response types
```

The key principle: **provider-specific types stay inside their module.** The `anthropic::types` module defines `AnthropicRequest`, `AnthropicResponse`, `ContentBlockDelta`, and so on. These types are `pub(crate)` at most — they exist only to serialize requests and deserialize responses. No code outside the `anthropic` module ever sees them.

```rust
// In src/provider/anthropic/types.rs
// These types match Anthropic's API shape exactly.
// They are NOT your canonical types — they exist only for serde.

use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(crate) struct AnthropicRequest {
    pub model: String,
    pub max_tokens: u32,
    pub messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<AnthropicTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    pub stream: bool,
}

#[derive(Serialize)]
pub(crate) struct AnthropicMessage {
    pub role: String,
    pub content: Vec<AnthropicContentBlock>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum AnthropicContentBlock {
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

#[derive(Deserialize)]
pub(crate) struct AnthropicResponse {
    pub content: Vec<AnthropicContentBlock>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub usage: AnthropicUsage,
}

#[derive(Deserialize)]
pub(crate) struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(default)]
    pub cache_creation_input_tokens: Option<u32>,
    #[serde(default)]
    pub cache_read_input_tokens: Option<u32>,
}
```

::: python Coming from Python
In Python, you would likely use dictionaries or Pydantic models for API types. The adapter pattern works the same way conceptually, but Python does not enforce the encapsulation. Nothing prevents code in your agentic loop from importing `AnthropicResponse` and depending on its shape directly. In Rust, `pub(crate)` visibility means the Anthropic response types genuinely cannot be imported outside the provider crate — the compiler enforces the boundary you are designing.
:::

## Translation Methods

The adapter's core job is translation. Let's look at how the conversion functions work for converting your canonical types to and from provider-specific types.

```rust
impl AnthropicProvider {
    /// Convert our canonical ChatRequest into Anthropic's API format.
    fn to_anthropic_request(&self, request: &ChatRequest) -> AnthropicRequest {
        let messages = request.messages.iter().map(|msg| {
            AnthropicMessage {
                role: match msg.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                    Role::System => "user".to_string(), // Anthropic uses system param
                },
                content: msg.content.iter().map(|block| {
                    match block {
                        ContentBlock::Text { text } => {
                            AnthropicContentBlock::Text { text: text.clone() }
                        }
                        ContentBlock::ToolUse { id, name, input } => {
                            AnthropicContentBlock::ToolUse {
                                id: id.clone(),
                                name: name.clone(),
                                input: input.clone(),
                            }
                        }
                        ContentBlock::ToolResult { tool_use_id, content, is_error } => {
                            AnthropicContentBlock::ToolResult {
                                tool_use_id: tool_use_id.clone(),
                                content: content.clone(),
                                is_error: Some(*is_error),
                            }
                        }
                    }
                }).collect(),
            }
        }).collect();

        AnthropicRequest {
            model: request.model.clone(),
            max_tokens: request.max_tokens,
            messages,
            system: request.system_prompt.clone(),
            tools: request.tools.as_ref().map(|tools| {
                tools.iter().map(|t| AnthropicTool {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    input_schema: t.input_schema.clone(),
                }).collect()
            }),
            temperature: request.temperature,
            stream: false,
        }
    }

    /// Convert Anthropic's response into our canonical ChatResponse.
    fn to_chat_response(&self, response: AnthropicResponse) -> ChatResponse {
        let content = response.content.into_iter().map(|block| {
            match block {
                AnthropicContentBlock::Text { text } => ContentBlock::Text { text },
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

        let stop_reason = match response.stop_reason.as_deref() {
            Some("end_turn") => StopReason::EndTurn,
            Some("tool_use") => StopReason::ToolUse,
            Some("max_tokens") => StopReason::MaxTokens,
            _ => StopReason::EndTurn,
        };

        ChatResponse {
            content,
            model: response.model,
            usage: Usage {
                input_tokens: response.usage.input_tokens,
                output_tokens: response.usage.output_tokens,
                cache_read_tokens: response.usage.cache_read_input_tokens,
                cache_write_tokens: response.usage.cache_creation_input_tokens,
            },
            stop_reason,
        }
    }
}
```

This is methodical, repetitive code — and that is exactly what you want. Each field mapping is explicit and easy to verify. When Anthropic changes their API, you update the types in `anthropic/types.rs` and the translation in these methods. Nothing else in the codebase is affected.

## Handling Provider-Specific Features via Extensions

Sometimes a provider has a feature that does not map to any canonical type. Anthropic's `cache_control` headers and OpenAI's `response_format` with JSON schema enforcement are examples. The `extensions` field on `ChatRequest` handles this:

```rust
impl AnthropicProvider {
    fn to_anthropic_request(&self, request: &ChatRequest) -> AnthropicRequest {
        let mut api_request = AnthropicRequest {
            // ... standard field mapping ...
            model: request.model.clone(),
            max_tokens: request.max_tokens,
            messages: vec![], // populated above
            system: request.system_prompt.clone(),
            tools: None,
            temperature: request.temperature,
            stream: false,
        };

        // Check for Anthropic-specific extensions
        if let Some(thinking_budget) = request.extensions.get("thinking_budget") {
            if let Some(budget) = thinking_budget.as_u64() {
                api_request.thinking = Some(ThinkingConfig {
                    thinking_type: "enabled".to_string(),
                    budget_tokens: budget as u32,
                });
            }
        }

        api_request
    }
}
```

The agentic loop can pass provider-specific hints without knowing which provider will consume them. If the user is connected to Anthropic, the adapter reads the `thinking_budget` extension. If they switch to OpenAI, that adapter simply ignores the unknown key. No errors, no special cases in the core logic.

::: wild In the Wild
Claude Code uses an adapter layer that converts between its internal message format and each API's requirements. The adapters handle subtle differences like how Anthropic expects `content` to be an array of blocks while OpenAI expects either a string or an array of objects with different shapes. OpenCode takes a similar approach in Go, with each provider implementing a common interface and handling its own serialization internally.
:::

## The Factory Function

To complete the pattern, you need a way to construct the right adapter from configuration. A factory function maps a provider name to a concrete adapter:

```rust
pub fn create_provider(config: &ProviderConfig) -> Result<Box<dyn Provider>, ProviderError> {
    match config.provider.as_str() {
        "anthropic" => {
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| ProviderError::Auth("ANTHROPIC_API_KEY not set".into()))?;
            Ok(Box::new(AnthropicProvider::new(
                api_key,
                config.model.clone(),
            )))
        }
        "openai" => {
            let api_key = std::env::var("OPENAI_API_KEY")
                .map_err(|_| ProviderError::Auth("OPENAI_API_KEY not set".into()))?;
            Ok(Box::new(OpenAiProvider::new(
                api_key,
                config.model.clone(),
            )))
        }
        "ollama" => {
            let base_url = config.base_url.clone()
                .unwrap_or_else(|| "http://localhost:11434".to_string());
            Ok(Box::new(OllamaProvider::new(
                base_url,
                config.model.clone(),
            )))
        }
        other => Err(ProviderError::Other(format!("Unknown provider: {other}"))),
    }
}
```

This is the only place in your codebase where concrete provider types are mentioned alongside each other. The factory returns `Box<dyn Provider>`, and from that point forward, all code works through the trait.

## Key Takeaways

- The adapter pattern in Rust uses a struct that holds an HTTP client and implements the `Provider` trait. Translation between canonical types and provider-specific types happens in private methods on the adapter struct.
- Keep provider-specific types (`AnthropicRequest`, `OpenAiResponse`, etc.) in their own modules with restricted visibility. They exist solely for serialization and should never leak into agent logic.
- Use the `extensions` field on `ChatRequest` to pass provider-specific features (like thinking budgets or cache control) without changing the trait or shared types.
- A factory function maps configuration strings to concrete `Box<dyn Provider>` instances, keeping the rest of the codebase free of concrete provider references.
- Translation code is intentionally repetitive and explicit — each field mapping is visible, easy to verify, and easy to update when a provider's API changes.
