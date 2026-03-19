# OpenAI Responses API

## Introduction

The Responses API is OpenAI's next-generation API for building agentic applications. Released in
early 2025, it was designed to replace the Chat Completions API as the primary interface for
interacting with OpenAI models. While Chat Completions remains supported, the Responses API is
where OpenAI is investing all new features—built-in tools, stateful conversations, background
execution, and multi-turn agent loops.

### Why a New API?

The Chat Completions API was designed in 2022 for a simple request-response pattern: send messages,
get a completion. As the ecosystem evolved toward agents—systems that use tools, maintain state, and
run multi-step workflows—the Chat Completions API started showing its limitations:

1. **Tool calling was bolted on.** Function calling was added as an afterthought. The developer had
   to manage the full loop: send messages → get tool calls → execute tools → append results → send
   again. Every developer wrote the same boilerplate.

2. **No built-in tools.** If you wanted web search, code execution, or file retrieval, you had to
   build or integrate those yourself. The model couldn't natively search the web or run code.

3. **Stateless by design.** Every request required sending the full conversation history. For long
   conversations, this meant managing ever-growing message arrays and dealing with context window
   limits manually.

4. **No background execution.** Long-running agentic tasks (research, code generation, multi-step
   analysis) had to complete within a single HTTP request timeout.

The Responses API addresses all of these by treating agentic behavior as a first-class primitive
rather than an extension.

---

## Endpoint

```
POST https://api.openai.com/v1/responses
```

Authentication via Bearer token in the Authorization header:

```
Authorization: Bearer sk-...
```

---

## Key Differences from Chat Completions

### Items-Based Model vs Messages Array

Chat Completions uses a flat `messages` array where each message has a `role` and `content`:

```json
{
  "messages": [
    { "role": "system", "content": "You are helpful." },
    { "role": "user", "content": "Hello" },
    { "role": "assistant", "content": "Hi there!" }
  ]
}
```

The Responses API uses an `input` field that accepts either a simple string or an array of **items**.
Items are richer objects that can represent messages, tool calls, tool results, and more:

```json
{
  "input": [
    { "type": "message", "role": "user", "content": "Search for the latest news on AI" }
  ]
}
```

Or simply:

```json
{
  "input": "Search for the latest news on AI"
}
```

The output is also an array of items, not a single message. A single response can contain multiple
output items—for example, a tool call item followed by a message item.

### Built-in Tools

The Responses API ships with tools that the model can use natively, without any developer
infrastructure:

| Tool               | Description                                    |
| ------------------ | ---------------------------------------------- |
| `web_search`       | Real-time web search via Bing                  |
| `file_search`      | RAG over uploaded files via vector stores       |
| `code_interpreter` | Sandboxed Python execution environment          |
| `computer_use`     | Desktop automation (CUA, preview)              |
| `mcp`              | Connect to any MCP server                       |

These are declared in the `tools` array and the model decides when to invoke them. The API handles
execution—the developer never sees the intermediate tool calls for built-in tools (unless using
streaming or requesting them via `include`).

### Stateful Conversations with previous_response_id

Instead of resending the entire conversation history, you can reference a previous response:

```json
{
  "model": "gpt-4.1",
  "input": [
    { "type": "message", "role": "user", "content": "Now summarize that" }
  ],
  "previous_response_id": "resp_abc123"
}
```

OpenAI stores the conversation server-side (when `store: true`) and automatically prepends the
history. This eliminates manual context management for multi-turn conversations.

### Simplified Tool Calling Flow

With Chat Completions, custom tool calling requires a multi-step loop:

1. Send request with tools defined
2. Receive `tool_calls` in the assistant message
3. Execute each tool call yourself
4. Append tool results as messages with `role: "tool"`
5. Send the updated messages array back

With the Responses API for **built-in tools**, the API handles steps 2-4 internally. You send one
request and get back the final answer. For **custom tools** (function type), you still manage the
loop, but the ergonomics are improved with the items model.

### Background Mode

For long-running tasks, the Responses API supports background execution:

```json
{
  "model": "gpt-4.1",
  "input": "Research and write a comprehensive report on quantum computing advances in 2025",
  "background": true
}
```

