// Chapter 13: Multi-Provider Support — Code snapshot
//
// Demonstrates the provider abstraction layer that lets the agent work with
// Anthropic's Claude and OpenAI's GPT models through a unified interface.
// Builds on ch12's permission system by making the underlying LLM pluggable.
//
// Key concepts:
//   - Provider trait with send_message() and name()
//   - AnthropicProvider  (Anthropic Messages API)
//   - OpenAIProvider     (OpenAI Chat Completions API)
//   - ProviderConfig     (CLI flag + env var based selection)
//   - Response normalization (both providers -> same ProviderResponse)
//   - Provider selection via --provider anthropic|openai

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;

// ---------------------------------------------------------------------------
// Provider-neutral types — the common vocabulary the agent core speaks
// ---------------------------------------------------------------------------

/// A role in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Role {
    User,
    Assistant,
    System,
}

/// A single content block within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
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

/// A message in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    role: Role,
    content: Vec<ContentBlock>,
}

/// A tool definition the model can call.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolDefinition {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

/// Token usage for a single request.
#[derive(Debug, Clone, Default)]
struct TokenUsage {
    input_tokens: u32,
    output_tokens: u32,
}

/// Why the model stopped generating.
#[derive(Debug, Clone, PartialEq)]
enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    Unknown(String),
}

/// The normalized response from any provider.
#[derive(Debug, Clone)]
struct ProviderResponse {
    content: Vec<ContentBlock>,
    usage: TokenUsage,
    model: String,
    stop_reason: StopReason,
}

/// Errors that can occur during provider operations.
#[derive(Debug)]
enum ProviderError {
    Http(reqwest::Error),
    Api {
        status: u16,
        message: String,
        retryable: bool,
    },
    RateLimited {
        retry_after_ms: Option<u64>,
    },
    Auth(String),
    Serialization(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::Http(e) => write!(f, "HTTP error: {e}"),
            ProviderError::Api { status, message, .. } => {
                write!(f, "API error (status {status}): {message}")
            }
            ProviderError::RateLimited { retry_after_ms } => {
                write!(f, "Rate limited, retry after {retry_after_ms:?}ms")
            }
            ProviderError::Auth(msg) => write!(f, "Auth error: {msg}"),
            ProviderError::Serialization(msg) => write!(f, "Serialization error: {msg}"),
        }
    }
}

impl From<reqwest::Error> for ProviderError {
    fn from(e: reqwest::Error) -> Self {
        ProviderError::Http(e)
    }
}

impl ProviderError {
    /// Returns true if the error is transient and the request could succeed on retry.
    fn is_retryable(&self) -> bool {
        matches!(
            self,
            ProviderError::Http(_)
                | ProviderError::RateLimited { .. }
                | ProviderError::Api {
                    retryable: true,
                    ..
                }
        )
    }

    /// Returns true if the error suggests trying a different provider.
    fn should_fallback(&self) -> bool {
        matches!(
            self,
            ProviderError::RateLimited { .. }
                | ProviderError::Auth(_)
                | ProviderError::Api {
                    status: 500..=599,
                    ..
                }
        )
    }
}

// ---------------------------------------------------------------------------
// Provider trait — the core abstraction every LLM adapter implements
// ---------------------------------------------------------------------------

/// The trait that all LLM provider adapters implement.
///
/// Uses `async_fn_in_trait` (stabilized in Rust 1.75) instead of the
/// `async_trait` crate to keep dependencies minimal.
///
/// Note: native async-fn-in-trait is not dyn-compatible; we use an enum
/// (`AnyProvider`) for runtime dispatch instead of `dyn Provider`. For a
/// small, known set of providers this is idiomatic and avoids boxing.
#[allow(async_fn_in_trait)]
trait Provider {
    /// A human-readable name for this provider (e.g. "anthropic", "openai").
    fn name(&self) -> &str;

    /// The model identifier currently in use.
    fn model(&self) -> &str;

    /// Send a non-streaming request and wait for the complete response.
    async fn send_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
        max_tokens: u32,
    ) -> Result<ProviderResponse, ProviderError>;
}

// ---------------------------------------------------------------------------
// Anthropic adapter — Messages API
// ---------------------------------------------------------------------------

