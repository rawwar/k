---
title: API Anatomy OpenAI
description: A detailed walkthrough of the OpenAI Chat Completions API including authentication, function calling format, and tool choice parameters.
---

# API Anatomy: OpenAI

> **What you'll learn:**
> - The structure of OpenAI's Chat Completions API request and response including function definitions
> - How OpenAI's tool_calls response format differs from Anthropic's content block approach
> - The OpenAI-specific parameters like tool_choice, response_format, and parallel_tool_calls

Even if you plan to use Anthropic's API as your primary provider, understanding OpenAI's API is valuable. A well-designed agent supports multiple providers, and many of the concepts translate between them. This subchapter mirrors the Anthropic API walkthrough, highlighting the structural differences that your provider abstraction layer will need to handle.

## Authentication and Headers

OpenAI uses Bearer token authentication:

```
POST https://api.openai.com/v1/chat/completions
Content-Type: application/json
Authorization: Bearer sk-proj-...
```

| Header | Purpose |
|---|---|
| `Authorization` | Bearer token with your API key |
| `Content-Type` | Always `application/json` |

Unlike Anthropic, there is no version header -- OpenAI manages versioning through the endpoint URL and model names.

In Rust:

```rust
let response = client
    .post("https://api.openai.com/v1/chat/completions")
    .header("Authorization", format!("Bearer {}", api_key))
    .header("Content-Type", "application/json")
    .json(&request_body)
    .send()
    .await?;
```

## Complete Request Body

Here is a complete request for a coding agent with tools:

```json
{
  "model": "gpt-4o",
  "max_completion_tokens": 8192,
  "temperature": 0,
  "messages": [
    {
      "role": "system",
      "content": "You are an expert Rust coding assistant with access to file and shell tools. Always read files before editing them. Verify changes with cargo check."
    },
    {
      "role": "user",
      "content": "The build is failing with a type error in src/main.rs. Can you fix it?"
    }
  ],
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "read_file",
        "description": "Read the contents of a file. Returns the file content as a string.",
        "parameters": {
          "type": "object",
          "properties": {
            "path": {
              "type": "string",
              "description": "Absolute or relative path to the file"
            }
          },
          "required": ["path"]
        }
      }
    },
    {
      "type": "function",
      "function": {
        "name": "write_file",
        "description": "Write content to a file, creating it if necessary.",
        "parameters": {
          "type": "object",
          "properties": {
            "path": {
              "type": "string",
              "description": "Path to the file to write"
            },
            "content": {
              "type": "string",
              "description": "Complete file content to write"
            }
          },
          "required": ["path", "content"]
        }
      }
    },
    {
      "type": "function",
      "function": {
        "name": "shell",
        "description": "Execute a shell command. Returns stdout, stderr, and exit code.",
        "parameters": {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "description": "The shell command to execute"
            }
          },
          "required": ["command"]
        }
      }
    }
  ],
  "tool_choice": "auto"
}
```

### Key Structural Differences from Anthropic

**System prompt location:** OpenAI places the system prompt as the first message with `role: "system"`. Anthropic uses a separate top-level `system` field.

**Tool definition wrapper:** OpenAI wraps each tool in a `{"type": "function", "function": {...}}` envelope. The schema key is `parameters` instead of Anthropic's `input_schema`.

**Max tokens field:** OpenAI uses `max_completion_tokens` (newer models) or `max_tokens` (older models) instead of just `max_tokens`.

**Tool choice:** OpenAI's `tool_choice` accepts string values (`"auto"`, `"required"`, `"none"`) in addition to the object format.

Here is a side-by-side comparison of tool definitions:

```json
// Anthropic
{
  "name": "read_file",
  "description": "Read a file's contents",
  "input_schema": {
    "type": "object",
    "properties": {
      "path": {"type": "string"}
    },
    "required": ["path"]
  }
}

// OpenAI
{
  "type": "function",
  "function": {
    "name": "read_file",
    "description": "Read a file's contents",
    "parameters": {
      "type": "object",
      "properties": {
        "path": {"type": "string"}
      },
      "required": ["path"]
    }
  }
}
```

