---
title: Context Compaction Strategies
description: Implement strategies for reducing context size when approaching window limits, including sliding windows, priority pruning, and hybrid approaches.
---

# Context Compaction Strategies

> **What you'll learn:**
> - How sliding window compaction drops the oldest messages while preserving recent context
> - How priority-based pruning keeps high-value messages like tool results and key decisions
> - How to combine multiple strategies into a hybrid compaction pipeline

Your budget tracker says the context is at 85% and climbing. What do you do? This subchapter gives you concrete strategies for shrinking the conversation history while preserving the information the model needs to keep working effectively. There is no single best strategy -- you will build several and combine them.

## Strategy 1: Sliding Window

The simplest compaction strategy is to drop the oldest messages, keeping only a fixed-size window of recent conversation. This works because recent context is almost always more relevant than old context.

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    id: u64,
    role: String,
    content: String,
    token_count: usize,
    is_pinned: bool,
}

/// Sliding window compaction: keep the most recent messages
/// that fit within the token budget.
fn sliding_window(
    messages: &[Message],
    token_budget: usize,
) -> Vec<Message> {
    // Always keep pinned messages (system prompt, etc.)
    let pinned: Vec<&Message> = messages.iter()
        .filter(|m| m.is_pinned)
        .collect();
    let pinned_tokens: usize = pinned.iter().map(|m| m.token_count).sum();

    let remaining_budget = token_budget.saturating_sub(pinned_tokens);

    // Walk backwards through non-pinned messages, accumulating until budget is full
    let non_pinned: Vec<&Message> = messages.iter()
        .filter(|m| !m.is_pinned)
        .collect();

    let mut kept_indices = Vec::new();
    let mut used = 0;

    for (i, msg) in non_pinned.iter().enumerate().rev() {
        if used + msg.token_count <= remaining_budget {
            used += msg.token_count;
            kept_indices.push(i);
        } else {
            break; // Stop once budget is exhausted
        }
    }

    kept_indices.reverse(); // Restore chronological order

    // Combine pinned messages with the kept window
    let mut result: Vec<Message> = pinned.into_iter().cloned().collect();
    for i in kept_indices {
        result.push(non_pinned[i].clone());
    }

    result
}

fn main() {
    let messages = vec![
        Message { id: 1, role: "system".into(), content: "You are a coding assistant.".into(), token_count: 8, is_pinned: true },
        Message { id: 2, role: "user".into(), content: "Read file A".into(), token_count: 100, is_pinned: false },
        Message { id: 3, role: "assistant".into(), content: "Here is file A...".into(), token_count: 500, is_pinned: false },
        Message { id: 4, role: "user".into(), content: "Read file B".into(), token_count: 100, is_pinned: false },
        Message { id: 5, role: "assistant".into(), content: "Here is file B...".into(), token_count: 500, is_pinned: false },
        Message { id: 6, role: "user".into(), content: "Now fix the bug in file B".into(), token_count: 50, is_pinned: false },
        Message { id: 7, role: "assistant".into(), content: "I'll fix the bug...".into(), token_count: 200, is_pinned: false },
    ];

    let total: usize = messages.iter().map(|m| m.token_count).sum();
    println!("Total tokens: {}", total);

    let compacted = sliding_window(&messages, 800);
    let kept: usize = compacted.iter().map(|m| m.token_count).sum();
    println!("After compaction (budget 800): {} messages, {} tokens",
        compacted.len(), kept);
    for msg in &compacted {
        println!("  [{}] {}: {} ({} tokens)",
            if msg.is_pinned { "P" } else { " " },
            msg.role, &msg.content[..30.min(msg.content.len())], msg.token_count);
    }
}
```

Sliding window is fast and predictable, but it has a major flaw: it discards everything beyond the window, regardless of importance. A critical user instruction from 20 messages ago gets dropped just as easily as a verbose tool output.

## Strategy 2: Priority-Based Pruning

Priority-based pruning uses the priority levels from your message data structure to make smarter decisions about what to drop:

```rust
/// Priority-based compaction: remove lowest-priority messages first,
/// preserving high-value context regardless of age.
fn priority_prune(
    messages: &[Message],
    token_budget: usize,
    priorities: &[u8], // Priority for each message (higher = more important)
) -> Vec<Message> {
    let total: usize = messages.iter().map(|m| m.token_count).sum();
    if total <= token_budget {
        return messages.to_vec(); // No compaction needed
    }

    let tokens_to_free = total - token_budget;

    // Build a list of (index, priority, token_count) sorted by priority ascending
    // (lowest priority gets removed first)
    let mut candidates: Vec<(usize, u8, usize)> = messages.iter()
        .enumerate()
        .zip(priorities.iter())
        .filter(|((_, msg), _)| !msg.is_pinned)
        .map(|((i, msg), &prio)| (i, prio, msg.token_count))
        .collect();

    // Sort by priority ascending, then by age ascending (oldest first)
    candidates.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));

    // Remove candidates until we have freed enough tokens
    let mut removed = std::collections::HashSet::new();
    let mut freed = 0;

    for (idx, _prio, tokens) in &candidates {
        if freed >= tokens_to_free {
            break;
        }
        removed.insert(*idx);
        freed += tokens;
    }

    // Return messages that were not removed, preserving order
    messages.iter()
        .enumerate()
        .filter(|(i, _)| !removed.contains(i))
        .map(|(_, m)| m.clone())
        .collect()
}

