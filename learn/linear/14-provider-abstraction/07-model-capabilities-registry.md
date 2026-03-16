---
title: Model Capabilities Registry
description: Design a registry that tracks which features each model and provider combination supports, enabling intelligent feature negotiation.
---

# Model Capabilities Registry

> **What you'll learn:**
> - How to model capabilities (tool use, streaming, vision, extended context) as a structured registry that the agent queries before making API calls
> - Strategies for keeping capability data current as providers add features, including static definitions, runtime probing, and hybrid approaches
> - How to use capability information to automatically select the best available model for a given task or gracefully disable unsupported features

So far, each provider adapter hardcodes its own `capabilities_for_model` function. That works for three providers, but as you add models and features, the capability data grows into something that deserves its own structure. A capabilities registry centralizes this knowledge, making it queryable by any part of the agent — not just the provider adapters.

## Why a Registry?

The agent needs capability information in several places:

- The **agentic loop** checks whether the current model supports tool use before sending tools in the request.
- The **UI layer** displays model info and warns users about limitations.
- The **fallback logic** needs to find a backup model that supports the same features.
- The **cost tracker** looks up per-model pricing.

Without a registry, you scatter this knowledge across the codebase. With one, every component queries a single source of truth.

## The Registry Data Model

Start with a `ModelInfo` struct that captures everything the agent needs to know about a model:

```rust
use std::collections::HashMap;

/// Complete information about a specific model.
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// The model identifier used in API calls (e.g., "claude-sonnet-4-20250514").
    pub model_id: String,
    /// Human-readable display name (e.g., "Claude Sonnet 4").
    pub display_name: String,
    /// Which provider serves this model.
    pub provider: ProviderKind,
    /// What this model can do.
    pub capabilities: ModelCapabilities,
    /// Pricing per million tokens.
    pub pricing: ModelPricing,
    /// Alternative model IDs that resolve to this model (e.g., "claude-sonnet-4").
    pub aliases: Vec<String>,
}

/// Identifies which provider backend to use.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    Anthropic,
    OpenAi,
    Ollama,
}

/// Pricing information for cost tracking.
#[derive(Debug, Clone, Default)]
pub struct ModelPricing {
    /// Cost per 1M input tokens in USD.
    pub input_per_million: f64,
    /// Cost per 1M output tokens in USD.
    pub output_per_million: f64,
    /// Cost per 1M cached input tokens (if supported).
    pub cached_input_per_million: Option<f64>,
}

/// The capabilities struct from earlier, now used registry-wide.
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

The `ProviderKind` enum connects a model to its provider adapter. When the user requests a model, the registry tells the agent which provider to instantiate.

## Building the Registry

The registry is a `HashMap` keyed by model ID, with alias resolution:

```rust
pub struct ModelRegistry {
    models: HashMap<String, ModelInfo>,
    /// Maps aliases to canonical model IDs.
    aliases: HashMap<String, String>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            models: HashMap::new(),
            aliases: HashMap::new(),
        };
        registry.register_defaults();
        registry
    }

    /// Look up a model by ID or alias.
    pub fn get(&self, model_id: &str) -> Option<&ModelInfo> {
        // Try direct lookup first
        if let Some(info) = self.models.get(model_id) {
            return Some(info);
        }
        // Try alias resolution
        if let Some(canonical) = self.aliases.get(model_id) {
            return self.models.get(canonical);
        }
        None
    }

    /// Register a new model.
    pub fn register(&mut self, info: ModelInfo) {
        for alias in &info.aliases {
            self.aliases.insert(alias.clone(), info.model_id.clone());
        }
        self.models.insert(info.model_id.clone(), info);
    }

    /// Find all models matching a capability requirement.
    pub fn find_models_with(&self, requirement: &dyn Fn(&ModelCapabilities) -> bool) -> Vec<&ModelInfo> {
        self.models.values()
            .filter(|info| requirement(&info.capabilities))
            .collect()
    }

    /// List all models for a given provider.
    pub fn models_for_provider(&self, provider: &ProviderKind) -> Vec<&ModelInfo> {
        self.models.values()
            .filter(|info| &info.provider == provider)
            .collect()
    }
}
```

The `find_models_with` method accepts a closure, giving callers flexible querying:

```rust
let registry = ModelRegistry::new();

