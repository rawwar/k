---
title: Observation Feeding
description: Feed tool execution results back into the conversation as tool_result messages so the model can react to them.
---

# Observation Feeding

> **What you'll learn:**
> - How to construct a tool_result content block with the tool_use_id, output, and optional is_error flag
> - How to append tool results to the message history so the model sees them in its next context window
> - Why the format and content of observations directly affects the model's ability to correct course and complete tasks

The agentic loop is only as good as the information that flows back to the model. After executing a tool, you need to feed the result -- the *observation* -- back into the conversation so the model can see what happened and decide its next move. This seems straightforward, but the quality and format of observations directly affects how well the model performs.

## What Is an Observation?

In agentic terminology, an "observation" is the result of an action that the agent took in the world. When the model asks to read a file, the observation is the file's contents. When it asks to run a shell command, the observation is the command's stdout and stderr. When it asks to write a file, the observation is a confirmation (or an error message).

In the Anthropic API, observations are sent as `tool_result` content blocks inside a user message:

```json
{
    "role": "user",
    "content": [
        {
            "type": "tool_result",
            "tool_use_id": "toolu_01ABC123",
            "content": "fn main() {\n    println!(\"Hello, world!\");\n}",
            "is_error": false
        }
    ]
}
```

Three fields define a tool result:

- **`tool_use_id`**: Must match the `id` from the corresponding `tool_use` block. This is non-negotiable -- the API will reject the request if IDs do not match.
- **`content`**: The output of the tool execution, as a string. This is what the model reads.
- **`is_error`**: Optional boolean. When `true`, it tells the model that the tool call failed. This is a signal that the model should try a different approach rather than proceeding with bad data.

## Building Tool Results

Let's look at how to construct tool results for different scenarios. In `src/types.rs`, you already have the `ContentBlock::ToolResult` variant. Here are helper functions for common cases:

```rust
impl ContentBlock {
    /// Create a successful tool result.
    pub fn tool_success(tool_use_id: &str, content: &str) -> Self {
        ContentBlock::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: content.to_string(),
            is_error: None,
        }
    }

    /// Create a failed tool result with an error message.
    pub fn tool_error(tool_use_id: &str, error: &str) -> Self {
        ContentBlock::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: error.to_string(),
            is_error: Some(true),
        }
    }
}
```

The distinction between `tool_success` and `tool_error` matters more than you might think. When the model sees `is_error: true`, it knows that the tool output is an error message, not a valid result. This changes its behavior -- it will typically try to fix the problem rather than proceeding as if the operation succeeded.

## Feeding Results Back: The Message Structure

After executing all tool calls from a single assistant message, you package the results into a *single* user message. This maintains the alternating message pattern the API expects:

```rust
impl Agent {
    /// Execute tool calls and feed results back into conversation state.
    async fn process_tool_calls(
        &self,
        content: &[ContentBlock],
        state: &mut ConversationState,
    ) {
        let mut results = Vec::new();

        for block in content {
            if let ContentBlock::ToolUse { id, name, input } = block {
                let result = self.execute_tool(name, input).await;
                match result {
                    Ok(output) => {
                        results.push(ContentBlock::tool_success(id, &output));
                    }
                    Err(error) => {
                        results.push(ContentBlock::tool_error(
                            id,
                            &error.to_string(),
                        ));
                    }
                }
            }
        }

        // All results go in a single user message
        if !results.is_empty() {
            state.add_tool_results(results);
        }
    }
}
```

The order of results in the user message should match the order of tool-use blocks in the assistant message. While the API uses the `tool_use_id` for matching (so order technically does not matter), maintaining order makes debugging much easier when you read the raw message history.

## Observation Quality Matters

The content of your observations directly affects how well the model performs on subsequent turns. There are some important principles.

**Be complete, not truncated.** If the model asks to read a file, return the entire file, not just the first 100 lines. The model might need a function at the bottom. If the file is too large for the context window, that is a context management problem (Chapter 9), not something to solve by silently truncating tool output.

**Include error details.** When a tool fails, include as much context as possible in the error message:

```rust
// Bad: vague error
ContentBlock::tool_error(id, "File not found")

// Good: actionable error
ContentBlock::tool_error(
    id,
    "File not found: src/mian.rs (did you mean src/main.rs?). \
     The directory src/ contains: main.rs, lib.rs, types.rs"
)
```

The model can act on detailed errors. A vague error leaves it guessing.

**Format output clearly.** When returning structured data, use consistent formatting:

```rust
/// Format a tool result for a shell command execution.
fn format_command_result(
    exit_code: i32,
    stdout: &str,
    stderr: &str,
) -> String {
    let mut output = String::new();

    if !stdout.is_empty() {
        output.push_str("stdout:\n");
        output.push_str(stdout);
    }

    if !stderr.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str("stderr:\n");
        output.push_str(stderr);
    }

    if output.is_empty() {
        output.push_str("(no output)");
    }

    output.push_str(&format!("\nexit code: {}", exit_code));
    output
}
```

