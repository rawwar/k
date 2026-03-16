---
title: Summary
description: Recap of the multi-provider support chapter, reviewing the provider abstraction, adapter implementations, and runtime features that make the agent provider-agnostic.
---

# Summary

> **What you'll learn:**
> - How the provider trait, adapters, and runtime features work together as a complete abstraction layer
> - Which aspects of multi-provider support are most impactful for real-world agent deployment
> - How to add support for new providers as they emerge in the rapidly evolving LLM landscape

You started this chapter with an agent hardwired to a single LLM provider. You now have a provider-agnostic agent that can talk to Anthropic, OpenAI, and local Ollama models through a unified interface, with automatic fallback, cost tracking, and runtime switching. Let's review what you built and how the pieces fit together.

## The Architecture at a Glance

The provider system has four layers:

**Layer 1 -- The Trait** defines the contract: `send_message` and `stream_message`. Every provider must implement these two async methods. The trait uses provider-neutral types (`Message`, `ContentBlock`, `StreamEvent`) so the agent core never touches provider-specific data structures.

**Layer 2 -- The Adapters** translate between the generic types and each provider's API format. The Anthropic adapter maps system prompts to a top-level field and parses SSE streaming events. The OpenAI adapter wraps tools in a "function" envelope and reassembles chunked streaming deltas. The Ollama adapter checks model availability, handles slower local inference, and falls back to text-based tool calling when native support is unavailable.

**Layer 3 -- The Runtime Features** add intelligence on top of the trait. The model switcher enables mid-conversation provider changes. The fallback chain tries alternative providers when the primary one fails. The cost tracker records token usage and enforces budget limits. The capabilities registry tells the agent what each model can do.

**Layer 4 -- The Configuration** ties everything together. TOML config files, environment variables, and CLI arguments merge into a single `ProviderConfig` that constructs the entire provider stack. A single call to `build_fallback_chain()` produces a resilient, cost-aware provider ready for the agent to use.

## What You Built, by File

Here is a summary of the modules and their responsibilities:

```
src/provider/
  mod.rs           Provider trait, ProviderError, module re-exports
  types.rs         Message, ContentBlock, StreamEvent, TokenUsage,
                   ProviderResponse, StopReason, ToolDefinition
  anthropic.rs     AnthropicProvider: Messages API adapter with SSE streaming
  openai.rs        OpenAIProvider: Chat Completions adapter with delta streaming
  ollama.rs        OllamaProvider: Local model adapter with availability checking
  capabilities.rs  ModelCapabilities, CapabilityRegistry with query methods
  fallback.rs      FallbackChain (implements Provider), CircuitBreaker
  cost.rs          CostTracker, CostRecord, budget enforcement
  config.rs        ProviderConfig, multi-source loading, provider construction
```

Each module has a clear boundary and a single responsibility. The adapter modules depend only on the types in `mod.rs` and `types.rs`. The runtime modules (`fallback.rs`, `cost.rs`) depend on the trait but not on any specific adapter. This means you can add a new provider by writing one file -- the adapter -- without touching any existing code.

## The Design Principles

Three principles guided the design:

**1. The trait is minimal.** The `Provider` trait has four methods: `name()`, `model()`, `send_message()`, `stream_message()`. Everything else -- capabilities, costs, configuration -- lives outside the trait. This keeps the adapter implementations focused and makes the contract easy to satisfy.

**2. Errors carry routing information.** `ProviderError` is not just an error message -- it tells the fallback chain whether to retry (`is_retryable()`) or try a different provider (`should_fallback()`). This turns error handling into a routing decision, which is the key insight behind transparent fallback.

**3. Composition over inheritance.** The fallback chain is a `Provider` that contains other `Provider`s. The model switcher wraps a `Provider` in a swappable container. The cost tracker decorates every provider call with accounting. These are all composition patterns -- you build complex behavior by combining simple pieces rather than extending a base class.

