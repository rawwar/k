# OpenAI Chat Completions API — Deep Dive

## Overview

The OpenAI Chat Completions API (`POST /v1/chat/completions`) is the most widely used LLM API in the world. It established the modern paradigm of structured conversation with message roles, and its format has been adopted by virtually every other LLM provider and inference platform. For coding agents, this is the foundational API to understand deeply.

This document covers every aspect of the Chat Completions API: the full request and response schemas, streaming protocol, tool calling, structured outputs, and practical patterns for building coding agents.

---

## History and Evolution

### The Original Completions API (2020-2023)

OpenAI's first public API was the **Completions** endpoint (`POST /v1/completions`), launched with GPT-3 in June 2020. It was a simple text-in, text-out interface:

```json
{
  "model": "text-davinci-003",
  "prompt": "Write a Python function that reverses a string:\n\ndef reverse_string(s):",
  "max_tokens": 100,
  "temperature": 0.7
}
```

The model would continue the text from where the prompt left off. This was powerful but had significant limitations:
- No structured way to separate instructions from user input
- No conversation history management
- Developers had to manually format multi-turn conversations with delimiters
- Prone to prompt injection because there was no role-based trust hierarchy

### The Chat Completions Revolution (March 2023)

With the release of GPT-3.5-turbo in March 2023, OpenAI introduced the Chat Completions API. The key innovation was the **messages array** with distinct roles:

```json
{
  "model": "gpt-3.5-turbo",
  "messages": [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user", "content": "Hello!"}
  ]
}
```

This seemingly simple change had profound implications:
- **System prompts** provided a dedicated channel for developer instructions
- **Multi-turn conversations** were first-class citizens
- **Role separation** enabled better safety and instruction following
- **Standardization** created a universal format that the entire industry adopted

### Key Milestones

| Date | Event |
|------|-------|
| March 2023 | Chat Completions API launched with GPT-3.5-turbo |
| June 2023 | Function calling added (first tool-calling support) |
| November 2023 | GPT-4 Turbo with 128K context, JSON mode, parallel function calls |
| January 2024 | Function calling renamed to "tool calling" with updated schema |
| May 2024 | GPT-4o released — faster, cheaper, multimodal |
| August 2024 | Structured Outputs with strict JSON schema enforcement |
| September 2024 | o1-preview released — first reasoning model |
| January 2025 | o3-mini released with `reasoning_effort` parameter |
| March 2025 | Responses API introduced as the successor to Chat Completions |
| April 2025 | GPT-4.1 family, o3, o4-mini released |

### Current Status

As of mid-2025, the Chat Completions API remains fully supported and is still the most widely used endpoint. OpenAI has introduced the Responses API as the "next generation" but has stated that Chat Completions will continue to receive new model support. Most coding agents still target Chat Completions due to its universal compatibility.

---

## Endpoint

```
POST https://api.openai.com/v1/chat/completions
```

### Headers

```http
Authorization: Bearer sk-...
Content-Type: application/json
OpenAI-Organization: org-...       # Optional: for multi-org accounts
OpenAI-Project: proj_...           # Optional: for project-scoped billing
```

### Compatible Endpoints

Because the Chat Completions format is the de facto standard, these endpoints accept the same schema:

| Provider | Base URL |
|----------|----------|
| Azure OpenAI | `https://{resource}.openai.azure.com/openai/deployments/{deployment}/chat/completions?api-version=2024-10-21` |
| Together AI | `https://api.together.xyz/v1/chat/completions` |
| Fireworks | `https://api.fireworks.ai/inference/v1/chat/completions` |
| Groq | `https://api.groq.com/openai/v1/chat/completions` |
| DeepSeek | `https://api.deepseek.com/v1/chat/completions` |
| Ollama | `http://localhost:11434/v1/chat/completions` |
| vLLM | `http://localhost:8000/v1/chat/completions` |

---

## Full Request Schema

### Minimal Request

```json
{
  "model": "gpt-4o",
  "messages": [
    {"role": "user", "content": "Hello!"}
  ]
}
```

### Complete Request (All Parameters)

```json
{
  "model": "gpt-4o",
  "messages": [...],
  "temperature": 0.7,
  "top_p": 1.0,
  "frequency_penalty": 0.0,
  "presence_penalty": 0.0,
  "max_completion_tokens": 4096,
  "stop": ["\n\n"],
  "stream": true,
  "stream_options": {"include_usage": true},
  "tools": [...],
  "tool_choice": "auto",
  "parallel_tool_calls": true,
  "response_format": {"type": "json_schema", "json_schema": {...}},
  "seed": 42,
  "logprobs": true,
  "top_logprobs": 5,
  "n": 1,
  "user": "user-123",
  "metadata": {"request_id": "req-abc"},
  "store": true,
  "reasoning_effort": "medium",
  "service_tier": "auto"
}
```

### Parameter Reference

#### `model` (string, required)

The model ID to use for the completion. Available models include:

