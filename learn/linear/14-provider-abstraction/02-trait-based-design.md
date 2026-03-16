---
title: Trait Based Design
description: Use Rust's trait system to define a provider contract that enforces correct implementation at compile time.
---

# Trait Based Design

> **What you'll learn:**
> - How to define async traits with associated types that model the provider interface, including message types, streaming responses, and errors
> - Techniques for using trait objects vs generics to achieve dynamic dispatch when the provider is selected at runtime
> - How to handle the async trait challenges in Rust, including object safety constraints and the role of the async-trait crate

In the previous subchapter you established the design principles for a good provider abstraction. Now it is time to turn those principles into Rust code. Rust's trait system is the natural mechanism for defining contracts between your agent's core logic and the provider adapters. But building async traits that support dynamic dispatch involves several decisions that Python developers rarely face. Let's work through them.

## Defining the Core Types

Before writing the trait itself, you need the types it operates on. These are your canonical message types — the "language" your agent speaks internally, independent of any provider.

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;
use futures::Stream;

/// Roles in a conversation, shared across all providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

/// A single piece of content within a message.
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

/// A message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

/// The request your agent sends to any provider.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub system_prompt: Option<String>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub extensions: HashMap<String, serde_json::Value>,
}

/// A tool the model can call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
```

Notice that `ContentBlock` uses an enum with variants for text, tool use, and tool results. This is your agent's internal representation. Each provider adapter translates to and from these types. The `extensions` field on `ChatRequest` is the escape hatch from the previous subchapter — a place for provider-specific parameters that have not been promoted to first-class fields.

## The Response and Streaming Types

You also need types for what comes back from the provider:

```rust
/// The response from a non-streaming API call.
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub content: Vec<ContentBlock>,
    pub model: String,
    pub usage: Usage,
    pub stop_reason: StopReason,
}

/// Token usage information, normalized across providers.
#[derive(Debug, Clone, Default)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: Option<u32>,
    pub cache_write_tokens: Option<u32>,
}

/// Why the model stopped generating.
#[derive(Debug, Clone, PartialEq)]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    StopSequence,
}

/// Events emitted during streaming.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// A chunk of text content.
    TextDelta(String),
    /// The model is starting a tool call.
    ToolUseStart { id: String, name: String },
    /// A chunk of tool call input JSON.
    ToolInputDelta(String),
    /// The stream has ended with final usage data.
    Done { usage: Usage, stop_reason: StopReason },
}
```

These types give your streaming consumer a uniform interface. Whether the tokens come from Anthropic's `content_block_delta` events or OpenAI's `delta.content` chunks, they arrive as `StreamEvent::TextDelta` values.

## Writing the Provider Trait

Now the trait itself. The central design question is: should you use generics (static dispatch) or trait objects (dynamic dispatch)?

Since the user selects the provider at runtime — via configuration or a `/model` command — you need **dynamic dispatch**. The agentic loop holds a `Box<dyn Provider>` and calls methods on it without knowing the concrete type.

Here is the trait definition:

```rust
use std::pin::Pin;
use futures::Stream;

/// Errors that any provider can produce.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API error (status {status}): {message}")]
    Api { status: u16, message: String },
    #[error("Rate limited, retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },
    #[error("Authentication failed: {0}")]
    Auth(String),
    #[error("Unsupported feature: {0}")]
    Unsupported(String),
    #[error("{0}")]
    Other(String),
}

/// The type returned by streaming methods.
pub type StreamResult = Pin<Box<dyn Stream<Item = Result<StreamEvent, ProviderError>> + Send>>;

/// The core provider contract. Every LLM backend implements this trait.
#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    /// Send a chat request and wait for the complete response.
    async fn send_message(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError>;

    /// Send a chat request and receive a stream of events.
    async fn stream_message(&self, request: ChatRequest) -> Result<StreamResult, ProviderError>;

    /// Query this provider's capabilities for the given model.
    fn capabilities(&self) -> &ModelCapabilities;

    /// The provider's display name (e.g., "anthropic", "openai", "ollama").
    fn name(&self) -> &str;

    /// The currently configured model identifier.
    fn model(&self) -> &str;
}
```

Let's break down the design decisions.

### Why `async_trait`?

Native async functions in traits became partially stable in Rust 1.75, but there is a catch: traits with `async fn` methods are not object-safe by default. You cannot write `Box<dyn Provider>` if the trait has native `async fn` methods without extra ceremony. The `async_trait` crate solves this by desugaring `async fn` into methods that return `Pin<Box<dyn Future>>`, which are object-safe.

```rust
// What you write:
#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    async fn send_message(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError>;
}

// What the macro expands to (approximately):
pub trait Provider: Send + Sync {
    fn send_message<'a>(
        &'a self,
        request: ChatRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ChatResponse, ProviderError>> + Send + 'a>>;
}
```

The boxed future adds one heap allocation per call. For LLM API calls that take hundreds of milliseconds, this overhead is negligible.

::: python Coming from Python
In Python, you would define a provider interface using `ABC` or `Protocol`:

```python
from abc import ABC, abstractmethod
from typing import AsyncIterator

