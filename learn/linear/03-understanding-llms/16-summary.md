---
title: Summary
description: A recap of LLM fundamentals for agent builders, from tokenization through API design to prompt engineering best practices.
---

# Summary

> **What you'll learn:**
> - A consolidated reference of the LLM concepts most critical for agent implementation
> - The key API differences between Anthropic and OpenAI that affect provider abstraction design
> - How the LLM knowledge from this chapter directly informs the agentic loop design in Chapter 4

This chapter covered a lot of ground -- from how tokens work to how to structure system prompts for reliable tool use. Let's consolidate the key concepts you will use directly as you build your coding agent. This is not a repetition of every detail; it is a distillation of the practical knowledge that shapes implementation decisions.

## The LLM as a Component

The core mental model: an LLM is a **stateless text-processing function**. You send it a sequence of messages, and it returns a continuation. It has no memory between calls. It does not execute code. It does not access the internet. Everything it "knows" comes from its training data and the context you provide in each request.

This means your agent must:

- **Manage conversation history** -- accumulate messages across turns and send the full history on each call
- **Provide tools** -- give the model structured ways to interact with the environment
- **Feed back results** -- send tool execution results back so the model can reason about them
- **Handle the output** -- parse responses, dispatch tool calls, and detect when the task is done

The model handles the intelligence (deciding what to do, generating code, reasoning about errors). Your agent handles everything else (execution, state management, I/O, safety).

## Tokenization and Context: The Constraints

Every design decision in your agent is constrained by tokens and context:

- **Tokens** are the unit of measurement for everything: input, output, cost, and rate limits. Code tokenizes at roughly 3-3.5 characters per token. JSON is more expensive (more structural characters). Tool definitions cost 100-300 tokens each.

- **Context windows** set the hard ceiling on what the model can consider in a single request. With a 200K context window, you have space for substantial conversation history, but it fills faster than you expect when tool results include full files and shell output.

- **Output limits** restrict how much the model can generate in one response (separate from the context window). Check `stop_reason` to detect when the model was cut off.

The practical implication: your agent needs a context management strategy from day one. Track token usage from API responses. Truncate large tool results. Plan for summarization or compaction when conversations get long. These are not optimizations -- they are core features.

## The Tool Use Protocol: How Agents Act

The tool use protocol is the mechanism that transforms a text generator into an agent. The lifecycle is:

1. **Define** tools with name, description, and JSON Schema in the API request
2. **Receive** tool calls from the model (structured JSON with tool name and arguments)
3. **Execute** the tool in your agent code (your safety checks, sandboxing, and logic)
4. **Return** results linked by ID to the original tool call

Your agent loop checks the `stop_reason` on every response:
- `tool_use` / `tool_calls` -- execute tools, append results, make another API call
- `end_turn` / `stop` -- display the response, wait for user input
- `max_tokens` / `length` -- handle truncation

This loop is the agentic loop, and you will build it in the next chapter.

### Tool Description Quality Matters

The tool description is the model's primary input for deciding when and how to use each tool. Invest in clear descriptions that include:
- What the tool does and what it returns
- When to use it (and when not to)
- Common usage patterns

Bad description: "Runs a command"
Good description: "Execute a shell command and return stdout, stderr, and exit code. Use for builds (cargo check), tests (cargo test), search (grep, ripgrep), and version control (git). Do not use for reading file contents (use read_file instead)."

## Provider Differences: The Abstraction Challenge

Building a provider-agnostic agent requires handling these structural differences:

| Aspect | Anthropic | OpenAI |
|---|---|---|
| Authentication | `x-api-key` header | `Authorization: Bearer` header |
| System prompt | Top-level `system` field | `role: "system"` message |
| Tool definitions | `input_schema` key | `function.parameters` key, wrapped in `{"type":"function"}` |
| Tool calls | Content blocks in `content` array | Separate `tool_calls` field |
| Tool arguments | Parsed JSON object (`input`) | JSON string (`arguments`) |
| Tool results | `tool_result` block in user message | Separate message with `role: "tool"` |
| Stop signal | `stop_reason: "tool_use"` | `finish_reason: "tool_calls"` |
| Token usage | `input_tokens` / `output_tokens` | `prompt_tokens` / `completion_tokens` |

Your internal data model should use a unified representation:

```rust
enum ContentBlock {
    Text(String),
    ToolUse { id: String, name: String, input: Value },
    ToolResult { tool_use_id: String, content: String, is_error: bool },
}

struct Message {
    role: Role,
    content: Vec<ContentBlock>,
}
```

