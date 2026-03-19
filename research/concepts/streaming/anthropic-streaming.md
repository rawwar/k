# Anthropic Streaming Format

Anthropic's Messages API uses a **content-block-oriented** streaming architecture
delivered over Server-Sent Events (SSE). Unlike flat-delta approaches, every piece
of content has an explicit lifecycle: start → deltas → stop. This makes parsing
deterministic and simplifies assembly of complex responses containing mixed text,
tool calls, and thinking blocks.

---

## 1. Content Block Streaming Architecture

### Enabling Streaming

Set `stream: true` in any Messages API request:

```json
POST /v1/messages
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 1024,
  "stream": true,
  "messages": [{"role": "user", "content": "Hello, Claude"}]
}
```

The response uses `Content-Type: text/event-stream` and sends SSE frames.

### SSE Transport

Each frame has a named `event:` line followed by a `data:` line containing JSON:

```
event: message_start
data: {"type":"message_start","message":{...}}

event: ping
data: {"type":"ping"}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{...}}
```

The named event types allow clients to dispatch without parsing the JSON first.

### Event Flow Lifecycle

A complete stream follows this strict ordering:

```
message_start
  ├── ping (dispersed throughout)
  ├── content_block_start   (index 0)
  │     ├── content_block_delta  (one or more)
  │     └── content_block_stop
  ├── content_block_start   (index 1)
  │     ├── content_block_delta  (one or more)
  │     └── content_block_stop
  ├── ... (more content blocks)
  ├── message_delta          (stop_reason, usage)
  └── message_stop
```

#### 1. `message_start`

The first event. Contains a full Message object with empty `content: []`:

```
event: message_start
data: {
  "type": "message_start",
  "message": {
    "id": "msg_01XFDUDYJgAACzvnptvVoYEL",
    "type": "message",
    "role": "assistant",
    "content": [],
    "model": "claude-sonnet-4-20250514",
    "stop_reason": null,
    "stop_sequence": null,
    "usage": {
      "input_tokens": 25,
      "output_tokens": 1,
      "cache_creation_input_tokens": 0,
      "cache_read_input_tokens": 0
    }
  }
}
```

Key fields: `id` for deduplication, `model` for verification, initial `usage`
(input tokens are known upfront; output starts at 1).

#### 2. Content Block Series

Each content block in the response gets three phases:

**`content_block_start`** — declares the block type and index:

```
event: content_block_start
data: {
  "type": "content_block_start",
  "index": 0,
  "content_block": {
    "type": "text",
    "text": ""
  }
}
```

**`content_block_delta`** — incremental content (one or many):

```
event: content_block_delta
data: {
  "type": "content_block_delta",
  "index": 0,
  "delta": {
    "type": "text_delta",
    "text": "Hello"
  }
}
```

**`content_block_stop`** — block is complete:

```
event: content_block_stop
data: {"type": "content_block_stop", "index": 0}
```

Blocks are strictly sequential — index 0 completes before index 1 begins.

#### 3. `message_delta`

Top-level message changes emitted near the end:

```
event: message_delta
data: {
  "type": "message_delta",
  "usage": {"output_tokens": 15},
  "delta": {
    "stop_reason": "end_turn",
    "stop_sequence": null
  }
}
```

The `stop_reason` values include:
- `"end_turn"` — natural completion
- `"max_tokens"` — hit the token limit
- `"stop_sequence"` — matched a stop sequence
- `"tool_use"` — model wants to call a tool

#### 4. `message_stop`

Final event confirming the stream is complete:

```
event: message_stop
data: {"type": "message_stop"}
```

### Ping Events

`ping` events are interspersed throughout for keep-alive:

```
event: ping
data: {"type": "ping"}
```

These prevent proxy/load-balancer timeouts. Clients should ignore them.

### Error Events

Errors can arrive mid-stream:

```
event: error
data: {
  "type": "error",
  "error": {
    "type": "overloaded_error",
    "message": "Overloaded"
  }
}
```

Common error types:
- `overloaded_error` — server is under heavy load
- `api_error` — internal server error
- `rate_limit_error` — too many requests (rare mid-stream)

After an error event, the stream ends. The client must handle partial content.

---

## 2. Content Types

### Text Content

The most common content type. A `text` block streams character-by-character
(or in small chunks):

**`content_block_start`:**
```json
{
  "type": "content_block_start",
  "index": 0,
  "content_block": {"type": "text", "text": ""}
}
```

