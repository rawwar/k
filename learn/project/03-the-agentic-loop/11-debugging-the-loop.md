---
title: Debugging the Loop
description: Add structured logging and diagnostic tools to trace message flow and identify problems in the agentic loop.
---

# Debugging the Loop

> **What you'll learn:**
> - How to use the tracing crate to log every LLM request, response, and tool execution with structured fields
> - How to dump the full message history at each iteration for offline debugging of stuck loops
> - Common failure modes like infinite tool-call cycles, missing tool_result messages, and context window overflow

An agentic loop is harder to debug than a single API call. When something goes wrong, the issue might be in the first turn or the tenth. The model might be making subtly wrong tool calls, or the observations might be formatted in a way the model cannot interpret. Without good logging, you are left guessing. Let's add structured logging and diagnostic tools that make the loop's behavior transparent.

## Adding the tracing Crate

The `tracing` crate is the standard Rust library for structured, leveled logging. Unlike `println!` debugging, `tracing` lets you attach structured fields to log events, filter by severity, and control output format. Add it to your `Cargo.toml`:

```toml
[dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

Initialize it at the start of your program:

```rust
use tracing_subscriber::EnvFilter;

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("agent=debug".parse().unwrap()),
        )
        .with_target(false)
        .with_thread_ids(false)
        .init();
}
```

This sets up a subscriber that reads the `RUST_LOG` environment variable for filtering. The `agent=debug` directive means all events from your agent module will be logged at debug level and above. You can run your agent with `RUST_LOG=agent=trace` for even more detail, or `RUST_LOG=agent=info` for just the highlights.

## Instrumenting the Loop

Add trace events at each decision point in the loop. Here is the instrumented version of the core loop:

```rust
use tracing::{debug, info, warn, error, instrument};

impl Agent {
    #[instrument(skip(self, state), fields(user_msg = %user_message))]
    pub async fn run(
        &self,
        state: &mut ConversationState,
        user_message: &str,
    ) -> Result<LoopResult, AgentError> {
        state.add_user_message(user_message);
        let mut inner_turns: usize = 0;

        info!(
            message_count = state.messages.len(),
            "Starting agentic loop"
        );

        loop {
            if self.config.turn_config.max_inner_turns > 0
                && inner_turns >= self.config.turn_config.max_inner_turns
            {
                warn!(
                    turns = inner_turns,
                    limit = self.config.turn_config.max_inner_turns,
                    "Turn limit reached"
                );
                let partial = self.last_assistant_text(state);
                return Ok(LoopResult::TurnLimitReached(partial));
            }

            let estimated_tokens = state.estimate_token_count();
            debug!(
                turn = inner_turns + 1,
                messages = state.messages.len(),
                estimated_tokens,
                "Calling LLM"
            );

            let response = self.call_api(state).await?;
            inner_turns += 1;
            state.record_usage(&response.usage);
            state.add_assistant_message(response.content.clone());

            info!(
                turn = inner_turns,
                stop_reason = ?response.stop_reason,
                content_blocks = response.content.len(),
                input_tokens = response.usage.input_tokens,
                output_tokens = response.usage.output_tokens,
                "Received LLM response"
            );

            match self.decide_action(
                response.stop_reason.as_deref(),
                &response.content,
            ) {
                LoopAction::ReturnToUser => {
                    let text = Self::extract_text(&response.content);
                    info!(
                        total_turns = inner_turns,
                        total_input_tokens = state.total_input_tokens,
                        total_output_tokens = state.total_output_tokens,
                        "Loop complete"
                    );
                    return Ok(LoopResult::Complete(text));
                }

                LoopAction::ExecuteTools => {
                    let tool_calls = extract_tool_calls(&response.content);
                    for call in &tool_calls {
                        debug!(
                            tool = %call.name,
                            id = %call.id,
                            input = %call.input,
                            "Executing tool"
                        );
                    }
                    let results = self.handle_tool_calls(&response.content).await;
                    for result in &results {
                        if let ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } = result
                        {
                            debug!(
                                id = %tool_use_id,
                                is_error = ?is_error,
                                output_len = content.len(),
                                "Tool result"
                            );
                        }
                    }
                    state.add_tool_results(results);
                }

                LoopAction::MaxTokensReached => {
                    warn!(
                        turn = inner_turns,
                        "Model hit max_tokens limit"
                    );
                    let text = Self::extract_text(&response.content);
                    return Ok(LoopResult::MaxTokens(text));
                }

                LoopAction::UnexpectedReason(reason) => {
                    error!(
                        stop_reason = %reason,
                        "Unexpected stop reason from API"
                    );
                    return Err(AgentError::UnexpectedStopReason(reason));
                }
            }
        }
    }
}
```

The `#[instrument]` attribute automatically creates a span for the function call, and the `skip(self, state)` parameter prevents large structs from being logged. Each `debug!` and `info!` call uses structured fields (the `key = value` syntax) which can be filtered, searched, and analyzed programmatically.

