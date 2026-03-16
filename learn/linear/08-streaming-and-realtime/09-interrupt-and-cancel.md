---
title: Interrupt and Cancel
description: Handling user-initiated cancellation of in-flight streaming requests, including HTTP connection teardown, partial result preservation, and UI state cleanup.
---

# Interrupt and Cancel

> **What you'll learn:**
> - How to cleanly abort an in-flight HTTP streaming request when the user presses Ctrl+C or issues a cancel command
> - Preserving partial streamed content that has already been rendered so the user does not lose visible output
> - Coordinating cancellation across the network, parser, and renderer layers using CancellationToken

When a user is watching a streaming response and realizes the LLM is heading in the wrong direction, they need to be able to stop it *immediately*. Not after the current paragraph. Not after the current sentence. Right now. Ctrl+C should feel instant, and the partial response that has already been rendered should be preserved -- the user saw it, and it might contain useful information. Building this interrupt capability correctly requires coordinating cancellation across every layer of your streaming pipeline.

## The Cancellation Challenge

Cancellation in a streaming pipeline is harder than it looks. When the user presses Ctrl+C, you need to:

1. **Stop reading from the network** -- close the HTTP connection so the server stops generating tokens (and you stop being billed for them).
2. **Drain or discard buffered events** -- any events sitting in channels between pipeline stages should not be processed.
3. **Preserve what was already rendered** -- the text visible on screen should stay visible.
4. **Clean up application state** -- the conversation model should record the partial response, tool call accumulators should be reset, and the agent should be ready for the next input.

The naive approach -- just dropping the entire pipeline -- can leave your application in an inconsistent state. A tool call might be half-accumulated, a markdown code block might be open but never closed, or the terminal might be in an alternate screen mode.

## CancellationToken: The Coordination Primitive

Tokio provides `CancellationToken` (via the `tokio-util` crate) as a lightweight, cloneable cancellation signal. You create one token, clone it into every pipeline stage, and when cancellation is needed, you cancel it once and every stage observes it:

```rust
use tokio_util::sync::CancellationToken;
use tokio::sync::mpsc;

pub struct StreamingSession {
    cancel_token: CancellationToken,
}

impl StreamingSession {
    pub fn new() -> Self {
        Self {
            cancel_token: CancellationToken::new(),
        }
    }

    /// Call this when the user presses Ctrl+C
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }

    /// Get a clone of the token for use in pipeline stages
    pub fn token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }
}
```

Each pipeline stage uses `tokio::select!` to race between its normal work and the cancellation signal:

```rust
use futures::StreamExt;

async fn cancellable_network_reader(
    response: reqwest::Response,
    tx: mpsc::Sender<bytes::Bytes>,
    cancel: CancellationToken,
) {
    let mut stream = response.bytes_stream();

    loop {
        tokio::select! {
            // Check for cancellation
            _ = cancel.cancelled() => {
                // Connection is dropped when `stream` goes out of scope,
                // which closes the HTTP connection and stops token generation.
                println!("[Network reader cancelled]");
                return;
            }
            // Try to read the next chunk
            chunk = stream.next() => {
                match chunk {
                    Some(Ok(bytes)) => {
                        if tx.send(bytes).await.is_err() {
                            return; // Downstream dropped
                        }
                    }
                    Some(Err(e)) => {
                        eprintln!("Network error: {}", e);
                        return;
                    }
                    None => return, // Stream complete
                }
            }
        }
    }
}
```

The critical detail: when the `cancel.cancelled()` branch fires, the function returns, which drops `stream`. Dropping the `reqwest` byte stream closes the underlying HTTP connection, which tells the server to stop generating tokens. This is important for cost -- you are billed per output token, so stopping generation early saves money.

::: python Coming from Python
In Python with `httpx`, cancellation typically uses `asyncio.Task.cancel()`:
```python
import asyncio

async def stream_response(client, url):
    async with client.stream("POST", url) as response:
        async for chunk in response.aiter_bytes():
            process(chunk)

task = asyncio.create_task(stream_response(client, url))
# Later, on Ctrl+C:
task.cancel()
try:
    await task
except asyncio.CancelledError:
    print("Stream cancelled")
```
Rust's `CancellationToken` serves a similar purpose to `asyncio.Task.cancel()`, but with an important difference: in Rust, cancellation is cooperative and explicit. You check for cancellation at specific points using `select!`, whereas Python's `CancelledError` can interrupt any `await` point. The Rust approach gives you more control over cleanup but requires you to check for cancellation in every loop.
:::

