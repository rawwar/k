---
title: API Anatomy Anthropic
description: A detailed walkthrough of the Anthropic Messages API including authentication, request structure, content blocks, and tool use format.
---

# API Anatomy: Anthropic

> **What you'll learn:**
> - The complete structure of an Anthropic Messages API request including headers, parameters, and body
> - How Anthropic represents tool definitions, tool use content blocks, and tool result messages
> - The specific response format including stop reasons, usage statistics, and content block types

This subchapter is a reference-style walkthrough of the Anthropic Messages API. You will use this API as the primary backend for your coding agent, so understanding every field in the request and response is important. We will go through a complete request-response cycle, annotating each field with its purpose and practical implications.

## Authentication and Headers

Every request to the Anthropic API requires these headers:

```
POST https://api.anthropic.com/v1/messages
Content-Type: application/json
x-api-key: sk-ant-api03-...
anthropic-version: 2023-06-01
```

| Header | Purpose |
|---|---|
| `x-api-key` | Your API key. Never hard-code this -- load from environment variable |
| `anthropic-version` | API version string. Pin this to avoid breaking changes |
| `Content-Type` | Always `application/json` |

The `anthropic-version` header is important. Anthropic can introduce breaking changes in new API versions, and pinning the version ensures your agent continues to work. Use the latest stable version when you start and update deliberately.

In Rust, you will set these headers on your HTTP client:

```rust
use reqwest::Client;

let client = Client::new();
let response = client
    .post("https://api.anthropic.com/v1/messages")
    .header("x-api-key", &api_key)
    .header("anthropic-version", "2023-06-01")
    .header("content-type", "application/json")
    .json(&request_body)
    .send()
    .await?;
```

## Complete Request Body

