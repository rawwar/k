---
title: Stop Conditions
description: Preventing infinite loops and runaway execution with well-designed stop conditions, iteration limits, and cost budgets.
---

# Stop Conditions

> **What you'll learn:**
> - The different types of stop conditions: model-initiated, iteration limits, cost caps, and user interrupts
> - How to set sensible defaults for maximum iterations and token budgets without being too restrictive
> - How to implement graceful degradation when stop conditions are hit mid-task

The agentic loop runs until the model says "I'm done." But what if the model never says that? What if it gets stuck in a cycle -- reading a file, editing it, running tests, seeing a failure, editing again, running tests again, failing again -- indefinitely? Without stop conditions, a coding agent can burn through your entire API budget in minutes, executing hundreds of LLM calls and tool invocations on a task that will never succeed.

Stop conditions are the safety rails of the agentic loop. They define the boundaries beyond which the agent must stop, regardless of what the model wants to do. Getting them right is a balancing act: too tight, and the agent cannot complete complex tasks; too loose, and you risk runaway execution.

## Types of Stop Conditions

There are four categories of stop conditions, each operating at a different level:

### 1. Model-Initiated Stop

The model produces an `end_turn` stop reason, indicating it believes the task is complete. This is the happy path -- the model decides it is done, and the loop ends naturally.

This is not really a "condition" you implement; it is the default way the loop ends. But it is worth listing because the other stop conditions exist precisely because the model's judgment is not always reliable. The model might think it needs "just one more tool call" indefinitely.

### 2. Iteration Limit

A hard cap on the number of inner loop iterations per user turn. Each iteration involves one LLM call and zero or more tool executions. When the limit is reached, the loop stops regardless of the model's intent.

```rust
struct LoopConfig {
    max_iterations: usize,
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
        }
    }
}

fn check_iteration_limit(
    current_iteration: usize,
    config: &LoopConfig,
) -> Result<(), StopCondition> {
    if current_iteration >= config.max_iterations {
        Err(StopCondition::IterationLimit {
            limit: config.max_iterations,
            reached: current_iteration,
        })
    } else {
        Ok(())
    }
}
```

What is a sensible default? It depends on the complexity of tasks your agent handles:

- **10 iterations** is sufficient for simple tasks (read a file, answer a question)
- **50 iterations** handles most real-world coding tasks (implement a feature, fix a bug)
- **100+ iterations** is needed for complex refactoring or multi-file changes

A default of 50 is reasonable for a general-purpose coding agent. Users should be able to override this with a flag or configuration.

### 3. Token Budget

A cap on total tokens consumed per turn (or per session). This directly controls cost:

```rust
struct TokenBudget {
    max_input_tokens_per_turn: u64,
    max_output_tokens_per_turn: u64,
    max_total_tokens_per_session: u64,
}

impl Default for TokenBudget {
    fn default() -> Self {
        Self {
            max_input_tokens_per_turn: 1_000_000,     // ~$3 at Sonnet pricing
            max_output_tokens_per_turn: 200_000,       // ~$3 at Sonnet pricing
            max_total_tokens_per_session: 10_000_000,  // ~$30 session budget
        }
    }
}

struct TokenTracker {
    turn_input_tokens: u64,
    turn_output_tokens: u64,
    session_total_tokens: u64,
}

impl TokenTracker {
    fn record_usage(&mut self, input: u64, output: u64) {
        self.turn_input_tokens += input;
        self.turn_output_tokens += output;
        self.session_total_tokens += input + output;
    }

    fn check_budget(&self, budget: &TokenBudget) -> Result<(), StopCondition> {
        if self.turn_input_tokens > budget.max_input_tokens_per_turn {
            return Err(StopCondition::TokenBudget {
                reason: format!(
                    "Input token limit exceeded: {} > {}",
                    self.turn_input_tokens, budget.max_input_tokens_per_turn
                ),
            });
        }
        if self.turn_output_tokens > budget.max_output_tokens_per_turn {
            return Err(StopCondition::TokenBudget {
                reason: format!(
                    "Output token limit exceeded: {} > {}",
                    self.turn_output_tokens, budget.max_output_tokens_per_turn
                ),
            });
        }
        if self.session_total_tokens > budget.max_total_tokens_per_session {
            return Err(StopCondition::TokenBudget {
                reason: format!(
                    "Session token limit exceeded: {} > {}",
                    self.session_total_tokens, budget.max_total_tokens_per_session
                ),
            });
        }
        Ok(())
    }

    fn reset_turn(&mut self) {
        self.turn_input_tokens = 0;
        self.turn_output_tokens = 0;
        // session_total_tokens persists
    }
}
```

