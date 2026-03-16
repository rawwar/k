---
title: Tool Results as Observations
description: Format tool execution results into tool_result messages that feed back into the agentic loop's conversation state.
---

# Tool Results as Observations

> **What you'll learn:**
> - How to construct a tool_result content block that matches the tool_use_id from the original request
> - How to truncate large tool outputs to avoid exceeding the model's context window
> - How observation quality affects the model's reasoning and why concise, structured results outperform raw dumps

The dispatch function produces a `ToolResult`. The agentic loop needs to convert that into a message that the model can read on its next turn. This is the final link in the tool call chain: the model asked, the tool answered, and now you feed the answer back as an *observation*. The format and quality of this observation directly affects how well the model reasons about the result.

## The tool_result Message Format

The Anthropic API requires tool results in a specific format. After receiving an assistant message with `tool_use` blocks, you must send a user message containing `tool_result` content blocks -- one for each `tool_use`:

```json
{
  "role": "user",
  "content": [
    {
      "type": "tool_result",
      "tool_use_id": "toolu_01XFDUDYJgAACzvnptvVer8z",
      "content": "fn main() {\n    println!(\"Hello, world!\");\n}",
      "is_error": false
    }
  ]
}
```

Three fields matter:

- **`tool_use_id`** -- Must match the `id` from the `tool_use` block. This links the result to the request. If these do not match, the API returns an error.
- **`content`** -- The tool's output as a string. This is what the model reads on its next turn.
- **`is_error`** -- When `true`, tells the model the tool call failed. The model adjusts its reasoning accordingly.

## Building the Observation Message

Here is how to convert your `ToolResult` structs into the API message format:

```rust
use serde_json::{json, Value};

pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
}

/// Convert a single ToolResult into a tool_result content block.
fn tool_result_to_content_block(result: &ToolResult) -> Value {
    let mut block = json!({
        "type": "tool_result",
        "tool_use_id": result.tool_use_id,
        "content": result.content,
    });

    // Only include is_error when true (the API treats missing as false)
    if result.is_error {
        block["is_error"] = json!(true);
    }

    block
}

/// Convert multiple ToolResults into a complete user message with
/// tool_result content blocks.
fn build_tool_results_message(results: &[ToolResult]) -> Value {
    let content: Vec<Value> = results
        .iter()
        .map(tool_result_to_content_block)
        .collect();

    json!({
        "role": "user",
        "content": content
    })
}

fn main() {
    let results = vec![
        ToolResult {
            tool_use_id: "toolu_01ABC".to_string(),
            content: "fn main() {\n    println!(\"Hello!\");\n}".to_string(),
            is_error: false,
        },
        ToolResult {
            tool_use_id: "toolu_02DEF".to_string(),
            content: "File 'missing.rs' not found.".to_string(),
            is_error: true,
        },
    ];

    let message = build_tool_results_message(&results);
    println!("{}", serde_json::to_string_pretty(&message).unwrap());
}
```

This produces:

```json
{
  "role": "user",
  "content": [
    {
      "type": "tool_result",
      "tool_use_id": "toolu_01ABC",
      "content": "fn main() {\n    println!(\"Hello!\");\n}"
    },
    {
      "type": "tool_result",
      "tool_use_id": "toolu_02DEF",
      "content": "File 'missing.rs' not found.",
      "is_error": true
    }
  ]
}
```

Notice that the first result (success) does not include `is_error` at all -- the API treats the absence of this field as `false`. The second result (failure) includes `is_error: true`. This message gets appended to the conversation history and sent in the next API call.

## Observations in the Agentic Loop

Here is how tool results feed into the loop from Chapter 3. The flow is:

1. The model produces a response with `tool_use` blocks.
2. You append the assistant message to the conversation.
3. You dispatch each tool call and collect results.
4. You build the tool results message and append it to the conversation.
5. You send the updated conversation back to the API.

```rust
use serde_json::{json, Value};

fn agentic_loop_step(
    messages: &mut Vec<Value>,
    assistant_response: &Value,
    tool_results: &[ToolResult],
) {
    // Append the assistant's message (including tool_use blocks)
    messages.push(assistant_response.clone());

    // Append the tool results as a user message
    let results_message = build_tool_results_message(tool_results);
    messages.push(results_message);

    // The conversation now looks like:
    // [user] "Read main.rs and fix the bug"
    // [assistant] tool_use: read_file(path: "src/main.rs")
    // [user] tool_result: "fn main() { ... }"
    // [assistant] tool_use: edit_file(...)    <-- next turn
    // [user] tool_result: "File updated"
    // ...
}
```

The model sees the tool results as if a user sent them. From the model's perspective, each `tool_result` is an observation about the world -- the file has these contents, the command produced this output, the search found these results. The model uses these observations to plan its next action.

::: python Coming from Python
The message assembly looks similar in Python:

```python
messages.append({"role": "assistant", "content": assistant_content})
messages.append({
    "role": "user",
    "content": [
        {"type": "tool_result", "tool_use_id": r.id, "content": r.content}
        for r in tool_results
    ]
})
```

