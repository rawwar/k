---
title: Streaming State Machine
description: Design a state machine that tracks the full lifecycle of a streaming response from connection through completion or failure.
---

# Streaming State Machine

> **What you'll learn:**
> - How to model streaming phases as explicit states: connecting, receiving, tool-calling, complete, errored
> - How to handle state transitions triggered by SSE events like message_start and message_stop
> - How to use Rust enums and pattern matching to build a type-safe streaming state machine

Up to this point you have been processing stream events with ad-hoc `match` statements scattered across your stream loop. That works for simple cases, but as complexity grows -- error recovery, reconnection, interrupt handling, progress display -- the logic becomes tangled. In this subchapter you will consolidate all streaming lifecycle management into a single state machine. Rust's enum system makes this natural and type-safe.

## Why a state machine

Consider the events your stream processor must handle and the different contexts in which they can occur:

- A `content_block_delta` is normal during the `Receiving` phase but suspicious if it arrives before `message_start`.
- A network error during `Connecting` means "retry immediately." During `Receiving`, it means "save partial content, then retry."
- A Ctrl+C interrupt during `Receiving` text is different from a Ctrl+C during tool call assembly.

Without explicit states, you end up with boolean flags like `has_started`, `is_receiving_tool`, `had_error` scattered through your code. These flags interact in combinatorial ways that are hard to reason about. A state machine makes every valid combination an explicit enum variant.

## Defining the states

Here are the states your streaming response passes through:

```rust
use std::time::Instant;

/// The current phase of a streaming response.
#[derive(Debug)]
pub enum StreamState {
    /// Waiting for the HTTP connection to be established.
    Connecting {
        started_at: Instant,
    },

    /// Connected, waiting for the first event (message_start).
    WaitingForMessage {
        connected_at: Instant,
    },

    /// Receiving text content from a content block.
    ReceivingText {
        block_index: usize,
        text_so_far: String,
        token_count: u32,
        first_token_at: Option<Instant>,
    },

    /// Receiving tool call JSON fragments.
    ReceivingToolCall {
        block_index: usize,
        tool_id: String,
        tool_name: String,
        json_buffer: String,
    },

    /// Between content blocks (received content_block_stop,
    /// waiting for next content_block_start or message_delta).
    BetweenBlocks {
        completed_blocks: usize,
    },

    /// Stream completed successfully.
    Complete {
        stop_reason: String,
        total_tokens: u32,
        duration: std::time::Duration,
    },

    /// Stream ended due to user interrupt.
    Interrupted {
        partial_text: String,
    },

    /// Stream failed with an error.
    Errored {
        error: StreamError,
        partial_text: String,
        recoverable: bool,
    },
}

#[derive(Debug)]
pub enum StreamError {
    Network(String),
    ParseError(String),
    ApiError { error_type: String, message: String },
    Timeout,
}
```

Each variant carries exactly the data relevant to that phase. `ReceivingText` tracks the accumulated text and token count. `ReceivingToolCall` tracks the tool ID and JSON buffer. `Errored` includes whether the error is recoverable (which drives the reconnection logic in the next subchapter).

## The state machine struct

Wrap the state in a struct that manages transitions:

```rust
use crate::sse::{ContentBlockStub, Delta, StreamEvent};
use crate::tool_accumulator::ToolCall;

/// Manages the streaming lifecycle as a state machine.
pub struct StreamStateMachine {
    state: StreamState,
    /// All text content accumulated across all text blocks.
    all_text: String,
    /// All completed tool calls.
    tool_calls: Vec<ToolCall>,
    /// Input token count from message_start.
    input_tokens: u32,
    /// Output token count from message_delta.
    output_tokens: u32,
    /// When the stream started (for duration tracking).
    stream_start: Instant,
}

/// Events emitted by the state machine for external consumers
/// (renderer, progress display, etc.).
#[derive(Debug)]
pub enum StreamAction {
    /// Display this text token.
    RenderToken(String),
    /// Show that a tool call is being assembled.
    ShowToolProgress { name: String },
    /// Execute this completed tool call.
    ExecuteToolCall(ToolCall),
    /// Update the progress display with current stats.
    UpdateProgress { tokens: u32, elapsed: std::time::Duration },
    /// The stream is finished -- here is the summary.
    Finished { stop_reason: String },
    /// An error occurred.
    ReportError(StreamError),
    /// No external action needed for this transition.
    None,
}

impl StreamStateMachine {
    pub fn new() -> Self {
        Self {
            state: StreamState::Connecting {
                started_at: Instant::now(),
            },
            all_text: String::new(),
            tool_calls: Vec::new(),
            input_tokens: 0,
            output_tokens: 0,
            stream_start: Instant::now(),
        }
    }

    /// Get the current state (for display/logging purposes).
    pub fn state(&self) -> &StreamState {
        &self.state
    }

    /// Transition to the next state based on an incoming stream event.
    /// Returns an action for external consumers.
    pub fn handle_event(&mut self, event: StreamEvent) -> StreamAction {
        match event {
            StreamEvent::MessageStart { message } => {
                self.input_tokens = message.usage.input_tokens;
                self.state = StreamState::WaitingForMessage {
                    connected_at: Instant::now(),
                };
                StreamAction::None
            }

            StreamEvent::ContentBlockStart { index, content_block } => {
                match content_block {
                    ContentBlockStub::Text { .. } => {
                        self.state = StreamState::ReceivingText {
                            block_index: index,
                            text_so_far: String::new(),
                            token_count: 0,
                            first_token_at: None,
                        };
                        StreamAction::None
                    }
                    ContentBlockStub::ToolUse { id, name } => {
                        let action = StreamAction::ShowToolProgress {
                            name: name.clone(),
                        };
                        self.state = StreamState::ReceivingToolCall {
                            block_index: index,
                            tool_id: id,
                            tool_name: name,
                            json_buffer: String::new(),
                        };
                        action
                    }
                }
            }

            StreamEvent::ContentBlockDelta { index, delta } => {
                match delta {
                    Delta::TextDelta { text } => {
                        if let StreamState::ReceivingText {
                            block_index,
                            text_so_far,
                            token_count,
                            first_token_at,
                        } = &mut self.state
                        {
                            if *block_index == index {
                                if first_token_at.is_none() {
                                    *first_token_at = Some(Instant::now());
                                }
                                *token_count += 1;
                                text_so_far.push_str(&text);
                                self.all_text.push_str(&text);

                                return StreamAction::RenderToken(text);
                            }
                        }
                        StreamAction::None
                    }

                    Delta::InputJsonDelta { partial_json } => {
                        if let StreamState::ReceivingToolCall {
                            block_index,
                            json_buffer,
                            ..
                        } = &mut self.state
                        {
                            if *block_index == index {
                                json_buffer.push_str(&partial_json);
                            }
                        }
                        StreamAction::None
                    }
                }
            }

            StreamEvent::ContentBlockStop { index: _ } => {
                // If we were receiving a tool call, finalize it
                let action = if let StreamState::ReceivingToolCall {
                    tool_id,
                    tool_name,
                    json_buffer,
                    ..
                } = &self.state
                {
                    match serde_json::from_str(json_buffer) {
                        Ok(arguments) => {
                            let tool_call = ToolCall {
                                id: tool_id.clone(),
                                name: tool_name.clone(),
                                arguments,
                            };
                            self.tool_calls.push(tool_call.clone());
                            StreamAction::ExecuteToolCall(tool_call)
                        }
                        Err(e) => StreamAction::ReportError(StreamError::ParseError(
                            format!("Invalid JSON for tool '{}': {}", tool_name, e),
                        )),
                    }
                } else {
                    StreamAction::None
                };

                self.state = StreamState::BetweenBlocks {
                    completed_blocks: self.tool_calls.len(),
                };

                action
            }

            StreamEvent::MessageDelta { delta, usage } => {
                self.output_tokens = usage.output_tokens;
                if let Some(reason) = delta.stop_reason {
                    self.state = StreamState::Complete {
                        stop_reason: reason.clone(),
                        total_tokens: self.output_tokens,
                        duration: self.stream_start.elapsed(),
                    };
                    StreamAction::Finished { stop_reason: reason }
                } else {
                    StreamAction::None
                }
            }

            StreamEvent::MessageStop => {
                if !matches!(self.state, StreamState::Complete { .. }) {
                    self.state = StreamState::Complete {
                        stop_reason: "end_turn".to_string(),
                        total_tokens: self.output_tokens,
                        duration: self.stream_start.elapsed(),
                    };
                }
                StreamAction::Finished {
                    stop_reason: "end_turn".to_string(),
                }
            }

            StreamEvent::Ping => StreamAction::None,

            StreamEvent::Error { error } => {
                let stream_error = StreamError::ApiError {
                    error_type: error.error_type,
                    message: error.message,
                };
                self.state = StreamState::Errored {
                    error: StreamError::ApiError {
                        error_type: "api_error".to_string(),
                        message: "See stream error".to_string(),
                    },
                    partial_text: self.all_text.clone(),
                    recoverable: false,
                };
                StreamAction::ReportError(stream_error)
            }
        }
    }

    /// Signal that the user interrupted the stream.
    pub fn interrupt(&mut self) {
        self.state = StreamState::Interrupted {
            partial_text: self.all_text.clone(),
        };
    }

    /// Signal that a network error occurred.
    pub fn network_error(&mut self, error: String) {
        self.state = StreamState::Errored {
            error: StreamError::Network(error),
            partial_text: self.all_text.clone(),
            recoverable: true,
        };
    }

    /// Get accumulated text content.
    pub fn text(&self) -> &str {
        &self.all_text
    }

    /// Take completed tool calls.
    pub fn take_tool_calls(&mut self) -> Vec<ToolCall> {
        std::mem::take(&mut self.tool_calls)
    }
}
```