/// Anthropic-specific request body.
#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    /// The system prompt is a top-level field in Anthropic's API, *not* a
    /// message with role "system".
    system: String,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<AnthropicTool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: Vec<AnthropicContent>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum AnthropicContent {
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
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

#[derive(Debug, Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    /// Anthropic calls the parameter schema `input_schema` (not `parameters`).
    input_schema: serde_json::Value,
}

/// Anthropic Messages API response.
#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    model: String,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

// -- Translation helpers: generic types <-> Anthropic wire types --

fn to_anthropic_messages(messages: &[Message]) -> Vec<AnthropicMessage> {
    messages
        .iter()
        .map(|msg| {
            let role = match msg.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                // Anthropic does not allow role:"system" in the messages array;
                // the system prompt goes in the top-level `system` field.
                Role::System => "user",
            };
            let content = msg
                .content
                .iter()
                .map(|block| match block {
                    ContentBlock::Text { text } => AnthropicContent::Text { text: text.clone() },
                    ContentBlock::ToolUse { id, name, input } => AnthropicContent::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    },
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => AnthropicContent::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        content: content.clone(),
                        is_error: if *is_error { Some(true) } else { None },
                    },
                })
                .collect();
            AnthropicMessage {
                role: role.to_string(),
                content,
            }
        })
        .collect()
}

fn to_anthropic_tools(tools: &[ToolDefinition]) -> Vec<AnthropicTool> {
    tools
        .iter()
        .map(|t| AnthropicTool {
            name: t.name.clone(),
            description: t.description.clone(),
            input_schema: t.parameters.clone(),
        })
        .collect()
}

/// Normalize an Anthropic response into the provider-neutral format.
fn parse_anthropic_response(resp: AnthropicResponse) -> ProviderResponse {
    let content = resp
        .content
        .into_iter()
        .map(|block| match block {
            AnthropicContent::Text { text } => ContentBlock::Text { text },
            AnthropicContent::ToolUse { id, name, input } => {
                ContentBlock::ToolUse { id, name, input }
            }
            AnthropicContent::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error: is_error.unwrap_or(false),
            },
        })
        .collect();

    let stop_reason = match resp.stop_reason.as_deref() {
        Some("end_turn") => StopReason::EndTurn,
        Some("tool_use") => StopReason::ToolUse,
        Some("max_tokens") => StopReason::MaxTokens,
        Some(other) => StopReason::Unknown(other.to_string()),
        None => StopReason::Unknown("none".to_string()),
    };

    ProviderResponse {
        content,
        usage: TokenUsage {
            input_tokens: resp.usage.input_tokens,
            output_tokens: resp.usage.output_tokens,
        },
        model: resp.model,
        stop_reason,
    }
}

/// Adapter for Anthropic's Messages API.
struct AnthropicProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl AnthropicProvider {
    fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            base_url: "https://api.anthropic.com".to_string(),
        }
    }

    /// Create with a custom base URL (useful for testing with mock servers).
    #[allow(dead_code)]
    fn with_base_url(api_key: String, model: String, base_url: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            base_url,
        }
    }
}

impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn send_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
        max_tokens: u32,
    ) -> Result<ProviderResponse, ProviderError> {
        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens,
            system: system.to_string(),
            messages: to_anthropic_messages(messages),
            tools: to_anthropic_tools(tools),
        };

        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status().as_u16();

        if status == 429 {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .map(|s| s * 1000);
            return Err(ProviderError::RateLimited {
                retry_after_ms: retry_after,
            });
        }

        if status == 401 {
            return Err(ProviderError::Auth(
                "Invalid Anthropic API key".to_string(),
            ));
        }

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api {
                status,
                message: body,
                retryable: status >= 500,
            });
        }

        let body: AnthropicResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Serialization(e.to_string()))?;

        Ok(parse_anthropic_response(body))
    }
}

// ---------------------------------------------------------------------------
// OpenAI adapter — Chat Completions API
// ---------------------------------------------------------------------------

/// OpenAI-specific request body.
#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OpenAIToolDef>,
    max_tokens: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAIMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OpenAIFunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAIFunctionCall {
    name: String,
    /// OpenAI sends tool-call arguments as a JSON *string*, not a parsed
    /// object. The adapter parses this back into `serde_json::Value` during
    /// response normalization.
    arguments: String,
}

#[derive(Debug, Serialize)]
struct OpenAIToolDef {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunctionDef,
}

#[derive(Debug, Serialize)]
struct OpenAIFunctionDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

