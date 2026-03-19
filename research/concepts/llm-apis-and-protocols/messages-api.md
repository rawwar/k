# Anthropic Messages API

## Introduction

The Messages API is Anthropic's primary interface for interacting with Claude models. Unlike
OpenAI's evolving API surface (Chat Completions → Responses), Anthropic has maintained a single,
stable Messages API that has grown through additive changes—content blocks, tool use, extended
thinking, prompt caching, and document support have all been added without breaking the core
interface.

### Anthropic's API Philosophy

Anthropic's API design reflects several deliberate choices:

1. **Explicit over implicit.** `max_tokens` is required, not optional. You must decide how much
   output you want. The system prompt is a separate parameter, not a message role. Content is
   structured as typed blocks, not overloaded strings.

2. **Content blocks as the universal primitive.** Everything—text, images, tool calls, tool results,
   thinking, documents—is a content block. This creates a consistent, extensible data model that
   doesn't require new roles or message types as capabilities expand.

3. **No hidden magic.** When Claude uses a tool, you see the tool call, execute it yourself, and
   send the result back. There are no built-in tools that execute server-side (unlike OpenAI's
   web_search or code_interpreter). You always control the tool execution loop.

4. **Versioned API with stability guarantees.** The `anthropic-version` header ensures your
   integration won't break as the API evolves. Each version is a stable contract.

---

## Endpoint

```
POST https://api.anthropic.com/v1/messages
```

### Authentication

Two required headers:

```
x-api-key: sk-ant-...
anthropic-version: 2023-06-01
Content-Type: application/json
```

The `anthropic-version` header is mandatory and specifies the API version. The current stable
version is `2023-06-01`. Additional beta features may require the `anthropic-beta` header:

```
anthropic-beta: interleaved-thinking-2025-05-14,extended-thinking-2025-05-14
```

Multiple beta features can be comma-separated.

---

## Request Schema

### Core Parameters

| Parameter          | Type              | Required | Description                                              |
| ------------------ | ----------------- | -------- | -------------------------------------------------------- |
| `model`            | string            | Yes      | Model identifier                                         |
| `messages`         | array             | Yes      | Array of message objects (user/assistant turns)          |
| `max_tokens`       | integer           | Yes      | Maximum tokens to generate (REQUIRED, unlike OpenAI)     |
| `system`           | string or array   | No       | System prompt (separate from messages)                   |
| `temperature`      | number            | No       | Sampling temperature (0.0 - 1.0, default 1.0)           |
| `top_p`            | number            | No       | Nucleus sampling threshold                               |
| `top_k`            | number            | No       | Top-K sampling (Anthropic-specific, not in OpenAI)       |
| `stop_sequences`   | array of strings  | No       | Custom stop sequences                                    |
| `stream`           | boolean           | No       | Enable SSE streaming                                     |
| `tools`            | array             | No       | Tool definitions                                         |
| `tool_choice`      | object            | No       | How to handle tool selection                             |
| `metadata`         | object            | No       | Request metadata (e.g., `user_id`)                       |
| `thinking`         | object            | No       | Extended thinking configuration                          |

### model

Available models (as of mid-2025):

| Model ID                          | Description                              | Context  |
| --------------------------------- | ---------------------------------------- | -------- |
| `claude-sonnet-4-20250514`        | Best balance of speed and intelligence   | 200K     |
| `claude-opus-4-20250514`          | Most capable model                       | 200K     |
| `claude-haiku-4-5-20251001`       | Fastest, most cost-effective             | 200K     |

Model aliases like `claude-sonnet-4-20250514` point to the latest snapshot. You can also use
date-pinned versions for reproducibility.

### messages

Array of message objects representing the conversation. Messages must alternate between `user` and
`assistant` roles (the first message must be `user`):

```json
{
  "messages": [
    { "role": "user", "content": "Hello, Claude!" },
    { "role": "assistant", "content": "Hello! How can I help you today?" },
    { "role": "user", "content": "Tell me about quantum computing." }
  ]
}
```

The `content` field can be either a string (shorthand for a single text block) or an array of
content blocks (required for multimodal input, tool use, etc.):

```json
{
  "role": "user",
  "content": [
    { "type": "text", "text": "What's in this image?" },
    {
      "type": "image",
      "source": {
        "type": "base64",
        "media_type": "image/png",
        "data": "iVBORw0KGgo..."
      }
    }
  ]
}
```

### system

The system prompt is a **separate top-level parameter**, not a message with `role: "system"`.
This is a key architectural difference from OpenAI:

**String form:**
```json
{
  "system": "You are a helpful coding assistant who always provides examples in Python.",
  "messages": [...]
}
```

