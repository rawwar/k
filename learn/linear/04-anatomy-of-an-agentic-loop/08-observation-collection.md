---
title: Observation Collection
description: Gathering tool execution results, formatting them as observations, and injecting them back into the conversation for the next LLM turn.
---

# Observation Collection

> **What you'll learn:**
> - How tool results are formatted into observation messages that the LLM can interpret
> - Strategies for truncating large tool outputs to fit within context limits without losing critical information
> - How to represent both successful results and error states in a way the model can reason about

Observation collection is the transition from ToolExecuting to ObservationReady in our state machine. Your tools have finished running. You have a vector of `ToolResult` objects -- some successes, some failures, each containing a string of output. Now you need to format these results into messages that the LLM can understand and inject them into the conversation history so the next LLM call has full context about what happened.

This phase sounds mechanical, but the decisions you make here directly affect the model's ability to reason about tool outputs. A tool result that is too long overwhelms the context window. A result that is too terse loses critical information. A result that is poorly formatted confuses the model about which result belongs to which tool call.

## The Observation Message Format

In the Anthropic API, tool results are sent as `user` role messages with a special `tool_result` content type. Each result must reference the `tool_use_id` of the corresponding tool call:

```rust
use serde_json::json;

fn format_tool_results(results: &[ToolResult]) -> serde_json::Value {
    let content: Vec<serde_json::Value> = results
        .iter()
        .map(|result| {
            let mut block = json!({
                "type": "tool_result",
                "tool_use_id": result.tool_use_id,
                "content": result.content,
            });

            if result.is_error {
                block["is_error"] = json!(true);
            }

            block
        })
        .collect();

    json!({
        "role": "user",
        "content": content
    })
}
```

Several things are important here:

**Role is "user."** Even though tool results come from your agent's code (not the human user), they are sent as `user` messages. This is a protocol convention. The API uses a strict alternation of `user` and `assistant` messages, and tool results are on the "user" side of that alternation.

**Multiple results in one message.** If the model requested three tool calls in one response, all three results go in a single `user` message as an array of `tool_result` blocks. You do not send them as three separate messages.

**The `tool_use_id` must match.** Each result references the exact `id` from the corresponding `tool_use` block. If there is a mismatch, the API will reject the request. This is why you must carefully track tool call IDs through the dispatch process.

## Adding Results to History

The observation message goes into the conversation history, right after the assistant message that requested the tools:

```rust
struct ConversationHistory {
    messages: Vec<serde_json::Value>,
}

impl ConversationHistory {
    fn add_assistant_response(&mut self, response: &LlmResponse) {
        // Build the content blocks array from the response
        let mut content = Vec::new();

        if !response.text.is_empty() {
            content.push(json!({
                "type": "text",
                "text": response.text
            }));
        }

        for call in &response.tool_calls {
            content.push(json!({
                "type": "tool_use",
                "id": call.id,
                "name": call.name,
                "input": call.input
            }));
        }

        self.messages.push(json!({
            "role": "assistant",
            "content": content
        }));
    }

    fn add_tool_results(&mut self, results: &[ToolResult]) {
        self.messages.push(format_tool_results(results));
    }
}
```

The message ordering in the history now looks like this after a tool execution cycle:

```text
[user]      "Read src/main.rs and fix any errors"
[assistant]  text: "I'll read that file."
             tool_use: read_file({path: "src/main.rs"})
[user]       tool_result: "fn main() {\n    println!(\"hello\")\n}"
[assistant]  text: "I see a missing semicolon..."
             tool_use: write_file({path: "src/main.rs", content: "..."})
[user]       tool_result: "File written successfully"
[assistant]  text: "I've fixed the missing semicolon in src/main.rs."
```

Each pair of `[assistant] tool_use` and `[user] tool_result` represents one iteration of the inner loop. The model sees this full history on every subsequent call, so it can track what it has done, what the results were, and what it should do next.