fn main() {
    let messages = vec![
        Message { id: 1, role: "system".into(), content: "You are a coding assistant.".into(), token_count: 8, is_pinned: true },
        Message { id: 2, role: "user".into(), content: "Important: always use error handling".into(), token_count: 100, is_pinned: false },
        Message { id: 3, role: "tool".into(), content: "[verbose file content: 500 lines]".into(), token_count: 2000, is_pinned: false },
        Message { id: 4, role: "assistant".into(), content: "I see the file uses unwrap()...".into(), token_count: 300, is_pinned: false },
        Message { id: 5, role: "tool".into(), content: "[another verbose file read]".into(), token_count: 1500, is_pinned: false },
        Message { id: 6, role: "user".into(), content: "Fix the error handling in auth.rs".into(), token_count: 50, is_pinned: false },
    ];

    // Priority: 3=pinned, 2=user msgs, 1=assistant, 0=tool results
    let priorities = vec![3, 2, 0, 1, 0, 2];

    let compacted = priority_prune(&messages, 500, &priorities);
    println!("After priority pruning (budget 500):");
    for msg in &compacted {
        println!("  {}: {} ({} tokens)",
            msg.role, &msg.content[..40.min(msg.content.len())], msg.token_count);
    }
}
```

Priority pruning correctly removes the 2,000-token tool result before removing the 50-token user instruction, even though the tool result is newer. This is much better for a coding agent where user instructions and key decisions are more valuable than raw file contents.

## Strategy 3: Tool Result Truncation

Tool results are often the largest messages in a conversation. Rather than removing them entirely, you can truncate them to keep a summary of what was there:

```rust
/// Truncate tool results to a maximum token count, preserving
/// the beginning and end of the content with an elision marker.
fn truncate_tool_results(
    messages: &mut Vec<Message>,
    max_tool_tokens: usize,
) -> usize {
    let mut freed = 0;

    for msg in messages.iter_mut() {
        if msg.role == "tool" && msg.token_count > max_tool_tokens {
            let original_tokens = msg.token_count;

            // Keep the first and last portions of the content
            let lines: Vec<&str> = msg.content.lines().collect();
            if lines.len() > 10 {
                let keep_start = 5;
                let keep_end = 5;
                let removed_count = lines.len() - keep_start - keep_end;

                let mut truncated = String::new();
                for line in &lines[..keep_start] {
                    truncated.push_str(line);
                    truncated.push('\n');
                }
                truncated.push_str(&format!(
                    "\n[... {} lines truncated ...]\n\n",
                    removed_count
                ));
                for line in &lines[lines.len() - keep_end..] {
                    truncated.push_str(line);
                    truncated.push('\n');
                }

                msg.content = truncated;
                // Rough estimate of new token count
                msg.token_count = max_tool_tokens;
                freed += original_tokens - max_tool_tokens;
            }
        }
    }

    freed
}

fn main() {
    let mut messages = vec![
        Message {
            id: 1,
            role: "tool".into(),
            content: (0..50).map(|i| format!("line {}: some code here", i)).collect::<Vec<_>>().join("\n"),
            token_count: 500,
            is_pinned: false,
        },
    ];

    println!("Before truncation: {} tokens", messages[0].token_count);
    let freed = truncate_tool_results(&mut messages, 100);
    println!("After truncation: {} tokens (freed {})", messages[0].token_count, freed);
    println!("Content preview:\n{}", &messages[0].content[..200.min(messages[0].content.len())]);
}
```

::: python Coming from Python
In Python, you might truncate with simple string slicing:
```python
def truncate_tool_result(msg, max_chars=1000):
    if len(msg["content"]) > max_chars:
        msg["content"] = (msg["content"][:500]
                         + "\n\n[... truncated ...]\n\n"
                         + msg["content"][-500:])
```
The Rust version works with lines rather than raw character slices to avoid
splitting in the middle of a UTF-8 character or a line of code. This matters
for code content where a mid-line split would be confusing to the model.
:::

## Strategy 4: Hybrid Pipeline

The real power comes from combining strategies into a pipeline. Each strategy handles a different type of waste:

```rust
/// A compaction pipeline that applies strategies in order of
/// increasing aggressiveness.
pub struct CompactionPipeline {
    /// Maximum tokens for tool results before truncation
    tool_truncation_limit: usize,
    /// Target utilization after compaction (0.0 to 1.0)
    target_utilization: f64,
}

impl CompactionPipeline {
    pub fn new() -> Self {
        Self {
            tool_truncation_limit: 200,
            target_utilization: 0.70,
        }
    }

