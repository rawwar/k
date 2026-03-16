---
title: Adding Tool Descriptions
description: Write clear tool descriptions that help the model understand when and how to use each tool correctly.
---

# Adding Tool Descriptions

> **What you'll learn:**
> - How tool descriptions function as prompts that guide the model's tool-selection decisions
> - How to write descriptions that specify the tool's purpose, expected inputs, and common use patterns
> - How to include usage examples and edge-case guidance in descriptions to reduce tool-call errors

You have built the trait, registry, dispatch, validation, and execution layers. The system works. But there is one piece that is easy to overlook and expensive to get wrong: the text descriptions attached to each tool. These descriptions are not documentation for humans -- they are prompts for the model. The model reads them and uses them to decide *when* to call a tool, *how* to construct the arguments, and *what to expect* from the result. A mediocre description leads to wasted tool calls, wrong arguments, and confusion.

## Descriptions Are Prompts

When you send the `tools` array to the API, each tool has a `description` field:

```json
{
  "name": "read_file",
  "description": "Read the contents of a file...",
  "input_schema": { ... }
}
```

The model treats this description the same way it treats the system prompt -- as instructions that guide its behavior. A description that says "reads a file" gives the model almost nothing to work with. A description that says "Read the contents of a file at the given path. Returns the file contents with line numbers. Use this when you need to examine existing code before making changes" tells the model three things: what the tool does, what it returns, and when to use it.

## Anatomy of a Good Description

A good tool description has four parts:

1. **What it does** -- One sentence describing the tool's action.
2. **What it returns** -- What the output looks like so the model knows what to expect.
3. **When to use it** -- Guidance on the appropriate situations for this tool.
4. **Caveats or limits** -- Anything that might cause surprising behavior.

Let's apply this to the tools you will build in the next chapters.

### Read File Description

```rust
use serde_json::{json, Value};

fn read_file_description() -> &'static str {
    "Read the contents of a file at the given path. Returns the file \
     contents with line numbers prefixed to each line. Use this to \
     examine existing code before making edits. For large files, use \
     the offset and limit parameters to read a specific range of lines \
     rather than the entire file."
}
```

This tells the model: the tool reads files, output has line numbers, use it before editing, and there are parameters for large files.

### Write File Description

```rust
fn write_file_description() -> &'static str {
    "Create or overwrite a file at the given path with the provided \
     content. The full file content must be provided — this tool does \
     not support partial writes. Use the edit_file tool instead if you \
     only need to change part of a file. Parent directories are created \
     automatically if they do not exist."
}
```

The critical phrase here is "Use the edit_file tool instead if you only need to change part of a file." Without this guidance, models often use `write_file` to rewrite entire files when they only need to change one function. This wastes tokens and risks introducing errors in the unchanged portions.

### Shell Execution Description

```rust
fn shell_description() -> &'static str {
    "Execute a shell command and return its stdout, stderr, and exit \
     code. Commands run in the project's root directory by default. \
     Use this for running tests (cargo test), checking compilation \
     (cargo check), installing dependencies, and any other terminal \
     commands. Commands time out after 30 seconds by default. Long \
     running commands like servers will be killed at the timeout."
}
```

The timeout information prevents the model from trying to start a web server and waiting for it to respond -- a common failure mode without this guidance.

## Property Descriptions Matter Too

The `description` fields on individual schema properties are just as important as the top-level tool description. They guide the model in constructing correct arguments:

```rust
fn edit_file_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "The path to the file to edit. Must be an existing file."
            },
            "old_string": {
                "type": "string",
                "description": "The exact text to find and replace. Must match the \
                    file contents exactly, including whitespace and indentation. \
                    If the string appears multiple times, only the first occurrence \
                    is replaced."
            },
            "new_string": {
                "type": "string",
                "description": "The replacement text. Can be empty to delete the \
                    old_string."
            }
        },
        "required": ["path", "old_string", "new_string"]
    })
}
```

The `old_string` description is particularly important. Without "Must match the file contents exactly, including whitespace and indentation," models frequently produce edit commands that fail because of indentation mismatches. This single sentence saves many failed tool calls.

::: python Coming from Python
In Python, you might put this guidance in docstrings or Pydantic field descriptions:

```python
from pydantic import BaseModel, Field

class EditFileInput(BaseModel):
    path: str = Field(description="The path to the file to edit.")
    old_string: str = Field(
        description="The exact text to find and replace. Must match "
        "the file contents exactly, including whitespace."
    )
    new_string: str = Field(description="The replacement text.")
```

The concept is identical. In Rust, you put these descriptions in the `json!` macro when building the schema. The descriptions serve the same purpose: telling the model how to use the parameter correctly.
:::

## Anti-Patterns to Avoid

Here are common description mistakes and why they hurt:

**Too vague:** "Reads a file." The model does not know what the output looks like or when to prefer this tool over others.

**Too verbose:** A 500-word description that covers every edge case exhausts the model's attention. Keep descriptions under 100 words. Use the schema's property descriptions for parameter-specific details.

**Missing return format:** "Writes a file." Does it return the written content? A confirmation message? Nothing? The model needs to know what to expect so it can plan its next action.

**No disambiguation:** If you have both `read_file` and `search_files`, the model needs to know when to use which. "Use read_file when you know the exact path; use search_files when you need to find files matching a pattern."

**No limits mentioned:** If a tool has a timeout, a maximum output size, or a rate limit, mention it. Otherwise the model will discover these limits through errors, wasting turns.

## Iterative Description Refinement

Tool descriptions are not write-once. You will refine them as you observe how the model uses your tools. The process is:

1. **Write an initial description** following the four-part anatomy.
2. **Run the agent** on real tasks and watch for tool misuse.
3. **Identify patterns** -- does the model use `write_file` when it should use `edit_file`? Does it pass absolute paths when the tool expects relative?
4. **Update the description** to address the observed confusion.
5. **Repeat.**

This iterative loop is one of the most impactful things you can do to improve agent quality. A single well-placed sentence in a tool description can eliminate an entire category of errors.

::: wild In the Wild
Claude Code's tool descriptions have been refined through extensive usage and feedback. For example, the `Edit` tool's description explicitly states that the `old_string` must be unique within the file. This was added after observing that the model sometimes produced edits that matched multiple locations, causing unexpected changes. OpenCode similarly iterates on its tool descriptions, treating them as a form of prompt engineering. The descriptions in production agents are typically 2-3 sentences long -- concise but carefully crafted.
:::

## A Complete Example: Registering Tools with Polished Descriptions

Let's bring it all together by defining a tool with a well-crafted description and registering it:

```rust
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
enum ToolError {
    InvalidInput(String),
    ExecutionFailed(String),
    SystemError(String),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            ToolError::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            ToolError::SystemError(msg) => write!(f, "System error: {}", msg),
        }
    }
}

trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    fn execute(&self, input: &Value) -> Result<String, ToolError>;
}

struct ReadFileTool;

impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file at the given path. Returns the file \
         contents with line numbers prefixed to each line (e.g., '  1 | fn main()').  \
         Use this to examine existing code before making edits. For large files, \
         use the offset and limit parameters to read a specific range of lines."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The absolute or relative path to the file to read."
                },
                "offset": {
                    "type": "integer",
                    "description": "The 1-based line number to start reading from. \
                        Defaults to 1 (beginning of file)."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to return. Defaults to \
                        the entire file. Use this for large files to avoid excessive output."
                }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, input: &Value) -> Result<String, ToolError> {
        let path = input.get("path").and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'path'".into()))?;

        let contents = std::fs::read_to_string(path)
            .map_err(|e| ToolError::ExecutionFailed(
                format!("Cannot read '{}': {}", path, e)
            ))?;

        let offset = input.get("offset")
            .and_then(|v| v.as_i64())
            .unwrap_or(1) as usize;

        let limit = input.get("limit")
            .and_then(|v| v.as_i64())
            .map(|v| v as usize);

        let lines: Vec<String> = contents.lines()
            .enumerate()
            .skip(offset.saturating_sub(1))
            .take(limit.unwrap_or(usize::MAX))
            .map(|(i, line)| format!("{:>4} | {}", i + 1, line))
            .collect();

        Ok(format!("Contents of {}:\n{}", path, lines.join("\n")))
    }
}

fn main() {
    let tool = ReadFileTool;

    println!("Name: {}", tool.name());
    println!("Description: {}", tool.description());
    println!("\nSchema:");
    println!("{}", serde_json::to_string_pretty(&tool.input_schema()).unwrap());

    // Execute on a real file
    let input = json!({"path": "Cargo.toml"});
    match tool.execute(&input) {
        Ok(output) => println!("\nOutput:\n{}", output),
        Err(e) => println!("\nError: {}", e),
    }
}
```

This `ReadFileTool` demonstrates every principle: the description explains what, how, when, and limitations. The property descriptions guide argument construction. The output format matches what the description promises.

## Key Takeaways

- Tool descriptions are prompts, not documentation. The model reads them to decide when and how to use each tool.
- A good description covers four things: what the tool does, what it returns, when to use it, and any caveats or limits.
- Property-level descriptions in the schema are equally important. "Must match exactly, including whitespace" prevents a whole category of errors.
- Disambiguate similar tools explicitly: tell the model when to use `read_file` vs `search_files`, or `write_file` vs `edit_file`.
- Descriptions are iterative. Watch how the model uses your tools, identify patterns of misuse, and refine the descriptions to address them.