// Find all models that support tool use
let tool_models = registry.find_models_with(&|caps| caps.supports_tools);

// Find models with large context windows
let large_context = registry.find_models_with(&|caps| caps.max_context_tokens >= 100_000);

// Find models that support both tools and vision
let multimodal_tool_models = registry.find_models_with(&|caps| {
    caps.supports_tools && caps.supports_vision
});
```

::: python Coming from Python
In Python, you might use a dictionary of dictionaries for a registry, querying with list comprehensions. Rust's approach uses typed structs and closures, giving you compile-time guarantees that capability checks reference real fields. If you typo `supports_tols` instead of `supports_tools`, the Rust compiler catches it immediately. Python's dictionary approach would silently return `None` for the missing key.
:::

## Populating Default Models

The `register_defaults` method populates the registry with known models:

```rust
impl ModelRegistry {
    fn register_defaults(&mut self) {
        // Anthropic models
        self.register(ModelInfo {
            model_id: "claude-sonnet-4-20250514".into(),
            display_name: "Claude Sonnet 4".into(),
            provider: ProviderKind::Anthropic,
            capabilities: ModelCapabilities {
                supports_tools: true,
                supports_streaming: true,
                supports_vision: true,
                supports_extended_thinking: true,
                supports_prompt_caching: true,
                max_context_tokens: 200_000,
                max_output_tokens: 16_384,
            },
            pricing: ModelPricing {
                input_per_million: 3.0,
                output_per_million: 15.0,
                cached_input_per_million: Some(0.30),
            },
            aliases: vec!["claude-sonnet-4".into(), "sonnet".into()],
        });

        self.register(ModelInfo {
            model_id: "claude-3-5-haiku-20241022".into(),
            display_name: "Claude 3.5 Haiku".into(),
            provider: ProviderKind::Anthropic,
            capabilities: ModelCapabilities {
                supports_tools: true,
                supports_streaming: true,
                supports_vision: false,
                supports_extended_thinking: false,
                supports_prompt_caching: true,
                max_context_tokens: 200_000,
                max_output_tokens: 8_192,
            },
            pricing: ModelPricing {
                input_per_million: 0.80,
                output_per_million: 4.0,
                cached_input_per_million: Some(0.08),
            },
            aliases: vec!["haiku".into()],
        });

        // OpenAI models
        self.register(ModelInfo {
            model_id: "gpt-4o".into(),
            display_name: "GPT-4o".into(),
            provider: ProviderKind::OpenAi,
            capabilities: ModelCapabilities {
                supports_tools: true,
                supports_streaming: true,
                supports_vision: true,
                supports_extended_thinking: false,
                supports_prompt_caching: false,
                max_context_tokens: 128_000,
                max_output_tokens: 16_384,
            },
            pricing: ModelPricing {
                input_per_million: 2.50,
                output_per_million: 10.0,
                cached_input_per_million: None,
            },
            aliases: vec!["4o".into()],
        });

        self.register(ModelInfo {
            model_id: "gpt-4o-mini".into(),
            display_name: "GPT-4o Mini".into(),
            provider: ProviderKind::OpenAi,
            capabilities: ModelCapabilities {
                supports_tools: true,
                supports_streaming: true,
                supports_vision: true,
                supports_extended_thinking: false,
                supports_prompt_caching: false,
                max_context_tokens: 128_000,
                max_output_tokens: 16_384,
            },
            pricing: ModelPricing {
                input_per_million: 0.15,
                output_per_million: 0.60,
                cached_input_per_million: None,
            },
            aliases: vec!["4o-mini".into(), "mini".into()],
        });

        // Ollama models (no pricing - they're free to run locally)
        self.register(ModelInfo {
            model_id: "qwen2.5-coder:7b".into(),
            display_name: "Qwen 2.5 Coder 7B".into(),
            provider: ProviderKind::Ollama,
            capabilities: ModelCapabilities {
                supports_tools: true,
                supports_streaming: true,
                supports_vision: false,
                supports_extended_thinking: false,
                supports_prompt_caching: false,
                max_context_tokens: 32_768,
                max_output_tokens: 4_096,
            },
            pricing: ModelPricing::default(), // Free
            aliases: vec!["qwen-coder".into()],
        });
    }
}
```

## Using the Registry for Feature Negotiation

The registry enables intelligent feature negotiation. Before sending tools in a request, the agent checks whether the model supports them:

```rust
impl Agent {
    async fn send_request(&self, request: &mut ChatRequest) -> Result<ChatResponse, ProviderError> {
        let capabilities = self.provider.capabilities();

        // Strip tools if the model doesn't support them
        if !capabilities.supports_tools {
            request.tools = None;
            // Optionally inject tool descriptions into the system prompt
            // so the model can still reason about tool use in text form
        }

        // Warn if the conversation might exceed the context window
        let estimated_tokens = self.estimate_token_count(&request.messages);
        if estimated_tokens > capabilities.max_context_tokens {
            eprintln!(
                "Warning: estimated {} tokens exceeds model limit of {}",
                estimated_tokens, capabilities.max_context_tokens
            );
            // Trigger context compaction...
        }

        self.provider.send_message(request.clone()).await
    }
}
```

The registry also supports model selection. If the user asks for a task that requires vision, the agent can find the best available model:

```rust
impl Agent {
    fn suggest_model_for_task(&self, needs_vision: bool, needs_tools: bool) -> Option<&ModelInfo> {
        let candidates = self.registry.find_models_with(&|caps| {
            (!needs_vision || caps.supports_vision)
                && (!needs_tools || caps.supports_tools)
        });

        // Prefer the current provider, then sort by context window size
        candidates.into_iter()
            .max_by_key(|info| info.capabilities.max_context_tokens)
    }
}
```

## Dynamic Registry Updates

For Ollama, the set of available models depends on what the user has downloaded. You can extend the registry at runtime by querying Ollama's model list:

```rust
impl ModelRegistry {
    /// Discover Ollama models and add them to the registry.
    pub async fn discover_ollama_models(&mut self, base_url: &str) -> Result<(), ProviderError> {
        let client = reqwest::Client::new();
        let response = client
            .get(format!("{base_url}/api/tags"))
            .send()
            .await
            .map_err(|_| ProviderError::Other("Cannot connect to Ollama".into()))?;

        let body: OllamaTagsResponse = response.json().await
            .map_err(|e| ProviderError::Other(format!("Failed to parse Ollama tags: {e}")))?;

        for model in body.models {
            let model_id = model.name.clone();
            if self.models.contains_key(&model_id) {
                continue; // Already registered
            }
            self.register(ModelInfo {
                model_id: model.name.clone(),
                display_name: model.name.clone(),
                provider: ProviderKind::Ollama,
                capabilities: OllamaProvider::infer_capabilities(&model.name),
                pricing: ModelPricing::default(),
                aliases: vec![],
            });
        }

        Ok(())
    }
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelTag>,
}