/// OpenAI Chat Completions response.
#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
    model: String,
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIChoiceMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoiceMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    #[allow(dead_code)]
    total_tokens: u32,
}

// -- Translation helpers: generic types <-> OpenAI wire types --

/// Translate the provider-neutral messages into OpenAI's format.
///
/// Key differences from Anthropic:
///   - The system prompt is a message with `role: "system"`, not a top-level field.
///   - An assistant message's tool calls live in a separate `tool_calls` field.
///   - Tool results are separate messages with `role: "tool"`.
fn to_openai_messages(system: &str, messages: &[Message]) -> Vec<OpenAIMessage> {
    let mut result = Vec::new();

    // System prompt becomes a "system" role message.
    if !system.is_empty() {
        result.push(OpenAIMessage {
            role: "system".to_string(),
            content: Some(system.to_string()),
            tool_calls: None,
            tool_call_id: None,
        });
    }

    for msg in messages {
        match msg.role {
            Role::User => {
                // Collect text blocks into a single content string.
                let text: String = msg
                    .content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                if !text.is_empty() {
                    result.push(OpenAIMessage {
                        role: "user".to_string(),
                        content: Some(text),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }

                // Tool results become separate "tool" role messages.
                for block in &msg.content {
                    if let ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        ..
                    } = block
                    {
                        result.push(OpenAIMessage {
                            role: "tool".to_string(),
                            content: Some(content.clone()),
                            tool_calls: None,
                            tool_call_id: Some(tool_use_id.clone()),
                        });
                    }
                }
            }
            Role::Assistant => {
                let text: Option<String> = {
                    let texts: Vec<&str> = msg
                        .content
                        .iter()
                        .filter_map(|b| match b {
                            ContentBlock::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect();
                    if texts.is_empty() {
                        None
                    } else {
                        Some(texts.join("\n"))
                    }
                };

                // Tool-use blocks become OpenAI `tool_calls` entries.
                let tool_calls: Vec<OpenAIToolCall> = msg
                    .content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::ToolUse { id, name, input } => Some(OpenAIToolCall {
                            id: id.clone(),
                            call_type: "function".to_string(),
                            function: OpenAIFunctionCall {
                                name: name.clone(),
                                arguments: serde_json::to_string(input).unwrap_or_default(),
                            },
                        }),
                        _ => None,
                    })
                    .collect();

                result.push(OpenAIMessage {
                    role: "assistant".to_string(),
                    content: text,
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
                    tool_call_id: None,
                });
            }
            Role::System => {
                let text = msg
                    .content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                result.push(OpenAIMessage {
                    role: "system".to_string(),
                    content: Some(text),
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
        }
    }

    result
}

fn to_openai_tools(tools: &[ToolDefinition]) -> Vec<OpenAIToolDef> {
    tools
        .iter()
        .map(|t| OpenAIToolDef {
            tool_type: "function".to_string(),
            function: OpenAIFunctionDef {
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: t.parameters.clone(),
            },
        })
        .collect()
}

/// Normalize an OpenAI response into the provider-neutral format.
fn parse_openai_response(resp: OpenAIResponse) -> ProviderResponse {
    let choice = &resp.choices[0];
    let mut content = Vec::new();

    // Text content.
    if let Some(text) = &choice.message.content {
        if !text.is_empty() {
            content.push(ContentBlock::Text { text: text.clone() });
        }
    }

    // Tool calls — parse the JSON-string arguments back into Value.
    if let Some(tool_calls) = &choice.message.tool_calls {
        for tc in tool_calls {
            let input: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            content.push(ContentBlock::ToolUse {
                id: tc.id.clone(),
                name: tc.function.name.clone(),
                input,
            });
        }
    }

    // Map OpenAI's finish_reason strings to the common StopReason enum.
    let stop_reason = match choice.finish_reason.as_deref() {
        Some("stop") => StopReason::EndTurn,
        Some("tool_calls") => StopReason::ToolUse,
        Some("length") => StopReason::MaxTokens,
        Some(other) => StopReason::Unknown(other.to_string()),
        None => StopReason::Unknown("none".to_string()),
    };

    let usage = resp
        .usage
        .map(|u| TokenUsage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
        })
        .unwrap_or_default();

    ProviderResponse {
        content,
        usage,
        model: resp.model,
        stop_reason,
    }
}

/// Adapter for OpenAI's Chat Completions API.
struct OpenAIProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAIProvider {
    fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            base_url: "https://api.openai.com".to_string(),
        }
    }

    /// Create with a custom base URL — works with any OpenAI-compatible API
    /// (Together AI, Groq, Anyscale, local vLLM, etc.).
    #[allow(dead_code)]
    fn with_base_url(api_key: String, model: String, base_url: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            base_url,
        }
    }
}