| Model | Context Window | Max Output | Best For |
|-------|---------------|------------|----------|
| `gpt-4o` | 128K | 16,384 | General-purpose, best quality/cost balance |
| `gpt-4o-mini` | 128K | 16,384 | Fast, cheap tasks — classification, extraction |
| `gpt-4-turbo` | 128K | 4,096 | Legacy, being superseded by GPT-4o |
| `gpt-4.1` | 1M | 32,768 | Largest context window in GPT family |
| `gpt-4.1-mini` | 1M | 32,768 | Budget-friendly large-context model |
| `gpt-4.1-nano` | 1M | 32,768 | Fastest/cheapest in 4.1 family |
| `o3` | 200K | 100,000 | Complex reasoning, math, code generation |
| `o4-mini` | 200K | 100,000 | Fast reasoning with good quality |
| `o3-mini` | 200K | 65,536 | Budget reasoning model |
| `gpt-3.5-turbo` | 16K | 4,096 | Legacy, not recommended for new projects |

**For coding agents:** `gpt-4o` is the workhorse for most coding tasks. `o3` and `o4-mini` excel at complex algorithmic problems and multi-step reasoning. `gpt-4o-mini` is ideal for simple sub-tasks like classification or reformatting.

#### `messages` (array, required)

The conversation history as an array of message objects. This is the core of the API and is covered in detail in the [Messages Deep-Dive](#messages-array-deep-dive) section below.

#### `temperature` (number, optional, default: 1)

Controls randomness in the output. Range: `0` to `2`.

| Value | Behavior | Use Case |
|-------|----------|----------|
| 0 | Deterministic (almost) — always picks the highest-probability token | Code generation, factual tasks, testing |
| 0.2-0.4 | Slightly creative but mostly consistent | General coding assistance |
| 0.7-1.0 | Balanced creativity | Creative writing, brainstorming |
| 1.5-2.0 | Very random, often incoherent | Rarely useful |

**For coding agents:** Use `temperature: 0` for deterministic code generation. Some agents use `temperature: 0.2` to add slight variation when retrying failed attempts.

**Note:** When using `o3` or `o4-mini` (reasoning models), the temperature is fixed at 1 and this parameter is ignored.

#### `top_p` (number, optional, default: 1)

Nucleus sampling — only considers tokens within the top `p` cumulative probability mass. An alternative to temperature for controlling randomness.

- `top_p: 0.1` → Only considers the top 10% most likely tokens
- `top_p: 0.9` → Considers the top 90% most likely tokens
- `top_p: 1.0` → Considers all tokens (default)

**Best practice:** Modify either `temperature` OR `top_p`, not both. For coding, `temperature: 0` with `top_p: 1` is the standard combination.

#### `frequency_penalty` (number, optional, default: 0)

Penalizes tokens based on how frequently they appear in the text so far. Range: `-2.0` to `2.0`.

- Positive values reduce repetition by penalizing tokens that have already appeared
- The penalty scales with the number of times the token has appeared
- Useful for preventing loops where the model repeats the same code pattern

#### `presence_penalty` (number, optional, default: 0)

Penalizes tokens based on whether they have appeared at all in the text so far. Range: `-2.0` to `2.0`.

- Unlike `frequency_penalty`, this is a flat penalty regardless of count
- Positive values encourage the model to introduce new topics/tokens
- Useful for encouraging diverse code approaches

**For coding agents:** Both penalties are typically left at 0. Repetition is rarely a problem with modern models, and these penalties can hurt code quality by discouraging valid repeated patterns (e.g., repeated function calls, similar variable names).

#### `max_completion_tokens` (integer, optional)

The maximum number of tokens the model can generate. This is a hard limit — the model will stop generating at this point.

**Important notes:**
- This parameter replaced the older `max_tokens` parameter (which is still accepted for backward compatibility)
- For reasoning models (o3, o4-mini), this includes both reasoning tokens and visible output tokens
- If not specified, the model uses its default maximum output length
- Setting this too low can cause truncated code — always set it high enough for the expected output

**For coding agents:** Set this to at least 4096 for code generation tasks. For multi-file edits, consider 8192-16384. For reasoning models, set it higher (e.g., 32000) to allow space for internal reasoning.

#### `stop` (string or array of strings, optional)

Up to 4 sequences that cause the model to stop generating. When the model outputs any of these sequences, generation halts (the stop sequence itself is not included in the output).

```json
"stop": ["\n```\n", "---END---"]
```

**For coding agents:** Useful for constraining output format. For example, if you want the model to generate only a single code block, you could use `stop: ["\n```\n"]` after the opening fence.

#### `stream` (boolean, optional, default: false)

When `true`, the response is streamed as server-sent events (SSE) instead of being returned as a single JSON object. See the [Streaming](#streaming-with-sse) section for full details.

#### `stream_options` (object, optional)

Only valid when `stream: true`. Currently supports one option:

```json
"stream_options": {"include_usage": true}
```

When `include_usage` is true, the final streamed chunk includes `usage` data (token counts). Without this, streamed responses don't include usage information.

#### `tools` (array, optional)

Defines the tools (functions) the model can call. Each tool has a type, name, description, and JSON schema for parameters. See the [Tool Calling](#functiontool-calling-flow) section for full details.

```json
"tools": [
  {
    "type": "function",
    "function": {
      "name": "read_file",
      "description": "Read the contents of a file at the given path",
      "parameters": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "The absolute path to the file"
          }
        },
        "required": ["path"],
        "additionalProperties": false
      },
      "strict": true
    }
  }
]
```

#### `tool_choice` (string or object, optional)

Controls how the model uses tools:

| Value | Behavior |
|-------|----------|
| `"auto"` | Model decides whether to call tools (default when tools are present) |
| `"none"` | Model will not call any tools |
| `"required"` | Model must call at least one tool |
| `{"type": "function", "function": {"name": "specific_fn"}}` | Model must call the specified function |

#### `parallel_tool_calls` (boolean, optional, default: true)

When `true`, the model can return multiple tool calls in a single response. When `false`, it returns at most one. Parallel tool calls are important for coding agents that can execute independent operations concurrently (e.g., reading multiple files).

#### `response_format` (object, optional)

Controls the format of the model's output:

```json
// Plain text (default)
{"type": "text"}