::: python Coming from Python
In Python, you might use the `logging` module with f-strings:
```python
import logging
logger = logging.getLogger("agent")

logger.info(f"Turn {turn}: calling LLM with {len(messages)} messages")
logger.debug(f"Tool call: {tool_name}({tool_input})")
```
Rust's `tracing` crate is more powerful because structured fields are machine-parseable, not just string formatting. You can configure a JSON subscriber and pipe your logs into analysis tools. The `#[instrument]` attribute also creates hierarchical spans, so you can see which tool call was part of which loop iteration without manual bookkeeping.
:::

## Dumping Message History

When the loop behaves unexpectedly, you need to see the complete message history. Add a function that dumps the state in a readable format:

```rust
impl ConversationState {
    /// Dump the complete message history for debugging.
    /// Returns a human-readable representation of all messages.
    pub fn dump_history(&self) -> String {
        let mut output = String::new();
        output.push_str(&format!(
            "=== Conversation History ({} messages) ===\n",
            self.messages.len()
        ));
        output.push_str(&format!(
            "System: {}...\n\n",
            &self.system_prompt[..self.system_prompt.len().min(100)]
        ));

        for (i, msg) in self.messages.iter().enumerate() {
            let role = match msg.role {
                Role::User => "USER",
                Role::Assistant => "ASST",
            };
            output.push_str(&format!("--- Message {} [{}] ---\n", i, role));

            for block in &msg.content {
                match block {
                    ContentBlock::Text { text } => {
                        // Truncate long text for readability
                        let display = if text.len() > 200 {
                            format!("{}... ({} chars)", &text[..200], text.len())
                        } else {
                            text.clone()
                        };
                        output.push_str(&format!("  [text] {}\n", display));
                    }
                    ContentBlock::ToolUse { id, name, input } => {
                        output.push_str(&format!(
                            "  [tool_use] {} (id: {}) input: {}\n",
                            name, id, input
                        ));
                    }
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        let err_flag = if *is_error == Some(true) {
                            " ERROR"
                        } else {
                            ""
                        };
                        let display = if content.len() > 200 {
                            format!(
                                "{}... ({} chars)",
                                &content[..200],
                                content.len()
                            )
                        } else {
                            content.clone()
                        };
                        output.push_str(&format!(
                            "  [tool_result{}] id:{} {}\n",
                            err_flag, tool_use_id, display
                        ));
                    }
                }
            }
            output.push('\n');
        }

        output.push_str(&format!(
            "Total tokens: {} input + {} output\n",
            self.total_input_tokens, self.total_output_tokens
        ));

        output
    }
}
```

You can save this dump to a file when debugging:

```rust
use std::fs;

/// Save the conversation history to a debug file.
fn save_debug_dump(state: &ConversationState, filename: &str) {
    let dump = state.dump_history();
    if let Err(e) = fs::write(filename, &dump) {
        warn!(error = %e, "Failed to save debug dump");
    } else {
        debug!(file = filename, "Saved conversation dump");
    }
}
```

## Common Failure Modes

Here are the bugs you will encounter most often when building and running the agentic loop.

### 1. Infinite Tool-Call Cycles

**Symptom**: The loop keeps running, the model keeps calling the same tool, and nothing converges.

**Cause**: Usually the tool returns an error, and the model retries with the same arguments. Or the tool succeeds but the model does not recognize the result as progress.

**Detection**: Track consecutive identical tool calls:

```rust
fn detect_repetition(
    history: &[Message],
    window_size: usize,
) -> Option<String> {
    let recent_tool_calls: Vec<_> = history
        .iter()
        .rev()
        .take(window_size * 2) // each tool call is 2 messages
        .filter_map(|m| {
            m.content.iter().find_map(|b| match b {
                ContentBlock::ToolUse { name, input, .. } => {
                    Some(format!("{}:{}", name, input))
                }
                _ => None,
            })
        })
        .collect();

    // Check if the same call appears 3+ times
    for call in &recent_tool_calls {
        let count = recent_tool_calls.iter().filter(|c| *c == call).count();
        if count >= 3 {
            return Some(call.clone());
        }
    }
    None
}
```

