---
title: Summary
description: Review the complete API integration pipeline from request construction through response parsing and error handling.
---

# Summary

> **What you'll learn:**
> - How all the Chapter 2 components fit together into a complete API integration pipeline
> - How to verify your integration works end-to-end by running a multi-turn conversation in the REPL
> - What to focus on next as you move from single-shot API calls to the agentic loop in Chapter 3

You started this chapter with a REPL that echoed user input. You are ending it with a REPL that talks to Claude. Let's review everything you built, see the complete working code, and look ahead to what comes next.

## What You Built

Over the course of 13 subchapters, you assembled a complete API integration pipeline:

1. **Conceptual foundation** -- You learned how LLM APIs work: stateless HTTP endpoints that accept a messages array and return generated text, billed by token.

2. **API knowledge** -- You mapped the Anthropic Messages API: the endpoint (`POST /v1/messages`), the three required headers (`x-api-key`, `anthropic-version`, `content-type`), and the request/response structure.

3. **Configuration** -- You built a `Config` struct that loads settings from environment variables with dotenvy, validates required values, provides defaults for optional ones, and redacts secrets in debug output.

4. **HTTP client** -- You used reqwest to build a reusable HTTP client with pre-configured headers, JSON serialization, and async I/O.

5. **Request/response types** -- You defined Rust structs that match the API's JSON schema and used serde's derive macros to serialize requests and deserialize responses.

6. **Message format** -- You modeled the full message hierarchy with enums for roles and content block types, helper methods for construction, and validation for message sequence rules.

7. **System prompts** -- You wrote a system prompt that defines your agent's identity, capabilities, and safety boundaries.

8. **Error handling** -- You created an `ApiError` enum that classifies errors as retryable or terminal and presents human-readable messages.

9. **Retry logic** -- You implemented exponential backoff with jitter, respecting the `retry-after` header on 429 responses.

10. **Async Rust** -- You learned how `async/await` works with tokio, why futures are lazy, and how the runtime drives them to completion.

11. **Streaming preview** -- You saw how server-sent events deliver tokens incrementally and what changes you will need to implement streaming in a later chapter.

## The Complete Code

Here is the final state of your agent after Chapter 2. This is the complete `src/main.rs` that you can run:

