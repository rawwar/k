---
title: Error States
description: Handling failures at every stage of the agentic loop — API errors, tool crashes, invalid model output, and context overflow.
---

# Error States

> **What you'll learn:**
> - The taxonomy of errors in an agentic loop: API failures, tool execution errors, parsing failures, and state corruption
> - How to feed error information back to the model so it can self-correct and retry
> - Recovery strategies including retry with backoff, fallback tools, and graceful degradation to user input

Errors in an agentic loop are not exceptional -- they are expected. APIs go down. Tools crash. The model generates invalid JSON. Files get deleted between reads. Unlike a simple script where an error means "stop and report," an agentic loop must handle errors as part of its normal operation. Many errors are recoverable: the model can try a different approach, use a different tool, or ask the user for help. The art is distinguishing recoverable errors from fatal ones and routing each to the right handler.

This subchapter catalogs every category of error that can occur in the agentic loop and describes the recovery strategy for each.

## The Error Taxonomy

Errors in an agentic loop fall into five categories, each requiring a different response:

```rust
enum AgentError {
    // Category 1: LLM API errors
    Api(ApiError),

    // Category 2: Tool execution errors
    Tool(ToolError),

    // Category 3: Model output parsing errors
    Parse(ParseError),

    // Category 4: Context/state errors
    Context(ContextError),

    // Category 5: System-level errors
    System(SystemError),
}

enum ApiError {
    RateLimit { retry_after_secs: u64 },
    Overloaded,
    ServerError(String),
    AuthenticationFailed(String),
    InvalidRequest(String),
    NetworkError(String),
    Timeout,
}

enum ToolError {
    ExecutionFailed { tool: String, message: String },
    Timeout { tool: String, timeout_secs: u64 },
    PermissionDenied { tool: String },
    InvalidOutput { tool: String, message: String },
}

enum ParseError {
    InvalidToolCallJson { tool: String, raw: String },
    UnknownTool(String),
    MissingToolId,
    MalformedResponse(String),
}

enum ContextError {
    WindowOverflow { current_tokens: usize, max_tokens: usize },
    HistoryCorrupted(String),
    MissingToolResult { tool_use_id: String },
}

enum SystemError {
    IoError(String),
    ConfigError(String),
    OutOfMemory,
}
```

Let's examine each category and its recovery strategies.

## Category 1: API Errors

API errors occur during the LLM invocation phase. They are the most common error type and the easiest to handle because they are well-documented and follow predictable patterns.

### Rate Limits (HTTP 429)

The API is telling you to slow down. The response includes a `retry-after` header or body field:

```rust
async fn handle_rate_limit(
    retry_after_secs: u64,
    attempt: usize,
    max_retries: usize,
) -> Result<RetryAction, AgentError> {
    if attempt >= max_retries {
        return Err(AgentError::Api(ApiError::RateLimit {
            retry_after_secs,
        }));
    }

    let wait = std::time::Duration::from_secs(retry_after_secs);
    println!("Rate limited. Waiting {} seconds before retry...", retry_after_secs);
    tokio::time::sleep(wait).await;
    Ok(RetryAction::Retry)
}

enum RetryAction {
    Retry,
    Abort,
}
```

### Server Errors (HTTP 500, 502, 503, 529)

The API server is having problems. Use exponential backoff:

```rust
async fn handle_server_error(
    attempt: usize,
    max_retries: usize,
) -> Result<RetryAction, AgentError> {
    if attempt >= max_retries {
        return Err(AgentError::Api(ApiError::ServerError(
            "Max retries exceeded".to_string(),
        )));
    }

    // Exponential backoff: 1s, 2s, 4s, 8s, 16s
    let base_delay = 1u64;
    let delay = base_delay * 2u64.pow(attempt as u32);
    let delay = delay.min(30); // Cap at 30 seconds

    println!("Server error. Retrying in {} seconds (attempt {}/{})...",
        delay, attempt + 1, max_retries);
    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
    Ok(RetryAction::Retry)
}
```

### Authentication Errors (HTTP 401)

The API key is invalid or expired. This is not retryable -- it is a configuration error:

```rust
fn handle_auth_error(message: &str) -> AgentError {
    eprintln!(
        "Authentication failed: {}\n\
         Please check your API key configuration.",
        message
    );
    AgentError::Api(ApiError::AuthenticationFailed(message.to_string()))
}
```

### The Retry Wrapper

All retryable API errors go through a common retry wrapper:

```rust
async fn call_llm_with_retry(
    request: &ApiRequest,
    client: &reqwest::Client,
    api_key: &str,
    max_retries: usize,
) -> Result<LlmResponse, AgentError> {
    let mut attempt = 0;

    loop {
        match call_llm_once(request, client, api_key).await {
            Ok(response) => return Ok(response),
            Err(AgentError::Api(ApiError::RateLimit { retry_after_secs })) => {
                handle_rate_limit(retry_after_secs, attempt, max_retries).await?;
            }
            Err(AgentError::Api(ApiError::Overloaded)) |
            Err(AgentError::Api(ApiError::ServerError(_))) => {
                handle_server_error(attempt, max_retries).await?;
            }
            Err(AgentError::Api(ApiError::Timeout)) => {
                if attempt >= max_retries {
                    return Err(AgentError::Api(ApiError::Timeout));
                }
                println!("Request timed out. Retrying...");
            }
            Err(e) => return Err(e), // Non-retryable errors propagate immediately
        }
        attempt += 1;
    }
}
```

