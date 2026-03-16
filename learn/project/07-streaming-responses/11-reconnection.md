---
title: Reconnection
description: Implement automatic reconnection with exponential backoff to recover from dropped connections during streaming.
---

# Reconnection

> **What you'll learn:**
> - How to detect connection drops versus intentional stream termination
> - How to implement exponential backoff with jitter for reconnection attempts
> - How to resume a conversation from the last successful point after reconnection

Network connections drop. WiFi flickers. Corporate proxies timeout idle connections. Your agent must handle these interruptions without losing the user's conversation or requiring them to start over. In this subchapter you will build a reconnection layer that automatically retries failed connections with exponential backoff and resumes the conversation from where it left off.

## Detecting connection drops

A dropped connection manifests differently depending on where in the pipeline it occurs:

1. **During connection setup** -- `reqwest` returns an error from `.send().await`. The request never reached the server.
2. **During streaming** -- the `bytes_stream()` yields an `Err`, or the stream ends (`None`) without a `message_stop` event.
3. **Silent timeout** -- the server stops sending data but the TCP connection stays open. No error is raised until you hit a read timeout.

Let's build detection for each case:

```rust
use std::time::{Duration, Instant};

/// Monitors a stream for signs of connection problems.
pub struct ConnectionMonitor {
    /// When we last received any data.
    last_data_at: Instant,
    /// How long to wait before considering the connection stale.
    stale_timeout: Duration,
    /// Whether we received a proper message_stop event.
    received_message_stop: bool,
    /// Whether the stream ended (no more chunks).
    stream_ended: bool,
}

impl ConnectionMonitor {
    pub fn new(stale_timeout: Duration) -> Self {
        Self {
            last_data_at: Instant::now(),
            stale_timeout,
            received_message_stop: false,
            stream_ended: false,
        }
    }

    /// Call this whenever any data arrives from the stream.
    pub fn record_data(&mut self) {
        self.last_data_at = Instant::now();
    }

    /// Call when a message_stop event is received.
    pub fn record_message_stop(&mut self) {
        self.received_message_stop = true;
    }

    /// Call when the byte stream returns None (end of stream).
    pub fn record_stream_end(&mut self) {
        self.stream_ended = true;
    }

    /// Check if the connection appears stale (no data for too long).
    pub fn is_stale(&self) -> bool {
        self.last_data_at.elapsed() > self.stale_timeout
    }

    /// Determine how the stream ended.
    pub fn termination_kind(&self) -> TerminationKind {
        if self.received_message_stop {
            TerminationKind::GracefulComplete
        } else if self.stream_ended {
            TerminationKind::UnexpectedEnd
        } else if self.is_stale() {
            TerminationKind::StaleTimeout
        } else {
            TerminationKind::StillActive
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum TerminationKind {
    /// Stream completed normally with message_stop.
    GracefulComplete,
    /// Stream ended without message_stop (connection dropped).
    UnexpectedEnd,
    /// No data received for longer than the stale timeout.
    StaleTimeout,
    /// Stream is still active.
    StillActive,
}
```

The key distinction is between `GracefulComplete` (normal) and the other two (abnormal). Only abnormal terminations should trigger reconnection.

## Exponential backoff with jitter

When retrying, you do not want to hit the server immediately. If the server is overloaded, a flood of instant retries makes things worse. Exponential backoff increases the delay between retries. Jitter adds randomness so that multiple clients do not all retry at the same time:

