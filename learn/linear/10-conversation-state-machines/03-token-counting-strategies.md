---
title: Token Counting Strategies
description: Accurate token counting using model-specific tokenizers, handling special tokens, caching token counts, and estimating costs before API calls.
---

# Token Counting Strategies

> **What you'll learn:**
> - How tokenizers like tiktoken (OpenAI) and SentencePiece (Anthropic) encode text into tokens and why counts differ between models
> - Accounting for special tokens, message formatting overhead, and tool definitions that consume context window space
> - Caching token counts per message to avoid re-tokenization and implementing fast estimation for cost preview

Accurate token counting is the foundation of context window management. If your count is too low, you'll hit the API's context limit and get an error. If it's too high, you'll compact prematurely and throw away useful context. The difference between the two can be dramatic -- in a conversation with 100,000 tokens of headroom, being off by even 5% means 5,000 tokens of wasted space or a failed API call.

The challenge is that every model family uses a different tokenizer, and the overhead added by message structure, tool definitions, and special tokens is not obvious from the documentation. Let's build a token counting system that handles all of this accurately.

## How Tokenizers Work

A tokenizer converts text into a sequence of integer tokens. Different tokenizer algorithms produce different splits:

```rust
/// Simplified view of what a tokenizer does:
/// "Hello, world!" -> [15339, 11, 1917, 0]  (4 tokens with cl100k)
/// "Hello, world!" -> [22557, 28725, 1526, 28808]  (4 tokens with SentencePiece)
///
/// Same text, different token IDs, potentially different token counts.

/// A trait abstracting over different tokenizer implementations
trait Tokenizer: Send + Sync {
    /// Count tokens in a text string
    fn count_tokens(&self, text: &str) -> u32;

    /// Encode text to token IDs (useful for truncation)
    fn encode(&self, text: &str) -> Vec<u32>;

    /// Decode token IDs back to text
    fn decode(&self, tokens: &[u32]) -> String;

    /// The name of this tokenizer for logging
    fn name(&self) -> &str;
}
```

The two dominant tokenizer families you'll encounter are:

**BPE (Byte Pair Encoding)**: Used by OpenAI's models via the `tiktoken` library. Builds a vocabulary by repeatedly merging the most frequent byte pairs. The `cl100k_base` encoding is used by GPT-4 and GPT-3.5, while `o200k_base` is used by newer models.

**SentencePiece**: Used by many models including those in the Claude family. Uses a unigram language model to find the most probable segmentation. It tends to handle multilingual text and code differently than BPE.

::: python Coming from Python
In Python, you'd use `tiktoken.encoding_for_model("gpt-4")` and call `len(encoding.encode(text))`. In Rust, you'll use crate bindings. The `tiktoken-rs` crate provides the same functionality, while for other tokenizers you might use the `tokenizers` crate from Hugging Face.
:::

## Implementing Token Counters in Rust

Let's build concrete tokenizer implementations. The `tiktoken-rs` crate gives you OpenAI-compatible tokenization:

```rust
use std::sync::Arc;

/// Tokenizer using tiktoken for OpenAI-compatible models
struct TiktokenCounter {
    bpe: tiktoken_rs::CoreBPE,
    name: String,
}

impl TiktokenCounter {
    fn for_model(model: &str) -> Result<Self, TokenizerError> {
        let bpe = tiktoken_rs::get_bpe_from_model(model)
            .map_err(|e| TokenizerError::ModelNotFound(model.to_string()))?;
        Ok(Self {
            bpe,
            name: format!("tiktoken-{}", model),
        })
    }
}

impl Tokenizer for TiktokenCounter {
    fn count_tokens(&self, text: &str) -> u32 {
        self.bpe.encode_with_special_tokens(text).len() as u32
    }

    fn encode(&self, text: &str) -> Vec<u32> {
        self.bpe.encode_with_special_tokens(text)
            .into_iter()
            .map(|t| t as u32)
            .collect()
    }

    fn decode(&self, tokens: &[u32]) -> String {
        let tokens: Vec<usize> = tokens.iter().map(|&t| t as usize).collect();
        self.bpe.decode(tokens).unwrap_or_default()
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Estimation-based counter when exact tokenization is too expensive
/// Uses the ~4 characters per token heuristic for English text
struct EstimationCounter {
    chars_per_token: f32,
}

impl EstimationCounter {
    fn new() -> Self {
        Self { chars_per_token: 4.0 }
    }

    fn for_code() -> Self {
        // Code typically has shorter tokens due to syntax characters
        Self { chars_per_token: 3.5 }
    }
}

impl Tokenizer for EstimationCounter {
    fn count_tokens(&self, text: &str) -> u32 {
        (text.len() as f32 / self.chars_per_token).ceil() as u32
    }

    fn encode(&self, _text: &str) -> Vec<u32> {
        vec![] // Estimation doesn't produce real tokens
    }

    fn decode(&self, _tokens: &[u32]) -> String {
        String::new()
    }

    fn name(&self) -> &str {
        "estimation"
    }
}
```

