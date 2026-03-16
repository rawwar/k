---
title: Streaming Protocol
description: How Server-Sent Events deliver incremental LLM responses and why streaming is essential for responsive agent interfaces.
---

# Streaming Protocol

> **What you'll learn:**
> - How SSE-based streaming delivers tokens incrementally and the event types you need to handle
> - Why streaming is critical for agent UX and how to detect tool calls within a streamed response
> - How to implement a streaming parser that accumulates text and identifies complete tool use blocks

When you make a non-streaming API call, you wait for the model to finish generating its entire response before you receive anything. For a response that takes 10 seconds to generate, your user stares at a blank screen for 10 seconds. Streaming fixes this by delivering tokens as they are generated, typically starting within a few hundred milliseconds. For a coding agent that might generate lengthy code or reasoning, streaming is not a nice-to-have -- it is essential for a usable interface.

## Server-Sent Events (SSE)

Both Anthropic and OpenAI use the **Server-Sent Events** protocol for streaming. SSE is a simple HTTP-based protocol where the server sends a stream of events over a long-lived HTTP connection. Each event is a text line prefixed with `data: `:

```
data: {"type": "content_block_start", ...}

data: {"type": "content_block_delta", ...}

data: {"type": "content_block_delta", ...}

data: {"type": "message_stop"}

```

Key characteristics of SSE:
- It uses a regular HTTP POST request with `"stream": true` in the body
- The response has `Content-Type: text/event-stream`
- Events are separated by blank lines
- The stream ends with a special termination event
- Each `data:` line contains a complete JSON object

To enable streaming, you add `"stream": true` to your API request:

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 4096,
  "stream": true,
  "messages": [
    {"role": "user", "content": "Write a Rust function to sort a vector"}
  ]
}
```

## Anthropic Streaming Events

Anthropic's streaming API sends a sequence of typed events. Here is the complete event flow for a response that contains text:

```
event: message_start
data: {"type":"message_start","message":{"id":"msg_01XFD...","type":"message","role":"assistant","content":[],"model":"claude-sonnet-4-20250514","stop_reason":null,"usage":{"input_tokens":25,"output_tokens":1}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Here"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"'s a"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" sorting"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" function"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":15}}

event: message_stop
data: {"type":"message_stop"}
```

The event types form a clear hierarchy:

| Event | Purpose |
|---|---|
| `message_start` | Begins the response, includes model info and input token count |
| `content_block_start` | Begins a content block (text or tool_use), includes the block type |
| `content_block_delta` | Delivers incremental content within a block |
| `content_block_stop` | Ends the current content block |
| `message_delta` | Delivers final message metadata including stop_reason and output token count |
| `message_stop` | Terminates the stream |

## Streaming Tool Use Events

When the model makes a tool call, the streaming events include tool use blocks. This is where streaming gets more complex for agents:

```
event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Let me read that file."}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: content_block_start
data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_01ABC","name":"read_file"}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"pa"}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"th\": \"sr"}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"c/main.rs\"}"}}

event: content_block_stop
data: {"type":"content_block_stop","index":1}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":42}}

event: message_stop
data: {"type":"message_stop"}
```

Notice the key difference: tool use blocks stream `input_json_delta` events containing partial JSON fragments. You cannot parse the tool call arguments until the entire block is complete -- the fragments `{"pa`, `th": "sr`, and `c/main.rs"}` only form valid JSON when concatenated: `{"path": "src/main.rs"}`.

## Building a Streaming Accumulator

Your agent needs a streaming parser that accumulates events and produces complete messages. Here is the conceptual structure:

```rust
struct StreamAccumulator {
    content_blocks: Vec<ContentBlock>,
    current_text: String,
    current_tool_json: String,
    current_block_index: Option<usize>,
    stop_reason: Option<String>,
    input_tokens: u32,
    output_tokens: u32,
}

