// Chapter 3: The Agentic Loop — Code snapshot
//
// Builds on ch02 (REPL + API calls) by implementing the core agentic loop:
//   send message -> check for tool_use -> execute tool -> send result -> repeat
//
// The loop continues until the model produces a final text response (end_turn)
// or a stop condition is met (max turns, max tokens, error).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead, Write};

// ---------------------------------------------------------------------------
// Message types: the Rust types that model the Anthropic Messages API format
// ---------------------------------------------------------------------------

/// The role of a message sender.
/// The Anthropic API uses "user" and "assistant" — the system prompt is sent
/// separately, not as a message role.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Role {
    User,
    Assistant,
}

/// A single content block within a message.
///
/// An assistant message can contain multiple blocks of different types: some
/// text, then a tool-use request, then more text, etc. We use serde's
/// internally-tagged representation so the JSON `"type"` field maps to the
/// correct variant automatically.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    /// Plain text from the user or the assistant.
    #[serde(rename = "text")]
    Text { text: String },

    /// A tool-use request from the assistant: "I want to call this tool."
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },

    /// A tool result sent back to the model after we execute the tool.
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

/// A single message in the conversation history.
/// Content is a Vec<ContentBlock> because one message can carry text *and*
/// tool-use requests at the same time.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    role: Role,
    content: Vec<ContentBlock>,
}

// Convenience constructors — keep the loop code clean.
impl Message {
    /// Create a user message containing a single text block.
    fn user(text: impl Into<String>) -> Self {
        Message {
            role: Role::User,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    /// Create an assistant message from the content blocks the API returned.
    fn assistant(content: Vec<ContentBlock>) -> Self {
        Message {
            role: Role::Assistant,
            content,
        }
    }

    /// Create a user message containing tool-result blocks.
    /// Tool results are sent as "user" messages — not a separate role.
    fn tool_results(results: Vec<ContentBlock>) -> Self {
        Message {
            role: Role::User,
            content: results,
        }
    }
}

// ---------------------------------------------------------------------------
// API request / response types
// ---------------------------------------------------------------------------

/// Tool definition sent to the API so the model knows what tools it can call.
#[derive(Debug, Serialize)]
struct ToolDefinition {
    name: String,
    description: String,
    input_schema: Value,
}

/// The request body sent to the Anthropic Messages API.
#[derive(Debug, Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ToolDefinition>,
}

/// The response body from the Anthropic Messages API.
#[derive(Debug, Deserialize)]
struct ApiResponse {
    #[allow(dead_code)]
    id: String,
    content: Vec<ContentBlock>,
    #[allow(dead_code)]
    model: String,
    /// "end_turn" = model is done, "tool_use" = model wants to call a tool,
    /// "max_tokens" = output was truncated.
    stop_reason: Option<String>,
    usage: Usage,
}

/// Token usage statistics returned by the API.
#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

// ---------------------------------------------------------------------------
// Conversation state: the growing message history the model sees each call
// ---------------------------------------------------------------------------

struct ConversationState {
    system_prompt: String,
    messages: Vec<Message>,
    total_input_tokens: u32,
    total_output_tokens: u32,
}

impl ConversationState {
    fn new(system_prompt: impl Into<String>) -> Self {
        ConversationState {
            system_prompt: system_prompt.into(),
            messages: Vec::new(),
            total_input_tokens: 0,
            total_output_tokens: 0,
        }
    }

    fn add_user_message(&mut self, text: &str) {
        self.messages.push(Message::user(text));
    }

    fn add_assistant_message(&mut self, content: Vec<ContentBlock>) {
        self.messages.push(Message::assistant(content));
    }

    fn add_tool_results(&mut self, results: Vec<ContentBlock>) {
        self.messages.push(Message::tool_results(results));
    }

    fn record_usage(&mut self, usage: &Usage) {
        self.total_input_tokens += usage.input_tokens;
        self.total_output_tokens += usage.output_tokens;
    }

    /// Rough token estimate: ~4 chars per token.
    fn estimate_token_count(&self) -> usize {
        let mut chars = self.system_prompt.len();
        for msg in &self.messages {
            for block in &msg.content {
                chars += match block {
                    ContentBlock::Text { text } => text.len(),
                    ContentBlock::ToolUse { name, input, .. } => {
                        name.len() + input.to_string().len()
                    }
                    ContentBlock::ToolResult { content, .. } => content.len(),
                };
            }
        }
        chars / 4
    }

