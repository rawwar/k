---
title: Abstraction Principles
description: Establish the design principles that guide a good provider abstraction, balancing generality with the ability to leverage provider-specific features.
---

# Abstraction Principles

> **What you'll learn:**
> - How to identify the right abstraction boundary between agent logic and provider details using the dependency inversion principle
> - The tension between lowest-common-denominator interfaces and provider-specific feature access, and strategies for resolving it
> - How to design abstractions that are stable over time even as provider APIs evolve and new providers emerge

Right now your coding agent likely talks directly to Anthropic's Messages API. Every function that sends a prompt knows about Anthropic-specific content blocks, Anthropic-specific streaming events, and Anthropic-specific error codes. That tight coupling works fine when you only support one provider -- but the moment a user asks "can I use GPT-4o instead?" you realize the API details have leaked into every layer of your codebase.

This subchapter establishes the design principles you will follow throughout the rest of this chapter. Before writing a single line of Rust, let's think clearly about what a good provider abstraction looks like.

## The Dependency Inversion Principle

The dependency inversion principle (DIP) states that high-level modules should not depend on low-level modules; both should depend on abstractions. In the context of your agent:

- **High-level module**: the agentic loop, which orchestrates turns of conversation, tool calls, and user interaction.
- **Low-level modules**: the HTTP clients that send requests to Anthropic, OpenAI, or Ollama.

Without DIP, your agentic loop directly imports and calls Anthropic-specific types. The dependency arrow points downward from the loop to the provider:

```
AgenticLoop  --->  AnthropicClient
```

With DIP, you introduce an abstraction — a `Provider` trait — that the agentic loop depends on. The concrete providers implement that trait. The dependency arrows now point inward toward the abstraction:

```
AgenticLoop  --->  Provider (trait)
                      ^
                      |
              AnthropicAdapter
              OpenAiAdapter
              OllamaAdapter
```

In Rust, this is natural. You define a trait, and your agentic loop accepts anything that implements it. The loop never needs to know which concrete provider is behind the trait object.

```rust
/// The agentic loop only knows about this trait, never concrete providers.
pub async fn run_loop(provider: &dyn Provider) -> Result<()> {
    let response = provider.send_message(request).await?;
    // Process response...
    Ok(())
}
```

The concrete type is decided elsewhere — in configuration, at startup, or even at runtime when the user issues a `/model` command. Your core logic stays untouched.

::: python Coming from Python
In Python, dependency inversion often happens implicitly through duck typing. If your function calls `provider.send_message(request)`, any object with that method works — no interface declaration needed. Rust requires you to be explicit: you define a trait, and the compiler verifies at compile time that every type claiming to be a `Provider` actually implements all required methods with the correct signatures. This catches integration errors that Python would only surface at runtime.
:::

## Finding the Right Abstraction Boundary

The hardest part of designing a provider abstraction is choosing what goes into the interface and what stays outside. Get this wrong and you end up with one of two failure modes.

### Failure Mode 1: Too Narrow (Lowest Common Denominator)

If you only include features that every provider supports, you lose access to powerful provider-specific capabilities. Anthropic's prompt caching, OpenAI's structured outputs with JSON schema enforcement, extended thinking — all become inaccessible because the interface does not model them.

```rust
// Too narrow: can only send text, get text back
pub trait Provider {
    async fn complete(&self, prompt: &str) -> Result<String>;
}
```

This interface is so restrictive it cannot even represent tool use, multi-turn conversations, or streaming. You would need to work around it constantly.

### Failure Mode 2: Too Wide (Kitchen Sink)

If you include every feature of every provider in the interface, the trait becomes a superset that no single provider fully implements. Most methods end up returning `Err(Unsupported)` for most providers, and the abstraction provides no useful guarantees.

```rust
// Too wide: most providers can't implement half of these
pub trait Provider {
    async fn complete(&self, req: Request) -> Result<Response>;
    async fn complete_with_cache(&self, req: Request, cache: CacheControl) -> Result<Response>;
    async fn complete_with_thinking(&self, req: Request, budget: u32) -> Result<Response>;
    async fn complete_with_structured_output(&self, req: Request, schema: JsonSchema) -> Result<Response>;
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    async fn moderate(&self, text: &str) -> Result<ModerationResult>;
    // ...dozens more methods
}
```

### The Right Boundary: Core + Capabilities

The approach that works in practice is to define a **core interface** that covers the fundamental operations every LLM provider can do (send messages, get responses, stream tokens) and a **capabilities system** that lets callers query which optional features a given provider supports.