Token budgets are especially important because agentic loops have a compounding cost problem. Each iteration adds more history to the context, so the input tokens grow with every call. A loop with 10 iterations does not cost 10x a single call -- it can cost 30x or more because of the growing context.

::: python Coming from Python
Python agent frameworks like LangChain and AutoGen provide similar budget controls. LangChain's `AgentExecutor` accepts `max_iterations` and `max_execution_time` parameters. The concepts are identical across languages. The Rust version benefits from compile-time type checking of the budget configuration, but the runtime behavior is the same: check the condition at the start of each iteration and stop if exceeded.
:::

### 4. User Interrupt

The user presses Ctrl+C to cancel the current operation. This must be handled at every point in the loop:

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn setup_interrupt_handler() -> Arc<AtomicBool> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancelled_clone = cancelled.clone();

    ctrlc::set_handler(move || {
        cancelled_clone.store(true, Ordering::SeqCst);
    })
    .expect("Failed to set Ctrl+C handler");

    cancelled
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<(), StopCondition> {
    if cancelled.load(Ordering::SeqCst) {
        Err(StopCondition::UserCancelled)
    } else {
        Ok(())
    }
}
```

User interrupts are different from the other stop conditions because they can arrive at any time, not just at iteration boundaries. During an LLM API call (which can take seconds), the user might press Ctrl+C. During a long-running tool execution (like `cargo build` on a large project), they might press Ctrl+C. Your code needs to check for interrupts at multiple points within each iteration.

## Integrating Stop Conditions into the Loop

All stop conditions converge on a single check function that runs at the beginning of each iteration:

```rust
enum StopCondition {
    ModelDone(String),
    IterationLimit { limit: usize, reached: usize },
    TokenBudget { reason: String },
    UserCancelled,
    ContextOverflow,
}

fn check_stop_conditions(
    iteration: usize,
    config: &LoopConfig,
    token_tracker: &TokenTracker,
    budget: &TokenBudget,
    cancelled: &AtomicBool,
) -> Result<(), StopCondition> {
    // Check in order of priority
    check_cancelled(cancelled)?;
    check_iteration_limit(iteration, config)?;
    token_tracker.check_budget(budget)?;
    Ok(())
}
```

This function is called at the top of the inner loop, before the LLM invocation:

```rust
fn run_inner_loop(
    state: &mut AgentContext,
    config: &LoopConfig,
    budget: &TokenBudget,
    cancelled: &AtomicBool,
) -> Result<String, StopCondition> {
    let mut iteration = 0;

    loop {
        // Check stop conditions before each iteration
        check_stop_conditions(
            iteration,
            config,
            &state.token_tracker,
            budget,
            cancelled,
        )?;

        // Call the LLM
        let response = call_llm(&state.history, &state.tools)
            .map_err(|e| StopCondition::from(e))?;

        // Track token usage
        if let Some(usage) = &response.usage {
            state.token_tracker.record_usage(
                usage.input_tokens as u64,
                usage.output_tokens as u64,
            );
        }

        // Check if model is done
        if response.stop_reason == "end_turn" {
            return Ok(response.text);
        }

        // Execute tools and continue
        let results = dispatch_tools(&response.tool_calls, &state.registry);
        collect_observations(&response.tool_calls, &results, &mut state.history);

        iteration += 1;
    }
}
```

## Graceful Degradation

When a stop condition fires, the agent should not just crash or print a cryptic error. It should gracefully present what it has accomplished so far and explain why it stopped:

```rust
fn handle_stop_condition(condition: StopCondition, state: &AgentContext) -> String {
    match condition {
        StopCondition::IterationLimit { limit, .. } => {
            format!(
                "I've reached the maximum number of iterations ({}) for this turn. \
                 Here's what I've accomplished so far:\n\n{}\n\n\
                 You can ask me to continue where I left off, or adjust the \
                 iteration limit with --max-iterations.",
                limit,
                summarize_progress(state)
            )
        }
        StopCondition::TokenBudget { reason } => {
            format!(
                "I've hit the token budget limit: {}\n\n\
                 Progress so far:\n{}\n\n\
                 You can increase the budget or ask me to continue \
                 with a fresh context.",
                reason,
                summarize_progress(state)
            )
        }
        StopCondition::UserCancelled => {
            format!(
                "Cancelled. Here's what I did before the interruption:\n\n{}",
                summarize_progress(state)
            )
        }
        StopCondition::ContextOverflow => {
            format!(
                "The conversation has exceeded the context window limit. \
                 I'll need to compact the history to continue.\n\n\
                 Progress so far:\n{}",
                summarize_progress(state)
            )
        }
        StopCondition::ModelDone(text) => text,
    }
}