The schema content is identical -- it is the wrapper structure that differs. Your provider abstraction stores the shared schema once and wraps it differently for each provider.

## Response Structure

Here is a complete response where the model calls a tool:

```json
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  "created": 1710000000,
  "model": "gpt-4o-2024-08-06",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": null,
        "tool_calls": [
          {
            "id": "call_abc123",
            "type": "function",
            "function": {
              "name": "read_file",
              "arguments": "{\"path\": \"src/main.rs\"}"
            }
          }
        ]
      },
      "finish_reason": "tool_calls"
    }
  ],
  "usage": {
    "prompt_tokens": 1450,
    "completion_tokens": 25,
    "total_tokens": 1475
  }
}
```

### Critical Difference: Tool Calls as a Separate Field

In Anthropic's format, tool calls are content blocks within the `content` array -- mixed with text. In OpenAI's format, tool calls are a separate `tool_calls` field on the message, and `content` is `null` when the model only makes tool calls.

When the model includes text alongside a tool call in OpenAI's format:

```json
{
  "message": {
    "role": "assistant",
    "content": "I'll read the file to check the error.",
    "tool_calls": [
      {
        "id": "call_abc123",
        "type": "function",
        "function": {
          "name": "read_file",
          "arguments": "{\"path\": \"src/main.rs\"}"
        }
      }
    ]
  }
}
```

Both `content` (text) and `tool_calls` can be present simultaneously.

### The `arguments` String

An important detail: OpenAI's `arguments` field is a **JSON string**, not a parsed JSON object. You need to parse it separately:

```rust
// OpenAI: arguments is a string that contains JSON
let args_str = &tool_call.function.arguments; // "{\"path\": \"src/main.rs\"}"
let args: serde_json::Value = serde_json::from_str(args_str)?;

// Anthropic: input is already a parsed JSON object
let args = &tool_use.input; // {"path": "src/main.rs"}
```

This double-serialization is a common source of bugs when first implementing OpenAI support. The model occasionally generates invalid JSON in the arguments string, so your parsing code needs error handling.

## Tool Result Messages

OpenAI uses a dedicated `tool` role for tool results, rather than Anthropic's approach of embedding tool results in user messages:

```json
{
  "role": "tool",
  "tool_call_id": "call_abc123",
  "content": "fn main() {\n    let x: i32 = \"hello\";\n    println!(\"{}\", x);\n}"
}
```

| Field | Purpose |
|---|---|
| `role` | Must be `"tool"` |
| `tool_call_id` | Links back to the tool call ID in the assistant message |
| `content` | The tool execution result as a string |

When the model makes multiple tool calls, you send multiple tool messages:

```json
[
  {
    "role": "assistant",
    "content": null,
    "tool_calls": [
      {"id": "call_AAA", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\":\"src/main.rs\"}"}},
      {"id": "call_BBB", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\":\"Cargo.toml\"}"}}
    ]
  },
  {
    "role": "tool",
    "tool_call_id": "call_AAA",
    "content": "// main.rs contents..."
  },
  {
    "role": "tool",
    "tool_call_id": "call_BBB",
    "content": "# Cargo.toml contents..."
  }
]
```

Each tool result is a separate message, all with role `"tool"`. The message ordering must match: the assistant message comes first, followed by all tool result messages, before the next user or assistant message.

::: python Coming from Python
The OpenAI Python SDK handles the `arguments` string parsing automatically when you use Pydantic function schemas. In Rust, you parse it manually with `serde_json::from_str`. The explicit parsing step actually makes your code more robust -- you handle the error case explicitly rather than having it silently propagate.
:::

## Finish Reasons

OpenAI uses `finish_reason` instead of Anthropic's `stop_reason`:

| Value | Meaning | Anthropic Equivalent |
|---|---|---|
| `"stop"` | Model finished naturally | `"end_turn"` |
| `"tool_calls"` | Model wants to call tools | `"tool_use"` |
| `"length"` | Hit max token limit | `"max_tokens"` |
| `"content_filter"` | Content was filtered by safety | No direct equivalent |

Your provider abstraction maps these to a common enum:

```rust
enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    ContentFilter,
}

impl From<&str> for StopReason {
    fn from(s: &str) -> Self {
        match s {
            // Anthropic
            "end_turn" => StopReason::EndTurn,
            "tool_use" => StopReason::ToolUse,
            "max_tokens" => StopReason::MaxTokens,
            // OpenAI
            "stop" => StopReason::EndTurn,
            "tool_calls" => StopReason::ToolUse,
            "length" => StopReason::MaxTokens,
            "content_filter" => StopReason::ContentFilter,
            _ => StopReason::EndTurn,
        }
    }
}
```

## OpenAI-Specific Parameters

Several parameters are unique to OpenAI:

**`parallel_tool_calls`:** Controls whether the model can make multiple tool calls in one response. Default is `true`. Set to `false` if your agent cannot handle parallel execution:

```json
{
  "parallel_tool_calls": false
}
```

**`response_format`:** Enables JSON mode or Structured Outputs (covered in [JSON Mode](/linear/03-understanding-llms/09-json-mode)):

```json
{
  "response_format": {"type": "json_object"}
}
```

**`seed`:** For reproducible outputs (covered in [Temperature and Sampling](/linear/03-understanding-llms/04-temperature-and-sampling)):

```json
{
  "seed": 42
}
```

**`logprobs`:** Returns log probabilities for generated tokens. Useful for confidence estimation but rarely needed for agents:

```json
{
  "logprobs": true,
  "top_logprobs": 5
}
```

## Usage and Token Tracking

OpenAI's usage object uses slightly different field names:

```json
{
  "usage": {
    "prompt_tokens": 1450,
    "completion_tokens": 25,
    "total_tokens": 1475
  }
}
```

Mapping to Anthropic:
- `prompt_tokens` = Anthropic's `input_tokens`
- `completion_tokens` = Anthropic's `output_tokens`
- `total_tokens` = sum (Anthropic does not include this)

Your token tracking code should normalize these into a common format:

```rust
struct TokenUsage {
    input_tokens: u32,
    output_tokens: u32,
}

// From Anthropic response
let usage = TokenUsage {
    input_tokens: response.usage.input_tokens,
    output_tokens: response.usage.output_tokens,
};

// From OpenAI response
let usage = TokenUsage {
    input_tokens: response.usage.prompt_tokens,
    output_tokens: response.usage.completion_tokens,
};
```

## Error Responses

OpenAI error responses follow a similar structure:

```json
{
  "error": {
    "message": "Rate limit reached for gpt-4o in organization org-xxx on tokens per min (TPM): Limit 30000, Used 29500, Requested 1500.",
    "type": "tokens",
    "param": null,
    "code": "rate_limit_exceeded"
  }
}
```

Common HTTP status codes:

| Status | Meaning | Action |
|---|---|---|
| 400 | Bad request | Fix the request |
| 401 | Invalid API key | Check credentials |
| 429 | Rate limit | Retry with backoff |
| 500 | Server error | Retry with backoff |
| 503 | Service unavailable | Retry with longer delay |

::: wild In the Wild
OpenCode, the open-source Go-based coding agent, implements provider support for both Anthropic and OpenAI APIs. It maintains a provider interface with separate implementations that handle the structural differences in tool definitions, message formats, and response parsing. The core agent loop works with a unified message type, and provider-specific serialization happens only at the HTTP boundary. This is the same pattern you will implement in Rust.
:::

## Key Takeaways

- OpenAI uses Bearer token auth, system messages in the messages array, and wraps tool definitions in a `{"type": "function", "function": {...}}` envelope with a `parameters` key instead of `input_schema`
- Tool calls appear as a separate `tool_calls` field on the assistant message (not content blocks), and the `arguments` field is a JSON string that needs separate parsing
- Tool results use a dedicated `tool` role with `tool_call_id` linking, rather than Anthropic's approach of `tool_result` blocks inside user messages
- The `finish_reason` field maps to Anthropic's `stop_reason`: `stop` = `end_turn`, `tool_calls` = `tool_use`, `length` = `max_tokens`
- Building a provider abstraction requires normalizing these differences: unified message types, common stop reason enums, and provider-specific serialization at the HTTP boundary
