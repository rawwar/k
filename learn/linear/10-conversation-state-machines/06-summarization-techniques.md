---
title: Summarization Techniques
description: Using LLMs to summarize conversation history, preserving key decisions and context while dramatically reducing token count for long-running sessions.
---

# Summarization Techniques

> **What you'll learn:**
> - How to use a secondary LLM call to summarize conversation segments into compact representations that preserve key facts
> - Designing summarization prompts that retain code changes, file paths, error messages, and decision rationale
> - Hierarchical summarization strategies that create summaries of summaries for extremely long sessions

The compaction algorithms in the previous subchapter remove messages. Summarization takes a different approach: it *replaces* messages with a shorter representation that captures their essential content. This is more expensive (it requires an LLM call) but dramatically better at preserving context. A sliding window drops 50 messages and loses everything they contained. Summarization condenses those same 50 messages into a single message that retains the key decisions, file paths, and state changes.

## The Summarization Pipeline

Summarization for coding agents is different from general text summarization. You're not condensing a news article -- you're condensing a work log. The summary must preserve actionable information: which files were changed, what errors occurred, what decisions were made and why, and what the current state of the task is.

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct ConversationSummary {
    /// What the user asked for and the overall goal
    task_description: String,
    /// Files that were read, created, or modified
    files_touched: Vec<FileTouched>,
    /// Key decisions made during the conversation
    decisions: Vec<String>,
    /// Errors encountered and how they were resolved
    errors_and_resolutions: Vec<ErrorResolution>,
    /// Current state: what's done and what remains
    current_state: String,
    /// Token count of this summary
    token_count: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FileTouched {
    path: String,
    action: FileAction,
    summary: String,
}

#[derive(Debug, Serialize, Deserialize)]
enum FileAction {
    Read,
    Created,
    Modified,
    Deleted,
}

#[derive(Debug, Serialize, Deserialize)]
struct ErrorResolution {
    error: String,
    resolution: String,
}
```

The structured summary format is deliberate. Instead of a freeform text summary (which might miss critical details), you define exactly which categories of information must be preserved. This gives the summarization prompt clear targets.

## Crafting the Summarization Prompt

The prompt you send to the LLM for summarization determines the quality of the result. Here's a prompt template designed for coding agent conversations:

```rust
fn build_summarization_prompt(messages: &[Message]) -> String {
    let conversation_text = messages.iter()
        .map(|msg| {
            let role = match msg.role {
                Role::System => "SYSTEM",
                Role::User => "USER",
                Role::Assistant => "ASSISTANT",
                Role::ToolCall => "TOOL_CALL",
                Role::ToolResult => "TOOL_RESULT",
            };
            let content = msg.content.iter()
                .map(|block| match block {
                    ContentBlock::Text(t) => t.clone(),
                    ContentBlock::ToolUse { name, input, .. } => {
                        format!("[Tool: {} with input: {}]", name, input)
                    }
                    ContentBlock::ToolResult { content, is_error, .. } => {
                        if *is_error {
                            format!("[Error: {}]", content)
                        } else if content.len() > 500 {
                            format!("[Output: {}... (truncated)]", &content[..500])
                        } else {
                            format!("[Output: {}]", content)
                        }
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("[{}]: {}", role, content)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        r#"Summarize the following coding agent conversation segment. Your summary must preserve:

1. **Task**: What the user asked for
2. **Files**: Every file path that was read, created, modified, or deleted, with a one-line description of what was done
3. **Decisions**: Key technical decisions and their rationale
4. **Errors**: Any errors encountered and how they were resolved
5. **Current state**: What has been accomplished and what remains

Be concise but complete. Do not omit file paths or error messages. Use this format:

## Task
[One sentence describing the goal]

## Files Touched
- `path/to/file.rs` - [action]: [what was done]

## Key Decisions
- [Decision and rationale]

## Errors & Resolutions
- Error: [error message] -> Resolution: [what fixed it]

## Current State
[What's done, what's pending]

---
CONVERSATION:
{}"#,
        conversation_text
    )
}
```

Notice the pre-truncation of tool results in the prompt itself. If a tool result contains 10,000 characters of file contents, you truncate it to 500 characters for the summarization input. The summary only needs to know "this file was read and it contained a Rust module with HTTP client functions," not the full file contents.

::: python Coming from Python
In Python, you'd build this prompt with f-strings or `str.format()`. Rust's `format!` macro works identically. The real difference is in how you'd call the LLM for summarization. In Python, `response = await client.messages.create(...)` is straightforward. In Rust, the async call returns a `Result` you must handle, but the structure is the same. The key design insight -- using an LLM to summarize for another LLM -- is language-independent.
:::

## Executing the Summarization

The summarization call itself uses a cheaper, faster model when possible. You don't need the most capable model to summarize a conversation -- you need it to be fast and cheap:

```rust
struct Summarizer {
    /// The LLM client for making summarization calls
    client: Box<dyn LlmClient>,
    /// Model to use for summarization (often cheaper than the main model)
    summary_model: String,
    /// Maximum tokens for the summary output
    max_summary_tokens: u32,
}

impl Summarizer {
    async fn summarize_segment(
        &self,
        messages: &[Message],
        tokenizer: &dyn Tokenizer,
    ) -> Result<Message, SummarizationError> {
        let prompt = build_summarization_prompt(messages);

        let response = self.client.complete(&CompletionRequest {
            model: self.summary_model.clone(),
            messages: vec![ApiMessage {
                role: "user".into(),
                content: vec![ApiContentBlock::Text {
                    block_type: "text".into(),
                    text: prompt,
                }],
            }],
            max_tokens: self.max_summary_tokens,
            temperature: 0.0, // Deterministic for summarization
        }).await?;

        let summary_text = response.extract_text();
        let token_count = tokenizer.count_tokens(&summary_text);

        // Verify compression ratio
        let original_tokens: u32 = messages.iter()
            .map(|m| m.token_count.unwrap_or(0))
            .sum();

        if token_count > original_tokens / 2 {
            return Err(SummarizationError::InsufficientCompression {
                original: original_tokens,
                summary: token_count,
            });
        }

        Ok(Message {
            id: uuid::Uuid::new_v4(),
            role: Role::User, // Summaries are injected as user context
            content: vec![ContentBlock::Text(format!(
                "[Summary of {} earlier messages]\n\n{}",
                messages.len(),
                summary_text
            ))],
            timestamp: std::time::SystemTime::now(),
            token_count: Some(token_count),
            metadata: MessageMetadata {
                is_synthetic: true,
                replaces: messages.iter().map(|m| m.id).collect(),
                ..Default::default()
            },
        })
    }
}
```

Key design decisions: temperature is set to 0.0 for deterministic summarization (you don't want creative interpretations of what happened). The compression ratio is verified -- if the summary is more than half the size of the original, it didn't compress enough to justify the LLM call. The summary message is marked as `is_synthetic` so you never accidentally summarize a summary without knowing it (though you will do this intentionally in hierarchical summarization).

## Segmented Summarization

For very long conversations, you don't summarize everything at once. Instead, divide the history into segments and summarize each independently:

```rust
struct SegmentedSummarizer {
    inner: Summarizer,
    /// Target size for each segment in tokens
    segment_size_tokens: u32,
}

impl SegmentedSummarizer {
    async fn summarize_history(
        &self,
        history: &mut MessageHistory,
        tokens_to_free: u32,
        tokenizer: &dyn Tokenizer,
    ) -> Result<Vec<CompactionResult>, SummarizationError> {
        let mut results = Vec::new();

        // Identify the range of messages to summarize
        // Keep system prompt (index 0) and recent messages
        let total = history.len();
        let keep_recent = 10; // Always keep last 10 messages
        let summarize_end = total.saturating_sub(keep_recent);

        if summarize_end <= 1 {
            return Ok(results); // Nothing to summarize
        }

        // Divide into segments
        let segments = self.create_segments(history, 1, summarize_end);

        let mut total_freed = 0u32;
        let mut summaries: Vec<(usize, usize, Message)> = Vec::new();

        for (seg_start, seg_end) in &segments {
            if total_freed >= tokens_to_free {
                break;
            }

            let segment_messages: Vec<Message> = history.messages
                .iter()
                .skip(*seg_start)
                .take(seg_end - seg_start)
                .cloned()
                .collect();

            let segment_tokens: u32 = segment_messages.iter()
                .map(|m| m.token_count.unwrap_or(0))
                .sum();

            let summary = self.inner.summarize_segment(
                &segment_messages, tokenizer,
            ).await?;

            let freed = segment_tokens - summary.token_count.unwrap_or(0);
            total_freed += freed;

            summaries.push((*seg_start, *seg_end, summary));
            results.push(CompactionResult {
                messages_removed: seg_end - seg_start,
                tokens_freed: freed,
                strategy: "summarization".into(),
            });
        }

        // Apply summaries in reverse order to maintain valid indices
        for (start, end, summary) in summaries.into_iter().rev() {
            // Remove the original segment
            history.messages.drain(start..end);
            // Insert the summary at the same position
            history.messages.insert(start, summary);
        }

        history.rebuild_indexes();
        Ok(results)
    }

    fn create_segments(
        &self,
        history: &MessageHistory,
        start: usize,
        end: usize,
    ) -> Vec<(usize, usize)> {
        let mut segments = Vec::new();
        let mut seg_start = start;
        let mut seg_tokens = 0u32;

        for i in start..end {
            if let Some(msg) = history.messages.get(i) {
                seg_tokens += msg.token_count.unwrap_or(0);

                if seg_tokens >= self.segment_size_tokens {
                    // End the segment at a natural boundary
                    let boundary = self.find_natural_boundary(history, seg_start, i);
                    segments.push((seg_start, boundary));
                    seg_start = boundary;
                    seg_tokens = 0;
                }
            }
        }

        // Don't forget the last segment
        if seg_start < end {
            segments.push((seg_start, end));
        }

        segments
    }

    fn find_natural_boundary(
        &self,
        history: &MessageHistory,
        start: usize,
        around: usize,
    ) -> usize {
        // Try to break at a user message (start of a new turn)
        for i in (start..=around + 2).rev() {
            if let Some(msg) = history.messages.get(i) {
                if msg.role == Role::User {
                    return i;
                }
            }
        }
        around + 1 // Fallback: break at the exact position
    }
}
```

Breaking at natural boundaries (user messages) ensures that each segment captures complete turns. If you split in the middle of a tool call/result pair, the summary of that segment would lose the connection between the call and its output.

::: wild In the Wild
Claude Code implements a summarization strategy it calls "auto-compact" that triggers when the conversation approaches the context limit. It summarizes older conversation turns while preserving recent context verbatim. The summarization prompt is carefully tuned to retain file paths, key code changes, and the reasoning behind decisions. The summary is injected as a special system-level message at the beginning of the context, right after the main system prompt, so the model always has access to the condensed history of the full session.
:::

## Hierarchical Summarization

For sessions lasting hours with hundreds of turns, even summaries grow too large. Hierarchical summarization creates summaries of summaries:

```rust
impl SegmentedSummarizer {
    async fn hierarchical_summarize(
        &self,
        history: &mut MessageHistory,
        tokenizer: &dyn Tokenizer,
    ) -> Result<(), SummarizationError> {
        // Count existing summaries
        let summary_count = history.messages.iter()
            .filter(|m| m.metadata.is_synthetic)
            .count();

        // If we have many summaries, summarize them too
        if summary_count > 5 {
            let summary_messages: Vec<Message> = history.messages.iter()
                .filter(|m| m.metadata.is_synthetic)
                .cloned()
                .collect();

            let meta_summary = self.inner.summarize_segment(
                &summary_messages, tokenizer,
            ).await?;

            // Remove old summaries and insert the meta-summary
            history.messages.retain(|m| !m.metadata.is_synthetic);
            // Insert after system prompt
            history.messages.insert(1, meta_summary);
            history.rebuild_indexes();
        }

        Ok(())
    }
}
```

This creates a three-tier memory system: the full recent conversation (verbatim), summaries of earlier segments, and a meta-summary of all summaries. It mirrors how humans remember: recent events in detail, older events in outline, and distant events as a general sense of "what happened."

## Key Takeaways

- Summarization replaces message segments with shorter representations, preserving semantic content that simple truncation would destroy -- especially critical for coding agents where file paths, decisions, and error context must survive compaction.
- Design summarization prompts with structured output formats (task, files touched, decisions, errors, current state) to ensure the summary captures all actionable information.
- Use a cheaper, faster model for summarization calls with temperature 0.0, and verify the compression ratio exceeds 50% to justify the cost.
- Segment long histories at natural boundaries (user message starts) to keep complete turns together, and summarize segments independently for better quality.
- Hierarchical summarization -- summaries of summaries -- handles extremely long sessions by creating a tiered memory that mirrors human recall: detailed recent, outlined middle, and condensed distant history.