This returns immediately with a response ID. You poll for completion or use webhooks. This is
essential for agentic tasks that may take minutes to complete (e.g., multi-step research with web
search and code execution).

---

## Request Schema

### Core Parameters

| Parameter                | Type                   | Required | Description                                                |
| ------------------------ | ---------------------- | -------- | ---------------------------------------------------------- |
| `model`                  | string                 | Yes      | Model ID (e.g., `gpt-4.1`, `o4-mini`, `gpt-4o`)          |
| `input`                  | string or array        | Yes      | User input as text or array of input items                 |
| `instructions`           | string or null         | No       | System-level instructions (replaces system message)        |
| `tools`                  | array                  | No       | Tools available to the model                               |
| `tool_choice`            | string or object       | No       | How the model should use tools                             |
| `temperature`            | number                 | No       | Sampling temperature (0-2, default varies by model)        |
| `top_p`                  | number                 | No       | Nucleus sampling parameter                                 |
| `max_output_tokens`      | integer                | No       | Maximum tokens in the response                             |
| `text`                   | object                 | No       | Text generation config (includes structured output)        |
| `reasoning`              | object                 | No       | Reasoning config for o-series models                       |
| `store`                  | boolean                | No       | Whether to store the response for later retrieval          |
| `previous_response_id`   | string                 | No       | ID of a previous response for multi-turn                   |
| `stream`                 | boolean                | No       | Whether to stream the response                             |
| `truncation`             | string                 | No       | Truncation strategy: `auto` or `disabled` (default)        |
| `include`                | array of strings       | No       | Additional data to include in response                     |
| `metadata`               | object                 | No       | Key-value pairs for tagging/filtering                      |
| `parallel_tool_calls`    | boolean                | No       | Whether to allow parallel tool calls (default true)        |

### input

The `input` parameter is the most flexible part of the API. It accepts:

**Simple string:**
```json
{ "input": "What is the capital of France?" }
```

**Array of items (messages, tool results, etc.):**
```json
{
  "input": [
    {
      "type": "message",
      "role": "user",
      "content": "Analyze this image"
    },
    {
      "type": "message",
      "role": "user",
      "content": [
        { "type": "input_text", "text": "What do you see?" },
        { "type": "input_image", "image_url": "https://example.com/photo.jpg" }
      ]
    }
  ]
}
```

Content within message items can also be an array of content parts for multimodal input
(text + images, text + files, etc.).

### instructions

Replaces the `system` message role from Chat Completions. This is a top-level string that sets the
model's behavior:

```json
{
  "instructions": "You are a senior software engineer. Always provide code examples in Python.",
  "input": "How do I implement a binary search tree?"
}
```

Instructions are cached server-side when using `previous_response_id`, so they only need to be sent
once in a conversation.

### tools

Array of tool definitions. Built-in tools use a simple type declaration; custom tools use JSON Schema
for parameters:

```json
{
  "tools": [
    { "type": "web_search" },
    { "type": "file_search", "vector_store_ids": ["vs_abc123"] },
    { "type": "code_interpreter" },
    {
      "type": "function",
      "name": "get_weather",
      "description": "Get current weather for a location",
      "parameters": {
        "type": "object",
        "properties": {
          "location": { "type": "string", "description": "City name" }
        },
        "required": ["location"]
      }
    }
  ]
}
```

### tool_choice

Controls how the model selects tools:

- `"auto"` — Model decides whether to use a tool (default)
- `"none"` — Model must not use any tool
- `"required"` — Model must use at least one tool
- `{ "type": "function", "name": "get_weather" }` — Force a specific function
- `{ "type": "web_search" }` — Force a specific built-in tool

### text (Structured Output)

The `text` parameter configures text generation, most importantly structured output via JSON Schema:

```json
{
  "text": {
    "format": {
      "type": "json_schema",
      "name": "analysis_result",
      "schema": {
        "type": "object",
        "properties": {
          "sentiment": { "type": "string", "enum": ["positive", "negative", "neutral"] },
          "confidence": { "type": "number" },
          "summary": { "type": "string" }
        },
        "required": ["sentiment", "confidence", "summary"],
        "additionalProperties": false
      },
      "strict": true
    }
  }
}
```

