---
title: Cost Tracking
description: Tracking token usage and estimated costs per request and per session across providers, with support for budget limits and cost-aware model selection.
---

# Cost Tracking

> **What you'll learn:**
> - How to extract token usage from provider responses and calculate costs using pricing tables
> - How to implement per-session budget limits that prevent runaway spending
> - Techniques for displaying cost summaries and making cost data available for model selection heuristics

::: warning Pricing and Limits Change Frequently
The specific prices and token limits in this chapter reflect values at the time of writing. Check each provider's current pricing page for up-to-date figures before making cost decisions.
:::

LLM API costs accumulate fast in a coding agent. Each tool call, each follow-up question, each retry adds tokens to the bill. Without cost tracking, users have no visibility into how much a session costs until their monthly invoice arrives. In this subchapter, you build a cost tracker that records every request's token usage, calculates costs per provider, and enforces budget limits.

## The Cost Tracker

The tracker stores per-request cost records and provides summary methods:

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Instant;

use crate::provider::types::TokenUsage;
use crate::provider::capabilities::ModelCapabilities;

/// A single cost record for one API request.
#[derive(Debug, Clone)]
pub struct CostRecord {
    pub provider: String,
    pub model: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_creation_tokens: u32,
    pub estimated_cost_usd: f64,
    pub timestamp: Instant,
}

/// Session-level cost tracking across all providers.
pub struct CostTracker {
    records: Arc<RwLock<Vec<CostRecord>>>,
    budget_limit_usd: Option<f64>,
}

impl CostTracker {
    pub fn new() -> Self {
        Self {
            records: Arc::new(RwLock::new(Vec::new())),
            budget_limit_usd: None,
        }
    }

    pub fn with_budget(mut self, limit_usd: f64) -> Self {
        self.budget_limit_usd = Some(limit_usd);
        self
    }

    /// Record a completed request's usage and return the estimated cost.
    pub async fn record(
        &self,
        provider: &str,
        model: &str,
        usage: &TokenUsage,
        capabilities: Option<&ModelCapabilities>,
    ) -> f64 {
        let cost = match capabilities {
            Some(caps) => caps.estimate_cost(usage.input_tokens, usage.output_tokens),
            None => estimate_cost_fallback(provider, usage),
        };

        let record = CostRecord {
            provider: provider.to_string(),
            model: model.to_string(),
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cache_read_tokens: usage.cache_read_tokens.unwrap_or(0),
            cache_creation_tokens: usage.cache_creation_tokens.unwrap_or(0),
            estimated_cost_usd: cost,
            timestamp: Instant::now(),
        };

        self.records.write().await.push(record);
        cost
    }

    /// Check if the session has exceeded its budget.
    pub async fn is_over_budget(&self) -> bool {
        match self.budget_limit_usd {
            Some(limit) => self.total_cost().await >= limit,
            None => false,
        }
    }

    /// Get the remaining budget, or None if no limit is set.
    pub async fn remaining_budget(&self) -> Option<f64> {
        self.budget_limit_usd.map(|limit| {
            // We need to block on the async total_cost, so use try_read
            // In practice, you'd call this from an async context
            0.0_f64.max(limit) // placeholder
        })
    }

    /// Calculate remaining budget (async version).
    pub async fn budget_remaining(&self) -> Option<f64> {
        match self.budget_limit_usd {
            Some(limit) => Some((limit - self.total_cost().await).max(0.0)),
            None => None,
        }
    }

    /// Get the total cost of the session so far.
    pub async fn total_cost(&self) -> f64 {
        self.records.read().await.iter()
            .map(|r| r.estimated_cost_usd)
            .sum()
    }

    /// Get total token counts across all requests.
    pub async fn total_tokens(&self) -> (u32, u32) {
        let records = self.records.read().await;
        let input: u32 = records.iter().map(|r| r.input_tokens).sum();
        let output: u32 = records.iter().map(|r| r.output_tokens).sum();
        (input, output)
    }

    /// Get the number of requests made.
    pub async fn request_count(&self) -> usize {
        self.records.read().await.len()
    }

    /// Get a per-provider cost breakdown.
    pub async fn cost_by_provider(&self) -> Vec<ProviderCostSummary> {
        let records = self.records.read().await;
        let mut summaries: std::collections::HashMap<String, ProviderCostSummary> =
            std::collections::HashMap::new();

        for record in records.iter() {
            let summary = summaries
                .entry(record.provider.clone())
                .or_insert_with(|| ProviderCostSummary {
                    provider: record.provider.clone(),
                    request_count: 0,
                    total_input_tokens: 0,
                    total_output_tokens: 0,
                    total_cost_usd: 0.0,
                });

            summary.request_count += 1;
            summary.total_input_tokens += record.input_tokens;
            summary.total_output_tokens += record.output_tokens;
            summary.total_cost_usd += record.estimated_cost_usd;
        }

        summaries.into_values().collect()
    }