The `EstimationCounter` is surprisingly useful. When you need a quick "are we close to the limit?" check, running the full tokenizer on every message is wasteful. Estimate first, then use the exact tokenizer only when you're within 10-20% of the threshold.

## Accounting for Message Overhead

Here's the part that trips up most developers: the token count of your text content is not the total token count of the API call. Every API adds structural tokens around messages. For the Anthropic API, the overhead looks approximately like this:

```rust
struct AnthropicOverhead;

impl AnthropicOverhead {
    /// Tokens added per message for role markers and delimiters
    const PER_MESSAGE: u32 = 4;

    /// Tokens for the conversation start/end framing
    const CONVERSATION_FRAME: u32 = 12;

    /// Overhead per tool definition in the request
    const PER_TOOL_DEFINITION: u32 = 30; // Rough: depends on schema size

    /// Overhead per tool_use content block
    const PER_TOOL_USE: u32 = 20;

    /// Overhead per tool_result content block
    const PER_TOOL_RESULT: u32 = 15;
}

/// Calculate total token usage for a complete API request
fn calculate_request_tokens(
    system_prompt: &str,
    messages: &[Message],
    tool_definitions: &[ToolDefinition],
    tokenizer: &dyn Tokenizer,
) -> TokenBudget {
    // System prompt tokens
    let system_tokens = tokenizer.count_tokens(system_prompt)
        + AnthropicOverhead::PER_MESSAGE;

    // Message tokens with overhead
    let message_tokens: u32 = messages.iter().map(|msg| {
        let content_tokens: u32 = msg.content.iter().map(|block| {
            match block {
                ContentBlock::Text(text) => tokenizer.count_tokens(text),
                ContentBlock::ToolUse { name, input, .. } => {
                    tokenizer.count_tokens(name)
                        + tokenizer.count_tokens(&input.to_string())
                        + AnthropicOverhead::PER_TOOL_USE
                }
                ContentBlock::ToolResult { content, .. } => {
                    tokenizer.count_tokens(content)
                        + AnthropicOverhead::PER_TOOL_RESULT
                }
            }
        }).sum();
        content_tokens + AnthropicOverhead::PER_MESSAGE
    }).sum();

    // Tool definition tokens
    let tool_def_tokens: u32 = tool_definitions.iter().map(|tool| {
        tokenizer.count_tokens(&tool.name)
            + tokenizer.count_tokens(&tool.description)
            + tokenizer.count_tokens(&tool.input_schema.to_string())
            + AnthropicOverhead::PER_TOOL_DEFINITION
    }).sum();

    TokenBudget {
        system: system_tokens,
        messages: message_tokens,
        tool_definitions: tool_def_tokens,
        framing: AnthropicOverhead::CONVERSATION_FRAME,
        total: system_tokens + message_tokens + tool_def_tokens
            + AnthropicOverhead::CONVERSATION_FRAME,
    }
}

#[derive(Debug)]
struct TokenBudget {
    system: u32,
    messages: u32,
    tool_definitions: u32,
    framing: u32,
    total: u32,
}
```

Tool definitions are an often-overlooked source of token consumption. If your agent has 15 tools, each with a JSON schema, the tool definitions alone might consume 2,000-5,000 tokens on every single API call. That's fixed overhead you pay regardless of conversation length.

## Caching Strategy

Token counting should happen exactly once per message. Here's a caching layer that sits between your message history and the tokenizer:

