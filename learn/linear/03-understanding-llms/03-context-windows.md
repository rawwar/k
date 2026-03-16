---
title: Context Windows
description: Understanding context window sizes, their impact on agent design, and strategies for fitting conversations within token limits.
---

# Context Windows

> **What you'll learn:**
> - What context windows are and how different models offer different window sizes
> - How conversation history, tool results, and system prompts compete for limited context space
> - Strategies for context management including summarization, truncation, and sliding windows

The context window is the total number of tokens a model can process in a single request -- input and output combined. It is the hard ceiling on everything your agent can "think about" at once: the system prompt, the entire conversation history, all tool definitions, all tool results, and the response the model generates. When you hit the limit, the API returns an error. Understanding context windows is not optional for agent builders -- it is a core design constraint that shapes every architectural decision.

## Context Window Sizes

Context windows have grown dramatically over the past few years, but they are still finite. Here are the relevant numbers as of early 2025:

| Model | Context Window | Max Output Tokens |
|---|---|---|
| Claude 3.5 Sonnet | 200K tokens | 8,192 tokens |
| Claude 3.5 Haiku | 200K tokens | 8,192 tokens |
| Claude 4 Sonnet | 200K tokens | 16,384 tokens |
| GPT-4o | 128K tokens | 16,384 tokens |
| GPT-4 Turbo | 128K tokens | 4,096 tokens |
| GPT-4.1 | 1M tokens | 32,768 tokens |

Note that the context window includes both input and output. If you have a 200K context window and your input is 195K tokens, the model can only generate 5K tokens of output before hitting the wall -- even if `max_tokens` is set higher.

Also note the **max output tokens** limit, which is separate from the context window. Even with a 200K window and only 10K tokens of input, Claude 3.5 Sonnet can only generate 8,192 tokens in a single response. This is important for agents: if the model needs to generate a long file, it might need to do so across multiple tool calls rather than in a single response.

## What Competes for Context Space

In an agent, the context window is shared by several components, and understanding their relative sizes helps you budget effectively:

**System prompt:** 500-2,000 tokens typically. A well-crafted agent system prompt with tool usage guidelines, behavioral constraints, and capability descriptions fits comfortably in this range.

**Tool definitions:** 100-300 tokens per tool. With 10-15 tools registered, this is 1,000-4,500 tokens. This is a fixed cost you pay on every API call.

**Conversation history:** This is the variable that grows unboundedly. Each turn adds the user message, the assistant response, and any tool call/result pairs. A single turn with a file read tool call that returns a 500-line file could add 3,000+ tokens.

**Current turn output budget:** You need to reserve tokens for the model's response. For a coding agent, reserving 4,000-8,000 tokens for output is reasonable.

Here is what a typical context budget looks like for an agent with a 200K context window after 10 turns of conversation:

```
System prompt:           1,500 tokens
Tool definitions (12):   2,400 tokens
Conversation history:   ~40,000 tokens (10 turns with tool use)
Reserved for output:     8,000 tokens
─────────────────────────────────────
Total used:             ~51,900 tokens
Remaining:             ~148,100 tokens
```

That looks comfortable. But now imagine the user asks the agent to read several large files and debug a complex issue. After 30 turns with multiple file reads and shell outputs:

```
System prompt:           1,500 tokens
Tool definitions (12):   2,400 tokens
Conversation history:  ~180,000 tokens
Reserved for output:     8,000 tokens
─────────────────────────────────────
Total used:            ~191,900 tokens
Remaining:               ~8,100 tokens  ← danger zone
```

This is why context management is an essential feature of any production agent.

## Context Management Strategies

When conversation history threatens to exceed the context window, you have several options. Each has trade-offs.

### Strategy 1: Truncation

The simplest approach: drop the oldest messages when the conversation gets too long. Keep the system prompt and tool definitions (they are always needed), keep the most recent N turns, and discard the rest.

```json
{
  "messages": [
    {"role": "user", "content": "... turn 28 ..."},
    {"role": "assistant", "content": "... turn 28 response ..."},
    {"role": "user", "content": "... turn 29 ..."},
    {"role": "assistant", "content": "... turn 29 response ..."},
    {"role": "user", "content": "... turn 30 (current) ..."}
  ]
}
```

**Pros:** Simple to implement, no additional API calls.
**Cons:** The model loses context about earlier decisions. It might re-read a file it already analyzed, or forget a constraint the user mentioned early in the conversation.

### Strategy 2: Summarization

