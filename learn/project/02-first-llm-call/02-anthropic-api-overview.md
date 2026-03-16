---
title: Anthropic API Overview
description: Explore the Anthropic Messages API endpoints, authentication scheme, and core request/response structure.
---

# Anthropic API Overview

> **What you'll learn:**
> - The structure of the Anthropic Messages API endpoint and its required headers
> - How the API versioning scheme works and which version to target
> - The roles of model selection, max_tokens, and other top-level request parameters

Now that you understand LLM APIs at a conceptual level, let's get specific. You are going to use the Anthropic Messages API throughout this book. This subchapter walks you through the exact endpoint, headers, and parameters you need. By the end, you will know exactly what bytes go over the wire when your agent talks to Claude.

## The Messages Endpoint

The Anthropic Messages API has a single endpoint for creating messages:

```
POST https://api.anthropic.com/v1/messages
```

That is the only URL you need for the entire agent. Every conversation turn -- whether it is a simple question, a multi-turn debugging session, or a tool-use request -- goes through this same endpoint. The behavior changes based on the JSON body you send, not the URL.

## Required Headers

Every request to the Anthropic API must include three headers:

| Header | Value | Purpose |
|---|---|---|
| `x-api-key` | Your API key | Authentication |
| `anthropic-version` | `2023-06-01` | API version targeting |
| `content-type` | `application/json` | Tells the server you are sending JSON |

Here is what a raw HTTP request looks like on the wire:

```http
POST /v1/messages HTTP/1.1
Host: api.anthropic.com
Content-Type: application/json
x-api-key: sk-ant-api03-xxxxx
anthropic-version: 2023-06-01

{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 1024,
  "messages": [
    { "role": "user", "content": "Hello, Claude!" }
  ]
}
```

The `x-api-key` header is how Anthropic authenticates your request. Unlike some APIs that use `Authorization: Bearer <token>`, Anthropic uses a custom header. This is important to get right in your Rust code -- if you use the wrong header name, you will get a `401 Unauthorized` error.

::: python Coming from Python
In Python's `requests` library, you set headers as a dictionary:
```python
import requests

response = requests.post(
    "https://api.anthropic.com/v1/messages",
    headers={
        "x-api-key": "sk-ant-api03-xxxxx",
        "anthropic-version": "2023-06-01",
        "content-type": "application/json",
    },
    json={"model": "claude-sonnet-4-20250514", "max_tokens": 1024, "messages": [...]},
)
```
The Rust `reqwest` crate has an almost identical API. The main difference is that `reqwest` uses typed `HeaderName` and `HeaderValue` types instead of plain strings, which catches typos at compile time.
:::

## API Versioning

The `anthropic-version` header pins your integration to a specific version of the API behavior. The current stable version is `2023-06-01`. This version string does not change frequently, and when it does, the old version continues to work for a deprecation period.

Why does this matter? API providers sometimes change response formats, add required fields, or alter default behaviors. By pinning a version, you ensure your Rust code continues to parse responses correctly even if Anthropic updates the API. You only upgrade when you are ready to handle the changes.

For this book, use `2023-06-01` everywhere. Hard-code it as a constant in your Rust code:

```rust
const API_VERSION: &str = "2023-06-01";
```

## Model Selection

The `model` field in your request body determines which Claude model processes your prompt. As of this writing, the primary models are:

| Model ID | Best For |
|---|---|
| `claude-sonnet-4-20250514` | Balanced performance and speed for most coding tasks |
| `claude-opus-4-20250514` | Highest capability for complex reasoning |
| `claude-haiku-3-5-20241022` | Fast, cost-effective for simple tasks |

For your coding agent, `claude-sonnet-4-20250514` is the best default. It has strong coding ability, handles tool use well, and responds faster than Opus. You will make this configurable later, but start with Sonnet.

```rust
const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";
```

## Required Request Body Fields

The Messages API requires exactly three fields in every request body:

### `model` (string, required)
The model identifier. The API rejects requests with an unrecognized model.

### `max_tokens` (integer, required)
The maximum number of tokens the model is allowed to generate in its response. This is a hard cap -- the model stops generating once it hits this limit, even if it is mid-sentence. Set this high enough to get useful responses but not so high that runaway generation burns through your budget.