```rust
pub trait Provider: Send + Sync {
    /// Every provider must be able to send a message and return a response.
    async fn send_message(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError>;

    /// Every provider must support streaming.
    async fn stream_message(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, ProviderError>> + Send>>, ProviderError>;

    /// Report which optional capabilities this provider/model supports.
    fn capabilities(&self) -> &ModelCapabilities;

    /// Return the provider's name for logging and display.
    fn name(&self) -> &str;
}
```

The `ChatRequest` and `ChatResponse` types are your own canonical types — not Anthropic's, not OpenAI's. Each adapter translates between your types and the provider's native format. The `capabilities()` method lets the agent check at runtime whether features like tool use, vision, or extended context are available, and adapt its behavior accordingly.

## Designing for Stability

Provider APIs change. Anthropic added extended thinking. OpenAI added structured outputs. New providers appear regularly. A good abstraction survives these changes without breaking existing code.

The key strategies are:

**Make the core trait small.** The fewer methods the trait has, the fewer reasons it has to change. Your `Provider` trait should have three to five methods, not thirty.

**Use rich request/response types.** Instead of many methods with different signatures, use a single `send_message` method with a `ChatRequest` type that uses `Option` fields for optional features:

```rust
pub struct ChatRequest {
    pub messages: Vec<Message>,
    pub model: String,
    pub max_tokens: u32,
    pub tools: Option<Vec<ToolDefinition>>,
    pub system_prompt: Option<String>,
    pub temperature: Option<f32>,
    /// Provider-specific extensions that don't fit the common model.
    pub extensions: HashMap<String, serde_json::Value>,
}
```

The `extensions` field is an escape hatch. If Anthropic adds a new `cache_control` parameter, you can pass it through `extensions` immediately without modifying the trait or the shared types. Over time, if a feature becomes universal, you promote it from `extensions` to a proper field.

**Separate what changes from what stays the same.** The message-sending protocol is stable. The list of supported models changes monthly. Keep model lists and pricing data in configuration files or a registry, not hardcoded in your trait implementations.

::: wild In the Wild
Claude Code structures its provider interaction around a message-passing interface where the core loop sends `Request` objects and receives `Response` objects, never touching HTTP directly. OpenCode follows a similar pattern with its Go interfaces, defining a `Provider` interface with `Chat` and `Stream` methods. Both agents use adapter modules that isolate API-specific serialization, so adding support for a new provider means writing a single new adapter without modifying the core loop.
:::

## The Extension Points Pattern

One more principle worth internalizing: design your abstraction with explicit extension points. These are the places where new functionality can be added without modifying existing code.

In the provider abstraction, the primary extension points are:

1. **New providers**: implement the `Provider` trait for a new struct. No existing code changes.
2. **New capabilities**: add a field to `ModelCapabilities`. Existing providers default to "not supported" via `Default`.
3. **New request features**: add an `Option` field to `ChatRequest`. Existing adapters ignore it (it is `None` by default).
4. **Provider-specific behavior**: use the `extensions` map for one-off features that do not warrant a shared type change.

This follows the open-closed principle: the system is open for extension but closed for modification. Each of these extension points lets you add functionality by writing new code rather than changing code that already works.

```rust
/// Capabilities use a struct with bool fields rather than an enum set,
/// making it trivial to add new capabilities with a Default of false.
#[derive(Debug, Clone, Default)]
pub struct ModelCapabilities {
    pub supports_tools: bool,
    pub supports_streaming: bool,
    pub supports_vision: bool,
    pub supports_extended_thinking: bool,
    pub supports_prompt_caching: bool,
    pub max_context_tokens: u32,
    pub max_output_tokens: u32,
}
```

When you add `supports_structured_output: bool` next month, every existing `ModelCapabilities` instance still compiles because `Default` fills the new field with `false`. No provider adapter needs updating unless it actually supports the feature.

## Key Takeaways

- The dependency inversion principle flips the dependency arrow: your agentic loop depends on the `Provider` trait, and concrete providers depend on that same trait. Neither high-level nor low-level code knows about the other.
- Avoid both the lowest-common-denominator trap (too narrow, losing useful features) and the kitchen-sink trap (too wide, every method returns "unsupported"). Instead, use a small core trait with a capabilities query method.
- Rich request/response types with `Option` fields and an `extensions` map let you handle provider-specific features without changing the trait signature.
- Design explicit extension points — new providers, new capabilities, new request fields — so that adding functionality means writing new code, not modifying working code.
- Keep the core trait small (three to five methods) to minimize reasons for it to change as provider APIs evolve.
