# Google Gemini Streaming

Deep-dive into how Google's Gemini API handles streaming — from the wire protocol
and multimodal payloads to the Live API's WebSocket layer and OpenAI compatibility.

---

## 1. GenerateContent Streaming

### Endpoint

```
POST https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent?alt=sse&key={API_KEY}
```

The `alt=sse` query parameter switches the response from a single JSON blob to a
Server-Sent Events stream. Without it you get a blocking unary response.

### Request body

```json
{
  "contents": [
    {
      "role": "user",
      "parts": [{ "text": "Explain quantum entanglement in simple terms." }]
    }
  ],
  "generationConfig": {
    "temperature": 0.7,
    "maxOutputTokens": 1024,
    "topP": 0.95,
    "topK": 40
  }
}
```

### SSE wire format

Each event is prefixed with `data: ` and separated by a blank line, following
the standard SSE spec:

```
data: {"candidates":[{"content":{"parts":[{"text":"Quantum"}],"role":"model"},"index":0,"safetyRatings":[{"category":"HARM_CATEGORY_SEXUALLY_EXPLICIT","probability":"NEGLIGIBLE"},{"category":"HARM_CATEGORY_HATE_SPEECH","probability":"NEGLIGIBLE"}]}]}

data: {"candidates":[{"content":{"parts":[{"text":" entanglement is"}],"role":"model"},"index":0}]}

data: {"candidates":[{"content":{"parts":[{"text":" a phenomenon where two particles become linked..."}],"role":"model"},"index":0,"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":8,"candidatesTokenCount":142,"totalTokenCount":150}}
```

### Response structure per chunk

| Field | Description |
|---|---|
| `candidates[].content.parts[]` | Array of Part objects (text, functionCall, etc.) |
| `candidates[].content.role` | Always `"model"` for generated output |
| `candidates[].index` | Candidate index (0 for single-candidate) |
| `candidates[].safetyRatings[]` | Per-category safety scores; present in every chunk |
| `candidates[].finishReason` | Only in the final chunk |
| `candidates[].citationMetadata` | Source citations when applicable |
| `usageMetadata` | Token counts; typically in the last chunk |

### finishReason values

| Value | Meaning |
|---|---|
| `STOP` | Natural stop or hit a stop sequence |
| `MAX_TOKENS` | Hit `maxOutputTokens` limit |
| `SAFETY` | Blocked by safety filters |
| `RECITATION` | Blocked due to recitation/copyright concern |
| `OTHER` | Other/unspecified reason |

---

## 2. How Gemini Streaming Differs from OpenAI / Anthropic

### Core architectural difference: Parts vs Deltas vs Content Blocks

Gemini uses a **parts-based** response model. Each streamed chunk carries one or
more `Part` objects inside `candidates[].content.parts[]`. A Part can be a text
fragment, a function call, executable code, or other typed content. There is no
explicit delta or patch semantics — each chunk is essentially a partial snapshot
containing new parts to append.

OpenAI uses a **delta model** where each chunk contains a `choices[].delta`
object with only the incremental change (e.g., `{"delta": {"content": "word"}}`).

Anthropic uses **content blocks** with explicit `content_block_start`,
`content_block_delta`, and `content_block_stop` events, giving fine-grained
lifecycle control over each block.

### Safety ratings in every chunk

Gemini includes `safetyRatings` in most streamed chunks, not just the final one.
This means the client can react to safety signals mid-stream (e.g., abort early
if a category probability spikes). OpenAI and Anthropic do not provide per-chunk
safety metadata in the stream itself.

### Citation metadata

Gemini can include `citationMetadata` with source attributions in streamed
chunks. This is baked into the response format rather than requiring a separate
API call.

### Structural comparison table

| Aspect | Gemini | OpenAI | Anthropic |
|---|---|---|---|
| Stream unit | `Part` objects in `candidates[].content.parts[]` | `delta` object in `choices[].delta` | Typed events (`content_block_delta`, etc.) |
| Event framing | SSE (`data: {...}`) | SSE (`data: {...}`) | SSE with explicit event types |
| Safety metadata | Per-chunk `safetyRatings[]` | Not in stream | Not in stream |
| Citations | `citationMetadata` in chunks | Not in stream | Not in stream |
| Finish signal | `finishReason` in last chunk | `finish_reason` in last chunk | `message_stop` event |
| Token counts | `usageMetadata` in last chunk | `usage` in last chunk (opt-in) | `message_delta` with `usage` |
| Function calls | `functionCall` Part | `tool_calls` delta | `tool_use` content block |
| Multiple candidates | `candidates[]` array (index field) | `choices[]` array (index field) | Single response only |