// JSON mode — model outputs valid JSON but no schema enforcement
{"type": "json_object"}

// Structured Outputs — strict JSON schema conformance
{
  "type": "json_schema",
  "json_schema": {
    "name": "code_edit",
    "strict": true,
    "schema": {
      "type": "object",
      "properties": {
        "file_path": {"type": "string"},
        "changes": {
          "type": "array",
          "items": {
            "type": "object",
            "properties": {
              "old_text": {"type": "string"},
              "new_text": {"type": "string"}
            },
            "required": ["old_text", "new_text"],
            "additionalProperties": false
          }
        }
      },
      "required": ["file_path", "changes"],
      "additionalProperties": false
    }
  }
}
```

#### `seed` (integer, optional)

Enables (best-effort) deterministic output. When a `seed` is provided along with `temperature: 0`, the model attempts to return the same output for the same input. The response includes a `system_fingerprint` that can be used to verify consistency.

**Note:** Determinism is not guaranteed — it's "best effort." Infrastructure changes can cause variation even with the same seed.

#### `logprobs` (boolean, optional, default: false)

When `true`, returns the log probabilities of each output token. Must be combined with `top_logprobs` to specify how many alternatives to return per position.

#### `top_logprobs` (integer, optional)

Number of most likely tokens to return at each position, along with their log probabilities. Range: 0 to 20. Requires `logprobs: true`.

**For coding agents:** Log probabilities can be used to detect model uncertainty. If the top token has a low probability, the model is uncertain about its output — useful for flagging potentially incorrect code suggestions.

#### `n` (integer, optional, default: 1)

Number of completions to generate. Returns `n` choices in the response. Each choice is an independent generation.

**For coding agents:** Some agents use `n > 1` to generate multiple candidate solutions and then select the best one (e.g., by running tests on each candidate). This is more expensive but can improve quality.

#### `user` (string, optional)

A unique identifier for the end user. Used for abuse monitoring and rate limiting. Does not affect model behavior.

#### `reasoning_effort` (string, optional)

Only for reasoning models (`o3`, `o4-mini`, `o3-mini`). Controls how much "thinking" the model does:

| Value | Behavior |
|-------|----------|
| `"low"` | Minimal reasoning, fastest responses |
| `"medium"` | Balanced reasoning |
| `"high"` | Maximum reasoning effort, best quality |

**For coding agents:** Use `"high"` for complex algorithmic problems and `"low"` for simple classification or extraction tasks.

#### `service_tier` (string, optional)

Controls the processing priority:

| Value | Behavior |
|-------|----------|
| `"auto"` | May use scale tier if available, falls back to default |
| `"default"` | Standard processing |
| `"flex"` | Lower priority but cheaper (for non-latency-sensitive workloads) |

---

## Full Response Schema

### Non-Streaming Response

```json
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  "created": 1719877200,
  "model": "gpt-4o-2024-08-06",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Here's a Python function that reverses a string:\n\n```python\ndef reverse_string(s: str) -> str:\n    return s[::-1]\n```",
        "refusal": null
      },
      "finish_reason": "stop",
      "logprobs": null
    }
  ],
  "usage": {
    "prompt_tokens": 25,
    "completion_tokens": 42,
    "total_tokens": 67,
    "prompt_tokens_details": {
      "cached_tokens": 0,
      "audio_tokens": 0
    },
    "completion_tokens_details": {
      "reasoning_tokens": 0,
      "audio_tokens": 0,
      "accepted_prediction_tokens": 0,
      "rejected_prediction_tokens": 0
    }
  },
  "system_fingerprint": "fp_abc123"
}
```

### Response Fields

#### `id` (string)
A unique identifier for the completion. Format: `chatcmpl-{random}`. Useful for logging and debugging.

#### `object` (string)
Always `"chat.completion"` for non-streaming responses.

#### `created` (integer)
Unix timestamp of when the completion was created.

#### `model` (string)
The actual model used. This may differ slightly from the requested model — for example, requesting `gpt-4o` may return `gpt-4o-2024-08-06` (the specific snapshot).

#### `choices` (array)
An array of completion choices. When `n=1` (default), this contains exactly one element.

Each choice contains:

- **`index`** (integer): The index of this choice in the array (0-based).
- **`message`** (object): The assistant's message:
  - `role`: Always `"assistant"`
  - `content`: The text response (can be `null` if the model only made tool calls)
  - `tool_calls`: Array of tool call objects (present when the model invokes tools)
  - `refusal`: A refusal message if the model declined the request (safety filters)
- **`finish_reason`** (string): Why the model stopped generating:

| Finish Reason | Meaning |
|--------------|---------|
| `"stop"` | Natural stop or hit a stop sequence |
| `"length"` | Hit `max_completion_tokens` limit — output was truncated |
| `"tool_calls"` | Model is making tool call(s) and waiting for results |
| `"content_filter"` | Content was filtered by safety systems |

#### `usage` (object)
Token usage for billing and monitoring:

- `prompt_tokens`: Number of tokens in the input
- `completion_tokens`: Number of tokens generated
- `total_tokens`: Sum of prompt and completion tokens
- `prompt_tokens_details.cached_tokens`: Tokens served from cache (cheaper)
- `completion_tokens_details.reasoning_tokens`: Internal reasoning tokens (o-series models)

#### `system_fingerprint` (string)
An identifier for the model configuration. Useful with `seed` for determinism checks.

---

## Messages Array Deep-Dive

The `messages` array is the heart of the Chat Completions API. It represents the full conversation history that the model uses to generate its response.

### System Messages

System messages provide instructions that guide the model's behavior throughout the conversation:

```json
{
  "role": "system",
  "content": "You are an expert Python developer. When writing code:\n- Use type hints\n- Follow PEP 8\n- Include docstrings\n- Handle errors gracefully"
}
```

**Key characteristics:**
- Typically the first message in the array
- Sets the persona, rules, and constraints
- The model treats system messages with higher priority than user messages
- Multiple system messages are allowed but unusual
- For coding agents, the system prompt often includes: tool descriptions, output format instructions, coding style guidelines, and safety rules

**For coding agents:** The system prompt is where you define the agent's capabilities, available tools, and behavioral constraints. It's common for coding agent system prompts to be 2,000-5,000+ tokens.

### User Messages

User messages represent input from the human (or from the agent framework acting on behalf of the human):

```json
// Simple text message
{
  "role": "user",
  "content": "Write a function that validates email addresses"
}