Other format options:
- `{ "type": "text" }` — Plain text (default)
- `{ "type": "json_object" }` — Any valid JSON

### reasoning (o-series Models)

For reasoning models like `o4-mini` and `o3`, you can configure reasoning behavior:

```json
{
  "model": "o4-mini",
  "reasoning": {
    "effort": "high",
    "summary": "auto"
  }
}
```

- `effort`: `"low"`, `"medium"`, or `"high"` — controls how much thinking the model does
- `summary`: `"auto"`, `"concise"`, or `"detailed"` — whether to include a summary of the
  reasoning chain in the output (since the full chain-of-thought is hidden)

### store and previous_response_id

When `store: true`, the response is saved server-side and can be referenced later:

```json
// First turn
{
  "model": "gpt-4.1",
  "input": "My name is Alice",
  "store": true
}
// Returns: { "id": "resp_abc123", ... }

// Second turn
{
  "model": "gpt-4.1",
  "input": "What's my name?",
  "previous_response_id": "resp_abc123",
  "store": true
}
```

### truncation

Controls what happens when the conversation exceeds the model's context window:

- `"disabled"` (default) — Returns an error if the input is too long
- `"auto"` — Automatically drops earlier items from the conversation to fit within the context
  window, preserving the most recent items

### include

Request additional data in the response. Options include:

- `"file_search_call.results"` — Include file search results with chunk text and scores
- `"message.input_image.image_url"` — Include resolved image URLs
- `"computer_call_output.output.image_url"` — Include CUA screenshot URLs

```json
{
  "include": ["file_search_call.results"]
}
```

---

## Response Schema

```json
{
  "id": "resp_abc123def456",
  "object": "response",
  "created_at": 1709000000,
  "model": "gpt-4.1-2025-04-14",
  "status": "completed",
  "output": [
    {
      "type": "message",
      "id": "msg_abc123",
      "role": "assistant",
      "content": [
        {
          "type": "output_text",
          "text": "The capital of France is Paris.",
          "annotations": []
        }
      ],
      "status": "completed"
    }
  ],
  "output_text": "The capital of France is Paris.",
  "usage": {
    "input_tokens": 25,
    "output_tokens": 12,
    "total_tokens": 37,
    "input_tokens_details": {
      "cached_tokens": 0
    },
    "output_tokens_details": {
      "reasoning_tokens": 0
    }
  },
  "metadata": {},
  "temperature": 1.0,
  "top_p": 1.0,
  "max_output_tokens": null,
  "truncation": "disabled",
  "tool_choice": "auto",
  "parallel_tool_calls": true
}
```

### Key Response Fields

| Field          | Type     | Description                                                       |
| -------------- | -------- | ----------------------------------------------------------------- |
| `id`           | string   | Unique response identifier (prefix: `resp_`)                     |
| `object`       | string   | Always `"response"`                                               |
| `created_at`   | integer  | Unix timestamp                                                    |
| `model`        | string   | Actual model used (includes date suffix)                          |
| `status`       | string   | `"completed"`, `"failed"`, `"in_progress"`, `"cancelled"`        |
| `output`       | array    | Array of output items                                             |
| `output_text`  | string   | Convenience field: concatenation of all text in output            |
| `usage`        | object   | Token usage statistics                                            |
| `metadata`     | object   | Echoed back from request                                          |
| `error`        | object   | Error details if status is `"failed"`                             |

### Output Item Types

The `output` array can contain several types of items:

**Message item:**
```json
{
  "type": "message",
  "role": "assistant",
  "content": [{ "type": "output_text", "text": "..." }]
}
```

**Function call item (for custom tools):**
```json
{
  "type": "function_call",
  "id": "fc_abc123",
  "call_id": "call_abc123",
  "name": "get_weather",
  "arguments": "{\"location\": \"San Francisco\"}"
}
```

**Web search call item:**
```json
{
  "type": "web_search_call",
  "id": "ws_abc123",
  "status": "completed"
}
```