---

## 3. Large Context Streaming (1M+ Tokens)

### Context window sizes

Gemini 1.5 Pro and Gemini 2.5 Pro support up to **1 million tokens** of input
context. Google has previewed 2 million token support. This dwarfs the 200K
limits of Claude and the 128K–200K limits of GPT-4 variants.

### Streaming behaviour with very large contexts

When you send a 1M-token prompt and stream the response:

1. **Time-to-first-token (TTFT)** increases significantly — the model must
   process the entire context before emitting the first output token. For 1M
   tokens, expect TTFT in the range of 30–60+ seconds depending on model and
   load.
2. **Token generation speed** (tokens/sec after first token) is generally
   unaffected by input size — decoding is autoregressive and largely independent
   of prompt length once KV caches are built.
3. **Client-side memory** — if you buffer the response, a 1M-token input likely
   produces a shorter output, but the request payload itself may be very large
   (especially with multimodal content like video frames).

### Prompt caching / context caching

To avoid re-processing the same large context on every request, Gemini offers
**Context Caching**:

```
POST /v1beta/cachedContents
```

```json
{
  "model": "models/gemini-2.5-flash",
  "contents": [
    {
      "role": "user",
      "parts": [{ "text": "<very large document text>" }]
    }
  ],
  "ttl": "3600s"
}
```

The response returns a `cachedContent` resource name. Subsequent
`streamGenerateContent` requests reference it via `cachedContent` field,
skipping prompt re-ingestion and reducing TTFT dramatically.

---

## 4. Multimodal Streaming

### Image + text request (streaming)

```json
{
  "contents": [
    {
      "role": "user",
      "parts": [
        {
          "inlineData": {
            "mimeType": "image/jpeg",
            "data": "<base64-encoded-image>"
          }
        },
        { "text": "Describe what you see in this image." }
      ]
    }
  ]
}
```

The response streams back exactly the same way as a text-only request — chunks
contain `text` Parts. The multimodal processing happens server-side before
streaming begins, adding to TTFT.

### Video understanding

Gemini can process video via the File API. Upload first, then reference:

```json
{
  "contents": [
    {
      "role": "user",
      "parts": [
        {
          "fileData": {
            "mimeType": "video/mp4",
            "fileUri": "https://generativelanguage.googleapis.com/v1beta/files/abc123"
          }
        },
        { "text": "Summarize the key events in this video." }
      ]
    }
  ]
}
```

Video adds significant TTFT because frames must be extracted and encoded into
the model's representation.

### Audio input

Audio files work the same way — upload via the File API or inline as base64:

```json
{
  "parts": [
    {
      "inlineData": {
        "mimeType": "audio/mp3",
        "data": "<base64-encoded-audio>"
      }
    },
    { "text": "Transcribe and summarize this audio." }
  ]
}
```

### Latency impact by modality

| Modality | Approximate TTFT overhead | Notes |
|---|---|---|
| Text only | Baseline | Fastest |
| Single image | +1–3s | Depends on resolution |
| Multiple images | +2–8s | Scales with count |
| Short video (<1 min) | +5–15s | Frame extraction |
| Long video (>10 min) | +20–60s+ | Heavy preprocessing |
| Audio (<5 min) | +3–10s | Depends on length |

---

## 5. Gemini CLI's Streaming Implementation

The open-source Gemini CLI (`github.com/anthropics/...` → actually Google's
`gemini-cli`) separates concerns into two packages:

### Architecture

- **`packages/core/`** — UI-agnostic logic: LLM client, agent loop, tool
  execution, config management. No terminal dependencies.
- **`packages/cli/`** — Ink-based terminal UI: renders markdown, handles user
  input, manages layout.

### Key files and flow

1. **`contentGenerator.ts`** — bridges the agent loop with the LLM client.
   Calls `streamGenerateContent`, yields chunks to the UI layer via an async
   iterator or callback pattern.

2. **`client.ts`** — the SSE handler. Opens an HTTP connection to the Gemini
   API, parses the `data: ` lines, deserializes each JSON chunk into a typed
   `GenerateContentResponse`, and emits them upstream.

3. **`baseLlmClient.ts`** — retry logic. Implements exponential backoff for
   transient errors (429, 503). Wraps the raw client with retry and timeout
   policies.