```rust
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use std::io::{self, BufRead, Write};

// --- Configuration ---

pub struct Config {
    pub api_key: String,
    pub model: String,
    pub max_tokens: u32,
    pub api_base_url: String,
    pub api_version: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let api_key = env::var("ANTHROPIC_API_KEY")
            .map_err(|_| "ANTHROPIC_API_KEY is not set. \
                          Get your key at https://console.anthropic.com".to_string())?;

        let model = env::var("ANTHROPIC_MODEL")
            .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());

        let max_tokens = env::var("MAX_TOKENS")
            .unwrap_or_else(|_| "4096".to_string())
            .parse::<u32>()
            .map_err(|e| format!("MAX_TOKENS is invalid: {e}"))?;

        let api_base_url = env::var("ANTHROPIC_API_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".to_string());

        let api_version = env::var("ANTHROPIC_API_VERSION")
            .unwrap_or_else(|_| "2023-06-01".to_string());

        Ok(Config { api_key, model, max_tokens, api_base_url, api_version })
    }

    pub fn messages_url(&self) -> String {
        format!("{}/v1/messages", self.api_base_url)
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("api_key", &"[REDACTED]")
            .field("model", &self.model)
            .field("max_tokens", &self.max_tokens)
            .field("api_base_url", &self.api_base_url)
            .finish()
    }
}

// --- API Types ---

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

#[derive(Debug, Deserialize)]
struct ChatResponse {
    #[allow(dead_code)]
    id: String,
    content: Vec<ContentBlock>,
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

// --- Error Handling ---

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

#[derive(Debug)]
enum ApiError {
    Network(reqwest::Error),
    ApiResponse { status: u16, error_type: String, message: String },
    UnexpectedResponse { status: u16, body: String },
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::Network(e) => write!(f, "Network error: {e}"),
            ApiError::ApiResponse { status, error_type, message } => {
                write!(f, "API error ({status} {error_type}): {message}")
            }
            ApiError::UnexpectedResponse { status, body } => {
                write!(f, "Unexpected error ({status}): {body}")
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

impl ApiError {
    fn is_retryable(&self) -> bool {
        match self {
            ApiError::Network(_) => true,
            ApiError::ApiResponse { status, .. } => matches!(status, 429 | 500 | 529),
            ApiError::UnexpectedResponse { status, .. } => *status >= 500,
        }
    }
}

// --- System Prompt ---

const SYSTEM_PROMPT: &str = r#"You are an expert coding assistant embedded in a command-line interface. Your primary goal is to help users write, debug, and understand code.

Guidelines:
- Write clean, idiomatic, well-commented code.
- Provide complete, runnable examples with all necessary imports.
- Use markdown code blocks with the appropriate language identifier.
- Keep explanations concise and practical.
- If you are unsure about something, say so explicitly."#;

// --- HTTP Client ---

fn build_client(config: &Config) -> Result<reqwest::Client, reqwest::Error> {
    let mut headers = HeaderMap::new();
    headers.insert("x-api-key", HeaderValue::from_str(&config.api_key).unwrap());
    headers.insert(
        "anthropic-version",
        HeaderValue::from_str(&config.api_version).unwrap(),
    );

    reqwest::Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(60))
        .build()
}

// --- API Call with Retry ---

async fn send_message(
    client: &reqwest::Client,
    config: &Config,
    messages: &[Message],
) -> Result<ChatResponse, ApiError> {
    let max_retries = 3u32;

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let delay = std::time::Duration::from_millis(
                (1000 * 2u64.pow(attempt - 1)) + (rand::random::<u64>() % 500),
            );
            eprintln!(
                "  Retrying in {:.1}s (attempt {}/{})",
                delay.as_secs_f64(), attempt, max_retries
            );
            tokio::time::sleep(delay).await;
        }

        let request = ChatRequest {
            model: config.model.clone(),
            max_tokens: config.max_tokens,
            messages: messages.to_vec(),
            system: Some(SYSTEM_PROMPT.to_string()),
        };

        let result = client
            .post(&config.messages_url())
            .json(&request)
            .send()
            .await;

        let response = match result {
            Ok(resp) => resp,
            Err(e) if attempt < max_retries => {
                eprintln!("  Network error: {e}");
                continue;
            }
            Err(e) => return Err(ApiError::Network(e)),
        };

        let status = response.status();

        if status.is_success() {
            return response.json().await.map_err(ApiError::Network);
        }

        let status_code = status.as_u16();
        let body = response.text().await.map_err(ApiError::Network)?;

        let is_retryable = matches!(status_code, 429 | 500 | 529);
        if is_retryable && attempt < max_retries {
            eprintln!("  API error {status_code}, retrying...");
            continue;
        }

        return match serde_json::from_str::<ApiErrorResponse>(&body) {
            Ok(err) => Err(ApiError::ApiResponse {
                status: status_code,
                error_type: err.error.error_type,
                message: err.error.message,
            }),
            Err(_) => Err(ApiError::UnexpectedResponse {
                status: status_code,
                body,
            }),
        };
    }

    unreachable!()
}

// --- REPL ---

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Configuration error: {e}");
            std::process::exit(1);
        }
    };

    let client = match build_client(&config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to build HTTP client: {e}");
            std::process::exit(1);
        }
    };

    let mut conversation: Vec<Message> = Vec::new();

    println!("CLI Agent (model: {})", config.model);
    println!("Type 'quit' to exit.\n");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("> ");
        stdout.flush().unwrap();

        let mut line = String::new();
        if stdin.lock().read_line(&mut line).is_err() {
            break;
        }
        let input = line.trim();

        if input.is_empty() {
            continue;
        }
        if input == "quit" {
            println!("Goodbye!");
            break;
        }

        conversation.push(Message {
            role: "user".to_string(),
            content: input.to_string(),
        });

        match send_message(&client, &config, &conversation).await {
            Ok(response) => {
                let text = response
                    .content
                    .iter()
                    .filter_map(|b| b.text.as_deref())
                    .collect::<Vec<_>>()
                    .join("\n");

                println!("\n{text}\n");
                println!(
                    "[{} input + {} output tokens]\n",
                    response.usage.input_tokens, response.usage.output_tokens
                );

                conversation.push(Message {
                    role: "assistant".to_string(),
                    content: text,
                });
            }
            Err(e) => {
                eprintln!("\nError: {e}\n");
                conversation.pop(); // Remove failed user message
            }
        }
    }
}
```

And the `Cargo.toml`:

```toml
[package]
name = "cli-agent"
version = "0.1.0"
edition = "2021"

[dependencies]
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
dotenvy = "0.15"
rand = "0.9"
```

::: python Coming from Python
Compare the complete Rust agent above with the equivalent Python code:
```python
import anthropic, os

client = anthropic.Anthropic()
conversation = []

while True:
    user_input = input("> ")
    if user_input == "quit":
        break
    conversation.append({"role": "user", "content": user_input})
    response = client.messages.create(
        model="claude-sonnet-4-20250514",
        max_tokens=4096,
        system="You are an expert coding assistant...",
        messages=conversation,
    )
    text = response.content[0].text
    print(f"\n{text}\n")
    conversation.append({"role": "assistant", "content": text})
```
The Python version is about 15 lines. The Rust version is about 250. The difference is not wasted code -- it is all the infrastructure that the Python SDK handles invisibly: HTTP client construction, header management, JSON serialization, error classification, retry logic, and type safety. You built every piece yourself, which means you understand exactly what happens at every layer. That understanding becomes invaluable as you add complexity in later chapters.
:::