    /// Get all individual records (for detailed reporting).
    pub async fn all_records(&self) -> Vec<CostRecord> {
        self.records.read().await.clone()
    }
}

#[derive(Debug, Clone)]
pub struct ProviderCostSummary {
    pub provider: String,
    pub request_count: usize,
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
    pub total_cost_usd: f64,
}
```

The tracker uses `Arc<RwLock<Vec<CostRecord>>>` so it can be shared across the agent's components. Recording is a write operation; querying is a read operation. Multiple components can read cost data concurrently.

## Fallback Cost Estimation

When the model is not in the capabilities registry, you still want a rough cost estimate:

```rust
/// Rough cost estimate when model capabilities are unknown.
fn estimate_cost_fallback(provider: &str, usage: &TokenUsage) -> f64 {
    // Conservative estimates per million tokens
    let (input_rate, output_rate) = match provider {
        "anthropic" => (3.0, 15.0),    // Assume Sonnet-tier pricing
        "openai" => (2.50, 10.0),      // Assume GPT-4o pricing
        "ollama" => (0.0, 0.0),        // Local models are free
        _ => (5.0, 15.0),              // Conservative default
    };

    let input_cost = (usage.input_tokens as f64 / 1_000_000.0) * input_rate;
    let output_cost = (usage.output_tokens as f64 / 1_000_000.0) * output_rate;
    input_cost + output_cost
}
```

The fallback uses conservative estimates -- it is better to slightly overcount than undercount when tracking costs. Users would rather see a budget warning too early than too late.

::: python Coming from Python
Python's `openai` and `anthropic` SDKs include usage data in responses:
```python
response = client.messages.create(...)
print(f"Input tokens: {response.usage.input_tokens}")
print(f"Output tokens: {response.usage.output_tokens}")
```
The token counts are easy to extract. What Python does not give you automatically is cost calculation, budget tracking, or per-session aggregation. The Rust `CostTracker` handles all of this in a single, thread-safe component.
:::

## Integrating with the Agent

Wire the cost tracker into the agent so every request is tracked:

```rust
use std::sync::Arc;
use crate::provider::capabilities::CapabilityRegistry;

pub struct Agent {
    provider: Arc<dyn Provider>,
    capabilities: Arc<CapabilityRegistry>,
    cost_tracker: Arc<CostTracker>,
    system_prompt: String,
    tools: Vec<ToolDefinition>,
    messages: Vec<Message>,
}

impl Agent {
    pub async fn send(&mut self) -> Result<ProviderResponse, ProviderError> {
        // Check budget before sending
        if self.cost_tracker.is_over_budget().await {
            return Err(ProviderError::Api {
                status: 0,
                message: format!(
                    "Session budget exhausted. Total spent: ${:.4}",
                    self.cost_tracker.total_cost().await
                ),
                retryable: false,
            });
        }

        let response = self.provider.send_message(
            &self.system_prompt,
            &self.messages,
            &self.tools,
            4096,
        ).await?;

        // Record the cost
        let caps = self.capabilities.get(self.provider.model());
        let cost = self.cost_tracker.record(
            self.provider.name(),
            self.provider.model(),
            &response.usage,
            caps,
        ).await;

        // Warn if approaching budget limit
        if let Some(remaining) = self.cost_tracker.budget_remaining().await {
            if remaining < 0.01 {
                eprintln!(
                    "[cost] Warning: ${:.4} remaining in session budget",
                    remaining
                );
            }
        }

        Ok(response)
    }
}
```

Budget enforcement happens at two points: before sending (hard stop) and after receiving (warning). The hard stop prevents a request when the budget is already exhausted. The warning alerts the user when they are close to the limit, giving them a chance to switch to a cheaper model.

## Displaying Cost Information

Add a `/cost` command to the agent's REPL that shows a session cost summary:

```rust
pub async fn format_cost_summary(tracker: &CostTracker) -> String {
    let total = tracker.total_cost().await;
    let (input_tokens, output_tokens) = tracker.total_tokens().await;
    let requests = tracker.request_count().await;
    let by_provider = tracker.cost_by_provider().await;

    let mut output = String::new();
    output.push_str(&format!("Session Cost Summary\n"));
    output.push_str(&format!("{}\n", "=".repeat(40)));
    output.push_str(&format!("Total requests: {}\n", requests));
    output.push_str(&format!("Total tokens:   {} in / {} out\n",
        format_number(input_tokens),
        format_number(output_tokens),
    ));
    output.push_str(&format!("Total cost:     ${:.4}\n\n", total));

    if by_provider.len() > 1 {
        output.push_str("By Provider:\n");
        output.push_str(&format!("{:<12} {:>8} {:>12} {:>12} {:>10}\n",
            "Provider", "Requests", "Input Tok", "Output Tok", "Cost"));
        output.push_str(&format!("{}\n", "-".repeat(56)));

        for summary in &by_provider {
            output.push_str(&format!("{:<12} {:>8} {:>12} {:>12} {:>10.4}\n",
                summary.provider,
                summary.request_count,
                format_number(summary.total_input_tokens),
                format_number(summary.total_output_tokens),
                summary.total_cost_usd,
            ));
        }
    }

    output
}

