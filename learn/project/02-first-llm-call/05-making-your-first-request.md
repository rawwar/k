---
title: Making Your First Request
description: Send a complete request to the Anthropic Messages API and receive Claude's response in your terminal.
---

# Making Your First Request

> **What you'll learn:**
> - How to assemble a valid Messages API request with model, messages, and max_tokens fields
> - How to send the request from your REPL and print Claude's text response to the terminal
> - How to diagnose common first-request failures like authentication errors and malformed payloads

This is the moment everything comes together. You have an API key, an HTTP client with the right headers, and you know the request format. Now you are going to send a real message to Claude and print the response in your terminal. By the end of this subchapter, your CLI will go from echoing user input to generating intelligent responses.

## Defining the Request Types

First, let's define Rust structs that match the Anthropic request format. You already saw these briefly in the reqwest subchapter. Now let's be more deliberate about them:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: String,
}
```

`ChatRequest` only derives `Serialize` because you send it (Rust -> JSON). `Message` derives both `Serialize` and `Deserialize` because you send user messages and receive assistant messages. The `Clone` derive on `Message` lets you copy messages when building conversation histories.

## Defining the Response Types

The API response has a richer structure. Here are the types you need to parse it:

```rust
#[derive(Debug, Deserialize)]
struct ChatResponse {
    id: String,
    content: Vec<ContentBlock>,
    model: String,
    stop_reason: Option<String>,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}
```

A few things to note:

- **`ContentBlock.block_type`** uses `#[serde(rename = "type")]` because `type` is a reserved keyword in Rust. The JSON field is `"type"`, but we access it as `block_type` in code.
- **`ContentBlock.text`** is `Option<String>` because not all content blocks have text -- tool use blocks have different fields. For now, you will only deal with text blocks.
- **`stop_reason`** is `Option<String>` to handle cases where the field might be null.

## The Complete First Request

Here is a complete, runnable program that sends a message to Claude and prints the response:

```rust
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    id: String,
    content: Vec<ContentBlock>,
    model: String,
    stop_reason: Option<String>,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

fn build_client(api_key: &str) -> Result<reqwest::Client, reqwest::Error> {
    let mut headers = HeaderMap::new();
    headers.insert("x-api-key", HeaderValue::from_str(api_key).unwrap());
    headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));

    reqwest::Client::builder()
        .default_headers(headers)
        .build()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let api_key = env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY must be set");

    let client = build_client(&api_key)?;

    let request = ChatRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        max_tokens: 1024,
        messages: vec![Message {
            role: "user".to_string(),
            content: "Explain what a REPL is in two sentences.".to_string(),
        }],
    };

    println!("Sending request to Claude...");

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .json(&request)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await?;
        eprintln!("API error ({}): {}", status, error_body);
        return Ok(());
    }

    let chat_response: ChatResponse = response.json().await?;

    // Extract text from the first content block
    if let Some(block) = chat_response.content.first() {
        if let Some(text) = &block.text {
            println!("\nClaude says:\n{text}");
        }
    }

    println!("\n[Tokens used: {} input, {} output]",
        chat_response.usage.input_tokens,
        chat_response.usage.output_tokens
    );

    Ok(())
}
```

Run this with `cargo run` (after setting `ANTHROPIC_API_KEY`), and you will see something like:

```
Sending request to Claude...

Claude says:
A REPL (Read-Eval-Print Loop) is an interactive programming environment that
reads user input, evaluates it as code or a command, prints the result, and
then loops back to wait for more input. It provides an immediate feedback
cycle that is invaluable for exploring APIs, testing code snippets, and
debugging.

[Tokens used: 18 input, 52 output]
```

Congratulations -- your Rust CLI just had its first conversation with an LLM.

## Integrating with the REPL

Now let's connect this to the REPL you built in Chapter 1. Instead of echoing user input, the REPL will send it to Claude:

```rust
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::env;
use std::io::{self, BufRead, Write};

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
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

fn build_client(api_key: &str) -> Result<reqwest::Client, reqwest::Error> {
    let mut headers = HeaderMap::new();
    headers.insert("x-api-key", HeaderValue::from_str(api_key).unwrap());
    headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));

    reqwest::Client::builder()
        .default_headers(headers)
        .build()
}

async fn send_message(
    client: &reqwest::Client,
    messages: &[Message],
) -> Result<ChatResponse, Box<dyn std::error::Error>> {
    let request = ChatRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        max_tokens: 4096,
        messages: messages.to_vec(),
    };

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await?;
        return Err(format!("API error ({}): {}", status, body).into());
    }

    let chat_response = response.json().await?;
    Ok(chat_response)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let api_key = env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY must be set");

    let client = build_client(&api_key)?;
    let mut conversation: Vec<Message> = Vec::new();

    println!("CLI Agent (type 'quit' to exit)");
    println!("================================\n");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("> ");
        stdout.flush()?;

        let mut line = String::new();
        stdin.lock().read_line(&mut line)?;
        let input = line.trim();

        if input.is_empty() {
            continue;
        }
        if input == "quit" {
            println!("Goodbye!");
            break;
        }

        // Add user message to conversation history
        conversation.push(Message {
            role: "user".to_string(),
            content: input.to_string(),
        });

        // Send to Claude
        match send_message(&client, &conversation).await {
            Ok(response) => {
                let assistant_text = response
                    .content
                    .iter()
                    .filter_map(|block| block.text.as_deref())
                    .collect::<Vec<_>>()
                    .join("\n");

                println!("\n{assistant_text}\n");

                // Add assistant response to conversation history
                conversation.push(Message {
                    role: "assistant".to_string(),
                    content: assistant_text,
                });

                println!(
                    "[{} input + {} output tokens]\n",
                    response.usage.input_tokens, response.usage.output_tokens
                );
            }
            Err(e) => {
                eprintln!("Error: {e}\n");
                // Remove the failed user message so we can retry
                conversation.pop();
            }
        }
    }

    Ok(())
}
```

This version maintains a `conversation` vector that grows with each turn. When the user types a message, it gets appended to the vector, the full conversation is sent to Claude, and the response is appended as an assistant message. The next user message will include the entire history, giving Claude context for follow-up questions.

::: python Coming from Python
In Python, this same loop looks like:
```python
import anthropic

client = anthropic.Anthropic()
conversation = []

while True:
    user_input = input("> ")
    conversation.append({"role": "user", "content": user_input})

    response = client.messages.create(
        model="claude-sonnet-4-20250514",
        max_tokens=4096,
        messages=conversation,
    )
    text = response.content[0].text
    print(text)
    conversation.append({"role": "assistant", "content": text})
```
The logic is identical. The Rust version is more verbose because you explicitly define the types and handle errors, but the structure -- accumulate messages, send the full history, append the response -- is the same.
:::

## Diagnosing Common First-Request Failures

When your first request does not work, here are the most common causes:

### 401 Unauthorized
```json
{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}
```
Your API key is wrong, missing, or you used the wrong header name. Double-check that `ANTHROPIC_API_KEY` is set and that the header is `x-api-key` (not `Authorization`).

### 400 Bad Request
```json
{"type":"error","error":{"type":"invalid_request_error","message":"messages: field required"}}
```
Your request body is malformed. Common causes: missing `messages` field, empty messages array, messages not alternating between user and assistant roles, or first message not from user.

### 404 Not Found
You are hitting the wrong URL. Verify it is `https://api.anthropic.com/v1/messages` -- note the `v1`, not `v2` or just `/messages`.

### Connection Errors
If you see a network error before getting any HTTP status, check your internet connection and firewall settings. Corporate networks sometimes block outbound HTTPS to unfamiliar domains.

### Timeout
Requests typically complete in 5-30 seconds depending on the model and response length. If you are behind a slow connection, increase the client timeout:

```rust
reqwest::Client::builder()
    .default_headers(headers)
    .timeout(std::time::Duration::from_secs(60))
    .build()
```

## Key Takeaways

- A complete Messages API call requires typed request structs (`ChatRequest`, `Message`), response structs (`ChatResponse`, `ContentBlock`, `Usage`), and a configured HTTP client.
- Always check `response.status().is_success()` before attempting to parse the response body as your expected type.
- Multi-turn conversations work by maintaining a `Vec<Message>` that accumulates user and assistant messages, sent in full with each request.
- Common first-request failures are almost always authentication errors (wrong API key or header) or malformed request bodies (missing required fields).
- Extracting text from the response requires iterating over the `content` array and filtering for text blocks, since responses can contain multiple block types.
