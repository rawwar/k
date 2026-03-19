# Incremental Parsing

Deep-dive into processing partial/incremental data from streaming LLM responses.
When an LLM streams tokens, downstream code must assemble coherent structures
(JSON, Markdown, tool calls) from an unpredictable trickle of fragments. This
document covers the strategies, trade-offs, and real-world code patterns used by
modern AI agents and SDKs.

---

## 1. The Fundamental Challenge

LLM tokens arrive one at a time, typically at 50–100 tokens per second. Each
Server-Sent Event (SSE) chunk carries a small delta—often a single word, a
partial JSON key, or half of a Markdown fence. The parser must cope with:

- **Arbitrary byte boundaries** — a multi-byte UTF-8 character can be split
  across two TCP segments; a JSON string value can arrive one character at a
  time.
- **Partial JSON** — tool call arguments are delivered as an incrementally
  growing string. You never know when the closing `}` will appear.
- **Context-dependent markup** — Markdown backtick fences, bold markers, and
  headings only make sense once their closing tokens arrive.
- **Interleaved content types** — a single response can mix plain text, tool
  calls, thinking tokens, and citations. The parser must demultiplex.
- **No backtracking** — once a token is emitted to the user, you cannot un-show
  it. Premature rendering of incomplete Markdown is a common UX bug.

Understanding these constraints is essential before choosing a parsing strategy.

---

## 2. Partial JSON Assembly

JSON is the lingua franca between LLMs and tool systems. Every function-calling
API (OpenAI, Anthropic, Google) delivers tool arguments as a stream of JSON
string fragments. Correctly reassembling those fragments is the single most
important incremental parsing problem.

### 2.1 The "Open Brace" Problem

A naïve first instinct: count braces.

```
received so far: {"file": "src/ma
```

Is this valid JSON? No. Can we tell when it *will* be? Only when every `{` has
a matching `}` and every `"` has a matching `"`. But brace counting is fragile:

```json
{"query": "find all { in the codebase"}
```

The `{` inside the string value is not structural. A simple counter would think
the object is still open. Escaped quotes make it worse:

```json
{"pattern": "he said \"hello\""}
```

A parser that flips a "in-string" flag on every `"` will be fooled by `\"`.
**Naïve brace counting is therefore unreliable.** Production systems avoid it.

### 2.2 Accumulate-and-Parse Strategy

The most common approach, used by OpenCode, Goose, Codex, and the majority of
agent frameworks:

1. Maintain a string buffer per tool call.
2. Append each `arguments` delta to the buffer.
3. **Do not attempt to parse until the stream signals completion.**
4. On completion, run `JSON.parse` / `json.loads` / `json.Unmarshal` once.

This is simple, correct, and fast. The downside: you cannot show the user
partial tool call parameters while streaming.

**Python example (OpenAI chat completions):**

```python
import json
from openai import OpenAI

client = OpenAI()
stream = client.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "Read the file src/main.py"}],
    tools=[{"type": "function", "function": {"name": "read_file", "parameters": {...}}}],
    stream=True,
)

accumulated_args: dict[int, str] = {}   # keyed by tool_call index
tool_meta: dict[int, dict] = {}         # id + name per index

for chunk in stream:
    delta = chunk.choices[0].delta
    if delta.tool_calls:
        for tc in delta.tool_calls:
            idx = tc.index
            if idx not in tool_meta:
                tool_meta[idx] = {"id": tc.id, "name": tc.function.name}
                accumulated_args[idx] = ""
            if tc.function.arguments:
                accumulated_args[idx] += tc.function.arguments

# Parse only after stream is done
for idx, raw in accumulated_args.items():
    args = json.loads(raw)
    print(f"Tool {tool_meta[idx]['name']}: {args}")
```

**Go example (accumulate into strings.Builder):**

```go
var argsBuf strings.Builder

for {
    chunk, err := stream.Recv()
    if errors.Is(err, io.EOF) {
        break
    }
    for _, tc := range chunk.Choices[0].Delta.ToolCalls {
        argsBuf.WriteString(tc.Function.Arguments)
    }
}

var args map[string]any
json.Unmarshal([]byte(argsBuf.String()), &args)
```

