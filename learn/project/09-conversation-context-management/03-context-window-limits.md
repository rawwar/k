---
title: Context Window Limits
description: Map out the context window budget including model limits, reserved space for responses, and practical working limits for agent operations.
---

# Context Window Limits

> **What you'll learn:**
> - How to determine the effective context budget after reserving space for the response and tool definitions
> - How different Claude models have different context window sizes and how to adapt dynamically
> - How to implement a token budget tracker that warns before hitting limits

::: warning Token Limits Change Frequently
The specific context window sizes and token limits in this chapter reflect values at the time of writing. Check each model's current documentation for up-to-date figures.
:::

Now that you can count tokens accurately, you need a system that tracks your context budget in real time. Think of it like a financial budget: you have income (the model's context limit), fixed expenses (system prompt, tool definitions), and discretionary spending (conversation history). The budget tracker makes sure you never overdraft.

## Model Limits Are Not Your Limit

Each model advertises a maximum context window, but the usable space for conversation history is always smaller. You need to subtract fixed costs before you know what is actually available:

```
Usable context = Model limit - System prompt - Tool definitions - Response reserve - Safety margin
```

Here are the real numbers for Claude models:

| Model | Advertised Limit | System + Tools | Response Reserve | Usable for History |
|-------|----------------:|---------------:|-----------------:|-------------------:|
| Claude 3.5 Sonnet | 200K | ~5K | 8K | ~187K |
| Claude 4 Sonnet | 200K | ~5K | 8K | ~187K |
| Claude Opus 4 | 200K (standard), up to 1M (extended) | ~5K | 8K | ~187K / ~987K |

The response reserve is critical and often overlooked. If you fill the context window to the brim, the model has no room to generate a response. You must always leave headroom for the reply. For a coding agent, 4,000--8,000 tokens of response reserve is reasonable -- the model needs space to write code, explanations, and tool call arguments.

## Building a Budget Tracker

Let's build a `ContextBudget` struct that tracks usage across all components:

```rust
use std::fmt;

/// Tracks token usage across all components of an API request.
#[derive(Debug, Clone)]
pub struct ContextBudget {
    /// Maximum tokens the model accepts
    model_limit: usize,
    /// Tokens reserved for the model's response
    response_reserve: usize,
    /// Current token count of the system prompt
    system_prompt_tokens: usize,
    /// Current token count of tool definitions
    tool_definition_tokens: usize,
    /// Current token count of conversation messages
    message_tokens: usize,
    /// Safety margin to avoid edge cases
    safety_margin: usize,
}

/// Status of the context budget
#[derive(Debug, PartialEq)]
pub enum BudgetStatus {
    /// Plenty of room remaining
    Healthy,
    /// Approaching the limit (>80% used)
    Warning,
    /// At or near the limit (>95% used) -- compaction needed
    Critical,
    /// Over budget -- must compact before next request
    Exceeded,
}

impl ContextBudget {
    pub fn new(model_limit: usize) -> Self {
        Self {
            model_limit,
            response_reserve: 8_000,
            system_prompt_tokens: 0,
            tool_definition_tokens: 0,
            message_tokens: 0,
            safety_margin: 200,
        }
    }

    /// Create a budget for a specific model by name.
    pub fn for_model(model_name: &str) -> Self {
        let limit = match model_name {
            "claude-sonnet-4-20250514" => 200_000,
            "claude-opus-4-20250514" => 200_000,
            "claude-3-5-sonnet-20241022" => 200_000,
            _ => 200_000, // Safe default
        };
        Self::new(limit)
    }

    /// Set the system prompt token count.
    pub fn set_system_prompt_tokens(&mut self, tokens: usize) {
        self.system_prompt_tokens = tokens;
    }

    /// Set the tool definition token count.
    pub fn set_tool_definition_tokens(&mut self, tokens: usize) {
        self.tool_definition_tokens = tokens;
    }

    /// Update the conversation message token count.
    pub fn set_message_tokens(&mut self, tokens: usize) {
        self.message_tokens = tokens;
    }

    /// Total tokens currently allocated across all components.
    pub fn total_used(&self) -> usize {
        self.system_prompt_tokens
            + self.tool_definition_tokens
            + self.message_tokens
            + self.response_reserve
            + self.safety_margin
    }

    /// How many tokens are available for additional messages?
    pub fn available(&self) -> usize {
        self.model_limit.saturating_sub(self.total_used())
    }

    /// What fraction of the budget is used (0.0 to 1.0+)?
    pub fn utilization(&self) -> f64 {
        self.total_used() as f64 / self.model_limit as f64
    }

    /// Current budget status.
    pub fn status(&self) -> BudgetStatus {
        let util = self.utilization();
        if util > 1.0 {
            BudgetStatus::Exceeded
        } else if util > 0.95 {
            BudgetStatus::Critical
        } else if util > 0.80 {
            BudgetStatus::Warning
        } else {
            BudgetStatus::Healthy
        }
    }

    /// How many tokens need to be freed to bring usage below the target ratio?
    pub fn tokens_to_free(&self, target_ratio: f64) -> usize {
        let target_total = (self.model_limit as f64 * target_ratio) as usize;
        self.total_used().saturating_sub(target_total)
    }
}

impl fmt::Display for ContextBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Context: {}/{} ({:.1}%) [{:?}] | sys:{} tools:{} msgs:{} rsv:{}",
            self.total_used(),
            self.model_limit,
            self.utilization() * 100.0,
            self.status(),
            self.system_prompt_tokens,
            self.tool_definition_tokens,
            self.message_tokens,
            self.response_reserve,
        )
    }
}

fn main() {
    let mut budget = ContextBudget::for_model("claude-sonnet-4-20250514");

    // Simulate building up context
    budget.set_system_prompt_tokens(1_500);
    budget.set_tool_definition_tokens(3_000);
    println!("After setup: {}", budget);

    // Simulate conversation growing
    for turn in 1..=20 {
        let new_tokens = budget.message_tokens + 8_000;
        budget.set_message_tokens(new_tokens);
        let status = budget.status();
        println!("Turn {:>2}: {} | Available: {}", turn, budget, budget.available());

        if status == BudgetStatus::Critical || status == BudgetStatus::Exceeded {
            let to_free = budget.tokens_to_free(0.70);
            println!("  ** COMPACTION NEEDED: free {} tokens to reach 70% **", to_free);
        }
    }
}
```

This tracker gives you everything you need to make compaction decisions. The `status()` method classifies how urgent the situation is, and `tokens_to_free()` tells you exactly how many tokens to reclaim.

::: python Coming from Python
In Python, you might track this with a simple class and dictionary:
```python
class ContextBudget:
    def __init__(self, limit):
        self.limit = limit
        self.used = {"system": 0, "tools": 0, "messages": 0}

    @property
    def available(self):
        return self.limit - sum(self.used.values()) - 8000  # reserve
```
The Rust version uses an enum for `BudgetStatus` instead of returning a string --
this gives you exhaustive match checking at compile time. If you add a new status
level later, the compiler forces you to handle it everywhere.
:::

## Dynamic Model Detection

Your agent should adapt its budget based on which model it is talking to. Rather than hardcoding limits, make the budget configurable:

```rust
use std::collections::HashMap;

/// Registry of known model configurations.
pub struct ModelRegistry {
    models: HashMap<String, ModelConfig>,
}

#[derive(Clone)]
pub struct ModelConfig {
    pub name: String,
    pub context_limit: usize,
    pub max_output_tokens: usize,
    pub cost_per_million_input: f64,
    pub cost_per_million_output: f64,
}

impl ModelRegistry {
    pub fn new() -> Self {
        let mut models = HashMap::new();

        models.insert("claude-sonnet-4-20250514".to_string(), ModelConfig {
            name: "Claude Sonnet 4".to_string(),
            context_limit: 200_000,
            max_output_tokens: 16_000,
            cost_per_million_input: 3.0,
            cost_per_million_output: 15.0,
        });

        models.insert("claude-opus-4-20250514".to_string(), ModelConfig {
            name: "Claude Opus 4".to_string(),
            context_limit: 200_000,
            max_output_tokens: 32_000,
            cost_per_million_input: 15.0,
            cost_per_million_output: 75.0,
        });

        Self { models }
    }

    /// Look up a model's configuration, falling back to conservative defaults.
    pub fn get(&self, model_id: &str) -> ModelConfig {
        self.models.get(model_id).cloned().unwrap_or(ModelConfig {
            name: format!("Unknown ({})", model_id),
            context_limit: 100_000, // Conservative default
            max_output_tokens: 4_096,
            cost_per_million_input: 10.0,
            cost_per_million_output: 30.0,
        })
    }

    /// Create a ContextBudget appropriate for a given model.
    pub fn budget_for(&self, model_id: &str) -> ContextBudget {
        let config = self.get(model_id);
        let mut budget = ContextBudget::new(config.context_limit);
        budget.response_reserve = config.max_output_tokens.min(8_000);
        budget
    }
}

fn main() {
    let registry = ModelRegistry::new();

    for model_id in &["claude-sonnet-4-20250514", "claude-opus-4-20250514", "unknown-model"] {
        let config = registry.get(model_id);
        let budget = registry.budget_for(model_id);
        println!(
            "{}: limit={}K, output={}K, budget available={}K",
            config.name,
            config.context_limit / 1000,
            config.max_output_tokens / 1000,
            budget.available() / 1000,
        );
    }
}
```

The conservative default for unknown models is deliberate -- if your agent encounters a model it does not recognize, it is better to underestimate the context limit than to overshoot it. You can always update the registry when new models are released.

## Integrating the Budget with Your Agent Loop

The budget tracker plugs directly into the agentic loop you built in Chapter 3. Before each API call, check the budget:

```rust
/// Simplified agentic loop showing budget integration.
fn agent_turn(
    budget: &mut ContextBudget,
    messages: &[(String, String)],
    token_counter: &TokenCounter,
) -> Result<(), String> {
    // Recount message tokens (or use cached totals)
    let msg_tokens: usize = messages.iter()
        .map(|(role, content)| token_counter.count_message(role, content))
        .sum();
    budget.set_message_tokens(msg_tokens);

    match budget.status() {
        BudgetStatus::Exceeded => {
            return Err(format!(
                "Context exceeded by {} tokens. Compact before retrying.",
                budget.tokens_to_free(0.90)
            ));
        }
        BudgetStatus::Critical => {
            let to_free = budget.tokens_to_free(0.70);
            println!("Warning: context at {:.0}%, need to free {} tokens",
                budget.utilization() * 100.0, to_free);
            // Trigger compaction (covered in subchapter 07)
        }
        BudgetStatus::Warning => {
            println!("Context at {:.0}% -- consider compaction soon",
                budget.utilization() * 100.0);
        }
        BudgetStatus::Healthy => {
            // All good, proceed
        }
    }

    println!("Proceeding with API call. {}", budget);
    Ok(())
}

fn main() {
    let mut budget = ContextBudget::for_model("claude-sonnet-4-20250514");
    budget.set_system_prompt_tokens(1_500);
    budget.set_tool_definition_tokens(3_000);

    let token_counter = TokenCounter::new();
    let messages: Vec<(String, String)> = vec![
        ("user".to_string(), "Read the file src/main.rs".to_string()),
        ("assistant".to_string(), "I'll read that file for you.".to_string()),
    ];

    match agent_turn(&mut budget, &messages, &token_counter) {
        Ok(()) => println!("Turn completed successfully"),
        Err(e) => println!("Error: {}", e),
    }
}
```

::: wild In the Wild
Claude Code monitors context utilization continuously and triggers automatic compaction when usage crosses a threshold. It does not wait until the context is full -- it proactively compacts at around 80% utilization to avoid the performance degradation that comes with nearly-full contexts. This matches the Warning threshold in our budget tracker. OpenCode uses a similar approach with configurable thresholds per model.
:::

## Key Takeaways

- The usable context budget is always smaller than the advertised model limit -- subtract system prompt, tool definitions, response reserve, and safety margin
- Build a budget tracker that classifies status as Healthy, Warning, Critical, or Exceeded so your agent can make graduated compaction decisions
- Use `saturating_sub` in budget calculations to avoid unsigned integer underflow panics
- Adapt the budget dynamically based on the model being used, with conservative defaults for unknown models
- Integrate budget checking into your agentic loop -- check before every API call and trigger compaction proactively at 80% rather than reactively at 100%