```rust
use std::collections::HashMap;
use std::sync::RwLock;

struct CachedTokenizer {
    inner: Box<dyn Tokenizer>,
    cache: RwLock<HashMap<u64, u32>>,
}

impl CachedTokenizer {
    fn new(tokenizer: Box<dyn Tokenizer>) -> Self {
        Self {
            inner: tokenizer,
            cache: RwLock::new(HashMap::new()),
        }
    }

    fn count_with_cache(&self, text: &str) -> u32 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        let key = hasher.finish();

        // Check cache first (read lock)
        if let Ok(cache) = self.cache.read() {
            if let Some(&count) = cache.get(&key) {
                return count;
            }
        }

        // Cache miss: tokenize and store (write lock)
        let count = self.inner.count_tokens(text);
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(key, count);
        }
        count
    }
}
```

This uses a hash of the text content as the cache key. Since messages are immutable once added to the history (a deliberate design choice from the previous subchapter), cache entries never go stale. The `RwLock` allows concurrent reads -- multiple threads checking token counts won't block each other, and writes only happen on cache misses.

::: tip In the Wild
Claude Code tracks token usage per-turn and uses the API response's `usage` field to get exact counts after each call. This gives perfect accuracy for the messages the API has already processed. For pre-call estimation (to decide whether compaction is needed before sending), it uses a fast approximation based on character count with a safety margin. The actual vs. estimated counts are compared over time to calibrate the estimation factor. OpenCode takes a simpler approach, using the `tiktoken-go` library for all counting.
:::

## Cost Estimation

Token counts directly translate to costs. Build this conversion in early so users always know what a conversation is costing:

```rust
#[derive(Debug, Clone)]
struct ModelPricing {
    input_cost_per_million: f64,   // USD per 1M input tokens
    output_cost_per_million: f64,  // USD per 1M output tokens
    cached_input_per_million: f64, // USD per 1M cached input tokens
}

impl ModelPricing {
    fn claude_sonnet() -> Self {
        Self {
            input_cost_per_million: 3.0,
            output_cost_per_million: 15.0,
            cached_input_per_million: 0.30,
        }
    }

    fn estimate_cost(&self, input_tokens: u32, output_tokens: u32) -> f64 {
        let input_cost = (input_tokens as f64 / 1_000_000.0)
            * self.input_cost_per_million;
        let output_cost = (output_tokens as f64 / 1_000_000.0)
            * self.output_cost_per_million;
        input_cost + output_cost
    }

    fn estimate_turn_cost(
        &self,
        input_tokens: u32,
        cached_tokens: u32,
        estimated_output: u32,
    ) -> f64 {
        let fresh_input = input_tokens.saturating_sub(cached_tokens);
        let fresh_cost = (fresh_input as f64 / 1_000_000.0)
            * self.input_cost_per_million;
        let cached_cost = (cached_tokens as f64 / 1_000_000.0)
            * self.cached_input_per_million;
        let output_cost = (estimated_output as f64 / 1_000_000.0)
            * self.output_cost_per_million;
        fresh_cost + cached_cost + output_cost
    }
}
```

Prompt caching changes the cost equation dramatically. If 80% of your input tokens are cached (system prompt + earlier conversation), the input cost drops by 90% on those tokens. We'll cover this in detail in the Cost Optimization subchapter.

## Choosing the Right Counting Strategy

Different situations call for different levels of accuracy:

| Situation | Strategy | Why |
|-----------|----------|-----|
| Pre-compaction check | Estimation (chars/4) | Speed matters; 10% margin is fine |
| Pre-API call validation | Exact tokenizer | Must not exceed context limit |
| Cost display to user | Exact with cache | Users expect accurate costs |
| Real-time typing preview | Estimation | Must not add latency to keystrokes |
| Post-API call tracking | API response `usage` field | Ground truth from the provider |

The pattern is: estimate cheaply to decide *if* you need to act, then count exactly *when* you need to act.

## Key Takeaways

- Every model family uses a different tokenizer (BPE for OpenAI, SentencePiece for others), and the same text produces different token counts depending on the model.
- Message structure overhead (role markers, delimiters, tool schemas) can add 10-20% beyond your raw text token count -- always include it in your calculations.
- Cache token counts per message using content hashing, and use a `RwLock<HashMap>` so concurrent reads don't block each other.
- Use a two-tier strategy: fast character-based estimation for frequent checks, exact tokenization for decisions that matter (pre-API validation, cost display).
- Track costs continuously using model pricing tables and the API's `usage` response field -- this is foundational for the budget controls we'll build in the Cost Optimization subchapter.
