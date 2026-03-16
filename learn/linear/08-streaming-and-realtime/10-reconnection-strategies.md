---
title: Reconnection Strategies
description: Recovering from network failures during streaming with exponential backoff, jitter, stream resumption via Last-Event-ID, and idempotency considerations.
---

# Reconnection Strategies

> **What you'll learn:**
> - How to implement exponential backoff with jitter for reconnection attempts after stream disconnection
> - Using the SSE Last-Event-ID mechanism to resume streams from the point of disconnection
> - Distinguishing between retryable errors (network timeouts) and terminal errors (authentication failures, rate limits)

Network connections fail. Especially long-lived streaming connections, which are open for seconds or minutes while an LLM generates a response. Wi-Fi drops, corporate VPNs reconnect, cellular networks switch towers, and load balancers enforce connection timeouts. Your agent must handle these failures gracefully -- reconnecting automatically when possible and providing clear feedback when the error is not recoverable.

## The Retry Landscape

Not all errors deserve a retry. The first step in any reconnection strategy is classifying the error:

```rust
#[derive(Debug)]
enum ErrorCategory {
    /// Network issues: retry with backoff
    Transient,
    /// Server overloaded: retry with longer backoff
    RateLimited { retry_after: Option<std::time::Duration> },
    /// Bad request, auth failure: do not retry
    Terminal,
    /// Unknown: retry a limited number of times
    Unknown,
}

fn categorize_error(status: Option<reqwest::StatusCode>, error: &reqwest::Error) -> ErrorCategory {
    // Check for network-level errors first
    if error.is_timeout() || error.is_connect() {
        return ErrorCategory::Transient;
    }

    // Check HTTP status codes
    match status {
        Some(status) if status == reqwest::StatusCode::TOO_MANY_REQUESTS => {
            ErrorCategory::RateLimited { retry_after: None }
        }
        Some(status) if status.is_server_error() => {
            // 500, 502, 503, 529 -- server-side issues, likely transient
            ErrorCategory::Transient
        }
        Some(status) if status == reqwest::StatusCode::UNAUTHORIZED => {
            ErrorCategory::Terminal // Bad API key, do not retry
        }
        Some(status) if status == reqwest::StatusCode::BAD_REQUEST => {
            ErrorCategory::Terminal // Malformed request, retrying won't help
        }
        Some(status) if status == reqwest::StatusCode::NOT_FOUND => {
            ErrorCategory::Terminal // Wrong endpoint
        }
        _ => ErrorCategory::Unknown,
    }
}
```

The golden rule: **only retry errors that might resolve on their own.** Network timeouts, DNS failures, and server errors (5xx) are transient -- the next attempt might succeed. Authentication failures (401), bad requests (400), and not found (404) are terminal -- retrying the same request will produce the same error.

## Exponential Backoff

When a transient error occurs, you should not retry immediately. If the server is overloaded, a flood of immediate retries makes the problem worse. Instead, you wait before retrying, doubling the wait time after each failure. This is **exponential backoff**:

```
Attempt 1: wait 1 second
Attempt 2: wait 2 seconds
Attempt 3: wait 4 seconds
Attempt 4: wait 8 seconds
Attempt 5: wait 16 seconds (capped at max_delay)
```

Here is a clean implementation:

```rust
use std::time::Duration;

pub struct BackoffConfig {
    /// Initial delay before the first retry
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Multiplier for each successive delay (typically 2.0)
    pub multiplier: f64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            max_retries: 5,
            multiplier: 2.0,
        }
    }
}

impl BackoffConfig {
    /// Calculate the delay for a given attempt number (0-indexed)
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay_ms = self.initial_delay.as_millis() as f64
            * self.multiplier.powi(attempt as i32);
        let capped = delay_ms.min(self.max_delay.as_millis() as f64);
        Duration::from_millis(capped as u64)
    }
}
```

## Adding Jitter

Pure exponential backoff has a thundering herd problem: if 100 clients disconnect at the same time (because a server restarted), they all retry at the same times (1s, 2s, 4s, 8s...), creating synchronized load spikes. **Jitter** randomizes the delay to spread retries over time:

```rust
use rand::Rng;

pub fn delay_with_jitter(base_delay: Duration) -> Duration {
    let mut rng = rand::thread_rng();
    // "Full jitter": random value between 0 and the base delay
    let jittered_ms = rng.gen_range(0..=base_delay.as_millis() as u64);
    Duration::from_millis(jittered_ms)
}

pub fn delay_with_equal_jitter(base_delay: Duration) -> Duration {
    let mut rng = rand::thread_rng();
    let half = base_delay.as_millis() as u64 / 2;
    // "Equal jitter": half the base delay + random value up to half
    let jittered_ms = half + rng.gen_range(0..=half);
    Duration::from_millis(jittered_ms)
}
```

There are several jitter strategies:

- **Full jitter:** Random between 0 and the base delay. Maximally spreads retries but can produce very short waits.
- **Equal jitter:** Half the base delay plus a random amount up to half. Guarantees a minimum wait while still spreading retries.
- **Decorrelated jitter:** Each delay is random between the initial delay and 3x the previous delay. Provides good spread with no state beyond the previous delay.

For an LLM streaming agent, equal jitter is a good default -- it avoids the "instant retry" case of full jitter while still preventing thundering herds.

::: python Coming from Python
Python's `tenacity` library provides retry logic with backoff:
```python
from tenacity import retry, wait_exponential, stop_after_attempt

@retry(wait=wait_exponential(multiplier=1, max=30), stop=stop_after_attempt(5))
async def stream_with_retry():
    async with client.stream("POST", url) as response:
        async for chunk in response.aiter_bytes():
            yield chunk
```
Rust does not have a decorator equivalent, so retry logic is typically implemented as a loop or via crates like `backon` or `tokio-retry`. The loop approach is more explicit but gives you fine-grained control over what state to preserve between retries -- important when you need to resume a stream from a specific event ID.
:::

## The Retry Loop

Here is a complete retry loop that combines error categorization, exponential backoff with jitter, and stream resumption:

```rust
use reqwest::Client;

pub struct StreamConnection {
    client: Client,
    url: String,
    request_body: String,
    backoff: BackoffConfig,
    last_event_id: Option<String>,
    accumulated_text: String,
}

impl StreamConnection {
    pub fn new(client: Client, url: String, request_body: String) -> Self {
        Self {
            client,
            url,
            request_body,
            backoff: BackoffConfig::default(),
            last_event_id: None,
            accumulated_text: String::new(),
        }
    }

    pub async fn stream_with_retry(
        &mut self,
    ) -> Result<String, StreamError> {
        let mut attempt = 0;

        loop {
            match self.attempt_stream().await {
                Ok(()) => {
                    // Stream completed successfully
                    return Ok(self.accumulated_text.clone());
                }
                Err(e) => {
                    let category = e.category();
                    match category {
                        ErrorCategory::Terminal => {
                            return Err(e);
                        }
                        ErrorCategory::RateLimited { retry_after } => {
                            let delay = retry_after.unwrap_or(Duration::from_secs(60));
                            eprintln!(
                                "Rate limited. Waiting {:?} before retry...",
                                delay
                            );
                            tokio::time::sleep(delay).await;
                            // Don't increment attempt counter for rate limits
                        }
                        ErrorCategory::Transient | ErrorCategory::Unknown => {
                            if attempt >= self.backoff.max_retries {
                                return Err(e);
                            }
                            let base_delay = self.backoff.delay_for_attempt(attempt);
                            let delay = delay_with_equal_jitter(base_delay);
                            eprintln!(
                                "Connection lost (attempt {}/{}). Retrying in {:?}...",
                                attempt + 1,
                                self.backoff.max_retries,
                                delay
                            );
                            tokio::time::sleep(delay).await;
                            attempt += 1;
                        }
                    }
                }
            }
        }
    }

    async fn attempt_stream(&mut self) -> Result<(), StreamError> {
        let mut request = self
            .client
            .post(&self.url)
            .header("content-type", "application/json")
            .header("accept", "text/event-stream")
            .body(self.request_body.clone());

        // Include Last-Event-ID if we're resuming
        if let Some(ref id) = self.last_event_id {
            request = request.header("Last-Event-ID", id);
        }

        let response = request.send().await.map_err(|e| {
            StreamError::new(e.to_string(), categorize_error(None, &e))
        })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let category = match status.as_u16() {
                429 => ErrorCategory::RateLimited { retry_after: None },
                401 | 403 => ErrorCategory::Terminal,
                400 => ErrorCategory::Terminal,
                _ if status.is_server_error() => ErrorCategory::Transient,
                _ => ErrorCategory::Unknown,
            };
            return Err(StreamError::new(
                format!("HTTP {}: {}", status, body),
                category,
            ));
        }

        // Process the stream
        use futures::StreamExt;
        let mut parser = SseStream::new();
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| {
                StreamError::new(e.to_string(), ErrorCategory::Transient)
            })?;
            let events = parser.feed(&bytes);

            for event in events {
                // Track event ID for resumption
                if let Some(ref id) = event.id {
                    self.last_event_id = Some(id.clone());
                }

                // Accumulate text
                if event.event_type() == "content_block_delta" {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&event.data) {
                        if let Some(text) = v.get("delta")
                            .and_then(|d| d.get("text"))
                            .and_then(|t| t.as_str())
                        {
                            self.accumulated_text.push_str(text);
                            print!("{}", text);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct StreamError {
    message: String,
    category: ErrorCategory,
}

impl StreamError {
    fn new(message: String, category: ErrorCategory) -> Self {
        Self { message, category }
    }

    fn category(&self) -> &ErrorCategory {
        &self.category
    }
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

// SseStream type referenced from the parsing subchapter
struct SseStream {
    // ... fields from earlier subchapter
}

impl SseStream {
    fn new() -> Self { todo!() }
    fn feed(&mut self, _data: &[u8]) -> Vec<SseEvent> { todo!() }
}

struct SseEvent {
    id: Option<String>,
    data: String,
    event_type: Option<String>,
}

impl SseEvent {
    fn event_type(&self) -> &str {
        self.event_type.as_deref().unwrap_or("message")
    }
}
```

