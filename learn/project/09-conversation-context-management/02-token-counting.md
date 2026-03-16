---
title: Token Counting
description: Implement accurate token counting using BPE tokenizers to track context usage and make informed decisions about compaction.
---

# Token Counting

> **What you'll learn:**
> - How BPE (Byte Pair Encoding) tokenization works and why character count is not a reliable proxy
> - How to use tiktoken-rs or a similar library to count tokens for Claude's tokenizer
> - How to account for token overhead from message structure, tool definitions, and system prompts

Before you can manage context, you need to measure it. Every decision your context manager makes -- when to compact, what to prune, how much headroom remains -- depends on knowing how many tokens each piece of the conversation consumes. Character counts and word counts are not good enough. You need actual token counts.

## Why Character Count Fails

It is tempting to estimate tokens by dividing character count by 4 (a common rule of thumb). But this approximation can be wildly wrong:

- The word "tokenization" is 12 characters but typically 2--3 tokens
- A code snippet like `fn main() { println!("hello"); }` might be 10 tokens despite being 35 characters
- JSON with lots of punctuation (`{"key": "value", "nested": {"a": 1}}`) tokenizes differently than prose
- Non-ASCII characters (comments in Chinese, variable names in other scripts) can consume more tokens per character

For a coding agent, this matters enormously. Tool results often contain code, JSON, file paths, and structured data -- exactly the content where character-based estimates break down. An estimate that is off by 30% means your compaction triggers too early (wasting context) or too late (risking API errors).

## How BPE Tokenization Works

Most modern LLMs use Byte Pair Encoding (BPE) tokenization. The idea is straightforward:

1. Start with a vocabulary of individual bytes (256 entries)
2. Scan a large training corpus and find the most frequent pair of adjacent tokens
3. Merge that pair into a new token and add it to the vocabulary
4. Repeat until the vocabulary reaches a target size (typically 50,000--100,000 tokens)

The result is a vocabulary where common words like "the" are single tokens, common subwords like "tion" are single tokens, and rare character sequences are broken into smaller pieces. This is why "tokenization" might be split into "token" + "ization" (2 tokens), while an unusual variable name like `xqz_buf` might be split into 4+ tokens.

::: python Coming from Python
In Python, the `tiktoken` library makes token counting trivial:
```python
import tiktoken
enc = tiktoken.encoding_for_model("gpt-4")
tokens = enc.encode("Hello, world!")
print(len(tokens))  # 4
```
Rust does not have an official `tiktoken` crate from Anthropic, but we can use
the `tiktoken-rs` crate which implements the same BPE algorithm, or we can build
a lightweight estimator. The key difference is that in Rust, the tokenizer is
loaded once and reused without the GIL concerns that Python developers face in
multi-threaded contexts.
:::

## Token Counting in Rust

Let's implement a token counter. We will start with `tiktoken-rs`, which provides BPE tokenization compatible with the common tokenizer formats. While Claude uses its own tokenizer, the token counts from `cl100k_base` (used by GPT-4) provide a reasonably close estimate. For exact counts, you can also use the token counting endpoint if your API provides one.

First, add the dependency to your `Cargo.toml`:

```toml
[dependencies]
tiktoken-rs = "0.6"
```

Now let's build a token counter module:

```rust
use tiktoken_rs::cl100k_base;
use std::sync::OnceLock;

/// A reusable tokenizer that counts tokens using BPE encoding.
/// We use OnceLock to initialize the tokenizer once and reuse it.
pub struct TokenCounter {
    bpe: tiktoken_rs::CoreBPE,
}

/// Global token counter - initialized once, used everywhere.
static GLOBAL_COUNTER: OnceLock<TokenCounter> = OnceLock::new();

impl TokenCounter {
    /// Create a new TokenCounter with the cl100k_base encoding.
    pub fn new() -> Self {
        let bpe = cl100k_base().expect("Failed to load cl100k_base tokenizer");
        Self { bpe }
    }

    /// Get the global shared TokenCounter instance.
    pub fn global() -> &'static TokenCounter {
        GLOBAL_COUNTER.get_or_init(TokenCounter::new)
    }

    /// Count the number of tokens in a string.
    pub fn count(&self, text: &str) -> usize {
        self.bpe.encode_ordinary(text).len()
    }

    /// Count tokens for a complete API message, including structural overhead.
    /// Each message has overhead from the role tag, content wrapping, etc.
    pub fn count_message(&self, role: &str, content: &str) -> usize {
        // Each message has ~4 tokens of overhead for role markers
        // and message boundaries in the API format
        let overhead = 4;
        self.count(role) + self.count(content) + overhead
    }

    /// Count tokens for an entire conversation.
    pub fn count_conversation(&self, messages: &[(String, String)]) -> usize {
        let mut total = 3; // Base overhead for conversation framing
        for (role, content) in messages {
            total += self.count_message(role, content);
        }
        total
    }
}

fn main() {
    let counter = TokenCounter::new();

    // Compare character count vs token count
    let examples = vec![
        "Hello, world!",
        "fn main() { println!(\"hello\"); }",
        "{\"key\": \"value\", \"nested\": {\"a\": 1, \"b\": 2}}",
        "The quick brown fox jumps over the lazy dog.",
        "impl<T: Send + Sync + 'static> Handler for Arc<T> where T: Handler {}",
    ];

    println!("{:<60} {:>6} {:>8}", "Text", "Chars", "Tokens");
    println!("{}", "-".repeat(76));

    for text in examples {
        let chars = text.len();
        let tokens = counter.count(text);
        let display = if text.len() > 55 {
            format!("{}...", &text[..52])
        } else {
            text.to_string()
        };
        println!("{:<60} {:>6} {:>8}", display, chars, tokens);
    }
}
```

Running this shows why character counting fails -- Rust generics with angle brackets and trait bounds tokenize very differently from plain English prose.

## The Estimation Fallback

Not every deployment can afford the memory and startup cost of loading a full BPE vocabulary. For lighter-weight use cases, you can build a calibrated estimator that is more accurate than simple character division:

```rust
/// A lightweight token estimator that does not require loading
/// a full BPE vocabulary. Useful for quick estimates and environments
/// where tiktoken-rs is too heavy.
pub struct TokenEstimator {
    /// Calibrated ratio for English prose
    prose_ratio: f64,
    /// Calibrated ratio for source code
    code_ratio: f64,
}

impl TokenEstimator {
    pub fn new() -> Self {
        Self {
            prose_ratio: 0.25, // ~4 chars per token for English
            code_ratio: 0.35,  // ~2.9 chars per token for code
        }
    }

    /// Estimate token count based on content analysis.
    pub fn estimate(&self, text: &str) -> usize {
        let ratio = if self.looks_like_code(text) {
            self.code_ratio
        } else {
            self.prose_ratio
        };
        (text.len() as f64 * ratio).ceil() as usize
    }

    /// Simple heuristic: does this text look like source code?
    fn looks_like_code(&self, text: &str) -> bool {
        let code_indicators = ['{', '}', '(', ')', ';', '=', '<', '>'];
        let indicator_count = text.chars()
            .filter(|c| code_indicators.contains(c))
            .count();
        let total_chars = text.len().max(1);
        // If more than 5% of characters are code indicators, treat as code
        (indicator_count as f64 / total_chars as f64) > 0.05
    }
}

fn main() {
    let estimator = TokenEstimator::new();

    let prose = "The quick brown fox jumps over the lazy dog.";
    let code = "fn main() { let x: Vec<String> = vec![]; println!(\"{:?}\", x); }";

    println!("Prose: '{}' -> ~{} tokens", prose, estimator.estimate(prose));
    println!("Code:  '{}' -> ~{} tokens", code, estimator.estimate(code));
}
```

This estimator distinguishes between prose and code, applying different ratios. It is not perfect, but it is much better than a flat 4:1 ratio and costs nothing to initialize.

## Accounting for Message Overhead

Raw content tokens are only part of the story. The API protocol adds overhead for every message:

```rust
/// Calculate the total token overhead for an API request.
pub fn calculate_request_overhead(
    counter: &TokenCounter,
    system_prompt: &str,
    tool_definitions: &[String],
) -> usize {
    let mut overhead = 0;

    // System prompt tokens
    overhead += counter.count(system_prompt);
    overhead += 4; // System message framing

    // Tool definitions -- each tool adds its schema
    for tool_def in tool_definitions {
        overhead += counter.count(tool_def);
        overhead += 10; // Per-tool framing overhead
    }

    // Base request overhead (conversation framing, etc.)
    overhead += 3;

    overhead
}

/// Represents a token budget for a single API request.
pub struct TokenBudget {
    pub model_limit: usize,
    pub system_prompt_tokens: usize,
    pub tool_definition_tokens: usize,
    pub response_reserve: usize,
}

impl TokenBudget {
    /// How many tokens are available for conversation messages?
    pub fn available_for_messages(&self) -> usize {
        self.model_limit
            .saturating_sub(self.system_prompt_tokens)
            .saturating_sub(self.tool_definition_tokens)
            .saturating_sub(self.response_reserve)
            .saturating_sub(50) // Safety margin
    }
}

fn main() {
    let budget = TokenBudget {
        model_limit: 200_000,
        system_prompt_tokens: 1_500,
        tool_definition_tokens: 3_000,
        response_reserve: 8_000,
    };

    println!("Model limit:        {:>8} tokens", budget.model_limit);
    println!("System prompt:      {:>8} tokens", budget.system_prompt_tokens);
    println!("Tool definitions:   {:>8} tokens", budget.tool_definition_tokens);
    println!("Response reserve:   {:>8} tokens", budget.response_reserve);
    println!("Available for msgs: {:>8} tokens", budget.available_for_messages());
}
```

Notice the use of `saturating_sub` -- this is a Rust idiom that subtracts without underflowing to a negative number. If the overhead somehow exceeds the model limit, you get 0 instead of a panic from unsigned integer underflow.

## Caching Token Counts

Token counting is not free. Running BPE encoding on every message for every API call wastes CPU. Since messages are immutable once added to the conversation, you should count tokens once and cache the result:

```rust
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// A cached token counter that avoids re-counting the same content.
pub struct CachedTokenCounter {
    counter: TokenCounter,
    cache: HashMap<u64, usize>,
}

impl CachedTokenCounter {
    pub fn new(counter: TokenCounter) -> Self {
        Self {
            counter,
            cache: HashMap::new(),
        }
    }

    /// Count tokens with caching. Uses a hash of the content as the key
    /// to avoid storing the full string in the cache.
    pub fn count(&mut self, text: &str) -> usize {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        let key = hasher.finish();

        *self.cache.entry(key).or_insert_with(|| {
            self.counter.count(text)
        })
    }

    /// How many entries are cached?
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }
}

fn main() {
    let counter = TokenCounter::new();
    let mut cached = CachedTokenCounter::new(counter);

    let text = "fn main() { println!(\"hello\"); }";

    // First call: actually tokenizes
    let count1 = cached.count(text);
    // Second call: hits cache
    let count2 = cached.count(text);

    assert_eq!(count1, count2);
    println!("Token count: {}, cache entries: {}", count1, cached.cache_size());
}
```

We store a hash of the content rather than the content itself -- this keeps the cache small while still avoiding redundant tokenization. In practice, the same tool results and system prompts appear repeatedly across turns, making this cache very effective.

::: wild In the Wild
Claude Code counts tokens carefully for every message and tool result, maintaining a running total that it uses to decide when compaction is needed. Rather than re-counting the entire conversation on every turn, it tracks the token count of each message at insertion time and maintains a running sum -- the same pattern we just built with our cached counter. OpenCode similarly pre-computes token counts and stores them alongside each message in its conversation state.
:::

## Key Takeaways

- Character count is a poor proxy for token count -- BPE tokenization treats code, prose, and structured data very differently
- Use `tiktoken-rs` with `cl100k_base` encoding for accurate token counts that closely approximate Claude's tokenizer
- Every API request has fixed overhead from the system prompt, tool definitions, and message framing -- account for it in your budget
- Cache token counts at insertion time since messages are immutable once added to the conversation
- For lightweight deployments, a calibrated estimator that distinguishes code from prose is much better than a flat character-to-token ratio
