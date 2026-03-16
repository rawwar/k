---
title: Runtime Switching
description: Implement the ability to switch between LLM providers and models at runtime without restarting the agent or losing conversation state.
---

# Runtime Switching

> **What you'll learn:**
> - How to architect the provider layer so that switching providers mid-session requires only swapping the active trait object, not rewiring the agent
> - Techniques for translating conversation history between provider formats when switching models during an active session
> - How to implement user-facing commands (like `/model`) that trigger provider switches and handle the transition gracefully

Users want to switch models mid-session. They might start with a fast, cheap model for simple questions, then switch to a more capable one when the task gets harder. Or they might switch to a local model when working on sensitive code. Your provider abstraction must support this without losing the conversation in progress.

## The Architecture for Hot-Swapping

Since your agent stores the provider as `Box<dyn Provider>`, switching is conceptually simple — replace the box. But the agent struct needs to be designed for this. Let's look at the components:

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Agent {
    /// The active provider, wrapped for concurrent access and replacement.
    provider: Arc<RwLock<Box<dyn Provider>>>,
    /// The model registry for looking up model info.
    registry: Arc<ModelRegistry>,
    /// Conversation history in canonical format.
    conversation: Vec<Message>,
    /// Tool definitions available to the agent.
    tools: ToolRegistry,
}
```

The `Arc<RwLock<Box<dyn Provider>>>` might look heavy, but each layer serves a purpose:

- `Box<dyn Provider>`: dynamic dispatch, enabling different concrete types.
- `RwLock`: allows safe replacement (write lock) while other code reads (read lock).
- `Arc`: shared ownership so multiple parts of the agent can hold a reference.

Most of the time, the agent acquires a read lock to send messages. Only during a model switch does it acquire a write lock.

```rust
impl Agent {
    pub fn new(provider: Box<dyn Provider>, registry: Arc<ModelRegistry>) -> Self {
        Self {
            provider: Arc::new(RwLock::new(provider)),
            registry,
            conversation: Vec::new(),
            tools: ToolRegistry::new(),
        }
    }

    pub async fn send_message(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let provider = self.provider.read().await;
        provider.send_message(request).await
    }
}
```

::: python Coming from Python
In Python, swapping a provider is trivial: `self.provider = new_provider`. No locks, no Arc, no type system considerations. Rust's ownership model requires you to be explicit about concurrent access patterns. The `RwLock` guarantees that no message is in flight when you swap the provider — a race condition Python code would silently allow.
:::

## The /model Command

Let's implement a `/model` command that users can type to switch providers:

```rust
impl Agent {
    /// Switch to a different model. Returns a status message for the user.
    pub async fn switch_model(&mut self, model_identifier: &str) -> Result<String, ProviderError> {
        // Look up the model in the registry
        let model_info = self.registry.get(model_identifier)
            .ok_or_else(|| ProviderError::Other(format!(
                "Unknown model: '{}'. Available models: {}",
                model_identifier,
                self.list_available_models()
            )))?;

        // Create the new provider
        let new_provider = create_provider_for_model(model_info)?;

        // Validate the conversation can transfer
        self.validate_conversation_transfer(model_info)?;

        // Swap the provider
        let mut provider_guard = self.provider.write().await;
        let old_name = provider_guard.name().to_string();
        let old_model = provider_guard.model().to_string();
        *provider_guard = new_provider;

        Ok(format!(
            "Switched from {} ({}) to {} ({})",
            old_name, old_model,
            model_info.provider_name(), model_info.model_id
        ))
    }

    fn list_available_models(&self) -> String {
        self.registry.all_models()
            .map(|info| format!("{} ({})", info.display_name, info.model_id))
            .collect::<Vec<_>>()
            .join(", ")
    }
}
```

The factory function creates the appropriate provider from the registry entry:

```rust
fn create_provider_for_model(info: &ModelInfo) -> Result<Box<dyn Provider>, ProviderError> {
    match info.provider {
        ProviderKind::Anthropic => {
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| ProviderError::Auth(
                    "ANTHROPIC_API_KEY not set. Cannot switch to Anthropic model.".into()
                ))?;
            Ok(Box::new(AnthropicProvider::new(api_key, info.model_id.clone())))
        }
        ProviderKind::OpenAi => {
            let api_key = std::env::var("OPENAI_API_KEY")
                .map_err(|_| ProviderError::Auth(
                    "OPENAI_API_KEY not set. Cannot switch to OpenAI model.".into()
                ))?;
            Ok(Box::new(OpenAiProvider::new(api_key, info.model_id.clone())))
        }
        ProviderKind::Ollama => {
            let base_url = std::env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".into());
            Ok(Box::new(OllamaProvider::new(base_url, info.model_id.clone())))
        }
    }
}
```

## Conversation History Transfer

The conversation history is stored in your canonical `Message` format, which is provider-independent. This is a direct benefit of the adapter pattern — since the agentic loop never stores provider-specific types, the history transfers automatically when you swap providers.

However, there are edge cases to handle:

```rust
impl Agent {
    /// Check if the current conversation is compatible with the target model.
    fn validate_conversation_transfer(
        &self,
        target: &ModelInfo,
    ) -> Result<(), ProviderError> {
        // Check 1: Does the conversation use features the target doesn't support?
        let uses_tools = self.conversation.iter().any(|msg| {
            msg.content.iter().any(|block| {
                matches!(block, ContentBlock::ToolUse { .. } | ContentBlock::ToolResult { .. })
            })
        });

        if uses_tools && !target.capabilities.supports_tools {
            return Err(ProviderError::Other(format!(
                "Current conversation contains tool calls, but {} does not support tools. \
                 The conversation may not work correctly with this model.",
                target.display_name
            )));
        }

        // Check 2: Is the conversation too long for the target's context window?
        let estimated_tokens = self.estimate_token_count(&self.conversation);
        if estimated_tokens > target.capabilities.max_context_tokens {
            return Err(ProviderError::Other(format!(
                "Current conversation (~{} tokens) exceeds {}'s context window ({} tokens). \
                 Consider compacting the conversation first.",
                estimated_tokens,
                target.display_name,
                target.capabilities.max_context_tokens
            )));
        }

        Ok(())
    }

