# Goose — Context Management

## Overview

Goose implements a multi-layered context management system to handle long conversations within LLM context windows. The system operates at three levels: **proactive compaction** (before context overflow), **reactive compaction** (on context overflow errors), and **background optimization** (tool-pair summarization). The default context limit is 128,000 tokens, with compaction triggered at 80% utilization.

Key implementation: `crates/goose/src/context_mgmt/mod.rs`

## Context Window Configuration

```rust
pub struct ModelConfig {
    pub model_name: String,
    pub context_limit: Option<usize>,   // Default: 128,000
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    // ...
}
```

Context limit resolution order:
1. Explicit configuration (`GOOSE_CONTEXT_LIMIT` env var)
2. Canonical model registry (known models with known limits)
3. Predefined model database
4. `DEFAULT_CONTEXT_LIMIT` = 128,000 tokens

## Compaction Threshold

```rust
pub const DEFAULT_COMPACTION_THRESHOLD: f64 = 0.8;  // 80% of context limit
```

This means for a 128K context window, compaction triggers when the conversation reaches ~102K tokens.

## Three-Level Context Management

### Level 1: Proactive Compaction (Pre-Loop)

Before entering the reply loop, Goose checks if compaction is needed:

```rust
pub async fn check_if_compaction_needed(
    provider: &dyn Provider,
    conversation: &Conversation,
    threshold_override: Option<f64>,
    session: &Session,
) -> Result<bool>
```

This function:
1. Gets the current token count from session metadata (or estimates it)
2. Compares against `context_limit * compaction_threshold`
3. Returns `true` if the conversation exceeds the threshold

If compaction is needed, `reply()` calls `compact_messages()` before entering `reply_internal()`, and yields an `AgentEvent::HistoryReplaced` event so the UI can refresh its display.

### Level 2: Reactive Compaction (Error Recovery)

During the reply loop, if the LLM returns a `ContextLengthExceeded` error:

```rust
Err(ProviderError::ContextLengthExceeded(_)) => {
    compaction_attempts += 1;
    if compaction_attempts >= 2 {
        // Give up after 2 attempts
        yield error message;
        break;
    }
    let (compacted, usage) = compact_messages(
        provider, session_id, &conversation, false
    ).await?;
    conversation = compacted;
    yield AgentEvent::HistoryReplaced(conversation.clone());
    // Retry the current turn
    continue;
}
```

This handles cases where:
- The proactive check underestimated token usage
- Tool results were larger than expected
- The model's actual context limit differs from the configured one

The 2-attempt limit prevents infinite compaction loops.

### Level 3: Background Tool-Pair Summarization

The most sophisticated optimization runs in the background each turn:

```rust
pub fn maybe_summarize_tool_pairs(
    provider: Arc<dyn Provider>,
    session_id: String,
    conversation: &Conversation,
    tool_call_cut_off: usize,
) -> JoinHandle<Vec<(usize, Message)>>
```

This function:
1. Identifies old tool request → tool response pairs (before `tool_call_cut_off`)
2. For each pair, generates a concise summary using the LLM
3. Returns a list of `(index, summary_message)` tuples