## Verifying End-to-End

To confirm everything works, run through this test script in your REPL:

```
$ cargo run

CLI Agent (model: claude-sonnet-4-20250514)
Type 'quit' to exit.

> Hello! Can you write a Rust function that reverses a string?

fn reverse_string(s: &str) -> String {
    s.chars().rev().collect()
}
...

> Now modify it to only reverse the alphabetic characters, keeping other characters in place.

fn reverse_alpha(s: &str) -> String {
    let alpha: Vec<char> = s.chars().filter(|c| c.is_alphabetic()).rev().collect();
    ...
}
...

> quit
Goodbye!
```

The second question tests multi-turn context: Claude needs to remember the first function to modify it. If the conversation history is working correctly, Claude will reference the original function in its response.

## What Comes Next

In Chapter 3, you will transform this single-shot request-response pattern into an **agentic loop**. Instead of the user driving every interaction, the agent will be able to:

- Decide when to use tools (like reading files or running commands)
- Execute tool calls and feed the results back to Claude
- Continue the conversation autonomously until the task is complete

The API integration you built in this chapter is the engine of that loop. Every component -- the client, the types, the error handling, the retry logic -- will be used directly. The difference is that instead of one request per user input, the agent makes multiple requests per turn as it reasons through a task.

You are also ready for these enhancements:

- **Streaming** (Chapter 4 or 5) -- replace the blocking `response.json()` call with incremental SSE processing for real-time output.
- **Tool definitions** -- add `tools` to your request body so Claude can call functions like `read_file` and `run_command`.
- **Context management** -- track token usage and implement compaction when conversations approach the context window limit.

The foundation is solid. Let's build on it.

## Exercises

Practice each concept with these exercises. They build on the API integration you created in this chapter.

### Exercise 1: Add a Token Usage Tracker (Easy)

Add a running total of tokens used across the entire session. After each API response, print both the per-request usage and the cumulative session total. Display the final total when the user quits.

- Add `total_input_tokens: u32` and `total_output_tokens: u32` variables before the REPL loop
- Accumulate the values from each `response.usage` after a successful call
- Print the session summary in the quit handler

### Exercise 2: Implement Request Timeout Configuration (Easy)

Make the HTTP client timeout configurable via an environment variable `REQUEST_TIMEOUT_SECS`. Default to 60 seconds if not set. Print the configured timeout at startup.

- Add a `timeout_secs: u64` field to your `Config` struct
- Parse it from the environment with a default of 60
- Pass it to `reqwest::Client::builder().timeout()`

### Exercise 3: Add Structured API Error Reporting (Medium)

Extend your `ApiError` enum with a `RateLimited { retry_after: Option<u64> }` variant. When you receive a 429 response, parse the `retry-after` header and use that value (if present) instead of your default backoff delay.

**Hints:**
- Access the header with `response.headers().get("retry-after")`
- Parse the header value as a `u64` representing seconds
- In your retry loop, check for this variant and use `retry_after` if `Some`, falling back to exponential backoff if `None`

### Exercise 4: Implement a /tokens Command (Medium)

Add a `/tokens` REPL command that estimates the token count for any text the user provides. Use the approximation that 1 token is roughly 4 characters of English text. Display both the character count and the estimated token count.

**Hints:**
- Match on input starting with `/tokens ` in your command handler
- Strip the prefix to get the text to measure
- Calculate `text.len() / 4` as the rough estimate
- Compare this with the actual usage the API reports to see how accurate the approximation is

### Exercise 5: Add Response Caching with a HashMap (Hard)

Implement a simple response cache: if the user sends an identical message to one sent earlier in the session (and no other messages have been sent in between), return the cached response instead of making an API call. Print a note when serving from cache.

**Hints:**
- Store a `HashMap<String, String>` mapping user messages to assistant responses
- Only cache single-shot messages (not messages that depend on conversation context)
- One approach: cache responses when `conversation.len() == 1` (first message) or when the preceding messages are identical to a previous cached sequence
- A simpler approach: cache the hash of the entire messages vector, so identical conversation states hit the cache

## Key Takeaways

- You built a complete pipeline from environment configuration through HTTP requests, JSON parsing, error handling, and retry logic to a working multi-turn REPL.
- The Rust implementation is more verbose than the equivalent Python, but every line serves a purpose: type safety, explicit error handling, and zero-cost abstractions.
- Multi-turn conversations work by accumulating messages in a vector and sending the full history with every request -- the API is stateless.
- The architecture you built (Config, Client, Types, Error handling, Retry) is the same architecture used by production agents -- the difference is scale, not shape.
- Chapter 3 will evolve the single request-response pattern into an agentic loop where Claude can use tools and drive multi-step tasks autonomously.