**File search call item:**
```json
{
  "type": "file_search_call",
  "id": "fs_abc123",
  "status": "completed",
  "queries": ["relevant query"],
  "results": [...]
}
```

**Code interpreter call item:**
```json
{
  "type": "code_interpreter_call",
  "id": "ci_abc123",
  "code": "print(2 + 2)",
  "results": [{ "type": "logs", "logs": "4\n" }]
}
```

**Reasoning item (o-series with summary):**
```json
{
  "type": "reasoning",
  "id": "rs_abc123",
  "summary": [{ "type": "summary_text", "text": "Thinking about the problem..." }]
}
```

---

## Built-in Tools Deep Dive

### web_search

When `web_search` is in the tools array, the model can search the internet in real-time. The API
handles the entire search flow: the model formulates queries, results are fetched, and the model
synthesizes an answer.

```json
{
  "model": "gpt-4.1",
  "tools": [{ "type": "web_search" }],
  "input": "What happened in tech news today?"
}
```

Configuration options:
- `search_context_size`: `"low"`, `"medium"` (default), or `"high"` — controls how much search
  context is retrieved. Higher means more tokens used but better grounding.
- `user_location`: Approximate location for geo-relevant queries (country, city, region, timezone).

The response includes `web_search_call` items in the output and annotations on text content with
source URLs for citations.

### file_search

Vector store-backed retrieval augmented generation (RAG). You upload files to a vector store via the
Files and Vector Stores APIs, then reference the store:

```json
{
  "model": "gpt-4.1",
  "tools": [{
    "type": "file_search",
    "vector_store_ids": ["vs_abc123"],
    "max_num_results": 10,
    "ranking_options": {
      "ranker": "auto",
      "score_threshold": 0.5
    }
  }],
  "input": "What does the Q3 report say about revenue?"
}
```

The model formulates search queries, retrieves relevant chunks from the vector store, and uses them
to answer. File search supports filtering by file metadata and customizing the ranking behavior.

### code_interpreter

A sandboxed Python environment where the model can write and execute code. Useful for math, data
analysis, chart generation, and file processing:

```json
{
  "model": "gpt-4.1",
  "tools": [{ "type": "code_interpreter" }],
  "input": "Calculate the first 20 Fibonacci numbers and plot them"
}
```

The model writes Python code, executes it in a sandbox, and can iterate if there are errors. The
response includes `code_interpreter_call` items showing the code and output. Generated files
(charts, CSVs, etc.) are returned as file references.

Configuration:
- `container`: Specify a custom container environment with pre-installed packages
- `files`: Attach file IDs for the interpreter to work with

### computer_use (Preview)

Computer Use Agent (CUA) enables the model to control a virtual desktop—clicking, typing, scrolling,
and taking screenshots. This is currently in preview and requires the `computer-use-2025-03-11`
model variant:

```json
{
  "model": "computer-use-preview",
  "tools": [{
    "type": "computer_use_preview",
    "display_width": 1024,
    "display_height": 768,
    "environment": "browser"
  }],
  "input": "Go to github.com and search for popular Python projects"
}
```

The model outputs `computer_call` items with actions (click, type, scroll, screenshot, etc.) and
expects screenshots as input for the next turn.

### mcp (Model Context Protocol)

The MCP tool type allows the model to connect to external MCP servers, enabling access to any
MCP-compatible tool or resource:

```json
{
  "model": "gpt-4.1",
  "tools": [{
    "type": "mcp",
    "server_label": "my_server",
    "server_url": "https://my-mcp-server.example.com/sse",
    "headers": { "Authorization": "Bearer token123" },
    "require_approval": "never"
  }],
  "input": "Use the database tool to query recent orders"
}
```

The model discovers available tools from the MCP server and can call them as needed. The
`require_approval` field controls whether tool calls need explicit approval (`always`, `never`).

---

## Why Codex CLI Uses This API Exclusively

OpenAI's Codex CLI (the open-source coding agent) uses the Responses API exclusively rather than
Chat Completions. The reasons illustrate why the Responses API is better for agents:

1. **Built-in conversation state.** Codex CLI uses `previous_response_id` to maintain conversation
   context without resending hundreds of messages. This is critical for long coding sessions.

