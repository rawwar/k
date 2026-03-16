// Chapter 2: First LLM Call — Code snapshot
//
// Builds on the Chapter 1 REPL (clap + rustyline) and adds:
// - reqwest HTTP client for calling the Anthropic Messages API
// - serde structs for request/response serialization
// - Async runtime via tokio
// - Basic error handling for API errors
// - Conversation history for multi-turn chat

use std::env;
use std::fmt;

use clap::Parser;
use reqwest::header::{HeaderMap, HeaderValue};
use rustyline::DefaultEditor;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// CLI arguments (carried forward from Chapter 1)
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "cli-agent", about = "A CLI agent powered by Claude")]
struct Cli {
    /// Override the default model
    #[arg(long, default_value = DEFAULT_MODEL)]
    model: String,

    /// Maximum tokens in the response
    #[arg(long, default_value_t = 4096)]
    max_tokens: u32,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";
const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";

const SYSTEM_PROMPT: &str = r#"You are an expert coding assistant embedded in a command-line interface. Your primary goal is to help users write, debug, and understand code.

Guidelines:
- Write clean, idiomatic, well-commented code.
- Provide complete, runnable examples with all necessary imports.
- Use markdown code blocks with the appropriate language identifier.
- Keep explanations concise and practical. Focus on the "why" behind design decisions.
- If you are unsure about something, say so explicitly rather than guessing.
- When asked to fix a bug, explain what was wrong before providing the fix.

You are running as a CLI tool on the user's machine. Be helpful, be accurate, and be concise."#;

// ---------------------------------------------------------------------------
// Anthropic Messages API — Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: String,
}

// ---------------------------------------------------------------------------
// Anthropic Messages API — Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ChatResponse {
    #[allow(dead_code)]
    id: String,
    content: Vec<ContentBlock>,
    #[allow(dead_code)]
    model: String,
    #[allow(dead_code)]
    stop_reason: Option<String>,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    block_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

// ---------------------------------------------------------------------------
// Anthropic Messages API — Error types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    error: ApiErrorDetail,
}

#[derive(Debug, Deserialize)]
struct ApiErrorDetail {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

/// All the ways an API call can fail.
#[derive(Debug)]
enum ApiError {
    /// Network-level failure (DNS, connection, timeout).
    Network(reqwest::Error),
    /// The API returned a structured error response.
    ApiResponse {
        status: u16,
        error_type: String,
        message: String,
    },
    /// The API returned an error we could not parse.
    UnexpectedResponse { status: u16, body: String },
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::Network(e) => write!(f, "Network error: {e}"),
            ApiError::ApiResponse {
                status,
                error_type,
                message,
            } => {
                write!(f, "API error ({status} {error_type}): {message}")
            }
            ApiError::UnexpectedResponse { status, body } => {
                write!(f, "Unexpected error response ({status}): {body}")
            }
        }
    }
}

impl std::error::Error for ApiError {}

impl From<reqwest::Error> for ApiError {
    fn from(err: reqwest::Error) -> Self {
        ApiError::Network(err)
    }
}

// ---------------------------------------------------------------------------
// HTTP client construction
// ---------------------------------------------------------------------------

/// Build a reqwest client with the required Anthropic headers baked in.
fn build_client(api_key: &str) -> anyhow::Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-api-key",
        HeaderValue::from_str(api_key)
            .map_err(|e| anyhow::anyhow!("Invalid API key header value: {e}"))?,
    );
    headers.insert("anthropic-version", HeaderValue::from_static(API_VERSION));

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;

    Ok(client)
}

// ---------------------------------------------------------------------------
// Send a message to the Anthropic Messages API
// ---------------------------------------------------------------------------

