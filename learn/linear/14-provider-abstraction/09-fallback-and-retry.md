---
title: Fallback and Retry
description: Build resilient provider chains with automatic retry logic and fallback to alternative providers when the primary is unavailable.
---

# Fallback and Retry

> **What you'll learn:**
> - How to implement exponential backoff with jitter for transient API errors, respecting provider-specific rate limit headers
> - Techniques for building fallback chains that automatically route requests to backup providers when the primary fails or is rate-limited
> - How to distinguish between retryable errors (rate limits, timeouts) and permanent failures (invalid API key, unsupported model) to avoid wasted retries

Cloud APIs fail. They rate-limit you, time out, and occasionally return 500 errors. A production agent cannot crash every time the API hiccups. You need retry logic for transient failures and fallback chains that route to backup providers when the primary is down. Both can be built on top of the `Provider` trait using the decorator pattern.

## Classifying Errors

The first step in retry logic is knowing which errors to retry. Retrying a permanent failure wastes time and money. Your `ProviderError` enum already distinguishes between categories:

```rust
impl ProviderError {
    /// Whether this error is worth retrying.
    pub fn is_retryable(&self) -> bool {
        match self {
            // Rate limits: always retry after waiting
            ProviderError::RateLimited { .. } => true,
            // HTTP errors: retry on server errors and timeouts
            ProviderError::Http(e) => e.is_timeout() || e.is_connect(),
            // API errors: retry on 5xx, not on 4xx
            ProviderError::Api { status, .. } => *status >= 500,
            // Auth errors: never retry (the key won't become valid)
            ProviderError::Auth(_) => false,
            // Unsupported features: never retry
            ProviderError::Unsupported(_) => false,
            // Other: don't retry by default
            ProviderError::Other(_) => false,
        }
    }

    /// Get the suggested retry delay, if any.
    pub fn retry_after(&self) -> Option<std::time::Duration> {
        match self {
            ProviderError::RateLimited { retry_after_ms } => {
                Some(std::time::Duration::from_millis(*retry_after_ms))
            }
            _ => None,
        }
    }
}
```

This classification is critical. If Anthropic returns a 401 (invalid API key), retrying three times with exponential backoff just delays the inevitable. If it returns a 429 (rate limited), waiting and retrying will likely succeed.

## Exponential Backoff with Jitter

The standard retry strategy for API calls is exponential backoff with jitter. Each retry waits longer than the last, and random jitter prevents multiple clients from retrying in lockstep (the "thundering herd" problem):

```rust
use std::time::Duration;
use rand::Rng;

#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Base delay before the first retry.
    pub base_delay: Duration,
    /// Maximum delay between retries (caps the exponential growth).
    pub max_delay: Duration,
    /// Jitter factor: 0.0 = no jitter, 1.0 = full jitter.
    pub jitter: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            jitter: 0.5,
        }
    }
}

impl RetryConfig {
    /// Calculate the delay for the nth retry attempt (0-indexed).
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        // Exponential backoff: base_delay * 2^attempt
        let exponential = self.base_delay.as_millis() as f64 * 2_f64.powi(attempt as i32);
        let capped = exponential.min(self.max_delay.as_millis() as f64);

        // Apply jitter: random value between (1 - jitter) * delay and delay
        let mut rng = rand::thread_rng();
        let jitter_min = capped * (1.0 - self.jitter);
        let jittered = rng.gen_range(jitter_min..=capped);

        Duration::from_millis(jittered as u64)
    }
}
```

::: python Coming from Python
Python's `tenacity` library provides retry decorators out of the box:
```python
@retry(wait=wait_exponential(multiplier=0.5, max=30), stop=stop_after_attempt(3))
async def send_message(self, request):
    ...
```
In Rust, you build the retry logic explicitly. Libraries like `backoff` exist, but the provider abstraction benefits from custom logic that integrates with your `ProviderError` classification. The explicit approach also makes it easier to respect provider-specific rate limit headers.
:::

## The RetryProvider Wrapper

Rather than embedding retry logic in every provider adapter, use the decorator pattern. A `RetryProvider` wraps any `Provider` and adds retry behavior:

```rust
pub struct RetryProvider {
    inner: Box<dyn Provider>,
    config: RetryConfig,
}

impl RetryProvider {
    pub fn new(inner: Box<dyn Provider>, config: RetryConfig) -> Self {
        Self { inner, config }
    }
}

#[async_trait::async_trait]
impl Provider for RetryProvider {
    async fn send_message(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            match self.inner.send_message(request.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    if !e.is_retryable() || attempt == self.config.max_retries {
                        return Err(e);
                    }

                    // Use the provider's suggested delay if available,
                    // otherwise fall back to exponential backoff
                    let delay = e.retry_after()
                        .unwrap_or_else(|| self.config.delay_for_attempt(attempt));

                    eprintln!(
                        "Provider {} returned {}, retrying in {}ms (attempt {}/{})",
                        self.inner.name(),
                        e,
                        delay.as_millis(),
                        attempt + 1,
                        self.config.max_retries,
                    );

                    tokio::time::sleep(delay).await;
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ProviderError::Other("Retry exhausted with no error".into())
        }))
    }

    async fn stream_message(&self, request: ChatRequest) -> Result<StreamResult, ProviderError> {
        // Retry only the initial connection, not mid-stream failures
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            match self.inner.stream_message(request.clone()).await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    if !e.is_retryable() || attempt == self.config.max_retries {
                        return Err(e);
                    }

                    let delay = e.retry_after()
                        .unwrap_or_else(|| self.config.delay_for_attempt(attempt));

                    tokio::time::sleep(delay).await;
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap())
    }

    fn capabilities(&self) -> &ModelCapabilities {
        self.inner.capabilities()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn model(&self) -> &str {
        self.inner.model()
    }
}
```