### 2.3 Streaming JSON Parsers

When you *do* need to process JSON incrementally—for example, to show partial
tool parameters in a UI—dedicated streaming parsers are required.

#### Jiter (Rust)

Jiter is a fast, iterable JSON parser written in Rust. Key properties:

- **4–10× faster** than `serde_json` for many workloads.
- **Iterator mode**: process key-value pairs as they arrive without building a
  full DOM.
- **Zero-copy parsing**: borrows from the input buffer rather than allocating
  new strings.
- Used internally by **Pydantic V2**, which means it underlies the OpenAI SDK,
  Anthropic SDK, and LiteLLM when they validate response objects.

```rust
use jiter::{Jiter, Peek};

let data = br#"{"name": "read_file", "path": "src/main.py"}"#;
let mut jiter = Jiter::new(data);

assert_eq!(jiter.next_object().unwrap().unwrap(), "name");
assert_eq!(jiter.next_str().unwrap(), "read_file");
assert_eq!(jiter.next_key().unwrap().unwrap(), "path");
assert_eq!(jiter.next_str().unwrap(), "src/main.py");
assert!(jiter.next_key().unwrap().is_none()); // end of object
```

#### @streamparser/json (TypeScript)

A SAX/pull-based streaming parser for incomplete JSON:

```typescript
import { JSONParser } from "@streamparser/json";

const parser = new JSONParser();
parser.onValue = ({ value, key, parent, stack }) => {
  console.log(`Key: ${key}, Value: ${JSON.stringify(value)}`);
};

// Feed partial chunks as they arrive
parser.write('{"na');
parser.write('me": "rea');
parser.write('d_file", "path"');
parser.write(': "src/main.py"}');
```

Each `onValue` callback fires as soon as a complete value is available, even
though the overall JSON object is still incomplete.

#### partial-json-parser (TypeScript)

Parses intentionally incomplete JSON—useful for showing partial tool call
arguments to the user while the stream is still in progress:

```typescript
import { parsePartialJson } from "partial-json-parser";

const incomplete = '{"file": "src/main.py", "line';
const result = parsePartialJson(incomplete);
// => { file: "src/main.py" }
// The incomplete "line" key is silently dropped
```

#### encoding/json.Decoder (Go)

Go's standard library includes a streaming JSON decoder that reads tokens from
any `io.Reader`:

```go
dec := json.NewDecoder(responseBody)
for dec.More() {
    tok, err := dec.Token()
    if err != nil {
        break
    }
    switch v := tok.(type) {
    case string:
        fmt.Println("string:", v)
    case json.Delim:
        fmt.Println("delim:", v) // {, }, [, ]
    }
}
```

#### serde_json::StreamDeserializer (Rust)

Iterates over a stream of concatenated JSON values:

```rust
use serde_json::{Deserializer, Value};

let data = r#"{"a":1}{"b":2}{"c":3}"#;
let stream = Deserializer::from_str(data).into_iter::<Value>();
for value in stream {
    println!("{}", value.unwrap());
}
```

### 2.4 Partial JSON Parsing with Pydantic

Pydantic V2 has experimental support for partial/lenient JSON parsing, powered
by Jiter under the hood:

```python
from pydantic import BaseModel

class ToolArgs(BaseModel):
    file: str | None = None
    line: int | None = None

# Imagine this is the accumulated (but incomplete) JSON so far
partial = '{"file": "src/main.py", "line'
try:
    args = ToolArgs.model_validate_json(partial)
except Exception:
    # Falls back gracefully—file is populated, line is None
    pass
```

This is useful for preview UIs that display partial tool parameters during
streaming.

---

## 3. Tool Call Reconstruction

Each LLM provider has a different wire format for streaming tool calls. The
parser must understand the provider's specific event lifecycle.

### 3.1 OpenAI Chat Completions: Index-Based Delta Accumulation

OpenAI's chat completion stream uses an **index-based** approach for tool calls:

- The first chunk for a tool call carries: `index`, `id`, `function.name`.
- Subsequent chunks carry: `index`, `function.arguments` (a fragment).
- When the model invokes multiple tools in parallel, chunks for different
  indices are interleaved.

**Complete reconstruction example (Python):**

```python
from openai import OpenAI
import json

client = OpenAI()
stream = client.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "Check the weather in NYC and London"}],
    tools=[
        {"type": "function", "function": {
            "name": "get_weather",
            "parameters": {
                "type": "object",
                "properties": {"city": {"type": "string"}},
            },
        }},
    ],
    stream=True,
)

tool_calls: dict[int, dict] = {}

for chunk in stream:
    choice = chunk.choices[0]
    if choice.delta.tool_calls:
        for tc_delta in choice.delta.tool_calls:
            idx = tc_delta.index

            # First appearance — initialize
            if idx not in tool_calls:
                tool_calls[idx] = {
                    "id": tc_delta.id,
                    "name": tc_delta.function.name,
                    "arguments": "",
                }

            # Accumulate argument fragments
            if tc_delta.function.arguments:
                tool_calls[idx]["arguments"] += tc_delta.function.arguments

    # Detect stream end
    if choice.finish_reason == "tool_calls":
        break

# Now parse each tool call's arguments
for idx in sorted(tool_calls):
    tc = tool_calls[idx]
    args = json.loads(tc["arguments"])
    print(f"Call {idx}: {tc['name']}({args})")
    # Call 0: get_weather({'city': 'NYC'})
    # Call 1: get_weather({'city': 'London'})
```

### 3.2 Anthropic: Content Block Lifecycle

Anthropic uses an explicit **content block lifecycle** with clear start/stop
boundaries:

| Event                  | Payload                                        |
|------------------------|------------------------------------------------|
| `content_block_start`  | `{type: "tool_use", id: "...", name: "..."}`   |
| `content_block_delta`  | `{type: "input_json_delta", partial_json: "…"}` |
| `content_block_stop`   | (empty — signals safe to parse)                |

**Python example (Anthropic SDK):**

```python
import anthropic
import json

client = anthropic.Anthropic()

tool_calls = {}
current_block_idx = None

with client.messages.stream(
    model="claude-sonnet-4-20250514",
    max_tokens=1024,
    tools=[{
        "name": "read_file",
        "description": "Read a file",
        "input_schema": {
            "type": "object",
            "properties": {"path": {"type": "string"}},
        },
    }],
    messages=[{"role": "user", "content": "Read src/main.py"}],
) as stream:
    for event in stream:
        if event.type == "content_block_start":
            if event.content_block.type == "tool_use":
                idx = event.index
                current_block_idx = idx
                tool_calls[idx] = {
                    "id": event.content_block.id,
                    "name": event.content_block.name,
                    "arguments": "",
                }

        elif event.type == "content_block_delta":
            if event.delta.type == "input_json_delta":
                tool_calls[current_block_idx]["arguments"] += event.delta.partial_json

        elif event.type == "content_block_stop":
            if current_block_idx in tool_calls:
                tc = tool_calls[current_block_idx]
                tc["parsed"] = json.loads(tc["arguments"])
                print(f"Tool ready: {tc['name']}({tc['parsed']})")
```

The key advantage of Anthropic's approach: `content_block_stop` gives you an
**explicit signal** that it is safe to parse. You never have to guess.

### 3.3 OpenAI Responses API: Item Lifecycle

The newer Responses API uses a richer event model:

```
response.output_item.added       → item shell (type, id)
response.function_call_arguments.delta  → argument fragment
response.function_call_arguments.done   → full arguments string
response.output_item.done        → item finalized
```

**TypeScript example:**

