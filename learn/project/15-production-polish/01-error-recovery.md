---
title: Error Recovery
description: Implementing resilient error handling for network failures, API errors, malformed LLM responses, and interrupted operations with automatic retry and graceful degradation.
---

# Error Recovery

> **What you'll learn:**
> - How to classify errors by recoverability and implement appropriate retry strategies for each
> - Techniques for recovering from malformed LLM responses without losing conversation state
> - How to implement graceful degradation that keeps the agent usable when subsystems fail

Your coding agent talks to external APIs over the network, reads and writes files on disk, spawns child processes, and parses unpredictable LLM output. Every one of these operations can fail, and in production, they *will* fail. The difference between a frustrating tool and a reliable one is how it handles those failures. In this subchapter, you will build error recovery strategies that keep your agent running through the chaos of real-world conditions.

## Classifying Errors by Recoverability

Not all errors deserve the same treatment. A typo in the config file requires the user to fix something. A network timeout might resolve itself in two seconds. You need a classification system that drives your recovery strategy.

Let's define an error taxonomy for the agent:

```rust
use std::fmt;
use std::time::Duration;

/// How an error should be handled by the recovery system.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrorRecovery {
    /// Retry after a delay -- transient failures like network timeouts
    Retry { max_attempts: u32, base_delay: Duration },
    /// Skip this operation and continue -- non-critical failures
    Skip,
    /// Ask the user what to do -- ambiguous situations
    Prompt,
    /// Stop execution -- unrecoverable errors
    Fatal,
}

/// Categorized agent errors with recovery metadata.
#[derive(Debug)]
pub enum AgentError {
    Network { source: reqwest::Error, url: String },
    ApiRateLimit { retry_after: Duration },
    ApiAuth { message: String },
    MalformedResponse { raw: String, parse_error: String },
    ToolExecution { tool: String, source: Box<dyn std::error::Error + Send + Sync> },
    FileSystem { path: String, source: std::io::Error },
    ConfigInvalid { field: String, message: String },
}

impl AgentError {
    /// Determine the appropriate recovery strategy for this error.
    pub fn recovery(&self) -> ErrorRecovery {
        match self {
            AgentError::Network { .. } => ErrorRecovery::Retry {
                max_attempts: 3,
                base_delay: Duration::from_secs(1),
            },
            AgentError::ApiRateLimit { retry_after } => ErrorRecovery::Retry {
                max_attempts: 5,
                base_delay: *retry_after,
            },
            AgentError::ApiAuth { .. } => ErrorRecovery::Fatal,
            AgentError::MalformedResponse { .. } => ErrorRecovery::Retry {
                max_attempts: 2,
                base_delay: Duration::from_millis(500),
            },
            AgentError::ToolExecution { .. } => ErrorRecovery::Skip,
            AgentError::FileSystem { source, .. } => match source.kind() {
                std::io::ErrorKind::PermissionDenied => ErrorRecovery::Prompt,
                std::io::ErrorKind::NotFound => ErrorRecovery::Skip,
                _ => ErrorRecovery::Retry {
                    max_attempts: 2,
                    base_delay: Duration::from_millis(200),
                },
            },
            AgentError::ConfigInvalid { .. } => ErrorRecovery::Fatal,
        }
    }
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::Network { url, .. } => write!(f, "Network error connecting to {url}"),
            AgentError::ApiRateLimit { retry_after } => {
                write!(f, "Rate limited, retry after {}s", retry_after.as_secs())
            }
            AgentError::ApiAuth { message } => write!(f, "Authentication failed: {message}"),
            AgentError::MalformedResponse { parse_error, .. } => {
                write!(f, "Failed to parse LLM response: {parse_error}")
            }
            AgentError::ToolExecution { tool, source } => {
                write!(f, "Tool '{tool}' failed: {source}")
            }
            AgentError::FileSystem { path, source } => {
                write!(f, "File system error at {path}: {source}")
            }
            AgentError::ConfigInvalid { field, message } => {
                write!(f, "Invalid config '{field}': {message}")
            }
        }
    }
}

impl std::error::Error for AgentError {}
```

The key insight here is that `recovery()` is a method on the error itself. Each error knows how it should be handled. This keeps your recovery logic centralized rather than scattered across call sites.

::: python Coming from Python
In Python, you typically catch exceptions by type with `try`/`except` blocks and decide recovery inline. Rust takes a different approach -- errors are values that you return and match on. This `AgentError` enum with a `recovery()` method is analogous to having a base exception class with a `get_recovery_strategy()` method, but the compiler guarantees you handle every variant. You cannot accidentally forget to handle `ApiAuth` the way you might forget to catch `AuthenticationError` in Python.
:::

## Implementing Retry with Exponential Backoff

The most common recovery strategy is retry with exponential backoff. This pattern is essential for network calls and API interactions where transient failures are expected.