**Content blocks form (needed for cache_control):**
```json
{
  "system": [
    {
      "type": "text",
      "text": "You are a helpful coding assistant.",
      "cache_control": { "type": "ephemeral" }
    }
  ],
  "messages": [...]
}
```

### max_tokens

**This is REQUIRED.** Unlike OpenAI where `max_tokens` is optional (defaulting to the model's
maximum), Anthropic requires you to explicitly set it. This forces developers to think about output
length and cost:

```json
{
  "max_tokens": 4096
}
```

For extended thinking, the budget tokens for thinking come from `thinking.budget_tokens`, while
`max_tokens` controls the visible output.

Maximum values depend on the model—Claude Sonnet 4 supports up to 16384 output tokens (or 64000
with extended thinking enabled).

### temperature, top_p, top_k

- `temperature`: Controls randomness. Range 0.0 to 1.0 (default 1.0). Note: Anthropic's default is
  1.0, while OpenAI's default varies.
- `top_p`: Nucleus sampling. Only consider tokens with cumulative probability ≤ top_p.
- `top_k`: Anthropic-specific parameter. Only consider the top K most likely tokens. Not available
  in OpenAI's API.

**Important:** When `thinking` is enabled, `temperature` is forced to 1.0 and cannot be changed.

### stop_sequences

Custom strings that cause the model to stop generating:

```json
{
  "stop_sequences": ["```", "END_OF_RESPONSE", "\n\nHuman:"]
}
```

The model stops generating when it produces any of these strings. The matched stop sequence is
returned in the `stop_sequence` field of the response.

---

## Content Blocks System

Content blocks are the fundamental data structure in the Messages API. Every piece of content—
input and output—is represented as a typed block. This is more explicit than OpenAI's approach
where content is usually a plain string.

### TextBlock

The most basic content block. Used in both input and output:

```json
{
  "type": "text",
  "text": "Hello, how can I help you today?"
}
```

In responses, text blocks may include `citations` when the model references provided documents:

```json
{
  "type": "text",
  "text": "According to the report, revenue increased by 15%.",
  "citations": [
    {
      "type": "document",
      "document_index": 0,
      "document_title": "Q3 Report",
      "start_char_index": 45,
      "end_char_index": 78,
      "cited_text": "Revenue grew 15% year-over-year..."
    }
  ]
}
```

### ImageBlock

For sending images to Claude. Supports base64 encoding and URLs:

**Base64:**
```json
{
  "type": "image",
  "source": {
    "type": "base64",
    "media_type": "image/jpeg",
    "data": "/9j/4AAQSkZJRg..."
  }
}
```

**URL:**
```json
{
  "type": "image",
  "source": {
    "type": "url",
    "url": "https://example.com/image.png"
  }
}
```

Supported formats: JPEG, PNG, GIF, WebP. Maximum size: 20MB. Images are resized if they exceed
certain dimension limits.

### ToolUseBlock

Generated by the model when it wants to call a tool. Appears in assistant message content:

```json
{
  "type": "tool_use",
  "id": "toolu_01A09q90qw90lq917835lgs",
  "name": "get_weather",
  "input": {
    "location": "San Francisco",
    "unit": "fahrenheit"
  }
}
```

Key fields:
- `id`: Unique identifier for this tool call. Must be referenced in the corresponding tool result.
- `name`: The tool name matching one of the defined tools.
- `input`: The arguments as a parsed JSON object (not a string like OpenAI).

### ToolResultBlock

Sent by the user to provide the result of a tool call. Must reference the `tool_use_id`:

```json
{
  "type": "tool_result",
  "tool_use_id": "toolu_01A09q90qw90lq917835lgs",
  "content": "72°F, sunny, 15% humidity"
}
```

The `content` can be a string or an array of content blocks (text, images) for rich tool results:

```json
{
  "type": "tool_result",
  "tool_use_id": "toolu_01A09q90qw90lq917835lgs",
  "content": [
    { "type": "text", "text": "Weather data retrieved successfully." },
    {
      "type": "image",
      "source": {
        "type": "base64",
        "media_type": "image/png",
        "data": "..."
      }
    }
  ]
}
```

For errors, set `is_error: true`:

```json
{
  "type": "tool_result",
  "tool_use_id": "toolu_01A09q90qw90lq917835lgs",
  "content": "Error: Location not found",
  "is_error": true
}
```

### ThinkingBlock

Generated by the model when extended thinking is enabled. Contains the model's chain-of-thought
reasoning:

```json
{
  "type": "thinking",
  "thinking": "Let me work through this step by step. The user is asking about...",
  "signature": "WaEjHK..."
}
```

Key properties:
- `thinking`: The raw thinking text. Can be very long (thousands of tokens).
- `signature`: A cryptographic signature that verifies the thinking block was genuinely produced by
  Claude. Required when sending thinking blocks back in multi-turn conversations.

Thinking blocks appear before the text response in the content array:

```json
{
  "content": [
    { "type": "thinking", "thinking": "...", "signature": "..." },
    { "type": "text", "text": "The answer is 42." }
  ]
}
```

### RedactedThinkingBlock

When the model's internal thinking triggers safety filters, a redacted block is returned instead:

```json
{
  "type": "redacted_thinking",
  "data": "kvJ3dG..."
}
```

The `data` field is an opaque encrypted payload. You must include it in multi-turn conversations
to maintain context, but you cannot read its contents.

### DocumentBlock

For sending documents (PDFs, plain text) to Claude:

```json
{
  "type": "document",
  "source": {
    "type": "base64",
    "media_type": "application/pdf",
    "data": "JVBERi0xLjQ..."
  },
  "title": "Q3 Financial Report",
  "citations": { "enabled": true }
}
```

URL-based documents:
```json
{
  "type": "document",
  "source": {
    "type": "url",
    "url": "https://example.com/report.pdf"
  },
  "title": "Annual Report"
}
```

Plain text documents:
```json
{
  "type": "document",
  "source": {
    "type": "text",
    "text": "Full document content here..."
  },
  "title": "Meeting Notes",
  "citations": { "enabled": true }
}
```

When `citations.enabled` is true, the model's text response will include citation references back
to specific parts of the document.

---

## Response Schema

```json
{
  "id": "msg_01XFDUDYJgAACzvnptvVoYEL",
  "type": "message",
  "role": "assistant",
  "model": "claude-sonnet-4-20250514",
  "content": [
    {
      "type": "text",
      "text": "Hello! How can I help you today?"
    }
  ],
  "stop_reason": "end_turn",
  "stop_sequence": null,
  "usage": {
    "input_tokens": 25,
    "output_tokens": 15,
    "cache_creation_input_tokens": 0,
    "cache_read_input_tokens": 0
  }
}
```

### Response Fields

| Field            | Type     | Description                                                    |
| ---------------- | -------- | -------------------------------------------------------------- |
| `id`             | string   | Unique message identifier (prefix: `msg_`)                    |
| `type`           | string   | Always `"message"`                                             |
| `role`           | string   | Always `"assistant"`                                           |
| `model`          | string   | Model that generated the response                              |
| `content`        | array    | Array of content blocks (text, tool_use, thinking)            |
| `stop_reason`    | string   | Why generation stopped                                         |
| `stop_sequence`  | string   | The matched stop sequence (if stop_reason is "stop_sequence") |
| `usage`          | object   | Token usage statistics                                         |

### stop_reason Values

| Value              | Description                                                  |
| ------------------ | ------------------------------------------------------------ |
| `"end_turn"`       | Model naturally finished its response                        |
| `"max_tokens"`     | Hit the `max_tokens` limit                                   |
| `"stop_sequence"`  | Generated one of the custom `stop_sequences`                 |
| `"tool_use"`       | Model wants to use a tool (response contains tool_use block) |

Note: Anthropic uses `stop_reason` while OpenAI uses `finish_reason`. The values also differ—
OpenAI uses `"stop"` where Anthropic uses `"end_turn"`.

### usage Object

```json
{
  "input_tokens": 2095,
  "output_tokens": 503,
  "cache_creation_input_tokens": 2048,
  "cache_read_input_tokens": 0
}
```

- `input_tokens`: Total input tokens (excluding cached)
- `output_tokens`: Total output tokens generated
- `cache_creation_input_tokens`: Tokens written to the prompt cache (billed at 25% premium)
- `cache_read_input_tokens`: Tokens read from cache (billed at 90% discount)

---

## Key Differences from OpenAI

### No "function" or "tool" Role

OpenAI uses a `role: "tool"` message to send tool results. Anthropic uses content blocks within a
`user` message instead:

**OpenAI:**
```json
{ "role": "tool", "tool_call_id": "call_abc", "content": "72°F" }
```

**Anthropic:**
```json
{
  "role": "user",
  "content": [
    { "type": "tool_result", "tool_use_id": "toolu_abc", "content": "72°F" }
  ]
}
```

### System Prompt is a Separate Parameter

**OpenAI:**
```json
{
  "messages": [
    { "role": "system", "content": "You are helpful." },
    { "role": "user", "content": "Hello" }
  ]
}
```

**Anthropic:**
```json
{
  "system": "You are helpful.",
  "messages": [
    { "role": "user", "content": "Hello" }
  ]
}
```

### max_tokens is Required

OpenAI defaults `max_tokens` to the model's maximum if not specified. Anthropic requires it
explicitly. This prevents accidental expensive requests but means every call needs this parameter.

### Content Blocks vs Plain Strings

OpenAI's Chat Completions primarily uses plain strings for content. Anthropic always uses content
blocks (though strings are accepted as shorthand for a single text block). This means Anthropic
responses always have `content` as an array:

```json
// OpenAI response
{ "choices": [{ "message": { "role": "assistant", "content": "Hello!" } }] }