2. **Reasoning model support.** Codex CLI uses o4-mini and o3 models that benefit from the
   `reasoning` parameter (effort levels, summaries). This config isn't available in Chat Completions
   in the same way.

3. **Simplified tool loop.** While Codex CLI uses custom function tools (shell commands, file
   operations), the items-based model makes the tool call/result cycle cleaner.

4. **Streaming with rich item types.** The streaming format surfaces tool calls, reasoning, and text
   as distinct items, making it easier to build rich terminal UIs.

5. **Future-proof.** All new OpenAI features ship on the Responses API first. Building on Chat
   Completions means missing out on new capabilities.

---

## Streaming Format

The Responses API uses Server-Sent Events (SSE) for streaming, but with a different event structure
than Chat Completions.

### Chat Completions streaming (old):
```
data: {"id":"chatcmpl-...","object":"chat.completion.chunk","choices":[{"delta":{"content":"Hello"}}]}
```

### Responses API streaming (new):
```
event: response.created
data: {"id":"resp_...","object":"response","status":"in_progress",...}

event: response.output_item.added
data: {"type":"message","role":"assistant",...}

event: response.content_part.added
data: {"type":"output_text","text":""}

event: response.output_text.delta
data: {"delta":"Hello"}

event: response.output_text.delta
data: {"delta":" world"}

event: response.output_text.done
data: {"text":"Hello world"}

event: response.content_part.done
data: {...}

event: response.output_item.done
data: {...}

event: response.completed
data: {"id":"resp_...","status":"completed",...}
```

Key differences:
- **Named event types** instead of generic `data:` lines. Events like `response.output_text.delta`,
  `response.function_call_arguments.delta`, `response.web_search_call.in_progress` etc.
- **Lifecycle events** for items and content parts (added → delta → done)
- **Final response object** is sent as `response.completed` event
- **Tool events** are streamed as they happen (search in progress, code executing, etc.)

---

## Multi-Turn Conversation Management

### Using previous_response_id (Recommended)

The simplest approach—let OpenAI manage state:

```python
import openai

client = openai.OpenAI()

# Turn 1
response1 = client.responses.create(
    model="gpt-4.1",
    input="My name is Alice and I'm working on a Python web scraper",
    store=True
)

# Turn 2 - references turn 1 automatically
response2 = client.responses.create(
    model="gpt-4.1",
    input="What libraries should I use for that?",
    previous_response_id=response1.id,
    store=True
)

# Turn 3 - references turn 2 (which chains to turn 1)
response3 = client.responses.create(
    model="gpt-4.1",
    input="Show me an example with BeautifulSoup",
    previous_response_id=response2.id,
    store=True
)
```

### Manual Conversation Management

You can also manage the conversation yourself by including previous items in the input:

```python
response = client.responses.create(
    model="gpt-4.1",
    input=[
        {"type": "message", "role": "user", "content": "My name is Alice"},
        {"type": "message", "role": "assistant", "content": "Nice to meet you, Alice!"},
        {"type": "message", "role": "user", "content": "What's my name?"}
    ]
)
```

This is useful when you want full control over what context the model sees, or when you want to
inject or modify conversation history.

---

## Code Examples

### Python

```python
from openai import OpenAI

client = OpenAI()  # Uses OPENAI_API_KEY env var

# Simple text response
response = client.responses.create(
    model="gpt-4.1",
    input="Explain quantum entanglement in simple terms"
)
print(response.output_text)

# With web search
response = client.responses.create(
    model="gpt-4.1",
    tools=[{"type": "web_search"}],
    input="What are the top AI papers published this week?"
)
print(response.output_text)

# Structured output
response = client.responses.create(
    model="gpt-4.1",
    input="Analyze the sentiment of: 'This product is amazing!'",
    text={
        "format": {
            "type": "json_schema",
            "name": "sentiment_analysis",
            "schema": {
                "type": "object",
                "properties": {
                    "sentiment": {"type": "string"},
                    "score": {"type": "number"}
                },
                "required": ["sentiment", "score"],
                "additionalProperties": False
            },
            "strict": True
        }
    }
)
import json
result = json.loads(response.output_text)

# Streaming
stream = client.responses.create(
    model="gpt-4.1",
    input="Write a short story about a robot learning to paint",
    stream=True
)
for event in stream:
    if event.type == "response.output_text.delta":
        print(event.delta, end="", flush=True)

# With reasoning model
response = client.responses.create(
    model="o4-mini",
    input="Solve this step by step: If a train travels at 60mph for 2.5 hours, how far does it go?",
    reasoning={"effort": "medium", "summary": "concise"}
)
print(response.output_text)
```