**`content_block_delta`** (repeated):
```json
{
  "type": "content_block_delta",
  "index": 0,
  "delta": {"type": "text_delta", "text": "Hello"}
}
```
```json
{
  "type": "content_block_delta",
  "index": 0,
  "delta": {"type": "text_delta", "text": "! How can I"}
}
```
```json
{
  "type": "content_block_delta",
  "index": 0,
  "delta": {"type": "text_delta", "text": " help you today?"}
}
```

**`content_block_stop`:**
```json
{"type": "content_block_stop", "index": 0}
```

Assembled result: `"Hello! How can I help you today?"`

Text deltas are variable-length. A single delta might be one character or
several words. Clients should append and flush incrementally.

### Tool Use Content

Tool use blocks stream the JSON arguments as string fragments:

**`content_block_start`** — includes tool name and generated ID:
```json
{
  "type": "content_block_start",
  "index": 1,
  "content_block": {
    "type": "tool_use",
    "id": "toolu_01T1x1fJ34qAmk2tNTrN7Up6",
    "name": "get_weather",
    "input": {}
  }
}
```

**`content_block_delta`** — `input_json_delta` with `partial_json`:
```json
{
  "type": "content_block_delta",
  "index": 1,
  "delta": {
    "type": "input_json_delta",
    "partial_json": "{\"location\": \"San"
  }
}
```
```json
{
  "type": "content_block_delta",
  "index": 1,
  "delta": {
    "type": "input_json_delta",
    "partial_json": " Francisco\","
  }
}
```
```json
{
  "type": "content_block_delta",
  "index": 1,
  "delta": {
    "type": "input_json_delta",
    "partial_json": " \"unit\": \"celsius\"}"
  }
}
```

**`content_block_stop`:**
```json
{"type": "content_block_stop", "index": 1}
```

#### How Tool Use Blocks Build Incrementally

The `partial_json` strings are raw JSON fragments. Key behaviors:

1. **Concatenate all `partial_json` values** to get the complete JSON string
2. **Parse only after `content_block_stop`** for guaranteed validity
3. Current models tend to emit one complete key-value pair at a time, but this
   is not guaranteed — fragments may split mid-value
4. There may be noticeable delays between deltas during tool argument generation
   as the model reasons about parameter values

#### Assembly Strategies

**Manual accumulation + final parse (safest):**
```python
json_parts = []
for event in stream:
    if event.type == "content_block_delta" and event.delta.type == "input_json_delta":
        json_parts.append(event.delta.partial_json)
# After content_block_stop:
tool_input = json.loads("".join(json_parts))
```

**Pydantic partial JSON parsing (progressive):**
Libraries like `partial-json-parser` or Pydantic's `model_validate_json` with
`strict=False` can parse incomplete JSON, letting you show partial tool inputs
to users as they stream.

**SDK helpers:**
The Anthropic SDKs provide `InputJSONEvent` and accumulation methods that handle
the concatenation automatically (see §8).

### Thinking Content

Extended thinking blocks stream the model's chain-of-thought reasoning:

**`content_block_start`:**
```json
{
  "type": "content_block_start",
  "index": 0,
  "content_block": {"type": "thinking", "thinking": ""}
}
```

**`content_block_delta`** — `thinking_delta` events:
```json
{
  "type": "content_block_delta",
  "index": 0,
  "delta": {
    "type": "thinking_delta",
    "thinking": "Let me analyze this step by step.\n\nFirst, I need to"
  }
}
```
```json
{
  "type": "content_block_delta",
  "index": 0,
  "delta": {
    "type": "thinking_delta",
    "thinking": " consider the constraints of the problem..."
  }
}
```

**`signature_delta`** — sent just before `content_block_stop`:
```json
{
  "type": "content_block_delta",
  "index": 0,
  "delta": {
    "type": "signature_delta",
    "signature": "EqQBCgIYAhIM1gbcDa9GJwZA2b..."
  }
}
```

**`content_block_stop`:**
```json
{"type": "content_block_stop", "index": 0}
```

#### Omitted Thinking Display

When thinking is configured with `display: "omitted"`, thinking blocks still
appear but contain no `thinking_delta` events — only a `signature_delta` before
`content_block_stop`. The content is generated internally but not transmitted.

#### Budget Tokens

The `budget_tokens` parameter controls how many tokens the model allocates to
thinking. Higher budgets allow deeper reasoning but increase latency and cost.
Thinking tokens are billed separately from output tokens.

