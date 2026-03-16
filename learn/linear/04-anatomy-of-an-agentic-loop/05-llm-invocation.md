---
title: LLM Invocation
description: The mechanics of calling the LLM API within the agentic loop, including request construction, streaming, and response parsing.
---

# LLM Invocation

> **What you'll learn:**
> - How to construct the API request with system prompt, conversation history, and tool definitions
> - How streaming responses are parsed in real-time to provide immediate user feedback
> - The error handling required for API calls including timeouts, rate limits, and malformed responses

LLM invocation is the Processing state in our state machine -- the moment when your agent sends the assembled context to the model and waits for a response. This is the most time-consuming step in the loop (often hundreds of milliseconds to several seconds) and the most network-dependent. It is also where the model makes its decision: should I respond with text, or should I request a tool?

This subchapter covers the mechanics of the API call itself: how the request is structured, how streaming works, how to parse the response, and how to handle the many ways this call can fail.

## The API Request

Every LLM API call sends the same core payload: a model identifier, a system prompt, an array of messages, and (for agents) a list of tool definitions. Here is what the request looks like for the Anthropic Messages API:

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ToolDefinition>,
}

#[derive(Serialize, Clone)]
struct ApiMessage {
    role: String,
    content: serde_json::Value, // Can be a string or an array of content blocks
}

#[derive(Serialize, Clone)]
struct ToolDefinition {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}
```

The `content` field of a message deserves attention. For simple text messages, it is a plain string. But when the conversation includes tool calls and tool results, it becomes an array of content blocks. This is because a single assistant message can contain both text and tool use requests:

```rust
use serde_json::json;

// A simple user message
let user_msg = json!({
    "role": "user",
    "content": "Read the file src/main.rs"
});

// An assistant message with both text and a tool call
let assistant_msg = json!({
    "role": "assistant",
    "content": [
        {
            "type": "text",
            "text": "I'll read that file for you."
        },
        {
            "type": "tool_use",
            "id": "toolu_01A2B3C4",
            "name": "read_file",
            "input": {"path": "src/main.rs"}
        }
    ]
});

// A user message containing a tool result
let tool_result_msg = json!({
    "role": "user",
    "content": [
        {
            "type": "tool_result",
            "tool_use_id": "toolu_01A2B3C4",
            "content": "fn main() {\n    println!(\"hello\");\n}"
        }
    ]
});
```

Note the role assignment: tool results are sent as `user` role messages, even though they are not typed by the user. This is a protocol convention -- the Anthropic API uses `user` for all messages that are not from the assistant, including tool results and system-injected context.

::: python Coming from Python
In the Anthropic Python SDK, you pass messages as a list of dictionaries. The Rust version is structurally identical, using `serde_json::Value` for the same flexibility. The key difference is that in Python, you can build messages as loose dicts and the SDK handles serialization. In Rust, you choose between loose JSON (`serde_json::Value`) for flexibility or typed structs (`#[derive(Serialize)]`) for compile-time safety. Production agents typically use typed structs for the common cases and fall back to raw JSON for edge cases.
:::

## Streaming vs. Non-Streaming

There are two ways to receive the LLM's response: wait for the complete response (non-streaming), or process it incrementally as tokens arrive (streaming).

**Non-streaming** is simpler. You send the request, wait, and get back a complete response object. The downside: the user sees nothing until the entire response is generated, which can take many seconds for long responses.

**Streaming** sends tokens as they are generated. The user sees text appearing in real-time, which provides a much better interactive experience. The downside: your code must parse an incremental stream of events and assemble them into a complete response.

For a coding agent, streaming is essentially mandatory. Users expect to see the model "thinking" in real-time, and long tool execution sequences can take minutes -- the user needs feedback that something is happening.

Here is the basic structure of a streaming API call in Rust:

```rust
use reqwest::Client;

#[derive(Deserialize)]
struct StreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    // Different fields depending on event_type
    index: Option<usize>,
    delta: Option<Delta>,
    content_block: Option<ContentBlock>,
    message: Option<MessageResponse>,
}

#[derive(Deserialize)]
struct Delta {
    #[serde(rename = "type")]
    delta_type: Option<String>,
    text: Option<String>,
    partial_json: Option<String>,
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    id: Option<String>,
    name: Option<String>,
    text: Option<String>,
}

#[derive(Deserialize)]
struct MessageResponse {
    id: String,
    stop_reason: Option<String>,
    usage: TokenUsage,
}

#[derive(Deserialize)]
struct TokenUsage {
    input_tokens: u32,
    output_tokens: u32,
}

async fn call_llm_streaming(
    client: &Client,
    request: &ApiRequest,
    api_key: &str,
) -> Result<LlmResponse, ApiError> {
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(request)
        .send()
        .await
        .map_err(|e| ApiError::Network(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(classify_api_error(status, &body));
    }

    // Process the SSE stream
    let mut assembled = ResponseAssembler::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream_next(&mut stream).await {
        let chunk = chunk.map_err(|e| ApiError::Stream(e.to_string()))?;
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    break;
                }
                if let Ok(event) = serde_json::from_str::<StreamEvent>(data) {
                    assembled.process_event(&event);
                }
            }
        }
    }

    Ok(assembled.finish())
}

async fn stream_next(
    stream: &mut impl futures::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
) -> Option<Result<bytes::Bytes, reqwest::Error>> {
    use futures::StreamExt;
    stream.next().await
}
```

The streaming protocol uses **Server-Sent Events (SSE)** -- a simple text-based format where each event is a line starting with `data:` followed by JSON. Your code reads these events one by one and assembles them into the complete response.

## The Response Assembler

Streaming events arrive in a specific sequence. The assembler tracks the current state and builds up the complete response:

```rust
struct ResponseAssembler {
    content_blocks: Vec<AssembledBlock>,
    current_block_index: Option<usize>,
    stop_reason: Option<String>,
    usage: Option<TokenUsage>,
}

enum AssembledBlock {
    Text(String),
    ToolUse {
        id: String,
        name: String,
        input_json: String, // Accumulated partial JSON
    },
}

impl ResponseAssembler {
    fn new() -> Self {
        Self {
            content_blocks: Vec::new(),
            current_block_index: None,
            stop_reason: None,
            usage: None,
        }
    }

    fn process_event(&mut self, event: &StreamEvent) {
        match event.event_type.as_str() {
            "content_block_start" => {
                if let Some(block) = &event.content_block {
                    match block.block_type.as_str() {
                        "text" => {
                            self.content_blocks.push(AssembledBlock::Text(String::new()));
                        }
                        "tool_use" => {
                            self.content_blocks.push(AssembledBlock::ToolUse {
                                id: block.id.clone().unwrap_or_default(),
                                name: block.name.clone().unwrap_or_default(),
                                input_json: String::new(),
                            });
                        }
                        _ => {}
                    }
                    self.current_block_index = Some(self.content_blocks.len() - 1);
                }
            }
            "content_block_delta" => {
                if let (Some(idx), Some(delta)) = (self.current_block_index, &event.delta) {
                    if let Some(block) = self.content_blocks.get_mut(idx) {
                        match block {
                            AssembledBlock::Text(text) => {
                                if let Some(t) = &delta.text {
                                    text.push_str(t);
                                    // Print text as it arrives for real-time feedback
                                    print!("{}", t);
                                }
                            }
                            AssembledBlock::ToolUse { input_json, .. } => {
                                if let Some(json) = &delta.partial_json {
                                    input_json.push_str(json);
                                }
                            }
                        }
                    }
                }
            }
            "content_block_stop" => {
                self.current_block_index = None;
            }
            "message_delta" => {
                if let Some(delta) = &event.delta {
                    self.stop_reason = delta.stop_reason.clone();
                }
            }
            "message_start" => {
                if let Some(msg) = &event.message {
                    self.usage = Some(msg.usage.clone());
                }
            }
            _ => {} // Ignore unknown events
        }
    }

    fn finish(self) -> LlmResponse {
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();

        for block in self.content_blocks {
            match block {
                AssembledBlock::Text(text) => text_parts.push(text),
                AssembledBlock::ToolUse { id, name, input_json } => {
                    let input = serde_json::from_str(&input_json)
                        .unwrap_or(serde_json::Value::Null);
                    tool_calls.push(ToolCall { id, name, input });
                }
            }
        }

        LlmResponse {
            text: text_parts.join(""),
            tool_calls,
            stop_reason: self.stop_reason.unwrap_or_else(|| "end_turn".to_string()),
            usage: self.usage,
        }
    }
}
```

The key insight about streaming is the `print!("{}", t)` inside the text delta handler. This is what gives the user real-time feedback -- each token appears on screen as it arrives from the API, rather than waiting for the complete response.

## Error Handling

