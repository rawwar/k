---
title: Why Context Matters
description: Understand how the context window constrains agent behavior and why intelligent context management is essential for effective coding agents.
---

# Why Context Matters

> **What you'll learn:**
> - How context window limits directly impact response quality, tool use accuracy, and agent capability
> - Why naive append-only conversation histories degrade performance over long sessions
> - What the cost and latency implications are of sending large context windows per request

Up to this point, your agent has been treating conversation history like an ever-growing list. Every user message, every assistant response, every tool call and its result gets appended, and the whole thing ships off to the API on each turn. For short conversations, this works fine. But a real coding agent session can involve dozens of tool calls, thousands of lines of file content, and hundreds of back-and-forth exchanges. Without context management, your agent will break.

## The Context Window Is a Hard Limit

LLMs do not have infinite memory. They process a fixed-size window of tokens on every request. For Claude models, this ranges from 200K tokens (Claude 3.5 Sonnet) to 1M tokens (Claude Opus 4). That sounds like a lot, but consider what a coding agent actually sends in each request:

- A **system prompt** with identity, instructions, and constraints (500--2,000 tokens)
- **Tool definitions** describing every available tool's schema (1,000--5,000 tokens)
- **The full conversation history** -- every user message, assistant response, and tool result
- **File contents** read by tools -- a single large file can be 10,000+ tokens

A session where the agent reads five files, executes a few shell commands, and iterates on a solution can easily consume 50,000--100,000 tokens of context. After a dozen such iterations, you are approaching the limit even on the largest models.

When you exceed the limit, the API returns an error. Your agent crashes, the user loses their work, and the session is unrecoverable. That is the hard failure. But there is a softer, more insidious failure mode too.

## Degraded Performance Before the Limit

Long before you hit the token limit, response quality degrades. Research consistently shows that LLMs perform worse when critical information is buried in the middle of a long context (the "lost in the middle" effect). This means your agent might:

- **Forget earlier instructions** -- it starts ignoring constraints from early messages
- **Misuse tools** -- it calls tools with incorrect arguments because it lost track of the conversation flow
- **Repeat work** -- it re-reads files it already read, wasting tokens and time
- **Contradict itself** -- it gives advice that conflicts with what it said 20 messages ago

For a coding agent, these failures are not abstract. They mean your agent edits the wrong file, overwrites good code, or enters an infinite loop of failed tool calls.

## The Cost Problem

Every token you send costs money. As of 2025, Claude's API charges per input token, and the cost scales linearly with context size. If your agent sends 100K tokens of context on every turn of a 30-turn conversation, you are paying for 3 million input tokens in a single session. That adds up fast.

There is also a latency cost. The time-to-first-token increases with context size. A request with 10K tokens of context returns noticeably faster than one with 200K tokens. For an interactive agent where the user is waiting for a response, this matters.

Here is a simple illustration of how costs grow without management:

```rust
/// Illustrates naive vs. managed context cost growth
fn compare_context_strategies() {
    let tokens_per_turn = 3_000; // Average tokens added per turn
    let cost_per_million_input = 3.0_f64; // dollars per 1M input tokens

    println!("{:<6} {:>12} {:>12} {:>12}", "Turn", "Naive Total", "Managed", "Savings");
    println!("{}", "-".repeat(48));

    for turn in 1..=30 {
        // Naive: every previous turn's tokens are re-sent
        let naive_context: u64 = (1..=turn).map(|t| t * tokens_per_turn).sum();

        // Managed: keep a sliding window of ~20K tokens max
        let managed_context: u64 = naive_context.min(20_000);

        let naive_cost = naive_context as f64 / 1_000_000.0 * cost_per_million_input;
        let managed_cost = managed_context as f64 / 1_000_000.0 * cost_per_million_input;
        let savings = ((1.0 - managed_cost / naive_cost) * 100.0).max(0.0);

        println!(
            "{:<6} {:>10} tk {:>10} tk {:>10.0}%",
            turn, naive_context, managed_context, savings
        );
    }
}

fn main() {
    compare_context_strategies();
}
```

By turn 30, the naive approach sends nearly 1.4 million tokens of context per request, while a managed approach caps at 20K tokens. That is a 98% reduction in input costs.

## What Good Context Management Looks Like

Effective context management is not about throwing away information. It is about making smart decisions about what to keep, what to summarize, and what to discard. A well-managed agent:

1. **Tracks token usage** precisely, knowing exactly how many tokens each message consumes
2. **Reserves budget** for the response and essential context (system prompt, tool definitions)
3. **Compacts old context** by summarizing or truncating messages that are no longer critical
4. **Persists full history** to disk so nothing is truly lost, even if it leaves the context window
5. **Manages the system prompt** efficiently, including only what is needed for the current task

::: python Coming from Python
In Python, you might manage conversation history with a simple list of dicts:
```python
messages = []
messages.append({"role": "user", "content": "..."})
# When it gets too long, just slice it
if len(messages) > 50:
    messages = messages[:1] + messages[-20:]  # Keep system + recent
```
This naive approach loses critical context and has no awareness of token counts.
In Rust, we will build a proper data structure that tracks tokens, supports
prioritized compaction, and serializes efficiently. The ownership model actually
helps here -- when you remove a message from the context window, Rust drops its
memory immediately, with no garbage collector delay.
:::

## The Agent's Context Budget

Think of the context window as a budget. Every API request starts with a fixed total, and you need to allocate it carefully:

| Component | Typical Size | Priority |
|-----------|-------------|----------|
| System prompt | 500--2,000 tokens | Must keep |
| Tool definitions | 1,000--5,000 tokens | Must keep |
| Response headroom | 4,000--8,000 tokens | Must reserve |
| Recent messages | 5,000--20,000 tokens | High priority |
| Older conversation | 10,000--100,000+ tokens | Compactable |
| Tool results (file contents) | Varies widely | Often compactable |

The "must keep" items are non-negotiable -- they are required for every request. Response headroom reserves space for the model's reply. That leaves whatever remains for actual conversation history. Your job in this chapter is to make the best possible use of that remaining space.

::: wild In the Wild
Claude Code implements a sophisticated context management system that tracks token usage across the entire conversation and triggers compaction when approaching limits. It uses a combination of truncation and summarization -- older tool results get truncated first, then if more space is needed, conversation segments get summarized into compact blocks. OpenCode takes a similar approach, maintaining a "context budget" that adapts based on the model being used.
:::

## What You Will Build

Over the next 13 subchapters, you will build a complete context management system for your agent. Here is the roadmap:

- **Token counting** -- accurately measuring how many tokens each message costs
- **Message history** -- a data structure designed for context management, not just storage
- **Session persistence** -- saving and loading full sessions from disk
- **Compaction** -- intelligent strategies for shrinking context when it grows too large
- **Summarization** -- using the LLM itself to compress old conversation segments
- **System prompt management** -- composing and managing the ever-present system prompt
- **Conversation forking** -- branching conversations for exploratory work
- **Multi-session support** -- managing multiple independent conversations

Each piece builds on the last. By the end of this chapter, your agent will handle long, complex coding sessions without breaking a sweat.

## Key Takeaways

- The context window is a hard limit -- exceeding it crashes your agent, and approaching it degrades response quality
- Every token costs money and adds latency; naive append-only histories become prohibitively expensive over long sessions
- The "lost in the middle" effect means performance degrades well before hitting token limits
- Context management is about smart allocation: reserve space for essentials, keep recent context, compact or summarize the rest
- A complete context management system includes token counting, compaction, persistence, and system prompt optimization