```rust
use std::time::Duration;
use tokio::time::sleep;

/// Retry a fallible async operation with exponential backoff.
///
/// Returns the successful result or the last error after all attempts are exhausted.
pub async fn retry_with_backoff<F, Fut, T, E>(
    max_attempts: u32,
    base_delay: Duration,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut last_error = None;

    for attempt in 0..max_attempts {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(e) => {
                let delay = base_delay * 2u32.pow(attempt);
                eprintln!(
                    "Attempt {}/{} failed: {}. Retrying in {}ms...",
                    attempt + 1,
                    max_attempts,
                    e,
                    delay.as_millis()
                );
                last_error = Some(e);

                if attempt + 1 < max_attempts {
                    sleep(delay).await;
                }
            }
        }
    }

    Err(last_error.unwrap())
}
```

Here is how you use it in the agent's API call path:

```rust
use std::time::Duration;

async fn call_llm_with_retry(
    client: &reqwest::Client,
    api_url: &str,
    body: &serde_json::Value,
) -> Result<String, AgentError> {
    retry_with_backoff(3, Duration::from_secs(1), || async {
        let response = client
            .post(api_url)
            .json(body)
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| AgentError::Network {
                source: e,
                url: api_url.to_string(),
            })?;

        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(5);
            return Err(AgentError::ApiRateLimit {
                retry_after: Duration::from_secs(retry_after),
            });
        }

        let text = response.text().await.map_err(|e| AgentError::Network {
            source: e,
            url: api_url.to_string(),
        })?;

        Ok(text)
    })
    .await
}
```

The backoff delays are 1s, 2s, 4s for the three attempts. This avoids hammering a struggling server while still recovering quickly from brief network glitches.

## Recovering from Malformed LLM Responses

LLMs are unpredictable. They sometimes return incomplete JSON, forget to close a tool call block, or produce output that does not match the expected schema. Your agent needs to handle all of these gracefully.

```rust
use serde_json::Value;

/// Attempt to extract a valid tool call from a potentially malformed LLM response.
/// Falls back to treating the response as plain text if parsing fails.
pub fn recover_tool_call(raw_response: &str) -> RecoveredResponse {
    // First, try standard JSON parsing
    if let Ok(parsed) = serde_json::from_str::<Value>(raw_response) {
        if let Some(tool_calls) = extract_tool_calls(&parsed) {
            return RecoveredResponse::ToolCalls(tool_calls);
        }
        if let Some(text) = parsed.get("content").and_then(|v| v.as_str()) {
            return RecoveredResponse::Text(text.to_string());
        }
    }

    // Try to find JSON embedded in text (LLM sometimes wraps JSON in markdown)
    if let Some(json_str) = extract_json_from_markdown(raw_response) {
        if let Ok(parsed) = serde_json::from_str::<Value>(&json_str) {
            if let Some(tool_calls) = extract_tool_calls(&parsed) {
                return RecoveredResponse::ToolCalls(tool_calls);
            }
        }
    }

    // Try to repair truncated JSON by closing open brackets
    let repaired = repair_truncated_json(raw_response);
    if let Ok(parsed) = serde_json::from_str::<Value>(&repaired) {
        if let Some(tool_calls) = extract_tool_calls(&parsed) {
            return RecoveredResponse::ToolCalls(tool_calls);
        }
    }

    // Fall back to treating the whole thing as text
    RecoveredResponse::Text(raw_response.to_string())
}

pub enum RecoveredResponse {
    ToolCalls(Vec<ToolCall>),
    Text(String),
}

pub struct ToolCall {
    pub name: String,
    pub arguments: Value,
}

/// Extract JSON blocks from markdown-fenced code.
fn extract_json_from_markdown(text: &str) -> Option<String> {
    let start = text.find("```json").map(|i| i + 7)
        .or_else(|| text.find("```").map(|i| i + 3))?;
    let rest = &text[start..];
    let end = rest.find("```")?;
    Some(rest[..end].trim().to_string())
}

/// Attempt to fix truncated JSON by closing unclosed brackets and braces.
fn repair_truncated_json(text: &str) -> String {
    let mut result = text.to_string();
    let mut open_braces = 0i32;
    let mut open_brackets = 0i32;
    let mut in_string = false;
    let mut prev_char = '\0';

    for ch in text.chars() {
        if ch == '"' && prev_char != '\\' {
            in_string = !in_string;
        }
        if !in_string {
            match ch {
                '{' => open_braces += 1,
                '}' => open_braces -= 1,
                '[' => open_brackets += 1,
                ']' => open_brackets -= 1,
                _ => {}
            }
        }
        prev_char = ch;
    }

    // Close any unclosed strings
    if in_string {
        result.push('"');
    }

    // Close unclosed brackets and braces
    for _ in 0..open_brackets {
        result.push(']');
    }
    for _ in 0..open_braces {
        result.push('}');
    }

    result
}

