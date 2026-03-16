---
title: Partial Tool Call Assembly
description: Reassemble streamed JSON fragments of tool call arguments into complete, parseable tool invocations for execution.
---

# Partial Tool Call Assembly

> **What you'll learn:**
> - How tool_use content blocks are streamed as incremental JSON fragments
> - How to buffer and concatenate partial JSON until the tool call is complete
> - How to detect tool call completion boundaries and trigger tool execution

Text streaming is straightforward -- each delta is a string fragment, and you concatenate them. Tool calls are harder. When the model decides to call a tool, it streams the tool's JSON arguments as a series of fragments that are individually unparseable. You need to buffer these fragments, detect when the complete JSON is assembled, and then trigger tool execution. Get this wrong and your agent either crashes on invalid JSON or never executes tools.

## How tool calls stream

Recall from the SSE protocol subchapter that tool_use content blocks follow this pattern:

```
event: content_block_start
data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_01ABC","name":"read_file"}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"path\":"}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":" \"/src"}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"/main.rs\"}"}}

event: content_block_stop
data: {"type":"content_block_stop","index":1}
```

The `content_block_start` event gives you the tool name (`read_file`) and a unique ID (`toolu_01ABC`). Then a series of `content_block_delta` events carry `input_json_delta` payloads, each containing a fragment of the JSON arguments string. Finally, `content_block_stop` signals that all fragments have been delivered.

When you concatenate the three `partial_json` values, you get:

```json
{"path": "/src/main.rs"}
```

Only the concatenated result is valid JSON. Each individual fragment -- `{"path":`, ` "/src`, `/main.rs"}` -- is not valid on its own.

## The tool call accumulator

Let's build a struct that buffers partial tool call data and assembles complete tool invocations:

```rust
use std::collections::HashMap;
use serde_json::Value;

/// Represents a complete, ready-to-execute tool call.
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// Tracks in-progress tool calls as JSON fragments arrive.
#[derive(Debug)]
pub struct ToolCallAccumulator {
    /// Active tool calls indexed by content block index.
    active: HashMap<usize, PartialToolCall>,
    /// Completed tool calls ready for execution.
    completed: Vec<ToolCall>,
}

#[derive(Debug)]
struct PartialToolCall {
    id: String,
    name: String,
    json_buffer: String,
}

impl ToolCallAccumulator {
    pub fn new() -> Self {
        Self {
            active: HashMap::new(),
            completed: Vec::new(),
        }
    }

    /// Called when a content_block_start event arrives for a tool_use block.
    pub fn start_tool_call(&mut self, index: usize, id: String, name: String) {
        self.active.insert(
            index,
            PartialToolCall {
                id,
                name,
                json_buffer: String::new(),
            },
        );
    }

    /// Called when a content_block_delta event arrives with input_json_delta.
    /// Appends the JSON fragment to the buffer for the given content block.
    pub fn append_json(&mut self, index: usize, partial_json: &str) {
        if let Some(partial) = self.active.get_mut(&index) {
            partial.json_buffer.push_str(partial_json);
        }
    }

    /// Called when a content_block_stop event arrives.
    /// Attempts to parse the accumulated JSON and finalize the tool call.
    pub fn finish_block(&mut self, index: usize) -> Result<Option<ToolCall>, ToolCallError> {
        let partial = match self.active.remove(&index) {
            Some(p) => p,
            None => return Ok(None), // Not a tool_use block, ignore
        };

        // Parse the accumulated JSON
        let arguments: Value = serde_json::from_str(&partial.json_buffer).map_err(|e| {
            ToolCallError::InvalidJson {
                tool_name: partial.name.clone(),
                json_fragment: partial.json_buffer.clone(),
                source: e,
            }
        })?;

        let tool_call = ToolCall {
            id: partial.id,
            name: partial.name,
            arguments,
        };

        self.completed.push(tool_call.clone());
        Ok(Some(tool_call))
    }

    /// Returns all completed tool calls and clears the completed list.
    pub fn take_completed(&mut self) -> Vec<ToolCall> {
        std::mem::take(&mut self.completed)
    }

    /// Check if any tool calls are currently being accumulated.
    pub fn has_active_calls(&self) -> bool {
        !self.active.is_empty()
    }
}

#[derive(Debug)]
pub enum ToolCallError {
    InvalidJson {
        tool_name: String,
        json_fragment: String,
        source: serde_json::Error,
    },
}

impl std::fmt::Display for ToolCallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolCallError::InvalidJson {
                tool_name,
                json_fragment,
                source,
            } => {
                write!(
                    f,
                    "Invalid JSON for tool '{}': {} (received: '{}')",
                    tool_name, source, json_fragment
                )
            }
        }
    }
}

impl std::error::Error for ToolCallError {}
```