Note how `accumulated_text` persists across retry attempts. If the stream disconnects after 200 tokens and reconnects, the agent continues from where it left off. The user sees a brief pause, then tokens resume flowing.

## The Retry-After Header

When you receive a 429 (Too Many Requests) response, the server may include a `Retry-After` header specifying how long to wait:

```rust
fn parse_retry_after(response: &reqwest::Response) -> Option<Duration> {
    let header = response.headers().get("retry-after")?;
    let value = header.to_str().ok()?;

    // Retry-After can be seconds (integer) or an HTTP date
    if let Ok(seconds) = value.parse::<u64>() {
        Some(Duration::from_secs(seconds))
    } else {
        // HTTP date format: parse and compute duration from now
        // For simplicity, fall back to a default
        None
    }
}
```

Always respect `Retry-After`. Ignoring it will likely result in your API key being temporarily banned, which is a worse outcome than waiting.

::: wild In the Wild
Claude Code implements retry logic with exponential backoff for network errors and respects the `Retry-After` header from rate limit responses. If a stream disconnects mid-response, Claude Code preserves the accumulated text and shows a brief "Reconnecting..." indicator. OpenCode similarly retries transient failures but takes a more conservative approach, showing an error dialog and letting the user decide whether to retry rather than retrying automatically. The choice between automatic and manual retry depends on the target audience -- developers generally prefer automatic retry for transient network issues.
:::

## Key Takeaways

- **Classify errors before retrying:** transient errors (network, 5xx) should be retried; terminal errors (401, 400) should not. Retrying terminal errors wastes time and can trigger rate limits.
- **Exponential backoff with jitter** prevents thundering herds and gives overloaded servers time to recover. Equal jitter (half base + random half) is a good default.
- **Track the `Last-Event-ID`** from SSE events and send it as a header on reconnection. Even if the server does not support stream resumption today, your code is ready for when it does.
- **Preserve accumulated state** across retry attempts so the user sees a seamless continuation after a brief reconnection pause.
- **Always respect `Retry-After` headers** from rate limit responses. Ignoring them risks getting your API key temporarily banned.