::: python Coming from Python
In Python, you might construct tool results as dictionaries:
```python
tool_result = {
    "type": "tool_result",
    "tool_use_id": tool_use_id,
    "content": file_contents,
}
# Easy to forget is_error on failures:
error_result = {
    "type": "tool_result",
    "tool_use_id": tool_use_id,
    "content": str(error),
    # Oops -- forgot "is_error": True
}
```
In Rust, using the `tool_error()` constructor makes it impossible to forget the `is_error` flag. And if you construct the `ContentBlock::ToolResult` variant directly, the compiler requires you to provide a value for every field. This is one place where Rust's strictness genuinely prevents bugs.
:::

## Handling Multiple Results

When the assistant's response contains multiple tool calls, you must provide a result for *every* call. Missing results will cause the API to reject your next request. Here is a robust implementation that ensures complete coverage:

```rust
/// Process all tool calls and guarantee a result for each one.
/// Even if execution panics or times out, we return an error result.
async fn execute_all_tool_calls(
    &self,
    content: &[ContentBlock],
) -> Vec<ContentBlock> {
    let mut results = Vec::new();

    for block in content {
        if let ContentBlock::ToolUse { id, name, input } = block {
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(30),
                self.execute_tool(name, input),
            )
            .await;

            match result {
                Ok(Ok(output)) => {
                    results.push(ContentBlock::tool_success(id, &output));
                }
                Ok(Err(error)) => {
                    results.push(ContentBlock::tool_error(
                        id,
                        &format!("Tool error: {}", error),
                    ));
                }
                Err(_timeout) => {
                    results.push(ContentBlock::tool_error(
                        id,
                        &format!(
                            "Tool '{}' timed out after 30 seconds",
                            name
                        ),
                    ));
                }
            }
        }
    }

    results
}
```

The `tokio::time::timeout` wrapper ensures that a hung tool does not block the entire loop forever. The nested `Result` from `timeout` (outer `Result` for the timeout itself, inner `Result` for the tool execution) is handled with a nested `match` that covers all four cases: success, tool error, and timeout.

## The Observation Cycle

Let's trace the complete observation cycle through a concrete example. The user asks: "What's in my Cargo.toml?"

**Turn 1: Model requests the file**
```text
Assistant: [Text("Let me read that for you."), ToolUse(id:"t1", name:"read_file", input:{"path":"Cargo.toml"})]
```

**Agent executes the tool and feeds back:**
```text
User: [ToolResult(tool_use_id:"t1", content:"[package]\nname = \"my-agent\"\n...", is_error:None)]
```

**Turn 2: Model reads the observation and responds**
```text
Assistant: [Text("Your Cargo.toml defines a package called 'my-agent' with...")]
stop_reason: "end_turn"
```

The model saw the file contents in the tool result and used them to formulate its response. Without the observation, the model would have had to guess or ask the user to paste the contents -- exactly the limitation of a chatbot versus an agent.

::: wild In the Wild
Claude Code pays special attention to observation formatting. File contents are returned with the full path in the result so the model can reference files unambiguously. Shell command results include the exact command that was run, the exit code, and both stdout and stderr separated clearly. This rich formatting helps the model understand context without ambiguity. OpenCode follows a similar practice, wrapping tool outputs in structured formats that distinguish between different output streams.
:::

## Testing Observation Feeding

You can test the observation cycle without calling the real API. Create mock responses and verify the message history structure:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_tool_results_maintain_id_pairing() {
        let mut state = ConversationState::new("system prompt");
        state.add_user_message("Read two files");

        // Simulate assistant response with two tool calls
        let assistant_content = vec![
            ContentBlock::ToolUse {
                id: "t1".to_string(),
                name: "read_file".to_string(),
                input: json!({"path": "a.rs"}),
            },
            ContentBlock::ToolUse {
                id: "t2".to_string(),
                name: "read_file".to_string(),
                input: json!({"path": "b.rs"}),
            },
        ];
        state.add_assistant_message(assistant_content);

        // Feed back results
        let results = vec![
            ContentBlock::tool_success("t1", "contents of a.rs"),
            ContentBlock::tool_success("t2", "contents of b.rs"),
        ];
        state.add_tool_results(results);

        // Verify the message structure
        assert_eq!(state.messages.len(), 3);
        assert_eq!(state.messages[2].role, Role::User);

        // Verify both results are present with correct IDs
        let result_blocks = &state.messages[2].content;
        assert_eq!(result_blocks.len(), 2);
        match &result_blocks[0] {
            ContentBlock::ToolResult { tool_use_id, .. } => {
                assert_eq!(tool_use_id, "t1");
            }
            _ => panic!("Expected ToolResult"),
        }
    }
}
```

## Key Takeaways

- Observations are tool results sent as `tool_result` content blocks in a user message -- they are how the model learns what happened when its requested actions were executed
- The `tool_use_id` must match exactly between the request and the result; the `is_error` flag tells the model whether the tool succeeded or failed
- Observation quality directly affects model performance: include complete outputs, detailed error messages, and clear formatting
- All tool results from a single assistant response must be packaged into one user message to maintain the alternating role pattern the API requires
- Use timeouts on tool execution to prevent a hung tool from blocking the entire loop, and always return an error result rather than no result