```typescript
import OpenAI from "openai";

const client = new OpenAI();
const stream = await client.responses.create({
  model: "gpt-4o",
  input: "What's the weather in Tokyo?",
  tools: [{ type: "function", function: { name: "get_weather", parameters: { type: "object", properties: { city: { type: "string" } } } } }],
  stream: true,
});

const items: Record<string, { name: string; arguments: string }> = {};

for await (const event of stream) {
  switch (event.type) {
    case "response.output_item.added":
      if (event.item.type === "function_call") {
        items[event.item.id] = { name: event.item.name, arguments: "" };
      }
      break;

    case "response.function_call_arguments.delta":
      items[event.item_id].arguments += event.delta;
      break;

    case "response.function_call_arguments.done":
      const item = items[event.item_id];
      const args = JSON.parse(item.arguments);
      console.log(`${item.name}(${JSON.stringify(args)})`);
      break;
  }
}
```

### 3.4 Handling Multiple Parallel Tool Calls

When the model invokes 3+ tools simultaneously, chunks for different tool calls
arrive interleaved:

```
chunk: tool_calls[0].function.arguments = '{"ci'
chunk: tool_calls[1].function.arguments = '{"ci'
chunk: tool_calls[0].function.arguments = 'ty": "NYC"}'
chunk: tool_calls[2].function.arguments = '{"ci'
chunk: tool_calls[1].function.arguments = 'ty": "London"}'
chunk: tool_calls[2].function.arguments = 'ty": "Tokyo"}'
```

**State machine approach:** each tool call is tracked independently via its
index (OpenAI) or content block index (Anthropic). The parser maintains a map
of accumulators:

```python
# State machine: each tool call has its own state
class ToolCallState:
    PENDING = "pending"       # seen start, no args yet
    ACCUMULATING = "accumulating"  # receiving argument fragments
    COMPLETE = "complete"     # finish signal received

accumulators: dict[int, ToolCallState] = {}
```

**Agent strategies observed in the wild:**

| Agent    | Strategy                                              |
|----------|-------------------------------------------------------|
| OpenCode | Per-tool accumulator keyed by index/ID                |
| Codex    | Item-based tracking via Responses API lifecycle       |
| Goose    | `categorize_tools()` after stream completes—no live parsing |
| Aider    | Accumulate all, parse at end, model-specific routing  |

---

## 4. Content Buffering Strategies

How you buffer and flush content to the display determines the user experience.

### 4.1 Character-by-Character (Immediate Flush)

Print each token the instant it arrives:

```python
for chunk in stream:
    text = chunk.choices[0].delta.content
    if text:
        sys.stdout.write(text)
        sys.stdout.flush()
```

- **Pros:** lowest latency, simplest code.
- **Cons:** flicker, no Markdown rendering, breaks mid-word formatting.

### 4.2 Line Buffering

Buffer until a newline appears, then flush the whole line:

```python
line_buffer = ""
for chunk in stream:
    text = chunk.choices[0].delta.content or ""
    line_buffer += text
    while "\n" in line_buffer:
        line, line_buffer = line_buffer.split("\n", 1)
        render_line(line)
```

Better for code output where lines are meaningful units, but still no Markdown
block awareness.

### 4.3 Semantic Buffering

Wait for semantically complete units before rendering:

```python
buffer = ""
for chunk in stream:
    buffer += chunk.choices[0].delta.content or ""

    # Flush complete sentences
    if re.search(r'[.!?]\s', buffer):
        idx = re.search(r'[.!?]\s', buffer).end()
        render(buffer[:idx])
        buffer = buffer[idx:]

    # Flush complete code blocks
    if buffer.count("```") >= 2 and buffer.count("```") % 2 == 0:
        render(buffer)
        buffer = ""
```

This is the sweet spot for most agent UIs: tolerate a small latency penalty in
exchange for properly rendered Markdown.

### 4.4 Differential Rendering

Only re-render the portions that changed. This is how Pi Coding Agent handles
its rich terminal UI:

```typescript
// Pseudocode: differential terminal update
let previousRender = "";

function onNewTokens(fullText: string) {
  const newRender = renderMarkdown(fullText);
  const diff = computeDiff(previousRender, newRender);
  applyDiffToTerminal(diff);
  previousRender = newRender;
}
```

This avoids the "full repaint flicker" that plagues naive approaches.

### 4.5 React Reconciliation (Ink-based Agents)

Agents built with Ink (React for terminals) leverage React's reconciliation
algorithm. The key primitive is `<Static>`:

```tsx
import { render, Static, Box, Text } from "ink";

function StreamingUI({ messages }) {
  const finalized = messages.slice(0, -1);
  const current = messages[messages.length - 1];

  return (
    <>
      {/* Finalized messages never re-render */}
      <Static items={finalized}>
        {(msg) => <Text key={msg.id}>{msg.content}</Text>}
      </Static>

      {/* Only the current streaming message re-renders */}
      <Box>
        <Text>{current.content}</Text>
      </Box>
    </>
  );
}
```

Ink's yoga layout engine provides flexbox-in-terminal, and React's diffing
ensures only changed components re-render.

---

## 5. Text Content Assembly

For plain text deltas, assembly is straightforward concatenation—with caveats.

### 5.1 Unicode Multi-Byte Characters

UTF-8 characters can be 1–4 bytes. If a TCP segment boundary falls in the
middle of a multi-byte character, the SSE parser may deliver a partial
character:

```python
# Chunk 1: b'\xc3'       (first byte of 'ã')
# Chunk 2: b'\xa3 hello'  (second byte + more text)
```

**Mitigation:** work at the string level (after SSE line decoding), not the
byte level. Most SSE libraries handle this, but raw `fetch` + manual line
splitting can hit this bug.

### 5.2 Emoji and Grapheme Clusters

Some emoji are composed of multiple Unicode code points joined by Zero-Width
Joiners (ZWJ):

```
👨‍👩‍👧‍👦 = U+1F468 U+200D U+1F469 U+200D U+1F467 U+200D U+1F466
```

If the LLM tokenizer splits this across chunks, the terminal may briefly
display partial emoji. There is no clean fix; agents generally accept this
transient artifact.

### 5.3 Encoding Edge Cases

- **SSE spec mandates UTF-8.** No other encoding is valid.
- **BOM (Byte Order Mark):** some proxies prepend `\xEF\xBB\xBF`. Strip it.
- **Null bytes:** some models occasionally emit `\x00`. Filter them out.

---

## 6. Thinking Token Processing

Extended thinking / chain-of-thought tokens require special handling.

### 6.1 Anthropic Thinking Blocks

Anthropic delivers thinking as separate content blocks:

```python
for event in stream:
    if event.type == "content_block_start":
        if event.content_block.type == "thinking":
            print("--- thinking ---")
    elif event.type == "content_block_delta":
        if event.delta.type == "thinking_delta":
            # Thinking text fragment
            display_thinking(event.delta.thinking)
        elif event.delta.type == "text_delta":
            # Regular text fragment
            display_text(event.delta.text)
```

### 6.2 OpenAI Reasoning Items

In the Responses API, reasoning is a separate output item:

```typescript
for await (const event of stream) {
  if (event.type === "response.reasoning.delta") {
    // Reasoning text fragment (o1, o3 models)
    showThinking(event.delta);
  }
}
```

### 6.3 Display Decisions

Agents differ on whether to show thinking:

| Agent    | Thinking Display                          |
|----------|-------------------------------------------|
| Claude Code | Shows in collapsed section             |
| Codex    | Hidden by default, `--full-reasoning` flag |
| OpenCode | Renders in dimmed text                    |
| Goose    | Shows as separate "Thinking" block        |

---

## 7. Error Cases

### 7.1 Malformed Chunks

Occasionally the SSE data field contains invalid JSON:

```
data: {"choices": [{... truncated
```

**Strategies:**
- **Skip and log:** most agents discard the chunk and continue.
- **Retry full request:** if critical data was lost.
- **OpenCode's approach:** finalizes the response with `FinishReasonCanceled`
  and surfaces the error to the user.

### 7.2 Interrupted Streams

A connection drop mid-stream leaves partial data in buffers:

```python
accumulated_args = '{"file": "src/main.py", "line_numbe'
# Connection lost — this is not valid JSON
```

**Recovery strategies:**
- **Discard and retry:** safest option. Re-send the request.
- **Attempt partial parse:** use `partial-json-parser` to salvage what you can.
- **Goose's approach:** `compact_messages()` rewrites conversation history when
  context overflows, gracefully handling interrupted responses.