::: python Coming from Python
Python's `tenacity` library provides retry decorators that handle backoff automatically. In Rust, you build retry logic explicitly or use a crate like `backon`. The explicit approach is more verbose but makes the retry behavior crystal clear in the code. You can see exactly what errors are retried, what the backoff schedule is, and when to give up.
:::

## Category 2: Tool Execution Errors

Tool errors happen during the ToolExecuting phase. The critical insight is that most tool errors are **recoverable by the model**. If a file read fails because the file does not exist, the model can try a different path. If a command fails, the model can try a different command.

The recovery strategy is: **feed the error back to the model as a tool result**:

```rust
fn handle_tool_error(call: &ToolCall, error: &ToolError) -> ToolResult {
    let message = match error {
        ToolError::ExecutionFailed { message, .. } => {
            format!("Tool execution failed: {}", message)
        }
        ToolError::Timeout { tool, timeout_secs } => {
            format!(
                "Tool '{}' timed out after {} seconds. \
                 Consider breaking the operation into smaller steps.",
                tool, timeout_secs
            )
        }
        ToolError::PermissionDenied { tool } => {
            format!(
                "Permission denied for tool '{}'. \
                 The user has not approved this operation.",
                tool
            )
        }
        ToolError::InvalidOutput { message, .. } => {
            format!("Tool produced invalid output: {}", message)
        }
    };

    ToolResult {
        tool_use_id: call.id.clone(),
        content: message,
        is_error: true,
    }
}
```

This result goes into the conversation history just like a successful result. The model sees the error on its next turn and can react. This is one of the most powerful features of the agentic loop: **errors become information that the model reasons about**.

Consider this sequence:

```text
Model: tool_use(read_file, {path: "src/mian.rs"})     // Typo in filename
Agent: tool_result(error: "File not found: src/mian.rs")
Model: tool_use(list_directory, {path: "src/"})         // Model self-corrects
Agent: tool_result(["main.rs", "lib.rs", "config.rs"])
Model: tool_use(read_file, {path: "src/main.rs"})      // Correct path this time
Agent: tool_result("fn main() { ... }")
```

The model made a mistake (typo in the filename), received an error, used another tool to discover the correct filename, and fixed its approach. No human intervention was needed. This self-correction loop is fundamental to how agentic systems work.

## Category 3: Parse Errors

Parse errors occur when the model's output cannot be properly interpreted. The most common case is invalid JSON in tool call parameters:

```rust
fn handle_parse_error(
    raw_response: &str,
    error: &ParseError,
    history: &mut ConversationHistory,
) -> AgentState {
    match error {
        ParseError::InvalidToolCallJson { tool, raw } => {
            // Feed the parse error back as a tool result
            // The model will see this and generate valid JSON next time
            let error_msg = format!(
                "Your tool call to '{}' contained invalid JSON: {}. \
                 The raw input was: {}. Please try again with valid JSON.",
                tool, error, raw
            );

            // We need to add the assistant's response to history first
            // (even though it was malformed) and then the error
            history.add_raw_assistant_message(raw_response);
            history.add_system_error(&error_msg);

            // Return to Processing -- the model will try again
            AgentState::Processing
        }
        ParseError::UnknownTool(name) => {
            // Model hallucinated a tool -- feed back the available tools
            let error_msg = format!(
                "Unknown tool '{}'. Available tools are: read_file, write_file, \
                 run_command. Please use one of these tools.",
                name
            );
            history.add_system_error(&error_msg);
            AgentState::Processing
        }
        ParseError::MissingToolId => {
            // This should not happen with well-formed API responses
            // but if it does, we cannot match results to calls
            AgentState::Error {
                error: AgentError::Parse(error.clone()),
            }
        }
        ParseError::MalformedResponse(msg) => {
            // The entire response was unparseable
            AgentState::Error {
                error: AgentError::Parse(ParseError::MalformedResponse(msg.clone())),
            }
        }
    }
}
```

Notice the pattern: for recoverable parse errors (invalid JSON, unknown tools), we feed the error back to the model and let it try again. For unrecoverable parse errors (completely malformed response, missing IDs), we transition to the Error state.

::: tip In the Wild
Claude Code handles invalid tool call JSON by sending an error result back to the model with the original malformed JSON included. This gives the model the context to fix its output. In practice, modern Claude models very rarely produce invalid JSON for tool calls, but the handling is still important for robustness. OpenCode includes similar error recovery in its tool dispatch layer, wrapping all JSON parsing in error handlers that generate informative error messages for the model.
:::

## Category 4: Context Errors

Context errors relate to the conversation state itself:

**Context window overflow** -- The accumulated history plus the new response exceeds the model's context limit. The API will reject the request with a 400 error:

```rust
fn handle_context_overflow(
    history: &mut ConversationHistory,
    system_prompt: &str,
    tools: &[ToolDefinition],
) -> Result<AgentState, AgentError> {
    // Try to compact the history
    let compacted = compact_history(history, system_prompt, tools)?;

    if compacted {
        // History was compacted successfully -- retry the LLM call
        Ok(AgentState::Processing)
    } else {
        // Cannot compact further -- the current message alone exceeds limits
        Err(AgentError::Context(ContextError::WindowOverflow {
            current_tokens: estimate_tokens(history),
            max_tokens: get_max_context_tokens(),
        }))
    }
}
```

**Missing tool results** -- The API requires that every `tool_use` block in an assistant message has a corresponding `tool_result` in the next user message. If one is missing (perhaps due to a bug), the API will reject the request:

```rust
fn validate_history(history: &ConversationHistory) -> Result<(), ContextError> {
    // Check that every tool_use has a matching tool_result
    let mut pending_tool_ids: Vec<String> = Vec::new();

    for message in history.all_messages() {
        if message.role == "assistant" {
            for block in &message.content_blocks {
                if let ContentBlock::ToolUse { id, .. } = block {
                    pending_tool_ids.push(id.clone());
                }
            }
        } else if message.role == "user" {
            for block in &message.content_blocks {
                if let ContentBlock::ToolResult { tool_use_id, .. } = block {
                    pending_tool_ids.retain(|id| id != tool_use_id);
                }
            }
        }
    }

    if !pending_tool_ids.is_empty() {
        return Err(ContextError::MissingToolResult {
            tool_use_id: pending_tool_ids[0].clone(),
        });
    }

    Ok(())
}
```

## Category 5: System Errors

System-level errors are generally not recoverable within the agent loop:

```rust
fn handle_system_error(error: &SystemError) {
    match error {
        SystemError::IoError(msg) => {
            eprintln!("I/O error: {}. Please check file permissions and disk space.", msg);
        }
        SystemError::ConfigError(msg) => {
            eprintln!("Configuration error: {}. Please check your agent configuration.", msg);
        }
        SystemError::OutOfMemory => {
            eprintln!("Out of memory. The conversation history may be too large. \
                       Try using /compact to reduce memory usage.");
        }
    }
}
```

## The Error Handling Strategy Matrix

Here is a summary of how each error type is handled:

| Error Type | Recoverable? | Strategy | State Transition |
|------------|-------------|----------|-----------------|
| Rate limit (429) | Yes | Wait and retry | Stay in Processing |
| Server error (5xx) | Yes | Exponential backoff | Stay in Processing |
| Auth error (401) | No | Report and exit | Error -> Idle |
| Tool execution failure | Yes | Feed error to model | ToolExecuting -> ObservationReady |
| Tool timeout | Partial | Feed timeout notice to model | ToolExecuting -> ObservationReady |
| Permission denied | Yes | Feed denial to model | ToolExecuting -> ObservationReady |
| Invalid tool JSON | Yes | Feed parse error to model | Processing (retry) |
| Unknown tool name | Yes | Feed available tools to model | Processing (retry) |
| Context overflow | Partial | Compact history, retry | Processing or Error |
| Missing tool result | No | Bug in agent code | Error |
| I/O error | No | Report and stop | Error -> Idle |

The general principle is: **if the model can fix it, feed the error to the model. If your code can fix it, retry. If nothing can fix it, report and stop.**

## Error Budgets

Just as you limit iterations and tokens, you should limit errors. A model stuck in an error loop (tool fails, model retries, fails again, retries again) can consume iterations and tokens rapidly:

```rust
struct ErrorTracker {
    consecutive_errors: usize,
    max_consecutive_errors: usize,
    total_errors: usize,
    max_total_errors: usize,
}

impl ErrorTracker {
    fn record_error(&mut self) -> Result<(), StopCondition> {
        self.consecutive_errors += 1;
        self.total_errors += 1;

        if self.consecutive_errors >= self.max_consecutive_errors {
            return Err(StopCondition::TooManyConsecutiveErrors {
                count: self.consecutive_errors,
            });
        }
        if self.total_errors >= self.max_total_errors {
            return Err(StopCondition::TooManyTotalErrors {
                count: self.total_errors,
            });
        }
        Ok(())
    }

    fn record_success(&mut self) {
        self.consecutive_errors = 0;
        // total_errors does not reset
    }
}
```

A limit of 3-5 consecutive errors is sensible. If the model fails three times in a row at the same thing, it is unlikely to succeed on the fourth try. Better to stop and let the user redirect.

## Key Takeaways

- Errors in an agentic loop fall into five categories: API errors, tool execution errors, parse errors, context errors, and system errors -- each requires a different recovery strategy
- The most powerful recovery mechanism is feeding errors back to the model as tool results, enabling self-correction: the model sees the error, reasons about what went wrong, and tries a different approach
- API errors (rate limits, server errors) are handled with retry and exponential backoff; authentication errors and malformed requests are not retryable
- Context overflow is handled by compacting conversation history; missing tool results indicate a bug in the agent code and are not recoverable at runtime
- Error budgets (limits on consecutive and total errors) prevent the model from getting stuck in error loops that waste iterations and tokens