    /// True when we are close to the context window ceiling.
    fn is_approaching_limit(&self, max_context_tokens: usize) -> bool {
        self.estimate_token_count() > (max_context_tokens * 80) / 100
    }
}

// ---------------------------------------------------------------------------
// Loop control types
// ---------------------------------------------------------------------------

/// What the loop should do after inspecting the API response.
enum LoopAction {
    /// Model is done — return its text to the user.
    ReturnToUser,
    /// Model wants to call one or more tools.
    ExecuteTools,
    /// Model was cut off by the output token limit.
    MaxTokensReached,
    /// An unknown stop_reason was received.
    UnexpectedReason(String),
}

/// The outcome of a single run of the agentic loop.
enum LoopResult {
    /// Normal completion (stop_reason == "end_turn").
    Complete(String),
    /// Output was truncated (stop_reason == "max_tokens").
    MaxTokens(String),
    /// Hit the configured inner-turn limit.
    TurnLimitReached(String),
    /// Conversation state grew too large for the context window.
    ContextOverflow,
}

impl LoopResult {
    fn text(&self) -> &str {
        match self {
            LoopResult::Complete(t) => t,
            LoopResult::MaxTokens(t) => t,
            LoopResult::TurnLimitReached(t) => t,
            LoopResult::ContextOverflow => {
                "The conversation has grown too long. Please start a new session."
            }
        }
    }

    fn is_complete(&self) -> bool {
        matches!(self, LoopResult::Complete(_))
    }
}

/// Errors that can terminate the agentic loop.
#[derive(Debug)]
#[allow(dead_code)]
enum AgentError {
    ApiError(String),
    EmptyResponse { turn: usize },
    UnexpectedStopReason(String),
    MaxTurnsReached { limit: usize },
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentError::ApiError(msg) => write!(f, "API error: {}", msg),
            AgentError::EmptyResponse { turn } => {
                write!(f, "Empty response from model on turn {}", turn)
            }
            AgentError::UnexpectedStopReason(reason) => {
                write!(f, "Unexpected stop reason: {}", reason)
            }
            AgentError::MaxTurnsReached { limit } => {
                write!(f, "Maximum turns reached: {}", limit)
            }
        }
    }
}

impl std::error::Error for AgentError {}

// ---------------------------------------------------------------------------
// The Agent: ties together the HTTP client, config, and the agentic loop
// ---------------------------------------------------------------------------

struct Agent {
    http_client: reqwest::Client,
    api_key: String,
    model: String,
    max_tokens: u32,
    max_inner_turns: usize,
}

impl Agent {
    fn new(api_key: String) -> Self {
        Agent {
            http_client: reqwest::Client::new(),
            api_key,
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 4096,
            max_inner_turns: 25,
        }
    }

    // --- Tool definitions sent to the API ------------------------------------

