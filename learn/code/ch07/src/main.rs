// Chapter 7: Streaming Responses — Code snapshot
//
// Demonstrates the full streaming pipeline:
//   HTTP bytes -> LineSplitter -> SseParser -> StreamEvent -> render/accumulate
//
// Builds on ch06 by replacing the batch API call with real-time SSE streaming,
// printing tokens as they arrive and assembling tool calls from partial JSON.

use std::collections::HashMap;
use std::io::{self, Write};

use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::Value;

// ---------------------------------------------------------------------------
// SSE data types — typed representation of Anthropic streaming events
// ---------------------------------------------------------------------------

/// A single parsed SSE event with its event type and data payload.
#[derive(Debug, Clone)]
struct SseEvent {
    event_type: String,
    data: String,
}

/// All streaming event types emitted by the Anthropic Messages API.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
enum StreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: MessageShell },

    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: usize,
        content_block: ContentBlockStub,
    },

    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: usize, delta: Delta },

    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: usize },

    #[serde(rename = "message_delta")]
    MessageDelta {
        delta: MessageDeltaBody,
        usage: OutputUsage,
    },

    #[serde(rename = "message_stop")]
    MessageStop,

    #[serde(rename = "ping")]
    Ping,

    #[serde(rename = "error")]
    Error { error: ApiError },
}

