---
title: Fallback Chains
description: Building automatic fallback chains that retry failed requests with alternative providers or models, handling rate limits, outages, and transient errors gracefully.
---

# Fallback Chains

> **What you'll learn:**
> - How to configure ordered fallback chains that try alternative providers on failure
> - Which error types should trigger fallback versus immediate failure reporting
> - Techniques for implementing exponential backoff and circuit breakers across provider chains

When your primary provider returns a 429 rate limit or a 503 outage, the user should not see an error. The agent should transparently retry with an alternative provider, just like how a load balancer routes around unhealthy servers. Fallback chains make this automatic. You configure an ordered list of providers, and the chain tries each one until a request succeeds or all options are exhausted.

## The Fallback Chain

A fallback chain is an ordered list of providers with retry logic:

```rust
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use async_trait::async_trait;

use crate::provider::{Provider, ProviderError, StreamHandle};
use crate::provider::types::*;

/// An entry in the fallback chain with its own retry policy.
#[derive(Clone)]
pub struct FallbackEntry {
    pub provider: Arc<dyn Provider>,
    pub max_retries: u32,
    pub base_delay_ms: u64,
}

/// A chain of providers that automatically falls back on failure.
pub struct FallbackChain {
    entries: Vec<FallbackEntry>,
    /// Track which providers are currently in a degraded state.
    circuit_breakers: Vec<CircuitBreaker>,
}

impl FallbackChain {
    pub fn new(entries: Vec<FallbackEntry>) -> Self {
        let circuit_breakers = entries.iter()
            .map(|_| CircuitBreaker::new())
            .collect();

        Self {
            entries,
            circuit_breakers,
        }
    }

    pub fn builder() -> FallbackChainBuilder {
        FallbackChainBuilder { entries: Vec::new() }
    }
}

pub struct FallbackChainBuilder {
    entries: Vec<FallbackEntry>,
}

impl FallbackChainBuilder {
    /// Add a provider with default retry settings.
    pub fn add(mut self, provider: Arc<dyn Provider>) -> Self {
        self.entries.push(FallbackEntry {
            provider,
            max_retries: 2,
            base_delay_ms: 500,
        });
        self
    }

    /// Add a provider with custom retry settings.
    pub fn add_with_retries(
        mut self,
        provider: Arc<dyn Provider>,
        max_retries: u32,
        base_delay_ms: u64,
    ) -> Self {
        self.entries.push(FallbackEntry {
            provider,
            max_retries,
            base_delay_ms,
        });
        self
    }

    pub fn build(self) -> FallbackChain {
        FallbackChain::new(self.entries)
    }
}
```

The builder pattern lets you construct chains fluently:

```rust
let chain = FallbackChain::builder()
    .add(Arc::new(AnthropicProvider::new(
        anthropic_key.clone(), "claude-sonnet-4-20250514".into(),
    )))
    .add(Arc::new(OpenAIProvider::new(
        openai_key.clone(), "gpt-4o".into(),
    )))
    .add(Arc::new(OllamaProvider::new("llama3:latest".into())))
    .build();
```

This chain tries Anthropic first, then OpenAI if Anthropic fails, then Ollama as a last resort.

## Implementing the Chain as a Provider

The fallback chain itself implements the `Provider` trait, which means the agent does not know it is using a chain. It sees a single provider and sends messages to it. The fallback logic is completely transparent:

```rust
#[async_trait]
impl Provider for FallbackChain {
    fn name(&self) -> &str {
        "fallback_chain"
    }

    fn model(&self) -> &str {
        // Return the primary provider's model
        self.entries.first()
            .map(|e| e.provider.model())
            .unwrap_or("unknown")
    }

    async fn send_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
        max_tokens: u32,
    ) -> Result<ProviderResponse, ProviderError> {
        let mut last_error = None;

        for (index, entry) in self.entries.iter().enumerate() {
            // Skip providers whose circuit breaker is open
            if self.circuit_breakers[index].is_open() {
                continue;
            }

            match self.try_provider(entry, system, messages, tools, max_tokens).await {
                Ok(response) => {
                    self.circuit_breakers[index].record_success();
                    return Ok(response);
                }
                Err(e) => {
                    self.circuit_breakers[index].record_failure();

                    // Only fall back for errors that warrant it
                    if e.should_fallback() {
                        eprintln!(
                            "[fallback] {}:{} failed ({}), trying next provider",
                            entry.provider.name(),
                            entry.provider.model(),
                            e
                        );
                        last_error = Some(e);
                        continue;
                    } else {
                        // Non-fallback errors (auth, bad request) fail immediately
                        return Err(e);
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| ProviderError::Api {
            status: 503,
            message: "All providers in fallback chain exhausted".to_string(),
            retryable: false,
        }))
    }

    async fn stream_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
        max_tokens: u32,
    ) -> Result<StreamHandle, ProviderError> {
        let mut last_error = None;

        for (index, entry) in self.entries.iter().enumerate() {
            if self.circuit_breakers[index].is_open() {
                continue;
            }

            match entry.provider.stream_message(
                system, messages, tools, max_tokens
            ).await {
                Ok(handle) => {
                    self.circuit_breakers[index].record_success();
                    return Ok(handle);
                }
                Err(e) => {
                    self.circuit_breakers[index].record_failure();
                    if e.should_fallback() {
                        last_error = Some(e);
                        continue;
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| ProviderError::Api {
            status: 503,
            message: "All providers in fallback chain exhausted".to_string(),
            retryable: false,
        }))
    }
}
```

The `should_fallback()` method on `ProviderError` is doing the critical work here. Rate limits, server errors, and timeouts trigger fallback. Authentication errors and serialization failures do not -- those indicate a configuration problem, not a transient issue, and falling back would just waste time.

::: python Coming from Python
Python's exception hierarchy lets you catch specific exception types:
```python
try:
    response = primary_provider.send(messages)
except RateLimitError:
    response = backup_provider.send(messages)
except AuthenticationError:
    raise  # Don't fallback, this is a config problem
```
Rust's enum-based errors serve the same purpose, but the `should_fallback()` method centralizes the decision logic. This is important because the fallback chain does not have to know about every error variant -- it just asks the error whether fallback is appropriate.
:::

## Retry with Exponential Backoff

Before falling back to a different provider, it is worth retrying the current one for transient errors. Exponential backoff prevents hammering a struggling server:

```rust
impl FallbackChain {
    async fn try_provider(
        &self,
        entry: &FallbackEntry,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
        max_tokens: u32,
    ) -> Result<ProviderResponse, ProviderError> {
        let mut last_error = None;

        for attempt in 0..=entry.max_retries {
            if attempt > 0 {
                let delay = calculate_backoff(
                    entry.base_delay_ms,
                    attempt,
                    &last_error,
                );
                sleep(Duration::from_millis(delay)).await;
            }

            match entry.provider.send_message(
                system, messages, tools, max_tokens
            ).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    if e.is_retryable() && attempt < entry.max_retries {
                        last_error = Some(e);
                        continue;
                    }
                    return Err(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| ProviderError::Api {
            status: 500,
            message: "Retry loop exited without result".to_string(),
            retryable: false,
        }))
    }
}

/// Calculate backoff delay with jitter.
fn calculate_backoff(
    base_ms: u64,
    attempt: u32,
    last_error: &Option<ProviderError>,
) -> u64 {
    // If the server told us to wait, respect that
    if let Some(ProviderError::RateLimited { retry_after_ms: Some(ms) }) = last_error {
        return *ms;
    }

    // Exponential backoff: base * 2^attempt
    let exponential = base_ms.saturating_mul(1 << attempt.min(6));

    // Add jitter: random value between 0 and exponential/2
    let jitter = (exponential / 2).min(1000);

    exponential + jitter
}
```

The backoff respects the `Retry-After` header when the provider includes one. Otherwise, it uses exponential backoff with jitter to spread out retries. The `min(6)` cap prevents the delay from growing beyond about 32 seconds per retry.

## Circuit Breakers