---

## 3. Extended Thinking Deep Dive

### Enabling Extended Thinking

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 16000,
  "thinking": {
    "type": "enabled",
    "budget_tokens": 10000
  },
  "messages": [{"role": "user", "content": "Solve this complex math problem..."}]
}
```

Requirements:
- `budget_tokens` must be less than `max_tokens`
- `temperature` must be 1 (the default) when thinking is enabled
- Not all models support extended thinking

### Thinking Block Ordering

Thinking content blocks always appear **before** text content blocks in the
response. A typical extended-thinking response streams as:

```
message_start
  content_block_start   (index 0, type: thinking)
    thinking_delta ...
    signature_delta
  content_block_stop
  content_block_start   (index 1, type: text)
    text_delta ...
  content_block_stop
  message_delta
  message_stop
```

### Streaming Thinking Progressively

Applications can choose to:
1. **Display thinking live** — show the chain-of-thought as it streams, giving
   users transparency into the model's reasoning process
2. **Hide thinking** — buffer thinking content and only show the final text
   response. Use `display: "omitted"` to avoid even transmitting thinking text.
3. **Toggle display** — let users switch thinking visibility. Claude Code uses
   Ctrl+O to toggle thinking display in the terminal.

### Thinking Signature

The `signature_delta` provides a cryptographic signature for integrity
verification. This allows downstream systems to verify that thinking content
was genuinely produced by the model and hasn't been tampered with. The signature
covers the full thinking text.

### Billing

Thinking tokens count toward output tokens for billing purposes but are tracked
separately in the usage object:

```json
{
  "usage": {
    "input_tokens": 50,
    "output_tokens": 500,
    "cache_creation_input_tokens": 0,
    "cache_read_input_tokens": 0
  }
}
```

The `output_tokens` total includes both thinking and text tokens.

---

## 4. Tool Use Streaming Deep Dive

### Complete Lifecycle of a Tool Use Stream

A response with text followed by a tool call:

```
message_start  (stop_reason: null)

content_block_start  (index 0, type: text, text: "")
  content_block_delta  (text_delta: "Let me check the weather for you.")
content_block_stop   (index 0)

content_block_start  (index 1, type: tool_use, name: "get_weather", id: "toolu_...")
  content_block_delta  (input_json_delta: "{\"location\":")
  content_block_delta  (input_json_delta: " \"New York\"}")
content_block_stop   (index 1)

message_delta  (stop_reason: "tool_use", output_tokens: 42)
message_stop
```

### Multiple Tool Calls

Claude can emit multiple tool_use blocks in a single response. Each gets its
own index and full start/delta/stop lifecycle:

```
content_block_start  (index 0, type: text)
  ... text deltas ...
content_block_stop   (index 0)

content_block_start  (index 1, type: tool_use, name: "search")
  ... input_json_delta deltas ...
content_block_stop   (index 1)

content_block_start  (index 2, type: tool_use, name: "calculate")
  ... input_json_delta deltas ...