```rust
use rand::Rng;
use std::time::Duration;

/// Calculates retry delays with exponential backoff and jitter.
pub struct BackoffStrategy {
    /// The base delay before the first retry.
    base_delay: Duration,
    /// Maximum delay cap.
    max_delay: Duration,
    /// Maximum number of retry attempts.
    max_retries: u32,
    /// Current attempt number (0-based).
    attempt: u32,
}

impl BackoffStrategy {
    pub fn new(base_delay: Duration, max_delay: Duration, max_retries: u32) -> Self {
        Self {
            base_delay,
            max_delay,
            max_retries,
            attempt: 0,
        }
    }

    /// Get the delay for the next retry, or None if max retries exceeded.
    pub fn next_delay(&mut self) -> Option<Duration> {
        if self.attempt >= self.max_retries {
            return None;
        }

        // Calculate exponential delay: base * 2^attempt
        let exponential_ms =
            self.base_delay.as_millis() as u64 * 2u64.pow(self.attempt);
        let capped_ms = exponential_ms.min(self.max_delay.as_millis() as u64);

        // Add jitter: random value between 0 and the calculated delay
        let mut rng = rand::thread_rng();
        let jitter_ms = rng.gen_range(0..=capped_ms / 2);
        let final_ms = capped_ms + jitter_ms;

        self.attempt += 1;

        Some(Duration::from_millis(final_ms))
    }

    /// Reset the attempt counter (call after a successful connection).
    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    /// Get the current attempt number.
    pub fn attempts(&self) -> u32 {
        self.attempt
    }
}
```

The typical progression looks like:

| Attempt | Base delay | With jitter (example) |
|---------|------------|----------------------|
| 0       | 1s         | 1.0 - 1.5s          |
| 1       | 2s         | 2.0 - 3.0s          |
| 2       | 4s         | 4.0 - 6.0s          |
| 3       | 8s         | 8.0 - 12.0s         |
| 4       | 16s        | 16.0 - 24.0s        |

::: python Coming from Python
Python's `tenacity` library provides retry with backoff out of the box:
```python
from tenacity import retry, wait_exponential, stop_after_attempt

@retry(wait=wait_exponential(multiplier=1, max=60), stop=stop_after_attempt(5))
async def call_api_with_retry():
    async with client.messages.stream(...) as stream:
        async for text in stream.text_stream:
            yield text
```
In Rust, you build the backoff logic explicitly, which gives you finer control. You can adjust the delay based on the specific error type (longer for rate limits, shorter for network blips), something that is harder to express with decorator-based retry libraries.
:::

## The reconnection loop

Now let's build the full reconnection wrapper. It manages the lifecycle of connect-stream-reconnect:

```rust
use crate::sse::StreamEvent;

/// Outcome of a single stream attempt.
pub enum StreamAttemptResult {
    /// Stream completed successfully.
    Complete(StreamOutput),
    /// Stream failed but is recoverable.
    Recoverable {
        partial_text: String,
        error: String,
    },
    /// Stream failed and should not be retried.
    Fatal(String),
    /// User interrupted with Ctrl+C.
    Interrupted(StreamOutput),
}

pub async fn stream_with_reconnection(
    client: &reqwest::Client,
    api_key: &str,
    messages: &[serde_json::Value],
    cancel_token: CancellationToken,
) -> Result<StreamOutput, Box<dyn std::error::Error>> {
    let mut backoff = BackoffStrategy::new(
        Duration::from_secs(1),
        Duration::from_secs(60),
        5,
    );

    let mut accumulated_text = String::new();

    loop {
        // Check cancellation before attempting connection
        if cancel_token.is_cancelled() {
            return Ok(StreamOutput {
                text: accumulated_text,
                tool_calls: vec![],
                stop_reason: Some("user_interrupt".to_string()),
            });
        }

        let attempt_result = single_stream_attempt(
            client,
            api_key,
            messages,
            cancel_token.clone(),
        )
        .await;

        match attempt_result {
            StreamAttemptResult::Complete(output) => {
                return Ok(output);
            }

            StreamAttemptResult::Interrupted(output) => {
                return Ok(output);
            }

            StreamAttemptResult::Fatal(error) => {
                return Err(format!("Fatal streaming error: {}", error).into());
            }

            StreamAttemptResult::Recoverable { partial_text, error } => {
                accumulated_text.push_str(&partial_text);

                match backoff.next_delay() {
                    Some(delay) => {
                        eprintln!(
                            "\n[Connection lost: {}. Reconnecting in {:?} (attempt {})...]",
                            error,
                            delay,
                            backoff.attempts()
                        );

                        // Wait for the backoff delay, but allow cancellation
                        tokio::select! {
                            _ = tokio::time::sleep(delay) => {}
                            _ = cancel_token.cancelled() => {
                                return Ok(StreamOutput {
                                    text: accumulated_text,
                                    tool_calls: vec![],
                                    stop_reason: Some("user_interrupt".to_string()),
                                });
                            }
                        }
                    }
                    None => {
                        eprintln!(
                            "\n[Max reconnection attempts ({}) reached. Accepting partial response.]",
                            backoff.attempts()
                        );
                        return Ok(StreamOutput {
                            text: accumulated_text,
                            tool_calls: vec![],
                            stop_reason: Some("max_retries_exceeded".to_string()),
                        });
                    }
                }
            }
        }
    }
}

async fn single_stream_attempt(
    client: &reqwest::Client,
    api_key: &str,
    messages: &[serde_json::Value],
    cancel_token: CancellationToken,
) -> StreamAttemptResult {
    // Attempt to connect
    let byte_stream = match start_streaming_request(client, api_key, messages).await {
        Ok(stream) => stream,
        Err(e) => {
            return StreamAttemptResult::Recoverable {
                partial_text: String::new(),
                error: format!("Connection failed: {}", e),
            };
        }
    };

    // Process the stream
    match stream_with_state_machine(byte_stream, cancel_token.clone()).await {
        Ok(output) => {
            if output.stop_reason.as_deref() == Some("user_interrupt") {
                StreamAttemptResult::Interrupted(output)
            } else {
                StreamAttemptResult::Complete(output)
            }
        }
        Err(e) => StreamAttemptResult::Recoverable {
            partial_text: String::new(),
            error: e.to_string(),
        },
    }
}
```

