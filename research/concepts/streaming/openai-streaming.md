# OpenAI Streaming Formats

OpenAI exposes three distinct streaming interfaces, each designed for different
use cases. This document covers all three in depth: the Chat Completions SSE
stream, the newer Responses API event stream, and the Realtime WebSocket API.

---

## 1. Chat Completions API Streaming

### Endpoint and Activation

```
POST https://api.openai.com/v1/chat/completions
```

Set `stream: true` in the request body. The response changes from a single JSON
object to a stream of Server-Sent Events (SSE).

```json
{
  "model": "gpt-4o",
  "messages": [
    { "role": "system", "content": "You are a helpful assistant." },
    { "role": "user", "content": "Write a haiku about streaming." }
  ],
  "stream": true,
  "stream_options": { "include_usage": true }
}
```

### SSE Wire Format

Each event is a line prefixed with `data: ` followed by a JSON object, then a
blank line. The stream terminates with a special sentinel:

```
data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1719000000,"model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}\n\n
data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1719000000,"model":"gpt-4o","choices":[{"index":0,"delta":{"content":"Bytes"},"finish_reason":null}]}\n\n
data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1719000000,"model":"gpt-4o","choices":[{"index":0,"delta":{"content":" flow"},"finish_reason":null}]}\n\n
data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1719000000,"model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}\n\n
data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1719000000,"model":"gpt-4o","choices":[],"usage":{"prompt_tokens":22,"completion_tokens":17,"total_tokens":39}}\n\n
data: [DONE]\n\n
```

### Delta Object Structure

Each chunk's `choices[i].delta` is a partial object. Unlike the non-streaming
response which has a complete `message`, the delta contains only the *new*
fields for that chunk:

| Field | When Present | Description |
|-------|-------------|-------------|
| `role` | First chunk only | Always `"assistant"` |
| `content` | Text generation chunks | Fragment of the text response |
| `tool_calls` | Tool call chunks | Incremental tool call data |
| `refusal` | Content refusal | Safety refusal message fragments |

The `delta` object is intentionally sparse — it only includes fields that have
new data. An empty `delta: {}` with a `finish_reason` signals the end of
generation for that choice.

### Incremental Tool Call Assembly

Tool calls stream incrementally, which is the most complex part of Chat
Completions streaming. Here is exactly how it works:

**First chunk for a tool call:**
```json
{
  "choices": [{
    "index": 0,
    "delta": {
      "tool_calls": [{
        "index": 0,
        "id": "call_abc123",
        "type": "function",
        "function": {
          "name": "get_weather",
          "arguments": ""
        }
      }]
    },
    "finish_reason": null
  }]
}
```

The first chunk carries the `id`, `type`, and `function.name`. The `arguments`
field starts empty.

**Subsequent chunks for that tool call:**
```json
{
  "choices": [{
    "index": 0,
    "delta": {
      "tool_calls": [{
        "index": 0,
        "function": {
          "arguments": "{\"lo"
        }
      }]
    },
    "finish_reason": null
  }]
}
```

```json
{
  "choices": [{
    "index": 0,
    "delta": {
      "tool_calls": [{
        "index": 0,
        "function": {
          "arguments": "catio"
        }
      }]
    },
    "finish_reason": null
  }]
}
```

```json
{
  "choices": [{
    "index": 0,
    "delta": {
      "tool_calls": [{
        "index": 0,
        "function": {
          "arguments": "n\": \"San"
        }
      }]
    },
    "finish_reason": null
  }]
}
```

```json
{
  "choices": [{
    "index": 0,
    "delta": {
      "tool_calls": [{
        "index": 0,
        "function": {
          "arguments": " Francisco\"}"
        }
      }]
    },
    "finish_reason": null
  }]
}
```

**Assembly algorithm:**

1. Maintain a map of `index -> { id, name, arguments_buffer }`.
2. On each chunk, look at `tool_calls[i].index` to identify which tool call.
3. If `id` is present, this is the first chunk — store `id` and `name`.
4. Concatenate `function.arguments` to the buffer for that index.
5. When `finish_reason` is `"tool_calls"`, parse each buffer as JSON.

**Parallel tool calls — interleaved by index:**

When the model makes multiple tool calls simultaneously, chunks interleave
using the `index` field:

```json
// Chunk 1: First tool call starts
{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_aaa","type":"function","function":{"name":"get_weather","arguments":""}}]}}]}

// Chunk 2: Second tool call starts
{"choices":[{"delta":{"tool_calls":[{"index":1,"id":"call_bbb","type":"function","function":{"name":"get_time","arguments":""}}]}}]}

// Chunk 3: Arguments for tool 0
{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"city\":"}}]}}]}

// Chunk 4: Arguments for tool 1
{"choices":[{"delta":{"tool_calls":[{"index":1,"function":{"arguments":"{\"timezone\":"}}]}}]}

// Chunk 5: More arguments for tool 0
{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"NYC\"}"}}]}}]}

// Chunk 6: More arguments for tool 1
{"choices":[{"delta":{"tool_calls":[{"index":1,"function":{"arguments":"\"EST\"}"}}]}}]}

// Final chunk
{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}
```

After assembly:
- Tool 0: `get_weather({"city":"NYC"})`  with id `call_aaa`
- Tool 1: `get_time({"timezone":"EST"})` with id `call_bbb`

### finish_reason Values

| Value | Meaning |
|-------|---------|
| `stop` | Natural end of generation or stop sequence hit |
| `tool_calls` | Model wants to invoke one or more tools |
| `length` | Hit `max_tokens` or context length limit |
| `content_filter` | Content was flagged by the safety system |
| `null` | Still generating (intermediate chunks) |

### The [DONE] Sentinel

The final line of every Chat Completions stream is:

```
data: [DONE]\n\n
```

This is NOT valid JSON. Clients must check for `data: [DONE]` as a string
before attempting JSON parse. The `[DONE]` sentinel signals that no more
events will follow and the HTTP connection will close.

### Usage Tracking: stream_options

By default, streaming responses do not include token usage. To get usage data,
pass:

```json
"stream_options": { "include_usage": true }
```

This adds a final chunk (before `[DONE]`) with an empty `choices` array and a
`usage` object:

```json
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion.chunk",
  "created": 1719000000,
  "model": "gpt-4o",
  "choices": [],
  "usage": {
    "prompt_tokens": 42,
    "completion_tokens": 128,
    "total_tokens": 170,
    "completion_tokens_details": {
      "reasoning_tokens": 0,
      "accepted_prediction_tokens": 0,
      "rejected_prediction_tokens": 0
    }
  }
}
```

---

## 2. Responses API Streaming (Newer Format)

### Endpoint and Activation

```
POST https://api.openai.com/v1/responses
```

Set `stream: true` in the request body. The Responses API uses named SSE events
(the `event:` field in SSE), unlike Chat Completions which only uses `data:`.

```json
{
  "model": "gpt-4o",
  "input": "Explain streaming in one sentence.",
  "stream": true
}
```

### SSE Wire Format with Named Events

```
event: response.created
data: {"id":"resp_abc123","object":"response","status":"in_progress","output":[],...}

event: response.in_progress
data: {"id":"resp_abc123","object":"response","status":"in_progress",...}

event: response.output_item.added
data: {"item":{"id":"item_0","type":"message","role":"assistant","content":[]},"output_index":0}

event: response.content_part.added
data: {"item_id":"item_0","output_index":0,"content_index":0,"part":{"type":"output_text","text":""}}

event: response.content_part.delta
data: {"item_id":"item_0","output_index":0,"content_index":0,"delta":{"type":"text_delta","text":"Streaming"}}

event: response.content_part.delta
data: {"item_id":"item_0","output_index":0,"content_index":0,"delta":{"type":"text_delta","text":" sends data"}}

event: response.content_part.delta
data: {"item_id":"item_0","output_index":0,"content_index":0,"delta":{"type":"text_delta","text":" incrementally."}}

event: response.content_part.done
data: {"item_id":"item_0","output_index":0,"content_index":0,"part":{"type":"output_text","text":"Streaming sends data incrementally."}}

event: response.output_item.done
data: {"item":{"id":"item_0","type":"message","role":"assistant","content":[{"type":"output_text","text":"Streaming sends data incrementally."}]},"output_index":0}

event: response.completed
data: {"id":"resp_abc123","object":"response","status":"completed","output":[...],"usage":{"input_tokens":12,"output_tokens":7,"total_tokens":19}}
```

### Complete Event Type Reference

**Response lifecycle events:**

| Event | Description |
|-------|-------------|
| `response.created` | Response object created, status `in_progress` |
| `response.in_progress` | Generation has started |
| `response.completed` | Generation finished successfully; includes full response + usage |
| `response.failed` | Generation failed; includes error details |
| `response.cancelled` | Response was cancelled by the client |
| `response.incomplete` | Response ended incomplete (e.g., max tokens) |