### TypeScript

```typescript
import OpenAI from "openai";

const client = new OpenAI(); // Uses OPENAI_API_KEY env var

// Simple request
const response = await client.responses.create({
  model: "gpt-4.1",
  input: "Explain quantum entanglement in simple terms",
});
console.log(response.output_text);

// With tools and multi-turn
const response1 = await client.responses.create({
  model: "gpt-4.1",
  tools: [{ type: "web_search" }],
  input: "Find the current Bitcoin price",
  store: true,
});

const response2 = await client.responses.create({
  model: "gpt-4.1",
  input: "How does that compare to last month?",
  previous_response_id: response1.id,
  store: true,
});

// Streaming
const stream = await client.responses.create({
  model: "gpt-4.1",
  input: "Write a haiku about programming",
  stream: true,
});

for await (const event of stream) {
  if (event.type === "response.output_text.delta") {
    process.stdout.write(event.delta);
  }
}
```

### curl

```bash
# Simple request
curl https://api.openai.com/v1/responses \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4.1",
    "input": "What is the capital of France?"
  }'

# With web search
curl https://api.openai.com/v1/responses \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4.1",
    "tools": [{"type": "web_search"}],
    "input": "What happened in tech news today?"
  }'

# With custom function tool
curl https://api.openai.com/v1/responses \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4.1",
    "tools": [{
      "type": "function",
      "name": "get_weather",
      "description": "Get weather for a location",
      "parameters": {
        "type": "object",
        "properties": {
          "location": {"type": "string"}
        },
        "required": ["location"]
      }
    }],
    "input": "What is the weather in Tokyo?"
  }'

# Structured output
curl https://api.openai.com/v1/responses \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4.1",
    "input": "Extract: John is 30 and lives in NYC",
    "text": {
      "format": {
        "type": "json_schema",
        "name": "person",
        "schema": {
          "type": "object",
          "properties": {
            "name": {"type": "string"},
            "age": {"type": "integer"},
            "city": {"type": "string"}
          },
          "required": ["name", "age", "city"],
          "additionalProperties": false
        },
        "strict": true
      }
    }
  }'

# Multi-turn with previous_response_id
curl https://api.openai.com/v1/responses \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4.1",
    "input": "What did I just ask you?",
    "previous_response_id": "resp_abc123def456"
  }'
```

---

## Migration Guide: Chat Completions → Responses API

### Conceptual Mapping

| Chat Completions          | Responses API                     |
| ------------------------- | --------------------------------- |
| `messages` array          | `input` items array               |
| `role: "system"` message  | `instructions` parameter          |
| `role: "user"` message    | `type: "message", role: "user"`   |
| `role: "assistant"`       | `type: "message", role: "assistant"` |
| `role: "tool"` message    | `type: "function_call_output"` item |
| `functions` / `tools`     | `tools` array (same for functions)|
| `function_call` / `tool_choice` | `tool_choice`               |
| `response_format`         | `text.format`                     |
| `finish_reason`           | `status` (on response) + `stop_reason` on items |
| `choices[0].message`      | `output` array                    |
| N/A                       | `previous_response_id`            |
| N/A                       | `store`                           |
| N/A                       | `background`                      |
| N/A                       | Built-in tools                    |

### Before (Chat Completions)

```python
response = client.chat.completions.create(
    model="gpt-4o",
    messages=[
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "Hello!"}
    ],
    temperature=0.7,
    max_tokens=500
)
print(response.choices[0].message.content)
```

### After (Responses API)

```python
response = client.responses.create(
    model="gpt-4.1",
    instructions="You are a helpful assistant.",
    input="Hello!",
    temperature=0.7,
    max_output_tokens=500
)
print(response.output_text)
```