    /// Build the list of tool definitions the model is allowed to call.
    /// For ch03 we expose a simple "echo" tool to demonstrate the loop.
    fn tool_definitions(&self) -> Vec<ToolDefinition> {
        vec![ToolDefinition {
            name: "echo".to_string(),
            description: "Echoes back the provided message. \
                          Useful for testing the agentic loop."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "The text to echo back"
                    }
                },
                "required": ["message"]
            }),
        }]
    }

    // --- The agentic loop ----------------------------------------------------

    /// Run the agentic loop for a single user message.
    ///
    /// 1. Send the conversation to the LLM
    /// 2. Inspect the response's stop_reason
    /// 3. If tool_use  -> execute tools, feed results back, continue
    /// 4. If end_turn  -> extract the text and return
    /// 5. Repeat until done or a stop condition fires
    async fn run(
        &self,
        state: &mut ConversationState,
        user_message: &str,
    ) -> Result<LoopResult, AgentError> {
        state.add_user_message(user_message);

        let mut inner_turns: usize = 0;

        loop {
            // --- Stop condition: turn limit ---
            if self.max_inner_turns > 0 && inner_turns >= self.max_inner_turns {
                let partial = self.last_assistant_text(state);
                return Ok(LoopResult::TurnLimitReached(partial));
            }

            // --- Stop condition: context window ---
            if state.is_approaching_limit(180_000) {
                return Ok(LoopResult::ContextOverflow);
            }

            // --- Phase 1: Call the LLM ---
            let response = self.call_api(state).await?;
            inner_turns += 1;

            // Record token usage
            state.record_usage(&response.usage);

            // --- Phase 2: Append assistant response to history ---
            state.add_assistant_message(response.content.clone());

            // --- Stop condition: empty response ---
            if is_empty_response(&response.content) {
                return Err(AgentError::EmptyResponse { turn: inner_turns });
            }

            // --- Phase 3: Decide what to do next based on stop_reason ---
            match decide_action(response.stop_reason.as_deref(), &response.content) {
                LoopAction::ReturnToUser => {
                    let text = extract_text(&response.content);
                    return Ok(LoopResult::Complete(text));
                }

                LoopAction::ExecuteTools => {
                    // Execute every tool call and collect results
                    let tool_results = self.handle_tool_calls(&response.content);
                    // Feed observations back into the conversation
                    state.add_tool_results(tool_results);
                    // Loop continues — model will see the results next iteration
                    println!("  [loop] executed tools, continuing (turn {})", inner_turns);
                }

                LoopAction::MaxTokensReached => {
                    let text = extract_text(&response.content);
                    return Ok(LoopResult::MaxTokens(text));
                }

                LoopAction::UnexpectedReason(reason) => {
                    return Err(AgentError::UnexpectedStopReason(reason));
                }
            }
        }
    }

    // --- API call ------------------------------------------------------------

    /// Send the current conversation state to the Anthropic Messages API.
    async fn call_api(&self, state: &ConversationState) -> Result<ApiResponse, AgentError> {
        let request_body = ApiRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            system: state.system_prompt.clone(),
            messages: state.messages.clone(),
            tools: self.tool_definitions(),
        };

        let response = self
            .http_client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AgentError::ApiError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "failed to read error body".to_string());
            return Err(AgentError::ApiError(format!("HTTP {}: {}", status, body)));
        }

        response
            .json::<ApiResponse>()
            .await
            .map_err(|e| AgentError::ApiError(e.to_string()))
    }

    // --- Tool execution (placeholder for ch04) --------------------------------

    /// Handle all tool-use blocks in the assistant's response.
    /// Returns tool-result content blocks to feed back as a user message.
    ///
    /// NOTE: In ch03 we only implement "echo" as a concrete tool.
    /// Chapter 4 replaces this with a real tool registry and dispatch system.
    fn handle_tool_calls(&self, content: &[ContentBlock]) -> Vec<ContentBlock> {
        let mut results = Vec::new();

        for block in content {
            if let ContentBlock::ToolUse { id, name, input } = block {
                let (output, is_error) = match name.as_str() {
                    // The "echo" tool: returns the message the model sent.
                    "echo" => {
                        let msg = input["message"]
                            .as_str()
                            .unwrap_or("<missing message>");
                        (msg.to_string(), false)
                    }
                    // Any unknown tool: return an error so the model knows.
                    unknown => (
                        format!("Unknown tool: '{}'. Available tools: echo", unknown),
                        true,
                    ),
                };

                println!("  [tool] {} -> {}", name, &output);

                results.push(ContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    content: output,
                    is_error: if is_error { Some(true) } else { None },
                });
            }
        }

        results
    }

    // --- Helpers --------------------------------------------------------------

    /// Get the text from the last assistant message in the conversation.
    fn last_assistant_text(&self, state: &ConversationState) -> String {
        state
            .messages
            .iter()
            .rev()
            .find(|m| m.role == Role::Assistant)
            .map(|m| extract_text(&m.content))
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Free functions used by the loop
// ---------------------------------------------------------------------------

/// Determine what the loop should do based on the API's stop_reason.
fn decide_action(stop_reason: Option<&str>, content: &[ContentBlock]) -> LoopAction {
    match stop_reason {
        Some("end_turn") => LoopAction::ReturnToUser,
        Some("tool_use") => LoopAction::ExecuteTools,
        Some("max_tokens") => LoopAction::MaxTokensReached,
        Some("stop_sequence") => LoopAction::ReturnToUser,
        Some(other) => LoopAction::UnexpectedReason(other.to_string()),
        None => {
            // Fallback: inspect content blocks for tool_use
            if content
                .iter()
                .any(|b| matches!(b, ContentBlock::ToolUse { .. }))
            {
                LoopAction::ExecuteTools
            } else {
                LoopAction::ReturnToUser
            }
        }
    }
}

/// Extract all text from response content blocks, concatenated.
fn extract_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// True if the response has no meaningful content.
fn is_empty_response(content: &[ContentBlock]) -> bool {
    if content.is_empty() {
        return true;
    }
    content.iter().all(|block| match block {
        ContentBlock::Text { text } => text.trim().is_empty(),
        _ => false,
    })
}

// ---------------------------------------------------------------------------
// REPL — the outer loop that reads user input (built in ch01/ch02)
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    println!("Chapter 3: The Agentic Loop");
    println!("Type a message to chat. The model can call the 'echo' tool.");
    println!("Type 'quit' or Ctrl-D to exit.\n");

    // Read the API key from the environment
    let api_key = match std::env::var("ANTHROPIC_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            eprintln!("Error: ANTHROPIC_API_KEY environment variable not set.");
            std::process::exit(1);
        }
    };

    let agent = Agent::new(api_key);
    let mut state = ConversationState::new(
        "You are a helpful assistant. You have access to an 'echo' tool that \
         echoes back whatever message you give it. Use it when the user asks \
         you to echo or repeat something. For normal questions, just respond \
         with text.",
    );

    let stdin = io::stdin();
    print!("> ");
    io::stdout().flush().unwrap();

    for line in stdin.lock().lines() {
        let input = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        let trimmed = input.trim();
        if trimmed.is_empty() {
            print!("> ");
            io::stdout().flush().unwrap();
            continue;
        }
        if trimmed == "quit" || trimmed == "exit" {
            break;
        }

        // Run the agentic loop for this user message
        match agent.run(&mut state, trimmed).await {
            Ok(result) => {
                println!("\n{}", result.text());
                if !result.is_complete() {
                    println!("(Note: response may be incomplete)");
                }
            }
            Err(e) => {
                eprintln!("\nError: {}", e);
            }
        }

        // Show cumulative token usage
        println!(
            "  [tokens] input: {} | output: {}\n",
            state.total_input_tokens, state.total_output_tokens
        );

        print!("> ");
        io::stdout().flush().unwrap();
    }

    println!("\nGoodbye!");
}