**Output item events:**

| Event | Description |
|-------|-------------|
| `response.output_item.added` | New output item started (message, function_call, etc.) |
| `response.output_item.done` | Output item completed with full content |

**Content part events (for message items):**

| Event | Description |
|-------|-------------|
| `response.content_part.added` | New content part started within a message |
| `response.content_part.delta` | Incremental text/audio delta |
| `response.content_part.done` | Content part completed |

**Function call events:**

| Event | Description |
|-------|-------------|
| `response.function_call_arguments.delta` | Partial JSON arguments for a tool call |
| `response.function_call_arguments.done` | Complete arguments string for the tool call |

**Reasoning events (o-series models):**

| Event | Description |
|-------|-------------|
| `response.reasoning_summary_part.added` | Reasoning summary part started |
| `response.reasoning_summary_part.delta` | Reasoning summary text delta |
| `response.reasoning_summary_part.done` | Reasoning summary part completed |

### Output Item Types

The Responses API models output as a list of typed items:

- **`message`** — A text or multimodal assistant message with content parts
- **`function_call`** — A request to invoke a tool; has `name`, `call_id`, `arguments`
- **`function_call_output`** — The result of a tool invocation (client-provided)
- **`reasoning`** — Internal chain-of-thought for o-series models (o1, o3, etc.)

### Function Call Streaming in Responses API

Tool calls have dedicated lifecycle events, making them much cleaner than the
Chat Completions index-based approach:

```
event: response.output_item.added
data: {"item":{"id":"fc_001","type":"function_call","name":"get_weather","call_id":"call_xyz","arguments":""},"output_index":1}

event: response.function_call_arguments.delta
data: {"item_id":"fc_001","output_index":1,"delta":"{\"lo"}

event: response.function_call_arguments.delta
data: {"item_id":"fc_001","output_index":1,"delta":"cation\":"}

event: response.function_call_arguments.delta
data: {"item_id":"fc_001","output_index":1,"delta":" \"Paris\"}"}

event: response.function_call_arguments.done
data: {"item_id":"fc_001","output_index":1,"arguments":"{\"location\": \"Paris\"}"}

event: response.output_item.done
data: {"item":{"id":"fc_001","type":"function_call","name":"get_weather","call_id":"call_xyz","arguments":"{\"location\": \"Paris\"}"},"output_index":1}
```

Key differences from Chat Completions tool call streaming:
- Each function call is a separate **output item** with its own lifecycle
- `function_call_arguments.done` gives you the complete arguments string —
  no manual concatenation required
- `item_id` and `call_id` provide clear identity (no index-based multiplexing)
- Parallel tool calls are separate items with separate event streams

### How It Differs from Chat Completions

| Aspect | Chat Completions | Responses API |
|--------|-----------------|---------------|
| Event granularity | Flat chunks with `choices[i].delta` | Named events per item/content part |
| Tool call streaming | Index-based interleaving, manual assembly | Dedicated lifecycle events per call |
| Reasoning tokens | Hidden, only in usage counts | Exposed as `reasoning` output items |
| Event naming | No SSE event names (`data:` only) | Named SSE events (`event:` + `data:`) |
| Completion signal | `data: [DONE]` | `response.completed` event |
| Usage | Optional via `stream_options` | Always in `response.completed` |
| Multi-turn state | Stateless (client manages history) | Can use `previous_response_id` |

### Why Codex Uses the Responses API Exclusively

Codex (the CLI agent) uses the Responses API for several architectural reasons:

1. **Tool call lifecycle management** — The explicit `output_item.added` →
   `function_call_arguments.delta` → `function_call_arguments.done` →
   `output_item.done` lifecycle maps cleanly to agent execution states.

2. **Native local shell call support** — The Responses API supports custom tool
   types that align with Codex's local command execution model.

3. **Reasoning items for o-series models** — When using o3/o4-mini, reasoning
   items stream as separate output items, letting the agent display thinking
   progress distinctly from final output.

4. **Item-based architecture** — The item/content-part hierarchy maps naturally
   to Codex's internal event model where each output item corresponds to an
   agent action (text response, tool invocation, reasoning step).

---

## 3. Realtime API (WebSocket-Based)

### Connection

```
wss://api.openai.com/v1/realtime?model=gpt-4o-realtime-preview
```

Authentication via headers:

```
Authorization: Bearer sk-...
OpenAI-Beta: realtime=v1
```

### Architecture