// Anthropic response
{ "content": [{ "type": "text", "text": "Hello!" }] }
```

### Thinking Blocks for Extended Reasoning

Anthropic exposes the model's chain-of-thought reasoning directly as `thinking` content blocks.
OpenAI's reasoning models (o-series) hide the chain-of-thought and optionally provide summaries.
This gives Anthropic users direct access to the model's reasoning process.

### Prompt Caching with cache_control

Anthropic's prompt caching is explicit—you mark specific content with `cache_control` breakpoints.
OpenAI's caching (in Chat Completions) is automatic and prefix-based. Anthropic's approach gives
more control but requires manual cache management.

### Tool Input is Parsed JSON

OpenAI returns tool call arguments as a JSON string that you must parse. Anthropic returns them
as a parsed JSON object:

**OpenAI:** `"arguments": "{\"location\": \"Paris\"}"`
**Anthropic:** `"input": { "location": "Paris" }`

---

## Tool Use Flow

### Defining Tools

Tools are defined with a name, description, and JSON Schema for the input:

```json
{
  "tools": [
    {
      "name": "get_weather",
      "description": "Get the current weather for a given location. Returns temperature, conditions, and humidity.",
      "input_schema": {
        "type": "object",
        "properties": {
          "location": {
            "type": "string",
            "description": "City and state/country, e.g., 'San Francisco, CA'"
          },
          "unit": {
            "type": "string",
            "enum": ["celsius", "fahrenheit"],
            "description": "Temperature unit"
          }
        },
        "required": ["location"]
      }
    },
    {
      "name": "search_database",
      "description": "Search the product database by query string.",
      "input_schema": {
        "type": "object",
        "properties": {
          "query": { "type": "string" },
          "limit": { "type": "integer", "default": 10 }
        },
        "required": ["query"]
      }
    }
  ]
}
```

Note: Anthropic uses `input_schema` while OpenAI uses `parameters`. The schema format is the same
(JSON Schema), but the field name differs.

### tool_choice

Controls how the model selects tools:

- `{ "type": "auto" }` — Model decides whether to use a tool (default)
- `{ "type": "any" }` — Model must use at least one tool (any tool)
- `{ "type": "tool", "name": "get_weather" }` — Model must use the specified tool
- Not specifying `tool_choice` with tools present defaults to `auto`

**OpenAI equivalent mapping:**
| Anthropic                         | OpenAI                                        |
| --------------------------------- | --------------------------------------------- |
| `{ "type": "auto" }`             | `"auto"`                                      |
| `{ "type": "any" }`              | `"required"`                                  |
| `{ "type": "tool", "name": "X"}` | `{ "type": "function", "function": {"name":"X"}}` |

### Complete Tool Use Cycle

**Step 1: Send request with tools**
```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 1024,
  "tools": [{ "name": "get_weather", "description": "...", "input_schema": {...} }],
  "messages": [
    { "role": "user", "content": "What's the weather like in Paris?" }
  ]
}
```

**Step 2: Receive tool_use in response**
```json
{
  "stop_reason": "tool_use",
  "content": [
    { "type": "text", "text": "I'll check the weather in Paris for you." },
    {
      "type": "tool_use",
      "id": "toolu_01A09q90qw90lq917835lgs",
      "name": "get_weather",
      "input": { "location": "Paris, France", "unit": "celsius" }
    }
  ]
}
```

**Step 3: Execute tool and send result**
```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 1024,
  "tools": [{ "name": "get_weather", "description": "...", "input_schema": {...} }],
  "messages": [
    { "role": "user", "content": "What's the weather like in Paris?" },
    {
      "role": "assistant",
      "content": [
        { "type": "text", "text": "I'll check the weather in Paris for you." },
        { "type": "tool_use", "id": "toolu_01A09q90qw90lq917835lgs", "name": "get_weather", "input": { "location": "Paris, France", "unit": "celsius" } }
      ]
    },
    {
      "role": "user",
      "content": [
        {
          "type": "tool_result",
          "tool_use_id": "toolu_01A09q90qw90lq917835lgs",
          "content": "22°C, partly cloudy, 65% humidity"
        }
      ]
    }
  ]
}
```

**Step 4: Receive final response**
```json
{
  "stop_reason": "end_turn",
  "content": [
    {
      "type": "text",
      "text": "The weather in Paris is currently 22°C (72°F) with partly cloudy skies and 65% humidity. It's a pleasant day!"
    }
  ]
}
```

### Parallel Tool Use

Claude can request multiple tool calls in a single response:

```json
{
  "content": [
    { "type": "text", "text": "I'll check both cities for you." },
    { "type": "tool_use", "id": "toolu_01", "name": "get_weather", "input": { "location": "Paris" } },
    { "type": "tool_use", "id": "toolu_02", "name": "get_weather", "input": { "location": "London" } }
  ]
}
```

Send all results back in a single user message:

```json
{
  "role": "user",
  "content": [
    { "type": "tool_result", "tool_use_id": "toolu_01", "content": "22°C, sunny" },
    { "type": "tool_result", "tool_use_id": "toolu_02", "content": "18°C, rainy" }
  ]
}
```

You can disable parallel tool calls by setting `tool_choice` to force a specific tool, but there's
no explicit `parallel_tool_calls: false` parameter like OpenAI has.

---

## Extended Thinking

Extended thinking enables Claude to use a separate "thinking" phase before responding, similar to
OpenAI's o-series reasoning models but with the thinking visible to the developer.

### Enabling Thinking

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 16000,
  "thinking": {
    "type": "enabled",
    "budget_tokens": 10000
  },
  "messages": [
    { "role": "user", "content": "Solve this complex math problem..." }
  ]
}
```