**Fix**: When repetition is detected, inject a system hint telling the model to try a different approach, or break the loop with a partial result.

### 2. Missing Tool Result Messages

**Symptom**: The API returns an error like "messages: tool_use block without matching tool_result."

**Cause**: The assistant's response contained a `tool_use` block, but the next message in the history does not contain a `tool_result` block with the matching ID.

**Detection**: Validate message pairs before each API call:

```rust
fn validate_tool_pairs(messages: &[Message]) -> Result<(), String> {
    for (i, msg) in messages.iter().enumerate() {
        if msg.role != Role::Assistant {
            continue;
        }
        let tool_use_ids: Vec<_> = msg.content.iter().filter_map(|b| {
            match b {
                ContentBlock::ToolUse { id, .. } => Some(id.as_str()),
                _ => None,
            }
        }).collect();

        if tool_use_ids.is_empty() {
            continue;
        }

        // The next message must be a user message with matching tool results
        let next = messages.get(i + 1).ok_or_else(|| {
            format!("Message {} has tool_use but no following message", i)
        })?;

        for expected_id in &tool_use_ids {
            let found = next.content.iter().any(|b| {
                matches!(b, ContentBlock::ToolResult { tool_use_id, .. }
                    if tool_use_id == expected_id)
            });
            if !found {
                return Err(format!(
                    "Missing tool_result for tool_use_id '{}'",
                    expected_id
                ));
            }
        }
    }
    Ok(())
}
```

### 3. Context Window Overflow

**Symptom**: API returns a 400 error about the request being too large, or the model's responses become incoherent as the context fills up.

**Detection**: Log the estimated token count at each iteration (already done in our instrumented loop) and alert when it crosses a threshold:

```rust
if estimated_tokens > 150_000 {
    warn!(
        tokens = estimated_tokens,
        "Context window is getting large -- consider truncation"
    );
}
```

### 4. Malformed Tool Input from the Model

**Symptom**: The tool crashes because the model sent unexpected JSON as input.

**Detection**: Always validate tool input before execution. Log the raw input when validation fails:

```rust
if let Some(err) = validate_tool_input(&call.name, &call.input) {
    warn!(
        tool = %call.name,
        input = %call.input,
        error = %err,
        "Tool input validation failed"
    );
}
```

::: wild In the Wild
Claude Code has extensive debug logging behind a `--verbose` flag. When enabled, it shows every API request, every tool call with its arguments, and every tool result. For development, it also supports dumping the full message history to a file. This is invaluable when investigating why the agent made a particular decision. Most production agents include similar debug facilities because agentic loops are inherently harder to debug than linear code -- the control flow depends on the model's decisions, which are not deterministic.
:::

## A Debug REPL Command

Add a `/debug` command to your REPL that dumps the current conversation state:

```rust
fn handle_repl_command(
    input: &str,
    state: &ConversationState,
) -> Option<String> {
    match input.trim() {
        "/debug" => {
            Some(state.dump_history())
        }
        "/tokens" => {
            Some(format!(
                "Estimated context: {} tokens\n\
                 Total input tokens used: {}\n\
                 Total output tokens used: {}",
                state.estimate_token_count(),
                state.total_input_tokens,
                state.total_output_tokens,
            ))
        }
        "/messages" => {
            Some(format!("{} messages in history", state.messages.len()))
        }
        _ => None,
    }
}
```

This gives you real-time visibility into the conversation state without stopping the agent or reading log files.

## Key Takeaways

- Use the `tracing` crate with structured fields for logging -- it provides filterable, machine-readable logs that are far more useful than `println!` for debugging asynchronous loops
- Log at every decision point: before API calls (turn count, token estimate), after responses (stop reason, content block count), and during tool execution (tool name, input, result)
- Implement a conversation history dump that shows every message with truncated content -- this is the single most useful debugging tool when the loop behaves unexpectedly
- The four most common failures are: infinite tool-call cycles, missing tool result messages, context window overflow, and malformed tool input -- validate and detect all four proactively
- Add debug commands to your REPL (`/debug`, `/tokens`, `/messages`) for real-time visibility into the loop's internal state during development