## Handling Ctrl+C with Signal Handlers

To connect user input (Ctrl+C) to your cancellation token, you need a signal handler. Tokio provides `tokio::signal::ctrl_c()`:

```rust
use tokio_util::sync::CancellationToken;

async fn run_with_interrupt() {
    let cancel = CancellationToken::new();
    let cancel_for_signal = cancel.clone();

    // Spawn a task that waits for Ctrl+C
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl+c");
        println!("\n[Interrupt received, cancelling stream...]");
        cancel_for_signal.cancel();
    });

    // Run the streaming pipeline with the cancel token
    let result = streaming_pipeline(cancel.clone()).await;

    match result {
        StreamResult::Complete(response) => {
            println!("Response complete: {} tokens", response.token_count);
        }
        StreamResult::Cancelled(partial) => {
            println!("Response cancelled after {} tokens", partial.token_count);
            // The partial response is preserved in conversation state
        }
        StreamResult::Error(e) => {
            eprintln!("Stream error: {}", e);
        }
    }
}

enum StreamResult {
    Complete(ResponseData),
    Cancelled(ResponseData),
    Error(String),
}

struct ResponseData {
    text: String,
    token_count: usize,
}

async fn streaming_pipeline(cancel: CancellationToken) -> StreamResult {
    let mut accumulated_text = String::new();
    let mut token_count = 0;

    // Simulated event loop
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(20));

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                return StreamResult::Cancelled(ResponseData {
                    text: accumulated_text,
                    token_count,
                });
            }
            _ = interval.tick() => {
                // Simulate receiving a token
                let token = "word ";
                accumulated_text.push_str(token);
                token_count += 1;
                print!("{}", token);
                std::io::stdout().flush().ok();

                if token_count >= 200 {
                    return StreamResult::Complete(ResponseData {
                        text: accumulated_text,
                        token_count,
                    });
                }
            }
        }
    }
}

use std::io::Write;
```

Notice that the `Cancelled` variant still carries a `ResponseData` with the accumulated text. The user saw this text, and it should be preserved in the conversation state so the LLM has context for the next turn.

## Preserving Partial State

When cancellation occurs mid-stream, your application must decide what to keep and what to discard. Here is a structured approach:

```rust
pub struct ConversationTurn {
    pub role: String,
    pub content: Vec<ContentBlock>,
    pub is_complete: bool,
    pub stop_reason: Option<String>,
}

pub enum ContentBlock {
    Text {
        text: String,
        is_complete: bool,
    },
    ToolUse {
        id: String,
        name: String,
        arguments: Option<serde_json::Value>,
        is_complete: bool,
    },
}

pub fn finalize_cancelled_turn(
    accumulated_text: &str,
    tool_accumulator: Option<&ToolCallAccumulator>,
) -> ConversationTurn {
    let mut content = Vec::new();

    // Preserve any text that was accumulated
    if !accumulated_text.is_empty() {
        content.push(ContentBlock::Text {
            text: accumulated_text.to_string(),
            is_complete: false, // Mark as incomplete
        });
    }

    // If a tool call was in progress, try to preserve what we have
    if let Some(acc) = tool_accumulator {
        let arguments = serde_json::from_str(acc.raw()).ok();
        content.push(ContentBlock::ToolUse {
            id: acc.tool_id.clone(),
            name: acc.tool_name.clone(),
            arguments,
            is_complete: false,
        });
    }

    ConversationTurn {
        role: "assistant".to_string(),
        content,
        is_complete: false,
        stop_reason: Some("user_cancelled".to_string()),
    }
}

// ToolCallAccumulator referenced from the partial JSON handling subchapter
pub struct ToolCallAccumulator {
    pub tool_id: String,
    pub tool_name: String,
    buffer: String,
}

impl ToolCallAccumulator {
    pub fn raw(&self) -> &str {
        &self.buffer
    }
}
```

The key design decision: mark everything as `is_complete: false`. This allows the UI to show a visual indicator (like "[interrupted]") and lets the agent decide whether to retry the operation or move on.

## Cancelling Specific Streams

