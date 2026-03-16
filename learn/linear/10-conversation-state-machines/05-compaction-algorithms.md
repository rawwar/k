---
title: Compaction Algorithms
description: Algorithms for reducing conversation size while preserving essential context, including sliding window, importance scoring, and semantic deduplication approaches.
---

# Compaction Algorithms

> **What you'll learn:**
> - The sliding window approach: dropping oldest messages while preserving the system prompt and recent context
> - Importance scoring algorithms that rank messages by relevance to the current task using heuristics and embeddings
> - Semantic deduplication techniques that identify and merge redundant tool outputs, repeated file contents, and similar error messages

The context window manager from the previous subchapter decides *when* to compact and *how many tokens* to free. Now you need algorithms that decide *which* messages to remove. This is the hard part -- cut the wrong messages and the LLM loses critical context about what it's doing and why. Cut the right messages and the conversation continues seamlessly, as if nothing happened.

There's no single best algorithm. Different strategies trade off between speed, context quality, and implementation complexity. Let's build three approaches, from simplest to most sophisticated, so you can choose the right one for your situation.

## Sliding Window Compaction

The simplest compaction strategy: keep the most recent N messages and drop everything else. This works because LLM conversations have temporal locality -- recent messages are almost always more relevant than old ones.

```rust
struct SlidingWindowCompactor {
    /// Always preserve the system prompt (index 0)
    preserve_system: bool,
    /// Number of recent messages to always keep
    keep_recent: usize,
}

impl SlidingWindowCompactor {
    fn new(keep_recent: usize) -> Self {
        Self {
            preserve_system: true,
            keep_recent,
        }
    }

    fn compact(
        &self,
        history: &mut MessageHistory,
        tokens_to_free: u32,
    ) -> CompactionResult {
        let total_messages = history.len();
        if total_messages <= self.keep_recent + 1 {
            // Can't compact: too few messages
            return CompactionResult {
                messages_removed: 0,
                tokens_freed: 0,
                strategy: "sliding_window".into(),
            };
        }

        // Calculate the window of messages eligible for removal
        let start = if self.preserve_system { 1 } else { 0 };
        let end = total_messages - self.keep_recent;

        // Walk forward from start, accumulating tokens until we've freed enough
        let mut tokens_freed = 0u32;
        let mut remove_count = 0;

        for i in start..end {
            if let Some(msg) = history.messages.get(i) {
                tokens_freed += msg.token_count.unwrap_or(0);
                remove_count += 1;
                if tokens_freed >= tokens_to_free {
                    break;
                }
            }
        }

        // Perform the removal
        // First, handle system prompt preservation
        if self.preserve_system && start == 1 {
            // Remove messages from index 1 to (1 + remove_count)
            let removed: Vec<_> = history.messages.drain(1..1 + remove_count).collect();
            let actual_freed: u32 = removed.iter()
                .map(|m| m.token_count.unwrap_or(0))
                .sum();
            history.total_tokens -= actual_freed;
            history.rebuild_indexes();

            CompactionResult {
                messages_removed: removed.len(),
                tokens_freed: actual_freed,
                strategy: "sliding_window".into(),
            }
        } else {
            history.truncate_front(remove_count);
            CompactionResult {
                messages_removed: remove_count,
                tokens_freed,
                strategy: "sliding_window".into(),
            }
        }
    }
}

#[derive(Debug)]
struct CompactionResult {
    messages_removed: usize,
    tokens_freed: u32,
    strategy: String,
}
```

Sliding window is fast (O(n) in removed messages), simple to understand, and never fails. Its weakness is that it doesn't consider content -- a message from 30 turns ago that contains the project's architecture decision is just as expendable as a "yes, that looks good" from the same era.

::: python Coming from Python
In Python you'd implement this with list slicing: `messages = [messages[0]] + messages[-keep_recent:]`. Rust's `VecDeque::drain` is the equivalent, but it modifies the collection in place rather than creating a new one. This avoids the memory allocation overhead of creating a new list on every compaction.
:::

## Importance-Scored Compaction

Better compaction preserves high-value messages even if they're old, while aggressively removing low-value ones. The key is defining "importance" -- here's a heuristic scoring system that works well for coding agents:

```rust
struct ImportanceScorer;

impl ImportanceScorer {
    fn score(msg: &Message, position: usize, total: usize) -> f32 {
        let mut score = 0.0f32;

        // Recency boost: recent messages score higher
        let recency = position as f32 / total as f32; // 0.0 (oldest) to 1.0 (newest)
        score += recency * 3.0;

        // Role-based scoring
        match msg.role {
            Role::System => score += 100.0,  // Never remove system prompt
            Role::User => score += 2.0,      // User intent is important
            Role::Assistant => score += 1.5,  // Assistant reasoning
            Role::ToolCall => score += 1.0,   // Tool calls less important than results
            Role::ToolResult => score += 0.5, // Tool output is often bulky and redundant
        }

        // Content-based heuristics
        for block in &msg.content {
            match block {
                ContentBlock::Text(text) => {
                    // Messages containing file paths are often important context
                    if text.contains('/') && text.contains('.') {
                        score += 1.0;
                    }
                    // Error messages are high value -- they explain what went wrong
                    if text.to_lowercase().contains("error")
                        || text.to_lowercase().contains("failed") {
                        score += 1.5;
                    }
                    // Short confirmations are low value
                    if text.len() < 50 {
                        score -= 0.5;
                    }
                    // Very long messages are expensive to keep
                    let token_penalty = (msg.token_count.unwrap_or(0) as f32
                        / 1000.0).min(2.0);
                    score -= token_penalty * 0.3;
                }
                ContentBlock::ToolUse { name, .. } => {
                    // File-editing tools are more important than read-only tools
                    if name.contains("write") || name.contains("edit")
                        || name.contains("create") {
                        score += 2.0;
                    }
                }
                ContentBlock::ToolResult { is_error, content, .. } => {
                    if *is_error {
                        score += 2.0; // Error results are very important
                    }
                    // Huge tool outputs (like full file contents) are compaction targets
                    if content.len() > 5000 {
                        score -= 1.0;
                    }
                }
            }
        }

        score
    }
}

struct ImportanceCompactor {
    min_keep: usize,
}

impl ImportanceCompactor {
    fn compact(
        &self,
        history: &mut MessageHistory,
        tokens_to_free: u32,
    ) -> CompactionResult {
        let total = history.len();

        // Score all messages
        let mut scored: Vec<(usize, f32, u32)> = history.messages.iter()
            .enumerate()
            .map(|(i, msg)| {
                let score = ImportanceScorer::score(msg, i, total);
                let tokens = msg.token_count.unwrap_or(0);
                (i, score, tokens)
            })
            .collect();

        // Sort by score ascending (lowest importance first)
        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        // Select messages to remove until we free enough tokens
        let mut tokens_freed = 0u32;
        let mut indices_to_remove: Vec<usize> = Vec::new();

        for (idx, _score, tokens) in &scored {
            if tokens_freed >= tokens_to_free {
                break;
            }
            if total - indices_to_remove.len() <= self.min_keep {
                break; // Don't go below minimum
            }
            indices_to_remove.push(*idx);
            tokens_freed += tokens;
        }

        // Remove in reverse order to maintain valid indices
        indices_to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for idx in &indices_to_remove {
            history.messages.remove(*idx);
        }
        history.total_tokens -= tokens_freed;
        history.rebuild_indexes();

        CompactionResult {
            messages_removed: indices_to_remove.len(),
            tokens_freed,
            strategy: "importance_scored".into(),
        }
    }
}
```

The scoring system balances several factors: recency (recent messages score higher), role (system prompts are untouchable, tool results are cheap to discard), content heuristics (error messages are valuable, large file dumps are not), and token cost (removing a 5,000-token message is more efficient than removing five 100-token messages).

## Semantic Deduplication

Coding agent conversations are full of redundancy. The LLM reads a file, the user asks for a change, the LLM reads the same file again, makes an edit, reads it a third time to verify. Each read produces a near-identical tool output. Semantic deduplication identifies these redundancies:

```rust
struct SemanticDeduplicator {
    /// Similarity threshold (0.0 to 1.0) for considering messages duplicates
    similarity_threshold: f32,
}

impl SemanticDeduplicator {
    fn new(threshold: f32) -> Self {
        Self {
            similarity_threshold: threshold,
        }
    }

    /// Find groups of semantically similar tool results
    fn find_duplicate_groups(&self, history: &MessageHistory) -> Vec<DuplicateGroup> {
        let tool_results: Vec<(usize, &Message)> = history.messages.iter()
            .enumerate()
            .filter(|(_, msg)| msg.role == Role::ToolResult)
            .collect();

        let mut groups: Vec<DuplicateGroup> = Vec::new();
        let mut assigned: std::collections::HashSet<usize> = std::collections::HashSet::new();

        for (i, (idx_a, msg_a)) in tool_results.iter().enumerate() {
            if assigned.contains(idx_a) {
                continue;
            }

            let mut group = DuplicateGroup {
                indices: vec![*idx_a],
                representative: *idx_a,
                total_tokens: msg_a.token_count.unwrap_or(0),
            };

            for (idx_b, msg_b) in tool_results.iter().skip(i + 1) {
                if assigned.contains(idx_b) {
                    continue;
                }
                if self.messages_similar(msg_a, msg_b) {
                    group.indices.push(*idx_b);
                    group.total_tokens += msg_b.token_count.unwrap_or(0);
                    assigned.insert(*idx_b);
                }
            }

            if group.indices.len() > 1 {
                // Keep the most recent as representative
                group.representative = *group.indices.last().unwrap();
                assigned.extend(group.indices.iter());
                groups.push(group);
            }
        }

        groups
    }

    fn messages_similar(&self, a: &Message, b: &Message) -> bool {
        let text_a = self.extract_text(a);
        let text_b = self.extract_text(b);

        // Quick length check: if lengths differ by more than 20%, not duplicates
        let len_ratio = text_a.len().min(text_b.len()) as f32
            / text_a.len().max(text_b.len()).max(1) as f32;
        if len_ratio < 0.8 {
            return false;
        }

        // Simple Jaccard similarity on line sets
        let lines_a: std::collections::HashSet<&str> = text_a.lines().collect();
        let lines_b: std::collections::HashSet<&str> = text_b.lines().collect();

        let intersection = lines_a.intersection(&lines_b).count();
        let union = lines_a.union(&lines_b).count();

        if union == 0 {
            return false;
        }

        let similarity = intersection as f32 / union as f32;
        similarity >= self.similarity_threshold
    }

    fn extract_text(&self, msg: &Message) -> String {
        msg.content.iter().map(|block| match block {
            ContentBlock::Text(t) => t.clone(),
            ContentBlock::ToolResult { content, .. } => content.clone(),
            ContentBlock::ToolUse { input, .. } => input.to_string(),
        }).collect::<Vec<_>>().join("\n")
    }
}

#[derive(Debug)]
struct DuplicateGroup {
    /// Indices of duplicate messages in the history
    indices: Vec<usize>,
    /// Index of the message to keep
    representative: usize,
    /// Total tokens across all messages in the group
    total_tokens: u32,
}
```

