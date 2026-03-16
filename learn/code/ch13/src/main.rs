// Chapter 13: Multi-Provider — Code snapshot

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Unified message format across providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    role: String,
    content: Value,
}

/// Trait that each LLM provider must implement.
#[allow(async_fn_in_trait)]
trait Provider {
    /// Send a conversation and return the assistant's response.
    async fn chat(&self, messages: &[Message]) -> Result<Message, String>;

    /// Return the provider's name.
    fn name(&self) -> &str;
}

// TODO: Implement AnthropicProvider (Messages API)
// TODO: Implement OpenAIProvider (Chat Completions API)
// TODO: Implement a provider registry / factory
// TODO: Handle tool format translation between providers

struct AnthropicProvider {
    // TODO: api_key, model, base_url
}

struct OpenAIProvider {
    // TODO: api_key, model, base_url
}

#[tokio::main]
async fn main() {
    println!("Chapter 13: Multi-Provider");

    // TODO: Select provider based on config or CLI flag
    // TODO: Translate tool definitions per provider format
    // TODO: Normalize responses into a common format

    println!("TODO: Support multiple LLM providers");
}