// Multimodal message with text and image
{
  "role": "user",
  "content": [
    {"type": "text", "text": "What does this error screenshot show?"},
    {
      "type": "image_url",
      "image_url": {
        "url": "data:image/png;base64,iVBOR...",
        "detail": "high"
      }
    }
  ]
}
```

**Content types:**
- **Text**: Plain string or `{"type": "text", "text": "..."}` in array form
- **Image**: `{"type": "image_url", "image_url": {"url": "...", "detail": "auto|low|high"}}` — supports URLs and base64 data URIs
- The `detail` parameter controls image tokenization: `low` uses ~85 tokens, `high` uses up to ~1,105 tokens per 512x512 tile

**For coding agents:** User messages often contain code context (file contents, error messages, test output) prepended by the agent framework, not just the raw user input.

### Assistant Messages

Assistant messages represent the model's prior responses. They're included in the messages array to provide conversation history:

```json
// Text response
{
  "role": "assistant",
  "content": "Here's the implementation:\n\n```python\nimport re\n\ndef validate_email(email: str) -> bool:\n    pattern = r'^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$'\n    return bool(re.match(pattern, email))\n```"
}

// Tool call response
{
  "role": "assistant",
  "content": null,
  "tool_calls": [
    {
      "id": "call_abc123",
      "type": "function",
      "function": {
        "name": "read_file",
        "arguments": "{\"path\": \"/src/utils.py\"}"
      }
    }
  ]
}
```

**Key characteristics:**
- Can have `content` (text), `tool_calls` (tool invocations), or both
- When making tool calls, `content` is often `null`
- The `tool_calls[].function.arguments` field is a JSON string (not a parsed object)
- Each tool call has a unique `id` that must be referenced in the corresponding tool response

### Tool Messages

Tool messages provide the results of function calls back to the model:

```json
{
  "role": "tool",
  "tool_call_id": "call_abc123",
  "content": "def validate_email(email: str) -> bool:\n    \"\"\"Validate an email address.\"\"\"\n    import re\n    pattern = r'^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$'\n    return bool(re.match(pattern, email))"
}
```

**Key requirements:**
- `tool_call_id` must match the `id` from the corresponding tool call in the assistant message
- `content` must be a string (not a JSON object) — serialize objects with `JSON.stringify()` or `json.dumps()`
- One tool message per tool call — if the assistant made 3 parallel tool calls, provide 3 tool messages
- Tool messages must appear in the messages array immediately after the assistant message that made the tool calls

### Message Ordering Rules

The messages array must follow these ordering rules:
1. System messages can appear at the beginning (recommended) or interspersed
2. After a system message, messages alternate between user and assistant (with tool messages interspersed as needed)
3. Tool messages must immediately follow the assistant message containing the tool calls they respond to
4. The last message should be a `user` or `tool` message (to prompt the model to generate a response)

Example of a complete tool-calling conversation:

```json
[
  {"role": "system", "content": "You are a coding assistant with file access."},
  {"role": "user", "content": "What's in the main.py file?"},
  {"role": "assistant", "content": null, "tool_calls": [
    {"id": "call_1", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\": \"main.py\"}"}}
  ]},
  {"role": "tool", "tool_call_id": "call_1", "content": "print('hello world')"},
  {"role": "assistant", "content": "The main.py file contains a simple print statement: `print('hello world')`"},
  {"role": "user", "content": "Add error handling to it"},
  {"role": "assistant", "content": null, "tool_calls": [
    {"id": "call_2", "type": "function", "function": {"name": "write_file", "arguments": "{\"path\": \"main.py\", \"content\": \"try:\\n    print('hello world')\\nexcept Exception as e:\\n    print(f'Error: {e}')\"}"}}
  ]},
  {"role": "tool", "tool_call_id": "call_2", "content": "File written successfully"},
  {"role": "assistant", "content": "I've updated main.py with try/except error handling."}
]
```

---

## Streaming with SSE

### Enabling Streaming

Set `stream: true` in the request body. The response will use `Content-Type: text/event-stream` and deliver chunks as server-sent events.

### Event Format

Each event is a line prefixed with `data: ` followed by a JSON object:

```
data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1719877200,"model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1719877200,"model":"gpt-4o","choices":[{"index":0,"delta":{"content":"Here"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1719877200,"model":"gpt-4o","choices":[{"index":0,"delta":{"content":"'s"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1719877200,"model":"gpt-4o","choices":[{"index":0,"delta":{"content":" a"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1719877200,"model":"gpt-4o","choices":[{"index":0,"delta":{}},"finish_reason":"stop"}]}

