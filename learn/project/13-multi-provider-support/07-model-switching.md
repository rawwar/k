---
title: Model Switching
description: Implementing runtime model switching that lets the agent or user change models mid-session, with proper conversation history translation between provider formats.
---

# Model Switching

> **What you'll learn:**
> - How to switch providers mid-conversation while preserving message history coherence
> - Techniques for translating conversation state between different provider message formats
> - Strategies for automatic model selection based on task complexity and cost constraints

Your agent can now talk to three different providers, but it uses whichever one was configured at startup for the entire session. In practice, users want to switch models mid-conversation -- start with a fast, cheap model for exploration, then escalate to a more capable one when they hit a complex task. Or the user might want to compare how different models approach the same problem. This subchapter builds the runtime model switching system.

## The Model Switcher

The model switcher wraps the current provider in a mutable container that can be swapped at runtime. Since the agent runs async code across multiple tasks, you need thread-safe interior mutability:

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::provider::{Provider, ProviderError};
use crate::provider::types::*;

/// Wraps a provider with the ability to switch it at runtime.
pub struct ModelSwitcher {
    current: Arc<RwLock<Arc<dyn Provider>>>,
    /// History of which models were used, in order.
    switch_history: Arc<RwLock<Vec<SwitchRecord>>>,
}

#[derive(Debug, Clone)]
pub struct SwitchRecord {
    pub from_provider: String,
    pub from_model: String,
    pub to_provider: String,
    pub to_model: String,
    pub reason: SwitchReason,
    pub timestamp: std::time::Instant,
}

#[derive(Debug, Clone)]
pub enum SwitchReason {
    UserRequested,
    CostOptimization,
    FallbackTriggered,
    TaskComplexity,
}