The Realtime API is fully bidirectional over WebSocket. Both client and server
send JSON-encoded events. This is fundamentally different from the SSE-based
APIs which are unidirectional (server → client only).

### Session Configuration

```json
{
  "type": "session.update",
  "session": {
    "modalities": ["text", "audio"],
    "voice": "alloy",
    "input_audio_format": "pcm16",
    "output_audio_format": "pcm16",
    "input_audio_transcription": { "model": "whisper-1" },
    "turn_detection": {
      "type": "server_vad",
      "threshold": 0.5,
      "prefix_padding_ms": 300,
      "silence_duration_ms": 500
    },
    "tools": [
      {
        "type": "function",
        "name": "get_weather",
        "description": "Get weather for a location",
        "parameters": {
          "type": "object",
          "properties": {
            "location": { "type": "string" }
          },
          "required": ["location"]
        }
      }
    ]
  }
}
```

### Audio Streaming

**Supported formats:**
- `pcm16` — 16-bit PCM at 24kHz, little-endian, mono
- `g711_ulaw` — G.711 µ-law at 8kHz
- `g711_alaw` — G.711 A-law at 8kHz

**Sending audio:**
```json
{
  "type": "input_audio_buffer.append",
  "audio": "<base64-encoded-audio-chunk>"
}
```

**Receiving audio:**
```json
{
  "type": "response.audio.delta",
  "response_id": "resp_001",
  "item_id": "item_001",
  "output_index": 0,
  "content_index": 0,
  "delta": "<base64-encoded-audio-chunk>"
}
```

### Voice Activity Detection (VAD)

Server-side VAD detects when the user starts and stops speaking:

- `input_audio_buffer.speech_started` — user began speaking
- `input_audio_buffer.speech_stopped` — user stopped speaking
- `input_audio_buffer.committed` — audio committed for processing

Alternatively, disable VAD and manually commit audio buffers for push-to-talk
style interaction.

### Function Calling in Realtime

The server emits function call events similar to the Responses API:

```json
{
  "type": "response.function_call_arguments.delta",
  "response_id": "resp_001",
  "item_id": "item_002",
  "output_index": 1,
  "call_id": "call_abc",
  "delta": "{\"location\":"
}
```

After assembling the arguments, the client sends the result:

```json
{
  "type": "conversation.item.create",
  "item": {
    "type": "function_call_output",
    "call_id": "call_abc",
    "output": "{\"temperature\": 72, \"unit\": \"F\"}"
  }
}
```

Then trigger a new response:
```json
{ "type": "response.create" }
```

### When to Use Realtime API

- Voice-based coding assistants
- Real-time conversational interfaces
- Audio-in / audio-out applications
- Scenarios requiring sub-second latency for speech

---

## 4. Code Examples

### Python — Chat Completions Streaming

```python
from openai import OpenAI

client = OpenAI()

stream = client.chat.completions.create(
    model="gpt-4o",
    messages=[
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "Explain TCP in two sentences."},
    ],
    stream=True,
    stream_options={"include_usage": True},
)

collected_content = ""
for chunk in stream:
    # Usage-only chunk has empty choices
    if not chunk.choices:
        print(f"\nUsage: {chunk.usage}")
        continue

    delta = chunk.choices[0].delta

    if delta.content:
        collected_content += delta.content
        print(delta.content, end="", flush=True)

    if chunk.choices[0].finish_reason:
        print(f"\nFinish reason: {chunk.choices[0].finish_reason}")
```

### Python — Chat Completions with Tool Call Assembly

```python
import json
from openai import OpenAI

client = OpenAI()

tools = [
    {
        "type": "function",
        "function": {
            "name": "get_weather",
            "description": "Get weather for a location",
            "parameters": {
                "type": "object",
                "properties": {
                    "location": {"type": "string"},
                    "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]},
                },
                "required": ["location"],
            },
        },
    }
]

stream = client.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "What's the weather in Tokyo and London?"}],
    tools=tools,
    stream=True,
)

# Accumulate tool calls by index
tool_calls_acc: dict[int, dict] = {}

for chunk in stream:
    if not chunk.choices:
        continue

    delta = chunk.choices[0].delta

    if delta.tool_calls:
        for tc in delta.tool_calls:
            idx = tc.index
            if idx not in tool_calls_acc:
                tool_calls_acc[idx] = {
                    "id": tc.id,
                    "name": tc.function.name,
                    "arguments": "",
                }
            if tc.function and tc.function.arguments:
                tool_calls_acc[idx]["arguments"] += tc.function.arguments

    if chunk.choices[0].finish_reason == "tool_calls":
        for idx in sorted(tool_calls_acc):
            tc = tool_calls_acc[idx]
            args = json.loads(tc["arguments"])
            print(f"Tool call {idx}: {tc['name']}({args}) [id={tc['id']}]")
```