data: [DONE]
```

### Key Differences from Non-Streaming

| Aspect | Non-Streaming | Streaming |
|--------|--------------|-----------|
| `object` field | `"chat.completion"` | `"chat.completion.chunk"` |
| Message field | `message` | `delta` |
| Content delivery | Complete text | Incremental tokens |
| Finish reason | On the single response | On the final chunk |
| Usage data | Always included | Only with `stream_options.include_usage` |
| Token counting | In response | In final chunk (if enabled) |

### Delta Objects

The `delta` object in streaming chunks contains only the new content for that chunk:

- **First chunk**: `{"role": "assistant"}` — establishes the role
- **Content chunks**: `{"content": "token"}` — each token as it's generated
- **Tool call chunks**: `{"tool_calls": [{"index": 0, "function": {"arguments": "partial"}}]}` — incremental tool call data
- **Final chunk**: `{}` with `finish_reason` set

### Streaming Tool Calls

Tool calls are streamed incrementally. The function name and arguments arrive in pieces:

```
data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_abc","type":"function","function":{"name":"read_file","arguments":""}}]}}]}
data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"pa"}}]}}]}
data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"th\":"}}]}}]}
data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":" \"src"}}]}}]}
data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"/main"}}]}}]}
data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":".py\"}"}}]}}]}
data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}]}
data: [DONE]
```

The client must concatenate the `arguments` strings across chunks and parse the complete JSON when streaming finishes.

### The `[DONE]` Sentinel

The stream ends with `data: [DONE]` (literally the string `[DONE]`, not a JSON object). This signals that the stream is complete and the client should close the connection.

---

## Function/Tool Calling Flow

Tool calling is the mechanism that transforms a language model into an agent. The model can request the execution of external functions, receive results, and incorporate them into its response.

### Defining Tools

Tools are defined in the `tools` array of the request:

```json
{
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "search_codebase",
        "description": "Search for code patterns across the repository using regex. Returns matching file paths and line numbers.",
        "parameters": {
          "type": "object",
          "properties": {
            "pattern": {
              "type": "string",
              "description": "Regular expression pattern to search for"
            },
            "file_glob": {
              "type": "string",
              "description": "Optional glob pattern to filter files (e.g., '*.py', 'src/**/*.ts')"
            },
            "max_results": {
              "type": "integer",
              "description": "Maximum number of results to return",
              "default": 20
            }
          },
          "required": ["pattern"],
          "additionalProperties": false
        },
        "strict": true
      }
    },
    {
      "type": "function",
      "function": {
        "name": "edit_file",
        "description": "Make a precise edit to a file by replacing an exact string match with new content.",
        "parameters": {
          "type": "object",
          "properties": {
            "path": {
              "type": "string",
              "description": "Absolute path to the file to edit"
            },
            "old_string": {
              "type": "string",
              "description": "The exact string to find and replace (must match exactly)"
            },
            "new_string": {
              "type": "string",
              "description": "The string to replace it with"
            }
          },
          "required": ["path", "old_string", "new_string"],
          "additionalProperties": false
        },
        "strict": true
      }
    }
  ]
}
```

**Best practices for tool definitions:**
- Write clear, specific descriptions — the model uses these to decide when and how to call tools
- Use `strict: true` to enable Structured Outputs for tool parameters (guarantees valid JSON)
- Set `additionalProperties: false` on all object schemas when using strict mode
- Include descriptions for each parameter
- Use `required` to specify mandatory parameters

### Tool Choice Options

The `tool_choice` parameter controls how the model uses tools:

```json
// Model decides (default when tools are present)
"tool_choice": "auto"

