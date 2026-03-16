---
title: Wiring Components
description: Implement the dependency injection and component wiring that connects all agent subsystems into a functioning whole.
---

# Wiring Components

> **What you'll learn:**
> - How to wire agent components together using constructor injection, avoiding global state and enabling testability
> - Techniques for managing shared state (conversation history, configuration, usage tracking) across components without tight coupling
> - The builder pattern for agent construction — assembling the agent from its parts with sensible defaults and optional overrides

The architecture review gave you the map. Now you need to connect the boxes. Wiring is the process of instantiating each component, handing it the dependencies it needs, and assembling the result into a functioning agent. This sounds straightforward, but in practice it involves real decisions: do you use trait objects or generics? How do you share mutable state between the loop and the tools? How do you keep the wiring testable without building a full dependency injection framework?

In this subchapter, you will build the `Agent` struct and its builder, implementing the wiring patterns that production agents use.

## The Wiring Problem

Consider what the agentic loop needs to do its job:

- A **provider** to call the LLM API
- A **tool registry** to look up and execute tools
- A **safety layer** to gate tool calls
- A **context manager** to track tokens and conversation history
- A **renderer** to display streamed output
- A **configuration** to control behavior

Each of these is a distinct subsystem. Some of them need to share state — the context manager and the loop both touch the conversation history. Some of them need to reference each other — the safety layer needs access to the configuration to know which paths are allowed. The wiring layer is where you decide how these dependencies connect.

::: python Coming from Python
In Python, you might wire components together with simple constructor arguments, or use a dependency injection library like `dependency-injector`. You might also rely on module-level singletons — a `config.py` that exports a global config object. In Rust, the type system and ownership model actively push you away from global mutable state and toward explicit dependency passing. This is initially more verbose but pays off enormously in testability and clarity about who owns what.
:::

## Constructor Injection

The simplest and most effective wiring technique in Rust is constructor injection — passing dependencies as arguments to `new()` or a builder method. Here's what the core `Agent` struct looks like:

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Agent {
    config: Arc<Config>,
    provider: Box<dyn Provider>,
    tool_registry: ToolRegistry,
    safety: SafetyLayer,
    context: Arc<RwLock<ContextManager>>,
    renderer: Box<dyn Renderer>,
}
```

Notice the types carefully:

- `Arc<Config>` — the configuration is shared (multiple components read it) but immutable after startup, so `Arc` suffices without a lock.
- `Box<dyn Provider>` — the provider is a trait object, allowing runtime polymorphism. The agent does not know whether it is talking to Anthropic or OpenAI.
- `ToolRegistry` — owned directly. The registry manages its own collection of tools.
- `SafetyLayer` — also owned directly. It holds a reference to the config via `Arc<Config>`.
- `Arc<RwLock<ContextManager>>` — the context manager is both shared and mutable. Multiple components need to read token counts, and the loop needs to write new messages. `RwLock` allows concurrent reads with exclusive writes.
- `Box<dyn Renderer>` — the renderer is a trait object, enabling different output modes (plain text, TUI, JSON for piping).

## The Builder Pattern

Constructing an `Agent` directly is unwieldy — you would need to pass six arguments in the right order. The builder pattern gives you a fluent API with sensible defaults:

```rust
pub struct AgentBuilder {
    config: Option<Config>,
    model_override: Option<String>,
    provider_override: Option<Box<dyn Provider>>,
    additional_tools: Vec<Box<dyn Tool>>,
    renderer_override: Option<Box<dyn Renderer>>,
    verbose: bool,
}

impl AgentBuilder {
    pub fn new() -> Self {
        Self {
            config: None,
            model_override: None,
            provider_override: None,
            additional_tools: Vec::new(),
            renderer_override: None,
            verbose: false,
        }
    }

    pub fn with_config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    pub fn with_model_override(mut self, model: Option<String>) -> Self {
        self.model_override = model;
        self
    }

    pub fn with_tool(mut self, tool: Box<dyn Tool>) -> Self {
        self.additional_tools.push(tool);
        self
    }

    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    pub async fn build(self) -> anyhow::Result<Agent> {
        // Step 1: Configuration (foundation layer)
        let config = Arc::new(self.config.unwrap_or_else(Config::default));

        // Step 2: Provider (depends on config)
        let provider = match self.provider_override {
            Some(p) => p,
            None => create_provider(&config, self.model_override.as_deref())?,
        };

        // Step 3: Tool registry (depends on config)
        let mut tool_registry = ToolRegistry::new_with_defaults(&config);
        for tool in self.additional_tools {
            tool_registry.register(tool);
        }

        // Step 4: Safety layer (depends on config)
        let safety = SafetyLayer::new(Arc::clone(&config));

        // Step 5: Context manager (depends on config and provider)
        let max_tokens = provider.context_window_size();
        let context = Arc::new(RwLock::new(
            ContextManager::new(max_tokens, &config)
        ));

        // Step 6: Renderer (depends on config)
        let renderer = match self.renderer_override {
            Some(r) => r,
            None => create_renderer(&config, self.verbose)?,
        };

        Ok(Agent {
            config,
            provider,
            tool_registry,
            safety,
            context,
            renderer,
        })
    }
}

impl Agent {
    pub fn builder() -> AgentBuilder {
        AgentBuilder::new()
    }
}
```

The `build()` method is where the actual wiring happens. Notice the explicit ordering: config first (everything depends on it), then provider (context manager needs the token limit), then tools, safety, context, and renderer. This ordering is not arbitrary — it follows the dependency graph.

## Shared State with `Arc` and `RwLock`

The most nuanced wiring decision involves shared mutable state. In a coding agent, the primary shared state is the conversation history. The agentic loop writes new messages. The context manager reads message counts and token totals. The renderer might read the last message for formatting purposes.

Rust's ownership model forces you to be explicit about this. You cannot have two mutable references to the same data. `Arc<RwLock<T>>` is the standard solution for shared mutable state in async Rust:

```rust
// Creating the shared context
let context = Arc::new(RwLock::new(ContextManager::new(max_tokens, &config)));