Then implement serialization to each provider's format at the API boundary. The core agent loop works with the unified representation and never sees provider-specific details.

## Streaming: Why and How

Streaming delivers tokens incrementally over SSE (Server-Sent Events), starting within hundreds of milliseconds. For agents, streaming provides:

- **Responsiveness** -- users see the model's reasoning as it generates
- **Cancellation** -- users can stop a response going in the wrong direction
- **Progress visibility** -- long operations show continuous activity

The implementation challenge: text deltas can be displayed immediately, but tool call JSON must be accumulated until the content block completes. You cannot parse `{"pa` -- you need the full `{"path": "src/main.rs"}`.

Both providers use SSE but with different event structures. Anthropic uses typed events (`content_block_start`, `content_block_delta`, `content_block_stop`). OpenAI uses flat `data:` lines with JSON chunks. Your streaming parser handles this difference behind the provider abstraction.

## Prompt Engineering: Guiding Agent Behavior

System prompt engineering for agents focuses on:

1. **Tool usage rules** -- when to use each tool, mandatory verification steps
2. **Workflow patterns** -- step-by-step procedures for common tasks
3. **Error recovery** -- what to do when tool calls fail
4. **Output formatting** -- how verbose to be, when to show code
5. **Safety constraints** -- forbidden operations, confirmation requirements

The process is iterative: start minimal, observe failures, add targeted rules, test regressions. Keep the system prompt between 500-1,500 tokens for the static portion.

Key techniques:
- Explicit "Do NOT" instructions prevent known failure modes
- "Good approach / Bad approach" examples are highly effective
- Think-before-acting instructions improve tool call quality
- Error recovery instructions prevent premature task abandonment

::: python Coming from Python
As a Python developer, you are used to flexible, dynamically-typed APIs where you can iterate quickly. The Rust agent you are about to build will use strongly-typed structs for messages, tool definitions, and API responses. This might feel like more upfront work, but it pays off: the compiler catches malformed tool results, missing fields, and type mismatches at build time rather than in production. The serde library makes JSON serialization and deserialization ergonomic, and Rust enums are perfect for representing the variant types (text blocks, tool calls, tool results) in the message protocol.
:::

## Cost and Performance: Operational Concerns

Agent operations compound costs because:
- Each API call resends the full conversation history (growing input tokens)
- Output tokens cost 3-5x more than input tokens
- A single task may involve 5-30 API calls

Optimize with:
- **Prompt caching** -- reduces cost of repetitive system prompt and tool definitions
- **Model routing** -- cheaper models for simple operations, frontier models for complex reasoning
- **Context compaction** -- summarize old tool results, truncate large outputs
- **Token tracking** -- monitor usage and display costs to the user

Implement retry logic with exponential backoff and jitter for rate limits (HTTP 429) and overloaded conditions (HTTP 529/503).

## What Comes Next

With this understanding of how LLMs work, how their APIs are structured, and how to guide their behavior through prompts and tool definitions, you are ready to build the agentic loop. In Chapter 4, you will implement the core loop that:

1. Sends messages to the LLM
2. Parses the response
3. Detects and executes tool calls
4. Feeds results back
5. Continues until the task is complete

Every concept from this chapter -- token management, message formatting, tool use protocol, streaming, and error handling -- will be applied directly in that implementation. The LLM knowledge you have built here is the foundation; the agentic loop is where it all comes together.

::: wild In the Wild
Every production coding agent -- Claude Code, OpenCode, Codex, and others -- implements the concepts covered in this chapter. They all manage conversation history, define tools with JSON Schema, parse tool calls from model responses, handle streaming, implement retry logic, and tune their system prompts iteratively. The differences between agents are in the details: which tools they provide, how they manage context, what safety constraints they enforce, and how they present results to the user. The fundamentals are universal, and they are what you now understand.
:::

## Key Takeaways

- The LLM is a stateless text-processing function -- your agent provides all context on every call and manages state, execution, and safety around it
- The tool use protocol (define, receive, execute, return) is the mechanism that transforms text generation into autonomous action, driven by the `stop_reason` field in the agent loop
- Provider abstraction requires handling structural differences in authentication, system prompts, tool definitions, tool calls, and token reporting -- use a unified internal representation with provider-specific serialization
- Prompt engineering for agents is iterative and focuses on tool usage rules, workflow patterns, error recovery, and preventing specific failure modes -- it is the highest-leverage ongoing maintenance activity
- Start with Claude Sonnet as your default model, implement model routing for cost optimization, and build a task-specific benchmark suite to evaluate new models as they are released