impl Provider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn send_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
        max_tokens: u32,
    ) -> Result<ProviderResponse, ProviderError> {
        let request = OpenAIRequest {
            model: self.model.clone(),
            messages: to_openai_messages(system, messages),
            tools: to_openai_tools(tools),
            max_tokens,
        };

        let response = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status().as_u16();

        if status == 429 {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .map(|s| s * 1000);
            return Err(ProviderError::RateLimited {
                retry_after_ms: retry_after,
            });
        }

        if status == 401 {
            return Err(ProviderError::Auth("Invalid OpenAI API key".to_string()));
        }

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api {
                status,
                message: body,
                retryable: status >= 500,
            });
        }

        let body: OpenAIResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Serialization(e.to_string()))?;

        Ok(parse_openai_response(body))
    }
}

// ---------------------------------------------------------------------------
// AnyProvider — enum dispatch for runtime provider selection
// ---------------------------------------------------------------------------

/// An enum that wraps all known provider implementations.
///
/// Native async-fn-in-trait is *not* dyn-compatible, so we cannot use
/// `Arc<dyn Provider>` for runtime dispatch.  For a small, closed set of
/// providers an enum is the idiomatic Rust alternative — it avoids heap
/// allocation and the `async_trait` macro dependency while still allowing
/// the agent to select a provider at runtime.
enum AnyProvider {
    Anthropic(AnthropicProvider),
    OpenAI(OpenAIProvider),
}

impl Provider for AnyProvider {
    fn name(&self) -> &str {
        match self {
            AnyProvider::Anthropic(p) => p.name(),
            AnyProvider::OpenAI(p) => p.name(),
        }
    }

    fn model(&self) -> &str {
        match self {
            AnyProvider::Anthropic(p) => p.model(),
            AnyProvider::OpenAI(p) => p.model(),
        }
    }

