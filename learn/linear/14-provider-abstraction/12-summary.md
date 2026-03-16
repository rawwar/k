---
title: Summary
description: Review the complete provider abstraction architecture and its role in making the coding agent flexible and provider-agnostic.
---

# Summary

> **What you'll learn:**
> - How the trait, adapter, registry, and fallback components compose into a complete provider abstraction layer
> - The key design decisions that determine whether adding a new provider is a one-file change or a codebase-wide refactor
> - Guidelines for maintaining the provider layer as APIs evolve and new model capabilities become available

You have built a complete provider abstraction layer over the course of this chapter. Let's step back and see how all the pieces fit together, what you have gained, and what to watch for as you maintain it going forward.

## The Full Architecture

Here is the layered structure you have built, from bottom to top:

**Layer 1: Provider Trait and Canonical Types.** The `Provider` trait defines four methods: `send_message`, `stream_message`, `capabilities`, and `name`/`model`. The canonical types — `ChatRequest`, `ChatResponse`, `Message`, `ContentBlock`, `StreamEvent` — form the language your agent speaks internally. No provider-specific types cross this boundary.

**Layer 2: Concrete Adapters.** Three adapter structs implement the trait: `AnthropicProvider` (Messages API), `OpenAiProvider` (Chat Completions API), and `OllamaProvider` (local REST API). Each adapter translates between your canonical types and the provider's native format, handling all the structural differences — system prompts as messages vs parameters, tool calls as content blocks vs separate arrays, arguments as JSON objects vs JSON strings.

**Layer 3: Model Registry.** The `ModelRegistry` centralizes knowledge about every model — capabilities, pricing, aliases, and provider association. Any component that needs to know "does this model support tools?" or "how much does this model cost?" queries the registry rather than hardcoding that information.

**Layer 4: Decorator Stack.** Three decorator providers wrap the base adapters:
- `RetryProvider` adds exponential backoff with jitter for transient failures.
- `FallbackProvider` routes to backup providers when the primary is exhausted.
- `TrackingProvider` records usage and cost for every API call.

Each decorator implements `Provider`, so they compose transparently.

**Layer 5: Runtime Switching.** The agent holds the provider behind `Arc<RwLock<Box<dyn Provider>>>`, enabling hot-swapping via the `/model` command. Conversation history transfers automatically because it is stored in canonical types.

```
┌─────────────────────────────────────────────────┐
│                  Agent / REPL                    │
│  ┌───────────────────────────────────────────┐  │
│  │         Arc<RwLock<Box<dyn Provider>>>     │  │
│  │  ┌─────────────────────────────────────┐  │  │
│  │  │         TrackingProvider             │  │  │
│  │  │  ┌───────────────────────────────┐  │  │  │
│  │  │  │       RetryProvider           │  │  │  │
│  │  │  │  ┌─────────────────────────┐  │  │  │  │
│  │  │  │  │    FallbackProvider     │  │  │  │  │
│  │  │  │  │  ┌───────┐ ┌────────┐  │  │  │  │  │
│  │  │  │  │  │Anthro.│ │OpenAI  │  │  │  │  │  │
│  │  │  │  │  └───────┘ └────────┘  │  │  │  │  │
│  │  │  │  └─────────────────────────┘  │  │  │  │
│  │  │  └───────────────────────────────┘  │  │  │
│  │  └─────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────┘  │
│                 ModelRegistry                    │
└─────────────────────────────────────────────────┘
```

## What You Gained

The provider abstraction delivers several concrete benefits:

**Adding a new provider is a single-file change.** To support a new LLM backend — say, Google Gemini — you create one module with a struct that implements `Provider`. You write the translation between your canonical types and Gemini's API format. You add model entries to the registry. No code in the agentic loop, tool system, or UI changes. No existing adapter is modified.

**Testing is clean and fast.** The `MockProvider` lets you test your entire agent logic without any HTTP calls. Mock HTTP servers test adapter serialization. Contract tests verify cross-adapter consistency. None of these require API keys or real API calls.

**Users can switch models without losing context.** Because conversation history is stored in canonical types, switching from Claude to GPT-4o mid-session preserves the full conversation. The adapter translates the history into the new provider's format transparently.

**Failures are handled gracefully.** The retry and fallback decorators absorb transient failures without the agent's core logic knowing about them. The user experiences fewer errors, and when errors do reach the surface, they are classified (auth vs rate limit vs server error) with appropriate recovery guidance.

**Cost visibility is built in.** Every API call is tracked with token counts, estimated cost, and latency. Users can see where their tokens go and set budgets to prevent runaway spending.

