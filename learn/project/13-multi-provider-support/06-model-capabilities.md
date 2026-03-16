---
title: Model Capabilities
description: Defining and querying model capabilities like context window size, tool calling support, vision, and extended thinking so the agent can adapt its behavior per model.
---

# Model Capabilities

> **What you'll learn:**
> - How to define a capabilities struct that describes what each model can and cannot do
> - How to use capability information to adapt prompt construction and tool selection
> - Patterns for maintaining an up-to-date capabilities registry as new models are released

Your agent now has three provider adapters, each supporting multiple models. But not all models are created equal. Claude Opus has extended thinking capabilities that Llama 3 does not. GPT-4o supports vision inputs while GPT-4o-mini has a smaller context window. A local 7B model might not handle tool calling at all. Without a capabilities system, your agent would blindly send the same prompts and tool definitions to every model, leading to failures that are hard to diagnose.

The capabilities system solves this by giving the agent metadata about each model -- what it can do, how much context it can handle, and what features it supports. The agent uses this information to adapt its behavior: trimming tools, adjusting prompts, or falling back to simpler strategies when talking to less capable models.

## The Capabilities Struct

Start with a struct that captures the dimensions along which models differ:

```rust
use serde::{Deserialize, Serialize};

/// Describes what a specific model can and cannot do.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapabilities {
    /// The model identifier (e.g., "claude-sonnet-4-20250514", "gpt-4o").
    pub model_id: String,

    /// Which provider serves this model.
    pub provider: String,

    /// Maximum input context window in tokens.
    pub max_context_tokens: u32,

    /// Maximum output tokens the model can generate.
    pub max_output_tokens: u32,

    /// Whether the model supports native tool/function calling.
    pub supports_tool_calling: bool,

    /// Whether the model supports streaming responses.
    pub supports_streaming: bool,

    /// Whether the model can process image inputs.
    pub supports_vision: bool,

    /// Whether the model supports a system prompt (separate from messages).
    pub supports_system_prompt: bool,

    /// Whether the model supports extended thinking / chain-of-thought.
    pub supports_extended_thinking: bool,

    /// Whether the model supports prompt caching.
    pub supports_caching: bool,

    /// Relative quality tier (1 = basic, 2 = standard, 3 = premium).
    pub quality_tier: u8,

    /// Cost per 1M input tokens in USD (approximate).
    pub input_cost_per_million: f64,

    /// Cost per 1M output tokens in USD (approximate).
    pub output_cost_per_million: f64,
}

impl ModelCapabilities {
    /// Estimate the cost of a request given token counts.
    pub fn estimate_cost(&self, input_tokens: u32, output_tokens: u32) -> f64 {
        let input_cost = (input_tokens as f64 / 1_000_000.0)
            * self.input_cost_per_million;
        let output_cost = (output_tokens as f64 / 1_000_000.0)
            * self.output_cost_per_million;
        input_cost + output_cost
    }
}
```

Each field captures a specific capability that affects how the agent constructs requests. The `quality_tier` is a rough ranking that helps with automatic model selection -- when the agent needs to pick between multiple available models, it can use the tier as a heuristic.

::: python Coming from Python
Python would use a dataclass or Pydantic model for this:
```python
@dataclass
class ModelCapabilities:
    model_id: str
    max_context_tokens: int
    supports_tool_calling: bool = True
    supports_vision: bool = False
    # ...
```
In Rust, `#[derive(Serialize, Deserialize)]` gives you the same default-value behavior through `#[serde(default)]`. The struct fields are checked at compile time -- you cannot accidentally access `supports_vission` (typo) without the compiler catching it.
:::

## The Capabilities Registry

You need a central registry that maps model identifiers to their capabilities. This registry is populated at startup and can be queried whenever the agent needs to make a capability-dependent decision.

