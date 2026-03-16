---
title: Error Handling Strategy
description: Design a unified error handling strategy that keeps the agent running through partial failures while providing clear diagnostics.
---

# Error Handling Strategy

> **What you'll learn:**
> - How to categorize agent errors into fatal (must stop), recoverable (can retry), and degraded (continue with reduced capability) with appropriate handling for each
> - Techniques for implementing error boundaries between components so that a tool failure does not crash the provider and a provider failure does not lose conversation state
> - How to design error messages that are useful to both end users (what went wrong, what to do) and developers (stack context, component origin, retry history)

A coding agent has more error surfaces than most CLI tools. Network calls to LLM APIs can time out. Tool execution can fail — a file might not exist, a shell command might segfault, a search query might match nothing. The model itself can produce invalid tool call arguments, hallucinate tools that do not exist, or generate malformed JSON. MCP servers can disconnect mid-conversation. The terminal might lose its connection.

If any one of these errors crashes the agent, the user loses their conversation state and their trust. A production agent must handle failures gracefully, continuing where it can and communicating clearly where it cannot.

## The Error Taxonomy

Every error in the system falls into one of three categories:

### Fatal Errors

Fatal errors mean the agent cannot continue in any meaningful way. These are rare but real:

- **Missing API key at startup** — cannot call any provider
- **Corrupted configuration** — cannot load safety rules
- **Terminal I/O failure** — cannot communicate with the user

Fatal errors produce a clear message and exit with a non-zero status code:

```rust
#[derive(Debug, thiserror::Error)]
pub enum FatalError {
    #[error("No API key configured. Set ANTHROPIC_API_KEY or add provider.api_key to config.")]
    NoApiKey,

    #[error("Configuration is invalid: {0}")]
    InvalidConfig(String),

    #[error("Terminal I/O failed: {0}")]
    TerminalFailure(#[from] std::io::Error),
}
```

### Recoverable Errors

Recoverable errors are temporary failures that can be retried:

- **Network timeout** on an API call — retry with backoff
- **Rate limiting** (HTTP 429) — wait and retry
- **Provider overloaded** (HTTP 529) — try again or fall back to another provider

```rust
#[derive(Debug)]
pub struct RecoverableError {
    pub kind: RecoverableKind,
    pub message: String,
    pub retry_after: Option<Duration>,
    pub attempts: usize,
    pub max_attempts: usize,
}

#[derive(Debug)]
pub enum RecoverableKind {
    NetworkTimeout,
    RateLimited,
    ProviderOverloaded,
    ProviderUnavailable,
}

impl RecoverableError {
    pub fn should_retry(&self) -> bool {
        self.attempts < self.max_attempts
    }

    pub fn backoff_duration(&self) -> Duration {
        match self.retry_after {
            Some(d) => d,
            None => {
                // Exponential backoff: 1s, 2s, 4s, 8s...
                let base = Duration::from_secs(1);
                base * 2u32.pow(self.attempts as u32 - 1)
            }
        }
    }
}
```

### Degraded Errors

Degraded errors mean something failed but the agent can continue with reduced capability:

- **Tool execution failure** — the tool could not complete, but other tools still work
- **MCP server disconnected** — that server's tools are unavailable, but built-in tools remain
- **Context compaction failed** — the agent can still work, just with a shorter history
- **A non-critical config section is invalid** — use defaults for that section

Degraded errors are converted into informational messages for the model:

```rust
fn handle_tool_failure(
    tool_call: &ToolCall,
    error: anyhow::Error,
) -> ToolResult {
    tracing::warn!(
        tool = %tool_call.name,
        error = %error,
        "Tool execution failed"
    );

    // Return the error as a tool result — the model will see it
    // and can adjust its approach
    ToolResult::error(
        tool_call.id.clone(),
        format!(
            "Tool '{}' failed: {}. You may want to try a different approach.",
            tool_call.name,
            error
        ),
    )
}
```

