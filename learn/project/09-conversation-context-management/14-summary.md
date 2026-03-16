---
title: Summary
description: Review the complete context management system and reflect on the trade-offs between context richness and efficiency.
---

# Summary

> **What you'll learn:**
> - How all context management components work together to maximize effective context usage
> - Which compaction strategies work best for different conversation patterns and session lengths
> - What metrics to track to understand and optimize context management in production

You have built a complete context management system over the last 13 subchapters. Let's step back and see how all the pieces fit together, review the key trade-offs, and discuss how to measure whether your context management is working well in practice.

## The Complete Architecture

Here is how every component connects in your agent's request cycle:

```rust
use std::path::PathBuf;

/// This struct represents the complete context management system
/// and shows how every component from this chapter connects.
pub struct ContextManager {
    /// Token counting (Subchapter 2)
    token_counter: TokenCounter,
    /// Budget tracking (Subchapter 3)
    budget: ContextBudget,
    /// Message history (Subchapter 4)
    history: ConversationHistory,
    /// Session persistence (Subchapter 5)
    session_store: SessionStore,
    /// System prompt builder (Subchapter 9)
    system_prompt: SystemPromptBuilder,
    /// Compaction pipeline (Subchapter 7)
    compaction: CompactionPipeline,
    /// Summarizer (Subchapter 8)
    summarizer: Summarizer,
    /// Auto-save policy (Subchapter 5)
    auto_save: AutoSavePolicy,
}

/// Simplified stand-ins for the types we built throughout the chapter.
/// In your real agent, these are the full implementations from each subchapter.
struct TokenCounter;
impl TokenCounter {
    fn count(&self, text: &str) -> usize { (text.len() as f64 * 0.3) as usize }
}

struct ContextBudget { used: usize, limit: usize }
impl ContextBudget {
    fn utilization(&self) -> f64 { self.used as f64 / self.limit as f64 }
    fn needs_compaction(&self) -> bool { self.utilization() > 0.80 }
    fn update(&mut self, tokens: usize) { self.used = tokens; }
}

struct ConversationHistory { messages: Vec<(String, String, usize)>, total: usize }
impl ConversationHistory {
    fn push(&mut self, role: &str, content: &str, tokens: usize) {
        self.messages.push((role.into(), content.into(), tokens));
        self.total += tokens;
    }
    fn total_tokens(&self) -> usize { self.total }
    fn len(&self) -> usize { self.messages.len() }
}

struct SessionStore;
impl SessionStore {
    fn save(&self, _history: &ConversationHistory) -> Result<(), String> { Ok(()) }
}

struct SystemPromptBuilder;
impl SystemPromptBuilder {
    fn build(&self, _budget: usize) -> (String, usize) {
        ("You are a coding assistant.".into(), 8)
    }
}

struct CompactionPipeline;
impl CompactionPipeline {
    fn compact(&self, _history: &mut ConversationHistory, _budget: usize) -> usize { 0 }
}

struct Summarizer;
struct AutoSavePolicy { messages_since_save: usize }
impl AutoSavePolicy {
    fn should_save(&self) -> bool { self.messages_since_save >= 5 }
    fn did_save(&mut self) { self.messages_since_save = 0; }
    fn on_message(&mut self) { self.messages_since_save += 1; }
}

impl ContextManager {
    /// The main flow for processing a new message through the context system.
    /// This is the entry point that orchestrates all components.
    pub fn process_message(
        &mut self,
        role: &str,
        content: &str,
    ) -> Result<ContextStatus, String> {
        // Step 1: Count tokens (Subchapter 2)
        let tokens = self.token_counter.count(content);

        // Step 2: Add to history (Subchapter 4)
        self.history.push(role, content, tokens);

        // Step 3: Update budget (Subchapter 3)
        self.budget.update(self.history.total_tokens());

        // Step 4: Check if compaction is needed (Subchapter 7)
        let compacted = if self.budget.needs_compaction() {
            let freed = self.compaction.compact(
                &mut self.history,
                self.budget.limit,
            );
            self.budget.update(self.history.total_tokens());
            freed > 0
        } else {
            false
        };

        // Step 5: Auto-save check (Subchapter 5)
        self.auto_save.on_message();
        if self.auto_save.should_save() {
            self.session_store.save(&self.history)?;
            self.auto_save.did_save();
        }

        // Step 6: Build system prompt for next API call (Subchapter 9)
        let remaining_budget = self.budget.limit - self.history.total_tokens();
        let (_system_prompt, prompt_tokens) = self.system_prompt.build(remaining_budget);

        Ok(ContextStatus {
            total_tokens: self.history.total_tokens() + prompt_tokens,
            utilization: self.budget.utilization(),
            message_count: self.history.len(),
            compacted_this_turn: compacted,
        })
    }
}

/// Status report after processing a message.
#[derive(Debug)]
pub struct ContextStatus {
    pub total_tokens: usize,
    pub utilization: f64,
    pub message_count: usize,
    pub compacted_this_turn: bool,
}

fn main() {
    let mut cm = ContextManager {
        token_counter: TokenCounter,
        budget: ContextBudget { used: 0, limit: 200_000 },
        history: ConversationHistory { messages: Vec::new(), total: 0 },
        session_store: SessionStore,
        system_prompt: SystemPromptBuilder,
        compaction: CompactionPipeline,
        summarizer: Summarizer,
        auto_save: AutoSavePolicy { messages_since_save: 0 },
    };

    // Simulate a conversation
    let turns = vec![
        ("user", "Read the file src/auth.rs"),
        ("assistant", "I'll read that file for you."),
        ("tool", &"x".repeat(5000)), // Large tool result
        ("assistant", "The auth module has several functions..."),
        ("user", "Fix the password hashing"),
        ("assistant", "I'll update the verify_password function..."),
        ("tool", "File written successfully"),
        ("assistant", "Done! I've updated the password hashing to use bcrypt."),
    ];

    for (role, content) in &turns {
        match cm.process_message(role, content) {
            Ok(status) => {
                println!("[{:>10}] {:.1}% used, {} msgs{}",
                    role,
                    status.utilization * 100.0,
                    status.message_count,
                    if status.compacted_this_turn { " (compacted)" } else { "" },
                );
            }
            Err(e) => println!("Error: {}", e),
        }
    }
}
```