```rust
use std::collections::HashMap;

pub struct CapabilityRegistry {
    models: HashMap<String, ModelCapabilities>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            models: HashMap::new(),
        };
        registry.register_defaults();
        registry
    }

    /// Look up capabilities for a model. Returns None if the model is unknown.
    pub fn get(&self, model_id: &str) -> Option<&ModelCapabilities> {
        self.models.get(model_id)
    }

    /// Register or update capabilities for a model.
    pub fn register(&mut self, capabilities: ModelCapabilities) {
        self.models.insert(capabilities.model_id.clone(), capabilities);
    }

    /// Get all models for a specific provider.
    pub fn models_for_provider(&self, provider: &str) -> Vec<&ModelCapabilities> {
        self.models.values()
            .filter(|m| m.provider == provider)
            .collect()
    }

    /// Find the cheapest model that meets minimum requirements.
    pub fn cheapest_model_matching(
        &self,
        min_context: u32,
        needs_tools: bool,
        needs_vision: bool,
    ) -> Option<&ModelCapabilities> {
        self.models.values()
            .filter(|m| m.max_context_tokens >= min_context)
            .filter(|m| !needs_tools || m.supports_tool_calling)
            .filter(|m| !needs_vision || m.supports_vision)
            .min_by(|a, b| {
                a.input_cost_per_million
                    .partial_cmp(&b.input_cost_per_million)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Find the highest quality model available.
    pub fn best_model(&self) -> Option<&ModelCapabilities> {
        self.models.values()
            .max_by_key(|m| m.quality_tier)
    }

    fn register_defaults(&mut self) {
        // Anthropic models
        self.register(ModelCapabilities {
            model_id: "claude-sonnet-4-20250514".to_string(),
            provider: "anthropic".to_string(),
            max_context_tokens: 200_000,
            max_output_tokens: 8_192,
            supports_tool_calling: true,
            supports_streaming: true,
            supports_vision: true,
            supports_system_prompt: true,
            supports_extended_thinking: false,
            supports_caching: true,
            quality_tier: 2,
            input_cost_per_million: 3.0,
            output_cost_per_million: 15.0,
        });

        self.register(ModelCapabilities {
            model_id: "claude-opus-4-20250514".to_string(),
            provider: "anthropic".to_string(),
            max_context_tokens: 200_000,
            max_output_tokens: 32_000,
            supports_tool_calling: true,
            supports_streaming: true,
            supports_vision: true,
            supports_system_prompt: true,
            supports_extended_thinking: true,
            supports_caching: true,
            quality_tier: 3,
            input_cost_per_million: 15.0,
            output_cost_per_million: 75.0,
        });

        self.register(ModelCapabilities {
            model_id: "claude-haiku-3-5-20241022".to_string(),
            provider: "anthropic".to_string(),
            max_context_tokens: 200_000,
            max_output_tokens: 8_192,
            supports_tool_calling: true,
            supports_streaming: true,
            supports_vision: true,
            supports_system_prompt: true,
            supports_extended_thinking: false,
            supports_caching: true,
            quality_tier: 1,
            input_cost_per_million: 0.80,
            output_cost_per_million: 4.0,
        });

        // OpenAI models
        self.register(ModelCapabilities {
            model_id: "gpt-4o".to_string(),
            provider: "openai".to_string(),
            max_context_tokens: 128_000,
            max_output_tokens: 16_384,
            supports_tool_calling: true,
            supports_streaming: true,
            supports_vision: true,
            supports_system_prompt: true,
            supports_extended_thinking: false,
            supports_caching: false,
            quality_tier: 2,
            input_cost_per_million: 2.50,
            output_cost_per_million: 10.0,
        });

        self.register(ModelCapabilities {
            model_id: "gpt-4o-mini".to_string(),
            provider: "openai".to_string(),
            max_context_tokens: 128_000,
            max_output_tokens: 16_384,
            supports_tool_calling: true,
            supports_streaming: true,
            supports_vision: true,
            supports_system_prompt: true,
            supports_extended_thinking: false,
            supports_caching: false,
            quality_tier: 1,
            input_cost_per_million: 0.15,
            output_cost_per_million: 0.60,
        });

        // Ollama / local models (costs are 0 since they run locally)
        self.register(ModelCapabilities {
            model_id: "llama3:latest".to_string(),
            provider: "ollama".to_string(),
            max_context_tokens: 8_192,
            max_output_tokens: 4_096,
            supports_tool_calling: false,
            supports_streaming: true,
            supports_vision: false,
            supports_system_prompt: true,
            supports_extended_thinking: false,
            supports_caching: false,
            quality_tier: 1,
            input_cost_per_million: 0.0,
            output_cost_per_million: 0.0,
        });

        self.register(ModelCapabilities {
            model_id: "qwen2.5-coder:latest".to_string(),
            provider: "ollama".to_string(),
            max_context_tokens: 32_768,
            max_output_tokens: 8_192,
            supports_tool_calling: true,
            supports_streaming: true,
            supports_vision: false,
            supports_system_prompt: true,
            supports_extended_thinking: false,
            supports_caching: false,
            quality_tier: 1,
            input_cost_per_million: 0.0,
            output_cost_per_million: 0.0,
        });
    }
}
```

## Adapting Behavior Based on Capabilities

The real value of the capabilities system is in how the agent uses it. Here are the key adaptation points:

### Tool Selection

If a model does not support tool calling, the agent can either skip tools entirely or embed them in the system prompt as text:

```rust
fn prepare_tools_for_model(
    all_tools: &[ToolDefinition],
    capabilities: &ModelCapabilities,
) -> Vec<ToolDefinition> {
    if !capabilities.supports_tool_calling {
        // Return empty -- the caller should embed tools in the system prompt
        return Vec::new();
    }

    // For models with smaller context windows, limit the number of tools
    // to avoid consuming too much of the context with tool definitions
    if capabilities.max_context_tokens < 16_000 {
        // Keep only the most essential tools
        all_tools.iter()
            .filter(|t| is_essential_tool(&t.name))
            .cloned()
            .collect()
    } else {
        all_tools.to_vec()
    }
}

fn is_essential_tool(name: &str) -> bool {
    matches!(name, "read_file" | "write_file" | "shell" | "search")
}
```

### Prompt Adjustment

Smaller models benefit from more explicit, concise prompts. A capabilities-aware prompt builder can adjust:

```rust
fn build_system_prompt(
    base_prompt: &str,
    capabilities: &ModelCapabilities,
    tools: &[ToolDefinition],
) -> String {
    let mut prompt = base_prompt.to_string();

    // For models without native tool calling, embed tool descriptions
    if !capabilities.supports_tool_calling && !tools.is_empty() {
        prompt.push_str("\n\n## Available Tools\n");
        prompt.push_str("Respond with JSON to call a tool:\n");
        prompt.push_str("```json\n{\"tool\": \"name\", \"input\": {...}}\n```\n\n");
        for tool in tools {
            prompt.push_str(&format!("### {}\n{}\n\n", tool.name, tool.description));
        }
    }

    // For models with small context windows, trim verbose instructions
    if capabilities.max_context_tokens < 16_000 {
        // Use a condensed version of the prompt
        prompt = condense_prompt(&prompt, capabilities.max_context_tokens);
    }

    prompt
}

fn condense_prompt(prompt: &str, max_tokens: u32) -> String {
    // Rough estimate: 1 token ~= 4 characters
    let max_chars = (max_tokens as usize / 4).min(prompt.len());
    if prompt.len() <= max_chars {
        prompt.to_string()
    } else {
        // Keep the first portion and add a truncation notice
        let truncated = &prompt[..max_chars.saturating_sub(50)];
        format!("{}...\n[System prompt truncated for context limit]", truncated)
    }
}
```

### Max Tokens Adjustment

Never request more output tokens than the model supports:

```rust
fn effective_max_tokens(requested: u32, capabilities: &ModelCapabilities) -> u32 {
    requested.min(capabilities.max_output_tokens)
}
```

## Querying Capabilities at Runtime

Integrate the registry into your agent so it can query capabilities before each request:

```rust
use std::sync::Arc;

pub struct Agent {
    provider: Arc<dyn Provider>,
    capabilities: Arc<CapabilityRegistry>,
    system_prompt: String,
    tools: Vec<ToolDefinition>,
    messages: Vec<Message>,
}

impl Agent {
    pub async fn send(&self) -> Result<ProviderResponse, ProviderError> {
        let model = self.provider.model();
        let caps = self.capabilities.get(model);

        // Adapt tools based on model capabilities
        let tools = match caps {
            Some(c) => prepare_tools_for_model(&self.tools, c),
            None => self.tools.clone(), // Unknown model, send everything
        };

        // Adapt max tokens
        let max_tokens = match caps {
            Some(c) => effective_max_tokens(4096, c),
            None => 4096,
        };

        // Adapt system prompt
        let system = match caps {
            Some(c) => build_system_prompt(&self.system_prompt, c, &self.tools),
            None => self.system_prompt.clone(),
        };

        self.provider.send_message(&system, &self.messages, &tools, max_tokens).await
    }
}
```

When the model is unknown (not in the registry), the agent falls back to sending everything unmodified. This keeps the system functional even for newly released models that have not been added to the registry yet.

::: wild In the Wild
Claude Code adapts its behavior based on the model being used. When running with a less capable model, it adjusts tool selection and prompt complexity. OpenCode maintains a model registry that tracks context window sizes and capabilities, using this information to manage conversation history and decide when to compact context.
:::

## Key Takeaways

- A `ModelCapabilities` struct captures the dimensions along which models differ: context size, tool support, vision, cost, and quality tier
- The `CapabilityRegistry` maps model identifiers to capabilities and provides query methods for finding models that match specific requirements
- The agent uses capabilities to adapt three key areas: which tools to include, how to construct the system prompt, and how many output tokens to request
- Unknown models get a permissive default -- send everything and let the API reject what it cannot handle, rather than failing preemptively
- The `cheapest_model_matching` query enables automatic cost optimization by finding the cheapest model that can handle a specific task
