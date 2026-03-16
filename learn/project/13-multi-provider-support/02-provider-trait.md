---
title: Provider Trait
description: Designing the core provider trait that abstracts message sending, streaming, and token counting into a common interface that all LLM adapters implement.
---

# Provider Trait

> **What you'll learn:**
> - How to design an async trait in Rust that captures the common operations across LLM providers
> - Which methods belong in the core trait versus provider-specific extensions
> - How to handle the differences in message formats, tool calling, and response structures at the trait boundary

The provider trait is the foundation of the entire multi-provider architecture. It defines the contract that every LLM adapter must satisfy, and it is the only interface your agentic loop interacts with. Getting this design right means the rest of the chapter flows naturally. Getting it wrong means you will be fighting the abstraction in every adapter.

Let's design the trait from the ground up, starting with the common types that all providers share, then building the trait itself.

## Common Types: The Provider-Neutral Vocabulary

Before you can define the trait, you need types that represent messages, responses, and tool calls without any provider-specific details. These types form the language that your agent core speaks.

Create `src/provider/types.rs`:

```rust
use serde::{Deserialize, Serialize};

/// A role in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

/// A single content block within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
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
        is_error: bool,
    },
}

/// A message in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

/// A tool definition the model can call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value, // JSON Schema
}

/// Token usage for a single request.
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: Option<u32>,
    pub cache_creation_tokens: Option<u32>,
}

/// The complete response from a non-streaming request.
#[derive(Debug, Clone)]
pub struct ProviderResponse {
    pub content: Vec<ContentBlock>,
    pub usage: TokenUsage,
    pub model: String,
    pub stop_reason: StopReason,
}

/// Why the model stopped generating.
#[derive(Debug, Clone, PartialEq)]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    StopSequence,
    Unknown(String),
}
```

Notice that these types borrow concepts from both Anthropic and OpenAI but are not specific to either. The `ContentBlock` enum uses a tagged union to represent text, tool calls, and tool results in a single type. The `ToolDefinition` uses a JSON Schema value for parameters, which both major providers accept.

::: python Coming from Python
In Python, you might use dataclasses or Pydantic models for these types:
```python
@dataclass
class Message:
    role: str
    content: list[ContentBlock]
```
Python's duck typing means any dict with the right keys would work too. Rust's enums and structs give you compile-time guarantees that every message has a valid role and every content block is one of the known variants. You cannot accidentally create a `ContentBlock` with `type: "unknown"` -- the compiler rejects it.
:::

## The Streaming Types

Streaming requires its own set of types. Each provider sends chunks differently, but your agent needs a uniform stream of events:

```rust
use tokio::sync::mpsc;

/// A single event from a streaming response.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// A chunk of text content.
    TextDelta(String),

    /// The start of a tool use block.
    ToolUseStart {
        id: String,
        name: String,
    },

    /// A chunk of tool input JSON.
    ToolInputDelta(String),

    /// The tool use block is complete.
    ToolUseEnd,

    /// Token usage information (typically sent at the end).
    Usage(TokenUsage),

    /// The stream is complete.
    Done { stop_reason: StopReason },

    /// An error occurred during streaming.
    Error(String),
}

/// A handle to a streaming response.
pub struct StreamHandle {
    pub receiver: mpsc::Receiver<StreamEvent>,
}
```

The `StreamEvent` enum captures every meaningful event that can occur during streaming. Text arrives as deltas (chunks), tool calls arrive as a start event followed by input deltas and an end event, and usage arrives when the stream completes. This design lets the UI render incrementally without knowing which provider is generating the events.

## The Provider Trait

Now for the trait itself. The key design decision is what belongs in the trait and what stays outside it.

In `src/provider/mod.rs`:

```rust
use async_trait::async_trait;
use crate::provider::types::*;

/// The core provider trait that all LLM adapters implement.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Returns the provider's name (e.g., "anthropic", "openai", "ollama").
    fn name(&self) -> &str;

    /// Returns the currently configured model identifier.
    fn model(&self) -> &str;

    /// Send a complete (non-streaming) request and wait for the full response.
    async fn send_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
        max_tokens: u32,
    ) -> Result<ProviderResponse, ProviderError>;

    /// Send a streaming request and return a handle to receive events.
    async fn stream_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
        max_tokens: u32,
    ) -> Result<StreamHandle, ProviderError>;
}
```

The trait uses `async_trait` because both `send_message` and `stream_message` are async operations that involve network I/O. The `Send + Sync` bounds are required because the provider will be shared across async tasks.

Notice what is *not* in the trait:

- **Configuration and construction**: Each provider has different configuration needs (API keys, base URLs, model-specific options). Construction happens outside the trait.
- **Cost calculation**: Pricing is provider-specific and changes frequently. It belongs in a separate cost tracking layer.
- **Capability queries**: Model capabilities are metadata about a provider, not operations the provider performs. They live in a separate capabilities system.

This keeps the trait focused on the one thing all providers must do: send messages and receive responses.

## The Error Type

Provider errors need to carry enough information for the fallback system to make routing decisions:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error (status {status}): {message}")]
    Api {
        status: u16,
        message: String,
        retryable: bool,
    },

    #[error("Rate limited, retry after {retry_after_ms:?}ms")]
    RateLimited {
        retry_after_ms: Option<u64>,
    },

    #[error("Authentication failed: {0}")]
    AuthError(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Request timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

impl ProviderError {
    /// Returns true if the error is transient and the request could succeed on retry.
    pub fn is_retryable(&self) -> bool {
        match self {
            ProviderError::Http(_) => true,
            ProviderError::Api { retryable, .. } => *retryable,
            ProviderError::RateLimited { .. } => true,
            ProviderError::Timeout { .. } => true,
            ProviderError::StreamError(_) => true,
            ProviderError::AuthError(_) => false,
            ProviderError::ModelNotFound(_) => false,
            ProviderError::Serialization(_) => false,
        }
    }

    /// Returns true if the error suggests trying a different provider entirely.
    pub fn should_fallback(&self) -> bool {
        match self {
            ProviderError::RateLimited { .. } => true,
            ProviderError::Api { status, .. } => *status >= 500,
            ProviderError::Timeout { .. } => true,
            ProviderError::AuthError(_) => true,
            _ => false,
        }
    }
}
```

The `is_retryable` and `should_fallback` methods are critical for the fallback chain system you will build in a later subchapter. They encode the knowledge about which failures are transient (worth retrying) and which suggest switching to a different provider entirely.

## Using the Trait with Dynamic Dispatch

Your agentic loop will hold the provider as a trait object so it can work with any concrete implementation:

```rust
use std::sync::Arc;

pub struct Agent {
    provider: Arc<dyn Provider>,
    system_prompt: String,
    tools: Vec<ToolDefinition>,
    messages: Vec<Message>,
}

impl Agent {
    pub fn new(provider: Arc<dyn Provider>, system_prompt: String) -> Self {
        Agent {
            provider,
            system_prompt,
            tools: Vec::new(),
            messages: Vec::new(),
        }
    }

    pub async fn send(&self) -> Result<ProviderResponse, ProviderError> {
        self.provider.send_message(
            &self.system_prompt,
            &self.messages,
            &self.tools,
            4096,
        ).await
    }
}
```

Using `Arc<dyn Provider>` gives you dynamic dispatch -- the concrete provider type is determined at runtime, and the agent does not need to be generic over the provider type. This simplifies the agent's type signature and makes it easy to swap providers at runtime (which you will do in the model switching subchapter).

::: python Coming from Python
Python's approach would look nearly identical:
```python
class Agent:
    def __init__(self, provider: LLMProvider, system_prompt: str):
        self.provider = provider
```
The difference is that Python resolves the method call at runtime by looking up `send_message` in the object's attribute dictionary. Rust resolves it through a vtable -- a table of function pointers associated with the trait object. The performance is comparable, but Rust guarantees at compile time that the provider implements every required method.
:::

## Module Structure

Organize the provider code in a clean module hierarchy:

```
src/
  provider/
    mod.rs           # The Provider trait and ProviderError
    types.rs         # Message, ContentBlock, StreamEvent, etc.
    anthropic.rs     # Anthropic adapter
    openai.rs        # OpenAI adapter
    ollama.rs        # Ollama adapter
    capabilities.rs  # Model capability definitions
    fallback.rs      # Fallback chain logic
    cost.rs          # Cost tracking
    config.rs        # Provider configuration
```

Each adapter file implements the `Provider` trait for its specific API. The `mod.rs` file re-exports the trait and common types so the rest of the codebase only needs `use crate::provider::*`.

## Key Takeaways

- The provider trait defines two core operations: `send_message` for complete responses and `stream_message` for incremental streaming -- every LLM API supports both patterns
- Provider-neutral types (`Message`, `ContentBlock`, `StreamEvent`) form the vocabulary that isolates your agent core from any specific API format
- The error type carries classification metadata (`is_retryable`, `should_fallback`) that powers the fallback chain system
- Dynamic dispatch with `Arc<dyn Provider>` lets the agent hold any provider implementation and swap it at runtime without changing the agent's type
- Keep the trait minimal: configuration, cost calculation, and capability queries belong in separate systems, not in the core trait