Notice that the tool failure does not crash the loop. It becomes a tool result that the model receives and reasons about. If a file read fails because the path does not exist, the model sees "File not found: src/nonexistent.rs" and can ask the user for the correct path or search for the file.

## Error Boundaries

Error boundaries prevent failures from cascading across component boundaries. Each subsystem catches its own errors and converts them to the appropriate category.

The key principle: **a component should never propagate raw errors from its internal dependencies to its callers**. Instead, it wraps them with context and classifies them.

```rust
// The provider wraps HTTP errors into provider-specific errors
impl AnthropicProvider {
    async fn stream_completion(
        &self,
        messages: &[Message],
    ) -> Result<ResponseStream, ProviderError> {
        let response = self.client
            .post(&self.api_url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::Recoverable(RecoverableError {
                        kind: RecoverableKind::NetworkTimeout,
                        message: format!("Request timed out after {:?}", self.timeout),
                        retry_after: None,
                        attempts: 0,
                        max_attempts: 3,
                    })
                } else if e.is_connect() {
                    ProviderError::Recoverable(RecoverableError {
                        kind: RecoverableKind::ProviderUnavailable,
                        message: "Could not connect to Anthropic API".into(),
                        retry_after: Some(Duration::from_secs(5)),
                        attempts: 0,
                        max_attempts: 3,
                    })
                } else {
                    ProviderError::Fatal(format!("HTTP error: {e}"))
                }
            })?;

        match response.status().as_u16() {
            200 => Ok(self.parse_stream(response)),
            429 => {
                let retry_after = response.headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .map(Duration::from_secs);

                Err(ProviderError::Recoverable(RecoverableError {
                    kind: RecoverableKind::RateLimited,
                    message: "Rate limited by Anthropic API".into(),
                    retry_after,
                    attempts: 0,
                    max_attempts: 5,
                }))
            }
            401 => Err(ProviderError::Fatal(
                "Invalid API key. Check your ANTHROPIC_API_KEY.".into()
            )),
            status => Err(ProviderError::Fatal(
                format!("Unexpected HTTP status: {status}")
            )),
        }
    }
}
```

::: python Coming from Python
In Python, you might handle this with a chain of `except` clauses: `except requests.Timeout`, `except requests.ConnectionError`, and so on. The Rust approach encodes recovery information directly in the error type — `RecoverableError` carries a retry count, backoff duration, and attempt history. This makes retry logic compositional: a generic retry wrapper can handle any `RecoverableError` without knowing whether it came from a provider, a tool, or an MCP connection.
:::

## The Retry Wrapper

A generic retry wrapper handles recoverable errors from any component:

```rust
pub async fn with_retry<F, Fut, T>(
    operation_name: &str,
    mut operation: F,
) -> Result<T, anyhow::Error>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, ProviderError>>,
{
    let mut last_error = None;

    for attempt in 1..=5 {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(ProviderError::Fatal(msg)) => {
                anyhow::bail!("{}: {}", operation_name, msg);
            }
            Err(ProviderError::Recoverable(mut err)) => {
                err.attempts = attempt;
                if !err.should_retry() {
                    anyhow::bail!(
                        "{} failed after {} attempts: {}",
                        operation_name,
                        attempt,
                        err.message
                    );
                }

                let delay = err.backoff_duration();
                tracing::warn!(
                    "{}: {} (attempt {}/{}, retrying in {:?})",
                    operation_name,
                    err.message,
                    attempt,
                    err.max_attempts,
                    delay
                );
                tokio::time::sleep(delay).await;
                last_error = Some(err);
            }
        }
    }

    anyhow::bail!(
        "{} failed after exhausting retries: {}",
        operation_name,
        last_error.map_or("unknown error".into(), |e| e.message)
    )
}
```

This wrapper is reusable across any component that can produce recoverable errors. The calling code is clean:

```rust
let stream = with_retry("LLM API call", || {
    provider.stream_completion(&messages)
}).await?;
```

## User-Facing vs. Developer-Facing Errors

Error messages serve two audiences, and they need different information:

**Users** need to know: What went wrong? Can I fix it? What should I do next?

**Developers** (including you, debugging at 2 AM) need to know: Which component failed? What was the input? What was the error chain?

The solution is to format errors with a user-facing summary and a developer-facing detail section:

```rust
fn format_error_for_user(error: &anyhow::Error) -> String {
    // The top-level message is user-facing
    let mut message = format!("Error: {}\n", error);

    // Check for actionable suggestions
    if let Some(provider_err) = error.downcast_ref::<ProviderError>() {
        match provider_err {
            ProviderError::Fatal(msg) if msg.contains("API key") => {
                message.push_str(
                    "\nTo fix this: Set the ANTHROPIC_API_KEY environment variable\n\
                     or add provider.api_key to your agent.toml config file."
                );
            }
            _ => {}
        }
    }

    // In verbose mode, include the full error chain
    if tracing::enabled!(tracing::Level::DEBUG) {
        message.push_str("\nDebug details:\n");
        for (i, cause) in error.chain().enumerate() {
            message.push_str(&format!("  {}: {}\n", i, cause));
        }
    }

    message
}
```

::: wild In the Wild
Claude Code displays tool execution errors inline in the conversation with enough context for the model to self-correct. When a shell command fails, the model sees both the exit code and stderr output, which is often enough information to diagnose and fix the problem on the next iteration. OpenCode takes a similar approach but adds structured error metadata (error code, component origin, timestamp) to its log output, making it easier to diagnose issues in automated environments.
:::

## Preserving Conversation State Through Errors

The worst outcome of an error is losing conversation state. If the agent crashes mid-conversation, the user loses their entire session. The error handling strategy must protect conversation state even when other components fail.

The rule is simple: **persist state before attempting risky operations**.

```rust
// Before making an LLM call (which might fail)
{
    let ctx = context.read().await;
    if config.context.session_persistence {
        persist_session(&ctx).await.ok(); // Best-effort persistence
    }
}

// Now make the potentially-failing call
let stream = provider.stream_completion(&messages).await?;
```

The `.ok()` on the persistence call is intentional — if saving the session fails, you still want to attempt the LLM call. Session persistence is a safety net, not a hard requirement.

## The Error Handling Hierarchy at a Glance

Here is how the complete error handling strategy maps across the system:

| Component | Error Type | Handling |
|-----------|-----------|----------|
| CLI parsing | Fatal | Exit with usage message |
| Config loading | Defaultable or Fatal | Use defaults or exit |
| Provider init | Fatal | Exit with setup instructions |
| API call | Recoverable | Retry with backoff |
| Tool execution | Degraded | Return error to model |
| Safety check | Degraded | Deny tool call, inform model |
| Context compaction | Degraded | Warn, continue with full history |
| MCP connection | Degraded | Warn, continue without MCP tools |
| Session save | Best-effort | Log warning, continue |
| Renderer | Fatal (if terminal lost) | Exit gracefully |

## Key Takeaways

- Categorize every error as fatal (must stop), recoverable (can retry with backoff), or degraded (continue with reduced capability) — this classification drives consistent handling across all components.
- Implement error boundaries at every component interface so that a tool failure does not crash the provider and a provider failure does not lose the conversation — each component wraps its internal errors with context and classification.
- Use a generic retry wrapper with exponential backoff for all recoverable errors (network timeouts, rate limits, transient API failures), carrying retry metadata directly in the error type.
- Format error messages for two audiences: users get actionable summaries ("Set your API key to fix this"), developers get error chains and component context in verbose mode.
- Protect conversation state by persisting before risky operations and treating session saves as best-effort — never let a persistence failure cascade into a conversation loss.