content_block_stop   (index 2)
```

### Interleaved Content Blocks

Text and tool_use blocks can interleave. The `index` field identifies which
block each delta belongs to. Since blocks are strictly sequential (never
overlapping), you always know which block is active.

### Eager Input Streaming

The `eager_input_streaming` flag can be set per-tool definition to get tool
input arguments streamed more aggressively:

```json
{
  "tools": [{
    "name": "search",
    "description": "Search the web",
    "input_schema": {...},
    "eager_input_streaming": true
  }]
}
```

With eager streaming enabled, the model sends partial JSON more frequently
and with smaller chunks. This is useful when you want to show users partial
tool inputs in real-time (e.g., showing a search query as it forms).

### Partial JSON Parsing Strategies

For progressive display of tool inputs:

1. **Best-effort parse** — try `json.loads()` on accumulated string. If it
   fails, show raw text. Works well since models usually emit complete k/v pairs.

2. **Streaming JSON parser** — libraries that handle incomplete JSON:
   ```python
   from partial_json_parser import loads as partial_loads
   partial_result = partial_loads('{"query": "weather in')
   # Returns: {"query": "weather in"}
   ```

3. **Schema-aware parsing** — use the tool's input_schema to validate partial
   results and provide type-safe access to streamed fields.

### Comparison with OpenAI's Approach

**Anthropic:** Explicit content block lifecycle
```
content_block_start  → identifies block type, index, metadata
content_block_delta  → incremental content within that block
content_block_stop   → block is complete
```

**OpenAI Chat Completions:** Index-based flat deltas
```json
{"choices": [{"delta": {"tool_calls": [{"index": 0, "function": {"arguments": "..."}}]}}]}
```

**Key differences:**
- Anthropic's explicit start/stop events provide clear boundaries
- OpenAI uses implicit start (first appearance of an index) and no explicit stop
- Anthropic's block model is simpler when content types are mixed
- OpenAI allows overlapping argument streaming across tool call indices
- Anthropic's delta types are named (`text_delta`, `input_json_delta`), making
  dispatch straightforward

---

## 5. Error Recovery

### Capturing Partial Response

When a stream errors mid-way, the client should preserve all content blocks
received so far. Fully completed blocks (those with `content_block_stop`) are
safe to use.

### Constructing a Continuation Request

To resume after an error, include the partial assistant response in the next
request:

```json
{
  "messages": [
    {"role": "user", "content": "Write a long essay..."},
    {"role": "assistant", "content": [
      {"type": "text", "text": "The partial text received so far..."}
    ]},
    {"role": "user", "content": "Please continue from where you left off."}
  ]
}
```

### Model-Specific Behavior

- **Claude 4.5 Sonnet and earlier:** You can pass the partial assistant message
  and the model will continue generating from that point
- **Claude 4.6 (Sonnet/Opus):** You must add a user message after the partial
  assistant content to prompt continuation — the model won't auto-continue from
  a trailing assistant message alone

### Limitations

- **Tool use blocks cannot be partially recovered.** If a `tool_use` block was
  mid-stream when the error occurred (no `content_block_stop`), discard it
- **Thinking blocks cannot be partially recovered.** Discard incomplete thinking
  blocks — the signature won't be valid
- **Resume from the most recent complete text block.** Include all fully-stopped
  content blocks in the assistant message for continuation

---

## 6. Code Examples

### Python — Basic Streaming

```python
import anthropic

client = anthropic.Anthropic()

with client.messages.stream(
    model="claude-sonnet-4-20250514",
    max_tokens=1024,
    messages=[{"role": "user", "content": "Explain quantum computing briefly"}],
) as stream:
    for text in stream.text_stream:
        print(text, end="", flush=True)

print()  # newline after stream completes
```

### Python — Event-Based Streaming

```python
import anthropic

client = anthropic.Anthropic()

with client.messages.stream(
    model="claude-sonnet-4-20250514",
    max_tokens=1024,
    messages=[{"role": "user", "content": "Hello"}],
) as stream:
    for event in stream:
        match event.type:
            case "message_start":
                print(f"Message ID: {event.message.id}")
            case "content_block_start":
                print(f"\n[Block {event.index}: {event.content_block.type}]")
            case "content_block_delta":
                if event.delta.type == "text_delta":
                    print(event.delta.text, end="", flush=True)
                elif event.delta.type == "input_json_delta":
                    print(f"[json] {event.delta.partial_json}", end="")
                elif event.delta.type == "thinking_delta":
                    print(f"[think] {event.delta.thinking}", end="")
            case "content_block_stop":
                print(f"\n[Block {event.index} complete]")
            case "message_delta":
                print(f"\nStop reason: {event.delta.stop_reason}")
                print(f"Output tokens: {event.usage.output_tokens}")
            case "message_stop":
                print("Stream complete.")
```

### Python — Tool Use Streaming

```python
import json
import anthropic

client = anthropic.Anthropic()

tools = [{
    "name": "get_weather",
    "description": "Get current weather for a location",
    "input_schema": {
        "type": "object",
        "properties": {
            "location": {"type": "string", "description": "City name"},
            "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]}
        },
        "required": ["location"]
    }
}]

# Track tool use blocks
tool_inputs = {}  # index -> accumulated JSON string

with client.messages.stream(
    model="claude-sonnet-4-20250514",
    max_tokens=1024,
    tools=tools,
    messages=[{"role": "user", "content": "What's the weather in Tokyo?"}],
) as stream:
    for event in stream:
        if event.type == "content_block_start":
            if event.content_block.type == "tool_use":
                tool_inputs[event.index] = {
                    "id": event.content_block.id,
                    "name": event.content_block.name,
                    "json_parts": []
                }
        elif event.type == "content_block_delta":
            if event.delta.type == "input_json_delta":
                tool_inputs[event.index]["json_parts"].append(
                    event.delta.partial_json
                )
        elif event.type == "content_block_stop":
            if event.index in tool_inputs:
                tool = tool_inputs[event.index]
                raw = "".join(tool["json_parts"])
                parsed = json.loads(raw)
                print(f"Tool call: {tool['name']}({parsed})")