A reasonable default for a coding agent is `4096`. Complex code generation sometimes needs more, so you might increase this to `8192` or higher depending on the task.

### `messages` (array, required)
The conversation history. Each element is an object with `role` (`"user"` or `"assistant"`) and `content`. The messages must alternate between `user` and `assistant` roles, and the first message must be from the `user`.

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 4096,
  "messages": [
    { "role": "user", "content": "Write a Rust function to reverse a string." }
  ]
}
```

## Optional Request Body Fields

Beyond the three required fields, the API accepts several optional parameters that give you fine-grained control:

### `system` (string, optional)
A system prompt that sets the model's behavior and persona. This is separate from the `messages` array and is covered in detail in [System Prompts](/project/02-first-llm-call/08-system-prompts).

### `temperature` (float, optional, 0.0 to 1.0)
Controls randomness in the response. Lower values make output more deterministic; higher values make it more creative. Default is `1.0`. For a coding agent, you typically want lower values like `0.3` to `0.7` for more predictable code generation.

### `stop_sequences` (array of strings, optional)
Custom strings that cause the model to stop generating. Useful for structured output formats.

### `top_p` (float, optional)
Nucleus sampling parameter. An alternative to temperature for controlling randomness. Usually you set one or the other, not both.

## The Response Structure

A successful response (HTTP 200) looks like this:

```json
{
  "id": "msg_01XFDUDYJgAACzvnptvVoYEL",
  "type": "message",
  "role": "assistant",
  "model": "claude-sonnet-4-20250514",
  "content": [
    {
      "type": "text",
      "text": "Here's a Rust function to reverse a string:\n\n```rust\nfn reverse_string(s: &str) -> String {\n    s.chars().rev().collect()\n}\n```"
    }
  ],
  "stop_reason": "end_turn",
  "stop_sequence": null,
  "usage": {
    "input_tokens": 18,
    "output_tokens": 42
  }
}
```

Let's break down the key response fields:

- **`id`** -- A unique identifier for this message. Useful for logging and debugging.
- **`type`** -- Always `"message"` for non-streaming responses.
- **`role`** -- Always `"assistant"` in responses.
- **`content`** -- An array of content blocks. Each block has a `type` field. For text responses, the type is `"text"` and the generated text is in the `text` field. For tool use, the type is `"tool_use"` (covered in a later chapter).
- **`stop_reason`** -- Why the model stopped generating. Common values are `"end_turn"` (natural stop), `"max_tokens"` (hit the limit), and `"stop_sequence"` (hit a custom stop sequence).
- **`usage`** -- Token counts for billing and monitoring.

## Putting It Together: The Full Picture

Here is the complete anatomy of a Messages API call:

```
Your Code                         Anthropic API
   |                                    |
   |  POST /v1/messages                 |
   |  x-api-key: sk-ant-...            |
   |  anthropic-version: 2023-06-01     |
   |  content-type: application/json    |
   |  { model, max_tokens, messages }   |
   |----------------------------------->|
   |                                    |  (model generates tokens)
   |  HTTP 200                          |
   |  { id, content, usage, ... }       |
   |<-----------------------------------|
   |                                    |
```

In the next few subchapters, you will implement each part of this flow in Rust: loading the API key, constructing the HTTP client, building the request body, sending it, and parsing the response.

::: wild In the Wild
Claude Code sends requests to this same Messages API endpoint. It sets a generous `max_tokens` value (often 8192 or higher) because code generation tasks can produce long outputs. It also always includes a system prompt that defines Claude's role as a coding assistant with tool-use capabilities. You will build toward the same architecture, one piece at a time.
:::

## Key Takeaways

- The Anthropic Messages API lives at a single endpoint: `POST https://api.anthropic.com/v1/messages`.
- Every request requires three headers (`x-api-key`, `anthropic-version`, `content-type`) and three body fields (`model`, `max_tokens`, `messages`).
- The response contains an array of content blocks, a stop reason, and token usage counts -- all of which you will parse into Rust structs.
- Pin the `anthropic-version` header to `2023-06-01` so your parsing code does not break if the API evolves.
- Start with `claude-sonnet-4-20250514` as your default model -- it balances capability and speed for coding tasks.