The key design decisions here:

1. **Index-based tracking** -- tool calls are identified by their content block index, which the API includes in every delta event. This correctly handles the case where the model generates multiple tool calls in a single response.
2. **Deferred parsing** -- JSON is only parsed when `content_block_stop` arrives. Attempting to parse on each delta would fail because fragments are not valid JSON.
3. **Error preservation** -- if the accumulated JSON is malformed, the error includes the raw buffer for debugging.

## Integrating with the stream processor

Now let's update the stream processing loop to handle both text and tool calls:

```rust
use crate::chunked::LineSplitter;
use crate::sse::{ContentBlockStub, Delta, SseParser, StreamEvent};
use crate::tool_accumulator::{ToolCall, ToolCallAccumulator};
use crate::renderer::TokenRenderer;

pub struct StreamOutput {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub stop_reason: Option<String>,
}

pub async fn process_stream_with_tools(
    mut byte_stream: impl futures::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
) -> Result<StreamOutput, Box<dyn std::error::Error>> {
    let mut splitter = LineSplitter::new();
    let mut parser = SseParser::new();
    let mut renderer = TokenRenderer::new();
    let mut tool_accumulator = ToolCallAccumulator::new();
    let mut stop_reason = None;

    while let Some(chunk_result) = byte_stream.next().await {
        let chunk = chunk_result?;

        for line in splitter.feed(&chunk) {
            let Some(sse_event) = parser.feed_line(&line) else {
                continue;
            };

            if sse_event.event_type == "ping" {
                continue;
            }

            let stream_event: StreamEvent = serde_json::from_str(&sse_event.data)?;

            match stream_event {
                StreamEvent::ContentBlockStart {
                    index,
                    content_block: ContentBlockStub::ToolUse { id, name },
                } => {
                    tool_accumulator.start_tool_call(index, id, name);
                }

                StreamEvent::ContentBlockDelta {
                    delta: Delta::TextDelta { text },
                    ..
                } => {
                    renderer.render_delta(&text)?;
                }

                StreamEvent::ContentBlockDelta {
                    index,
                    delta: Delta::InputJsonDelta { partial_json },
                } => {
                    tool_accumulator.append_json(index, &partial_json);
                }

                StreamEvent::ContentBlockStop { index } => {
                    if let Some(tool_call) = tool_accumulator.finish_block(index)? {
                        eprintln!("\n[Tool call: {}({})]", tool_call.name, tool_call.arguments);
                    }
                }

                StreamEvent::MessageDelta { delta, .. } => {
                    stop_reason = delta.stop_reason;
                }

                StreamEvent::MessageStop => break,

                _ => {}
            }
        }
    }

    let render_result = renderer.finish()?;

    Ok(StreamOutput {
        text: render_result.text,
        tool_calls: tool_accumulator.take_completed(),
        stop_reason,
    })
}
```