// Model must not use any tools
"tool_choice": "none"

// Model must call at least one tool
"tool_choice": "required"

// Model must call this specific function
"tool_choice": {"type": "function", "function": {"name": "edit_file"}}
```

**For coding agents:**
- Use `"auto"` for general agent loops where the model decides what action to take
- Use `"required"` when you know the model should act (e.g., after providing file contents and asking for an edit)
- Use a specific function when implementing a targeted workflow step

### Parallel Tool Calls

By default (`parallel_tool_calls: true`), the model can return multiple tool calls in a single response:

```json
{
  "role": "assistant",
  "content": null,
  "tool_calls": [
    {
      "id": "call_1",
      "type": "function",
      "function": {"name": "read_file", "arguments": "{\"path\": \"src/auth.py\"}"}
    },
    {
      "id": "call_2",
      "type": "function",
      "function": {"name": "read_file", "arguments": "{\"path\": \"src/models.py\"}"}
    },
    {
      "id": "call_3",
      "type": "function",
      "function": {"name": "read_file", "arguments": "{\"path\": \"tests/test_auth.py\"}"}
    }
  ]
}
```

The agent should execute all three calls (potentially in parallel) and return all results:

```json
[
  {"role": "tool", "tool_call_id": "call_1", "content": "... contents of auth.py ..."},
  {"role": "tool", "tool_call_id": "call_2", "content": "... contents of models.py ..."},
  {"role": "tool", "tool_call_id": "call_3", "content": "... contents of test_auth.py ..."}
]
```

### The Agent Loop

The standard coding agent loop using tool calling:

```
1. Send messages + tools to Chat Completions API
2. Receive response
3. If finish_reason == "tool_calls":
   a. Parse tool calls from response
   b. Execute each tool call
   c. Append assistant message (with tool_calls) to messages
   d. Append tool result messages to messages
   e. Go to step 1
4. If finish_reason == "stop":
   a. Extract final text response
   b. Present to user
   c. Done (or wait for next user input)
```

---

## Structured Outputs

Structured Outputs guarantee that the model's response conforms to a specified JSON schema. This is critical for coding agents that need to parse model output programmatically.

### JSON Mode (Basic)

```json
"response_format": {"type": "json_object"}
```

The model will output valid JSON, but there's no schema enforcement. You must instruct the model about the desired structure in the prompt.

### JSON Schema Mode (Strict)

```json
"response_format": {
  "type": "json_schema",
  "json_schema": {
    "name": "code_changes",
    "strict": true,
    "schema": {
      "type": "object",
      "properties": {
        "thinking": {
          "type": "string",
          "description": "Step-by-step reasoning about the changes needed"
        },
        "changes": {
          "type": "array",
          "items": {
            "type": "object",
            "properties": {
              "file_path": {"type": "string"},
              "action": {"type": "string", "enum": ["create", "edit", "delete"]},
              "content": {"type": ["string", "null"]}
            },
            "required": ["file_path", "action", "content"],
            "additionalProperties": false
          }
        }
      },
      "required": ["thinking", "changes"],
      "additionalProperties": false
    }
  }
}
```

**Schema restrictions in strict mode:**
- All object properties must be listed in `required`
- `additionalProperties` must be `false` on all objects
- Only a subset of JSON Schema is supported (no `$ref`, no `oneOf` with complex types, etc.)
- Recursive schemas are supported (for tree-like structures)

**For coding agents:** Structured Outputs are powerful for forcing the model to produce machine-parseable edit instructions. Instead of asking the model to output code in markdown and then parsing it, you can get structured JSON with file paths, edit locations, and new content.

---

## Code Examples

### Python (openai library)

```python
from openai import OpenAI

client = OpenAI()  # Uses OPENAI_API_KEY env var

# Basic completion
response = client.chat.completions.create(
    model="gpt-4o",
    messages=[
        {"role": "system", "content": "You are a Python expert."},
        {"role": "user", "content": "Write a binary search function."}
    ],
    temperature=0,
    max_completion_tokens=2048
)

print(response.choices[0].message.content)
print(f"Tokens: {response.usage.prompt_tokens} in, {response.usage.completion_tokens} out")

# Streaming
stream = client.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "Explain quicksort"}],
    stream=True,
    stream_options={"include_usage": True}
)

for chunk in stream:
    if chunk.choices and chunk.choices[0].delta.content:
        print(chunk.choices[0].delta.content, end="", flush=True)

# Tool calling
tools = [
    {
        "type": "function",
        "function": {
            "name": "run_tests",
            "description": "Run the test suite and return results",
            "parameters": {
                "type": "object",
                "properties": {
                    "test_path": {"type": "string", "description": "Path to test file or directory"}
                },
                "required": ["test_path"],
                "additionalProperties": False
            },
            "strict": True
        }
    }
]

response = client.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "Run the auth tests"}],
    tools=tools,
    tool_choice="auto"
)

if response.choices[0].finish_reason == "tool_calls":
    for tool_call in response.choices[0].message.tool_calls:
        print(f"Tool: {tool_call.function.name}")
        print(f"Args: {tool_call.function.arguments}")
```

### TypeScript (openai library)

```typescript
import OpenAI from 'openai';