### 7.3 Encoding Issues

- SSE is strictly UTF-8. Binary data (e.g., base64 images sent raw) can cause
  parse failures.
- Agents skip offending chunks or strip non-UTF-8 bytes.

### 7.4 Out-of-Order Events

While rare (SSE over HTTP/2 should be ordered), network middleboxes can
theoretically reorder:

- **Codex:** buffers updates for unknown item IDs, replaying them when the
  corresponding `item.added` event arrives.
- **Most agents:** assume strict ordering and break gracefully if violated.

### 7.5 Provider-Specific Quirks

- **Gemini:** non-standard tool call format; wraps function calls differently
  than OpenAI/Anthropic.
- **Aider:** implements model-specific routing (`model.send_completion()`) as a
  workaround for provider differences.
- **Ollama:** sometimes sends empty delta chunks; parsers must tolerate them.

---

## 8. Code Examples

### 8.1 Resilient Streaming Parser (Python)

A production-grade streaming parser that handles text, tool calls, and errors:

```python
import json
import sys
from dataclasses import dataclass, field
from openai import OpenAI


@dataclass
class StreamState:
    text: str = ""
    tool_calls: dict[int, dict] = field(default_factory=dict)
    finish_reason: str | None = None
    error: str | None = None


def process_stream(stream) -> StreamState:
    state = StreamState()

    try:
        for chunk in stream:
            choice = chunk.choices[0]

            # Text content
            if choice.delta.content:
                state.text += choice.delta.content
                sys.stdout.write(choice.delta.content)
                sys.stdout.flush()

            # Tool calls
            if choice.delta.tool_calls:
                for tc in choice.delta.tool_calls:
                    idx = tc.index
                    if idx not in state.tool_calls:
                        state.tool_calls[idx] = {
                            "id": tc.id,
                            "name": tc.function.name,
                            "arguments": "",
                        }
                    if tc.function.arguments:
                        state.tool_calls[idx]["arguments"] += tc.function.arguments

            # Finish reason
            if choice.finish_reason:
                state.finish_reason = choice.finish_reason

    except Exception as e:
        state.error = str(e)
        state.finish_reason = "error"

    # Parse accumulated tool call arguments
    for idx, tc in state.tool_calls.items():
        try:
            tc["parsed_args"] = json.loads(tc["arguments"])
        except json.JSONDecodeError:
            tc["parsed_args"] = None
            tc["parse_error"] = True

    return state
```

### 8.2 Tool Call Assembly (TypeScript)

```typescript
interface ToolCall {
  id: string;
  name: string;
  arguments: string;
}

async function assembleToolCalls(
  stream: AsyncIterable<ChatCompletionChunk>
): Promise<ToolCall[]> {
  const calls = new Map<number, ToolCall>();

  for await (const chunk of stream) {
    const delta = chunk.choices[0]?.delta;
    if (!delta?.tool_calls) continue;

    for (const tc of delta.tool_calls) {
      if (!calls.has(tc.index)) {
        calls.set(tc.index, {
          id: tc.id!,
          name: tc.function!.name!,
          arguments: "",
        });
      }
      const existing = calls.get(tc.index)!;
      if (tc.function?.arguments) {
        existing.arguments += tc.function.arguments;
      }
    }
  }

  return Array.from(calls.values()).map((tc) => ({
    ...tc,
    // Validate JSON before returning
    arguments: JSON.parse(tc.arguments),
  }));
}
```

### 8.3 Streaming JSON Token Reader (Go)

```go
package main

import (
"encoding/json"
"fmt"
"strings"
)

func streamParseJSON(input string) {
dec := json.NewDecoder(strings.NewReader(input))

for dec.More() {
tok, err := dec.Token()
if err != nil {
fmt.Printf("Error: %v\n", err)
return
}

switch v := tok.(type) {
case json.Delim:
fmt.Printf("Delimiter: %c\n", v)
case string:
fmt.Printf("String: %q\n", v)
case float64:
fmt.Printf("Number: %g\n", v)
case bool:
fmt.Printf("Bool: %t\n", v)
case nil:
fmt.Println("Null")
}
}
}

func main() {
// Simulates incrementally assembled JSON
data := `{"tool": "read_file", "args": {"path": "main.go", "line": 42}}`
streamParseJSON(data)
}
```