#[derive(Debug, Clone, Deserialize)]
struct MessageShell {
    id: String,
    model: String,
    #[allow(dead_code)]
    role: String,
    usage: InputUsage,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
enum ContentBlockStub {
    #[serde(rename = "text")]
    Text {
        #[allow(dead_code)]
        text: String,
    },
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
enum Delta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Clone, Deserialize)]
struct MessageDeltaBody {
    stop_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct InputUsage {
    input_tokens: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct OutputUsage {
    output_tokens: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiError {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

// ---------------------------------------------------------------------------
// LineSplitter — buffers HTTP chunks into complete lines for the SSE parser
// ---------------------------------------------------------------------------

struct LineSplitter {
    buffer: String,
}

impl LineSplitter {
    fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    /// Feed a chunk of bytes. Returns all complete lines extracted so far.
    /// Incomplete trailing data stays buffered until the next chunk.
    fn feed(&mut self, chunk: &[u8]) -> Vec<String> {
        let text = match std::str::from_utf8(chunk) {
            Ok(s) => s,
            Err(_) => &String::from_utf8_lossy(chunk),
        };
        self.buffer.push_str(text);

        let mut lines = Vec::new();
        while let Some(pos) = self.buffer.find('\n') {
            let line = self.buffer[..pos].to_string();
            self.buffer = self.buffer[pos + 1..].to_string();
            let line = line.strip_suffix('\r').unwrap_or(&line).to_string();
            lines.push(line);
        }
        lines
    }
}

// ---------------------------------------------------------------------------
// SseParser — converts lines into SseEvent structs (event type + data)
// ---------------------------------------------------------------------------

struct SseParser {
    event_type: Option<String>,
    data_lines: Vec<String>,
}

impl SseParser {
    fn new() -> Self {
        Self {
            event_type: None,
            data_lines: Vec::new(),
        }
    }

    /// Feed one line. Returns `Some(SseEvent)` when a blank line completes an event.
    fn feed_line(&mut self, line: &str) -> Option<SseEvent> {
        // Blank line — dispatch the accumulated event
        if line.is_empty() {
            return self.dispatch();
        }
        // Comment lines (`:` prefix) are ignored (keepalives, etc.)
        if line.starts_with(':') {
            return None;
        }
        // Split on the first `:` to separate field name from value
        let (field, value) = if let Some(pos) = line.find(':') {
            let f = &line[..pos];
            let v = &line[pos + 1..];
            (f, v.strip_prefix(' ').unwrap_or(v))
        } else {
            (line, "")
        };

        match field {
            "event" => self.event_type = Some(value.to_string()),
            "data" => self.data_lines.push(value.to_string()),
            _ => { /* id, retry, unknown — ignored for now */ }
        }
        None
    }

    fn dispatch(&mut self) -> Option<SseEvent> {
        if self.data_lines.is_empty() && self.event_type.is_none() {
            return None;
        }
        let event = SseEvent {
            event_type: self
                .event_type
                .take()
                .unwrap_or_else(|| "message".to_string()),
            data: self.data_lines.join("\n"),
        };
        self.data_lines.clear();
        Some(event)
    }
}

// ---------------------------------------------------------------------------
// ToolCallAccumulator — buffers partial JSON fragments for tool_use blocks
// ---------------------------------------------------------------------------

/// A fully assembled, ready-to-execute tool call.
#[derive(Debug, Clone)]
struct ToolCall {
    id: String,
    name: String,
    arguments: Value,
}

struct PartialToolCall {
    id: String,
    name: String,
    json_buffer: String,
}

struct ToolCallAccumulator {
    /// In-progress tool calls keyed by content block index.
    active: HashMap<usize, PartialToolCall>,
    /// Completed tool calls ready for execution.
    completed: Vec<ToolCall>,
}

impl ToolCallAccumulator {
    fn new() -> Self {
        Self {
            active: HashMap::new(),
            completed: Vec::new(),
        }
    }

    /// Begin tracking a new tool_use content block.
    fn start_tool_call(&mut self, index: usize, id: String, name: String) {
        self.active.insert(
            index,
            PartialToolCall {
                id,
                name,
                json_buffer: String::new(),
            },
        );
    }

    /// Append a JSON fragment for the given content block index.
    fn append_json(&mut self, index: usize, partial_json: &str) {
        if let Some(partial) = self.active.get_mut(&index) {
            partial.json_buffer.push_str(partial_json);
        }
    }

    /// Finalize the tool call when content_block_stop arrives.
    /// Parses the accumulated JSON and moves the call to `completed`.
    fn finish_block(&mut self, index: usize) -> Result<Option<ToolCall>, String> {
        let partial = match self.active.remove(&index) {
            Some(p) => p,
            None => return Ok(None), // Not a tool_use block
        };

        let arguments: Value = serde_json::from_str(&partial.json_buffer).map_err(|e| {
            format!(
                "Invalid JSON for tool '{}': {} (received: '{}')",
                partial.name, e, partial.json_buffer
            )
        })?;

        let tool_call = ToolCall {
            id: partial.id,
            name: partial.name,
            arguments,
        };
        self.completed.push(tool_call.clone());
        Ok(Some(tool_call))
    }

    /// Drain and return all completed tool calls.
    fn take_completed(&mut self) -> Vec<ToolCall> {
        std::mem::take(&mut self.completed)
    }
}

// ---------------------------------------------------------------------------
// StreamOutput — the accumulated result after the full stream completes
// ---------------------------------------------------------------------------

struct StreamOutput {
    /// The full assistant text content, accumulated from text deltas.
    text: String,
    /// All tool calls assembled from partial JSON fragments.
    tool_calls: Vec<ToolCall>,
    /// The stop reason: "end_turn", "tool_use", etc.
    stop_reason: Option<String>,
    /// Token usage reported by the API.
    input_tokens: u32,
    output_tokens: u32,
}

// ---------------------------------------------------------------------------
// Core streaming function — processes the SSE byte stream end-to-end
// ---------------------------------------------------------------------------

/// Process a streaming response from the Anthropic API.
///
/// Pipeline: HTTP chunks -> LineSplitter -> SseParser -> StreamEvent
///
/// Text deltas are printed to stdout immediately (flushed per token).
/// Tool call JSON fragments are buffered and assembled on block completion.
/// The full text and tool calls are accumulated and returned for conversation history.
async fn handle_stream(response: reqwest::Response) -> Result<StreamOutput, Box<dyn std::error::Error>> {
    let mut splitter = LineSplitter::new();
    let mut parser = SseParser::new();
    let mut tool_acc = ToolCallAccumulator::new();

    let mut full_text = String::new();
    let mut stop_reason: Option<String> = None;
    let mut input_tokens: u32 = 0;
    let mut output_tokens: u32 = 0;

    let mut byte_stream = response.bytes_stream();

    while let Some(chunk_result) = byte_stream.next().await {
        let chunk = chunk_result?;
        let lines = splitter.feed(&chunk);

        for line in lines {
            let Some(sse_event) = parser.feed_line(&line) else {
                continue;
            };

            // Skip keepalive pings
            if sse_event.event_type == "ping" {
                continue;
            }

            // Deserialize the SSE data payload into a typed StreamEvent
            let stream_event: StreamEvent = match serde_json::from_str(&sse_event.data) {
                Ok(ev) => ev,
                Err(e) => {
                    eprintln!(
                        "\n[Warning: failed to parse {} event: {}]",
                        sse_event.event_type, e
                    );
                    continue; // Skip malformed events
                }
            };

            match stream_event {
                // -- message_start: capture message metadata and input usage --
                StreamEvent::MessageStart { message } => {
                    input_tokens = message.usage.input_tokens;
                    eprintln!(
                        "[model: {}, id: {}, input_tokens: {}]",
                        message.model, message.id, input_tokens
                    );
                }

                // -- content_block_start: begin a text or tool_use block --
                StreamEvent::ContentBlockStart {
                    index,
                    content_block,
                } => match content_block {
                    ContentBlockStub::Text { .. } => {
                        // Text block starting — nothing to do yet, deltas will follow
                    }
                    ContentBlockStub::ToolUse { id, name } => {
                        eprintln!("\n[Tool call starting: {}]", name);
                        tool_acc.start_tool_call(index, id, name);
                    }
                },

                // -- content_block_delta: the workhorse event --
                StreamEvent::ContentBlockDelta { index, delta } => match delta {
                    // Text token — print immediately and accumulate
                    Delta::TextDelta { text } => {
                        print!("{}", text);
                        io::stdout().flush()?;
                        full_text.push_str(&text);
                    }
                    // Tool call JSON fragment — buffer for later assembly
                    Delta::InputJsonDelta { partial_json } => {
                        tool_acc.append_json(index, &partial_json);
                    }
                },

                // -- content_block_stop: finalize the content block --
                StreamEvent::ContentBlockStop { index } => {
                    match tool_acc.finish_block(index) {
                        Ok(Some(tool_call)) => {
                            eprintln!(
                                "\n[Tool call complete: {}({})]",
                                tool_call.name, tool_call.arguments
                            );
                        }
                        Ok(None) => {
                            // Text block ended — nothing special to do
                        }
                        Err(e) => {
                            eprintln!("\n[Tool call assembly error: {}]", e);
                        }
                    }
                }

                // -- message_delta: stop reason and output token count --
                StreamEvent::MessageDelta { delta, usage } => {
                    stop_reason = delta.stop_reason;
                    output_tokens = usage.output_tokens;
                }

                // -- message_stop: stream complete --
                StreamEvent::MessageStop => {
                    println!(); // Final newline after streamed text
                }

                // -- ping: already filtered above, but match for completeness --
                StreamEvent::Ping => {}

                // -- error: API-level error event --
                StreamEvent::Error { error } => {
                    eprintln!(
                        "\n[API error: {} — {}]",
                        error.error_type, error.message
                    );
                    return Err(format!(
                        "API error ({}): {}",
                        error.error_type, error.message
                    )
                    .into());
                }
            }
        }
    }

    Ok(StreamOutput {
        text: full_text,
        tool_calls: tool_acc.take_completed(),
        stop_reason,
        input_tokens,
        output_tokens,
    })
}

// ---------------------------------------------------------------------------
// API request helpers
// ---------------------------------------------------------------------------

/// Build the conversation messages array for the API.
fn build_messages(conversation: &[Value]) -> Value {
    Value::Array(conversation.to_vec())
}

/// Send a streaming request to the Anthropic Messages API.
/// Returns the raw HTTP response whose body is an SSE byte stream.
async fn send_streaming_request(
    client: &reqwest::Client,
    api_key: &str,
    messages: &[Value],
    tools: &[Value],
) -> Result<reqwest::Response, reqwest::Error> {
    let mut body = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 4096,
        "stream": true,
        "messages": build_messages(messages),
    });

    // Include tools only if there are any defined
    if !tools.is_empty() {
        body["tools"] = Value::Array(tools.to_vec());
    }

    client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?
        .error_for_status()
}

/// Convert a StreamOutput into an assistant message for conversation history.
fn assistant_message_from_output(output: &StreamOutput) -> Value {
    let mut content = Vec::new();

    // Add text content block if present
    if !output.text.is_empty() {
        content.push(serde_json::json!({
            "type": "text",
            "text": output.text,
        }));
    }

    // Add tool_use content blocks
    for tc in &output.tool_calls {
        content.push(serde_json::json!({
            "type": "tool_use",
            "id": tc.id,
            "name": tc.name,
            "input": tc.arguments,
        }));
    }

    serde_json::json!({
        "role": "assistant",
        "content": content,
    })
}

/// Build a tool_result message for the conversation history.
fn tool_result_message(tool_call_id: &str, result: &str) -> Value {
    serde_json::json!({
        "role": "user",
        "content": [{
            "type": "tool_result",
            "tool_use_id": tool_call_id,
            "content": result,
        }],
    })
}

/// Execute a tool call (stub — returns a placeholder result).
fn execute_tool(tool_call: &ToolCall) -> String {
    match tool_call.name.as_str() {
        "get_weather" => {
            let location = tool_call
                .arguments
                .get("location")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            format!("Weather in {}: 72°F, sunny", location)
        }
        _ => format!(
            "Tool '{}' executed with args: {}",
            tool_call.name, tool_call.arguments
        ),
    }
}

// ---------------------------------------------------------------------------
// main — the streaming-enabled agentic loop
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Chapter 7: Streaming Responses\n");

    // Read API key from environment
    let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_else(|_| {
        eprintln!("Warning: ANTHROPIC_API_KEY not set. Set it to make real API calls.");
        "test-key".to_string()
    });

    let client = reqwest::Client::new();

    // Define an example tool so we can demonstrate tool_use streaming
    let tools = vec![serde_json::json!({
        "name": "get_weather",
        "description": "Get the current weather for a given location.",
        "input_schema": {
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "City and state, e.g. San Francisco, CA"
                }
            },
            "required": ["location"]
        }
    })];

    // Conversation history
    let mut messages: Vec<Value> = vec![serde_json::json!({
        "role": "user",
        "content": "What's the weather like in San Francisco? After getting the weather, give me a brief summary."
    })];

    // --- Agentic loop with streaming ---
    // Each iteration sends the conversation to the API with stream: true,
    // processes SSE events in real time, and loops back for tool results.
    loop {
        eprintln!("\n--- Sending streaming request ---");

        let response = match send_streaming_request(&client, &api_key, &messages, &tools).await {
            Ok(resp) => resp,
            Err(e) => {
                eprintln!("Request failed: {}", e);
                break;
            }
        };

        // Process the SSE stream: prints tokens as they arrive
        let output = match handle_stream(response).await {
            Ok(out) => out,
            Err(e) => {
                eprintln!("Stream processing error: {}", e);
                break;
            }
        };

        eprintln!(
            "[stop_reason: {:?}, input_tokens: {}, output_tokens: {}]",
            output.stop_reason, output.input_tokens, output.output_tokens
        );

        // Append the assistant's response to conversation history
        messages.push(assistant_message_from_output(&output));

        // Check the stop reason to decide what to do next
        match output.stop_reason.as_deref() {
            Some("tool_use") => {
                // Execute each tool call and add results to conversation
                for tool_call in &output.tool_calls {
                    eprintln!(
                        "\n--- Executing tool: {} ---",
                        tool_call.name
                    );
                    let result = execute_tool(tool_call);
                    eprintln!("[Tool result: {}]", result);
                    messages.push(tool_result_message(&tool_call.id, &result));
                }
                // Loop back to send tool results to the API
            }
            Some("end_turn") | None => {
                // Model finished — exit the agentic loop
                eprintln!("\n--- Stream complete ---");
                break;
            }
            Some(other) => {
                eprintln!("\n--- Unexpected stop reason: {} ---", other);
                break;
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- LineSplitter tests --

    #[test]
    fn test_line_splitter_complete_lines() {
        let mut splitter = LineSplitter::new();
        let lines = splitter.feed(b"hello\nworld\n");
        assert_eq!(lines, vec!["hello", "world"]);
    }

    #[test]
    fn test_line_splitter_partial_line() {
        let mut splitter = LineSplitter::new();
        let lines1 = splitter.feed(b"hel");
        assert!(lines1.is_empty());
        let lines2 = splitter.feed(b"lo\n");
        assert_eq!(lines2, vec!["hello"]);
    }

    #[test]
    fn test_line_splitter_crlf() {
        let mut splitter = LineSplitter::new();
        let lines = splitter.feed(b"line one\r\nline two\r\n");
        assert_eq!(lines, vec!["line one", "line two"]);
    }

    #[test]
    fn test_line_splitter_split_across_chunks() {
        let mut splitter = LineSplitter::new();
        let l1 = splitter.feed(b"event: content_block_delta\ndata: {\"partial");
        assert_eq!(l1, vec!["event: content_block_delta"]);
        let l2 = splitter.feed(b"_data\"}\n\n");
        assert_eq!(l2, vec!["data: {\"partial_data\"}", ""]);
    }

    // -- SseParser tests --

    #[test]
    fn test_sse_parser_simple_event() {
        let mut parser = SseParser::new();
        assert!(parser.feed_line("event: message_stop").is_none());
        assert!(parser
            .feed_line("data: {\"type\":\"message_stop\"}")
            .is_none());
        let event = parser.feed_line("").expect("should dispatch");
        assert_eq!(event.event_type, "message_stop");
        assert_eq!(event.data, "{\"type\":\"message_stop\"}");
    }

    #[test]
    fn test_sse_parser_comment_ignored() {
        let mut parser = SseParser::new();
        assert!(parser.feed_line(": keepalive ping").is_none());
        assert!(parser.feed_line("").is_none()); // No event to dispatch
    }

    #[test]
    fn test_sse_parser_text_delta() {
        let mut parser = SseParser::new();
        parser.feed_line("event: content_block_delta");
        parser.feed_line(
            r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#,
        );
        let event = parser.feed_line("").expect("should dispatch");
        assert_eq!(event.event_type, "content_block_delta");

        let stream_event: StreamEvent =
            serde_json::from_str(&event.data).expect("should parse");
        match stream_event {
            StreamEvent::ContentBlockDelta {
                index,
                delta: Delta::TextDelta { text },
            } => {
                assert_eq!(index, 0);
                assert_eq!(text, "Hello");
            }
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn test_sse_parser_multi_line_data() {
        let mut parser = SseParser::new();
        parser.feed_line("data: line one");
        parser.feed_line("data: line two");
        let event = parser.feed_line("").expect("should dispatch");
        assert_eq!(event.data, "line one\nline two");
    }

    // -- StreamEvent deserialization tests --

    #[test]
    fn test_deserialize_message_start() {
        let json = r#"{"type":"message_start","message":{"id":"msg_01","model":"claude-sonnet-4-20250514","role":"assistant","content":[],"usage":{"input_tokens":25}}}"#;
        let event: StreamEvent = serde_json::from_str(json).unwrap();
        match event {
            StreamEvent::MessageStart { message } => {
                assert_eq!(message.id, "msg_01");
                assert_eq!(message.usage.input_tokens, 25);
            }
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn test_deserialize_content_block_start_tool_use() {
        let json = r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_01","name":"get_weather"}}"#;
        let event: StreamEvent = serde_json::from_str(json).unwrap();
        match event {
            StreamEvent::ContentBlockStart {
                index,
                content_block: ContentBlockStub::ToolUse { id, name },
            } => {
                assert_eq!(index, 1);
                assert_eq!(id, "toolu_01");
                assert_eq!(name, "get_weather");
            }
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn test_deserialize_input_json_delta() {
        let json = r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"location\":"}}"#;
        let event: StreamEvent = serde_json::from_str(json).unwrap();
        match event {
            StreamEvent::ContentBlockDelta {
                index,
                delta: Delta::InputJsonDelta { partial_json },
            } => {
                assert_eq!(index, 1);
                assert_eq!(partial_json, "{\"location\":");
            }
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn test_deserialize_message_delta() {
        let json = r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":42}}"#;
        let event: StreamEvent = serde_json::from_str(json).unwrap();
        match event {
            StreamEvent::MessageDelta { delta, usage } => {
                assert_eq!(delta.stop_reason.as_deref(), Some("end_turn"));
                assert_eq!(usage.output_tokens, 42);
            }
            other => panic!("unexpected: {:?}", other),
        }
    }

    // -- ToolCallAccumulator tests --

    #[test]
    fn test_tool_call_assembly() {
        let mut acc = ToolCallAccumulator::new();
        acc.start_tool_call(1, "toolu_01".into(), "get_weather".into());
        acc.append_json(1, "{\"location\":");
        acc.append_json(1, " \"San Francisco, CA\"}");
        let tc = acc.finish_block(1).unwrap().expect("should produce tool call");
        assert_eq!(tc.name, "get_weather");
        assert_eq!(tc.arguments["location"], "San Francisco, CA");
    }

    #[test]
    fn test_tool_call_multiple() {
        let mut acc = ToolCallAccumulator::new();
        acc.start_tool_call(0, "t1".into(), "get_weather".into());
        acc.start_tool_call(1, "t2".into(), "get_weather".into());
        acc.append_json(0, "{\"location\": \"NYC\"}");
        acc.append_json(1, "{\"location\": \"LA\"}");
        acc.finish_block(0).unwrap();
        acc.finish_block(1).unwrap();
        let completed = acc.take_completed();
        assert_eq!(completed.len(), 2);
        assert_eq!(completed[0].arguments["location"], "NYC");
        assert_eq!(completed[1].arguments["location"], "LA");
    }

    #[test]
    fn test_tool_call_invalid_json() {
        let mut acc = ToolCallAccumulator::new();
        acc.start_tool_call(0, "t1".into(), "bad_tool".into());
        acc.append_json(0, "{not valid json");
        assert!(acc.finish_block(0).is_err());
    }

    #[test]
    fn test_tool_call_non_tool_block_ignored() {
        let mut acc = ToolCallAccumulator::new();
        let result = acc.finish_block(99).unwrap();
        assert!(result.is_none());
    }

    // -- Integration: full SSE event sequence through all layers --

    #[test]
    fn test_full_pipeline_text_stream() {
        let raw_sse = concat!(
            "event: message_start\n",
            "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_01\",\"model\":\"claude-sonnet-4-20250514\",\"role\":\"assistant\",\"content\":[],\"usage\":{\"input_tokens\":10}}}\n",
            "\n",
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n",
            "\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n",
            "\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" world\"}}\n",
            "\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":0}\n",
            "\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":5}}\n",
            "\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n",
            "\n",
        );

        let mut splitter = LineSplitter::new();
        let mut parser = SseParser::new();
        let mut full_text = String::new();
        let mut stop_reason = None;

        let lines = splitter.feed(raw_sse.as_bytes());
        for line in lines {
            if let Some(sse_event) = parser.feed_line(&line) {
                if sse_event.event_type == "ping" {
                    continue;
                }
                let stream_event: StreamEvent =
                    serde_json::from_str(&sse_event.data).unwrap();
                match stream_event {
                    StreamEvent::ContentBlockDelta {
                        delta: Delta::TextDelta { text },
                        ..
                    } => {
                        full_text.push_str(&text);
                    }
                    StreamEvent::MessageDelta { delta, .. } => {
                        stop_reason = delta.stop_reason;
                    }
                    _ => {}
                }
            }
        }

        assert_eq!(full_text, "Hello world");
        assert_eq!(stop_reason.as_deref(), Some("end_turn"));
    }

    #[test]
    fn test_full_pipeline_tool_use_stream() {
        let raw_sse = concat!(
            "event: message_start\n",
            "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_02\",\"model\":\"claude-sonnet-4-20250514\",\"role\":\"assistant\",\"content\":[],\"usage\":{\"input_tokens\":15}}}\n",
            "\n",
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_abc\",\"name\":\"get_weather\"}}\n",
            "\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"location\\\": \\\"\"}}\n",
            "\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"San Francisco, CA\\\"}\"}}\n",
            "\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":0}\n",
            "\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":20}}\n",
            "\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n",
            "\n",
        );

        let mut splitter = LineSplitter::new();
        let mut parser = SseParser::new();
        let mut tool_acc = ToolCallAccumulator::new();
        let mut stop_reason = None;

        let lines = splitter.feed(raw_sse.as_bytes());
        for line in lines {
            if let Some(sse_event) = parser.feed_line(&line) {
                if sse_event.event_type == "ping" {
                    continue;
                }
                let stream_event: StreamEvent =
                    serde_json::from_str(&sse_event.data).unwrap();
                match stream_event {
                    StreamEvent::ContentBlockStart {
                        index,
                        content_block: ContentBlockStub::ToolUse { id, name },
                    } => {
                        tool_acc.start_tool_call(index, id, name);
                    }
                    StreamEvent::ContentBlockDelta {
                        index,
                        delta: Delta::InputJsonDelta { partial_json },
                    } => {
                        tool_acc.append_json(index, &partial_json);
                    }
                    StreamEvent::ContentBlockStop { index } => {
                        tool_acc.finish_block(index).unwrap();
                    }
                    StreamEvent::MessageDelta { delta, .. } => {
                        stop_reason = delta.stop_reason;
                    }
                    _ => {}
                }
            }
        }

        let completed = tool_acc.take_completed();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].name, "get_weather");
        assert_eq!(completed[0].arguments["location"], "San Francisco, CA");
        assert_eq!(stop_reason.as_deref(), Some("tool_use"));
    }
}
