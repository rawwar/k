---
title: Rate Limiting and Retries
description: Implement exponential backoff and retry logic to handle rate limits and transient API failures gracefully.
---

# Rate Limiting and Retries

> **What you'll learn:**
> - How to read rate limit headers from the API response to know your remaining quota
> - How to implement exponential backoff with jitter for retrying failed requests
> - How to set a maximum retry count and total timeout to prevent infinite retry loops

::: warning Pricing and Limits Change Frequently
The specific rate limit numbers and pricing in this chapter reflect values at the time of writing. Check each provider's current documentation for up-to-date figures.
:::

In the previous subchapter, you classified errors as retryable or terminal. Now you are going to build the retry logic that automatically handles transient failures. A well-behaved API client does not hammer the server after getting a 429 -- it backs off, waits, and tries again with increasing patience. This is exponential backoff, and every production API integration needs it.

## Rate Limit Headers

When the Anthropic API responds, it includes headers that tell you your current rate limit status:

| Header | Meaning |
|---|---|
| `anthropic-ratelimit-requests-limit` | Maximum requests allowed per minute |
| `anthropic-ratelimit-requests-remaining` | Requests remaining in the current window |
| `anthropic-ratelimit-requests-reset` | When the request limit resets (ISO 8601 timestamp) |
| `anthropic-ratelimit-tokens-limit` | Maximum tokens allowed per minute |
| `anthropic-ratelimit-tokens-remaining` | Tokens remaining in the current window |
| `anthropic-ratelimit-tokens-reset` | When the token limit resets |
| `retry-after` | Seconds to wait before retrying (only on 429 responses) |

You can read these from the response headers in Rust:

```rust
fn read_rate_limit_info(response: &reqwest::Response) {
    let headers = response.headers();

    if let Some(remaining) = headers.get("anthropic-ratelimit-requests-remaining") {
        println!("Requests remaining: {}", remaining.to_str().unwrap_or("?"));
    }

    if let Some(remaining) = headers.get("anthropic-ratelimit-tokens-remaining") {
        println!("Tokens remaining: {}", remaining.to_str().unwrap_or("?"));
    }

    if let Some(retry_after) = headers.get("retry-after") {
        println!("Retry after: {}s", retry_after.to_str().unwrap_or("?"));
    }
}
```

The `retry-after` header is the most important for your retry logic. When present, it tells you exactly how many seconds to wait before your next request will be accepted.

## Exponential Backoff

The simplest retry strategy is to wait a fixed amount of time between retries. But this is suboptimal -- if many clients are all retrying at the same fixed interval, they create thundering herds that all hit the API at the same moment.

Exponential backoff solves this by doubling the wait time after each failed attempt:

- Attempt 1 fails -> wait 1 second
- Attempt 2 fails -> wait 2 seconds
- Attempt 3 fails -> wait 4 seconds
- Attempt 4 fails -> wait 8 seconds

Here is the basic formula: `delay = base * 2^attempt`, where `base` is typically 1 second.

## Adding Jitter

Even with exponential backoff, clients that start at the same time will retry at the same times. **Jitter** adds randomness to break up the synchronization:

```
delay = base * 2^attempt + random(0, base)
```

This spreads retries across a time window instead of concentrating them at exact intervals.

## Implementation

Here is a complete retry wrapper for your API calls:

```rust
use std::time::Duration;
use rand::Rng;

/// Configuration for retry behavior.
struct RetryConfig {
    /// Maximum number of retry attempts (not counting the initial request).
    max_retries: u32,
    /// Base delay between retries (doubles each attempt).
    base_delay: Duration,
    /// Maximum delay between retries (caps the exponential growth).
    max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        RetryConfig {
            max_retries: 3,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
        }
    }
}

/// Calculate the delay for a given retry attempt with jitter.
fn retry_delay(config: &RetryConfig, attempt: u32) -> Duration {
    let exp_delay = config.base_delay * 2u32.pow(attempt);
    let capped = exp_delay.min(config.max_delay);

    // Add jitter: random value between 0 and base_delay
    let jitter_ms = rand::rng().random_range(0..config.base_delay.as_millis() as u64);
    capped + Duration::from_millis(jitter_ms)
}
```

Note that this uses the `rand` crate. Add it to your `Cargo.toml`:

```toml
[dependencies]
rand = "0.9"
```

Now wrap your API call function with retry logic:

```rust
use serde::{Deserialize, Serialize};
use std::time::Duration;

// (Using the types defined in previous subchapters)

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

async fn send_message_with_retry(
    client: &reqwest::Client,
    messages: &[Message],
    system: Option<&str>,
) -> Result<ChatResponse, ApiError> {
    let config = RetryConfig::default();
    let mut last_error: Option<ApiError> = None;

    for attempt in 0..=config.max_retries {
        if attempt > 0 {
            let delay = retry_delay(&config, attempt - 1);
            eprintln!(
                "Retrying in {:.1}s (attempt {}/{})",
                delay.as_secs_f64(),
                attempt,
                config.max_retries
            );
            tokio::time::sleep(delay).await;
        }

        match send_message_once(client, messages, system).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                if e.is_retryable() && attempt < config.max_retries {
                    eprintln!("Request failed: {e}");
                    last_error = Some(e);
                    continue;
                }
                return Err(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| ApiError::Network(
        reqwest::Client::new().get("").send().now_or_never().unwrap().unwrap_err()
    )))
}

async fn send_message_once(
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
        .await?;

    let status = response.status();

    if status.is_success() {
        let chat_response = response.json().await?;
        Ok(chat_response)
    } else {
        let status_code = status.as_u16();
        let body = response.text().await?;

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

Let's also clean up the implementation by providing a simpler fallback for the last-error case:

```rust
async fn send_message_with_retry(
    client: &reqwest::Client,
    messages: &[Message],
    system: Option<&str>,
) -> Result<ChatResponse, ApiError> {
    let config = RetryConfig::default();

    for attempt in 0..=config.max_retries {
        if attempt > 0 {
            let delay = retry_delay(&config, attempt - 1);
            eprintln!(
                "Retrying in {:.1}s (attempt {}/{})",
                delay.as_secs_f64(),
                attempt,
                config.max_retries
            );
            tokio::time::sleep(delay).await;
        }

        let result = send_message_once(client, messages, system).await;

        match result {
            Ok(response) => return Ok(response),
            Err(ref e) if e.is_retryable() && attempt < config.max_retries => {
                eprintln!("Request failed (will retry): {e}");
                continue;
            }
            Err(e) => return Err(e),
        }
    }

    // This is unreachable because the loop always returns, but Rust needs it
    unreachable!("retry loop should always return")
}
```

::: python Coming from Python
In Python, you might use the `tenacity` or `backoff` library for retries:
```python
from tenacity import retry, stop_after_attempt, wait_exponential

