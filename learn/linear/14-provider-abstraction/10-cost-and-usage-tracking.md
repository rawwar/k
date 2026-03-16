---
title: Cost and Usage Tracking
description: Implement transparent cost and token usage tracking across all providers, giving users visibility into agent resource consumption.
---

# Cost and Usage Tracking

> **What you'll learn:**
> - How to extract token usage data from each provider's response format and normalize it into a unified usage record
> - Techniques for calculating real-time cost estimates using per-model pricing tables, including input/output token differentiation
> - How to implement usage budgets and alerts that warn users or halt execution when spending thresholds are approached

::: warning Pricing and Limits Change Frequently
The specific prices and token limits in this chapter reflect values at the time of writing. Check each provider's current pricing page for up-to-date figures before making cost decisions.
:::

LLM API calls cost money, and agentic loops can burn through tokens fast. A coding agent that makes 20 tool calls in a loop, each with the full conversation history, can easily consume millions of tokens in a single session. Users need visibility into what they are spending. In this subchapter you build a tracking system that records usage per request, calculates costs, and enforces budgets.

## The Usage Tracking Data Model

Start with types that represent what you are tracking:

```rust
use std::time::{Duration, Instant};

/// A single API call's usage data.
#[derive(Debug, Clone)]
pub struct UsageRecord {
    /// Which provider handled the request.
    pub provider: String,
    /// Which model was used.
    pub model: String,
    /// Token counts.
    pub usage: Usage,
    /// Estimated cost in USD.
    pub estimated_cost: f64,
    /// How long the API call took.
    pub latency: Duration,
    /// When the call was made.
    pub timestamp: Instant,
}

/// Accumulated usage across an entire session.
#[derive(Debug, Clone, Default)]
pub struct SessionUsage {
    /// Total input tokens consumed.
    pub total_input_tokens: u64,
    /// Total output tokens generated.
    pub total_output_tokens: u64,
    /// Total tokens read from cache.
    pub total_cache_read_tokens: u64,
    /// Total tokens written to cache.
    pub total_cache_write_tokens: u64,
    /// Total estimated cost in USD.
    pub total_cost: f64,
    /// Number of API calls made.
    pub request_count: u32,
    /// Individual records for detailed analysis.
    pub records: Vec<UsageRecord>,
}
```

The `SessionUsage` struct accumulates across the entire agent session, while each `UsageRecord` captures a single API call. Storing individual records lets you show the user a breakdown of where tokens went.

## Cost Calculation

Cost depends on the model and whether tokens are input or output. You already have the `ModelPricing` struct in the registry — now use it:

```rust
impl SessionUsage {
    /// Add a new usage record and update totals.
    pub fn record(
        &mut self,
        provider: &str,
        model: &str,
        usage: &Usage,
        pricing: &ModelPricing,
        latency: Duration,
    ) {
        let input_cost = (usage.input_tokens as f64 / 1_000_000.0) * pricing.input_per_million;
        let output_cost = (usage.output_tokens as f64 / 1_000_000.0) * pricing.output_per_million;

        let cache_cost = match (usage.cache_read_tokens, pricing.cached_input_per_million) {
            (Some(tokens), Some(price)) => (tokens as f64 / 1_000_000.0) * price,
            _ => 0.0,
        };

        let estimated_cost = input_cost + output_cost + cache_cost;

        let record = UsageRecord {
            provider: provider.to_string(),
            model: model.to_string(),
            usage: usage.clone(),
            estimated_cost,
            latency,
            timestamp: Instant::now(),
        };

        self.total_input_tokens += usage.input_tokens as u64;
        self.total_output_tokens += usage.output_tokens as u64;
        self.total_cache_read_tokens += usage.cache_read_tokens.unwrap_or(0) as u64;
        self.total_cache_write_tokens += usage.cache_write_tokens.unwrap_or(0) as u64;
        self.total_cost += estimated_cost;
        self.request_count += 1;
        self.records.push(record);
    }

    /// Format a human-readable summary.
    pub fn summary(&self) -> String {
        format!(
            "Session: {} requests | {} input + {} output tokens | ${:.4} estimated cost",
            self.request_count,
            self.total_input_tokens,
            self.total_output_tokens,
            self.total_cost,
        )
    }

    /// Detailed per-request breakdown.
    pub fn detailed_report(&self) -> String {
        let mut lines = vec![
            "Request # | Model                  | In Tokens | Out Tokens | Cost     | Latency"
                .to_string(),
            "-".repeat(85),
        ];

        for (i, record) in self.records.iter().enumerate() {
            lines.push(format!(
                "{:>9} | {:22} | {:>9} | {:>10} | ${:>6.4} | {:>5}ms",
                i + 1,
                record.model,
                record.usage.input_tokens,
                record.usage.output_tokens,
                record.estimated_cost,
                record.latency.as_millis(),
            ));
        }

        lines.push("-".repeat(85));
        lines.push(format!(
            "    Total | {:22} | {:>9} | {:>10} | ${:>6.4}",
            "",
            self.total_input_tokens,
            self.total_output_tokens,
            self.total_cost,
        ));

        lines.join("\n")
    }
}
```

