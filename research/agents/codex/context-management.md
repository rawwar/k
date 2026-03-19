# Codex CLI — Context Management

## Overview

Codex CLI manages context through a `ContextManager` that tracks conversation
history, estimates token usage via byte-based heuristics (no real tokenizer),
applies per-item truncation, and triggers automatic remote compaction when
approaching the context window limit.

## ContextManager

The central data structure for conversation history:

```rust
// codex-core/src/context_manager/history.rs
struct ContextManager {
    items: Vec<ResponseItem>,           // ordered oldest → newest
    token_info: Option<TokenUsageInfo>,
    reference_context_item: Option<TurnContextItem>,
}
```

### Key Methods

| Method | Purpose |
|---|---|
| `record_items()` | Add items with truncation policy applied |
| `for_prompt()` | Normalize history for model consumption |
| `estimate_token_count()` | Byte-based heuristic estimation |
| `get_total_token_usage()` | Combine API tokens + local estimate |
| `drop_last_n_user_turns()` | Thread rollback / undo support |
| `remove_first_item()` | Trim oldest items |
| `remove_last_item()` | Trim newest items |

### Prompt Building

```rust
impl ContextManager {
    pub fn for_prompt(&self) -> Vec<ResponseInputItem> {
        let items = self.items.clone();

        // 1. Strip image items if model doesn't support them
        let items = if !model_supports_images {
            items.filter(|i| !i.is_image())
        } else { items };

        // 2. Normalize function call/output pairs
        // 3. Convert ResponseItem → ResponseInputItem
        // 4. Apply any pending truncation

        items.into_iter()
            .filter_map(|item| item.to_input_item())
            .collect()
    }
}
```

## Token Estimation — No Tokenizer

A key design decision: Codex uses a **byte-based heuristic** instead of a real
tokenizer for speed:

```rust
// codex-core/src/context_manager/truncate.rs
const APPROX_BYTES_PER_TOKEN: usize = 4;

fn approx_token_count(text: &str) -> usize {
    text.len() / APPROX_BYTES_PER_TOKEN
}
```

### Token Usage Breakdown

```rust
struct TotalTokenUsageBreakdown {
    /// Tokens reported by the last API response
    last_api_response_total_tokens: i64,

    /// Total bytes of all history items visible to the model
    all_history_items_model_visible_bytes: i64,

    /// Estimated tokens of items added since last successful API response
    estimated_tokens_of_items_added_since_last_successful_api_response: i64,

    /// Raw bytes of items added since last successful API response
    estimated_bytes_of_items_added_since_last_successful_api_response: i64,
}
```

The estimation combines two sources:
1. **Server-reported tokens** from the last API response (`usage.total_tokens`)
2. **Local byte-based estimate** for items added since that response

This avoids the cost of running a tokenizer while being accurate enough for
compaction threshold decisions.

## Truncation Policies

### Per-Item Truncation

Each model has a `TruncationPolicy` that limits individual tool outputs:

```rust
pub enum TruncationPolicy {
    Bytes(usize),    // Truncate at byte count
    Tokens(usize),   // Truncate at estimated token count
}

pub struct TruncationPolicyConfig {
    pub mode: TruncationMode,  // Bytes or Tokens
    pub limit: i64,
}
```

Default: `Bytes(10_000)` — each tool output is capped at ~10KB.

### Truncation Algorithm

```rust
fn truncate_text(text: &str, policy: &TruncationPolicy) -> String {
    let limit = match policy {
        TruncationPolicy::Bytes(n) => *n,
        TruncationPolicy::Tokens(n) => n * APPROX_BYTES_PER_TOKEN,
    };

    if text.len() <= limit {
        return text.to_string();
    }

    // Preserve prefix and suffix, drop middle
    let prefix_size = limit * 2 / 3;   // ~67% from start
    let suffix_size = limit / 3;        // ~33% from end

    let prefix = &text[..prefix_size];
    let suffix = &text[text.len() - suffix_size..];
    let truncated_chars = text.len() - prefix_size - suffix_size;

    format!("{prefix}\n…{truncated_chars} chars truncated…\n{suffix}")
}
```

Key characteristics:
- Preserves the **beginning** (67%) and **end** (33%) of output
- Drops the middle with a `"…N chars truncated…"` marker
- The asymmetric split keeps error messages (typically at the end) visible

### Global Tool Output Truncation

When the total context is too large, a global pass trims all function outputs:

```rust
fn truncate_function_output_items_with_policy(
    items: &mut Vec<ResponseItem>,
    policy: &TruncationPolicy,
) {
    for item in items.iter_mut() {
        if let ResponseItem::FunctionCallOutput { output, .. } = item {
            *output = truncate_text(output, policy);
        }
    }
}
```

## Model Context Windows

### Model Metadata

```rust
struct ModelInfo {
    slug: String,                    // e.g. "gpt-5.2-codex"
    context_window: Option<i64>,     // e.g. 272_000 tokens
    auto_compact_token_limit: Option<i64>,
    effective_context_window_percent: i64, // default 95%
    truncation_policy: TruncationPolicyConfig,
    // ...
}
```

### Key Parameters

| Parameter | Default | Purpose |
|---|---|---|
| `context_window` | 272,000 tokens | Maximum context size |
| `effective_context_window_percent` | 95% | Usable fraction of context window |
| `auto_compact_token_limit` | 90% of context window | Trigger compaction |
| `truncation_policy` | Bytes(10,000) | Per-output truncation |

### Compaction Threshold Calculation

```rust
impl ModelInfo {
    fn auto_compact_token_limit(&self) -> i64 {
        // If explicitly set, use it
        if let Some(limit) = self.auto_compact_token_limit {
            return limit;
        }
        // Otherwise, 90% of context window
        let context = self.context_window.unwrap_or(272_000);
        (context as f64 * 0.90) as i64
    }
}
```