This is the flow for every message your agent processes. Each step is handled by a dedicated component, and the `ContextManager` orchestrates them in sequence. The key insight is that context management is not a single operation -- it is a pipeline that runs on every turn.

## Strategy Selection Guide

Different conversation patterns call for different compaction strategies. Here is a decision guide based on what you built:

| Scenario | Best Strategy | Why |
|----------|--------------|-----|
| Short sessions (<20 turns) | No compaction | Unlikely to hit limits |
| Tool-heavy sessions | Tool result truncation first | Tool outputs are the biggest tokens-per-message |
| Long exploratory sessions | Summarization | Old exploration is still partially relevant |
| Focused debugging sessions | Sliding window + pinning | Recent context is most relevant; pin key error messages |
| Multi-file refactoring | Priority pruning | Keep user instructions, drop old file reads |
| Cost-sensitive deployments | Aggressive truncation | Minimize tokens per request at all costs |

The hybrid pipeline you built in Subchapter 7 handles most cases well because it applies strategies in order of increasing aggressiveness. In practice, most compaction cycles only need the first stage (tool truncation), with summarization as a rare fallback.

## Metrics to Track

You cannot improve what you do not measure. Here are the key metrics for context management:

```rust
/// Metrics collected across the lifetime of a session.
#[derive(Debug, Default)]
pub struct ContextMetrics {
    /// Peak token usage (high water mark)
    pub peak_tokens: usize,
    /// Number of compaction events
    pub compaction_count: u32,
    /// Total tokens freed by compaction
    pub tokens_freed: usize,
    /// Number of summarization API calls
    pub summarization_count: u32,
    /// Tokens spent on summarization calls (input + output)
    pub summarization_tokens_spent: usize,
    /// Average utilization across all turns
    pub utilization_samples: Vec<f64>,
    /// Number of auto-saves performed
    pub auto_save_count: u32,
}

impl ContextMetrics {
    pub fn record_turn(&mut self, tokens: usize, utilization: f64) {
        self.peak_tokens = self.peak_tokens.max(tokens);
        self.utilization_samples.push(utilization);
    }

    pub fn record_compaction(&mut self, tokens_freed: usize) {
        self.compaction_count += 1;
        self.tokens_freed += tokens_freed;
    }

    pub fn record_summarization(&mut self, tokens_spent: usize) {
        self.summarization_count += 1;
        self.summarization_tokens_spent += tokens_spent;
    }

    pub fn average_utilization(&self) -> f64 {
        if self.utilization_samples.is_empty() {
            return 0.0;
        }
        self.utilization_samples.iter().sum::<f64>()
            / self.utilization_samples.len() as f64
    }

    /// Print an end-of-session report.
    pub fn report(&self) {
        println!("=== Context Management Report ===");
        println!("Peak usage:          {} tokens", self.peak_tokens);
        println!("Avg utilization:     {:.1}%", self.average_utilization() * 100.0);
        println!("Compaction events:   {}", self.compaction_count);
        println!("Tokens freed:        {}", self.tokens_freed);
        println!("Summarizations:      {}", self.summarization_count);
        println!("Summarization cost:  {} tokens", self.summarization_tokens_spent);
        println!("Auto-saves:          {}", self.auto_save_count);

        // Efficiency score: how much useful work per context token
        if self.tokens_freed > 0 {
            let efficiency = self.tokens_freed as f64
                / self.summarization_tokens_spent.max(1) as f64;
            println!("Compaction efficiency: {:.1}x (tokens freed per token spent)",
                efficiency);
        }
    }
}

fn main() {
    let mut metrics = ContextMetrics::default();

    // Simulate a 30-turn session
    for turn in 0..30 {
        let tokens = 5000 + turn * 3000;
        let util = tokens as f64 / 200_000.0;
        metrics.record_turn(tokens, util);

        if turn == 15 {
            metrics.record_compaction(40_000);
        }
        if turn == 22 {
            metrics.record_compaction(30_000);
            metrics.record_summarization(2_000);
        }
    }

    metrics.auto_save_count = 6;
    metrics.report();
}
```

