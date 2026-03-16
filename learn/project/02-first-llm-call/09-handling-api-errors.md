---
title: Handling API Errors
description: Parse and respond to Anthropic API error responses including validation errors, overload conditions, and server failures.
---

# Handling API Errors

> **What you'll learn:**
> - How to distinguish between client errors (400, 401, 422) and server errors (500, 529) in API responses
> - How to deserialize the Anthropic error response format into a typed Rust error enum
> - How to present meaningful error messages to the user instead of raw HTTP status codes

So far you have handled the happy path: you send a request, get a 200 response, and parse the result. But real-world API calls fail -- frequently. Networks drop, API keys expire, rate limits kick in, and servers have bad days. A production agent needs to handle all of these gracefully. Let's build that error handling layer.

## The Anthropic Error Format

When the Anthropic API returns an error, the response body follows a consistent structure:

```json
{
  "type": "error",
  "error": {
    "type": "authentication_error",
    "message": "invalid x-api-key"
  }
}
```

The outer `type` is always `"error"`. The inner `error` object contains:
- **`type`** -- a machine-readable error category
- **`message`** -- a human-readable description

Here are the error types you will encounter:

| HTTP Status | Error Type | Meaning |
|---|---|---|
| 400 | `invalid_request_error` | Malformed request body, missing fields, invalid parameters |
| 401 | `authentication_error` | Bad or missing API key |
| 403 | `permission_error` | API key lacks permission for this operation |
| 404 | `not_found_error` | Invalid endpoint URL |
| 429 | `rate_limit_error` | Too many requests, slow down |
| 500 | `api_error` | Internal server error |
| 529 | `overloaded_error` | API is temporarily overloaded |

## Modeling Errors in Rust

Let's create Rust types that capture this error structure:

```rust
use serde::Deserialize;
use std::fmt;

/// The top-level error response from the Anthropic API.
#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    #[serde(rename = "type")]
    response_type: String,
    error: ApiErrorDetail,
}

/// The inner error detail with a type and human-readable message.
#[derive(Debug, Deserialize)]
struct ApiErrorDetail {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}
```

Now create an error enum that covers both API errors and network errors:

```rust
/// All the ways an API call can fail.
#[derive(Debug)]
enum ApiError {
    /// Network-level failure (DNS, connection, timeout)
    Network(reqwest::Error),
    /// API returned an error response with a status code
    ApiResponse {
        status: u16,
        error_type: String,
        message: String,
    },
    /// Could not parse the error response body
    UnexpectedResponse {
        status: u16,
        body: String,
    },
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::Network(e) => write!(f, "Network error: {e}"),
            ApiError::ApiResponse { status, error_type, message } => {
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
```

The `Display` implementation controls what the user sees. Instead of a raw HTTP status code and a blob of JSON, they get a formatted message like:

```
API error (401 authentication_error): invalid x-api-key
```

::: python Coming from Python
In Python, the `anthropic` SDK raises typed exceptions:
```python
from anthropic import AuthenticationError, RateLimitError

try:
    response = client.messages.create(...)
except AuthenticationError as e:
    print(f"Bad API key: {e}")
except RateLimitError as e:
    print(f"Rate limited: {e}")
```
In Rust, you achieve the same thing with an error enum and pattern matching. The Rust approach is more explicit -- you define every variant yourself -- but it gives you full control over how errors are categorized and displayed.
:::

## Parsing Error Responses

Now update your `send_message` function to parse error responses properly:

```rust
use serde::{Deserialize, Serialize};

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
    id: String,
    content: Vec<ContentBlock>,
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

async fn send_message(
    client: &reqwest::Client,
    messages: &[Message],
    system: Option<&str>,
) -> Result<ChatResponse, ApiError> {
    let request = ChatRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        max_tokens: 4096,
        messages: messages.to_vec(),
        system: system.map(String::from),
    };

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .json(&request)
        .send()
        .await?; // Network errors become ApiError::Network via From impl

    let status = response.status();

    if status.is_success() {
        let chat_response = response.json().await?;
        Ok(chat_response)
    } else {
        let status_code = status.as_u16();
        let body = response.text().await?;

        // Try to parse the error as the standard Anthropic format
        match serde_json::from_str::<ApiErrorResponse>(&body) {
            Ok(error_response) => Err(ApiError::ApiResponse {
                status: status_code,
                error_type: error_response.error.error_type,
                message: error_response.error.message,
            }),
            Err(_) => Err(ApiError::UnexpectedResponse {
                status: status_code,
                body,
            }),
        }
    }
}
```

