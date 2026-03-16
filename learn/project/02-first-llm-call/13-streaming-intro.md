---
title: Streaming Intro
description: Preview the streaming API that delivers tokens incrementally for real-time output in your CLI.
---

# Streaming Intro

> **What you'll learn:**
> - How server-sent events (SSE) deliver tokens one at a time instead of waiting for the full response
> - How streaming improves perceived latency and enables real-time typewriter-style output in your REPL
> - The high-level changes needed to switch from synchronous to streaming API calls (detailed implementation in a later chapter)

So far, your agent sends a request and waits for the entire response before displaying anything. For a short answer, that is fine. But when Claude generates several paragraphs of code, the user stares at a blank terminal for 10-30 seconds. That is a terrible experience. Streaming fixes this by delivering tokens as they are generated, creating the typewriter effect you see in chat interfaces like claude.ai.

This subchapter gives you a conceptual introduction to streaming. You will implement it fully in a later chapter, but understanding the idea now will help you appreciate the architecture decisions you make along the way.

## The Latency Problem

When you make a non-streaming API call, here is what happens:

```
Time 0s:   Request sent
Time 0.5s: Model starts generating tokens
Time 1s:   Token 1 generated
Time 1.5s: Token 2 generated
...
Time 15s:  Token 300 generated (end of response)
Time 15s:  Full response sent to your client
Time 15s:  User sees the response
```

The user waits 15 seconds and then sees everything at once. The model started producing useful output at 1 second, but that output was held back until generation was complete.

With streaming:

```
Time 0s:   Request sent (with "stream": true)
Time 0.5s: Model starts generating tokens
Time 1s:   Token 1 streamed to client -> user sees it immediately
Time 1.5s: Token 2 streamed to client -> user sees it
...
Time 15s:  Token 300 streamed -> user sees final token
```

The user sees the first token after about 1 second instead of 15. The total time is the same, but the **perceived** latency drops dramatically.

## How Streaming Works: Server-Sent Events

The Anthropic API uses **server-sent events (SSE)** for streaming. SSE is a simple protocol built on top of HTTP: the server sends a stream of text events over a long-lived HTTP connection, one event at a time.

To request streaming, add `"stream": true` to your request:

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 4096,
  "stream": true,
  "messages": [
    { "role": "user", "content": "Write a hello world in Rust." }
  ]
}
```

Instead of a single JSON response, the server sends a series of events:

```
event: message_start
data: {"type":"message_start","message":{"id":"msg_01...","type":"message","role":"assistant","content":[],"model":"claude-sonnet-4-20250514","stop_reason":null,"usage":{"input_tokens":15,"output_tokens":0}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Here"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"'s"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" a"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" simple"}}

...

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":42}}

event: message_stop
data: {"type":"message_stop"}
```

Each `content_block_delta` event carries a small piece of text (often just one token). Your client reads these events as they arrive and prints each text fragment immediately.

::: python Coming from Python
In Python, the `anthropic` SDK handles streaming with an iterator:
```python
with client.messages.stream(
    model="claude-sonnet-4-20250514",
    max_tokens=4096,
    messages=[{"role": "user", "content": "Hello"}],
) as stream:
    for text in stream.text_stream:
        print(text, end="", flush=True)
```
In Rust, you will process the SSE stream using reqwest's byte streaming and parse the events manually (or with an SSE parsing crate). The concept is the same -- iterate over chunks as they arrive -- but the low-level details require more code.
:::

## SSE Event Types

The Anthropic streaming API sends these event types in order:

| Event Type | Purpose |
|---|---|
| `message_start` | Begins the response. Contains the message ID, model, and initial usage. |
| `content_block_start` | Begins a new content block. Contains the block type (text or tool_use). |
| `content_block_delta` | Delivers a chunk of content for the current block. For text blocks, this is a text fragment. |
| `content_block_stop` | Marks the end of the current content block. |
| `message_delta` | Delivers end-of-message metadata: stop reason and final usage counts. |
| `message_stop` | Signals the complete end of the response. |

For a simple text response, the flow is:

```
message_start -> content_block_start -> delta, delta, delta, ... -> content_block_stop -> message_delta -> message_stop
```

For a response with multiple content blocks (like text followed by a tool call), you get multiple `content_block_start` / `delta` / `content_block_stop` sequences.

## What Changes in Your Code

Switching from non-streaming to streaming affects several layers:

### 1. The Request

Add `stream: true` to your request body:

```rust
use serde::Serialize;

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}
```

### 2. The Response Handling

Instead of `response.json().await?`, you read the response body as a stream of bytes:

```rust
// Non-streaming: wait for the full response
let chat_response: ChatResponse = response.json().await?;

// Streaming (conceptual): process chunks as they arrive
let mut stream = response.bytes_stream();
while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    // Parse SSE events from the chunk
    // Print text deltas immediately
}
```

### 3. The Output

Instead of printing the complete response at once, you print each text fragment as it arrives:

```rust
// Non-streaming
println!("{}", full_response_text);

