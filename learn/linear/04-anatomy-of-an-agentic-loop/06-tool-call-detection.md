---
title: Tool Call Detection
description: How to identify tool use requests in the LLM response, distinguish them from plain text, and extract parameters for execution.
---

# Tool Call Detection

> **What you'll learn:**
> - How tool calls are signaled in LLM responses via stop reasons and content block types
> - How to parse tool call parameters from JSON and validate them against the tool's schema
> - How to handle edge cases like partial tool calls in streaming, multiple tool calls, and malformed JSON

Tool call detection is the transition from Processing to ToolDetected in our state machine. After the LLM responds, your code must examine the response and answer one question: does the model want to execute any tools? If yes, you extract the tool calls and their parameters. If no, you have a final text response to show the user.

This sounds straightforward, but the details matter. Tool calls arrive as structured data embedded in the response stream, and they can be interleaved with text, come in multiples, or contain malformed parameters. Robust detection and parsing is what separates an agent that works reliably from one that breaks on edge cases.

## How Tool Calls Are Signaled

In the Anthropic API, the model signals tool use in two ways:

1. **Stop reason** -- The response includes a `stop_reason` field. If it is `"tool_use"`, the model wants to execute at least one tool. If it is `"end_turn"`, the model is providing a final response.

2. **Content blocks** -- The response body contains an array of content blocks. Each block has a `type` field: `"text"` for plain text, `"tool_use"` for a tool call. A single response can contain multiple blocks of both types.

Here is what a response with a tool call looks like in JSON:

```json
{
  "id": "msg_01XYZ",
  "type": "message",
  "role": "assistant",
  "content": [
    {
      "type": "text",
      "text": "I'll read that file for you."
    },
    {
      "type": "tool_use",
      "id": "toolu_01ABC",
      "name": "read_file",
      "input": {
        "path": "src/main.rs"
      }
    }
  ],
  "stop_reason": "tool_use",
  "usage": {
    "input_tokens": 1250,
    "output_tokens": 47
  }
}
```

The model has done two things in one response: explained what it is about to do ("I'll read that file for you") and requested a tool execution. Your code needs to handle both the text (display it to the user) and the tool call (execute it and feed the result back).

## Parsing Tool Calls

After receiving the complete response (either from streaming assembly or a non-streaming call), you extract the tool calls:

```rust
struct ToolCall {
    id: String,
    name: String,
    input: serde_json::Value,
}

struct DetectionResult {
    text_content: String,
    tool_calls: Vec<ToolCall>,
    should_continue: bool,
}

fn detect_tool_calls(response: &LlmResponse) -> DetectionResult {
    let mut text_parts: Vec<String> = Vec::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();

    for block in &response.content_blocks {
        match block {
            ContentBlock::Text { text } => {
                text_parts.push(text.clone());
            }
            ContentBlock::ToolUse { id, name, input } => {
                tool_calls.push(ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                });
            }
        }
    }

    let should_continue = response.stop_reason == "tool_use" && !tool_calls.is_empty();

    DetectionResult {
        text_content: text_parts.join("\n"),
        tool_calls,
        should_continue,
    }
}
```

The `should_continue` flag is what drives the inner loop. If it is `true`, the agent transitions to `ToolDetected` and eventually executes the tools. If `false`, the agent transitions to `Done` and displays the text to the user.

Notice the defensive check: `response.stop_reason == "tool_use" && !tool_calls.is_empty()`. In theory, a `tool_use` stop reason always comes with tool call blocks. In practice, you should never assume that -- validate both signals.

## Validating Tool Call Parameters

The model generates tool call parameters as JSON, but this JSON might not match what your tools expect. The model can hallucinate parameter names, use wrong types, or omit required fields. Validation catches these issues before execution:

```rust
use serde_json::Value;

struct ToolSchema {
    name: String,
    required_params: Vec<String>,
    param_types: std::collections::HashMap<String, ParamType>,
}

enum ParamType {
    String,
    Integer,
    Boolean,
    Object,
    Array,
}

enum ValidationError {
    UnknownTool(String),
    MissingParam { tool: String, param: String },
    WrongType { tool: String, param: String, expected: String, got: String },
    InvalidJson(String),
}

fn validate_tool_call(
    call: &ToolCall,
    schemas: &[ToolSchema],
) -> Result<(), ValidationError> {
    // Find the schema for this tool
    let schema = schemas
        .iter()
        .find(|s| s.name == call.name)
        .ok_or_else(|| ValidationError::UnknownTool(call.name.clone()))?;

    // Check that input is an object
    let params = call.input.as_object()
        .ok_or_else(|| ValidationError::InvalidJson(
            format!("Tool {} input is not a JSON object", call.name)
        ))?;

    // Check required parameters
    for required in &schema.required_params {
        if !params.contains_key(required) {
            return Err(ValidationError::MissingParam {
                tool: call.name.clone(),
                param: required.clone(),
            });
        }
    }

    // Check parameter types
    for (param_name, expected_type) in &schema.param_types {
        if let Some(value) = params.get(param_name) {
            let type_matches = match expected_type {
                ParamType::String => value.is_string(),
                ParamType::Integer => value.is_i64() || value.is_u64(),
                ParamType::Boolean => value.is_boolean(),
                ParamType::Object => value.is_object(),
                ParamType::Array => value.is_array(),
            };

            if !type_matches {
                return Err(ValidationError::WrongType {
                    tool: call.name.clone(),
                    param: param_name.clone(),
                    expected: format!("{:?}", expected_type),
                    got: json_type_name(value).to_string(),
                });
            }
        }
    }

    Ok(())
}

fn json_type_name(value: &Value) -> &str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
```

When validation fails, you have a choice: reject the tool call and feed the error back to the model (so it can try again), or attempt a best-effort fix (e.g., coerce a string `"true"` to a boolean `true`). Most production agents choose the first approach -- let the model self-correct based on the error message.

::: python Coming from Python
In Python, you might use Pydantic models or `jsonschema` validation to check tool parameters. Rust does not have Pydantic, but `serde_json` combined with `schemars` provides similar capability. The key difference is timing: Python validates at runtime and throws exceptions, while Rust's type system catches many issues at compile time. For dynamic JSON from an LLM, both approaches end up doing runtime validation -- but Rust's `Result` type ensures you handle the validation failure explicitly rather than letting an exception propagate uncaught.
:::

## Handling Unknown Tools

Sometimes the model hallucinates a tool that does not exist. This is more common than you might expect, especially when the model has been exposed to many tools in training but your agent only provides a few. The model might try to call `search_web` when your agent only has `read_file` and `run_command`.

The correct response is to return an error as the tool result:

```rust
fn handle_unknown_tool(call: &ToolCall) -> ToolResult {
    ToolResult {
        tool_use_id: call.id.clone(),
        content: format!(
            "Error: Unknown tool '{}'. Available tools are: read_file, run_command, write_file. \
             Please use one of these tools instead.",
            call.name
        ),
        is_error: true,
    }
}
```

By listing the available tools in the error message, you give the model the information it needs to self-correct on the next iteration. The model will typically choose the correct tool on its next attempt.

## Multiple Tool Calls in One Response

Modern LLM APIs support multiple tool calls in a single response. The model might decide it needs to read three files to understand a codebase, and it requests all three reads at once rather than one at a time:

```json
{
  "content": [
    {
      "type": "text",
      "text": "Let me read the relevant source files."
    },
    {
      "type": "tool_use",
      "id": "toolu_01",
      "name": "read_file",
      "input": {"path": "src/main.rs"}
    },
    {
      "type": "tool_use",
      "id": "toolu_02",
      "name": "read_file",
      "input": {"path": "src/lib.rs"}
    },
    {
      "type": "tool_use",
      "id": "toolu_03",
      "name": "read_file",
      "input": {"path": "Cargo.toml"}
    }
  ],
  "stop_reason": "tool_use"
}
```