The concept is identical. The Rust version is more verbose because you construct `serde_json::Value` objects explicitly rather than using dictionary literals.
:::

## Structuring Tool Output for Better Reasoning

The *content* of a tool result is not just data -- it is input to the model's reasoning process. Well-structured observations help the model extract information faster and reason more accurately.

### Add Line Numbers to File Contents

When returning file contents, include line numbers. The model uses these to reference specific locations when making edits:

```rust
fn format_file_contents(path: &str, contents: &str) -> String {
    let numbered_lines: Vec<String> = contents
        .lines()
        .enumerate()
        .map(|(i, line)| format!("{:>4} | {}", i + 1, line))
        .collect();

    format!(
        "Contents of {}:\n{}",
        path,
        numbered_lines.join("\n")
    )
}

fn main() {
    let contents = "fn main() {\n    println!(\"Hello\");\n}\n";
    println!("{}", format_file_contents("src/main.rs", contents));
}
```

Output:

```
Contents of src/main.rs:
   1 | fn main() {
   2 |     println!("Hello");
   3 | }
```

### Include Metadata in Shell Output

When returning shell command results, include the exit code and separate stdout from stderr:

```rust
fn format_shell_result(command: &str, exit_code: i32, stdout: &str, stderr: &str) -> String {
    let mut output = format!("$ {}\n", command);

    if !stdout.is_empty() {
        output.push_str(stdout);
        if !stdout.ends_with('\n') {
            output.push('\n');
        }
    }

    if !stderr.is_empty() {
        output.push_str(&format!("[stderr]\n{}", stderr));
        if !stderr.ends_with('\n') {
            output.push('\n');
        }
    }

    output.push_str(&format!("[exit code: {}]", exit_code));
    output
}

fn main() {
    // Successful command
    println!("{}", format_shell_result(
        "cargo check",
        0,
        "    Finished dev profile [unoptimized + debuginfo]\n",
        "",
    ));

    println!();

    // Failed command
    println!("{}", format_shell_result(
        "cargo build",
        1,
        "",
        "error[E0308]: mismatched types\n  --> src/main.rs:5:12\n",
    ));
}
```

The structured format helps the model parse the output. It can see at a glance whether the command succeeded (exit code 0) and whether there are errors to address.

## Smart Truncation

In subchapter 8 you saw basic character truncation. For observations, you can be smarter about *what* to keep. Different tools benefit from different truncation strategies:

```rust
/// Truncate file contents, keeping the first and last portions.
fn truncate_file_output(contents: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = contents.lines().collect();

    if lines.len() <= max_lines {
        return contents.to_string();
    }

    let keep_start = max_lines / 2;
    let keep_end = max_lines - keep_start;
    let omitted = lines.len() - max_lines;

    let mut result = String::new();
    for line in &lines[..keep_start] {
        result.push_str(line);
        result.push('\n');
    }
    result.push_str(&format!(
        "\n... [{} lines omitted] ...\n\n",
        omitted
    ));
    for line in &lines[lines.len() - keep_end..] {
        result.push_str(line);
        result.push('\n');
    }

    result
}

fn main() {
    // Simulate a long file
    let lines: Vec<String> = (1..=100)
        .map(|i| format!("Line {}: some content here", i))
        .collect();
    let content = lines.join("\n");

    let truncated = truncate_file_output(&content, 10);
    println!("{}", truncated);
}
```

This shows the first 5 and last 5 lines of a 100-line file, with a note about the 90 omitted lines. The model gets context from both the beginning and end of the file, which is often more useful than just the beginning.

::: wild In the Wild
Claude Code uses different truncation strategies for different tools. File reads show the first and last portions. Shell output keeps the last portion (since error messages and results typically appear at the end). Search results are capped by the number of matches. The common principle is: show the model the parts that are most likely to be useful for its current task.
:::

## Observation Quality and Agent Performance

The observations you feed back are the model's *only* window into the real world. Every design decision you make here affects the model's ability to reason and act:

- **More context is not always better.** A 10,000-line file dump wastes tokens and makes it harder for the model to find the relevant section. A targeted read of lines 45-55 is more useful.
- **Structure helps parsing.** Line numbers, headers, and labeled sections (like `[stderr]` and `[exit code]`) help the model extract specific information without reading everything.
- **Error observations are as important as success observations.** A clear error message like "File 'src/mian.rs' not found" is more actionable than "error" or a stack trace.

These principles will guide your tool implementations in Chapters 5 and 6.

## Key Takeaways

- Tool results become `tool_result` content blocks in a user message, linked to the original `tool_use` by `tool_use_id`.
- The `is_error` flag signals failure to the model, guiding its recovery strategy. Omit it (or set to `false`) for successful results.
- Structure tool outputs for easy parsing: add line numbers to file contents, label stdout/stderr/exit codes for shell output.
- Smart truncation preserves the most useful portions of large outputs. Keep the beginning and end for files; keep the end for command output.
- Observation quality directly affects agent performance. Concise, structured, labeled results outperform raw dumps.