## Resuming after reconnection

When you reconnect after a dropped connection, you have a choice: re-send the original request (potentially getting duplicate content) or modify the request to continue from where you left off.

The simplest approach -- and the one most production agents use -- is to re-send the original request. The model will regenerate a complete response. The partial text you received from the first attempt is discarded from the model's perspective, but you have already displayed it to the user.

A more sophisticated approach adds the partial response to the conversation and asks the model to continue:

```rust
/// Build messages that include partial content for continuation.
pub fn build_continuation_messages(
    original_messages: &[serde_json::Value],
    partial_text: &str,
) -> Vec<serde_json::Value> {
    let mut messages = original_messages.to_vec();

    if !partial_text.is_empty() {
        // Add the partial response as an assistant message
        messages.push(serde_json::json!({
            "role": "assistant",
            "content": [
                {
                    "type": "text",
                    "text": partial_text
                }
            ]
        }));

        // Ask the model to continue from where it left off
        messages.push(serde_json::json!({
            "role": "user",
            "content": "Please continue from where you left off. Your previous response was interrupted."
        }));
    }

    messages
}
```

This approach has a tradeoff: the continuation will be a separate response, so the model might repeat context or change direction slightly. For short interruptions, re-sending the original request is usually better. For long responses where many tokens were already generated, continuation saves time and API costs.

::: wild In the Wild
Claude Code uses a simple retry-from-scratch approach for most connection failures. When a stream drops, it re-sends the original request and displays the new response from the beginning. The previous partial text is not preserved on the display -- the screen is cleared and the response starts fresh. This avoids the complexity of merging partial responses and gives the model a clean slate. OpenCode takes a similar approach, resetting the streaming state on reconnection and replaying from the start.
:::

## Key Takeaways

- Detect connection drops by distinguishing between graceful completion (`message_stop` received), unexpected end (stream closed without `message_stop`), and stale connections (no data received within a timeout).
- Exponential backoff with jitter prevents retry storms: each successive retry waits exponentially longer, with randomness to avoid synchronized retries from multiple clients.
- The reconnection loop wraps single stream attempts, accumulating partial text across retries and respecting the cancellation token during backoff delays.
- For most use cases, re-sending the original request on reconnection is simpler and more reliable than trying to continue a partial response.
- Always allow Ctrl+C to interrupt the backoff wait, not just the active stream.