::: python Coming from Python
Python does not have an equivalent to Rust's enum-based state machines. You would typically use a class with a `state` string attribute:
```python
class StreamProcessor:
    def __init__(self):
        self.state = "connecting"

    def handle_event(self, event):
        if self.state == "connecting" and event.type == "message_start":
            self.state = "receiving"
        elif self.state == "receiving" and event.type == "content_block_delta":
            # process delta
```
The Rust version is superior in two ways: (1) each state variant carries its own data, so you cannot accidentally access `json_buffer` when in the `ReceivingText` state, and (2) the compiler enforces exhaustive matching, so you cannot forget to handle a state/event combination.
:::

## Using the state machine in the stream loop

Here is how the state machine simplifies the main processing loop:

```rust
pub async fn stream_with_state_machine(
    mut byte_stream: impl futures::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
    cancel_token: CancellationToken,
) -> Result<StreamOutput, Box<dyn std::error::Error>> {
    let mut splitter = LineSplitter::new();
    let mut parser = SseParser::new();
    let mut sm = StreamStateMachine::new();

    loop {
        let chunk = tokio::select! {
            chunk = futures::StreamExt::next(&mut byte_stream) => {
                match chunk {
                    Some(Ok(bytes)) => bytes,
                    Some(Err(e)) => {
                        sm.network_error(e.to_string());
                        break;
                    }
                    None => break,
                }
            }
            _ = cancel_token.cancelled() => {
                sm.interrupt();
                break;
            }
        };

        for line in splitter.feed(&chunk) {
            let Some(sse_event) = parser.feed_line(&line) else { continue };
            if sse_event.event_type == "ping" { continue; }

            let stream_event: StreamEvent = serde_json::from_str(&sse_event.data)?;
            let action = sm.handle_event(stream_event);

            match action {
                StreamAction::RenderToken(text) => {
                    print!("{}", text);
                    std::io::Write::flush(&mut std::io::stdout())?;
                }
                StreamAction::ShowToolProgress { name } => {
                    eprintln!("\n[Assembling tool call: {}]", name);
                }
                StreamAction::ExecuteToolCall(tool_call) => {
                    eprintln!("[Tool ready: {}]", tool_call.name);
                }
                StreamAction::Finished { .. } => break,
                StreamAction::ReportError(err) => {
                    eprintln!("[Error: {:?}]", err);
                    break;
                }
                _ => {}
            }
        }

        if matches!(sm.state(), StreamState::Complete { .. } | StreamState::Errored { .. }) {
            break;
        }
    }

    println!();

    Ok(StreamOutput {
        text: sm.text().to_string(),
        tool_calls: sm.take_tool_calls(),
        stop_reason: match sm.state() {
            StreamState::Complete { stop_reason, .. } => Some(stop_reason.clone()),
            StreamState::Interrupted { .. } => Some("user_interrupt".to_string()),
            _ => None,
        },
    })
}
```

The loop body is now clean: read a chunk, feed it through the parser, hand each event to the state machine, and act on whatever the state machine tells you. All the complex logic about what events are valid in what context, how to accumulate tool calls, and when the stream is done lives inside `handle_event()`.

::: wild In the Wild
Claude Code internally models its streaming lifecycle as a state machine with states for "waiting," "streaming text," "streaming tool input," "executing tool," and "complete." Transitions between these states drive the UI -- the terminal display changes based on the current state, showing a spinner during waiting, flowing text during streaming, and a tool execution indicator during tool calls. This clean separation of state from presentation is a pattern worth adopting from the start.
:::

## Key Takeaways

- A state machine replaces scattered boolean flags and ad-hoc `match` statements with a single, explicit representation of the streaming lifecycle.
- Rust enums are ideal for state machines because each variant carries exactly the data relevant to that phase, and the compiler enforces exhaustive matching.
- The state machine's `handle_event()` method returns `StreamAction` values that tell external consumers (renderer, progress display) what to do, cleanly separating state logic from side effects.
- Helper methods like `interrupt()` and `network_error()` handle out-of-band transitions that do not come from SSE events.
- The main stream loop becomes simple and readable: read chunk, parse events, hand to state machine, execute actions.
