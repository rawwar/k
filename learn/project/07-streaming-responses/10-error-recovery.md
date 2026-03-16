---
title: Error Recovery
description: Handle mid-stream errors gracefully including malformed events, API errors, and network interruptions without losing context.
---

# Error Recovery

> **What you'll learn:**
> - How to detect and classify errors that occur during an active stream
> - How to preserve partial content and conversation state when a stream fails
> - How to decide whether to retry, resume, or abandon a failed streaming request

Streaming introduces failure modes that do not exist with batch requests. A batch request either returns a complete response or an error. A stream can fail midway through, after you have already received and displayed hundreds of tokens. How you handle these mid-stream failures determines whether your agent feels reliable or frustrating. Let's build a comprehensive error classification and recovery system.

## Error taxonomy

Not all streaming errors are equal. Here is how to classify them:

```rust
/// Classification of streaming errors by severity and recoverability.
#[derive(Debug, Clone)]
pub enum StreamErrorKind {
    /// Network-level failures: connection reset, timeout, DNS failure.
    /// Usually recoverable by reconnecting.
    Network {
        message: String,
        is_timeout: bool,
    },

    /// HTTP-level errors: 429 (rate limit), 500 (server error), 503 (overloaded).
    /// Recoverable depending on status code.
    HttpError {
        status: u16,
        body: String,
    },

    /// SSE parsing failures: malformed event, invalid field.
    /// Usually recoverable by skipping the malformed event.
    ParseError {
        message: String,
        raw_data: String,
    },

    /// JSON deserialization failures in event data.
    /// May be recoverable if only one event is affected.
    JsonError {
        message: String,
        event_type: String,
        raw_json: String,
    },

    /// API-level errors sent as SSE error events.
    /// Recoverability depends on the error type.
    ApiError {
        error_type: String,
        error_message: String,
    },

    /// The stream ended unexpectedly without a message_stop event.
    /// Recoverable by reconnecting and re-requesting.
    UnexpectedEnd {
        tokens_received: u32,
    },
}

impl StreamErrorKind {
    /// Should we attempt to retry/reconnect after this error?
    pub fn is_recoverable(&self) -> bool {
        match self {
            StreamErrorKind::Network { .. } => true,
            StreamErrorKind::HttpError { status, .. } => {
                matches!(status, 429 | 500 | 502 | 503 | 504)
            }
            StreamErrorKind::ParseError { .. } => true,
            StreamErrorKind::JsonError { .. } => true,
            StreamErrorKind::ApiError { error_type, .. } => {
                error_type == "overloaded_error" || error_type == "rate_limit_error"
            }
            StreamErrorKind::UnexpectedEnd { .. } => true,
        }
    }

    /// How long should we wait before retrying?
    pub fn suggested_retry_delay(&self) -> std::time::Duration {
        match self {
            StreamErrorKind::HttpError { status: 429, .. } => {
                std::time::Duration::from_secs(30) // Rate limit: wait longer
            }
            StreamErrorKind::HttpError { status, .. } if *status >= 500 => {
                std::time::Duration::from_secs(5) // Server error: brief wait
            }
            StreamErrorKind::Network { is_timeout: true, .. } => {
                std::time::Duration::from_secs(2) // Timeout: short retry
            }
            StreamErrorKind::ApiError { error_type, .. }
                if error_type == "overloaded_error" =>
            {
                std::time::Duration::from_secs(15) // Overloaded: moderate wait
            }
            _ => std::time::Duration::from_secs(1), // Default: quick retry
        }
    }
}
```

::: python Coming from Python
In Python, you might handle streaming errors with nested try/except blocks:
```python
try:
    with client.messages.stream(...) as stream:
        try:
            for text in stream.text_stream:
                print(text, end="", flush=True)
        except httpx.ReadTimeout:
            print("[timeout, retrying...]")
        except json.JSONDecodeError as e:
            print(f"[parse error: {e}]")
except anthropic.APIStatusError as e:
    if e.status_code == 429:
        time.sleep(30)
```
Rust's approach with a typed error enum gives you several advantages: exhaustive pattern matching ensures you handle every error type, the `is_recoverable()` method centralizes retry logic, and `suggested_retry_delay()` keeps timing decisions close to the error classification.
:::

## Error detection during streaming

Errors can occur at different layers of the streaming pipeline. Let's build detection into each layer:

```rust
use crate::sse::{SseEvent, StreamEvent};

/// Result of attempting to process one SSE event.
pub enum EventProcessResult {
    /// Event processed successfully, here is the typed event.
    Ok(StreamEvent),
    /// Event was malformed but we can continue (skip it).
    Skippable(StreamErrorKind),
    /// Fatal error, stream must stop.
    Fatal(StreamErrorKind),
}

pub fn process_sse_event(sse_event: SseEvent) -> EventProcessResult {
    // Handle error events from the API
    if sse_event.event_type == "error" {
        return match serde_json::from_str::<StreamEvent>(&sse_event.data) {
            Ok(StreamEvent::Error { error }) => {
                let kind = StreamErrorKind::ApiError {
                    error_type: error.error_type,
                    error_message: error.message,
                };
                if kind.is_recoverable() {
                    EventProcessResult::Skippable(kind)
                } else {
                    EventProcessResult::Fatal(kind)
                }
            }
            _ => EventProcessResult::Fatal(StreamErrorKind::ApiError {
                error_type: "unknown".to_string(),
                error_message: sse_event.data.clone(),
            }),
        };
    }

    // Try to parse the event data as JSON
    match serde_json::from_str::<StreamEvent>(&sse_event.data) {
        Ok(event) => EventProcessResult::Ok(event),
        Err(e) => {
            // JSON parse failure -- usually we can skip this event
            let kind = StreamErrorKind::JsonError {
                message: e.to_string(),
                event_type: sse_event.event_type.clone(),
                raw_json: sse_event.data.clone(),
            };
            EventProcessResult::Skippable(kind)
        }
    }
}
```

## Preserving partial content

When a stream fails, the partial content you have already received and displayed is valuable. The user has seen it, and the model generated it. Your error recovery strategy must preserve this content:

```rust
/// Captures the state at the point of failure for recovery decisions.
#[derive(Debug)]
pub struct PartialStreamResult {
    /// Text content received before the error.
    pub text: String,
    /// Number of text tokens received.
    pub tokens_received: u32,
    /// Tool calls that completed before the error.
    pub completed_tool_calls: Vec<ToolCall>,
    /// Whether a tool call was in progress (and therefore lost).
    pub had_active_tool_call: bool,
    /// The error that ended the stream.
    pub error: StreamErrorKind,
}

impl PartialStreamResult {
    /// Decide whether to retry the entire request or accept partial results.
    pub fn recovery_strategy(&self) -> RecoveryStrategy {
        if !self.error.is_recoverable() {
            return RecoveryStrategy::Abandon;
        }

        if self.tokens_received == 0 {
            // Failed before generating any content -- just retry
            return RecoveryStrategy::Retry;
        }

        if self.had_active_tool_call {
            // A tool call was in progress -- the partial JSON is useless.
            // Retry the full request so the model regenerates the tool call.
            return RecoveryStrategy::Retry;
        }

        // We have usable partial text. Accept it and let the model continue
        // from where it left off.
        RecoveryStrategy::AcceptPartial
    }
}

#[derive(Debug, PartialEq)]
pub enum RecoveryStrategy {
    /// Retry the entire request from scratch.
    Retry,
    /// Accept the partial content and add it to conversation history.
    AcceptPartial,
    /// Do not retry -- the error is not recoverable.
    Abandon,
}
```

## Building the retry wrapper

Wrap the stream processing function with retry logic:

```rust
use std::time::Duration;

/// Configuration for stream retry behavior.
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
        }
    }
}

pub async fn stream_with_retry(
    client: &reqwest::Client,
    api_key: &str,
    messages: &[serde_json::Value],
    cancel_token: CancellationToken,
    config: &RetryConfig,
) -> Result<StreamOutput, StreamErrorKind> {
    let mut attempts = 0;

    loop {
        attempts += 1;

        let byte_stream = match start_streaming_request(client, api_key, messages).await {
            Ok(stream) => stream,
            Err(e) => {
                let kind = StreamErrorKind::Network {
                    message: e.to_string(),
                    is_timeout: false,
                };

                if attempts > config.max_retries || !kind.is_recoverable() {
                    return Err(kind);
                }

                let delay = calculate_delay(attempts, &kind, config);
                eprintln!(
                    "[Connection failed (attempt {}/{}), retrying in {:?}]",
                    attempts, config.max_retries, delay
                );
                tokio::time::sleep(delay).await;
                continue;
            }
        };

        match stream_with_state_machine(byte_stream, cancel_token.clone()).await {
            Ok(output) => return Ok(output),
            Err(e) => {
                // Convert the error to our error type
                let kind = StreamErrorKind::Network {
                    message: e.to_string(),
                    is_timeout: false,
                };

                let partial = PartialStreamResult {
                    text: String::new(), // Would come from the state machine
                    tokens_received: 0,
                    completed_tool_calls: vec![],
                    had_active_tool_call: false,
                    error: kind.clone(),
                };

                match partial.recovery_strategy() {
                    RecoveryStrategy::Retry if attempts <= config.max_retries => {
                        let delay = calculate_delay(attempts, &kind, config);
                        eprintln!(
                            "[Stream failed (attempt {}/{}), retrying in {:?}]",
                            attempts, config.max_retries, delay
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    RecoveryStrategy::AcceptPartial => {
                        eprintln!("[Accepting partial response]");
                        return Ok(StreamOutput {
                            text: partial.text,
                            tool_calls: partial.completed_tool_calls,
                            stop_reason: Some("partial_error".to_string()),
                        });
                    }
                    _ => return Err(kind),
                }
            }
        }
    }
}

fn calculate_delay(attempt: u32, error: &StreamErrorKind, config: &RetryConfig) -> Duration {
    let base = error.suggested_retry_delay();
    let exponential = config.base_delay * 2u32.pow(attempt.saturating_sub(1));
    let delay = base.max(exponential);
    delay.min(config.max_delay)
}
```