// Streaming
print!("{}", text_fragment);  // no newline, no buffering
std::io::stdout().flush()?;   // flush immediately so the user sees it
```

### 4. Assembling the Final Response

After streaming completes, you need the full response text to add to the conversation history. You accumulate the text fragments during streaming:

```rust
let mut full_text = String::new();

// During streaming:
full_text.push_str(&text_delta);
print!("{}", text_delta);

// After streaming completes:
conversation.push(Message::assistant(&full_text));
```

## A Preview of the Streaming Implementation

Here is a sketch of what the streaming code will look like when you implement it fully. This is not a complete implementation -- it is meant to show the shape of the solution:

```rust
use futures_util::StreamExt;

async fn send_message_streaming(
    client: &reqwest::Client,
    messages: &[Message],
) -> Result<String, Box<dyn std::error::Error>> {
    let request = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 4096,
        "stream": true,
        "messages": messages,
    });

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .json(&request)
        .send()
        .await?;

    let mut full_text = String::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let text = String::from_utf8_lossy(&chunk);

        // Each SSE chunk looks like:
        // event: content_block_delta\ndata: {"type":"content_block_delta",...}\n\n
        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                // Parse the JSON event and extract text deltas
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(delta_text) = event
                        .get("delta")
                        .and_then(|d| d.get("text"))
                        .and_then(|t| t.as_str())
                    {
                        print!("{}", delta_text);
                        std::io::Write::flush(&mut std::io::stdout())?;
                        full_text.push_str(delta_text);
                    }
                }
            }
        }
    }

    println!(); // Final newline after streaming completes
    Ok(full_text)
}
```

This is a simplified version. A production implementation needs to handle:
- SSE events that span multiple TCP chunks
- Different event types (not just `content_block_delta`)
- Error events in the stream
- Connection drops mid-stream
- Proper token usage tracking from `message_delta` events

You will build the full implementation in a dedicated streaming chapter later in the book.

::: wild In the Wild
Every production coding agent uses streaming. Claude Code streams all responses to provide real-time output as the model generates code and explanations. OpenCode similarly streams responses and uses the incremental output to update its terminal UI in real time. The non-streaming API is primarily used for background operations where the user does not need to see incremental progress. For your CLI agent, streaming is what makes the interaction feel responsive and alive.
:::

## Key Takeaways

- Streaming delivers tokens incrementally via server-sent events (SSE), reducing perceived latency from the full generation time to time-to-first-token (typically under 1 second).
- Enable streaming by adding `"stream": true` to your request body. The response becomes a series of SSE events instead of a single JSON object.
- The key event type is `content_block_delta`, which carries text fragments that you print immediately and accumulate for the conversation history.
- Streaming requires reading the response as a byte stream (`response.bytes_stream()`) instead of consuming it as a single JSON object.
- You will implement full streaming in a later chapter -- for now, the non-streaming approach works for getting your agent functional.