const client = new OpenAI(); // Uses OPENAI_API_KEY env var

// Basic completion
const response = await client.chat.completions.create({
  model: 'gpt-4o',
  messages: [
    { role: 'system', content: 'You are a TypeScript expert.' },
    { role: 'user', content: 'Write a debounce function.' }
  ],
  temperature: 0,
  max_completion_tokens: 2048,
});

console.log(response.choices[0].message.content);

// Streaming
const stream = await client.chat.completions.create({
  model: 'gpt-4o',
  messages: [{ role: 'user', content: 'Explain async/await' }],
  stream: true,
});

for await (const chunk of stream) {
  const content = chunk.choices[0]?.delta?.content;
  if (content) process.stdout.write(content);
}

// Tool calling with agent loop
async function agentLoop(userMessage: string) {
  const messages: OpenAI.ChatCompletionMessageParam[] = [
    { role: 'system', content: 'You are a coding assistant.' },
    { role: 'user', content: userMessage }
  ];

  while (true) {
    const response = await client.chat.completions.create({
      model: 'gpt-4o',
      messages,
      tools: [/* tool definitions */],
    });

    const choice = response.choices[0];
    messages.push(choice.message);

    if (choice.finish_reason === 'stop') {
      return choice.message.content;
    }

    if (choice.finish_reason === 'tool_calls') {
      for (const toolCall of choice.message.tool_calls ?? []) {
        const result = await executeToolCall(toolCall);
        messages.push({
          role: 'tool',
          tool_call_id: toolCall.id,
          content: JSON.stringify(result),
        });
      }
    }
  }
}
```

### curl

```bash
# Basic request
curl https://api.openai.com/v1/chat/completions \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "Write a hello world in Rust"}
    ],
    "temperature": 0,
    "max_completion_tokens": 1024
  }'

# Streaming request
curl https://api.openai.com/v1/chat/completions \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H "Content-Type: application/json" \
  -N \
  -d '{
    "model": "gpt-4o",
    "messages": [{"role": "user", "content": "Count to 10"}],
    "stream": true
  }'

# With tools
curl https://api.openai.com/v1/chat/completions \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o",
    "messages": [{"role": "user", "content": "What files are in the src directory?"}],
    "tools": [{
      "type": "function",
      "function": {
        "name": "list_directory",
        "description": "List files in a directory",
        "parameters": {
          "type": "object",
          "properties": {
            "path": {"type": "string"}
          },
          "required": ["path"]
        }
      }
    }]
  }'
```

---

## Token Counting with tiktoken

Understanding token counts is essential for managing context windows and costs:

```python
import tiktoken

# Get the tokenizer for a specific model
enc = tiktoken.encoding_for_model("gpt-4o")

# Count tokens in a string
text = "def hello_world():\n    print('Hello, World!')"
tokens = enc.encode(text)
print(f"Token count: {len(tokens)}")  # Output: Token count: 14

# Decode tokens back to text
for token in tokens:
    print(f"  {token} -> {repr(enc.decode([token]))}")

# Estimate message token count (approximate)
def count_message_tokens(messages, model="gpt-4o"):
    """Approximate token count for a messages array."""
    enc = tiktoken.encoding_for_model(model)
    num_tokens = 0
    for message in messages:
        num_tokens += 4  # Every message has overhead: <|im_start|>{role}\n ... <|im_end|>\n
        for key, value in message.items():
            if isinstance(value, str):
                num_tokens += len(enc.encode(value))
    num_tokens += 2  # Priming tokens for assistant reply
    return num_tokens
```

**Token counting for coding agents:**
- Count tokens before sending requests to avoid context length errors
- Implement a "context window budget" — reserve tokens for the response, tools, and system prompt
- Truncate or summarize older conversation turns when approaching the limit
- Code typically tokenizes to more tokens per character than prose due to variable names, operators, and whitespace

---

## Error Handling

### Common Error Codes

| Status Code | Error Type | Cause | Resolution |
|-------------|-----------|-------|------------|
| 400 | `invalid_request_error` | Malformed request, invalid parameters | Fix the request body |
| 401 | `authentication_error` | Invalid or missing API key | Check `OPENAI_API_KEY` |
| 403 | `permission_error` | API key lacks required permissions | Check key permissions/org access |
| 404 | `not_found_error` | Invalid model or endpoint | Check model name |
| 429 | `rate_limit_error` | Too many requests or token limit | Implement backoff/retry |
| 500 | `server_error` | OpenAI internal error | Retry with exponential backoff |
| 503 | `service_unavailable` | API is overloaded | Retry with exponential backoff |

### Rate Limit Headers

```http
x-ratelimit-limit-requests: 500
x-ratelimit-limit-tokens: 30000
x-ratelimit-remaining-requests: 499
x-ratelimit-remaining-tokens: 29500
x-ratelimit-reset-requests: 200ms
x-ratelimit-reset-tokens: 6s
retry-after: 2
```

### Context Length Exceeded

When the total tokens (input + requested output) exceed the model's context window:

```json
{
  "error": {
    "message": "This model's maximum context length is 128000 tokens. However, your messages resulted in 130256 tokens. Please reduce the length of the messages.",
    "type": "invalid_request_error",
    "code": "context_length_exceeded"
  }
}
```

**Mitigation strategies for coding agents:**
1. Count tokens before sending and truncate if necessary
2. Implement a sliding window over conversation history
3. Summarize older tool call results instead of including full content
4. Use models with larger context windows (Gemini 2.5 Pro with 1M tokens)
5. Implement RAG to include only relevant code rather than entire files

### Retry Strategy

```python
import time
import random
from openai import OpenAI, RateLimitError, APIError