The compaction efficiency metric is particularly useful: it tells you how many tokens you recovered per token you spent on summarization. A ratio above 10x means summarization is paying for itself handsomely. Below 3x, you might be summarizing too aggressively.

::: python Coming from Python
Python developers are used to relying on memory profilers like `memory_profiler`
or `tracemalloc` after the fact. Rust's approach of building metrics directly into
the data structures gives you real-time visibility without attaching external tools.
The `ContextMetrics` struct is always available, always up to date, and costs
almost nothing in terms of performance -- just a few integer increments per turn.
:::

## What You Built in This Chapter

Let's recap every component and where it lives in the codebase:

| Component | Subchapter | Purpose |
|-----------|-----------|---------|
| Token counter | 2 | Accurate BPE token measurement |
| Token budget | 3 | Real-time context budget tracking |
| Message history | 4 | O(1) token tracking, priority tagging |
| Session persistence | 5 | Atomic saves, auto-save policy |
| Serialization | 6 | JSON + MessagePack dual format |
| Compaction pipeline | 7 | Multi-stage context reduction |
| Summarization | 8 | LLM-powered context compression |
| System prompt | 9 | Layered, budget-aware prompt builder |
| Config files | 10 | Project-specific context injection |
| Forking | 11 | Tree-structured conversation branching |
| Multi-session | 12 | Independent parallel conversations |
| Memory management | 13 | String interning, lazy loading, buffer reuse |

These components work together as a pipeline that runs on every turn of the agentic loop. The result is an agent that can handle arbitrarily long coding sessions without degrading in quality, cost, or responsiveness.

::: wild In the Wild
Production coding agents like Claude Code and OpenCode implement all of these patterns to varying degrees. Claude Code's context management is particularly sophisticated -- it maintains a token budget that accounts for system prompts, tool definitions, and conversation history; it uses multi-stage compaction with truncation, pruning, and summarization; and it persists sessions for later resumption. The difference between a toy agent and a production agent is largely context management -- the agentic loop is the same, but how you manage the finite context window determines whether the agent works for 5 minutes or 5 hours.
:::