If a provider has been consistently failing, you should stop sending it requests for a while rather than wasting time on retries that will not work. A circuit breaker tracks failure rates and temporarily removes a provider from the chain:

```rust
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

pub struct CircuitBreaker {
    failures: AtomicU32,
    successes: AtomicU32,
    /// Timestamp (epoch ms) when the circuit was opened. 0 means closed.
    opened_at: AtomicU64,
    /// How long to keep the circuit open before trying again.
    cooldown_ms: u64,
    /// Number of failures before opening the circuit.
    failure_threshold: u32,
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self {
            failures: AtomicU32::new(0),
            successes: AtomicU32::new(0),
            opened_at: AtomicU64::new(0),
            cooldown_ms: 30_000, // 30 seconds
            failure_threshold: 3,
        }
    }

    /// Returns true if the circuit is open (provider should be skipped).
    pub fn is_open(&self) -> bool {
        let opened = self.opened_at.load(Ordering::Relaxed);
        if opened == 0 {
            return false;
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // If cooldown has elapsed, move to half-open state (allow one try)
        if now - opened > self.cooldown_ms {
            self.opened_at.store(0, Ordering::Relaxed);
            return false;
        }

        true
    }

    pub fn record_failure(&self) {
        let failures = self.failures.fetch_add(1, Ordering::Relaxed) + 1;
        if failures >= self.failure_threshold {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            self.opened_at.store(now, Ordering::Relaxed);
        }
    }

    pub fn record_success(&self) {
        self.failures.store(0, Ordering::Relaxed);
        self.successes.fetch_add(1, Ordering::Relaxed);
        self.opened_at.store(0, Ordering::Relaxed);
    }
}
```

The circuit breaker has three states:
- **Closed**: Normal operation, requests flow through.
- **Open**: Too many failures, skip this provider. Transitions back to half-open after the cooldown period.
- **Half-open**: One request is allowed through. If it succeeds, the circuit closes. If it fails, the circuit opens again.

Using atomics instead of a mutex keeps the overhead minimal -- circuit breaker checks happen on every request and should not block.

::: wild In the Wild
Production coding agents typically implement some form of provider fallback. OpenCode supports configuring multiple providers and will use the first available one. The circuit breaker pattern is common in microservice architectures and applies directly to LLM provider chains, where a provider might be degraded for minutes at a time during an incident.
:::

## Full Chain Example

Putting it all together with a complete usage example:

```rust
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chain = FallbackChain::builder()
        .add_with_retries(
            Arc::new(AnthropicProvider::new(
                std::env::var("ANTHROPIC_API_KEY")?,
                "claude-sonnet-4-20250514".into(),
            )),
            3,   // max retries
            500, // base delay ms
        )
        .add_with_retries(
            Arc::new(OpenAIProvider::new(
                std::env::var("OPENAI_API_KEY")?,
                "gpt-4o".into(),
            )),
            2,
            1000,
        )
        .add(Arc::new(OllamaProvider::new("llama3:latest".into())))
        .build();

    // Use the chain exactly like a single provider
    let response = chain.send_message(
        "You are a helpful coding assistant.",
        &[Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: "Explain Rust's ownership model.".into(),
            }],
        }],
        &[],
        4096,
    ).await?;

    println!("Response from: {}", response.model);
    Ok(())
}
```

The chain is used as a `Provider`, so the agent code does not need to change at all. The fallback behavior is completely transparent.

## Key Takeaways

- A fallback chain implements the `Provider` trait itself, making fallback transparent to the agent -- it sees a single provider that happens to be resilient
- The `should_fallback()` method on `ProviderError` controls which errors trigger fallback (rate limits, server errors) versus immediate failure (auth errors, bad requests)
- Exponential backoff with jitter prevents retry storms, and the `Retry-After` header is respected when the provider includes one
- Circuit breakers track provider health and temporarily remove failing providers from the chain, avoiding wasted retries during prolonged outages
- The chain is ordered by preference: primary provider first, then alternatives by decreasing capability, with a local model as the final fallback