    /// Run the full compaction pipeline. Returns the number of tokens freed.
    pub fn compact(
        &self,
        messages: &mut Vec<Message>,
        token_budget: usize,
    ) -> CompactionResult {
        let initial_tokens: usize = messages.iter().map(|m| m.token_count).sum();
        let target_tokens = (token_budget as f64 * self.target_utilization) as usize;

        if initial_tokens <= target_tokens {
            return CompactionResult {
                tokens_freed: 0,
                messages_removed: 0,
                messages_truncated: 0,
                strategy_used: "none".to_string(),
            };
        }

        let mut total_freed = 0;
        let mut total_removed = 0;
        let mut total_truncated = 0;
        let mut strategy = Vec::new();

        // Stage 1: Truncate tool results
        let freed = truncate_tool_results(messages, self.tool_truncation_limit);
        if freed > 0 {
            total_freed += freed;
            total_truncated += messages.iter().filter(|m| m.role == "tool").count();
            strategy.push("tool_truncation");
        }

        let current: usize = messages.iter().map(|m| m.token_count).sum();
        if current <= target_tokens {
            return CompactionResult {
                tokens_freed: total_freed,
                messages_removed: total_removed,
                messages_truncated: total_truncated,
                strategy_used: strategy.join(" + "),
            };
        }

        // Stage 2: Remove low-priority messages (tool results that have been truncated)
        let before = messages.len();
        messages.retain(|m| m.is_pinned || m.role != "tool" || m.token_count <= 50);
        let removed = before - messages.len();
        let new_total: usize = messages.iter().map(|m| m.token_count).sum();
        total_freed += current.saturating_sub(new_total);
        total_removed += removed;
        if removed > 0 {
            strategy.push("low_priority_removal");
        }

        let current: usize = messages.iter().map(|m| m.token_count).sum();
        if current <= target_tokens {
            return CompactionResult {
                tokens_freed: total_freed,
                messages_removed: total_removed,
                messages_truncated: total_truncated,
                strategy_used: strategy.join(" + "),
            };
        }

        // Stage 3: Sliding window on remaining messages
        let window = sliding_window(messages, target_tokens);
        let window_removed = messages.len() - window.len();
        let window_tokens: usize = window.iter().map(|m| m.token_count).sum();
        total_freed += current.saturating_sub(window_tokens);
        total_removed += window_removed;
        *messages = window;
        if window_removed > 0 {
            strategy.push("sliding_window");
        }

        CompactionResult {
            tokens_freed: total_freed,
            messages_removed: total_removed,
            messages_truncated: total_truncated,
            strategy_used: strategy.join(" + "),
        }
    }
}

/// Result of a compaction operation for logging and diagnostics.
#[derive(Debug)]
pub struct CompactionResult {
    pub tokens_freed: usize,
    pub messages_removed: usize,
    pub messages_truncated: usize,
    pub strategy_used: String,
}

fn main() {
    let mut messages = vec![
        Message { id: 1, role: "system".into(), content: "You are helpful.".into(), token_count: 5, is_pinned: true },
        Message { id: 2, role: "user".into(), content: "Read file A".into(), token_count: 50, is_pinned: false },
        Message { id: 3, role: "tool".into(), content: "Very long file content...".into(), token_count: 3000, is_pinned: false },
        Message { id: 4, role: "assistant".into(), content: "File A contains...".into(), token_count: 200, is_pinned: false },
        Message { id: 5, role: "user".into(), content: "Read file B".into(), token_count: 50, is_pinned: false },
        Message { id: 6, role: "tool".into(), content: "Another long file...".into(), token_count: 2500, is_pinned: false },
        Message { id: 7, role: "assistant".into(), content: "File B shows...".into(), token_count: 300, is_pinned: false },
        Message { id: 8, role: "user".into(), content: "Fix the bug on line 42".into(), token_count: 30, is_pinned: false },
    ];

    let total: usize = messages.iter().map(|m| m.token_count).sum();
    println!("Before compaction: {} messages, {} tokens", messages.len(), total);

    let pipeline = CompactionPipeline::new();
    let result = pipeline.compact(&mut messages, 1000);

    let after: usize = messages.iter().map(|m| m.token_count).sum();
    println!("After compaction: {} messages, {} tokens", messages.len(), after);
    println!("Result: {:?}", result);
}
```

The pipeline applies strategies from least to most aggressive. It stops as soon as the target utilization is reached, which means most compaction cycles only need the first stage (tool truncation).

::: wild In the Wild
Claude Code uses a multi-stage compaction pipeline similar to what we built here. It first truncates large tool results, then removes old tool outputs entirely, and finally falls back to summarization (covered in the next subchapter) if more aggressive compaction is needed. The key insight is that tool results are the biggest source of token waste -- they are often large and become irrelevant quickly once the agent has acted on them.
:::

## Key Takeaways

- Sliding window compaction is simple and fast but does not respect message importance -- use it as a last resort, not a first choice
- Priority-based pruning removes the least valuable messages first, preserving user instructions and key decisions
- Tool result truncation is the highest-value compaction strategy because tool outputs are typically the largest and least durable messages
- Build a multi-stage pipeline that applies strategies from least to most aggressive, stopping as soon as the target utilization is reached
- Return detailed compaction results for logging and diagnostics -- you need to know what was removed to debug context management issues