When duplicates are found, you replace the group with only the most recent occurrence, optionally prefixing it with "[This content appeared N times during the conversation]". The token savings can be enormous -- in a file-editing workflow, the same 200-line file might appear 5 times in tool results, consuming 5,000+ tokens that could be reduced to 1,000.

::: wild In the Wild
Claude Code uses a multi-strategy approach to compaction. It first removes tool outputs that are exact or near-exact duplicates (like reading the same file multiple times), then applies summarization to older conversation segments. The system is careful to preserve the "chain of thought" -- user requests and the decisions made in response -- even when tool outputs are aggressively compacted. Codex takes a different approach, using its sandbox architecture to maintain a smaller context window since it can always re-read files from the sandbox filesystem rather than relying on conversation history.
:::

## Hybrid Compaction

The best results come from combining strategies. Apply them in order of increasing aggressiveness:

```rust
struct HybridCompactor {
    deduplicator: SemanticDeduplicator,
    importance_scorer: ImportanceCompactor,
    sliding_window: SlidingWindowCompactor,
}

impl HybridCompactor {
    fn compact(
        &mut self,
        history: &mut MessageHistory,
        tokens_to_free: u32,
    ) -> Vec<CompactionResult> {
        let mut results = Vec::new();
        let mut remaining = tokens_to_free;

        // Phase 1: Remove semantic duplicates (least disruptive)
        let groups = self.deduplicator.find_duplicate_groups(history);
        for group in groups {
            if remaining == 0 {
                break;
            }
            let to_remove: Vec<usize> = group.indices.iter()
                .filter(|&&i| i != group.representative)
                .copied()
                .collect();

            let freed: u32 = to_remove.iter()
                .filter_map(|&i| history.messages.get(i))
                .map(|m| m.token_count.unwrap_or(0))
                .sum();

            // Remove in reverse order
            let mut sorted_remove = to_remove;
            sorted_remove.sort_unstable_by(|a, b| b.cmp(a));
            for idx in sorted_remove {
                history.messages.remove(idx);
            }

            remaining = remaining.saturating_sub(freed);
            results.push(CompactionResult {
                messages_removed: to_remove.len(),
                tokens_freed: freed,
                strategy: "deduplication".into(),
            });
        }

        // Phase 2: Importance-based removal (moderate disruption)
        if remaining > 0 {
            let result = self.importance_scorer.compact(history, remaining);
            remaining = remaining.saturating_sub(result.tokens_freed);
            results.push(result);
        }

        // Phase 3: Sliding window as last resort
        if remaining > 0 {
            let result = self.sliding_window.compact(history, remaining);
            results.push(result);
        }

        history.rebuild_indexes();
        results
    }
}
```

This three-phase approach is ordered by information loss: deduplication removes truly redundant content (zero information loss), importance scoring removes low-value content (minimal loss), and sliding window removes everything that's old (highest loss but guaranteed to free space).

## Key Takeaways

- Sliding window compaction is the simplest algorithm: keep the N most recent messages, drop the rest. It's fast and reliable but ignores message importance.
- Importance scoring ranks messages by heuristics (recency, role, content patterns, token cost) and removes the least important first, preserving critical context regardless of age.
- Semantic deduplication identifies near-identical tool outputs (like repeated file reads) and replaces groups with a single representative, often freeing thousands of tokens with zero information loss.
- A hybrid approach that applies deduplication first, then importance scoring, then sliding window gives the best balance between context quality and space efficiency.
- Always preserve tool call/result pairing integrity during compaction -- removing a tool call without its result (or vice versa) creates an invalid message sequence that will confuse the LLM.