fn format_number(n: u32) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}
```

A typical output:

```
Session Cost Summary
========================================
Total requests: 14
Total tokens:   45.2K in / 12.8K out
Total cost:     $0.3284

By Provider:
Provider     Requests    Input Tok   Output Tok       Cost
--------------------------------------------------------
anthropic          10       38.1K        10.2K     0.2673
openai              3        6.5K         2.4K     0.0563
ollama              1         612          186     0.0000
```

## Cost-Aware Model Selection

The cost tracker feeds into the automatic model selection system from the previous subchapter. When budget is low, the agent prefers cheaper models:

```rust
use crate::provider::capabilities::CapabilityRegistry;

/// Select a model based on remaining budget.
pub async fn budget_aware_model_selection(
    registry: &CapabilityRegistry,
    tracker: &CostTracker,
) -> Option<String> {
    let remaining = match tracker.budget_remaining().await {
        Some(r) => r,
        None => return None, // No budget set, no preference
    };

    // Estimate tokens for a typical request
    let estimated_input = 5_000_u32;
    let estimated_output = 2_000_u32;

    // Find the best model that fits within the remaining budget
    let mut candidates: Vec<_> = registry.models_for_provider("anthropic")
        .into_iter()
        .chain(registry.models_for_provider("openai"))
        .chain(registry.models_for_provider("ollama"))
        .filter(|m| {
            let cost = m.estimate_cost(estimated_input, estimated_output);
            cost <= remaining
        })
        .collect();

    // Sort by quality tier (descending), then by cost (ascending)
    candidates.sort_by(|a, b| {
        b.quality_tier.cmp(&a.quality_tier)
            .then(a.input_cost_per_million.partial_cmp(&b.input_cost_per_million)
                .unwrap_or(std::cmp::Ordering::Equal))
    });

    candidates.first().map(|m| m.model_id.clone())
}
```

This function finds the highest-quality model that the remaining budget can afford. When the budget is ample, it picks the best model. As the budget shrinks, it automatically downgrades to cheaper alternatives.

## Cache-Aware Cost Calculation

Anthropic's prompt caching can significantly reduce costs by caching large system prompts and conversation prefixes. The cost tracker should account for cached tokens being cheaper:

```rust
fn calculate_cost_with_caching(
    capabilities: &ModelCapabilities,
    usage: &TokenUsage,
) -> f64 {
    let base_input_cost = (usage.input_tokens as f64 / 1_000_000.0)
        * capabilities.input_cost_per_million;
    let output_cost = (usage.output_tokens as f64 / 1_000_000.0)
        * capabilities.output_cost_per_million;

    // Cache reads are typically 90% cheaper than regular input
    let cache_read_cost = match usage.cache_read_tokens {
        Some(tokens) => {
            (tokens as f64 / 1_000_000.0)
                * capabilities.input_cost_per_million * 0.1
        }
        None => 0.0,
    };

    // Cache creation has a one-time premium (typically 25% more)
    let cache_creation_cost = match usage.cache_creation_tokens {
        Some(tokens) => {
            (tokens as f64 / 1_000_000.0)
                * capabilities.input_cost_per_million * 1.25
        }
        None => 0.0,
    };

    base_input_cost + output_cost + cache_read_cost + cache_creation_cost
}
```

Prompt caching can reduce the effective input token cost dramatically on subsequent requests. Tracking this separately gives users accurate cost data and lets the cost-aware model selector make better decisions.

## Key Takeaways

- The `CostTracker` records per-request token usage and estimated costs, providing total, per-provider, and per-request summaries
- Budget enforcement happens at two points: a hard stop before sending when the budget is exhausted, and a warning after receiving when the budget is nearly depleted
- Fallback cost estimation uses conservative per-provider rates when a model is not in the capabilities registry, erring on the side of overestimating costs
- Cost-aware model selection picks the highest-quality model that fits within the remaining budget, automatically downgrading as the budget shrinks
- Cache-aware cost calculation accounts for Anthropic's prompt caching discounts, which can reduce effective input costs by up to 90%