### Tool Calling Migration

**Before:**
```python
# Step 1: Initial request
response = client.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "What's the weather in Paris?"}],
    tools=[{
        "type": "function",
        "function": {
            "name": "get_weather",
            "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}
        }
    }]
)

# Step 2: Extract tool call
tool_call = response.choices[0].message.tool_calls[0]

# Step 3: Execute and send result
result = get_weather(json.loads(tool_call.function.arguments))
response2 = client.chat.completions.create(
    model="gpt-4o",
    messages=[
        {"role": "user", "content": "What's the weather in Paris?"},
        response.choices[0].message,
        {"role": "tool", "tool_call_id": tool_call.id, "content": json.dumps(result)}
    ]
)
```

**After:**
```python
# Step 1: Initial request (same)
response = client.responses.create(
    model="gpt-4.1",
    input="What's the weather in Paris?",
    tools=[{
        "type": "function",
        "name": "get_weather",
        "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}
    }]
)

# Step 2: Check for function calls and send results
if response.output and response.output[0].type == "function_call":
    fc = response.output[0]
    result = get_weather(json.loads(fc.arguments))
    response2 = client.responses.create(
        model="gpt-4.1",
        input=[{
            "type": "function_call_output",
            "call_id": fc.call_id,
            "output": json.dumps(result)
        }],
        previous_response_id=response.id
    )
```

---

## Advantages for Agent Development

1. **Reduced boilerplate.** Built-in tools eliminate the need to implement web search, RAG, and code
   execution infrastructure. An agent can search the web, analyze files, and run code out of the box.

2. **Server-side state management.** With `previous_response_id` and `store`, the API manages
   conversation history. Agents don't need to track and resend growing message arrays.

3. **Automatic truncation.** The `truncation: "auto"` setting handles context window overflow
   gracefully, which is essential for long-running agents that accumulate context.

4. **Rich output types.** The items-based output model lets agents distinguish between text, tool
   calls, reasoning, and other output types cleanly.

5. **Background execution.** Long-running agentic tasks can run asynchronously, with the client
   polling for results instead of holding open a connection.

6. **MCP integration.** Native support for Model Context Protocol means agents can connect to any
   MCP-compatible tool server without custom integration code.

7. **Reasoning control.** For reasoning models, the `effort` parameter lets agents balance speed vs
   quality dynamically based on task complexity.

---

## Current Limitations

1. **No equivalent of `n` parameter.** Chat Completions can generate multiple completions per
   request (`n > 1`). The Responses API generates one response per request.

2. **No logprobs.** Token log probabilities are not available in the Responses API (available in
   Chat Completions for non-reasoning models).

3. **Stored responses require API access.** When using `previous_response_id`, the conversation
   state lives on OpenAI's servers. You can't inspect or modify intermediate state without
   retrieving the response via the GET endpoint.

4. **Built-in tool costs.** Web search, file search, and code interpreter have additional token
   costs and per-call costs beyond the base model pricing. Web search in particular can add
   significant cost for high-volume applications.

5. **Background mode polling.** There's no webhook/push notification for background responses yet—
   you must poll the GET endpoint to check completion status.

6. **Computer use is preview-only.** The CUA tool is experimental, limited to specific model
   variants, and not recommended for production use.

7. **Not all models supported.** Older models (GPT-3.5 Turbo, earlier GPT-4 variants) are not
   available through the Responses API. It targets GPT-4o, GPT-4.1, and o-series models.

8. **MCP tool is relatively new.** The MCP integration is still evolving and may have limitations
   around authentication, tool discovery, and error handling compared to native built-in tools.

---

## Further Reading

- [OpenAI Responses API Reference](https://platform.openai.com/docs/api-reference/responses)
- [Responses vs Chat Completions Guide](https://platform.openai.com/docs/guides/responses-vs-chat-completions)
- [Built-in Tools Guide](https://platform.openai.com/docs/guides/tools)
- [Structured Outputs Guide](https://platform.openai.com/docs/guides/structured-outputs)
- [Codex CLI Source Code](https://github.com/openai/codex) — Real-world Responses API usage