    fn estimate_token_count(&self, messages: &[Message]) -> u32 {
        // Rough estimate: ~4 characters per token for English text
        let total_chars: usize = messages.iter()
            .flat_map(|msg| msg.content.iter())
            .map(|block| match block {
                ContentBlock::Text { text } => text.len(),
                ContentBlock::ToolUse { input, .. } => {
                    serde_json::to_string(input).map(|s| s.len()).unwrap_or(100)
                }
                ContentBlock::ToolResult { content, .. } => content.len(),
            })
            .sum();

        (total_chars / 4) as u32
    }
}
```

The validation catches two common problems: switching to a model that does not support features already used in the conversation, and switching to a model with a smaller context window than the conversation requires.

## Handling In-Flight Requests

What happens if a model switch occurs while a streaming response is in progress? The `RwLock` prevents this by design — the streaming code holds a read lock for the duration of the stream, and the switch requires a write lock. But you should handle the user experience:

```rust
impl Agent {
    pub async fn switch_model(&mut self, model_identifier: &str) -> Result<String, ProviderError> {
        // Try to acquire the write lock with a timeout
        let provider_guard = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.provider.write(),
        ).await;

        match provider_guard {
            Ok(mut guard) => {
                // Proceed with the switch...
                let model_info = self.registry.get(model_identifier)
                    .ok_or_else(|| ProviderError::Other("Unknown model".into()))?;
                let new_provider = create_provider_for_model(model_info)?;
                *guard = new_provider;
                Ok(format!("Switched to {}", model_info.display_name))
            }
            Err(_) => {
                Err(ProviderError::Other(
                    "Cannot switch models while a response is streaming. \
                     Wait for the current response to complete.".into()
                ))
            }
        }
    }
}
```

The timeout-based approach gives the user immediate feedback instead of silently blocking. If a response is streaming, the switch fails with a clear message.

## Command Parsing

Integrate the `/model` command into your REPL's command parser:

```rust
pub enum UserCommand {
    Chat(String),
    SwitchModel(String),
    ListModels,
    CurrentModel,
    Quit,
}

pub fn parse_user_input(input: &str) -> UserCommand {
    let trimmed = input.trim();

    if let Some(model) = trimmed.strip_prefix("/model ") {
        UserCommand::SwitchModel(model.trim().to_string())
    } else if trimmed == "/models" {
        UserCommand::ListModels
    } else if trimmed == "/current" {
        UserCommand::CurrentModel
    } else if trimmed == "/quit" || trimmed == "/exit" {
        UserCommand::Quit
    } else {
        UserCommand::Chat(trimmed.to_string())
    }
}
```

And in the main loop:

```rust
loop {
    let input = read_user_input()?;
    match parse_user_input(&input) {
        UserCommand::SwitchModel(model) => {
            match agent.switch_model(&model).await {
                Ok(msg) => println!("{msg}"),
                Err(e) => eprintln!("Error: {e}"),
            }
        }
        UserCommand::ListModels => {
            let provider = agent.provider.read().await;
            let current = provider.model();
            for info in agent.registry.all_models() {
                let marker = if info.model_id == current { " (active)" } else { "" };
                println!("  {} - {}{}", info.model_id, info.display_name, marker);
            }
        }
        UserCommand::CurrentModel => {
            let provider = agent.provider.read().await;
            println!("Current: {} ({})", provider.model(), provider.name());
        }
        UserCommand::Chat(text) => {
            // Normal chat flow...
            let response = agent.chat(&text).await?;
            println!("{response}");
        }
        UserCommand::Quit => break,
    }
}
```

::: wild In the Wild
Claude Code supports switching between models during a session. The conversation history carries over because it is stored in the agent's internal format, not in any provider-specific structure. OpenCode also supports runtime model switching, implementing it as a configuration change that takes effect on the next message rather than requiring the user to restart the session.
:::

## Key Takeaways

- Store the provider behind `Arc<RwLock<Box<dyn Provider>>>` to enable safe replacement while other code reads from it. The read lock is held during message sending; the write lock is held only during a swap.
- Conversation history transfers automatically when you switch providers because it is stored in canonical `Message` types, not provider-specific formats.
- Validate before switching: check that the target model supports features used in the current conversation and that the context window is large enough for the existing history.
- Use a timeout when acquiring the write lock during a model switch, so users get immediate feedback if a streaming response is in progress rather than waiting indefinitely.
- The `/model`, `/models`, and `/current` commands give users ergonomic control over which model backs their agent session.