::: python Coming from Python
This composition approach might feel unfamiliar if you are used to Python's class inheritance:
```python
class RetryingProvider(BaseProvider):
    def send_message(self, messages):
        # retry logic wrapping super().send_message()
        ...
```
In Rust, you cannot inherit from a trait implementation. Instead, you wrap it:
```rust
struct RetryingProvider {
    inner: Arc<dyn Provider>,
}
```
This pattern is more explicit and avoids the fragile base class problem. Each layer of composition is independent and testable in isolation.
:::

## Trait-Based Polymorphism vs. Duck Typing

Throughout this chapter, you have been using Rust's trait-based polymorphism as a replacement for Python's duck typing. Let's make the comparison explicit.

In Python, any object with a `send_message` method works as a provider -- no declaration needed. This is flexible but fragile. A typo like `send_mesage` will not be caught until runtime, and only if that code path is actually executed.

In Rust, a struct must explicitly declare that it implements the `Provider` trait with `impl Provider for MyStruct`. The compiler verifies that every required method exists with the correct signature. If you forget `stream_message`, the code does not compile. If you return the wrong type, the code does not compile.

Python's `typing.Protocol` closes some of this gap by providing structural type checking at type-check time (with mypy or pyright). But it is optional and not enforced at runtime. Rust's traits are enforced at compile time, always.

The practical result: when you add a new method to the `Provider` trait, the compiler immediately shows you every adapter that needs updating. In Python, you would discover missing methods through test failures -- or through production errors if test coverage is incomplete.

## Adding a New Provider

The architecture makes it straightforward to add support for new providers. Here is the checklist:

1. **Create the adapter file** (`src/provider/newprovider.rs`). Implement the `Provider` trait, translating between your generic types and the new provider's API format.

2. **Add to the capabilities registry.** Register the new provider's models with their capabilities in `register_defaults()` or through configuration.

3. **Add to the configuration schema.** Add a new config section for the provider's settings (API key, base URL, default model).

4. **Update `build_named_provider`.** Add a match arm that constructs the new provider from configuration.

5. **Write tests.** Add recorded response tests, a mock server, and run the contract tests against the new adapter.

No existing code needs to change. The agentic loop, the tool system, the fallback chain, the cost tracker -- they all work with the new provider automatically because they depend on the trait, not on any specific adapter.

```rust
// Step 1: The adapter
pub struct MistralProvider { /* ... */ }

#[async_trait]
impl Provider for MistralProvider {
    fn name(&self) -> &str { "mistral" }
    fn model(&self) -> &str { &self.model }

    async fn send_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
        max_tokens: u32,
    ) -> Result<ProviderResponse, ProviderError> {
        // Translate to Mistral's API format and send
        todo!()
    }

    async fn stream_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
        max_tokens: u32,
    ) -> Result<StreamHandle, ProviderError> {
        todo!()
    }
}
```

Since many providers now follow the OpenAI chat completions format, you can often reuse the OpenAI adapter with a different base URL. The `with_base_url` constructor handles providers like Together AI, Groq, Anyscale, and any vLLM deployment with no adapter code at all.

## What Comes Next

With provider abstraction in place, your agent is now truly flexible:

- **Chapter 14** builds on this foundation with the extensibility and plugin system, where third-party plugins can register their own providers.
- **Chapter 15** covers production polish, where the provider system gets monitoring, logging, and graceful degradation under load.

The multi-provider system is one of the most impactful features you can add to a coding agent. It transforms the agent from a single-vendor tool into a platform that adapts to each user's needs, budget, and constraints.

## Key Takeaways

- The provider system has four layers: the trait (contract), adapters (translation), runtime features (intelligence), and configuration (setup) -- each with clear boundaries
- The minimal trait design (four methods) keeps adapters simple and makes adding new providers a single-file change
- Composition patterns (fallback wrapping providers, switcher containing a provider, cost tracker decorating calls) build complex behavior from simple pieces
- Rust's trait-based polymorphism catches missing method implementations at compile time, unlike Python's duck typing which catches them at runtime
- Adding a new provider requires only an adapter file, a capabilities entry, a config section, and tests -- no existing code changes