### Python — Responses API Streaming

```python
from openai import OpenAI

client = OpenAI()

stream = client.responses.create(
    model="gpt-4o",
    input="Write a haiku about APIs.",
    stream=True,
)

for event in stream:
    if event.type == "response.content_part.delta":
        print(event.delta.text, end="", flush=True)
    elif event.type == "response.function_call_arguments.done":
        print(f"\nTool call ready: {event.arguments}")
    elif event.type == "response.completed":
        usage = event.response.usage
        print(f"\nTokens: {usage.input_tokens} in, {usage.output_tokens} out")
```

### TypeScript — Chat Completions Streaming

```typescript
import OpenAI from "openai";

const client = new OpenAI();

async function main() {
  const stream = await client.chat.completions.create({
    model: "gpt-4o",
    messages: [{ role: "user", content: "Hello, how are you?" }],
    stream: true,
  });

  for await (const chunk of stream) {
    const content = chunk.choices[0]?.delta?.content;
    if (content) {
      process.stdout.write(content);
    }
  }
  console.log();
}

main();
```

### TypeScript — Responses API Streaming

```typescript
import OpenAI from "openai";

const client = new OpenAI();

async function main() {
  const stream = await client.responses.create({
    model: "gpt-4o",
    input: "Explain REST in one sentence.",
    stream: true,
  });

  for await (const event of stream) {
    switch (event.type) {
      case "response.content_part.delta":
        process.stdout.write(event.delta.text ?? "");
        break;
      case "response.function_call_arguments.done":
        console.log(`\nTool: ${event.arguments}`);
        break;
      case "response.completed":
        const usage = event.response.usage;
        console.log(`\nTokens: ${usage.input_tokens}+${usage.output_tokens}`);
        break;
    }
  }
}

main();
```

### curl — Chat Completions Streaming

```bash
curl -N https://api.openai.com/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -d '{
    "model": "gpt-4o",
    "messages": [{"role": "user", "content": "Say hello"}],
    "stream": true
  }'
```

The `-N` flag disables output buffering, critical for seeing SSE events as
they arrive.

### curl — Responses API Streaming

```bash
curl -N https://api.openai.com/v1/responses \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -d '{
    "model": "gpt-4o",
    "input": "Say hello",
    "stream": true
  }'
```

---

## 5. Streaming with Function/Tool Calls — Complete Walkthrough

### Scenario: Model calls two tools in parallel

**Request:**
```json
{
  "model": "gpt-4o",
  "messages": [{"role": "user", "content": "Weather in NYC and time in Tokyo?"}],
  "tools": [
    {"type": "function", "function": {"name": "get_weather", "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}}},
    {"type": "function", "function": {"name": "get_time", "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}}}
  ],
  "stream": true
}
```

**Chunk-by-chunk wire trace:**

```
data: {"id":"chatcmpl-X","choices":[{"index":0,"delta":{"role":"assistant","content":null,"tool_calls":[{"index":0,"id":"call_w1","type":"function","function":{"name":"get_weather","arguments":""}}]},"finish_reason":null}]}

data: {"id":"chatcmpl-X","choices":[{"index":0,"delta":{"tool_calls":[{"index":1,"id":"call_t1","type":"function","function":{"name":"get_time","arguments":""}}]},"finish_reason":null}]}

data: {"id":"chatcmpl-X","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"ci"}}]},"finish_reason":null}]}

data: {"id":"chatcmpl-X","choices":[{"index":0,"delta":{"tool_calls":[{"index":1,"function":{"arguments":"{\"ci"}}]},"finish_reason":null}]}

data: {"id":"chatcmpl-X","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"ty\": \"NYC\"}"}}]},"finish_reason":null}]}

data: {"id":"chatcmpl-X","choices":[{"index":0,"delta":{"tool_calls":[{"index":1,"function":{"arguments":"ty\": \"Tokyo\"}"}}]},"finish_reason":null}]}

data: {"id":"chatcmpl-X","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}

data: [DONE]
```

**State after processing each chunk:**