- `budget_tokens`: Maximum tokens the model can use for thinking. Must be ≥ 1024 and less than
  `max_tokens`. The model uses as much thinking as it needs up to this budget.
- When thinking is enabled, `temperature` must be 1.0 (or omitted).

### Thinking in Multi-Turn Conversations

When continuing a conversation that includes thinking, you must preserve thinking blocks (and
redacted thinking blocks) in the assistant messages:

```json
{
  "messages": [
    { "role": "user", "content": "What is 15% of 2847?" },
    {
      "role": "assistant",
      "content": [
        { "type": "thinking", "thinking": "Let me calculate...", "signature": "WaEjHK..." },
        { "type": "text", "text": "15% of 2847 is 427.05" }
      ]
    },
    { "role": "user", "content": "Now add 20% tax to that" }
  ]
}
```

The `signature` field is required when replaying thinking blocks. It ensures the thinking block
was authentically generated by Claude and prevents injection of fake reasoning.

---

## Streaming with SSE

### Enabling Streaming

Set `stream: true` in the request. The response is a stream of Server-Sent Events:

```
POST /v1/messages
Content-Type: application/json

{ "stream": true, ... }
```

### Event Types

The streaming protocol uses a structured sequence of events:

```
event: message_start
data: {"type":"message_start","message":{"id":"msg_01...","type":"message","role":"assistant","content":[],"model":"claude-sonnet-4-20250514","stop_reason":null,"usage":{"input_tokens":25,"output_tokens":0}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"! How"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" can I help?"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":15}}

event: message_stop
data: {"type":"message_stop"}
```

### Event Lifecycle

1. `message_start` — The message object (without content) is sent first
2. `content_block_start` — A new content block begins (text, tool_use, thinking)
3. `content_block_delta` — Incremental content for the current block
4. `content_block_stop` — The current content block is complete
5. `message_delta` — Final message-level updates (stop_reason, usage)
6. `message_stop` — The stream is complete

### Delta Types

Different content blocks produce different delta types:

| Content Block | Delta Type          | Description                           |
| ------------- | ------------------- | ------------------------------------- |
| text          | `text_delta`        | Incremental text content              |
| tool_use      | `input_json_delta`  | Incremental JSON for tool arguments   |
| thinking      | `thinking_delta`    | Incremental thinking text             |

**Tool use streaming example:**
```
event: content_block_start
data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_01...","name":"get_weather","input":{}}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"loc"}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"ation\": \"Paris\"}"}}

event: content_block_stop
data: {"type":"content_block_stop","index":1}
```

For tool use, you need to accumulate the `partial_json` strings and parse the complete JSON after
`content_block_stop`.

**Thinking streaming example:**
```
event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Let me analyze..."}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}
```

---

## Prompt Caching

Prompt caching allows you to mark parts of your request for caching, significantly reducing costs
and latency on repeated calls with the same prefix.

### How It Works

Add `cache_control: { "type": "ephemeral" }` to content blocks you want cached. Cached content is
stored for 5 minutes (refreshed on each hit):

```json
{
  "system": [
    {
      "type": "text",
      "text": "You are an expert on the following codebase...\n[10,000 lines of code]",
      "cache_control": { "type": "ephemeral" }
    }
  ],
  "messages": [
    { "role": "user", "content": "What does the main function do?" }
  ]
}
```