Your detection code already handles this -- it collects all `tool_use` blocks into the `tool_calls` vector. The interesting design decision comes in the dispatch phase: do you execute these tools sequentially or in parallel? We will address that in the next subchapter.

Each tool call has a unique `id`. This is critical for matching results back to calls. When you send tool results back to the model, each result must reference the `tool_use_id` of the call it responds to. If you mix these up, the model will misinterpret which result goes with which request.

## Streaming Complications

When using streaming, tool calls arrive incrementally. You might receive the tool name in one chunk and the input JSON spread across many chunks. The response assembler from the LLM Invocation subchapter handles this, but there are specific complications for tool detection:

**Partial JSON** -- The tool's input parameters arrive as `partial_json` deltas during streaming. You accumulate these fragments and parse the complete JSON only after the `content_block_stop` event. Attempting to parse mid-stream will fail because the JSON is incomplete.

**Interleaved blocks** -- Text and tool use blocks can be interleaved in the stream. The `content_block_start` event tells you which type of block is beginning, and the `index` field tracks which block subsequent deltas belong to.

**Early detection** -- You know a tool call is coming as soon as you see a `content_block_start` event with `type: "tool_use"`. You can use this to prepare the UI (e.g., showing a "tool executing" indicator) even before the full parameters have arrived.

```rust
fn handle_stream_event(
    event: &StreamEvent,
    assembler: &mut ResponseAssembler,
    ui: &mut UserInterface,
) {
    match event.event_type.as_str() {
        "content_block_start" => {
            if let Some(block) = &event.content_block {
                if block.block_type == "tool_use" {
                    // We know a tool call is coming
                    ui.show_tool_indicator(&block.name.as_deref().unwrap_or("unknown"));
                }
            }
        }
        "content_block_delta" => {
            // Accumulate partial data -- text or JSON fragments
            assembler.process_event(event);
        }
        "content_block_stop" => {
            // Block is complete -- tool input JSON is now fully available
            assembler.process_event(event);
        }
        _ => {
            assembler.process_event(event);
        }
    }
}
```

::: wild In the Wild
Claude Code streams tool calls to the terminal in real-time. As soon as it detects a tool use block starting, it shows the tool name and begins rendering a formatted display of the tool execution. The tool parameters are parsed from the accumulated partial JSON only after the block is complete. OpenCode follows a similar pattern in its TUI, showing tool execution progress as the stream arrives.
:::

## The Detection Decision Point

After detection, the agentic loop reaches its critical decision point:

```rust
fn process_llm_response(response: LlmResponse) -> AgentState {
    let detection = detect_tool_calls(&response);

    if detection.should_continue {
        // Display any text the model included alongside tool calls
        if !detection.text_content.is_empty() {
            println!("{}", detection.text_content);
        }
        AgentState::ToolDetected {
            tool_calls: detection.tool_calls,
        }
    } else {
        AgentState::Done {
            response: detection.text_content,
        }
    }
}
```

This is a small function, but it is the branching point of the entire agentic loop. Every iteration passes through here, and the `should_continue` flag determines whether the agent does more work or stops. The rest of the chapter -- tool dispatch, observation collection, response generation -- only executes when this detection says "the model wants tools."

## Key Takeaways

- Tool calls are signaled through two mechanisms: the `stop_reason` field (`"tool_use"` vs `"end_turn"`) and `tool_use` content blocks in the response body -- always check both
- Tool call parameters are LLM-generated JSON that must be validated against the tool's schema before execution, because models can hallucinate parameters, use wrong types, or call tools that do not exist
- Multiple tool calls can appear in a single response, each with a unique `id` that must be matched to its corresponding result when feeding observations back to the model
- Streaming adds complexity because tool parameters arrive as partial JSON fragments that must be fully accumulated before parsing
- The detection decision point is the single most important branch in the agentic loop -- it determines whether the inner loop continues or the agent returns a response to the user