The key logic is in the `else` branch: read the body as text, try to parse it as the Anthropic error format, and fall back to `UnexpectedResponse` if parsing fails (which can happen with infrastructure-level errors like 502 Bad Gateway from a proxy).

## Handling Errors in the REPL

Different errors deserve different treatments in your REPL:

```rust
async fn handle_user_input(
    client: &reqwest::Client,
    conversation: &mut Vec<Message>,
    input: &str,
) {
    conversation.push(Message {
        role: "user".to_string(),
        content: input.to_string(),
    });

    match send_message(client, conversation, Some(SYSTEM_PROMPT)).await {
        Ok(response) => {
            let text = response
                .content
                .iter()
                .filter_map(|b| b.text.as_deref())
                .collect::<Vec<_>>()
                .join("\n");
            println!("\n{text}\n");

            conversation.push(Message {
                role: "assistant".to_string(),
                content: text,
            });
        }
        Err(ApiError::Network(e)) => {
            eprintln!("\nNetwork error: {e}");
            eprintln!("Check your internet connection and try again.\n");
            conversation.pop(); // Remove the failed user message
        }
        Err(ApiError::ApiResponse { status, error_type, message }) => {
            match status {
                401 => {
                    eprintln!("\nAuthentication failed: {message}");
                    eprintln!("Check your ANTHROPIC_API_KEY environment variable.\n");
                }
                429 => {
                    eprintln!("\nRate limited: {message}");
                    eprintln!("Wait a moment and try again.\n");
                }
                529 => {
                    eprintln!("\nAPI is overloaded: {message}");
                    eprintln!("The service is temporarily busy. Try again in a few seconds.\n");
                }
                _ => {
                    eprintln!("\nAPI error ({status} {error_type}): {message}\n");
                }
            }
            conversation.pop(); // Remove the failed user message
        }
        Err(ApiError::UnexpectedResponse { status, body }) => {
            eprintln!("\nUnexpected error ({status}): {body}\n");
            conversation.pop();
        }
    }
}

const SYSTEM_PROMPT: &str = "You are an expert coding assistant.";
```

Notice the pattern: on success, append the assistant message to the conversation. On failure, pop the user message that failed. This keeps the conversation history clean and allows the user to retry without duplicate messages.

## Classifying Errors: Retryable vs. Terminal

Not all errors are equal. Some are worth retrying, others are not:

**Retryable errors** (transient, might succeed if you try again):
- 429 Rate Limit -- wait and retry
- 500 Internal Server Error -- might be a blip
- 529 Overloaded -- server is temporarily busy
- Network timeouts -- connection might recover

**Terminal errors** (will not succeed no matter how many times you retry):
- 400 Invalid Request -- your request body is wrong
- 401 Authentication Error -- your API key is bad
- 403 Permission Error -- your key lacks access
- 404 Not Found -- wrong URL

Add a method to classify errors:

```rust
impl ApiError {
    /// Returns true if the error is transient and the request should be retried.
    fn is_retryable(&self) -> bool {
        match self {
            ApiError::Network(_) => true, // Network errors are often transient
            ApiError::ApiResponse { status, .. } => {
                matches!(status, 429 | 500 | 529)
            }
            ApiError::UnexpectedResponse { status, .. } => {
                *status >= 500
            }
        }
    }
}
```

You will use this classification in the next subchapter to implement automatic retries with exponential backoff.

::: wild In the Wild
Claude Code wraps all API calls in a retry layer that automatically retries transient errors up to 3 times with exponential backoff. It distinguishes between retryable and terminal errors just like the classification above. OpenCode implements similar retry logic and also logs error details for debugging. In both agents, the user never sees a raw HTTP status code -- every error is translated into a human-readable message with actionable guidance.
:::

## Key Takeaways

- The Anthropic API returns errors in a consistent JSON format with a `type` and `message` field inside an `error` object.
- Model API errors as a Rust enum with variants for network errors, parsed API errors, and unexpected response formats -- this gives you exhaustive matching in error handlers.
- Classify errors as retryable (429, 500, 529, network) or terminal (400, 401, 403) to decide whether to retry or inform the user immediately.
- On error, remove the failed user message from the conversation history to keep the message sequence valid for retry.
- Implement the `Display` trait on your error type to give users human-readable messages instead of raw status codes and JSON blobs.