async fn send_message(
    client: &reqwest::Client,
    model: &str,
    max_tokens: u32,
    messages: &[Message],
) -> Result<ChatResponse, ApiError> {
    let request = ChatRequest {
        model: model.to_string(),
        max_tokens,
        messages: messages.to_vec(),
        system: Some(SYSTEM_PROMPT.to_string()),
    };

    let response = client.post(API_URL).json(&request).send().await?;

    let status = response.status();

    if status.is_success() {
        let chat_response: ChatResponse = response.json().await?;
        Ok(chat_response)
    } else {
        let status_code = status.as_u16();
        let body = response.text().await?;

        // Try to parse the body as the standard Anthropic error format.
        match serde_json::from_str::<ApiErrorResponse>(&body) {
            Ok(parsed) => Err(ApiError::ApiResponse {
                status: status_code,
                error_type: parsed.error.error_type,
                message: parsed.error.message,
            }),
            Err(_) => Err(ApiError::UnexpectedResponse {
                status: status_code,
                body,
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Extract assistant text from a ChatResponse
// ---------------------------------------------------------------------------

fn extract_text(response: &ChatResponse) -> String {
    response
        .content
        .iter()
        .filter_map(|block| block.text.as_deref())
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// REPL — main entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Read the API key from the environment.
    let api_key = env::var("ANTHROPIC_API_KEY")
        .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY environment variable is not set"))?;

    let client = build_client(&api_key)?;
    let mut conversation: Vec<Message> = Vec::new();

    // Set up the rustyline editor for a nicer REPL experience.
    let mut rl = DefaultEditor::new()?;

    println!("CLI Agent — Chapter 2: First LLM Call");
    println!("Model: {}", cli.model);
    println!("Type a message to chat with Claude. Press Ctrl-D or type \"quit\" to exit.\n");

    loop {
        let readline = rl.readline("> ");

        match readline {
            Ok(line) => {
                let input = line.trim();
                if input.is_empty() {
                    continue;
                }
                if input == "quit" {
                    println!("Goodbye!");
                    break;
                }

                // Record the line in rustyline history.
                let _ = rl.add_history_entry(input);

                // Add the user message to conversation history.
                conversation.push(Message {
                    role: "user".to_string(),
                    content: input.to_string(),
                });

                // Send the full conversation to Claude.
                match send_message(&client, &cli.model, cli.max_tokens, &conversation).await {
                    Ok(response) => {
                        let assistant_text = extract_text(&response);
                        println!("\n{assistant_text}\n");

                        // Append assistant reply so the next turn has context.
                        conversation.push(Message {
                            role: "assistant".to_string(),
                            content: assistant_text,
                        });

                        println!(
                            "[tokens: {} input, {} output]\n",
                            response.usage.input_tokens, response.usage.output_tokens,
                        );
                    }
                    Err(ApiError::Network(e)) => {
                        eprintln!("\nNetwork error: {e}");
                        eprintln!("Check your internet connection and try again.\n");
                        conversation.pop(); // Remove the failed user message.
                    }
                    Err(ApiError::ApiResponse {
                        status,
                        error_type,
                        message,
                    }) => {
                        match status {
                            401 => {
                                eprintln!("\nAuthentication failed: {message}");
                                eprintln!(
                                    "Check your ANTHROPIC_API_KEY environment variable.\n"
                                );
                            }
                            429 => {
                                eprintln!("\nRate limited: {message}");
                                eprintln!("Wait a moment and try again.\n");
                            }
                            529 => {
                                eprintln!("\nAPI is overloaded: {message}");
                                eprintln!(
                                    "The service is temporarily busy. Try again shortly.\n"
                                );
                            }
                            _ => {
                                eprintln!(
                                    "\nAPI error ({status} {error_type}): {message}\n"
                                );
                            }
                        }
                        conversation.pop(); // Remove the failed user message.
                    }
                    Err(ApiError::UnexpectedResponse { status, body }) => {
                        eprintln!("\nUnexpected error ({status}): {body}\n");
                        conversation.pop();
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                println!("Goodbye!");
                break;
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("Interrupted. Type \"quit\" or press Ctrl-D to exit.");
            }
            Err(e) => {
                eprintln!("Readline error: {e}");
                break;
            }
        }
    }

    Ok(())
}