// In the agentic loop (needs write access)
let mut ctx = context.write().await;
ctx.add_message(user_message);
ctx.add_message(assistant_message);
drop(ctx); // Release the write lock before the next await point

// In a monitoring component (only needs read access)
let ctx = context.read().await;
let token_count = ctx.total_tokens();
let message_count = ctx.message_count();
```

The `RwLock` allows multiple simultaneous readers or one exclusive writer. This is the right primitive for conversation history because reads are far more frequent than writes.

::: wild In the Wild
Claude Code manages shared state through a centralized conversation store that components access through controlled interfaces rather than direct shared references. OpenCode uses Go's channel-based concurrency model instead — components communicate by passing messages rather than sharing memory. Both approaches avoid the pitfalls of uncontrolled global state, but they reflect the idioms of their respective languages. In Rust, `Arc<RwLock<T>>` is the idiomatic equivalent: explicit, compiler-verified, and zero-overhead when uncontended.
:::

## Trait Objects vs. Generics

You might wonder why the `Agent` struct uses `Box<dyn Provider>` instead of a generic type parameter like `Agent<P: Provider>`. Both work, but they have different tradeoffs.

**Trait objects** (`Box<dyn Provider>`) use dynamic dispatch — a vtable lookup at runtime. They enable runtime polymorphism: you can switch providers based on a config string without the calling code knowing the concrete type. The cost is one pointer indirection per method call.

**Generics** (`Agent<P: Provider>`) use static dispatch — the compiler monomorphizes a separate `Agent` for each concrete provider type. This is faster (no vtable) but means the provider type must be known at compile time.

For a coding agent, trait objects are the right choice for providers and renderers because these are selected at runtime based on user configuration. The performance cost of dynamic dispatch is negligible compared to network latency. Use generics for performance-critical inner loops where the concrete type is known at compile time.

```rust
// Runtime polymorphism — selected based on config
fn create_provider(config: &Config, model_override: Option<&str>) -> anyhow::Result<Box<dyn Provider>> {
    let provider_name = model_override
        .and_then(|m| infer_provider_from_model(m))
        .unwrap_or(&config.default_provider);

    match provider_name.as_str() {
        "anthropic" => Ok(Box::new(AnthropicProvider::new(config)?)),
        "openai" => Ok(Box::new(OpenAiProvider::new(config)?)),
        other => anyhow::bail!("Unknown provider: {other}"),
    }
}
```

## Testing the Wiring

One of the biggest payoffs of constructor injection is testability. You can substitute any component with a mock or stub by implementing the same trait:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct MockProvider {
        responses: Vec<LlmResponse>,
    }

    #[async_trait]
    impl Provider for MockProvider {
        async fn complete(&self, messages: &[Message]) -> Result<LlmResponse> {
            Ok(self.responses[0].clone())
        }

        fn context_window_size(&self) -> usize {
            128_000
        }
    }

    #[tokio::test]
    async fn test_agent_processes_simple_response() {
        let agent = Agent::builder()
            .with_config(Config::default())
            .with_provider(Box::new(MockProvider {
                responses: vec![LlmResponse::text("Hello!")],
            }))
            .build()
            .await
            .unwrap();

        let result = agent.run_once("Say hello").await.unwrap();
        assert!(result.contains("Hello!"));
    }
}
```

No real API calls. No network access. No configuration files on disk. The test controls every dependency explicitly. This is only possible because the wiring is done through constructor injection rather than global state or hardcoded dependencies.

## Avoiding the God Object

A common mistake when wiring components is creating a "god object" — a single struct that contains every piece of state and every method. The `Agent` struct should be thin. It holds references to subsystems but does not contain their logic. The agentic loop lives in its own module. Tool execution lives in the tool registry. Safety checks live in the safety layer. The `Agent` is an orchestrator, not an implementor.

```rust
impl Agent {
    pub async fn run_interactive(&self) -> anyhow::Result<()> {
        // The agent delegates to specialized subsystems
        let mut repl = Repl::new()?;

        loop {
            let input = repl.read_line()?;
            if input.is_empty() {
                continue;
            }

            // Delegate to the agentic loop
            agentic_loop::run_turn(
                &input,
                &*self.provider,
                &self.tool_registry,
                &self.safety,
                &self.context,
                &*self.renderer,
            ).await?;
        }
    }
}
```

The `Agent` method is five lines. It reads input and delegates. All the interesting logic lives in `agentic_loop::run_turn`, which we revisit in subchapter 4.

## Key Takeaways

- Constructor injection is the primary wiring technique in Rust: pass dependencies as arguments to builders or constructors rather than using global state or service locators.
- Use `Arc<T>` for shared immutable state (like configuration) and `Arc<RwLock<T>>` for shared mutable state (like conversation history) — the type system enforces correct concurrent access at compile time.
- The builder pattern provides a fluent API for agent construction with sensible defaults, optional overrides, and clear ordering of initialization steps.
- Prefer `Box<dyn Trait>` (trait objects) for components selected at runtime based on configuration, and generics for performance-critical paths where the concrete type is known at compile time.
- Keep the top-level `Agent` struct thin — it orchestrates subsystems but does not contain their logic, avoiding the god-object antipattern that makes code untestable and hard to change.