4. **Fallback module** — model routing logic that tries a primary model and
   falls back to alternatives (e.g., `gemini-2.5-pro` → `gemini-2.5-flash`)
   on capacity errors.

### Headless output modes

The CLI supports non-interactive modes for scripting:

| Mode | Flag | Behaviour |
|---|---|---|
| Text | `--text` | Prints plain text as chunks arrive |
| JSON | `--json` | Buffers entire response, outputs single JSON |
| Stream-JSON | `--stream-json` | Emits one JSON object per chunk (NDJSON) |

Stream-JSON is particularly useful for piping into `jq` or other processors:

```bash
gemini --stream-json "Explain TCP" | jq -r '.text'
```

---

## 6. Function Calling in Streaming Mode

### Declaring tools

```json
{
  "contents": [{ "role": "user", "parts": [{ "text": "What is the weather in London?" }] }],
  "tools": [
    {
      "functionDeclarations": [
        {
          "name": "get_weather",
          "description": "Get current weather for a city",
          "parameters": {
            "type": "OBJECT",
            "properties": {
              "city": { "type": "STRING", "description": "City name" }
            },
            "required": ["city"]
          }
        }
      ]
    }
  ]
}
```

### How function calls appear in the stream

Instead of a `text` Part, the model emits a `functionCall` Part:

```
data: {"candidates":[{"content":{"parts":[{"functionCall":{"name":"get_weather","args":{"city":"London"}}}],"role":"model"},"index":0,"finishReason":"STOP"}]}
```

### Full function calling flow in streaming

```
1. Client  →  streamGenerateContent (with tools declared)
2. Server  ←  SSE chunks... final chunk has functionCall Part
3. Client  :  detects functionCall, executes get_weather("London")
4. Client  →  streamGenerateContent (append functionResponse to contents)
   {
     "contents": [
       { "role": "user", "parts": [{ "text": "What is the weather in London?" }] },
       { "role": "model", "parts": [{ "functionCall": { "name": "get_weather", "args": { "city": "London" } } }] },
       { "role": "function", "parts": [{ "functionResponse": { "name": "get_weather", "response": { "temperature": "12°C", "condition": "Cloudy" } } }] }
     ]
   }
5. Server  ←  SSE chunks with natural language response incorporating the data
```

### Multiple function calls

Gemini can emit multiple `functionCall` Parts in a single response (parallel
tool use). The client should execute all of them, then send all
`functionResponse` Parts back in a single turn.

---

## 7. Thinking in Gemini

### Overview

Gemini 2.5 models (Flash and Pro) support a "thinking" mode where the model
performs internal chain-of-thought reasoning before producing a final answer.
This is conceptually similar to OpenAI's o-series reasoning and Anthropic's
extended thinking.

### Configuring thinking

```json
{
  "contents": [{ "role": "user", "parts": [{ "text": "Solve: 27x³ + 8 = 0" }] }],
  "generationConfig": {
    "thinkingConfig": {
      "thinkingBudget": 8192
    }
  }
}
```

`thinkingBudget` controls how many tokens the model may spend on internal
reasoning (range: 0–24576 depending on model). Setting it to 0 disables
thinking.

### How thinking appears in streaming

When thinking is enabled, streamed chunks may include `thought` Parts before the
final `text` Parts:

```
data: {"candidates":[{"content":{"parts":[{"thought":true,"text":"Let me factor 27x³ + 8..."}],"role":"model"},"index":0}]}

data: {"candidates":[{"content":{"parts":[{"thought":true,"text":"This is a sum of cubes: (3x)³ + 2³ = (3x+2)(9x²-6x+4)..."}],"role":"model"},"index":0}]}

data: {"candidates":[{"content":{"parts":[{"text":"The solutions are x = -2/3, and the complex roots from 9x² - 6x + 4 = 0..."}],"role":"model"},"index":0,"finishReason":"STOP"}]}
```

Parts with `"thought": true` contain the model's internal reasoning. Parts
without the `thought` flag (or with `thought: false`) are the final visible
output.

### Key differences from competitors

| Aspect | Gemini Thinking | OpenAI o-series | Anthropic Extended Thinking |
|---|---|---|---|
| Configuration | `thinkingBudget` in tokens | Implicit (model decides) | `budget_tokens` parameter |
| Visibility | `thought` Parts in stream | Summary only (no raw CoT) | `thinking` content blocks |
| Budget control | 0–24576 tokens | Not user-configurable | 1024–max_tokens |
| Streaming | Thought Parts stream live | Reasoning not streamed | Thinking blocks stream live |