fn summarize_progress(state: &AgentContext) -> String {
    let tool_calls: Vec<String> = state.turn_tracker.tool_calls
        .iter()
        .map(|tc| {
            let status = if tc.success { "done" } else { "failed" };
            format!("  - {} ({})", tc.name, status)
        })
        .collect();

    if tool_calls.is_empty() {
        "No actions were completed.".to_string()
    } else {
        format!("Actions taken:\n{}", tool_calls.join("\n"))
    }
}
```

The key principle is: **never lose work silently**. If the agent has made file edits, run commands, or gathered information before hitting a stop condition, the user should know about it. Otherwise they might re-run the same task, duplicating work or conflicting with changes already made.

::: tip In the Wild
Claude Code implements multiple stop conditions. It has a configurable token budget (shown in the `/cost` command output), it respects user interrupts via Ctrl+C at any point during execution, and it handles context overflow by offering to compact the conversation. When an iteration limit or budget is reached, it presents the partial progress and offers to continue. OpenCode similarly implements iteration limits and budget tracking, with its TUI displaying a running cost counter so users can see their budget consumption in real-time.
:::

## Tuning Stop Conditions

There is no universally correct set of stop condition thresholds. The right values depend on:

**The task type.** A quick question needs 2-3 iterations. A feature implementation needs 20-50. A large refactoring might need 100+. Consider providing task-specific overrides or letting the user set limits per request.

**The model.** More capable models tend to be more efficient (fewer iterations per task) but more expensive per iteration. The same token budget goes further with a cheaper model but might require more iterations.

**The user's tolerance.** Some users want to set it and forget it. Others want tight control. Provide sensible defaults but make everything configurable.

**The risk level.** Tasks that modify files or run destructive commands should have tighter limits as a safety measure. A bug in the loop that causes 100 file writes is much worse than one that causes 100 file reads.

A practical approach is to have tiered defaults:

```rust
struct StopConditionPresets;

impl StopConditionPresets {
    fn conservative() -> LoopConfig {
        LoopConfig { max_iterations: 10 }
    }

    fn standard() -> LoopConfig {
        LoopConfig { max_iterations: 50 }
    }

    fn permissive() -> LoopConfig {
        LoopConfig { max_iterations: 200 }
    }
}
```

## Key Takeaways

- Four types of stop conditions protect the agentic loop: model-initiated (end_turn), iteration limits, token budgets, and user interrupts -- each operates at a different level
- Iteration limits should default to around 50 for general-purpose coding agents, with user-configurable overrides for simple (10) or complex (200+) tasks
- Token budgets control cost and should account for the compounding effect of growing context: a 10-iteration loop can cost 30x a single call because input tokens grow each iteration
- User interrupts (Ctrl+C) must be checked at multiple points within each iteration, not just at iteration boundaries, since API calls and tool execution can take seconds
- When a stop condition fires, the agent must gracefully report what it accomplished before stopping -- never lose work silently