::: python Coming from Python
In Python, you might track usage with a simple dictionary and `defaultdict(int)` for accumulation. Rust's approach uses typed structs, which means the compiler ensures you never forget to track a field. If you add `cache_write_tokens` to `Usage`, any code that constructs a `UsageRecord` must handle it — there is no risk of silently dropping data.
:::

## The Tracking Provider Decorator

Like retry and fallback, usage tracking is implemented as a decorator. A `TrackingProvider` wraps any provider and records usage from every response:

```rust
use std::sync::{Arc, Mutex};

pub struct TrackingProvider {
    inner: Box<dyn Provider>,
    usage: Arc<Mutex<SessionUsage>>,
    pricing: ModelPricing,
}

impl TrackingProvider {
    pub fn new(
        inner: Box<dyn Provider>,
        usage: Arc<Mutex<SessionUsage>>,
        pricing: ModelPricing,
    ) -> Self {
        Self { inner, usage, pricing }
    }

    /// Get a snapshot of current usage.
    pub fn current_usage(&self) -> SessionUsage {
        self.usage.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl Provider for TrackingProvider {
    async fn send_message(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let start = Instant::now();
        let response = self.inner.send_message(request).await?;
        let latency = start.elapsed();

        // Record usage from the response
        self.usage.lock().unwrap().record(
            self.inner.name(),
            self.inner.model(),
            &response.usage,
            &self.pricing,
            latency,
        );

        Ok(response)
    }

    async fn stream_message(&self, request: ChatRequest) -> Result<StreamResult, ProviderError> {
        let start = Instant::now();
        let stream = self.inner.stream_message(request).await?;

        // Wrap the stream to capture the final Done event's usage data
        let usage = self.usage.clone();
        let pricing = self.pricing.clone();
        let provider_name = self.inner.name().to_string();
        let model_name = self.inner.model().to_string();

        let tracking_stream = async_stream::stream! {
            let mut stream = std::pin::pin!(stream);

            while let Some(event) = stream.next().await {
                match &event {
                    Ok(StreamEvent::Done { usage: event_usage, .. }) => {
                        let latency = start.elapsed();
                        usage.lock().unwrap().record(
                            &provider_name,
                            &model_name,
                            event_usage,
                            &pricing,
                            latency,
                        );
                    }
                    _ => {}
                }
                yield event;
            }
        };

        Ok(Box::pin(tracking_stream))
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

The streaming case is interesting: you cannot record usage when the stream starts because the token count is not known until the stream ends. The decorator wraps the stream in a new stream that watches for the `StreamEvent::Done` event and records usage at that point.

## Budget Enforcement

With tracking in place, you can enforce spending limits. A budget check runs before each request:

```rust
#[derive(Debug, Clone)]
pub struct UsageBudget {
    /// Maximum cost in USD for the session.
    pub max_cost: Option<f64>,
    /// Maximum total tokens for the session.
    pub max_tokens: Option<u64>,
    /// Warning threshold as a fraction of the limit (e.g., 0.8 = warn at 80%).
    pub warning_threshold: f64,
}

impl Default for UsageBudget {
    fn default() -> Self {
        Self {
            max_cost: None,
            max_tokens: None,
            warning_threshold: 0.8,
        }
    }
}