```

### Python — Extended Thinking

```python
import anthropic

client = anthropic.Anthropic()

with client.messages.stream(
    model="claude-sonnet-4-20250514",
    max_tokens=16000,
    thinking={
        "type": "enabled",
        "budget_tokens": 10000,
    },
    messages=[{"role": "user", "content": "What is 27 * 453?"}],
) as stream:
    current_block_type = None
    for event in stream:
        if event.type == "content_block_start":
            current_block_type = event.content_block.type
            if current_block_type == "thinking":
                print("--- Thinking ---")
            elif current_block_type == "text":
                print("\n--- Response ---")
        elif event.type == "content_block_delta":
            if event.delta.type == "thinking_delta":
                print(event.delta.thinking, end="", flush=True)
            elif event.delta.type == "text_delta":
                print(event.delta.text, end="", flush=True)
            elif event.delta.type == "signature_delta":
                pass  # signature for integrity verification
        elif event.type == "content_block_stop":
            print()
```

### TypeScript — Basic Streaming

```typescript
import Anthropic from "@anthropic-ai/sdk";

const client = new Anthropic();

const stream = client.messages.stream({
  model: "claude-sonnet-4-20250514",
  max_tokens: 1024,
  messages: [{ role: "user", content: "Hello" }],
});

stream.on("text", (text) => {
  process.stdout.write(text);
});

const finalMessage = await stream.finalMessage();
console.log("\nTokens used:", finalMessage.usage.output_tokens);
```

### TypeScript — Event-Based Streaming

```typescript
import Anthropic from "@anthropic-ai/sdk";

const client = new Anthropic();

const stream = client.messages.stream({
  model: "claude-sonnet-4-20250514",
  max_tokens: 1024,
  messages: [{ role: "user", content: "Hello" }],
});

stream.on("message_start", (event) => {
  console.log("Message ID:", event.message.id);
});

stream.on("contentBlockStart", (event) => {
  console.log(`Block ${event.index}: ${event.contentBlock.type}`);
});

stream.on("contentBlockDelta", (event) => {
  if (event.delta.type === "text_delta") {
    process.stdout.write(event.delta.text);
  } else if (event.delta.type === "input_json_delta") {
    process.stdout.write(event.delta.partial_json);
  }
});

stream.on("contentBlockStop", (event) => {
  console.log(`\nBlock ${event.index} complete`);
});

const message = await stream.finalMessage();
console.log("Stop reason:", message.stop_reason);
```

### TypeScript — Tool Use with Accumulation

```typescript
import Anthropic from "@anthropic-ai/sdk";

const client = new Anthropic();

const stream = client.messages.stream({
  model: "claude-sonnet-4-20250514",
  max_tokens: 1024,
  tools: [
    {
      name: "get_weather",
      description: "Get weather for a location",
      input_schema: {
        type: "object" as const,
        properties: {
          location: { type: "string" },
        },
        required: ["location"],
      },
    },
  ],
  messages: [{ role: "user", content: "Weather in Paris?" }],
});

// The SDK accumulates tool inputs automatically
const message = await stream.finalMessage();

for (const block of message.content) {
  if (block.type === "tool_use") {
    console.log(`Tool: ${block.name}, Input:`, block.input);
  }
}
```

### Raw HTTP / curl Example

```bash
curl https://api.anthropic.com/v1/messages \
  -H "Content-Type: application/json" \
  -H "x-api-key: $ANTHROPIC_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -d '{
    "model": "claude-sonnet-4-20250514",
    "max_tokens": 256,
    "stream": true,
    "messages": [{"role": "user", "content": "Hi"}]
  }'