### 8.4 Zero-Copy Streaming with Jiter (Rust)

```rust
use jiter::{Jiter, JsonValue, Peek};

fn parse_tool_args(data: &[u8]) -> Result<Vec<(String, JsonValue)>, jiter::JsonError> {
    let mut jiter = Jiter::new(data);
    let mut pairs = Vec::new();

    // Expect opening object brace
    let first_key = jiter.next_object()?;

    if let Some(key) = first_key {
        let value = jiter.next_value()?;
        pairs.push((key.to_string(), value));

        while let Some(key) = jiter.next_key()? {
            let value = jiter.next_value()?;
            pairs.push((key.to_string(), value));
        }
    }

    Ok(pairs)
}
```

---

## 9. Performance Considerations

### 9.1 Memory: String Concatenation vs Buffer Types

Repeated string concatenation (`s += delta`) creates a new string on every
append in most languages. For a tool call with 500 argument fragments:

| Language   | Naïve concat      | Buffer type           | Improvement |
|------------|--------------------|-----------------------|-------------|
| Python     | `s += delta`       | `io.StringIO`         | ~3-5×       |
| JavaScript | `s += delta`       | Array + `.join("")`   | ~2-4×       |
| Go         | `s += delta`       | `strings.Builder`     | ~10-50×     |
| Rust       | `String::push_str` | (already efficient)   | N/A         |

In practice, tool call arguments are small (< 10 KB), so this rarely matters.
But for streaming large code blocks, use a buffer.

### 9.2 CPU: Parse-per-Chunk vs Final Parse

**Parse on every chunk** (streaming parser):
- CPU: O(n²) if re-parsing from the start each time.
- Use case: live preview of partial tool arguments.

**Parse once at end** (accumulate-and-parse):
- CPU: O(n) — single pass.
- Use case: everything else. This is the default.

**Jiter's zero-copy advantage:** by borrowing string data from the input buffer
rather than allocating, Jiter avoids the allocator pressure that makes
`serde_json` slower for streaming workloads. Benchmarks show 4–10× improvement
for key-by-key iteration.

### 9.3 When to Use Which Strategy

| Scenario                          | Strategy                    |
|-----------------------------------|-----------------------------|
| Tool call args (no live preview)  | Accumulate-and-parse        |
| Tool call args (live preview UI)  | Streaming parser            |
| Plain text to terminal            | Character-by-character      |
| Markdown to terminal              | Semantic buffering          |
| Rich TUI (React/Ink)              | Differential rendering      |
| Large JSON responses              | Streaming parser (Jiter/SAX)|
| Multiple parallel tool calls      | Per-index accumulator       |

### 9.4 Latency Budget

At 80 tokens/second, each token arrives every ~12.5 ms. Any per-token
processing must complete well within that window:

- JSON parse attempt: ~0.01 ms (negligible)
- Markdown render: ~0.1–1 ms (acceptable)
- Full terminal repaint: ~5–20 ms (risky—may drop frames)
- React reconciliation (Ink): ~1–3 ms (good)

The bottleneck is almost always **terminal rendering**, not parsing.

---

## 10. Summary

Incremental parsing for LLM streaming boils down to three core problems:

1. **JSON assembly** — accumulate fragments, parse once at the end (or use a
   streaming parser if you need live previews).
2. **Tool call demultiplexing** — use the provider's index/lifecycle events to
   track each tool call independently.
3. **Content buffering** — choose the right flush strategy for your display
   target (immediate for raw terminals, semantic for Markdown, differential for
   rich UIs).

The accumulate-and-parse pattern handles 90% of cases. Reach for streaming
JSON parsers (Jiter, @streamparser/json) only when you need live partial
results. And always handle the error cases—interrupted streams, malformed
chunks, and encoding issues are not theoretical; they happen in production.
