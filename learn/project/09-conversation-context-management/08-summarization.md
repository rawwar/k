---
title: Summarization
description: Use the LLM itself to summarize older conversation segments, replacing verbose message sequences with compact summaries.
---

# Summarization

> **What you'll learn:**
> - How to identify conversation segments that are candidates for summarization
> - How to prompt the LLM to generate concise summaries that preserve key decisions and context
> - How to replace original messages with summary blocks and track what was summarized

The compaction strategies from the previous subchapter are mechanical -- they truncate and prune without understanding the content. Summarization is different. It uses the LLM itself to read a block of conversation and produce a compact summary that preserves the essential information. This is the most powerful compaction technique, but it comes with costs you need to manage carefully.

## When to Summarize

Summarization is not free. It requires an API call, which costs tokens and adds latency. You should summarize when:

1. **Mechanical compaction is not enough** -- the pipeline from the previous subchapter ran all its stages and the context is still too large
2. **There is a natural break point** -- the user started a new task, or a multi-step tool sequence completed
3. **Old context is still partially relevant** -- you cannot simply drop it (sliding window), but you do not need every detail

Let's build a segment identifier that finds good summarization candidates:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    id: u64,
    role: String,
    content: String,
    token_count: usize,
    is_summary: bool,
}

/// Identifies segments of conversation that are good candidates for summarization.
/// A segment is a contiguous block of messages that form a logical unit
/// (e.g., a user request, tool calls, and the assistant response).
pub struct SegmentIdentifier {
    /// Minimum tokens in a segment to be worth summarizing
    min_segment_tokens: usize,
    /// Minimum messages in a segment
    min_segment_messages: usize,
    /// Do not summarize messages newer than this many turns ago
    recency_threshold: usize,
}

#[derive(Debug)]
pub struct Segment {
    /// Indices into the message array
    pub start_idx: usize,
    pub end_idx: usize,
    /// Total tokens in this segment
    pub total_tokens: usize,
    /// Number of messages
    pub message_count: usize,
}

impl SegmentIdentifier {
    pub fn new() -> Self {
        Self {
            min_segment_tokens: 500,
            min_segment_messages: 3,
            recency_threshold: 6,
        }
    }

    /// Find summarizable segments in the conversation.
    /// Segments are identified by user message boundaries -- each user message
    /// starts a new segment that includes the subsequent tool calls and
    /// assistant response.
    pub fn find_segments(&self, messages: &[Message]) -> Vec<Segment> {
        if messages.len() <= self.recency_threshold {
            return Vec::new(); // Too short to summarize anything
        }

        let summarizable_end = messages.len().saturating_sub(self.recency_threshold);
        let mut segments = Vec::new();
        let mut current_start: Option<usize> = None;
        let mut current_tokens = 0;
        let mut current_count = 0;

        for (i, msg) in messages[..summarizable_end].iter().enumerate() {
            // Skip already-summarized messages and pinned messages
            if msg.is_summary {
                // Close current segment if open
                if let Some(start) = current_start.take() {
                    if current_tokens >= self.min_segment_tokens
                        && current_count >= self.min_segment_messages
                    {
                        segments.push(Segment {
                            start_idx: start,
                            end_idx: i,
                            total_tokens: current_tokens,
                            message_count: current_count,
                        });
                    }
                }
                current_tokens = 0;
                current_count = 0;
                continue;
            }

            // User messages mark segment boundaries
            if msg.role == "user" && current_start.is_some() {
                let start = current_start.unwrap();
                if current_tokens >= self.min_segment_tokens
                    && current_count >= self.min_segment_messages
                {
                    segments.push(Segment {
                        start_idx: start,
                        end_idx: i,
                        total_tokens: current_tokens,
                        message_count: current_count,
                    });
                }
                current_start = Some(i);
                current_tokens = msg.token_count;
                current_count = 1;
            } else {
                if current_start.is_none() {
                    current_start = Some(i);
                }
                current_tokens += msg.token_count;
                current_count += 1;
            }
        }

        // Close final segment
        if let Some(start) = current_start {
            if current_tokens >= self.min_segment_tokens
                && current_count >= self.min_segment_messages
            {
                segments.push(Segment {
                    start_idx: start,
                    end_idx: summarizable_end,
                    total_tokens: current_tokens,
                    message_count: current_count,
                });
            }
        }

        segments
    }
}