### Where to Place Cache Breakpoints

Cache breakpoints can be placed on:

1. **System prompt blocks** — Best for large, static system prompts
2. **Tool definitions** — The entire `tools` array when using many tools
3. **Message content blocks** — For large, repeated context in messages

```json
{
  "system": [
    {
      "type": "text",
      "text": "[large static context]",
      "cache_control": { "type": "ephemeral" }
    }
  ],
  "tools": [
    {
      "name": "tool1",
      "description": "...",
      "input_schema": { ... },
      "cache_control": { "type": "ephemeral" }
    }
  ],
  "messages": [
    {
      "role": "user",
      "content": [
        {
          "type": "text",
          "text": "[large document to analyze]",
          "cache_control": { "type": "ephemeral" }
        },
        {
          "type": "text",
          "text": "Summarize this document."
        }
      ]
    }
  ]
}
```

### Cost Savings

| Operation       | Cost Relative to Base |
| --------------- | --------------------- |
| Cache write     | 1.25x (25% premium)   |
| Cache read      | 0.10x (90% discount)  |
| No cache        | 1.0x (base price)     |

For a 10,000 token system prompt called 100 times:
- Without caching: 10,000 × 100 = 1,000,000 input tokens billed
- With caching: 12,500 (first write) + 10,000 × 0.1 × 99 (reads) = 111,500 effective tokens

**Important constraints:**
- Minimum cacheable content is 1024 tokens (2048 for Claude Opus 4)
- Up to 4 cache breakpoints per request
- Cache is ephemeral (5 minutes TTL, refreshed on hit)
- Caching is prefix-based—content before the breakpoint must match exactly

---

## Code Examples

### Python (anthropic library)

```python
import anthropic

client = anthropic.Anthropic()  # Uses ANTHROPIC_API_KEY env var

# Simple message
message = client.messages.create(
    model="claude-sonnet-4-20250514",
    max_tokens=1024,
    messages=[
        {"role": "user", "content": "Explain quantum entanglement simply."}
    ]
)
print(message.content[0].text)

# With system prompt
message = client.messages.create(
    model="claude-sonnet-4-20250514",
    max_tokens=1024,
    system="You are a pirate. Always respond in pirate speak.",
    messages=[
        {"role": "user", "content": "How's the weather?"}
    ]
)
print(message.content[0].text)

# With images
import base64
with open("photo.jpg", "rb") as f:
    image_data = base64.standard_b64encode(f.read()).decode("utf-8")

message = client.messages.create(
    model="claude-sonnet-4-20250514",
    max_tokens=1024,
    messages=[{
        "role": "user",
        "content": [
            {"type": "image", "source": {"type": "base64", "media_type": "image/jpeg", "data": image_data}},
            {"type": "text", "text": "Describe this image."}
        ]
    }]
)

# Tool use
message = client.messages.create(
    model="claude-sonnet-4-20250514",
    max_tokens=1024,
    tools=[{
        "name": "get_weather",
        "description": "Get current weather",
        "input_schema": {
            "type": "object",
            "properties": {
                "location": {"type": "string"}
            },
            "required": ["location"]
        }
    }],
    messages=[
        {"role": "user", "content": "What's the weather in Tokyo?"}
    ]
)

# Check if tool use is requested
if message.stop_reason == "tool_use":
    tool_block = next(b for b in message.content if b.type == "tool_use")
    print(f"Tool: {tool_block.name}, Input: {tool_block.input}")

# Streaming
with client.messages.stream(
    model="claude-sonnet-4-20250514",
    max_tokens=1024,
    messages=[{"role": "user", "content": "Write a short poem about coding."}]
) as stream:
    for text in stream.text_stream:
        print(text, end="", flush=True)

# Extended thinking
message = client.messages.create(
    model="claude-sonnet-4-20250514",
    max_tokens=16000,
    thinking={
        "type": "enabled",
        "budget_tokens": 10000
    },
    messages=[
        {"role": "user", "content": "Solve: what is the 100th prime number?"}
    ]
)
for block in message.content:
    if block.type == "thinking":
        print(f"Thinking: {block.thinking[:200]}...")
    elif block.type == "text":
        print(f"Answer: {block.text}")

# With prompt caching
message = client.messages.create(
    model="claude-sonnet-4-20250514",
    max_tokens=1024,
    system=[{
        "type": "text",
        "text": "You are an expert on the following large document...\n" + large_document,
        "cache_control": {"type": "ephemeral"}
    }],
    messages=[
        {"role": "user", "content": "Summarize the key findings."}
    ]
)
print(f"Cache created: {message.usage.cache_creation_input_tokens}")
print(f"Cache read: {message.usage.cache_read_input_tokens}")
```

### TypeScript