impl ModelSwitcher {
    pub fn new(initial: Arc<dyn Provider>) -> Self {
        Self {
            current: Arc::new(RwLock::new(initial)),
            switch_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get a reference to the current provider.
    pub async fn current(&self) -> Arc<dyn Provider> {
        self.current.read().await.clone()
    }

    /// Switch to a new provider. Returns the previous one.
    pub async fn switch_to(
        &self,
        new_provider: Arc<dyn Provider>,
        reason: SwitchReason,
    ) -> Arc<dyn Provider> {
        let mut current = self.current.write().await;
        let old = current.clone();

        let record = SwitchRecord {
            from_provider: old.name().to_string(),
            from_model: old.model().to_string(),
            to_provider: new_provider.name().to_string(),
            to_model: new_provider.model().to_string(),
            reason,
            timestamp: std::time::Instant::now(),
        };

        *current = new_provider;

        self.switch_history.write().await.push(record);

        old
    }

    /// Get the switch history for diagnostics.
    pub async fn history(&self) -> Vec<SwitchRecord> {
        self.switch_history.read().await.clone()
    }
}
```

The `RwLock` allows multiple concurrent reads (for sending messages) but exclusive access for writes (when switching). The `Arc<dyn Provider>` inside the lock is cloned on read, which means an in-flight request continues using the old provider even if a switch happens mid-request. This is the correct behavior -- you do not want a streaming response to suddenly change providers.

::: python Coming from Python
In Python, you might implement this with a simple attribute swap:
```python
class ModelSwitcher:
    def __init__(self, provider):
        self._provider = provider

    def switch_to(self, new_provider):
        old = self._provider
        self._provider = new_provider
        return old
```
Python's GIL means attribute assignment is atomic for simple types, so this is safe in threaded code. Rust does not have a GIL, so you need explicit synchronization. The `RwLock` provides the same safety guarantees while allowing true parallel reads across multiple threads.
:::

## Translating Conversation History

The trickiest part of model switching is the conversation history. Your agent has been accumulating messages in the provider-neutral format, which should work with any provider. But some subtle issues arise:

1. **Tool call IDs**: If the old provider generated tool call IDs like `toolu_abc123` (Anthropic style), the new provider needs to understand these when they appear in tool results.
2. **Content block structures**: Some providers are strict about which content block types can appear in which roles.
3. **System prompt handling**: If the old provider used extended thinking, the conversation might contain thinking blocks that the new provider does not understand.

The solution is a history translator that sanitizes the conversation when switching:

```rust
/// Clean up conversation history for compatibility with a new provider.
pub fn translate_history(
    messages: &[Message],
    target_capabilities: &ModelCapabilities,
) -> Vec<Message> {
    messages.iter().map(|msg| {
        let content: Vec<ContentBlock> = msg.content.iter().filter_map(|block| {
            match block {
                ContentBlock::Text { text } => {
                    Some(ContentBlock::Text { text: text.clone() })
                }
                ContentBlock::ToolUse { id, name, input } => {
                    if target_capabilities.supports_tool_calling {
                        Some(ContentBlock::ToolUse {
                            id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        })
                    } else {
                        // Convert tool use to text for models without tool support
                        Some(ContentBlock::Text {
                            text: format!(
                                "[Called tool '{}' with: {}]",
                                name,
                                serde_json::to_string_pretty(input)
                                    .unwrap_or_default()
                            ),
                        })
                    }
                }
                ContentBlock::ToolResult { tool_use_id, content, is_error } => {
                    if target_capabilities.supports_tool_calling {
                        Some(ContentBlock::ToolResult {
                            tool_use_id: tool_use_id.clone(),
                            content: content.clone(),
                            is_error: *is_error,
                        })
                    } else {
                        // Convert tool result to text
                        let prefix = if *is_error { "Tool error" } else { "Tool result" };
                        Some(ContentBlock::Text {
                            text: format!("[{}: {}]", prefix, content),
                        })
                    }
                }
            }
        }).collect();

        Message {
            role: msg.role.clone(),
            content,
        }
    }).collect()
}
```

When switching to a model without tool support, the translator converts tool use and tool result blocks into descriptive text blocks. The conversation remains coherent -- the new model can read the text descriptions even though it cannot make native tool calls.

## User-Initiated Switching

The simplest switching mechanism is a slash command in the agent's REPL. When the user types `/model gpt-4o` or `/model ollama:llama3`, the agent switches providers:

```rust
use crate::provider::anthropic::AnthropicProvider;
use crate::provider::openai::OpenAIProvider;
use crate::provider::ollama::OllamaProvider;
use crate::provider::capabilities::CapabilityRegistry;

/// Parse a model switch command and create the appropriate provider.
pub fn parse_model_command(
    input: &str,
    config: &ProviderConfig,
) -> Result<Arc<dyn Provider>, String> {
    let model_spec = input.trim();

    // Format: "provider:model" or just "model" (provider is inferred)
    let (provider_name, model_id) = if model_spec.contains(':') {
        let parts: Vec<&str> = model_spec.splitn(2, ':').collect();
        (parts[0], parts[1])
    } else {
        // Infer provider from model name
        let provider = infer_provider(model_spec);
        (provider, model_spec)
    };

    match provider_name {
        "anthropic" => {
            let api_key = config.anthropic_api_key.as_ref()
                .ok_or("Anthropic API key not configured")?;
            Ok(Arc::new(AnthropicProvider::new(
                api_key.clone(),
                model_id.to_string(),
            )))
        }
        "openai" => {
            let api_key = config.openai_api_key.as_ref()
                .ok_or("OpenAI API key not configured")?;
            Ok(Arc::new(OpenAIProvider::new(
                api_key.clone(),
                model_id.to_string(),
            )))
        }
        "ollama" => {
            Ok(Arc::new(OllamaProvider::new(model_id.to_string())))
        }
        other => Err(format!("Unknown provider: {}", other)),
    }
}

/// Guess which provider a model belongs to based on naming conventions.
fn infer_provider(model: &str) -> &str {
    if model.starts_with("claude") {
        "anthropic"
    } else if model.starts_with("gpt") || model.starts_with("o1") || model.starts_with("o3") {
        "openai"
    } else if model.starts_with("llama")
        || model.starts_with("qwen")
        || model.starts_with("codellama")
        || model.starts_with("deepseek")
    {
        "ollama"
    } else {
        "openai" // Default guess -- most compatible format
    }
}
```

The `infer_provider` function is a convenience that lets users type `/model gpt-4o` instead of `/model openai:gpt-4o`. It uses model naming conventions to guess the provider, falling back to OpenAI since its format is the most widely adopted.

## Automatic Model Selection

Beyond manual switching, the agent can automatically select models based on the task at hand. A simple heuristic approach uses conversation state to estimate task complexity:

```rust
use crate::provider::capabilities::{CapabilityRegistry, ModelCapabilities};

/// Suggest a model based on the current task characteristics.
pub fn suggest_model(
    registry: &CapabilityRegistry,
    message_count: usize,
    total_tokens_used: u32,
    last_tool_calls: &[String],
    budget_remaining: Option<f64>,
) -> Option<&ModelCapabilities> {
    // If budget is nearly exhausted, pick the cheapest capable model
    if let Some(budget) = budget_remaining {
        if budget < 0.01 {
            return registry.cheapest_model_matching(8_000, true, false);
        }
    }

    // Long conversations likely need large context windows
    let min_context = if total_tokens_used > 50_000 {
        128_000
    } else if total_tokens_used > 10_000 {
        32_000
    } else {
        8_000
    };

    // Complex tool usage patterns suggest higher capability needs
    let needs_premium = last_tool_calls.len() > 3
        || last_tool_calls.iter().any(|t| t == "write_file" || t == "shell");

    if needs_premium {
        registry.cheapest_model_matching(min_context, true, false)
            .filter(|m| m.quality_tier >= 2)
            .or_else(|| registry.best_model())
    } else {
        registry.cheapest_model_matching(min_context, true, false)
    }
}
```

This is a starting heuristic -- not a complete solution. The idea is to give the agent a sensible default that users can override. In practice, the logic would also consider factors like the specific task description, whether previous attempts failed, and user preferences stored in configuration.

## Integrating the Switcher into the Agent

Wire the model switcher into the main agent loop so the `/model` command and automatic switching both work:

```rust
use std::sync::Arc;
use crate::provider::capabilities::CapabilityRegistry;

pub struct Agent {
    switcher: ModelSwitcher,
    capabilities: Arc<CapabilityRegistry>,
    system_prompt: String,
    tools: Vec<ToolDefinition>,
    messages: Vec<Message>,
}

impl Agent {
    pub async fn handle_input(&mut self, input: &str) -> Result<String, ProviderError> {
        // Check for model switch command
        if let Some(model_spec) = input.strip_prefix("/model ") {
            return self.handle_model_switch(model_spec).await;
        }

        // Normal message flow
        let provider = self.switcher.current().await;
        let response = provider.send_message(
            &self.system_prompt,
            &self.messages,
            &self.tools,
            4096,
        ).await?;

        // ... process response ...

        Ok("response text".to_string())
    }

    async fn handle_model_switch(&mut self, model_spec: &str) -> Result<String, ProviderError> {
        let config = ProviderConfig::from_env();
        let new_provider = parse_model_command(model_spec, &config)
            .map_err(|e| ProviderError::Api {
                status: 0,
                message: e,
                retryable: false,
            })?;

        let new_model = new_provider.model().to_string();
        let new_name = new_provider.name().to_string();

        // Translate history if the new model has different capabilities
        if let Some(caps) = self.capabilities.get(&new_model) {
            self.messages = translate_history(&self.messages, caps);
        }

        let old = self.switcher.switch_to(
            new_provider,
            SwitchReason::UserRequested,
        ).await;

        Ok(format!(
            "Switched from {}:{} to {}:{}",
            old.name(), old.model(), new_name, new_model
        ))
    }
}
```

When a switch happens, the agent translates the conversation history for the new model and records the switch in the switcher's history. The next `send_message` call goes to the new provider automatically.

## Key Takeaways

- `RwLock<Arc<dyn Provider>>` enables safe runtime model switching: multiple readers for concurrent requests, exclusive access for switching
- Conversation history translation converts tool call blocks to text when switching to models without tool support, preserving coherence across providers
- Provider inference from model names lets users type `/model gpt-4o` without specifying the provider explicitly
- Automatic model selection uses conversation state heuristics -- token count, tool usage patterns, and budget -- to suggest the most appropriate model
- In-flight requests continue on the old provider after a switch, thanks to the `Arc` clone semantics of the read lock