#[derive(Deserialize)]
struct OllamaModelTag {
    name: String,
}
```

This hybrid approach — static definitions for cloud models, dynamic discovery for local models — gives you the best of both worlds. Cloud model capabilities are known in advance and change infrequently. Local model availability changes whenever the user pulls or removes a model.

::: wild In the Wild
Claude Code maintains an internal registry of model capabilities that it uses to determine which features to enable for each request. When a new model is released, the registry is updated in the configuration rather than in the provider adapter code. OpenCode takes a similar approach, storing model metadata in its configuration system so that adding support for a new model does not require code changes.
:::

## Key Takeaways

- A `ModelRegistry` centralizes model metadata — capabilities, pricing, aliases, and provider association — in a single queryable data structure.
- Alias resolution lets users type `sonnet` instead of `claude-sonnet-4-20250514`, improving ergonomics while maintaining precise model identification internally.
- Capability querying with closures (`find_models_with`) supports flexible searches like "models with tools and vision" without building specialized query methods for every combination.
- A hybrid approach works best: static registration for cloud models whose capabilities are known in advance, plus dynamic discovery for local models through Ollama's tags endpoint.
- Feature negotiation — checking capabilities before sending requests — prevents runtime errors from requesting features a model does not support, like sending tools to a model without tool support.
