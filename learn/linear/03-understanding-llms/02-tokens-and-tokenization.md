---
title: Tokens and Tokenization
description: How text is broken into tokens, why token counts differ from character counts, and how this impacts agent context management.
---

# Tokens and Tokenization

> **What you'll learn:**
> - How BPE and similar tokenization algorithms split text into subword tokens
> - Why code tokenizes differently than natural language and how this affects context budgets
> - How to estimate token counts for different content types to manage context window usage

Every interaction with an LLM is measured in tokens -- not characters, not words, not lines. Tokens are the fundamental unit of input and output for language models, and they directly determine how much context you can fit in a request, how fast the model responds, and how much each API call costs. As an agent builder, you will think about tokens constantly: "How many tokens is this file?" "Can I fit the error output in the remaining context?" "Is this conversation getting too long?" This subchapter gives you the foundation to answer those questions.

## What Is a Token?

A token is a chunk of text that the model treats as a single unit. Tokens are not characters. They are not words. They are subword units determined by a tokenization algorithm during model training. The most common algorithm is **Byte Pair Encoding (BPE)**, and understanding it intuitively will help you reason about token counts.

Here is how BPE works conceptually:

1. Start with every individual character as its own token
2. Find the most frequently occurring pair of adjacent tokens in the training corpus
3. Merge that pair into a new single token
4. Repeat thousands of times

After thousands of merges, common words become single tokens, common subwords (like "ing", "tion", "pre") become single tokens, and rare or unusual strings remain as individual characters or small pieces.

For example, the word "function" is common enough in English and code that most tokenizers encode it as a single token. But the word "defenestrate" would likely be split into something like `["def", "en", "est", "rate"]` because it is rare.

## How Code Tokenizes

This is where things get particularly relevant for coding agents. Code tokenizes differently than prose, and the differences matter for context management.

Common programming keywords and patterns tend to be single tokens:

```
"function"  -> 1 token
"return"    -> 1 token
"const"     -> 1 token
"import"    -> 1 token
```

But code also contains many characters that tokenize less efficiently:

```
"    "      -> might be 1 token (4 spaces) or multiple tokens
"{"         -> 1 token
"}"         -> 1 token
"::"        -> often 1 token (common in Rust/C++)
"=>"        -> often 1 token
```

Here is a practical example. Consider this Rust function:

```rust
fn process_file(path: &str) -> Result<String, std::io::Error> {
    let content = std::fs::read_to_string(path)?;
    Ok(content.to_uppercase())
}
```

This is 3 lines and roughly 100 characters. With Claude's tokenizer, it comes out to approximately 30-35 tokens. The rough rule of thumb -- **1 token is approximately 4 characters of English text or 3 characters of code** -- holds reasonably well, but there is meaningful variance.

Whitespace in code is a particular concern. Python's significant whitespace and common 4-space indentation means deeply nested Python code burns tokens on indentation. Rust's curly-brace syntax uses fewer whitespace tokens but adds brace tokens. In practice, most code averages about **2.5-3.5 characters per token**.

## Token Counts in Practice

Let's look at some real numbers that matter for agent design. These are approximate and vary by tokenizer, but they give you reliable estimates:

| Content Type | Approximate Tokens per 1K Characters |
|---|---|
| English prose | 250 tokens |
| Python code | 300 tokens |
| Rust code | 320 tokens |
| JSON data | 350 tokens |
| Minified code | 400+ tokens |
| Base64 data | 500+ tokens |

JSON is particularly expensive because of all the structural characters -- quotes, colons, commas, and braces. This matters for agents because tool results are often JSON, and a large JSON response from a tool can consume a surprising amount of context.

::: python Coming from Python
In Python, you might estimate string lengths with `len(text)`. For tokens, the equivalent is using a tokenizer library. The `tiktoken` library handles OpenAI's tokenizers, and Anthropic provides token counts in API responses. In your Rust agent, you will typically use the `input_tokens` and `output_tokens` counts returned by the API rather than counting tokens client-side, though client-side estimation is useful for pre-checking whether content fits in the context window.
:::

## Tokenizer Differences Between Providers

Different model families use different tokenizers. This means the same text produces different token counts depending on which model you are using:

- **Anthropic (Claude)** uses a custom BPE tokenizer. Claude's tokenizer tends to be efficient with code, often producing slightly fewer tokens than GPT tokenizers for the same source code.
- **OpenAI (GPT-4, GPT-4o)** uses the `cl100k_base` tokenizer (for GPT-4) or the `o200k_base` tokenizer (for GPT-4o). The `o200k_base` tokenizer has a 200K vocabulary and is more efficient with many common patterns.

In practice, the differences are small enough (within 10-15%) that you can use a single estimation approach in your agent and rely on the API's reported token counts for precise tracking.

## Special Tokens

Beyond text tokens, LLM APIs use special tokens that you do not see in the raw text but that count toward your token budget. These include:

- **Message role markers** -- each message in the conversation has overhead tokens for the role labels and structural formatting
- **Tool definitions** -- when you send tool schemas, those consume tokens too
- **Stop tokens** -- special tokens that signal the model to stop generating

The overhead from message structure typically adds 3-5 tokens per message. Tool definitions can be much more expensive -- a single tool with a detailed JSON Schema description might consume 100-200 tokens. If you register 10 tools, that is 1,000-2,000 tokens consumed before the conversation even starts.

This is why the API response includes a `usage` object that reports exact token counts:

```json
{
  "usage": {
    "input_tokens": 1247,
    "output_tokens": 356
  }
}
```

You should track these values in your agent to monitor context consumption and make decisions about when to summarize or truncate conversation history.

## Estimating Token Counts for Agent Operations

When building your agent, you will frequently need to estimate whether something fits in the remaining context. Here are practical estimation formulas:

**For a source file:**
```
estimated_tokens = file_size_bytes * 0.3
```
A 10KB source file is roughly 3,000 tokens. This is conservative but safe.

**For a shell command output:**
```
estimated_tokens = output_length_chars / 3.5
```

**For conversation history:**
```
estimated_tokens = sum(message_lengths_chars / 3.5) + (num_messages * 4)
```

The `+ (num_messages * 4)` accounts for the per-message overhead.

These estimates help you make decisions like "should I read this entire file into context, or should I search for the relevant section?" In a coding agent with a 200K token context window, a 10KB file is trivial. But if the user's project has a 500KB generated file, that alone would consume roughly 150K tokens -- most of your context budget.

::: wild In the Wild
Production coding agents like Claude Code are careful about what they put into context. Rather than reading entire files, they use tools like grep and targeted file reads to pull in only relevant sections. When shell command output is very long, they truncate it and let the model know the output was truncated. This token-aware approach to context management is a major differentiator between a working agent and one that runs out of context on real-world tasks.
:::

## The Input/Output Token Distinction

API pricing and rate limits distinguish between **input tokens** (what you send to the model) and **output tokens** (what the model generates). This distinction matters for two reasons:

1. **Cost**: Output tokens are typically 3-5x more expensive than input tokens. A response where the model writes 2,000 tokens of code costs significantly more than the 2,000 tokens of context you sent as input.

2. **Speed**: Output token generation is the bottleneck. The model generates output tokens sequentially (one at a time), while input tokens are processed in parallel. A request with 100K input tokens and 500 output tokens might take only a few seconds, while a request with 1K input tokens and 4,000 output tokens could take 10+ seconds.

For agents, this means that long model responses (detailed code generation, verbose explanations) are both slower and more expensive than short responses (tool calls, brief confirmations). This is one reason why well-designed tool descriptions encourage the model to take action through tools rather than generating long text explanations.

## Key Takeaways

- Tokens are subword units determined by BPE -- common words are single tokens, rare words are split into pieces, and code tokenizes at roughly 3-3.5 characters per token
- Tool definitions, message structure, and JSON formatting all consume tokens beyond the visible text, and the overhead can be significant with many tools registered
- Always use the API's reported `usage` counts for precise tracking, but use the formula `bytes * 0.3` for quick client-side estimates of file sizes
- Output tokens are more expensive and slower to generate than input tokens, which has direct implications for agent design -- prefer concise tool calls over verbose text generation
- Different providers use different tokenizers, but the variation is small enough (10-15%) that a single estimation strategy works in practice