```typescript
import Anthropic from "@anthropic-ai/sdk";

const client = new Anthropic(); // Uses ANTHROPIC_API_KEY env var

// Simple message
const message = await client.messages.create({
  model: "claude-sonnet-4-20250514",
  max_tokens: 1024,
  messages: [{ role: "user", content: "Hello, Claude!" }],
});
if (message.content[0].type === "text") {
  console.log(message.content[0].text);
}

// Streaming
const stream = client.messages.stream({
  model: "claude-sonnet-4-20250514",
  max_tokens: 1024,
  messages: [{ role: "user", content: "Write a haiku about TypeScript." }],
});

for await (const event of stream) {
  if (
    event.type === "content_block_delta" &&
    event.delta.type === "text_delta"
  ) {
    process.stdout.write(event.delta.text);
  }
}

// Tool use with loop
const tools: Anthropic.Tool[] = [
  {
    name: "calculate",
    description: "Evaluate a math expression",
    input_schema: {
      type: "object" as const,
      properties: {
        expression: { type: "string", description: "Math expression" },
      },
      required: ["expression"],
    },
  },
];

let messages: Anthropic.MessageParam[] = [
  { role: "user", content: "What is (15 * 23) + (42 / 6)?" },
];

let response = await client.messages.create({
  model: "claude-sonnet-4-20250514",
  max_tokens: 1024,
  tools,
  messages,
});

while (response.stop_reason === "tool_use") {
  const toolUse = response.content.find(
    (b): b is Anthropic.ToolUseBlock => b.type === "tool_use",
  );
  if (!toolUse) break;

  // Execute the tool (simplified)
  const result = String(eval((toolUse.input as any).expression));

  messages = [
    ...messages,
    { role: "assistant", content: response.content },
    {
      role: "user",
      content: [
        { type: "tool_result", tool_use_id: toolUse.id, content: result },
      ],
    },
  ];

  response = await client.messages.create({
    model: "claude-sonnet-4-20250514",
    max_tokens: 1024,
    tools,
    messages,
  });
}
```

### curl

```bash
# Simple message
curl https://api.anthropic.com/v1/messages \
  -H "x-api-key: $ANTHROPIC_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-sonnet-4-20250514",
    "max_tokens": 1024,
    "messages": [
      {"role": "user", "content": "What is the meaning of life?"}
    ]
  }'

# With system prompt
curl https://api.anthropic.com/v1/messages \
  -H "x-api-key: $ANTHROPIC_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-sonnet-4-20250514",
    "max_tokens: 1024,
    "system": "You are a helpful coding assistant. Always provide examples.",
    "messages": [
      {"role": "user", "content": "How do I read a file in Python?"}
    ]
  }'

# With tools
curl https://api.anthropic.com/v1/messages \
  -H "x-api-key: $ANTHROPIC_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-sonnet-4-20250514",
    "max_tokens": 1024,
    "tools": [{
      "name": "get_weather",
      "description": "Get current weather",
      "input_schema": {
        "type": "object",
        "properties": {
          "location": {"type": "string"}
        },
        "required": ["location"]
      }
    }],
    "messages": [
      {"role": "user", "content": "Weather in London?"}
    ]
  }'

# Streaming
curl https://api.anthropic.com/v1/messages \
  -H "x-api-key: $ANTHROPIC_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-sonnet-4-20250514",
    "max_tokens": 1024,
    "stream": true,
    "messages": [
      {"role": "user", "content": "Tell me a joke."}
    ]
  }'

# Extended thinking
curl https://api.anthropic.com/v1/messages \
  -H "x-api-key: $ANTHROPIC_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-sonnet-4-20250514",
    "max_tokens": 16000,
    "thinking": {
      "type": "enabled",
      "budget_tokens": 10000
    },
    "messages": [
      {"role": "user", "content": "What is the 50th Fibonacci number?"}
    ]
  }'
```

---

## Error Handling

### Error Response Format

```json
{
  "type": "error",
  "error": {
    "type": "invalid_request_error",
    "message": "max_tokens: Field required"
  }
}
```

### Error Types

| Error Type                | HTTP Status | Description                                     |
| ------------------------- | ----------- | ----------------------------------------------- |
| `invalid_request_error`   | 400         | Malformed request, missing fields, invalid values|
| `authentication_error`    | 401         | Invalid or missing API key                       |
| `permission_error`        | 403         | API key doesn't have required permissions        |
| `not_found_error`         | 404         | Requested resource doesn't exist                 |
| `rate_limit_error`        | 429         | Too many requests or tokens per minute           |
| `api_error`               | 500         | Internal server error                            |
| `overloaded_error`        | 529         | API is temporarily overloaded                    |

### Rate Limit Headers