@retry(stop=stop_after_attempt(4), wait=wait_exponential(multiplier=1, max=30))
def call_api(messages):
    response = requests.post(url, json=body, headers=headers)
    response.raise_for_status()
    return response.json()
```
The Rust version is more explicit -- you write the retry loop yourself rather than using a decorator. The advantage is that you have full control over which errors to retry, what to log, and how to compute delays. The structure is the same: try, check, sleep, repeat.
:::

## Respecting the retry-after Header

When the API returns a 429, it includes a `retry-after` header with the exact number of seconds to wait. Your retry logic should respect this instead of using its own calculated delay:

```rust
async fn send_message_with_retry_v2(
    client: &reqwest::Client,
    messages: &[Message],
    system: Option<&str>,
) -> Result<ChatResponse, ApiError> {
    let config = RetryConfig::default();

    for attempt in 0..=config.max_retries {
        let request = ChatRequest {
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 4096,
            messages: messages.to_vec(),
            system: system.map(String::from),
        };

        let result = client
            .post("https://api.anthropic.com/v1/messages")
            .json(&request)
            .send()
            .await;

        let response = match result {
            Ok(resp) => resp,
            Err(e) if attempt < config.max_retries => {
                let delay = retry_delay(&config, attempt);
                eprintln!("Network error (retrying in {:.1}s): {e}", delay.as_secs_f64());
                tokio::time::sleep(delay).await;
                continue;
            }
            Err(e) => return Err(ApiError::Network(e)),
        };

        let status = response.status();

        if status.is_success() {
            let chat_response = response.json().await.map_err(ApiError::Network)?;
            return Ok(chat_response);
        }

        // Check for retry-after header before consuming the body
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<f64>().ok());

        let status_code = status.as_u16();
        let body = response.text().await.map_err(ApiError::Network)?;

        // Decide whether to retry
        let is_retryable = matches!(status_code, 429 | 500 | 529);
        if is_retryable && attempt < config.max_retries {
            let delay = if let Some(seconds) = retry_after {
                Duration::from_secs_f64(seconds)
            } else {
                retry_delay(&config, attempt)
            };
            eprintln!(
                "API error {status_code} (retrying in {:.1}s): {}",
                delay.as_secs_f64(),
                body.chars().take(100).collect::<String>()
            );
            tokio::time::sleep(delay).await;
            continue;
        }

        // Non-retryable or out of retries
        return match serde_json::from_str::<ApiErrorResponse>(&body) {
            Ok(error_response) => Err(ApiError::ApiResponse {
                status: status_code,
                error_type: error_response.error.error_type,
                message: error_response.error.message,
            }),
            Err(_) => Err(ApiError::UnexpectedResponse {
                status: status_code,
                body,
            }),
        };
    }

    unreachable!()
}
```

This version reads the `retry-after` header from 429 responses and uses it as the delay instead of the calculated exponential backoff. If the header is missing, it falls back to the exponential backoff calculation.

## When Not to Retry

Be cautious about what you retry. Retrying a request that the server already processed could cause duplicate side effects. For the Messages API this is safe -- creating a message is idempotent in the sense that a duplicate request just costs extra tokens. But when you add tool use later (where the model might request to write a file), you will need to be more careful about when retries are appropriate.

Also, do not retry forever. The `max_retries` limit prevents your agent from hanging indefinitely when the API is down for an extended period. Three retries with exponential backoff means you wait at most about 15 seconds total before giving up and telling the user.

::: wild In the Wild
Claude Code implements a retry layer that respects the `retry-after` header and uses exponential backoff with jitter as a fallback. It caps retries at a small number (typically 2-3) and provides the user with a clear error message when retries are exhausted. OpenCode takes a similar approach and additionally monitors token usage rates to preemptively slow down requests before hitting rate limits.
:::

## Key Takeaways

- The Anthropic API includes rate limit headers (`anthropic-ratelimit-*-remaining`) and a `retry-after` header on 429 responses that tells you exactly how long to wait.
- Exponential backoff with jitter (`base * 2^attempt + random`) prevents thundering herds when multiple clients retry simultaneously.
- Always set a maximum retry count (3 is a good default) to prevent infinite retry loops when the API is persistently unavailable.
- Respect the `retry-after` header when present -- it is the server's best estimate of when your request will be accepted.
- Only retry transient errors (429, 500, 529, network errors). Never retry client errors like 400 or 401 -- they will fail the same way every time.