::: python Coming from Python
Python's Anthropic SDK handles tool call assembly for you behind the scenes:
```python
with client.messages.stream(...) as stream:
    response = stream.get_final_message()
    for block in response.content:
        if block.type == "tool_use":
            execute_tool(block.name, block.input)
```
You never see partial JSON fragments. In Rust, by building the accumulator yourself, you gain the ability to show partial progress ("assembling arguments for read_file..."), cancel assembly early on interrupt, and handle malformed JSON gracefully instead of crashing.
:::

## Handling multiple simultaneous tool calls

The Anthropic API can return multiple tool calls in a single response. For example, the model might want to read two files at once. The stream would look like:

```
content_block_start  (index=0, tool_use, name="read_file")
content_block_delta  (index=0, partial_json="{\"path\":\"/src/")
content_block_delta  (index=0, partial_json="main.rs\"}")
content_block_stop   (index=0)
content_block_start  (index=1, tool_use, name="read_file")
content_block_delta  (index=1, partial_json="{\"path\":\"/src/")
content_block_delta  (index=1, partial_json="lib.rs\"}")
content_block_stop   (index=1)
```

The `HashMap<usize, PartialToolCall>` in the accumulator handles this naturally -- each tool call is tracked by its content block index, so fragments are routed to the correct buffer.

## Testing the accumulator

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_tool_call_assembly() {
        let mut acc = ToolCallAccumulator::new();

        acc.start_tool_call(1, "toolu_01".to_string(), "read_file".to_string());

        acc.append_json(1, "{\"path\":");
        acc.append_json(1, " \"/src/main.rs\"}");

        let result = acc.finish_block(1).expect("should not error");
        let tool_call = result.expect("should produce a tool call");

        assert_eq!(tool_call.name, "read_file");
        assert_eq!(tool_call.arguments["path"], "/src/main.rs");
    }

    #[test]
    fn test_multiple_tool_calls() {
        let mut acc = ToolCallAccumulator::new();

        acc.start_tool_call(0, "toolu_01".to_string(), "read_file".to_string());
        acc.start_tool_call(1, "toolu_02".to_string(), "list_dir".to_string());

        acc.append_json(0, "{\"path\": \"/a.rs\"}");
        acc.append_json(1, "{\"path\": \"/src\"}");

        acc.finish_block(0).unwrap();
        acc.finish_block(1).unwrap();

        let completed = acc.take_completed();
        assert_eq!(completed.len(), 2);
        assert_eq!(completed[0].name, "read_file");
        assert_eq!(completed[1].name, "list_dir");
    }

    #[test]
    fn test_invalid_json_error() {
        let mut acc = ToolCallAccumulator::new();

        acc.start_tool_call(0, "toolu_01".to_string(), "read_file".to_string());
        acc.append_json(0, "{invalid json");

        let result = acc.finish_block(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_non_tool_block_ignored() {
        let mut acc = ToolCallAccumulator::new();
        // finish_block for an index that was never started
        let result = acc.finish_block(5).unwrap();
        assert!(result.is_none());
    }
}
```

::: wild In the Wild
Claude Code handles partial tool call assembly with an important optimization: it starts preparing for tool execution before the JSON is complete. When it sees a `content_block_start` with a `read_file` tool, it can begin resolving the file path as soon as the `path` field appears in the JSON stream, even before the closing brace arrives. This kind of speculative preparation shaves hundreds of milliseconds off tool execution latency. For your agent, the simpler approach of waiting for the complete JSON is the right starting point.
:::

## Key Takeaways

- Tool call arguments are streamed as `input_json_delta` fragments that are individually unparseable -- you must concatenate all fragments before attempting to parse the JSON.
- The `ToolCallAccumulator` tracks in-progress tool calls by content block index, supporting multiple simultaneous tool calls in a single response.
- JSON parsing happens only at `content_block_stop`, when you know all fragments have been received.
- Error handling preserves the raw JSON buffer for debugging when parsing fails.
- The accumulator integrates cleanly alongside text rendering in the stream processing loop -- text deltas go to the renderer, JSON deltas go to the accumulator.