### Config Overrides

Users can override context parameters in `config.toml`:

```toml
model_context_window = 128000
model_auto_compact_token_limit = 100000
tool_output_token_limit = 5000
```

## Auto-Compaction

When estimated token usage approaches `auto_compact_token_limit`, Codex
triggers **remote compaction** — asking the model itself to summarize the
conversation history.

### Compaction Flow

```rust
// codex-core/src/context_manager/compact_remote.rs
async fn run_remote_compact_task_inner_impl(
    session: &Session,
    turn_context: TurnContext,
    injection: Option<String>,
) {
    // 1. Clone current history
    let history = session.context_manager.items.clone();

    // 2. Trim function call items that overflow context window
    //    Drops codex-generated items from the END until estimated
    //    tokens fit within the context window
    let trimmed = trim_function_call_history_to_fit_context_window(
        history,
        turn_context.model_info.context_window,
    );

    // 3. Build compaction request
    let compaction_input = CompactionInput {
        model: turn_context.model.clone(),
        items: trimmed,
        instructions: turn_context.instructions.clone(),
    };

    // 4. Call model's compaction endpoint
    let compacted = model_client
        .compact_conversation_history(compaction_input)
        .await?;

    // 5. Filter compacted output
    //    Keep only user messages + compaction summaries
    let filtered = compacted.into_iter().filter(|item| {
        matches!(item,
            ResponseItem::Message { role: "user", .. } |
            ResponseItem::Compaction { .. }
        )
    }).collect();

    // 6. Replace history with compacted version
    //    Preserve GhostSnapshot items for undo support
    session.context_manager.replace_with_compacted(filtered);

    // 7. Recompute token usage estimate
    session.context_manager.recompute_token_estimates();

    // 8. Emit event
    tx_event.send(EventMsg::ContextCompacted(..)).await;
}
```

### Compaction API

```rust
// codex-api/src/lib.rs
pub struct CompactionInput {
    pub model: String,
    pub items: Vec<ResponseItem>,
    pub instructions: Option<String>,
}

// Returns: Vec<ResponseItem> — the compacted summary
```

### GhostSnapshot Preservation

During compaction, `GhostSnapshot` items are preserved. These are snapshots
of the context at specific points, used for undo/redo operations. Without them,
undoing past a compaction boundary would lose context.

### Manual Compaction

Users can trigger compaction manually via:
- `Op::Compact { .. }` from the API
- The TUI may expose a compact command

## History Management

### Adding Items

```rust
impl ContextManager {
    pub fn record_items(&mut self, items: Vec<ResponseItem>) {
        for item in items {
            // Apply truncation policy to function outputs
            let item = if let ResponseItem::FunctionCallOutput { output, .. } = &item {
                let truncated = truncate_text(output, &self.truncation_policy);
                item.with_output(truncated)
            } else {
                item
            };

            self.items.push(item);
        }

        // Update local token estimate
        self.recompute_local_token_estimates();
    }
}
```

### Removing Items

The context manager maintains **call/output pair integrity** when removing items:

```rust
impl ContextManager {
    pub fn remove_first_item(&mut self) {
        if self.items.is_empty() { return; }
        let removed = self.items.remove(0);
        // If we removed a function call, also remove its output
        // If we removed a function output, also remove its call
        self.cleanup_orphaned_pairs();
    }
}
```

### Thread Rollback

```rust
impl ContextManager {
    pub fn drop_last_n_user_turns(&mut self, n: usize) {
        // Walk backwards through items
        // Count user message boundaries
        // Remove everything after the Nth boundary from the end
        // Preserves GhostSnapshots for redo
    }
}
```

## Model-Specific Context Behavior

### Reasoning Summaries

Models that support reasoning (like `gpt-5.x`) emit `Reasoning` items that
count toward context usage:

```rust
pub enum ReasoningSummary {
    Auto,      // Model decides
    Concise,   // Brief summaries
    Detailed,  // Verbose summaries
    None,      // No reasoning output
}
```

Reasoning effort levels affect context consumption:

```rust
pub enum ReasoningEffort {
    None,     // No reasoning
    Minimal,
    Low,
    Medium,   // Default
    High,
    XHigh,
}
```

### Image Inputs

Images consume significant context tokens. The `for_prompt()` method strips
images when the model doesn't support `InputModality::Image`:

```rust
pub enum InputModality {
    Text,
    Image,
}

// In for_prompt():
if !model_info.input_modalities.contains(&InputModality::Image) {
    items.retain(|item| !matches!(item, ContentItem::InputImage { .. }));
}
```

## Token Usage Reporting

The core emits `TokenUsage` events after each API call:

```rust
pub struct TokenUsageEvent {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub reasoning_tokens: Option<i64>,
    pub cached_tokens: Option<i64>,
}
```

The TUI and exec mode display cumulative token usage. The `/status` command
shows current context state.

## Comparison with Other Agents

| Aspect | Codex CLI | Claude Code | Aider |
|---|---|---|---|
| **Token counting** | Byte heuristic (~4 B/tok) | tiktoken tokenizer | tiktoken tokenizer |
| **Compaction** | Remote (model-based) | Server-managed | Manual repo-map |
| **Default context** | 272,000 tokens | 200,000 tokens | Model-dependent |
| **Truncation** | Prefix+suffix preserve | Server-side | Repo-map pruning |
| **Context window** | Configurable per-model | Fixed per-model | Fixed per-model |
| **Multi-turn** | Full history + rollout | Full history | Git-based |

The byte heuristic is a deliberate trade-off: slightly less accurate than
tokenizer counting, but avoids the startup cost and dependency of loading
a tokenizer model.