API calls fail in many ways. Each failure type requires a different response:

```rust
enum ApiError {
    Network(String),          // Connection refused, DNS failure, timeout
    RateLimit { retry_after: u64 },  // HTTP 429
    Overloaded,               // HTTP 529 - server overloaded
    InvalidRequest(String),   // HTTP 400 - malformed request
    Authentication(String),   // HTTP 401 - bad API key
    ContextOverflow(String),  // HTTP 400 with specific error about token limits
    Stream(String),           // Error during streaming
    ServerError(String),      // HTTP 500/502/503
}

fn classify_api_error(status: u16, body: &str) -> ApiError {
    match status {
        429 => {
            // Parse retry-after from response body or headers
            let retry_after = parse_retry_after(body).unwrap_or(30);
            ApiError::RateLimit { retry_after }
        }
        529 => ApiError::Overloaded,
        401 => ApiError::Authentication(body.to_string()),
        400 if body.contains("context_length") || body.contains("too many tokens") => {
            ApiError::ContextOverflow(body.to_string())
        }
        400 => ApiError::InvalidRequest(body.to_string()),
        500 | 502 | 503 => ApiError::ServerError(body.to_string()),
        _ => ApiError::Network(format!("HTTP {}: {}", status, body)),
    }
}

fn parse_retry_after(body: &str) -> Option<u64> {
    // Parse the retry-after value from the error response
    serde_json::from_str::<serde_json::Value>(body)
        .ok()?
        .get("error")?
        .get("retry_after")?
        .as_u64()
}
```

Rate limits and server overload are retryable -- you wait and try again. Context overflow requires action (truncate history or compact). Authentication errors are fatal for the current session. The error classification directly feeds into the retry and recovery logic that we will cover in the Error States subchapter.

::: wild In the Wild
Claude Code implements retry logic with exponential backoff for rate limits and server errors. When it hits a rate limit, it waits for the `retry-after` duration specified by the API, then retries. For overloaded errors (HTTP 529), it uses exponential backoff starting at 1 second. OpenCode implements similar retry logic in its provider abstraction layer, where each LLM provider (Anthropic, OpenAI, etc.) has its own error handling and retry strategy.
:::

## The Complete LLM Response

After the stream is fully processed (or after the non-streaming call returns), you have a complete response:

```rust
struct LlmResponse {
    text: String,                    // Concatenated text from all text blocks
    tool_calls: Vec<ToolCall>,       // Parsed tool use requests
    stop_reason: String,             // "end_turn" or "tool_use"
    usage: Option<TokenUsage>,       // Token counts for budgeting
}

struct ToolCall {
    id: String,                      // Unique ID for matching results
    name: String,                    // Which tool to call
    input: serde_json::Value,        // Parameters for the tool
}
```

The `stop_reason` field is the critical signal for the agentic loop. If it is `"end_turn"`, the model is done -- transition to the Done state. If it is `"tool_use"`, the model wants to execute tools -- transition to ToolDetected. This single field controls whether the inner loop continues or stops.

The `usage` field tells you how many tokens were consumed. You track this cumulatively to enforce token budgets and to display cost information to the user. Each input token and output token has a price, and a complex agentic task can consume hundreds of thousands of tokens across many loop iterations.

## Timeouts

Network calls need timeouts. Without them, a hung connection can block your agent indefinitely. Set timeouts at two levels:

```rust
use std::time::Duration;

// Connection-level timeout (how long to wait for a connection)
let client = Client::builder()
    .connect_timeout(Duration::from_secs(10))
    .timeout(Duration::from_secs(300))  // Overall request timeout
    .build()?;
```

The overall request timeout for streaming calls should be generous -- a complex response can take minutes to generate. But it should not be infinite. Five minutes is a reasonable upper bound; if the API has not finished in five minutes, something is wrong.

## Key Takeaways

- The LLM API request combines four components: model identifier, system prompt, conversation history (including tool calls and results), and tool definitions -- all sent on every call
- Streaming is essential for interactive agents; it uses Server-Sent Events (SSE) to deliver tokens as they are generated, which your code assembles into a complete response
- The `stop_reason` field in the response is the primary control signal for the agentic loop: `"end_turn"` means stop, `"tool_use"` means continue with tool execution
- API errors must be classified by type (rate limit, auth, context overflow, server error) because each type requires a different recovery strategy
- Token usage tracking is critical for cost management and context window budgeting -- you accumulate usage across all loop iterations to enforce limits