::: python Coming from Python
In the Anthropic Python SDK, the message format is identical -- you build the same JSON structure. The difference is that Python lets you construct dicts on the fly (`{"role": "user", "content": [...]}`), while Rust encourages you to build these through typed constructors. Both approaches produce the same API payload. If you have used the Python SDK's tool use feature, the Rust version will feel very familiar -- the protocol is the same; only the language is different.
:::

## Truncating Large Outputs

Tool outputs can be enormous. A `read_file` on a large source file might return 10,000 lines. A `run_command` running `find . -name "*.rs"` in a large repository might return thousands of paths. A `grep` across a codebase might match hundreds of lines.

These large outputs consume precious context window tokens. If your context limit is 200,000 tokens and a single tool result takes up 50,000 tokens, you have severely limited the conversation length and the model's ability to reason about subsequent steps.

Truncation strategies help manage this:

```rust
const MAX_TOOL_OUTPUT_CHARS: usize = 30_000; // ~7,500 tokens
const TRUNCATION_KEEP_HEAD: usize = 10_000;
const TRUNCATION_KEEP_TAIL: usize = 10_000;

fn truncate_output(content: &str) -> String {
    if content.len() <= MAX_TOOL_OUTPUT_CHARS {
        return content.to_string();
    }

    let total_lines: usize = content.lines().count();
    let head = &content[..TRUNCATION_KEEP_HEAD];
    let tail = &content[content.len() - TRUNCATION_KEEP_TAIL..];

    // Find clean line boundaries
    let head_end = head.rfind('\n').unwrap_or(head.len());
    let tail_start = tail.find('\n').unwrap_or(0);

    let head_clean = &content[..head_end];
    let tail_clean = &content[content.len() - TRUNCATION_KEEP_TAIL + tail_start..];

    let omitted_chars = content.len() - head_clean.len() - tail_clean.len();

    format!(
        "{}\n\n... [{} characters omitted, {} total lines] ...\n\n{}",
        head_clean,
        omitted_chars,
        total_lines,
        tail_clean
    )
}
```

This strategy keeps the beginning and end of the output while omitting the middle. The rationale is that:

- The **beginning** often contains headers, column names, or the first few results -- important for understanding the output format
- The **end** often contains summaries, final results, or error messages that appear after a long output
- The **middle** is usually repetitive content that the model can infer from the head and tail

The truncation message (`[X characters omitted]`) tells the model that the output was truncated, so it does not assume it has seen everything. The model can then request a more targeted tool call if it needs specific information from the omitted section.

## Type-Specific Formatting

Different tools produce different kinds of output, and the formatting strategy should adapt:

```rust
fn format_tool_output(tool_name: &str, raw_output: &str) -> String {
    match tool_name {
        "read_file" => format_file_content(raw_output),
        "run_command" => format_command_output(raw_output),
        "list_directory" => format_directory_listing(raw_output),
        _ => truncate_output(raw_output),
    }
}

fn format_file_content(content: &str) -> String {
    // Add line numbers to help the model reference specific lines
    let numbered: String = content
        .lines()
        .enumerate()
        .map(|(i, line)| format!("{:4} | {}", i + 1, line))
        .collect::<Vec<_>>()
        .join("\n");

    truncate_output(&numbered)
}

fn format_command_output(output: &str) -> String {
    // For command output, preserve as-is but truncate
    // Commands often have important exit information at the end
    truncate_output(output)
}

fn format_directory_listing(listing: &str) -> String {
    // Directory listings can be huge in large projects
    // Show count and truncate
    let entries: Vec<&str> = listing.lines().collect();
    if entries.len() > 200 {
        let shown: String = entries[..100].join("\n");
        format!(
            "{}\n\n... [{} more entries, {} total] ...",
            shown,
            entries.len() - 100,
            entries.len()
        )
    } else {
        listing.to_string()
    }
}
```

Adding line numbers to file content is a common pattern in coding agents. When the model sees numbered lines, it can reference specific line numbers in its responses ("the error is on line 42"), making its explanations and edits more precise.