---

## 8. Code Examples

### Python (Google GenAI SDK)

```python
from google import genai

client = genai.Client()

# Simple streaming
for chunk in client.models.generate_content_stream(
    model="gemini-2.5-flash",
    contents="Explain quantum computing in simple terms"
):
    print(chunk.text, end="")
print()  # newline at end

# Streaming with configuration
from google.genai import types

config = types.GenerateContentConfig(
    temperature=0.7,
    max_output_tokens=2048,
    thinking_config=types.ThinkingConfig(thinking_budget=4096),
)

for chunk in client.models.generate_content_stream(
    model="gemini-2.5-flash",
    contents="Write a Python quicksort implementation",
    config=config,
):
    # Separate thinking from output
    for part in chunk.candidates[0].content.parts:
        if getattr(part, "thought", False):
            print(f"[thinking] {part.text}")
        else:
            print(part.text, end="")
```

### TypeScript / JavaScript (Google GenAI SDK)

```typescript
import { GoogleGenAI } from "@google/genai";

const ai = new GoogleGenAI({ apiKey: process.env.GEMINI_API_KEY });

async function streamResponse() {
  const response = await ai.models.generateContentStream({
    model: "gemini-2.5-flash",
    contents: "Explain how TCP works",
  });

  for await (const chunk of response) {
    process.stdout.write(chunk.text() ?? "");
  }
  console.log();

  // Access final usage metadata
  const usage = await response.usageMetadata;
  console.log(`Tokens used: ${usage?.totalTokenCount}`);
}

streamResponse();
```

### Go

```go
package main

import (
"context"
"fmt"
"log"
"os"

"google.golang.org/genai"
)

func main() {
ctx := context.Background()
client, err := genai.NewClient(ctx, &genai.ClientConfig{
APIKey:  os.Getenv("GEMINI_API_KEY"),
Backend: genai.BackendGeminiAPI,
})
if err != nil {
log.Fatal(err)
}

stream := client.Models.GenerateContentStream(
ctx,
"gemini-2.5-flash",
genai.Text("Explain quantum computing"),
nil,
)

for chunk, err := range stream {
if err != nil {
log.Fatal(err)
}
for _, part := range chunk.Candidates[0].Content.Parts {
fmt.Print(part.Text)
}
}
fmt.Println()
}
```

### curl / REST

```bash
curl -s -N \
  "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:streamGenerateContent?alt=sse&key=${GEMINI_API_KEY}" \
  -H "Content-Type: application/json" \
  -d '{
    "contents": [{
      "role": "user",
      "parts": [{"text": "Explain quantum computing"}]
    }],
    "generationConfig": {
      "temperature": 0.7,
      "maxOutputTokens": 1024
    }
  }' | while IFS= read -r line; do
    if [[ "$line" == data:* ]]; then
      echo "$line" | sed 's/^data: //' | jq -r '.candidates[0].content.parts[0].text // empty' 2>/dev/null
    fi
  done
```

---

## 9. Live API (Real-time Streaming)

### Overview

The Live API is a **WebSocket-based** interface for real-time, bidirectional
streaming. Unlike `streamGenerateContent` (which is request → streamed response),
the Live API supports continuous conversation with audio/video input and output.

### Connection

```
wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent?key={API_KEY}
```

### Session setup

First message on the WebSocket sets up the session:

```json
{
  "setup": {
    "model": "models/gemini-2.5-flash-preview-native-audio-dialog",
    "generationConfig": {
      "responseModalities": ["AUDIO"],
      "speechConfig": {
        "voiceConfig": {
          "prebuiltVoiceConfig": { "voiceName": "Kore" }
        }
      }
    },
    "tools": [{ "functionDeclarations": [...] }]
  }
}
```

### Audio streaming

After setup, send audio chunks as real-time input:

```json
{
  "realtimeInput": {
    "mediaChunks": [{
      "mimeType": "audio/pcm;rate=16000",
      "data": "<base64-pcm-audio-chunk>"
    }]
  }
}
```

The server responds with audio output chunks and/or text transcriptions.

### Tool use in Live mode

Function calls work similarly to the REST API — the server sends a
`toolCall` message, the client responds with `toolResponse`, and the
conversation continues without dropping the WebSocket.

### Ephemeral tokens

For browser-based clients, you can generate short-lived tokens to avoid
exposing API keys:

```bash
curl -s "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateEphemeralToken?key=${API_KEY}" \
  -H "Content-Type: application/json" \
  -d '{"config": {"responseModalities": ["TEXT"]}}'
```

The returned token is used in place of the API key for WebSocket connections.

---

## 10. OpenAI Compatibility Mode

### Endpoint

Gemini exposes an OpenAI-compatible endpoint so existing OpenAI SDK code works
with minimal changes:

```
https://generativelanguage.googleapis.com/v1beta/openai/
```

### Using with the OpenAI Python SDK

```python
from openai import OpenAI

client = OpenAI(
    api_key=os.environ["GEMINI_API_KEY"],
    base_url="https://generativelanguage.googleapis.com/v1beta/openai/"
)

stream = client.chat.completions.create(
    model="gemini-2.5-flash",
    messages=[{"role": "user", "content": "Explain streaming APIs"}],
    stream=True,
)

for chunk in stream:
    delta = chunk.choices[0].delta
    if delta.content:
        print(delta.content, end="")
```

### How streaming maps

| OpenAI concept | Gemini native equivalent |
|---|---|
| `choices[].delta.content` | `candidates[].content.parts[].text` |
| `choices[].delta.tool_calls` | `candidates[].content.parts[].functionCall` |
| `choices[].finish_reason` | `candidates[].finishReason` |
| `usage` in final chunk | `usageMetadata` in final chunk |

### Differences from native Gemini streaming

- Safety ratings are **not** included in the OpenAI-compatible stream format
- Citation metadata is **not** mapped
- Thinking/thought Parts are **not** exposed through the compatibility layer
- `usageMetadata` is mapped to OpenAI's `usage` structure
- Some Gemini-specific `finishReason` values (like `RECITATION`) may map to
  generic values

The compatibility layer is useful for quick migration but sacrifices
Gemini-specific features. For full control, use the native API or SDKs.

---

## 11. Comparison Table

| Feature | Gemini | OpenAI | Anthropic |
|---|---|---|---|
| **Stream protocol** | SSE (`alt=sse`) | SSE | SSE with typed events |
| **Response model** | Parts-based | Delta/choices | Content blocks |
| **Max context window** | 1M tokens (2M preview) | 200K (GPT-4.1) | 200K (Claude) |
| **Max output tokens** | 65K (2.5 Pro) | 16K–100K | 64K (with extended) |
| **Multimodal input** | Image, video, audio, PDF | Image, audio | Image, PDF |
| **Multimodal streaming** | Native for all modalities | Image+audio input | Image input only |
| **Real-time/WebSocket** | Live API (bidirectional) | Realtime API | Not available |
| **Thinking/reasoning** | `thinkingConfig` budget | o-series (implicit) | Extended thinking |
| **Thinking visibility** | Full CoT in stream | Summary only | Full CoT in stream |
| **Safety in stream** | Per-chunk ratings | Not in stream | Not in stream |
| **Citations in stream** | Yes | No | No |
| **Function calling** | `functionCall` Parts | `tool_calls` deltas | `tool_use` blocks |
| **Parallel tool calls** | Yes | Yes | Yes |
| **Context caching** | Native API | Not available | Prompt caching (auto) |
| **OpenAI compat layer** | Yes (built-in) | N/A | Not available |
| **Prompt caching** | Explicit cache API | Automatic | Automatic |
| **Batch API** | Yes | Yes | Yes (Message Batches) |

---

## Key Takeaways

1. **Parts-based model** — Gemini's streaming is fundamentally different from
   OpenAI's delta model. Each chunk is a self-contained set of Parts rather
   than an incremental patch.

2. **Safety-first streaming** — per-chunk safety ratings let clients react
   mid-stream, a feature unique to Gemini.

3. **Massive context** — 1M+ token contexts change the streaming UX: TTFT
   becomes the dominant latency. Context caching is essential for repeated
   large-context queries.

4. **Multimodal native** — video, audio, and image input all work with the
   same streaming endpoint; no separate APIs needed.

5. **Live API** — the WebSocket-based Live API enables true real-time
   bidirectional streaming that goes beyond request/response patterns.

6. **Thinking transparency** — unlike OpenAI's o-series, Gemini streams the
   full chain-of-thought reasoning to the client, giving visibility into the
   model's reasoning process.

7. **Migration path** — the OpenAI compatibility layer provides an easy
   on-ramp but sacrifices Gemini-specific features like safety ratings,
   citations, and thinking visibility.