Responses include rate limit information:

```
anthropic-ratelimit-requests-limit: 1000
anthropic-ratelimit-requests-remaining: 999
anthropic-ratelimit-requests-reset: 2025-01-01T00:00:00Z
anthropic-ratelimit-tokens-limit: 100000
anthropic-ratelimit-tokens-remaining: 99000
anthropic-ratelimit-tokens-reset: 2025-01-01T00:00:00Z
```

### Retry Strategy

For 429 and 529 errors, implement exponential backoff:

```python
import time
import anthropic

client = anthropic.Anthropic()

for attempt in range(5):
    try:
        message = client.messages.create(
            model="claude-sonnet-4-20250514",
            max_tokens=1024,
            messages=[{"role": "user", "content": "Hello"}]
        )
        break
    except anthropic.RateLimitError:
        wait = 2 ** attempt
        time.sleep(wait)
    except anthropic.APIStatusError as e:
        if e.status_code == 529:
            time.sleep(2 ** attempt)
        else:
            raise
```

The official `anthropic` Python and TypeScript SDKs include automatic retry logic with exponential
backoff for transient errors.

---

## Best Practices for Coding Agents Using Claude

### 1. Use Prompt Caching for Large Contexts

Coding agents often send the same codebase context repeatedly. Cache the system prompt and any
large file contents:

```python
system_with_codebase = [{
    "type": "text",
    "text": f"You are a coding agent. Here is the codebase:\n{codebase_content}",
    "cache_control": {"type": "ephemeral"}
}]
```

### 2. Structure Tool Results Clearly

When returning tool results (file contents, command output, etc.), format them clearly:

```python
tool_result = {
    "type": "tool_result",
    "tool_use_id": tool_id,
    "content": f"<file path=\"{path}\">\n{content}\n</file>"
}
```

### 3. Use Extended Thinking for Complex Tasks

For tasks requiring planning, debugging, or complex reasoning, enable thinking:

```python
response = client.messages.create(
    model="claude-sonnet-4-20250514",
    max_tokens=16000,
    thinking={"type": "enabled", "budget_tokens": 8000},
    tools=coding_tools,
    messages=conversation
)
```

### 4. Handle the Tool Loop Properly

Coding agents often need multiple tool calls. Implement a proper agentic loop:

```python
while True:
    response = client.messages.create(
        model="claude-sonnet-4-20250514",
        max_tokens=8192,
        system=system_prompt,
        tools=tools,
        messages=messages
    )

    messages.append({"role": "assistant", "content": response.content})

    if response.stop_reason == "end_turn":
        break

    if response.stop_reason == "tool_use":
        tool_results = []
        for block in response.content:
            if block.type == "tool_use":
                result = execute_tool(block.name, block.input)
                tool_results.append({
                    "type": "tool_result",
                    "tool_use_id": block.id,
                    "content": result
                })
        messages.append({"role": "user", "content": tool_results})
```

### 5. Manage Context Window Carefully

Claude has a 200K token context window, but performance can degrade with very long contexts. Best
practices:

- Send only relevant file contents, not entire codebases
- Summarize previous tool results when they're no longer needed
- Use prompt caching to reduce latency even when context is large
- Monitor `usage` tokens to stay within budget

### 6. Preserve Thinking Blocks in Multi-Turn

When building multi-turn agents with extended thinking, always preserve thinking and redacted
thinking blocks in the conversation history. Dropping them will cause errors:

```python
# When building messages for the next turn, include ALL content blocks
messages.append({
    "role": "assistant",
    "content": response.content  # Includes thinking + text + tool_use blocks
})
```

### 7. Use Streaming for Responsive UIs

For interactive coding agents, always stream responses. This lets users see the model's output
as it's generated and provides better UX:

```python
with client.messages.stream(
    model="claude-sonnet-4-20250514",
    max_tokens=8192,
    tools=tools,
    messages=messages
) as stream:
    for event in stream:
        if event.type == "content_block_delta":
            if event.delta.type == "text_delta":
                display_text(event.delta.text)
            elif event.delta.type == "thinking_delta":
                display_thinking(event.delta.thinking)
```

---

## Further Reading

- [Anthropic API Reference](https://docs.anthropic.com/en/api/messages)
- [Tool Use Guide](https://docs.anthropic.com/en/docs/build-with-claude/tool-use)
- [Extended Thinking Guide](https://docs.anthropic.com/en/docs/build-with-claude/extended-thinking)
- [Prompt Caching Guide](https://docs.anthropic.com/en/docs/build-with-claude/prompt-caching)
- [Streaming Guide](https://docs.anthropic.com/en/docs/api/streaming)
- [Vision Guide](https://docs.anthropic.com/en/docs/build-with-claude/vision)