fn main() {
    let messages = vec![
        Message { id: 1, role: "user".into(), content: "Read main.rs".into(), token_count: 50, is_summary: false },
        Message { id: 2, role: "tool".into(), content: "[file contents...]".into(), token_count: 800, is_summary: false },
        Message { id: 3, role: "assistant".into(), content: "The file contains...".into(), token_count: 200, is_summary: false },
        Message { id: 4, role: "user".into(), content: "Now read lib.rs".into(), token_count: 50, is_summary: false },
        Message { id: 5, role: "tool".into(), content: "[file contents...]".into(), token_count: 600, is_summary: false },
        Message { id: 6, role: "assistant".into(), content: "This module has...".into(), token_count: 150, is_summary: false },
        Message { id: 7, role: "user".into(), content: "Fix the bug".into(), token_count: 30, is_summary: false },
        Message { id: 8, role: "assistant".into(), content: "I'll fix it...".into(), token_count: 100, is_summary: false },
        // Recent messages -- should not be summarized
        Message { id: 9, role: "user".into(), content: "Run the tests".into(), token_count: 20, is_summary: false },
        Message { id: 10, role: "tool".into(), content: "All tests pass".into(), token_count: 50, is_summary: false },
        Message { id: 11, role: "assistant".into(), content: "Tests pass!".into(), token_count: 30, is_summary: false },
        Message { id: 12, role: "user".into(), content: "Great".into(), token_count: 5, is_summary: false },
    ];

    let identifier = SegmentIdentifier::new();
    let segments = identifier.find_segments(&messages);

    for (i, seg) in segments.iter().enumerate() {
        println!("Segment {}: messages[{}..{}] = {} msgs, {} tokens",
            i, seg.start_idx, seg.end_idx, seg.message_count, seg.total_tokens);
        for j in seg.start_idx..seg.end_idx {
            println!("  [{}] {}: {}", messages[j].id, messages[j].role,
                &messages[j].content[..40.min(messages[j].content.len())]);
        }
    }
}
```

## Crafting the Summarization Prompt

The quality of your summary depends entirely on the prompt. A good summarization prompt must:

1. Tell the model what to preserve (decisions, file names, key findings)
2. Tell the model what to drop (verbatim code, raw output)
3. Set a target length
4. Specify the output format

```rust
/// Build a prompt that asks the LLM to summarize a conversation segment.
pub fn build_summarization_prompt(messages: &[Message]) -> String {
    let mut conversation_text = String::new();

    for msg in messages {
        let role_label = match msg.role.as_str() {
            "user" => "User",
            "assistant" => "Assistant",
            "tool" => "Tool Result",
            _ => &msg.role,
        };
        conversation_text.push_str(&format!("[{}]: {}\n\n", role_label, msg.content));
    }

    format!(
r#"Summarize the following conversation segment concisely. Your summary will replace
these messages in the conversation history, so preserve all information needed for
the conversation to continue coherently.

PRESERVE:
- File names and paths mentioned or modified
- Key decisions made and their rationale
- Errors encountered and how they were resolved
- Current state of any ongoing task
- Any constraints or requirements stated by the user

OMIT:
- Verbatim file contents (just note which files were read)
- Raw tool output (summarize the findings instead)
- Pleasantries and filler

Format your summary as a single paragraph prefixed with "[Context Summary]".
Keep it under 200 words.

--- Conversation Segment ---
{conversation_text}
--- End Segment ---

Summary:"#)
}

fn main() {
    let messages = vec![
        Message { id: 1, role: "user".into(), content: "Read src/auth.rs and check for security issues".into(), token_count: 15, is_summary: false },
        Message { id: 2, role: "tool".into(), content: "use bcrypt::hash;\nfn verify_password(input: &str, stored: &str) -> bool {\n    input == stored // BUG: plaintext comparison!\n}\n".into(), token_count: 40, is_summary: false },
        Message { id: 3, role: "assistant".into(), content: "I found a critical security issue in src/auth.rs: the verify_password function compares passwords in plaintext instead of using bcrypt verification. This needs to be fixed immediately.".into(), token_count: 35, is_summary: false },
    ];

    let prompt = build_summarization_prompt(&messages);
    println!("{}", prompt);
}
```

::: python Coming from Python
In Python, you would build the prompt with f-strings or `str.join()`:
```python
def build_summary_prompt(messages):
    convo = "\n".join(f"[{m['role']}]: {m['content']}" for m in messages)
    return f"Summarize this conversation:\n{convo}\n\nSummary:"
```
The Rust version uses `format!` with a raw string literal (`r#"..."#`) to avoid
escaping all the quotes and newlines. Raw strings are especially useful for
prompt templates where you have lots of formatting characters.
:::

## Executing Summarization

Here is how to tie segment identification, prompt construction, and the API call together:

```rust
/// A summarizer that compacts conversation segments using the LLM.
pub struct Summarizer {
    segment_identifier: SegmentIdentifier,
}

/// Represents a completed summarization operation.
#[derive(Debug)]
pub struct SummarizationResult {
    /// The segment that was summarized
    pub start_idx: usize,
    pub end_idx: usize,
    /// The summary text
    pub summary: String,
    /// Tokens in the original segment
    pub original_tokens: usize,
    /// Tokens in the summary
    pub summary_tokens: usize,
    /// Compression ratio (smaller is better)
    pub compression_ratio: f64,
}

impl Summarizer {
    pub fn new() -> Self {
        Self {
            segment_identifier: SegmentIdentifier::new(),
        }
    }

    /// Identify segments and summarize the largest one.
    /// In production, the `call_llm` function would make an actual API call.
    /// Here we accept it as a closure for testability.
    pub fn summarize_largest<F>(
        &self,
        messages: &[Message],
        call_llm: F,
    ) -> Option<SummarizationResult>
    where
        F: Fn(&str) -> String,
    {
        let mut segments = self.segment_identifier.find_segments(messages);
        if segments.is_empty() {
            return None;
        }

        // Sort by total tokens descending -- summarize the largest segment first
        segments.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens));
        let segment = &segments[0];

        let segment_messages = &messages[segment.start_idx..segment.end_idx];
        let prompt = build_summarization_prompt(segment_messages);
        let summary = call_llm(&prompt);

        // Estimate summary token count (in production, use the actual counter)
        let summary_tokens = (summary.len() as f64 * 0.3) as usize;

        Some(SummarizationResult {
            start_idx: segment.start_idx,
            end_idx: segment.end_idx,
            summary,
            original_tokens: segment.total_tokens,
            summary_tokens,
            compression_ratio: summary_tokens as f64 / segment.total_tokens as f64,
        })
    }

    /// Apply a summarization result to the message list.
    pub fn apply_summary(
        messages: &mut Vec<Message>,
        result: &SummarizationResult,
        next_id: &mut u64,
    ) {
        // Create the summary message
        let summary_msg = Message {
            id: *next_id,
            role: "assistant".to_string(),
            content: result.summary.clone(),
            token_count: result.summary_tokens,
            is_summary: true,
        };
        *next_id += 1;

        // Replace the segment with the summary
        let range = result.start_idx..result.end_idx;
        messages.splice(range, std::iter::once(summary_msg));
    }
}

fn main() {
    let mut messages = vec![
        Message { id: 1, role: "user".into(), content: "Read the auth module".into(), token_count: 50, is_summary: false },
        Message { id: 2, role: "tool".into(), content: "[500 lines of auth code]".into(), token_count: 2000, is_summary: false },
        Message { id: 3, role: "assistant".into(), content: "The auth module has a plaintext password bug".into(), token_count: 200, is_summary: false },
        Message { id: 4, role: "user".into(), content: "Fix the password check".into(), token_count: 30, is_summary: false },
        Message { id: 5, role: "tool".into(), content: "[write_file result]".into(), token_count: 100, is_summary: false },
        Message { id: 6, role: "assistant".into(), content: "Fixed, now using bcrypt".into(), token_count: 80, is_summary: false },
        // Recent -- should not be summarized
        Message { id: 7, role: "user".into(), content: "Run tests".into(), token_count: 10, is_summary: false },
        Message { id: 8, role: "tool".into(), content: "3 passed, 1 failed".into(), token_count: 30, is_summary: false },
        Message { id: 9, role: "assistant".into(), content: "One test failed".into(), token_count: 40, is_summary: false },
        Message { id: 10, role: "user".into(), content: "Show me the failure".into(), token_count: 10, is_summary: false },
        Message { id: 11, role: "assistant".into(), content: "The test_login test failed".into(), token_count: 60, is_summary: false },
        Message { id: 12, role: "user".into(), content: "Fix it".into(), token_count: 5, is_summary: false },
    ];

    let total_before: usize = messages.iter().map(|m| m.token_count).sum();

    let summarizer = Summarizer::new();

    // Simulate LLM call with a mock
    let mock_llm = |_prompt: &str| -> String {
        "[Context Summary] User asked to review the auth module. \
         Read src/auth.rs (500 lines) and found a critical bug: \
         verify_password() compares passwords in plaintext instead of \
         using bcrypt. Fixed the function to use bcrypt::verify(). \
         User then asked to run tests."
            .to_string()
    };

    if let Some(result) = summarizer.summarize_largest(&messages, mock_llm) {
        println!("Summarizing messages[{}..{}]", result.start_idx, result.end_idx);
        println!("Original: {} tokens -> Summary: {} tokens ({:.0}% compression)",
            result.original_tokens, result.summary_tokens, (1.0 - result.compression_ratio) * 100.0);

        let mut next_id = 13;
        Summarizer::apply_summary(&mut messages, &result, &mut next_id);
    }

    let total_after: usize = messages.iter().map(|m| m.token_count).sum();
    println!("\nTotal: {} -> {} tokens (freed {})",
        total_before, total_after, total_before - total_after);

    println!("\nFinal messages:");
    for msg in &messages {
        let label = if msg.is_summary { "[SUMMARY]" } else { "" };
        println!("  [{}] {} {}: {}",
            msg.id, msg.role, label,
            &msg.content[..60.min(msg.content.len())]);
    }
}
```

## Costs and Trade-offs

Summarization is powerful but not free. Here are the trade-offs:

| Factor | Cost | Benefit |
|--------|------|---------|
| API call | ~1,000--5,000 tokens + latency | 10--50x compression of old context |
| Information loss | Nuances and details are lost | Essential facts preserved in compact form |
| Latency | 1--3 seconds for the summary | Future turns are faster (smaller context) |
| Cascading summaries | Summaries of summaries lose quality | Rarely needed if initial summary is good |

A good heuristic: summarize a segment only if the summary would be at least 5x smaller than the original. If a segment is only 500 tokens, the overhead of the summarization call (prompt + response) might cost more than you save.

::: wild In the Wild
Claude Code uses a two-pass approach to summarization. First, it identifies the oldest segments of conversation that have not been summarized yet. Then it sends those segments to the model with a carefully crafted prompt that instructs it to preserve file paths, error messages, and decisions while dropping verbatim code and raw output. The result is typically a 10--20x compression ratio. OpenCode similarly triggers summarization when context usage exceeds a configured threshold, focusing on tool-heavy segments that contain the most compressible content.
:::

## Key Takeaways

- Summarization uses the LLM itself to compress old conversation segments, achieving 10--20x compression while preserving essential context
- Identify segments by user message boundaries -- each user request and its tool calls/responses form a natural summarization unit
- Craft the summarization prompt carefully: explicitly state what to preserve (file paths, decisions, errors) and what to omit (raw code, tool output)
- Only summarize when mechanical compaction is insufficient and the compression ratio justifies the API call cost
- Mark summary messages with an `is_summary` flag so they are not re-summarized and so the UI can display them differently