class Provider(ABC):
    @abstractmethod
    async def send_message(self, request: ChatRequest) -> ChatResponse: ...

    @abstractmethod
    async def stream_message(self, request: ChatRequest) -> AsyncIterator[StreamEvent]: ...
```

Python's `ABC` checks at instantiation time; if you forget to implement a method, you get a `TypeError` when constructing the object. Rust's trait checks at compile time — if you forget to implement `stream_message`, your code does not compile. This means you will never encounter a "method not implemented" error in production.
:::

### The `Send + Sync` Bound

The `Provider: Send + Sync` supertrait bound ensures that any `Provider` implementation can be shared safely across threads. This matters because your agent may hold the provider in an `Arc<dyn Provider>` and access it from multiple async tasks — for example, streaming a response in one task while checking capabilities in another.

### The `StreamResult` Type Alias

Returning a stream from an async trait requires a type alias to keep signatures readable. `StreamResult` is a pinned, boxed, sendable stream of `Result<StreamEvent, ProviderError>`. Each provider adapter creates a concrete stream type internally and boxes it before returning.

## Using the Trait: Static vs Dynamic Dispatch

With the trait defined, let's see how to use it in agent code.

**Static dispatch** (generics) — the compiler generates specialized code for each provider type. Fast, but the provider type must be known at compile time:

```rust
async fn run_loop<P: Provider>(provider: &P) -> Result<()> {
    let response = provider.send_message(request).await?;
    // ...
    Ok(())
}
```

**Dynamic dispatch** (trait objects) — a vtable lookup at each method call. Slightly slower, but the provider type can be determined at runtime:

```rust
async fn run_loop(provider: &dyn Provider) -> Result<()> {
    let response = provider.send_message(request).await?;
    // ...
    Ok(())
}
```

For the provider abstraction, dynamic dispatch is the right choice. The cost of a vtable lookup is lost in the noise of an HTTP round trip, and you gain the ability to swap providers at runtime.

In your agent struct, store the provider as a boxed trait object:

```rust
pub struct Agent {
    provider: Box<dyn Provider>,
    tools: ToolRegistry,
    conversation: Vec<Message>,
}

impl Agent {
    pub fn new(provider: Box<dyn Provider>) -> Self {
        Self {
            provider,
            tools: ToolRegistry::new(),
            conversation: Vec::new(),
        }
    }

    pub async fn chat(&mut self, user_input: &str) -> Result<String, ProviderError> {
        self.conversation.push(Message {
            role: Role::User,
            content: vec![ContentBlock::Text { text: user_input.to_string() }],
        });

        let request = ChatRequest {
            model: self.provider.model().to_string(),
            messages: self.conversation.clone(),
            system_prompt: Some("You are a helpful coding assistant.".into()),
            max_tokens: 4096,
            temperature: None,
            tools: None,
            extensions: HashMap::new(),
        };

        let response = self.provider.send_message(request).await?;

        // Extract text from response
        let text = response.content.iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        self.conversation.push(Message {
            role: Role::Assistant,
            content: response.content,
        });

        Ok(text)
    }
}
```

Notice that the `Agent` struct has no knowledge of Anthropic, OpenAI, or Ollama. It works entirely through the `Provider` trait. To switch providers, you replace the `Box<dyn Provider>` — no other code changes needed.

## Object Safety Rules to Remember

Not every trait can be used as a trait object. Rust enforces "object safety" rules. The most common traps:

1. **No generic methods.** Trait methods cannot have type parameters. Use concrete types or trait objects instead.
2. **No `Self` in return position.** Methods cannot return `Self` because the concrete type is erased behind the trait object. (Methods that take `&self` are fine.)
3. **No associated constants with complex bounds.** Keep associated types simple or avoid them in object-safe traits.

Your `Provider` trait avoids all of these: methods take `&self`, return concrete types (`Result<ChatResponse, ProviderError>`), and have no generic parameters.

::: wild In the Wild
OpenCode defines its provider interface in Go using an `interface` type with `Chat` and `Stream` methods. Go interfaces are implicitly satisfied — any type with the right method signatures automatically implements the interface, similar to Python's duck typing but with compiler verification. Rust traits are explicitly opted into with `impl Provider for MyStruct`, giving you a clear audit trail of which types claim to be providers.
:::

## Key Takeaways

- Define canonical types (`Message`, `ChatRequest`, `ChatResponse`, `StreamEvent`) that belong to your agent, not to any provider. Adapters translate between these types and provider-native formats.
- Use the `async_trait` crate to make async trait methods object-safe, enabling `Box<dyn Provider>` for runtime provider selection. The boxing overhead is negligible compared to API call latency.
- Choose dynamic dispatch (`dyn Provider`) over static dispatch (generics) when the concrete provider type is determined at runtime, which is the common case for a user-configurable agent.
- Enforce `Send + Sync` bounds on the trait so providers can be shared across async tasks safely.
- Keep the trait object-safe by avoiding generic methods, `Self` return types, and complex associated types.