::: python Coming from Python
If you have built a similar abstraction in Python, you might have used `ABC` or `Protocol` for the interface and `@retry` decorators for resilience. The Rust version achieves the same goals with stronger guarantees: the compiler enforces that every adapter implements every method, `Send + Sync` bounds guarantee thread safety, and `pub(crate)` visibility prevents provider types from leaking. The trade-off is more upfront code — but the errors you avoid at runtime are worth the investment.
:::

## Design Decisions That Mattered

Looking back across the chapter, several decisions had outsized impact on the quality of the abstraction:

**Small trait surface.** The `Provider` trait has four or five methods. A larger trait would have been harder to implement for each provider and would change more often. The small surface is why adding a new provider is straightforward.

**Canonical types with Option fields.** Using `Option<Vec<ToolDefinition>>` for tools and `HashMap<String, Value>` for extensions lets you represent everything from a simple text exchange to a complex multi-tool interaction with provider-specific features, all in the same types.

**Decorator pattern for cross-cutting concerns.** Retry, tracking, and fallback are implemented as decorators rather than being baked into the adapters. This means you can change the retry strategy without touching any adapter, add tracking without modifying the retry logic, and test each concern in isolation.

**Capabilities as data, not code.** The `ModelCapabilities` struct and `ModelRegistry` represent capability knowledge as data rather than if-else chains scattered through the codebase. When a model gains a new capability, you update a data entry, not logic spread across multiple files.

**Dynamic dispatch for flexibility.** Using `Box<dyn Provider>` and `Arc<RwLock<...>>` instead of generics means the provider type can change at runtime. The cost — one vtable lookup per method call — is negligible compared to the latency of an API call.

## Maintaining the Provider Layer

As provider APIs evolve, you will need to update the abstraction. Here are guidelines for keeping it healthy:

**When a provider changes its API:** update the provider-specific serde types in the adapter's `types.rs` module and adjust the translation methods. If the change is additive (a new optional field), your existing code continues to work because `serde` skips unknown fields by default.

**When a provider adds a new feature:** if the feature is provider-specific (like Anthropic's cache control), route it through the `extensions` map. If multiple providers support the same feature, consider promoting it to a proper field on `ChatRequest` or `ChatResponse`.

**When adding a new capability to the trait:** add a field to `ModelCapabilities` with a `false` default. Existing providers are unaffected. Only update the adapters that actually support the new capability.

**When adding a new provider:** create a new module under `src/provider/`, implement the `Provider` trait, add model entries to the registry, and add a case to the factory function. Write contract tests against a mock HTTP server. No existing code changes.

**When updating pricing:** update `ModelPricing` entries in the registry's `register_defaults` method, or better yet, load pricing from a configuration file that can be updated without recompilation.

::: wild In the Wild
Claude Code and OpenCode both evolve their provider layers as new models and features appear. The pattern of keeping provider-specific code isolated in adapter modules means these updates are low-risk and localized. Production agents also version their provider APIs — keeping the `anthropic-version` header pinned to a known-working version and only upgrading after testing with the new version.
:::

## What to Build Next

The provider abstraction is a foundation. Several extensions build naturally on top of it:

- **Provider-specific optimizations**: prompt caching for Anthropic, batch API for OpenAI — implemented in the adapters without touching the core.
- **Model routing**: automatically selecting the cheapest model that meets the task's capability requirements, using the registry.
- **Usage analytics**: long-term tracking across sessions, stored to disk, enabling cost reports over time.
- **Provider health monitoring**: tracking error rates per provider to inform fallback decisions automatically, rather than waiting for failures.

Each of these is a new module or decorator, not a modification to existing code. That is the hallmark of a well-designed abstraction: it grows by addition, not by surgery.

## Key Takeaways

- The complete provider abstraction is five layers: trait and types, concrete adapters, model registry, decorator stack (retry, fallback, tracking), and runtime switching.
- Adding a new provider means writing one adapter module and adding registry entries. No existing code is modified — the system is open for extension but closed for modification.
- Design decisions that pay off the most: small trait surface, canonical types with optional fields, decorator pattern for cross-cutting concerns, capabilities as data, and dynamic dispatch for runtime flexibility.
- Maintaining the provider layer follows clear patterns: API changes update the adapter's serde types, new features route through extensions or become proper fields, new capabilities add fields with false defaults, and new providers are self-contained modules.
- The abstraction grows by addition, not modification — new decorators, new adapters, and new registry entries compose on top of the existing structure without disturbing it.