::: wild In the Wild
Claude Code adds line numbers to file contents returned by its read tool and truncates outputs that exceed a configurable limit. It also formats command output to clearly separate stdout and stderr, and includes the exit code so the model knows whether the command succeeded. OpenCode applies similar formatting, wrapping tool results in structured blocks that help the model parse the output reliably.
:::

## Error Result Formatting

When a tool fails, the error message needs to be informative enough for the model to either fix the issue or explain it to the user:

```rust
fn format_error_result(tool_name: &str, error: &str) -> ToolResult {
    // Provide context that helps the model self-correct
    let formatted = match tool_name {
        "read_file" => {
            if error.contains("No such file") {
                format!(
                    "Error reading file: {}. The file does not exist. \
                     You can use the list_directory tool to see what files \
                     are available.",
                    error
                )
            } else if error.contains("Permission denied") {
                format!(
                    "Error reading file: {}. Permission denied. \
                     This file may be protected or owned by another user.",
                    error
                )
            } else {
                format!("Error reading file: {}", error)
            }
        }
        "run_command" => {
            format!(
                "Command failed: {}. You can try a different command \
                 or check that the required tools are installed.",
                error
            )
        }
        _ => format!("Tool '{}' failed: {}", tool_name, error),
    };

    ToolResult {
        tool_use_id: String::new(),
        content: formatted,
        is_error: true,
    }
}
```

Notice the pattern: each error message includes not just what went wrong, but a hint about what the model can do about it. "The file does not exist. You can use list_directory to see what files are available." This is not for the human user -- it is for the model. The model reads this error message on its next turn and uses the hint to choose its next action.

## The Complete Observation Pipeline

Putting all the pieces together, here is the complete observation collection function:

```rust
fn collect_observations(
    tool_calls: &[ToolCall],
    raw_results: &[ToolResult],
    history: &mut ConversationHistory,
) -> ObservationSummary {
    let mut formatted_results: Vec<ToolResult> = Vec::new();
    let mut total_result_tokens: usize = 0;

    for (call, result) in tool_calls.iter().zip(raw_results.iter()) {
        // Format the output based on tool type and success/error
        let formatted_content = if result.is_error {
            format_error_result(&call.name, &result.content).content
        } else {
            format_tool_output(&call.name, &result.content)
        };

        total_result_tokens += formatted_content.len() / 4; // rough estimate

        formatted_results.push(ToolResult {
            tool_use_id: call.id.clone(),
            content: formatted_content,
            is_error: result.is_error,
        });
    }

    // Add results to conversation history
    history.add_tool_results(&formatted_results);

    ObservationSummary {
        results: formatted_results,
        total_tokens_estimated: total_result_tokens,
        any_errors: raw_results.iter().any(|r| r.is_error),
    }
}

struct ObservationSummary {
    results: Vec<ToolResult>,
    total_tokens_estimated: usize,
    any_errors: bool,
}
```

After this function runs, the conversation history contains the tool results formatted as proper observation messages. The state machine transitions to ObservationReady, and the next step is to call the LLM again with the updated history. The model will see its previous tool requests, the results of those requests, and can then decide what to do next -- request more tools, or provide a final response.

## Key Takeaways

- Tool results are formatted as `user` role messages with `tool_result` content type, each referencing the `tool_use_id` of the corresponding tool call -- this pairing is critical and must be exact
- Large tool outputs must be truncated to manage context window usage; a head-and-tail strategy preserves the most useful information while keeping the truncation notification visible to the model
- Type-specific formatting improves model reasoning: line numbers on file content, separated stdout/stderr for commands, and entry counts for directory listings
- Error results should include actionable hints ("file not found -- try list_directory to see available files") that help the model self-correct on its next turn
- The observation collection phase is the bridge between tool execution and the next LLM call -- the quality of formatting directly affects the model's ability to reason about tool outputs