Here is a complete request body for a coding agent that includes a system prompt, tools, and conversation history:

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 8192,
  "temperature": 0,
  "system": "You are an expert Rust coding assistant with access to file and shell tools. Always read files before editing them. Verify changes with cargo check.",
  "tools": [
    {
      "name": "read_file",
      "description": "Read the contents of a file. Returns the file content as a string, or an error if the file does not exist.",
      "input_schema": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "Absolute or relative path to the file"
          }
        },
        "required": ["path"]
      }
    },
    {
      "name": "write_file",
      "description": "Write content to a file, creating it if it doesn't exist. Always read the file first to understand current contents.",
      "input_schema": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "Path to the file to write"
          },
          "content": {
            "type": "string",
            "description": "The complete file content to write"
          }
        },
        "required": ["path", "content"]
      }
    },
    {
      "name": "shell",
      "description": "Execute a shell command. Returns stdout, stderr, and exit code. Use for builds, tests, git, and file operations.",
      "input_schema": {
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
  ],
  "messages": [
    {
      "role": "user",
      "content": "The build is failing with a type error in src/main.rs. Can you fix it?"
    }
  ]
}
```

Let's annotate each top-level field:

| Field | Type | Purpose |
|---|---|---|
| `model` | string | Which Claude model to use. Determines capability, speed, and cost |
| `max_tokens` | integer | Maximum tokens the model can generate. Required field |
| `temperature` | float | Sampling temperature (0-1). Optional, defaults to 1.0 |
| `system` | string | System prompt. Optional but strongly recommended for agents |
| `tools` | array | Tool definitions with JSON Schema. Optional |
| `messages` | array | Conversation history. Required, must start with user role |

Additional optional fields you may use:

| Field | Type | Purpose |
|---|---|---|
| `stream` | boolean | Enable SSE streaming (covered in previous subchapter) |
| `tool_choice` | object | Control tool use behavior: `auto`, `any`, or specific tool |
| `stop_sequences` | array | Custom stop sequences that halt generation |
| `top_p` | float | Nucleus sampling parameter |
| `top_k` | integer | Top-k sampling parameter |
| `metadata` | object | Custom metadata, e.g., `{"user_id": "user123"}` for abuse tracking |

## Response Structure

Here is a complete non-streaming response where the model calls a tool:

```json
{
  "id": "msg_01XFDUDYJgAACzvnptvVer6C",
  "type": "message",
  "role": "assistant",
  "model": "claude-sonnet-4-20250514",
  "content": [
    {
      "type": "text",
      "text": "I'll read the file to see the error."
    },
    {
      "type": "tool_use",
      "id": "toolu_01A09q90qw90lq917835lq9",
      "name": "read_file",
      "input": {
        "path": "src/main.rs"
      }
    }
  ],
  "stop_reason": "tool_use",
  "stop_sequence": null,
  "usage": {
    "input_tokens": 1523,
    "output_tokens": 87,
    "cache_creation_input_tokens": 0,
    "cache_read_input_tokens": 0
  }
}
```

**Response fields:**

| Field | Purpose |
|---|---|
| `id` | Unique message identifier. Useful for logging and debugging |
| `type` | Always `"message"` for Messages API responses |
| `role` | Always `"assistant"` |
| `model` | The model that generated the response |
| `content` | Array of content blocks (text and/or tool_use) |
| `stop_reason` | Why the model stopped: `end_turn`, `tool_use`, `max_tokens`, or `stop_sequence` |
| `usage` | Token counts for the request |

## Content Block Types

The `content` array contains typed blocks. Here are all the types you will encounter:

### Text Block

```json
{
  "type": "text",
  "text": "Here's what I found in the file..."
}
```

Plain text content. This is what the user sees as the model's response.

### Tool Use Block

```json
{
  "type": "tool_use",
  "id": "toolu_01A09q90qw90lq917835lq9",
  "name": "read_file",
  "input": {
    "path": "src/main.rs"
  }
}
```

A tool call request. The `id` is generated by the API and must be referenced when returning the result. The `input` object conforms to the tool's `input_schema`.

### Tool Result Block (in user messages)

```json
{
  "type": "tool_result",
  "tool_use_id": "toolu_01A09q90qw90lq917835lq9",
  "content": "fn main() {\n    let x: i32 = \"hello\";\n    println!(\"{}\", x);\n}"
}
```

Or with an error:

```json
{
  "type": "tool_result",
  "tool_use_id": "toolu_01A09q90qw90lq917835lq9",
  "is_error": true,
  "content": "Error: No such file or directory: src/mian.rs"
}
```

The `tool_result` block also supports rich content -- you can pass an array of content blocks instead of a plain string:

```json
{
  "type": "tool_result",
  "tool_use_id": "toolu_01A09q90qw90lq917835lq9",
  "content": [
    {"type": "text", "text": "File contents (500 lines):"},
    {"type": "text", "text": "fn main() { ... }"}
  ]
}
```

## The Stop Reason Field

The `stop_reason` field is the most important field for your agent loop:

| Value | Meaning | Agent Action |
|---|---|---|
| `"end_turn"` | Model completed its response naturally | Display to user, wait for input |
| `"tool_use"` | Model wants to execute one or more tools | Extract tool calls, execute, return results |
| `"max_tokens"` | Response hit the `max_tokens` limit | Handle truncation -- possibly request continuation |
| `"stop_sequence"` | Model hit a custom stop sequence | Handle based on which sequence was hit |

Your agent loop dispatches on this field:

```rust
match response.stop_reason.as_str() {
    "tool_use" => {
        let tool_calls = extract_tool_calls(&response.content);
        let results = execute_tools(tool_calls).await;
        messages.push(assistant_message(response.content));
        messages.push(tool_results_message(results));
        // Continue loop - make another API call
    }
    "end_turn" => {
        display_response(&response.content);
        // Exit loop - wait for user input
    }
    "max_tokens" => {
        display_response(&response.content);
        // Optionally request continuation
    }
    _ => { /* handle unexpected stop reasons */ }
}
```

::: python Coming from Python
If you have used the `anthropic` Python SDK, you interact with response objects like `response.content[0].text` and `response.stop_reason`. In Rust, you will deserialize the JSON response into structs using `serde`. The type system ensures you handle all content block variants -- a Rust `match` on a `ContentBlock` enum forces you to handle `Text`, `ToolUse`, and any other variants, while Python's attribute access silently fails or raises at runtime.
:::

## Usage and Token Tracking

The `usage` object provides exact token counts:

```json
{
  "usage": {
    "input_tokens": 1523,
    "output_tokens": 87,
    "cache_creation_input_tokens": 0,
    "cache_read_input_tokens": 0
  }
}
```

| Field | Purpose |
|---|---|
| `input_tokens` | Tokens in the request (system + tools + messages) |
| `output_tokens` | Tokens generated by the model |
| `cache_creation_input_tokens` | Tokens cached for the first time (prompt caching) |
| `cache_read_input_tokens` | Tokens served from cache (reduced cost) |

Track these across your agent session to monitor context usage and cost. The cache fields are relevant if you use Anthropic's prompt caching feature, which can significantly reduce costs for the repetitive parts of your request (system prompt, tool definitions) that stay the same across calls.

## Error Responses

When the API returns an error, the response body has this structure:

```json
{
  "type": "error",
  "error": {
    "type": "rate_limit_error",
    "message": "You have exceeded your rate limit. Please try again in 30 seconds."
  }
}
```

Common error types:

| Error Type | HTTP Status | Typical Cause |
|---|---|---|
| `invalid_request_error` | 400 | Malformed request, missing required field |
| `authentication_error` | 401 | Invalid or missing API key |
| `permission_error` | 403 | API key lacks required permissions |
| `rate_limit_error` | 429 | Too many requests or tokens per minute |
| `api_error` | 500 | Server-side error, retry with backoff |
| `overloaded_error` | 529 | API is temporarily overloaded, retry later |

Your agent should handle 429 and 529 errors with exponential backoff retry logic, and surface 400/401/403 errors to the user immediately.

::: wild In the Wild
Claude Code implements robust error handling for all Anthropic API error types. Rate limit errors trigger automatic retry with exponential backoff. Overloaded errors are retried with longer delays. Authentication errors surface a clear message to the user about checking their API key. This kind of thorough error handling is what makes a production agent reliable across varied network conditions and API states.
:::

## Key Takeaways

- The Anthropic Messages API uses `x-api-key` authentication, a top-level `system` field (not a message), and tool definitions in a `tools` array with JSON Schema-based `input_schema`
- Responses contain typed content blocks (`text` and `tool_use`) in a `content` array, with `stop_reason` driving the agent loop dispatch
- Tool results are sent as `tool_result` blocks in user-role messages, linked to the original tool call by `tool_use_id`, with an optional `is_error` flag
- The `usage` object provides exact input/output token counts on every response -- track these for context management and cost monitoring
- Handle API errors by type: retry 429/529 with backoff, surface 400/401/403 to the user, and log 500 errors for debugging