fn extract_tool_calls(value: &Value) -> Option<Vec<ToolCall>> {
    // Implementation depends on your API response format
    let calls = value.get("tool_calls")?.as_array()?;
    let result: Vec<ToolCall> = calls
        .iter()
        .filter_map(|call| {
            Some(ToolCall {
                name: call.get("name")?.as_str()?.to_string(),
                arguments: call.get("arguments")?.clone(),
            })
        })
        .collect();

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}
```

This layered recovery approach tries progressively more aggressive parsing strategies. The key principle is: never throw away a response if you can extract something useful from it.

## Graceful Degradation in the Agent Loop

When a tool fails, the agent should not crash. It should report the failure back to the LLM and let it decide what to do next. This is graceful degradation at the agent loop level.

```rust
pub async fn run_agent_loop(agent: &mut Agent) -> Result<(), AgentError> {
    loop {
        let response = match call_llm_with_retry(
            &agent.client,
            &agent.api_url,
            &agent.build_request_body(),
        )
        .await
        {
            Ok(resp) => resp,
            Err(e) => match e.recovery() {
                ErrorRecovery::Fatal => return Err(e),
                ErrorRecovery::Prompt => {
                    eprintln!("Error: {e}");
                    eprintln!("Would you like to retry? (y/n)");
                    // Read user input and either retry or exit
                    continue;
                }
                _ => {
                    // Retries already exhausted by call_llm_with_retry
                    agent.add_system_message(format!(
                        "The previous API call failed after retries: {e}. \
                         Please acknowledge and continue."
                    ));
                    continue;
                }
            },
        };

        let recovered = recover_tool_call(&response);
        match recovered {
            RecoveredResponse::Text(text) => {
                agent.display_response(&text);
                if agent.is_done(&text) {
                    break;
                }
            }
            RecoveredResponse::ToolCalls(calls) => {
                for call in calls {
                    let result = match agent.execute_tool(&call).await {
                        Ok(output) => output,
                        Err(e) => {
                            // Report tool failure to the LLM instead of crashing
                            format!(
                                "Tool '{}' failed with error: {}. \
                                 Please try an alternative approach.",
                                call.name, e
                            )
                        }
                    };
                    agent.add_tool_result(&call.name, &result);
                }
            }
        }
    }

    Ok(())
}
```

::: wild In the Wild
Claude Code wraps every tool execution in error handling that feeds failures back into the conversation context. If a shell command fails, the LLM sees the error output and can decide to try a different command. OpenCode follows a similar pattern, treating tool errors as informational messages rather than fatal conditions. This feedback loop is what makes agents feel resilient -- they adapt to failures the way a human developer would.
:::

## Circuit Breaker Pattern

If a service is consistently failing, retrying every request wastes time and resources. A circuit breaker tracks failures and stops making requests once a threshold is reached, periodically testing whether the service has recovered.

```rust
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct CircuitBreaker {
    failure_count: AtomicU32,
    last_failure: AtomicU64,
    threshold: u32,
    reset_timeout_secs: u64,
}

#[derive(Debug, PartialEq)]
pub enum CircuitState {
    Closed,    // Normal operation
    Open,      // Rejecting requests
    HalfOpen,  // Testing if service recovered
}

impl CircuitBreaker {
    pub fn new(threshold: u32, reset_timeout_secs: u64) -> Self {
        Self {
            failure_count: AtomicU32::new(0),
            last_failure: AtomicU64::new(0),
            threshold,
            reset_timeout_secs,
        }
    }

    pub fn state(&self) -> CircuitState {
        let failures = self.failure_count.load(Ordering::Relaxed);
        if failures < self.threshold {
            return CircuitState::Closed;
        }

        let last = self.last_failure.load(Ordering::Relaxed);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if now - last > self.reset_timeout_secs {
            CircuitState::HalfOpen
        } else {
            CircuitState::Open
        }
    }

    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
    }

    pub fn record_failure(&self) {
        self.failure_count.fetch_add(1, Ordering::Relaxed);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_failure.store(now, Ordering::Relaxed);
    }

    pub fn allow_request(&self) -> bool {
        match self.state() {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => true,   // Allow one test request
            CircuitState::Open => false,
        }
    }
}
```

Use the circuit breaker in your API call path to fail fast when the provider is down, rather than making the user wait through multiple retry cycles for every message.

## Key Takeaways

- Classify errors by recoverability (retry, skip, prompt, fatal) and let each error type declare its own recovery strategy through a `recovery()` method on your error enum.
- Use exponential backoff for transient failures like network timeouts and rate limits -- start with a 1-second base delay and double it on each attempt.
- Recover from malformed LLM responses by trying progressively more aggressive parsing strategies (standard parse, markdown extraction, JSON repair) before falling back to plain text.
- Report tool failures back to the LLM as informational messages instead of crashing, letting the model adapt and try alternative approaches.
- Implement the circuit breaker pattern for external services to fail fast when a provider is consistently down, avoiding unnecessary retry delays.