## Looking Ahead

With context management in place, your agent can handle long, complex coding sessions. In Chapter 10, you will add search and code intelligence -- the ability to find symbols, navigate definitions, and understand code structure. This will make your agent more efficient with its context budget, because it can find exactly the right piece of code instead of reading entire files hoping the relevant section is there.

## Exercises

Practice each concept with these exercises. They build on the context management system you created in this chapter.

### Exercise 1: Add a /context Status Command (Easy)

Implement a `/context` REPL command that displays the current context state: total tokens used, budget utilization percentage, message count, and number of compaction events so far. Format the output as a compact dashboard the user can glance at during a session.

- Read the current values from `ContextManager` fields: `budget.used`, `budget.limit`, `history.len()`
- Calculate utilization as `(used as f64 / limit as f64) * 100.0`
- Print a formatted summary like `[Context: 45,200/200,000 tokens (22.6%) | 12 messages | 0 compactions]`

### Exercise 2: Implement a Message Priority Tagger (Easy)

Add a `priority: MessagePriority` field to your message history entries with variants `Critical`, `Normal`, and `Low`. Tag user messages as `Critical`, assistant text as `Normal`, and tool results as `Low`. During compaction, drop `Low` priority messages first, then `Normal`, never `Critical`.

- Define `enum MessagePriority { Critical, Normal, Low }`
- Assign priority in `ConversationHistory::push()` based on the `role` parameter
- In the compaction pipeline, sort candidates by priority before pruning

### Exercise 3: Add a Token Budget Warning System (Medium)

Implement a warning system that alerts the user at configurable utilization thresholds (e.g., 60%, 80%, 95%). Each threshold should fire only once per session. At 95%, automatically trigger compaction and report how many tokens were freed.

**Hints:**
- Store `triggered_warnings: HashSet<u32>` in `ContextManager` to track which thresholds have fired
- Define thresholds as a `Vec<(u32, &str)>` like `[(60, "Moderate"), (80, "High"), (95, "Critical")]`
- After each `budget.update()`, check if utilization crossed a new threshold
- At the 95% threshold, call `self.compaction.compact()` and report the result

### Exercise 4: Implement Conversation Export (Medium)

Add a `/export` command that saves the current conversation to a JSON file. Include message roles, content, token counts, and metadata (session ID, timestamp, model used, total tokens). The file should be importable back into the agent with a `/import` command that restores the conversation state.

**Hints:**
- Define a `ConversationExport` struct with `serde::Serialize` and `Deserialize`
- Include a `version: u32` field in the export format for forward compatibility
- Use `serde_json::to_string_pretty()` for human-readable output
- On import, validate the version and recalculate token counts (they may differ if the token counter changed)

### Exercise 5: Build a Sliding Window with Pinned Messages (Hard)

Implement a sliding window compaction strategy that keeps the most recent N messages but allows specific messages to be "pinned" so they survive compaction. Add a `/pin` command that pins the last assistant message and a `/pins` command that lists all pinned messages. Pinned messages always remain in context regardless of the window size.

**Hints:**
- Add a `pinned: bool` field to your message history entries, defaulting to `false`
- The sliding window should partition messages into pinned and unpinned, then keep all pinned messages plus the most recent N unpinned messages
- Maintain the original message order after filtering (pinned messages stay in their original position)
- Set a maximum pin count (e.g., 10) to prevent users from pinning everything
- Write tests verifying that pinned messages survive compaction while unpinned messages beyond the window are dropped

## Key Takeaways

- Context management is a pipeline, not a single operation -- token counting, budget tracking, compaction, and persistence all run on every turn
- The hybrid compaction strategy (truncation, then priority pruning, then sliding window, then summarization) handles most conversation patterns well
- Track metrics like peak utilization, compaction count, and compaction efficiency to understand whether your context management is working
- The system prompt is your most expensive per-turn cost -- keep it under 3% of usable context through layered, budget-aware composition
- The difference between a demo agent and a production agent is context management -- the same agentic loop that works for 5-minute sessions breaks without it for 5-hour sessions