```

Raw SSE response:

```
event: message_start
data: {"type":"message_start","message":{"id":"msg_01Ab...","type":"message","role":"assistant","content":[],"model":"claude-sonnet-4-20250514","stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":10,"output_tokens":1,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: ping
data: {"type":"ping"}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"! How can"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" I help you today?"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","usage":{"output_tokens":12},"delta":{"stop_reason":"end_turn","stop_sequence":null}}

event: message_stop
data: {"type":"message_stop"}
```

---

## 7. Comparison with OpenAI

| Feature | Anthropic Messages | OpenAI Chat Completions | OpenAI Responses API |
|---|---|---|---|
| **Transport** | SSE with named events | SSE with `data:` lines | SSE with named events |
| **Event model** | Content block lifecycle | Flat choice deltas | Item lifecycle events |
| **Text streaming** | `text_delta` in blocks | `delta.content` string | `output_text.delta` |
| **Tool call boundary** | Explicit start/stop | Index-based (implicit) | Item start/done events |
| **Tool arg format** | `partial_json` string | `arguments` string chunks | `arguments.delta` |
| **Multiple tool calls** | Sequential blocks | Parallel via index array | Parallel items |
| **Thinking/reasoning** | `thinking` blocks | `reasoning` items (o-series) | `reasoning` items |
| **Thinking signature** | `signature_delta` | Not applicable | Not applicable |
| **Reconnection** | Manual resume w/ partial msg | Manual retry | Built-in `after` cursor |
| **Error events** | In-stream `error` event | HTTP-level errors | In-stream `error` event |
| **Keep-alive** | `ping` events | Comment lines (`: OPENAI`) | Comment lines |
| **Content indexing** | Block index (sequential) | Choice index + tool index | Item ID based |
| **Final message** | SDK `get_final_message()` | SDK `.finalChatCompletion()` | `response.completed` |

### Key Architectural Differences

**Anthropic's block model** is explicitly scoped: every block has a declared
start and end. You always know when a tool call is complete because you get
`content_block_stop`. Mixed content (text + tools + thinking) is easy to parse
because each segment has its own index and lifecycle.

**OpenAI Chat Completions** uses a flatter model where tool calls are identified
by index within a delta array. There's no explicit "stop" event for individual
tool calls — completion is inferred when the stream ends or a new index appears.
This is more compact but requires more client-side state tracking.

**OpenAI Responses API** adopts an item-based model closer to Anthropic's
approach, with explicit item lifecycle events and support for reconnection via
the `after` cursor parameter.

---

## 8. SDK Helpers

### Python SDK

The Python SDK provides high-level streaming abstractions:

```python
# MessageStream context manager
with client.messages.stream(...) as stream:
    # Iterate text only
    for text in stream.text_stream:
        print(text, end="")

    # Or iterate all events
    for event in stream:
        ...

    # Get the fully assembled message at the end
    message = stream.get_final_message()
    # message.content contains fully assembled content blocks

    # Get final text only
    text = stream.get_final_text()
```

Key methods:
- `.text_stream` — yields only text deltas as strings
- `.get_final_message()` — returns complete `Message` object
- `.get_final_text()` — returns concatenated text content

### TypeScript SDK

```typescript
const stream = client.messages.stream({...});

// Event-based
stream.on("text", (text) => process.stdout.write(text));
stream.on("inputJson", (json, snapshot) => { /* partial tool input */ });

// Promise-based
const message = await stream.finalMessage();
const text = await stream.finalText();

// Async iteration
for await (const event of stream) {
  // raw SSE events
}
```

### Go SDK

```go
stream := client.Messages.New(ctx, params)
for stream.Next() {
    event := stream.Current()
    accumulated := message.Accumulate(event)
    // accumulated is the full message so far
}
if err := stream.Err(); err != nil {
    log.Fatal(err)
}
finalMessage := stream.Message // fully assembled
```

The `message.Accumulate(event)` helper incrementally builds the complete message
object as events arrive, handling all content block types.

### Java SDK

```java
MessageAccumulator accumulator = MessageAccumulator.create();
client.messages().createStreaming(params).stream()
    .peek(accumulator::accumulate)
    .forEach(event -> {
        if (event instanceof ContentBlockDeltaEvent delta) {
            if (delta.delta() instanceof TextDelta text) {
                System.out.print(text.text());
            }
        }
    });
Message finalMessage = accumulator.message();
```

### Ruby SDK

```ruby
stream = client.messages.create_streaming(
  model: "claude-sonnet-4-20250514",
  max_tokens: 1024,
  messages: [{ role: "user", content: "Hello" }]
)

stream.each do |event|
  case event
  when Anthropic::ContentBlockDelta
    print event.delta.text if event.delta.is_a?(Anthropic::TextDelta)
  end
end

# Get fully assembled message
message = stream.accumulated_message
```

All SDK helpers follow the same pattern: they internally track content block
state, accumulate deltas, and provide convenience accessors for the assembled
result. This frees application code from manual JSON concatenation, block
tracking, and delta type dispatch.