| Chunk | tool_calls_acc[0] | tool_calls_acc[1] |
|-------|-------------------|-------------------|
| 1 | `{id: "call_w1", name: "get_weather", args: ""}` | — |
| 2 | (unchanged) | `{id: "call_t1", name: "get_time", args: ""}` |
| 3 | `args: "{\"ci"` | (unchanged) |
| 4 | (unchanged) | `args: "{\"ci"` |
| 5 | `args: "{\"city\": \"NYC\"}"` | (unchanged) |
| 6 | (unchanged) | `args: "{\"city\": \"Tokyo\"}"` |
| 7 | Parse → `{"city": "NYC"}` | Parse → `{"city": "Tokyo"}` |

---

## 6. Usage Tracking in Streaming

### Chat Completions

Requires opt-in via `stream_options`:

```json
"stream_options": { "include_usage": true }
```

The usage chunk is the **last data chunk before `[DONE]`** and has an empty
`choices` array:

```json
{
  "id": "chatcmpl-abc",
  "choices": [],
  "usage": {
    "prompt_tokens": 55,
    "completion_tokens": 32,
    "total_tokens": 87,
    "completion_tokens_details": {
      "reasoning_tokens": 0,
      "accepted_prediction_tokens": 0,
      "rejected_prediction_tokens": 0
    },
    "prompt_tokens_details": {
      "cached_tokens": 0
    }
  }
}
```

For o-series models, `reasoning_tokens` will be non-zero, reflecting the
internal chain-of-thought tokens that are not visible in the output.

### Responses API

Usage is always included in the `response.completed` event — no opt-in needed:

```json
{
  "type": "response.completed",
  "response": {
    "id": "resp_abc",
    "status": "completed",
    "usage": {
      "input_tokens": 55,
      "output_tokens": 32,
      "total_tokens": 87,
      "output_tokens_details": {
        "reasoning_tokens": 0
      }
    }
  }
}
```

---

## 7. Error Handling

### HTTP-Level Errors (Before Streaming Starts)

If the request is invalid or authentication fails, you get a normal HTTP error
response (not SSE). Common cases:

| Status | Cause |
|--------|-------|
| `400` | Malformed request, invalid model, bad parameters |
| `401` | Invalid or missing API key |
| `403` | Insufficient permissions for the model |
| `429` | Rate limited — check `Retry-After` header |
| `500` | Server error |
| `503` | Service overloaded |

These return a standard JSON error body:

```json
{
  "error": {
    "message": "Rate limit exceeded",
    "type": "rate_limit_error",
    "code": "rate_limit_exceeded"
  }
}
```

### Mid-Stream Errors

Once streaming has started, errors can manifest as:

1. **Connection drop** — TCP connection closes without `[DONE]` or
   `response.completed`. The client must detect this (e.g., `fetch` ReadableStream
   closes, `EventSource` fires `onerror`). Recovery strategy: retry with the
   same request and skip already-received content tokens.

2. **Error event in Responses API** — The stream may emit a `response.failed`
   event:

```
event: response.failed
data: {"id":"resp_abc","status":"failed","error":{"type":"server_error","message":"Internal error"}}
```

3. **Chat Completions mid-stream error** — Less structured. The stream may
   simply end without a `finish_reason` or `[DONE]`. Some implementations send
   an error chunk:

```
data: {"error":{"message":"Server error","type":"server_error","code":null}}
```

### Rate Limiting During Streaming

If you hit a rate limit *before* the stream starts, you get a 429 HTTP response.
If you're already streaming and hit a token-per-minute limit, the stream may
terminate early. The model will stop generating and the connection closes.

Best practices:
- Implement exponential backoff for 429 errors
- Track token usage across requests to stay within limits
- Use `stream_options.include_usage` to monitor consumption

### Content Filter Triggered Mid-Stream

When the safety system flags content during generation:

- `finish_reason` is set to `"content_filter"`
- The `content` may be truncated or empty
- In the Responses API, a `response.incomplete` event is emitted
- The `delta.refusal` field may contain the refusal message text

```json
{
  "choices": [{
    "index": 0,
    "delta": { "refusal": "I'm unable to help with that request." },
    "finish_reason": "content_filter"
  }]
}
```

### Defensive Streaming Client Checklist

1. Always check for `data: [DONE]` before JSON parsing
2. Handle empty `choices` arrays (usage-only chunks)
3. Handle missing fields in `delta` (sparse updates)
4. Set a read timeout — if no chunk arrives in N seconds, reconnect
5. Buffer tool call arguments; never parse partial JSON
6. Track `finish_reason` to distinguish normal completion from errors
7. Handle both connection-level and application-level errors
8. For Responses API: handle `response.failed` and `response.incomplete`