## Handling skippable errors

Not every error should terminate the stream. A single malformed event can often be skipped without affecting the overall response. Here is how to integrate skippable errors into the stream loop:

```rust
pub async fn stream_with_error_tolerance(
    mut byte_stream: impl futures::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
    cancel_token: CancellationToken,
    max_skip_errors: usize,
) -> Result<StreamOutput, Box<dyn std::error::Error>> {
    let mut splitter = LineSplitter::new();
    let mut parser = SseParser::new();
    let mut sm = StreamStateMachine::new();
    let mut skipped_errors = 0;

    loop {
        let chunk = tokio::select! {
            chunk = futures::StreamExt::next(&mut byte_stream) => {
                match chunk {
                    Some(Ok(bytes)) => bytes,
                    Some(Err(e)) => {
                        sm.network_error(e.to_string());
                        break;
                    }
                    None => break,
                }
            }
            _ = cancel_token.cancelled() => {
                sm.interrupt();
                break;
            }
        };

        for line in splitter.feed(&chunk) {
            let Some(sse_event) = parser.feed_line(&line) else { continue };
            if sse_event.event_type == "ping" { continue; }

            match process_sse_event(sse_event) {
                EventProcessResult::Ok(stream_event) => {
                    let action = sm.handle_event(stream_event);
                    // Handle action (render, tool call, etc.)
                    handle_stream_action(action)?;
                }
                EventProcessResult::Skippable(error) => {
                    skipped_errors += 1;
                    eprintln!("[Warning: skipped malformed event: {:?}]", error);
                    if skipped_errors > max_skip_errors {
                        eprintln!("[Too many errors, stopping stream]");
                        sm.network_error("too many parse errors".to_string());
                        break;
                    }
                }
                EventProcessResult::Fatal(error) => {
                    eprintln!("[Fatal stream error: {:?}]", error);
                    sm.network_error(format!("{:?}", error));
                    break;
                }
            }
        }

        if matches!(sm.state(), StreamState::Complete { .. } | StreamState::Errored { .. }) {
            break;
        }
    }

    println!();

    Ok(StreamOutput {
        text: sm.text().to_string(),
        tool_calls: sm.take_tool_calls(),
        stop_reason: match sm.state() {
            StreamState::Complete { stop_reason, .. } => Some(stop_reason.clone()),
            StreamState::Interrupted { .. } => Some("user_interrupt".to_string()),
            StreamState::Errored { .. } => Some("error".to_string()),
            _ => None,
        },
    })
}

fn handle_stream_action(action: StreamAction) -> Result<(), std::io::Error> {
    match action {
        StreamAction::RenderToken(text) => {
            print!("{}", text);
            std::io::Write::flush(&mut std::io::stdout())?;
        }
        StreamAction::ShowToolProgress { name } => {
            eprintln!("\n[Assembling: {}]", name);
        }
        StreamAction::ReportError(err) => {
            eprintln!("[Stream error: {:?}]", err);
        }
        _ => {}
    }
    Ok(())
}
```

::: wild In the Wild
Claude Code implements a sophisticated error recovery system that distinguishes between "API overloaded" (wait and retry), "rate limited" (wait longer and retry), "invalid request" (do not retry), and "network error" (retry with backoff). It preserves partial streaming content across retries and can resume conversations seamlessly after transient failures. OpenCode takes a simpler approach, retrying all errors up to 3 times with fixed delays.
:::

## Key Takeaways

- Classify streaming errors by type (network, HTTP, parse, API) and recoverability -- not all errors deserve the same response.
- The `is_recoverable()` and `suggested_retry_delay()` methods centralize retry decisions close to the error classification.
- Preserve partial content when a stream fails -- the user has already seen the text, and discarding it breaks the mental model.
- The recovery strategy depends on context: retry if no tokens were received, accept partial content if text was delivered, and never retry non-recoverable errors.
- Tolerate a limited number of skippable errors (malformed events) without aborting the entire stream.
