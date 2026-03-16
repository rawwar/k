---
title: Context Window Management
description: Strategies for staying within model context limits by tracking token budgets, reserving space for responses, and triggering compaction at the right thresholds.
---

# Context Window Management

> **What you'll learn:**
> - How to implement a token budget tracker that accounts for system prompt, message history, tool definitions, and reserved response space
> - Setting compaction thresholds that trigger before hitting hard context limits to avoid truncation by the API
> - The tradeoff between aggressive early compaction (less context, lower cost) and late compaction (more context, higher cost)

You now know how to count tokens accurately. The next question is: what do you do with those counts? Context window management is the strategy layer that sits between token counting and compaction. It answers the question "should I compact now, and how much do I need to remove?" -- a question your agent asks before every single LLM call.

Get this wrong and one of two bad things happens: either the API rejects your request because you exceeded the context limit, or you compact too aggressively and the LLM loses context it needed to complete the task. Good context window management threads the needle between these two failure modes.

## The Token Budget Model

Think of the context window as a fixed budget that you allocate across competing needs:

```rust
#[derive(Debug, Clone)]
struct ContextWindowConfig {
    /// Maximum tokens the model accepts (e.g., 200_000 for Claude Sonnet)
    max_context_tokens: u32,
    /// Tokens reserved for the model's response
    response_reserve: u32,
    /// Safety margin to account for tokenizer estimation errors
    safety_margin: u32,
}

impl ContextWindowConfig {
    fn claude_sonnet() -> Self {
        Self {
            max_context_tokens: 200_000,
            response_reserve: 8_192,
            safety_margin: 500,
        }
    }

    fn gpt4() -> Self {
        Self {
            max_context_tokens: 128_000,
            response_reserve: 4_096,
            safety_margin: 500,
        }
    }

    /// The maximum tokens available for input (system + messages + tools)
    fn available_input_tokens(&self) -> u32 {
        self.max_context_tokens - self.response_reserve - self.safety_margin
    }
}

#[derive(Debug)]
struct TokenBudgetTracker {
    config: ContextWindowConfig,
    /// Current token usage breakdown
    system_prompt_tokens: u32,
    tool_definition_tokens: u32,
    message_tokens: u32,
    framing_tokens: u32,
}

impl TokenBudgetTracker {
    fn new(config: ContextWindowConfig) -> Self {
        Self {
            config,
            system_prompt_tokens: 0,
            tool_definition_tokens: 0,
            message_tokens: 0,
            framing_tokens: 12, // Base conversation framing
        }
    }

    fn total_input_tokens(&self) -> u32 {
        self.system_prompt_tokens
            + self.tool_definition_tokens
            + self.message_tokens
            + self.framing_tokens
    }

    fn remaining_tokens(&self) -> i64 {
        self.config.available_input_tokens() as i64 - self.total_input_tokens() as i64
    }

    fn utilization_percent(&self) -> f32 {
        let available = self.config.available_input_tokens() as f32;
        (self.total_input_tokens() as f32 / available) * 100.0
    }

    fn update_system_prompt(&mut self, tokens: u32) {
        self.system_prompt_tokens = tokens;
    }

    fn update_tool_definitions(&mut self, tokens: u32) {
        self.tool_definition_tokens = tokens;
    }

    fn update_messages(&mut self, tokens: u32) {
        self.message_tokens = tokens;
    }
}
```

The `response_reserve` is often overlooked. If you fill the context window to 100% with input tokens, the model has zero space for its response. You must always hold back enough tokens for the expected response. For coding agents, 8,192 tokens is a reasonable default -- that's roughly 400 lines of code or a detailed explanation with examples.

::: python Coming from Python
In Python, you might track this with a simple integer variable and `if` checks. The Rust approach of bundling the budget into a struct with methods ensures the calculations are always consistent. You can't accidentally check `remaining_tokens` using one formula in one place and a different formula somewhere else -- the logic lives in exactly one method.
:::

## Compaction Thresholds

When should you trigger compaction? The naive answer is "when you're out of space," but that's too late. Compaction itself takes time -- especially summarization-based compaction, which requires an LLM call. You need to trigger compaction while you still have enough headroom to finish the current operation.