After the summarization task completes, the agent:
1. Marks the original tool request/response messages as **agent-invisible** (they won't be sent to the LLM)
2. Inserts the summary messages in their place
3. The originals are kept for UI display and session persistence

```rust
// Mark old messages invisible
for (idx, _) in &summaries {
    conversation.messages[*idx].set_agent_visible(false);
}
// Insert summaries
for (_, summary) in summaries {
    conversation.messages.push(summary);
}
```

### Tool Call Cutoff Calculation

```rust
pub fn compute_tool_call_cutoff(
    context_limit: usize,
    compaction_threshold: f64,
) -> usize
```

This determines how many recent tool-call pairs to keep un-summarized. Older pairs beyond the cutoff are candidates for summarization. The cutoff is based on the context limit and compaction threshold.

## The Compaction Process

```rust
pub async fn compact_messages(
    provider: &dyn Provider,
    session_id: &str,
    conversation: &Conversation,
    manual_compact: bool,
) -> Result<(Conversation, ProviderUsage)>
```

Compaction works by:

1. **Selecting messages to summarize**: All messages except the most recent exchange
2. **Calling the LLM**: With a summarization prompt that asks for a concise summary of the conversation so far
3. **Building new conversation**: A single "summary" message replaces the old messages
4. **Preserving recent context**: The most recent messages are kept verbatim
5. **Returning usage**: Token counts for the compaction call itself

The result is a `Conversation` with:
- A summary message (covering all old context)
- The recent messages (preserved verbatim)

## Message Visibility System

Messages have an `agent_visible` flag that controls whether they're sent to the LLM:

```rust
impl Message {
    pub fn is_agent_visible(&self) -> bool;
    pub fn set_agent_visible(&mut self, visible: bool);
}
```

This is used by tool-pair summarization to keep originals in the conversation (for persistence and UI display) while hiding them from the LLM.

When streaming to the provider, messages are filtered:
```rust
let visible_messages: Vec<&Message> = messages.iter()
    .filter(|m| m.is_agent_visible())
    .collect();
```

## Conversation Fixing

Before sending to the LLM, `fix_conversation()` corrects ordering issues:

```rust
pub fn fix_conversation(messages: &[Message]) -> Vec<Message>
```

This ensures:
- Messages alternate between user and assistant roles (as required by most LLMs)
- Empty messages are handled
- Tool responses are properly paired with tool requests

## MOIM (Model-Oriented Information Management)

Each turn, before calling the LLM, Goose injects dynamic context from extensions:

```rust
let conversation_with_moim = super::moim::inject_moim(
    &conversation, &extension_manager
).await;
```

Each extension's `get_moim()` method is called, and any returned content is injected into the conversation. This is used by:
- **Top of Mind** extension: Injects persistent user instructions
- Other extensions that need per-turn context

## GooseHints (Static Context)

GooseHints provide static context that's loaded once at session start:

### Loading
1. Check for global hints: `~/.config/goose/.goosehints`
2. Check for local hints: `.goosehints` in current directory (and parent directories in git repos)
3. Also checks for `AGENTS.md` (configurable via `CONTEXT_FILE_NAMES`)
4. Combine all hints, local overrides global

### Injection
Hints are incorporated into the system prompt via the `PromptManager`:
```rust
pub struct PromptManager {
    // Manages system prompt construction including goosehints
}
```

### File References
Hints support `@filename` syntax for auto-including file contents:
```
@README.md          # Included in context automatically
docs/setup.md       # Referenced but not auto-included
```

## Token Counting

`crates/goose/src/token_counter.rs` provides token estimation:
- Used when session metadata doesn't have exact token counts
- Provides approximate counts for compaction threshold checks
- Falls back to character-based estimates for unknown tokenizers

## Context Flow Diagram

```
Session Start
    │
    ├── Load GooseHints (.goosehints, AGENTS.md)
    ├── Build system prompt (base + hints)
    │
    ▼
Each Turn
    │
    ├── Check proactive compaction (>80% context used?)
    │   └── Yes → compact_messages() → HistoryReplaced
    │
    ├── Inject MOIM context (per-extension dynamic context)
    │
    ├── Filter messages (agent_visible only)
    │
    ├── Send to LLM
    │   ├── Success → process response
    │   └── ContextLengthExceeded → emergency compact (≤2 attempts)
    │
    ├── Background: maybe_summarize_tool_pairs()
    │   ├── Identify old tool request/response pairs
    │   ├── Summarize each pair via LLM
    │   ├── Mark originals as agent-invisible
    │   └── Insert summaries
    │
    └── Persist all messages (originals + summaries)
```

## Key Design Decisions

1. **80% threshold**: Compaction triggers well before hitting the limit, leaving room for the next response plus tool results.

2. **Two-attempt emergency compaction**: If the proactive check misses (e.g., large tool outputs), reactive compaction catches it. But only 2 attempts to prevent infinite loops.

3. **Background summarization**: Tool-pair summarization is the primary context optimization strategy. It runs asynchronously each turn, progressively shrinking old tool interactions without blocking the response.

4. **Visibility vs. deletion**: Original messages are kept but marked invisible rather than deleted. This preserves the complete conversation history for the UI and session persistence while keeping the LLM's view compact.

5. **Provider-based compaction**: Compaction uses the same LLM provider as the main conversation. This means summaries are generated by the same model that will consume them, ensuring quality.

6. **Hierarchical hints**: GooseHints cascade from global to project to directory level, allowing layered context that's always relevant to the current scope.

## Comparison with Other Agents

| Feature | Goose | Claude Code | Codex CLI |
|---------|-------|-------------|-----------|
| Auto-compaction | Yes (80% threshold) | Yes (similar) | Limited |
| Background summarization | Yes (tool-pair) | No | No |
| Message visibility | Yes (invisible flag) | Message truncation | N/A |
| Context hints | .goosehints + AGENTS.md | CLAUDE.md | N/A |
| MOIM (per-turn injection) | Yes | No | No |
| Emergency compaction | Yes (2 attempts) | Yes | N/A |