impl StreamAccumulator {
    fn process_event(&mut self, event: &StreamEvent) {
        match event {
            StreamEvent::ContentBlockStart { index, content_block } => {
                self.current_block_index = Some(*index);
                match content_block.block_type.as_str() {
                    "text" => self.current_text.clear(),
                    "tool_use" => self.current_tool_json.clear(),
                    _ => {}
                }
            }
            StreamEvent::ContentBlockDelta { index, delta } => {
                match delta {
                    Delta::TextDelta { text } => {
                        self.current_text.push_str(text);
                        // Emit text to UI immediately for display
                    }
                    Delta::InputJsonDelta { partial_json } => {
                        self.current_tool_json.push_str(partial_json);
                        // Do NOT try to parse yet - wait for block_stop
                    }
                }
            }
            StreamEvent::ContentBlockStop { index } => {
                // Now the block is complete - finalize it
                // Parse accumulated JSON for tool calls
            }
            StreamEvent::MessageDelta { delta, usage } => {
                self.stop_reason = Some(delta.stop_reason.clone());
                self.output_tokens = usage.output_tokens;
            }
            _ => {}
        }
    }
}
```

The critical insight is: **display text deltas immediately, but accumulate tool call JSON until the block is complete**. Text can be streamed to the user character by character. Tool call arguments must be fully accumulated and parsed as complete JSON before you can execute the tool.

::: python Coming from Python
In Python, you consume SSE streams with a `for` loop over the response: `for event in client.messages.stream(...)`. The Anthropic and OpenAI Python SDKs handle SSE parsing internally. In Rust, you will use a library like `reqwest` with streaming response support and parse SSE events manually or with an `eventsource` crate. The Rust approach gives you zero-copy parsing and explicit control over buffering, but requires more setup code.
:::

## OpenAI Streaming Format

OpenAI's streaming format uses a similar SSE transport but different event structure:

```
data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}

data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Here"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"'s"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":" a sorting function"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: [DONE]
```

Key differences from Anthropic:
- No typed event names -- all events are `data:` lines with JSON
- Content is nested under `choices[0].delta`
- Text appears in `delta.content`
- Tool calls appear in `delta.tool_calls` as incremental fragments
- Stream ends with `data: [DONE]` instead of a `message_stop` event
- `finish_reason` replaces `stop_reason`

For tool calls in OpenAI's streaming format:

```
data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_abc","type":"function","function":{"name":"read_file","arguments":""}}]}}]}

data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"pa"}}]}}]}

data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"th\":"}}]}}]}

data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":" \"src/main.rs\"}"}}]}}]}

data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}]}

data: [DONE]
```

The same principle applies: accumulate `arguments` fragments until the stream ends, then parse the complete JSON.

## Why Streaming Matters for Agents

Beyond UX responsiveness, streaming provides functional benefits for agents:

**Early text display:** The model often explains what it is about to do before making a tool call. Streaming lets the user see "Let me read the file to understand the issue..." while the tool call JSON is still being generated.

**Progress indication:** During long responses (like generating a large function), streaming shows the user that work is happening, preventing them from assuming the agent is stuck.

**Cancellation:** With streaming, the user can cancel a response mid-generation if they see the model going in the wrong direction. Without streaming, they would have to wait for the full response.

**Latency hiding:** The time between the first token and the last token is perceived differently than waiting for the entire response. A 10-second generation that starts streaming after 200ms feels much faster than a 10-second wait followed by a full response.

## Handling Stream Errors

Streams can fail mid-response due to network issues, rate limits, or server errors. Your agent needs to handle:

**Connection drops:** The TCP connection closes unexpectedly. Retry the entire request.

**Incomplete JSON:** If the stream ends before `message_stop`, you may have accumulated partial content. Text content that was already displayed to the user should be preserved, but incomplete tool calls cannot be executed.

**Rate limit errors:** Even streaming responses can hit rate limits. The server might send an error event mid-stream or close the connection with an HTTP error.

A robust streaming implementation uses timeouts to detect stalled streams (no events for 30+ seconds) and implements retry logic for transient failures.

::: wild In the Wild
Claude Code streams all responses and renders text incrementally in the terminal. Tool calls are shown as "thinking..." until the complete tool call JSON is received, at which point the tool name and arguments are displayed. This approach gives the user continuous feedback about what the agent is doing. The streaming parser maintains a state machine that tracks which content block is currently active, accumulating text and tool call data independently.
:::

## Key Takeaways

- SSE streaming delivers tokens incrementally over a long-lived HTTP connection, starting response delivery within hundreds of milliseconds instead of waiting for the complete generation
- Anthropic uses typed events (`content_block_start`, `content_block_delta`, `content_block_stop`) while OpenAI uses a flat `data:` format with JSON chunks -- both require accumulating tool call JSON until the block completes
- Display text deltas immediately for responsiveness, but accumulate tool call JSON fragments until the content block ends before parsing -- partial JSON is not valid JSON
- Streaming enables cancellation, progress indication, and latency hiding, all of which are critical for agent UX during multi-step operations
- Implement timeout detection and retry logic for stream failures, preserving already-displayed text while discarding incomplete tool calls