```rust
#[derive(Debug, Clone)]
struct CompactionPolicy {
    /// Trigger compaction when utilization exceeds this percentage
    soft_threshold_percent: f32,
    /// Force compaction immediately at this percentage (even if mid-operation)
    hard_threshold_percent: f32,
    /// Target utilization after compaction
    target_percent: f32,
    /// Minimum messages to keep after compaction
    min_messages_retained: usize,
}

impl CompactionPolicy {
    fn balanced() -> Self {
        Self {
            soft_threshold_percent: 75.0,
            hard_threshold_percent: 90.0,
            target_percent: 50.0,
            min_messages_retained: 10,
        }
    }

    fn aggressive() -> Self {
        Self {
            soft_threshold_percent: 50.0,
            hard_threshold_percent: 75.0,
            target_percent: 30.0,
            min_messages_retained: 5,
        }
    }

    fn conservative() -> Self {
        Self {
            soft_threshold_percent: 85.0,
            hard_threshold_percent: 95.0,
            target_percent: 65.0,
            min_messages_retained: 20,
        }
    }
}

enum CompactionDecision {
    /// No compaction needed
    None,
    /// Compaction recommended but not urgent
    Suggested { tokens_to_free: u32 },
    /// Compaction required before next API call
    Required { tokens_to_free: u32 },
}

impl TokenBudgetTracker {
    fn evaluate_compaction(&self, policy: &CompactionPolicy) -> CompactionDecision {
        let utilization = self.utilization_percent();

        if utilization >= policy.hard_threshold_percent {
            let target_tokens = (self.config.available_input_tokens() as f32
                * policy.target_percent / 100.0) as u32;
            let tokens_to_free = self.total_input_tokens()
                .saturating_sub(target_tokens);
            CompactionDecision::Required { tokens_to_free }
        } else if utilization >= policy.soft_threshold_percent {
            let target_tokens = (self.config.available_input_tokens() as f32
                * policy.target_percent / 100.0) as u32;
            let tokens_to_free = self.total_input_tokens()
                .saturating_sub(target_tokens);
            CompactionDecision::Suggested { tokens_to_free }
        } else {
            CompactionDecision::None
        }
    }
}
```

Three thresholds define the policy:

**Soft threshold (75%)**: Compaction is suggested. The agent can finish its current tool execution or response, then compact before the next LLM call. This gives you the most graceful compaction -- you have time to summarize rather than just truncate.

**Hard threshold (90%)**: Compaction is required immediately. The next API call will fail if you don't compact now. At this level, you might fall back to fast truncation rather than waiting for a summarization call.

**Target (50%)**: After compaction, aim for this utilization level. You don't want to compact to 89% and immediately trigger another compaction cycle. Targeting 50% gives you runway for several more turns before the next compaction.

## The Pre-Call Check

Every LLM call should go through a pre-call check that evaluates the budget and decides whether to proceed, compact, or reject:

```rust
enum PreCallAction {
    /// Budget is fine, proceed with the API call
    Proceed,
    /// Compact first, then retry the API call
    CompactFirst {
        strategy: CompactionStrategy,
        tokens_to_free: u32,
    },
    /// Cannot fit even with compaction (e.g., single message too large)
    Reject { reason: String },
}

#[derive(Debug)]
enum CompactionStrategy {
    SlidingWindow,
    Summarize,
    Hybrid,
}

fn pre_call_check(
    tracker: &TokenBudgetTracker,
    policy: &CompactionPolicy,
    message_history: &MessageHistory,
) -> PreCallAction {
    let decision = tracker.evaluate_compaction(policy);

    match decision {
        CompactionDecision::None => PreCallAction::Proceed,

        CompactionDecision::Suggested { tokens_to_free } => {
            // Can we summarize? That gives better context retention
            let strategy = if tokens_to_free > 10_000 {
                CompactionStrategy::Hybrid
            } else {
                CompactionStrategy::SlidingWindow
            };
            PreCallAction::CompactFirst {
                strategy,
                tokens_to_free,
            }
        }

        CompactionDecision::Required { tokens_to_free } => {
            // Check if compaction can actually free enough tokens
            let compactable_tokens = estimate_compactable_tokens(
                tracker, message_history,
            );

            if compactable_tokens < tokens_to_free {
                PreCallAction::Reject {
                    reason: format!(
                        "Need to free {} tokens but only {} are compactable. \
                         System prompt ({}) + tool definitions ({}) leave \
                         insufficient space for conversation.",
                        tokens_to_free,
                        compactable_tokens,
                        tracker.system_prompt_tokens,
                        tracker.tool_definition_tokens,
                    ),
                }
            } else {
                // Use fast truncation for required compaction
                PreCallAction::CompactFirst {
                    strategy: CompactionStrategy::SlidingWindow,
                    tokens_to_free,
                }
            }
        }
    }
}

fn estimate_compactable_tokens(
    tracker: &TokenBudgetTracker,
    history: &MessageHistory,
) -> u32 {
    // Everything except the system prompt, tool defs, and minimum retained messages
    let min_message_tokens: u32 = history.last_n(10)
        .map(|m| m.token_count.unwrap_or(0))
        .sum();
    tracker.message_tokens.saturating_sub(min_message_tokens)
}
```

The `Reject` case handles an important edge condition: when the system prompt and tool definitions alone consume so much of the context window that even deleting all conversation messages won't help. This happens when you have many tools with large schemas, or when the system prompt has grown to include extensive project context.