Before dropping old messages, use the model itself to create a summary of the conversation so far. Insert the summary as a system message or early user message, then truncate the detailed history.

```json
{
  "messages": [
    {"role": "user", "content": "[Conversation summary: The user is debugging a deadlock in their web server. We identified that the mutex in connection_pool.rs is held across an await point. We edited the file to restructure the lock scope. The build now succeeds but the test in test_pool.rs still fails with a timeout.]"},
    {"role": "user", "content": "... turn 29 ..."},
    {"role": "assistant", "content": "... turn 29 response ..."},
    {"role": "user", "content": "Can you check the test timeout value?"}
  ]
}
```

**Pros:** Preserves key context across the entire conversation.
**Cons:** Requires an extra API call to generate the summary, adds latency, and the summary might miss details the model later needs.

### Strategy 3: Sliding Window with Anchors

Keep the first few messages (which often contain the user's original task description) and the most recent messages, discarding the middle:

```
[System prompt]
[First user message - the original task]
[First assistant response - initial plan]
... (middle turns discarded) ...
[Last 5-10 turns of conversation]
```

**Pros:** Preserves both the original intent and the recent state.
**Cons:** The model loses details about intermediate steps, which can cause it to re-explore paths already tried.

### Strategy 4: Tool Result Compaction

Instead of truncating entire messages, compress the expensive parts -- typically tool results. A file read that returned 500 lines can be replaced with a summary:

```json
{
  "role": "tool",
  "content": "[File src/main.rs was read - 500 lines, contains main() function, imports for tokio, reqwest, and serde. Key sections: HTTP client setup (lines 15-45), request handler (lines 50-120), error handling (lines 125-180)]"
}
```

This preserves the conversation flow while dramatically reducing token usage.

::: python Coming from Python
Python developers working with LLM APIs often use libraries like LangChain that provide built-in memory management with classes like `ConversationBufferWindowMemory` or `ConversationSummaryMemory`. In Rust, you will build this yourself, which gives you precise control over the strategy. The core data structure is simply a `Vec<Message>` that you trim, summarize, or compact as needed before each API call.
:::

## The Output Token Limit

Beyond the context window, there is a separate limit on how many tokens the model can generate in a single response. You control this with the `max_tokens` parameter:

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 4096,
  "messages": [...]
}
```

If the model's response hits `max_tokens`, it stops generating mid-sentence (or mid-code-block). The response will have a `stop_reason` of `"max_tokens"` instead of `"end_turn"`. Your agent should check for this and handle it -- perhaps by asking the model to continue, or by increasing `max_tokens` for code generation tasks.

For agents, a common pattern is to set `max_tokens` to the model's maximum (e.g., 8192 for Claude 3.5 Sonnet) and let the model decide how much to generate. The model will stop naturally when it finishes its response, and you only hit the limit if it tries to generate something unusually long.

::: wild In the Wild
Claude Code implements a context management system that tracks token usage across the conversation and compacts old messages when the context is getting full. It prioritizes keeping recent tool results intact while summarizing older ones, and it always preserves the system prompt and tool definitions. This adaptive approach means the agent can handle long coding sessions without running into context limits, while still maintaining enough history for the model to make coherent decisions.
:::

## Practical Guidelines for Agent Design

Based on how context windows work, here are concrete guidelines for your agent:

1. **Track token usage from every API response.** The `usage` object tells you exactly how many tokens the request consumed. Maintain a running total.

2. **Set a context budget threshold.** When total context exceeds 75% of the window, start considering compaction. When it exceeds 90%, compact aggressively.

3. **Budget for tool definitions up front.** Calculate the token cost of your tool schemas once and treat it as a fixed overhead.

4. **Truncate large tool results.** If a shell command outputs 10,000 lines, truncate to the first and last 100 lines with a note: "[output truncated, showing first and last 100 lines of 10,000]".

5. **Reserve output tokens.** Always ensure that `context_window - input_tokens >= max_tokens`, or the API call will fail.

## Key Takeaways

- The context window is the hard limit on total tokens (input + output) per API call, and it is the primary constraint on agent conversation length
- Conversation history, tool definitions, tool results, and the system prompt all compete for the same limited context space, with tool results often being the largest consumer
- Four main strategies for context management exist -- truncation, summarization, sliding window with anchors, and tool result compaction -- and production agents typically combine multiple approaches
- The max output token limit is separate from the context window and must be checked via `stop_reason` to detect when the model was cut off mid-generation
- Budget your context proactively: track usage from API responses, set thresholds for compaction, and truncate large tool results before they consume your budget