    async fn send_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
        max_tokens: u32,
    ) -> Result<ProviderResponse, ProviderError> {
        match self {
            AnyProvider::Anthropic(p) => {
                p.send_message(system, messages, tools, max_tokens).await
            }
            AnyProvider::OpenAI(p) => {
                p.send_message(system, messages, tools, max_tokens).await
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Provider configuration and factory
// ---------------------------------------------------------------------------

/// Configuration that determines which provider the agent uses.
///
/// Resolved from CLI arguments and environment variables with the following
/// precedence (highest to lowest):
///   1. CLI flag `--provider anthropic|openai`
///   2. Environment variable `AGENT_PROVIDER`
///   3. Default: "anthropic"
///
/// API keys are always read from the environment:
///   - `ANTHROPIC_API_KEY` for the Anthropic provider
///   - `OPENAI_API_KEY`    for the OpenAI provider
struct ProviderConfig {
    provider_name: String,
    model: String,
    api_key: String,
    base_url: Option<String>,
}

impl ProviderConfig {
    /// Resolve configuration from CLI arguments and environment variables.
    fn from_cli_and_env(args: &[String]) -> Result<Self, String> {
        // --provider flag takes highest priority, then AGENT_PROVIDER env var.
        let provider_name = parse_flag(args, "--provider")
            .or_else(|| env::var("AGENT_PROVIDER").ok())
            .unwrap_or_else(|| "anthropic".to_string());

        // --model flag (optional override for the provider's default model).
        let model_override = parse_flag(args, "--model");

        // --base-url flag (optional, for proxies or OpenAI-compatible APIs).
        let base_url = parse_flag(args, "--base-url");

        let (api_key, default_model) = match provider_name.as_str() {
            "anthropic" => {
                let key = env::var("ANTHROPIC_API_KEY")
                    .map_err(|_| "ANTHROPIC_API_KEY not set".to_string())?;
                (key, "claude-sonnet-4-20250514".to_string())
            }
            "openai" => {
                let key = env::var("OPENAI_API_KEY")
                    .map_err(|_| "OPENAI_API_KEY not set".to_string())?;
                (key, "gpt-4o".to_string())
            }
            other => return Err(format!("Unknown provider: {other}")),
        };

        let model = model_override.unwrap_or(default_model);

        Ok(Self {
            provider_name,
            model,
            api_key,
            base_url,
        })
    }

    /// Build a concrete provider from this configuration.
    fn build_provider(self) -> Result<AnyProvider, String> {
        match self.provider_name.as_str() {
            "anthropic" => {
                let provider = match self.base_url {
                    Some(url) => {
                        AnthropicProvider::with_base_url(self.api_key, self.model, url)
                    }
                    None => AnthropicProvider::new(self.api_key, self.model),
                };
                Ok(AnyProvider::Anthropic(provider))
            }
            "openai" => {
                let provider = match self.base_url {
                    Some(url) => {
                        OpenAIProvider::with_base_url(self.api_key, self.model, url)
                    }
                    None => OpenAIProvider::new(self.api_key, self.model),
                };
                Ok(AnyProvider::OpenAI(provider))
            }
            other => Err(format!("Unknown provider: {other}")),
        }
    }
}

/// Extract the value following `flag_name` in the argument list.
fn parse_flag(args: &[String], flag_name: &str) -> Option<String> {
    args.windows(2).find_map(|pair| {
        if pair[0] == flag_name {
            Some(pair[1].clone())
        } else {
            None
        }
    })
}

// ---------------------------------------------------------------------------
// Display helpers
// ---------------------------------------------------------------------------

fn display_response(response: &ProviderResponse) {
    for block in &response.content {
        match block {
            ContentBlock::Text { text } => println!("{text}"),
            ContentBlock::ToolUse { id, name, input } => {
                println!("[tool_use] {name} (id={id})");
                println!("  input: {input}");
            }
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                let status = if *is_error { "ERROR" } else { "OK" };
                println!("[tool_result {status}] (id={tool_use_id})");
                println!("  {content}");
            }
        }
    }
    println!(
        "\n--- {} | tokens: {} in / {} out | stop: {:?} ---",
        response.model, response.usage.input_tokens, response.usage.output_tokens,
        response.stop_reason,
    );
}

// ---------------------------------------------------------------------------
// Main — wire everything together
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    println!("Chapter 13: Multi-Provider Support\n");

    // Collect CLI args.
    let args: Vec<String> = env::args().collect();

    // Build provider from CLI flags + env vars.
    let config = match ProviderConfig::from_cli_and_env(&args) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Configuration error: {e}");
            eprintln!();
            eprintln!("Usage:");
            eprintln!(
                "  cli-agent-ch13 [--provider anthropic|openai] [--model MODEL] [--base-url URL]"
            );
            eprintln!();
            eprintln!("Environment variables:");
            eprintln!("  ANTHROPIC_API_KEY   Required when --provider anthropic (default)");
            eprintln!("  OPENAI_API_KEY      Required when --provider openai");
            eprintln!("  AGENT_PROVIDER      Default provider if --provider is omitted");
            std::process::exit(1);
        }
    };

    let provider = match config.build_provider() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to build provider: {e}");
            std::process::exit(1);
        }
    };

    println!(
        "Using provider: {} (model: {})\n",
        provider.name(),
        provider.model(),
    );

    // A sample tool definition to show both text and tool-calling paths.
    let tools = vec![ToolDefinition {
        name: "read_file".to_string(),
        description: "Read the contents of a file at the given path.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute path to the file"
                }
            },
            "required": ["path"]
        }),
    }];

    let system_prompt = "You are a helpful coding assistant. When asked about files, \
                         use the read_file tool to inspect them.";

    let messages = vec![Message {
        role: Role::User,
        content: vec![ContentBlock::Text {
            text: "What is 2 + 2? Answer in one sentence.".to_string(),
        }],
    }];

    // Send the request through the unified Provider interface.
    // Both Anthropic and OpenAI return the same normalized ProviderResponse.
    match provider
        .send_message(system_prompt, &messages, &tools, 256)
        .await
    {
        Ok(response) => display_response(&response),
        Err(e) => {
            eprintln!("Provider error: {e}");
            if e.is_retryable() {
                eprintln!("(this error is retryable)");
            }
            if e.should_fallback() {
                eprintln!("(consider falling back to another provider)");
            }
            std::process::exit(1);
        }
    }
}