impl UsageBudget {
    /// Check if the current usage is within budget.
    /// Returns Ok(warning) or Err(exceeded).
    pub fn check(&self, usage: &SessionUsage) -> BudgetStatus {
        if let Some(max_cost) = self.max_cost {
            if usage.total_cost >= max_cost {
                return BudgetStatus::Exceeded(format!(
                    "Cost budget exceeded: ${:.4} >= ${:.4}",
                    usage.total_cost, max_cost
                ));
            }
            if usage.total_cost >= max_cost * self.warning_threshold {
                return BudgetStatus::Warning(format!(
                    "Approaching cost limit: ${:.4} / ${:.4} ({:.0}%)",
                    usage.total_cost,
                    max_cost,
                    (usage.total_cost / max_cost) * 100.0
                ));
            }
        }

        if let Some(max_tokens) = self.max_tokens {
            let total = usage.total_input_tokens + usage.total_output_tokens;
            if total >= max_tokens {
                return BudgetStatus::Exceeded(format!(
                    "Token budget exceeded: {} >= {}",
                    total, max_tokens
                ));
            }
            if total >= (max_tokens as f64 * self.warning_threshold) as u64 {
                return BudgetStatus::Warning(format!(
                    "Approaching token limit: {} / {} ({:.0}%)",
                    total,
                    max_tokens,
                    (total as f64 / max_tokens as f64) * 100.0
                ));
            }
        }

        BudgetStatus::Ok
    }
}

#[derive(Debug)]
pub enum BudgetStatus {
    Ok,
    Warning(String),
    Exceeded(String),
}
```

Integrate the budget check into the agent loop:

```rust
impl Agent {
    async fn send_with_budget_check(
        &self,
        request: ChatRequest,
    ) -> Result<ChatResponse, ProviderError> {
        // Check budget before sending
        let current_usage = self.usage.lock().unwrap().clone();
        match self.budget.check(&current_usage) {
            BudgetStatus::Ok => {}
            BudgetStatus::Warning(msg) => {
                eprintln!("Warning: {msg}");
            }
            BudgetStatus::Exceeded(msg) => {
                return Err(ProviderError::Other(format!(
                    "Budget exceeded: {msg}. Use /budget to increase the limit."
                )));
            }
        }

        self.provider.read().await.send_message(request).await
    }
}
```

## Displaying Usage to the User

Add commands for users to check their session usage:

```rust
fn handle_usage_command(usage: &SessionUsage) {
    println!("\n{}", usage.summary());
    println!();

    if !usage.records.is_empty() {
        // Show last 5 requests
        let start = usage.records.len().saturating_sub(5);
        for (i, record) in usage.records[start..].iter().enumerate() {
            println!(
                "  #{}: {} {} — {} in / {} out — ${:.4} — {}ms",
                start + i + 1,
                record.provider,
                record.model,
                record.usage.input_tokens,
                record.usage.output_tokens,
                record.estimated_cost,
                record.latency.as_millis(),
            );
        }
    }
}
```

## Composing All Decorators

Now you see the full composition: tracking wraps retry, which wraps the base provider. All three are `Provider` implementations, stacking transparently:

```rust
fn build_production_provider(
    registry: &ModelRegistry,
    usage: Arc<Mutex<SessionUsage>>,
) -> Box<dyn Provider> {
    let model_info = registry.get("claude-sonnet-4-20250514").unwrap();

    // Layer 1: Base provider
    let base = AnthropicProvider::new(
        std::env::var("ANTHROPIC_API_KEY").unwrap(),
        model_info.model_id.clone(),
    );

    // Layer 2: Retry on transient failures
    let with_retry = RetryProvider::new(
        Box::new(base),
        RetryConfig::default(),
    );

    // Layer 3: Track usage and costs
    let with_tracking = TrackingProvider::new(
        Box::new(with_retry),
        usage,
        model_info.pricing.clone(),
    );

    Box::new(with_tracking)
}
```

Each layer adds behavior without the layers above or below knowing about it. The `TrackingProvider` records usage even for retried requests — if a request is retried twice, only the successful attempt's usage is recorded (because only the successful response reaches the tracking layer).

::: wild In the Wild
Claude Code displays token usage after each interaction, showing input tokens, output tokens, and estimated cost. This transparency helps users understand the cost of agentic workflows where tool calls create multiple round trips. OpenCode provides similar visibility through a status bar that updates in real time as tokens stream in. Both approaches help users make informed decisions about when to use expensive models versus cheaper alternatives.
:::

## Key Takeaways

- Track usage per-request with `UsageRecord` and accumulate into `SessionUsage`, keeping individual records for detailed breakdowns.
- Calculate costs using the `ModelPricing` data from the registry, differentiating between input tokens, output tokens, and cached tokens.
- The `TrackingProvider` decorator wraps any provider and records usage transparently. For streaming, it watches the `StreamEvent::Done` event to capture final token counts.
- Budget enforcement checks usage before each request and can either warn or halt execution when limits are approached. This prevents runaway agentic loops from burning through API credits.
- All three decorator layers — tracking, retry, fallback — compose as `Provider` implementations, giving you a production-grade provider stack from simple, testable building blocks.