client = OpenAI()

def chat_with_retry(messages, max_retries=5):
    """Call Chat Completions with exponential backoff retry."""
    for attempt in range(max_retries):
        try:
            return client.chat.completions.create(
                model="gpt-4o",
                messages=messages
            )
        except RateLimitError as e:
            if attempt == max_retries - 1:
                raise
            # Use retry-after header if available, otherwise exponential backoff
            retry_after = getattr(e, 'retry_after', None)
            wait = retry_after or (2 ** attempt + random.random())
            print(f"Rate limited. Retrying in {wait:.1f}s...")
            time.sleep(wait)
        except APIError as e:
            if e.status_code >= 500 and attempt < max_retries - 1:
                wait = 2 ** attempt + random.random()
                print(f"Server error {e.status_code}. Retrying in {wait:.1f}s...")
                time.sleep(wait)
            else:
                raise
```

---

## Best Practices for Coding Agents

### 1. System Prompt Design

- Place tool descriptions and usage instructions in the system prompt
- Include examples of how to use each tool (few-shot prompting)
- Define the agent's workflow: "First read the relevant files, then plan changes, then implement"
- Set clear boundaries: what the agent should and shouldn't do

### 2. Context Management

- Track token usage and implement a budget system
- Prioritize recent messages and relevant context over full history
- Summarize tool results that exceed a threshold (e.g., truncate large file contents)
- Use prompt caching by keeping the system prompt and tool definitions stable across requests

### 3. Tool Design

- Keep tool descriptions concise but informative
- Use `strict: true` for all tool parameter schemas
- Return structured, parseable results from tools
- Include error information in tool results so the model can self-correct

### 4. Streaming

- Always use streaming for user-facing agents (reduces perceived latency)
- Parse tool calls incrementally to begin execution as soon as the full arguments are received
- Display partial content to users as it arrives

### 5. Error Recovery

- Implement retry with exponential backoff for transient errors
- On context length errors, truncate older messages and retry
- When the model generates invalid tool calls, return a helpful error message in the tool result
- Set a maximum iteration count for agent loops to prevent infinite loops

### 6. Cost Optimization

- Use cheaper models (GPT-4o-mini) for simple sub-tasks
- Cache file contents and avoid re-reading unchanged files
- Minimize system prompt length without sacrificing quality
- Use batch API for offline/non-interactive workloads

---

## Comparison with the Responses API

OpenAI introduced the Responses API (`POST /v1/responses`) in March 2025 as the successor to Chat Completions. Key differences:

| Feature | Chat Completions | Responses API |
|---------|-----------------|---------------|
| **State management** | Client-side (send full message history) | Server-side (reference previous response ID) |
| **Built-in tools** | None (custom functions only) | Web search, file search, code interpreter |
| **Message format** | `messages[]` with role/content | `input` (string or items[]) |
| **Output format** | `choices[].message` | `output[]` items with typed content |
| **Streaming events** | Untyped `data:` chunks | Typed events (`response.created`, `response.output_item.added`, etc.) |
| **Background execution** | Not supported | `background: true` for async tasks |
| **Conversation threading** | Manual | Automatic via `previous_response_id` |
| **Ecosystem compatibility** | Universal (all providers) | OpenAI only |

**When to use Chat Completions:**
- Multi-provider support needed
- Existing codebase already uses it
- Need maximum compatibility with third-party tools
- Building on open-source inference (Ollama, vLLM, etc.)

**When to use Responses API:**
- Using OpenAI's built-in tools (web search, file search)
- Need server-side state management
- Building complex multi-step agents with background processing
- Starting a new project exclusively with OpenAI

**For coding agents:** Most coding agents still use Chat Completions because (1) they need provider flexibility, (2) they have custom tools (not built-in ones), and (3) the ecosystem of compatible providers is massive. The Responses API's server-side state management is appealing for complex agents, but client-side state is more portable and debuggable.

---

## Summary

The OpenAI Chat Completions API remains the cornerstone of LLM-powered coding agents. Its message-based paradigm, tool calling capabilities, structured output support, and streaming protocol provide everything needed to build sophisticated agent systems. While newer APIs like OpenAI's Responses API and Anthropic's Messages API offer additional features, the Chat Completions format is the universal standard — supported by every major provider, inference engine, and SDK.

For coding agent developers, mastering this API means understanding:
- How to structure conversations with system prompts, user context, and tool results
- How to define and orchestrate tool calls for file operations, code search, and testing
- How to manage context windows and token budgets efficiently
- How to stream responses for responsive user experiences
- How to handle errors gracefully and retry intelligently

This knowledge forms the foundation upon which all other LLM API understanding is built.