::: tip In the Wild
Claude Code implements a multi-tier context management strategy. It monitors token usage after each API response using the `usage` field, and when approaching the limit it first tries to drop cached tool outputs that are no longer relevant, then falls back to summarization of older conversation segments. The compaction process preserves the system prompt, the most recent user message, and any tool calls that are still in progress. OpenCode uses a simpler approach: when context is full, it truncates from the front while preserving the first and last N messages, inserting a "[earlier messages truncated]" marker.
:::

## Handling Fixed Overhead

System prompts and tool definitions are "fixed costs" that shrink the space available for conversation. As these grow, conversation capacity shrinks. Track this relationship explicitly:

```rust
impl TokenBudgetTracker {
    fn conversation_capacity(&self) -> u32 {
        self.config.available_input_tokens()
            .saturating_sub(self.system_prompt_tokens)
            .saturating_sub(self.tool_definition_tokens)
            .saturating_sub(self.framing_tokens)
    }

    fn print_budget_report(&self) {
        let available = self.config.available_input_tokens();
        println!("Context Window Budget Report");
        println!("============================");
        println!("Model capacity:     {:>8} tokens", self.config.max_context_tokens);
        println!("Response reserve:   {:>8} tokens", self.config.response_reserve);
        println!("Safety margin:      {:>8} tokens", self.config.safety_margin);
        println!("Available for input:{:>8} tokens", available);
        println!("----------------------------");
        println!("System prompt:      {:>8} tokens ({:.1}%)",
            self.system_prompt_tokens,
            self.system_prompt_tokens as f32 / available as f32 * 100.0);
        println!("Tool definitions:   {:>8} tokens ({:.1}%)",
            self.tool_definition_tokens,
            self.tool_definition_tokens as f32 / available as f32 * 100.0);
        println!("Messages:           {:>8} tokens ({:.1}%)",
            self.message_tokens,
            self.message_tokens as f32 / available as f32 * 100.0);
        println!("----------------------------");
        println!("Remaining:          {:>8} tokens ({:.1}%)",
            self.remaining_tokens(),
            (self.remaining_tokens() as f32 / available as f32) * 100.0);
    }
}
```

Running this on a typical Claude Code-style agent might produce:

```
Context Window Budget Report
============================
Model capacity:       200000 tokens
Response reserve:       8192 tokens
Safety margin:           500 tokens
Available for input:  191308 tokens
----------------------------
System prompt:         12000 tokens (6.3%)
Tool definitions:       4500 tokens (2.4%)
Messages:              98000 tokens (51.2%)
----------------------------
Remaining:             76808 tokens (40.1%)
```

When the system prompt alone takes 6% and tool definitions take another 2.4%, you've already used nearly 9% of your budget before the first message. In Chapter 11 (System Prompt Evolution), you'll learn techniques for keeping the system prompt lean as it accumulates project context.

## Dynamic Response Reserve

The static 8,192-token response reserve is a blunt instrument. Some turns need more space (generating a large function), some need less (a yes/no answer). You can make this adaptive:

```rust
impl ContextWindowConfig {
    fn dynamic_response_reserve(&self, recent_response_tokens: &[u32]) -> u32 {
        if recent_response_tokens.is_empty() {
            return self.response_reserve; // Fall back to default
        }

        // Use the 90th percentile of recent response sizes, with a floor
        let mut sorted = recent_response_tokens.to_vec();
        sorted.sort();
        let p90_index = (sorted.len() as f32 * 0.9) as usize;
        let p90 = sorted.get(p90_index).copied().unwrap_or(self.response_reserve);

        // Add 50% headroom above the p90
        let dynamic = (p90 as f32 * 1.5) as u32;

        // Clamp between reasonable bounds
        dynamic.clamp(2_048, self.max_context_tokens / 4)
    }
}
```

This tracks the 90th percentile of recent response sizes and reserves 1.5x that amount. Over time, if the agent is consistently producing 2,000-token responses, you only reserve 3,000 instead of 8,192 -- freeing up 5,000 tokens for conversation context.

## Key Takeaways

- Treat the context window as a budget with explicit allocations for system prompt, tool definitions, message history, response reserve, and safety margin.
- Use a three-threshold compaction policy: soft (start summarizing), hard (must truncate now), and target (compact down to this level to avoid thrashing).
- Every LLM call must pass through a pre-call check that either proceeds, triggers compaction, or rejects the call if the request is unfittable.
- Fixed overhead (system prompt + tool definitions) silently reduces your conversation capacity -- monitor it and optimize when it exceeds 10% of the window.
- Adapt the response reserve dynamically based on recent response sizes to avoid wasting context space on over-provisioned response budgets.