Sometimes you want to cancel one stream without affecting others. For example, if the agent is running two parallel tool calls and the user wants to cancel just one, you need per-stream cancellation:

```rust
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

pub struct StreamManager {
    /// Global cancellation: cancels everything
    global_cancel: CancellationToken,
    /// Per-stream cancellation tokens
    streams: HashMap<String, CancellationToken>,
}

impl StreamManager {
    pub fn new() -> Self {
        Self {
            global_cancel: CancellationToken::new(),
            streams: HashMap::new(),
        }
    }

    /// Create a new stream with its own cancellation token.
    /// The stream is cancelled when either its own token or the global token fires.
    pub fn create_stream(&mut self, id: &str) -> CancellationToken {
        let token = self.global_cancel.child_token();
        self.streams.insert(id.to_string(), token.clone());
        token
    }

    /// Cancel a specific stream
    pub fn cancel_stream(&self, id: &str) {
        if let Some(token) = self.streams.get(id) {
            token.cancel();
        }
    }

    /// Cancel all streams
    pub fn cancel_all(&self) {
        self.global_cancel.cancel();
    }
}
```

The `child_token()` method creates a token that is cancelled when either the child itself is cancelled or the parent is cancelled. This gives you a hierarchy: cancelling the global token cancels everything, but you can also cancel individual streams independently.

## Graceful vs. Immediate Cancellation

There are two cancellation styles, and your agent should support both:

**Immediate cancellation (Ctrl+C):** Stop everything right now. Drop the network connection, stop rendering, and return to the input prompt. This is what users expect when they hit Ctrl+C.

**Graceful cancellation (escape key or cancel command):** Signal that no more tokens are needed, but let the current event finish processing. This avoids cutting off mid-word and gives the cleanup code time to finalize state.

```rust
async fn graceful_cancel_loop(
    mut rx: mpsc::Receiver<StreamEvent>,
    cancel: CancellationToken,
) -> String {
    let mut text = String::new();

    loop {
        tokio::select! {
            biased;  // Check cancellation first

            _ = cancel.cancelled() => {
                // Graceful: drain any events already in the channel buffer
                // to avoid cutting off mid-word
                while let Ok(event) = rx.try_recv() {
                    if let StreamEvent::TextDelta(t) = event {
                        print!("{}", t);
                        text.push_str(&t);
                    }
                }
                break;
            }
            event = rx.recv() => {
                match event {
                    Some(StreamEvent::TextDelta(t)) => {
                        print!("{}", t);
                        text.push_str(&t);
                    }
                    Some(StreamEvent::MessageComplete) | None => break,
                    _ => {}
                }
            }
        }
    }

    std::io::stdout().flush().ok();
    text
}

// StreamEvent from the backpressure subchapter
#[derive(Debug)]
enum StreamEvent {
    TextDelta(String),
    ToolCallStart { id: String, name: String },
    ToolCallDelta { id: String, json_fragment: String },
    ToolCallEnd { id: String },
    MessageComplete,
    Error(String),
}
```

The `biased` keyword in `select!` ensures that the cancellation branch is checked first, so cancellation is handled promptly even when events are arriving rapidly.

::: wild In the Wild
Claude Code handles Ctrl+C by immediately cancelling the streaming HTTP request and preserving whatever text has been rendered. The partial response is kept in the conversation history so the LLM has context for the next turn. If the user presses Ctrl+C during a tool call that has not yet been executed, the tool call is discarded. If the tool was already executed and only the result rendering was interrupted, the tool result is preserved. This distinction between "cancel the operation" and "stop showing me output" is subtle but important for maintaining a coherent conversation state.
:::

## Key Takeaways

- **`CancellationToken`** from `tokio-util` is the primary coordination primitive for cancellation in async Rust. Clone it into every pipeline stage and check it with `tokio::select!`.
- **Dropping the `reqwest` byte stream** closes the HTTP connection, which signals the LLM server to stop generating tokens -- this saves API costs on cancelled requests.
- **Preserve partial state** when cancelling: accumulated text and partially-parsed tool calls should be recorded in the conversation history, marked as incomplete.
- Use **`child_token()`** for hierarchical cancellation where the global token cancels everything but individual streams can also be cancelled independently.
- Distinguish between **immediate cancellation** (Ctrl+C, stop right now) and **graceful cancellation** (drain buffered events, finish the current word, then stop).