The `RetryProvider` is itself a `Provider`, so it composes transparently. The rest of the agent has no idea retries are happening — it just sees a provider that succeeds more often.

Notice the streaming case: you retry the initial connection (the `stream_message` call), but once a stream is established and starts yielding events, mid-stream failures are not retried. Retrying mid-stream would require re-sending the entire request and discarding partial results, which is complex and often undesirable.

## Fallback Chains

When retries are exhausted, the next level of resilience is falling back to a different provider. A `FallbackProvider` tries each provider in a chain until one succeeds:

```rust
pub struct FallbackProvider {
    /// Providers to try, in priority order.
    providers: Vec<Box<dyn Provider>>,
    /// Retry config applied to each provider before moving to the next.
    retry_config: RetryConfig,
}

impl FallbackProvider {
    pub fn new(providers: Vec<Box<dyn Provider>>) -> Self {
        Self {
            providers,
            retry_config: RetryConfig::default(),
        }
    }

    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }
}

#[async_trait::async_trait]
impl Provider for FallbackProvider {
    async fn send_message(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let mut errors = Vec::new();

        for (i, provider) in self.providers.iter().enumerate() {
            // Adapt the request for this provider's model
            let mut adapted_request = request.clone();
            adapted_request.model = provider.model().to_string();

            // Check capabilities before trying
            let caps = provider.capabilities();
            if request.tools.is_some() && !caps.supports_tools {
                eprintln!(
                    "Skipping {} ({}): does not support tool use",
                    provider.name(),
                    provider.model()
                );
                continue;
            }

            match provider.send_message(adapted_request).await {
                Ok(response) => {
                    if i > 0 {
                        eprintln!(
                            "Fallback: using {} ({}) after {} failed provider(s)",
                            provider.name(),
                            provider.model(),
                            i
                        );
                    }
                    return Ok(response);
                }
                Err(e) => {
                    eprintln!(
                        "Provider {} ({}) failed: {}",
                        provider.name(),
                        provider.model(),
                        e
                    );
                    errors.push((provider.name().to_string(), e));
                }
            }
        }

        // All providers failed — build a comprehensive error message
        let error_details: Vec<String> = errors.iter()
            .map(|(name, e)| format!("  {name}: {e}"))
            .collect();

        Err(ProviderError::Other(format!(
            "All providers failed:\n{}",
            error_details.join("\n")
        )))
    }

    async fn stream_message(&self, request: ChatRequest) -> Result<StreamResult, ProviderError> {
        // Same fallback logic for streaming
        for provider in &self.providers {
            let mut adapted_request = request.clone();
            adapted_request.model = provider.model().to_string();

            match provider.stream_message(adapted_request).await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    eprintln!("Provider {} failed: {}, trying next", provider.name(), e);
                }
            }
        }
        Err(ProviderError::Other("All providers in fallback chain failed".into()))
    }

    fn capabilities(&self) -> &ModelCapabilities {
        // Return the primary provider's capabilities
        self.providers.first()
            .map(|p| p.capabilities())
            .expect("FallbackProvider must have at least one provider")
    }

    fn name(&self) -> &str {
        "fallback"
    }

    fn model(&self) -> &str {
        self.providers.first()
            .map(|p| p.model())
            .unwrap_or("unknown")
    }
}
```

## Composing Retry and Fallback

The beauty of the decorator pattern is that `RetryProvider` and `FallbackProvider` compose. Wrap each provider in retry logic, then chain them in a fallback:

```rust
fn build_resilient_provider(registry: &ModelRegistry) -> Box<dyn Provider> {
    let anthropic = RetryProvider::new(
        Box::new(AnthropicProvider::new(
            std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY required"),
            "claude-sonnet-4-20250514".into(),
        )),
        RetryConfig { max_retries: 2, ..Default::default() },
    );

    let openai = RetryProvider::new(
        Box::new(OpenAiProvider::new(
            std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            "gpt-4o".into(),
        )),
        RetryConfig { max_retries: 2, ..Default::default() },
    );

    Box::new(FallbackProvider::new(vec![
        Box::new(anthropic),
        Box::new(openai),
    ]))
}
```

This creates a provider that:
1. Tries Anthropic Claude with up to 2 retries per attempt.
2. If all Anthropic retries fail, falls back to OpenAI GPT-4o with its own 2 retries.
3. Only returns an error if both providers are exhausted.

The agentic loop sees a single `Box<dyn Provider>` and has no idea this layered resilience exists.

::: wild In the Wild
Production coding agents rarely expose fallback chain configuration to users, but they implement retry logic internally. Claude Code retries on transient errors with backoff. The Codex CLI handles rate limits by waiting for the `retry-after` header value before resending. Building resilience into the provider layer, rather than into the agentic loop, keeps the core logic clean and allows the resilience strategy to evolve independently.
:::

## Key Takeaways

- Classify errors as retryable (rate limits, server errors, timeouts) or permanent (auth failures, unsupported features) to avoid wasting time on retries that cannot succeed.
- Implement exponential backoff with jitter to prevent thundering herd problems when multiple agent instances hit the same rate limit simultaneously.
- The decorator pattern lets `RetryProvider` and `FallbackProvider` wrap any `Provider` without modifying the original adapters. They are providers themselves, composing transparently.
- For streaming, retry only the initial connection. Mid-stream failures are not retried because doing so would discard partial output and require the full request to be re-sent.
- Fallback chains check capability compatibility before trying a backup provider — there is no point falling back to a model that does not support features the request requires.
