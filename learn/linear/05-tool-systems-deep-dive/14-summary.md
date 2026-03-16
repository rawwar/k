---
title: Summary
description: A recap of tool system design principles and how they prepare us for implementing specific tools starting with file operations.
---

# Summary

> **What you'll learn:**
> - A consolidated checklist for designing, implementing, and testing a new agent tool
> - The key design principles that make tools reliable, safe, and LLM-friendly
> - How the tool system concepts from this chapter apply directly to the file system tools in Chapter 6

You have now covered the complete landscape of tool system design -- from the conceptual foundations of why tools matter to the practical details of JSON Schema, validation, execution, error handling, security, and LLM-optimized design. Let's consolidate everything into a reference you can use every time you design a new tool.

## The Tool Design Lifecycle

When you add a new tool to your agent, you go through a repeatable lifecycle:

**Step 1: Define the Purpose**

Before writing any code, answer three questions:
- What does this tool do? (One sentence, one capability)
- When should the model use it? (Specific scenarios)
- When should the model NOT use it? (Common confusions with other tools)

If you cannot answer these clearly, the tool is not well-defined yet.

**Step 2: Design the Schema**

Define the input parameters:
- Keep required parameters to three or fewer
- Use `Option<T>` in Rust for parameters with sensible defaults
- Use enums to constrain string parameters
- Set `minimum`/`maximum` on numeric parameters
- Document defaults in parameter descriptions

```rust
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MyToolInput {
    /// Description of the required parameter.
    pub required_param: String,

    /// Description including the default value. Defaults to 50.
    pub optional_param: Option<u32>,

    /// Mode selection. Use 'fast' for quick results, 'thorough' for complete results.
    pub mode: Option<MyToolMode>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub enum MyToolMode {
    #[serde(rename = "fast")]
    Fast,
    #[serde(rename = "thorough")]
    Thorough,
}
```

**Step 3: Write the Description**

A complete description covers five areas:

1. What the tool does
2. What it returns
3. When to use it
4. When NOT to use it
5. Edge cases and constraints

```rust
const MY_TOOL_DESCRIPTION: &str = "\
Perform a specific operation on the specified target. \
Returns a summary of the operation including how many items were affected. \
Use this when you need to accomplish X. \
For Y, use other_tool instead — this tool does not handle Y. \
The target must be an absolute path within the project directory.";
```

**Step 4: Implement Validation**

Layer your validation in order:

```rust
pub fn validate_and_execute(
    raw_input: serde_json::Value,
    project_root: &str,
) -> Result<String, ToolError> {
    // Layer 1: Schema validation (handled by the framework)

    // Layer 2: Deserialization
    let input: MyToolInput = serde_json::from_value(raw_input)
        .map_err(|e| ToolError::ToolFailure {
            message: format!("Invalid input: {}", e),
            suggestion: None,
        })?;

    // Layer 3: Semantic validation
    validate_semantics(&input, project_root)?;

    // Layer 4: Permission check
    check_permissions(&input, project_root)?;

    // Execute
    execute(input)
}
```

**Step 5: Implement Execution**

Choose the execution model:
- **In-process** for read-only and simple mutation tools
- **Subprocess** for shell commands and external programs
- **Sandboxed subprocess** for shell commands with untrusted input

**Step 6: Format Results**

Return results that the model can use:
- Plain text for content (source code, logs)
- One line per item for lists (search results, directory listings)
- Summary counts at the end ("5 matches in 3 files")
- Truncation notices when output is cut short

**Step 7: Test with the Model**

Run the agent on tasks that exercise the new tool:
- Does the model find and select the tool correctly?
- Does it fill in parameters correctly on the first try?
- When the tool returns an error, does the model self-correct?
- Does the model confuse this tool with similar tools?

Iterate on the description and schema based on what you observe.

## The Principles at a Glance

Here are the key principles from each subchapter, distilled to their essence:

| Principle | Summary |
|---|---|
| Tools are the bridge | Without tools, an LLM is limited to text. Tools enable perception, mutation, and verification. |
| Schema precision matters | Use JSON Schema types, enums, and constraints to tell the model exactly what inputs are valid. |
| Descriptions are UX | The description is the primary signal for tool selection. Include what, when, and when-not-to-use. |
| Validate in layers | Schema, then deserialization, then semantics, then permissions. Stop at the first failure. |
| Choose the right executor | In-process for simple tools, subprocess for external programs, sandboxed for untrusted input. |
| Async for parallelism | Use async execution to run parallel tool calls concurrently and enforce timeouts. |
| Errors guide recovery | Tool errors go to the model with suggestions. System errors are handled by the agent. |
| Results should be scannable | Plain text, one item per line, summary counts. Truncate proactively. |
| Specialize tools | Five categories, each with unique safety profiles. Six tools make a minimum viable set. |
| Compose deliberately | Model-driven composition for flexibility, system-driven for efficiency and consistency. |
| Discover dynamically | Register tools based on the project environment. Manage the token cost of tool definitions. |
| Defend in depth | Layer path validation, deny lists, permissions, and sandboxing. No single layer is enough. |
| Design for the model | `verb_noun` names, few parameters, tight enums, error messages as instructions. |

## The Complete Tool Implementation Template

Here is a template you can follow for every new tool:

```rust
use schemars::JsonSchema;
use serde::Deserialize;

// --- Input Schema ---

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExampleToolInput {
    /// Description of what this parameter controls.
    pub primary_param: String,

    /// Description including the default. Defaults to some_value.
    pub optional_param: Option<String>,
}

// --- Tool Definition ---

pub fn tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "example_tool",
        description: "What this tool does in one sentence. \
            Returns what the tool produces. \
            Use when the model needs to accomplish X. \
            Do NOT use for Y — use other_tool instead. \
            The primary_param must be an absolute path.",
        input_schema: schemars::schema_for!(ExampleToolInput),
    }
}

// --- Validation ---

pub fn validate(input: &ExampleToolInput, project_root: &str) -> Result<(), ToolError> {
    // Semantic checks
    if input.primary_param.is_empty() {
        return Err(ToolError::ToolFailure {
            message: "primary_param must not be empty.".to_string(),
            suggestion: Some("Provide a valid value.".to_string()),
        });
    }

    // Path containment check (if applicable)
    if !input.primary_param.starts_with(project_root) {
        return Err(ToolError::ToolFailure {
            message: format!(
                "Path '{}' is outside the project directory.",
                input.primary_param
            ),
            suggestion: Some(format!(
                "Use a path under '{}'.", project_root
            )),
        });
    }

    Ok(())
}

// --- Execution ---

pub fn execute(input: ExampleToolInput) -> Result<String, ToolError> {
    // Your tool logic here
    let result = do_the_work(&input.primary_param)?;

    // Format the result for the model
    Ok(format_result(&result))
}

// --- Result Formatting ---

fn format_result(raw: &str) -> String {
    let lines: Vec<&str> = raw.lines().collect();
    if lines.len() > 2000 {
        format!(
            "{}\n\n[Truncated: showing first 2000 of {} lines]",
            lines[..2000].join("\n"),
            lines.len()
        )
    } else {
        raw.to_string()
    }
}

fn do_the_work(param: &str) -> Result<String, ToolError> {
    todo!("Implement tool logic")
}
```

::: python Coming from Python
If you have been following along and comparing with Python, here is the Python equivalent of the template for reference:
```python
from pydantic import BaseModel

class ExampleToolInput(BaseModel):
    """Input schema for the example tool."""
    primary_param: str
    optional_param: str | None = None

TOOL_DEFINITION = {
    "name": "example_tool",
    "description": "What this tool does. Returns what. Use when X. Not for Y.",
    "input_schema": ExampleToolInput.model_json_schema(),
}

def validate(inp: ExampleToolInput, project_root: str) -> None:
    if not inp.primary_param.startswith(project_root):
        raise ToolFailure(f"Path outside project: {inp.primary_param}")

def execute(inp: ExampleToolInput) -> str:
    result = do_the_work(inp.primary_param)
    return format_result(result)
```
The structure is identical: schema, definition, validation, execution, formatting. The language changes; the pattern does not.
:::

## What Comes Next

In Chapter 6 (File System Operations), you will implement the first batch of real tools: file reading, file writing, and file editing. Every concept from this chapter applies directly:

- You will define JSON Schemas for `read_file`, `write_file`, and `edit_file` using `schemars`
- You will write descriptions that disambiguate between reading, writing, and editing
- You will implement layered validation with path containment checks
- You will use in-process execution for file operations
- You will format results with line numbers and truncation notices
- You will restrict all operations to the project directory

The theory you have built in this chapter becomes practice in the next one.

## Key Takeaways

- Every new tool follows the same lifecycle: define purpose, design schema, write description, implement validation, implement execution, format results, test with the model
- The thirteen principles of tool design (one from each subchapter) form a comprehensive checklist for evaluating any tool
- Use the tool implementation template as a starting point for every new tool -- it encodes all the patterns from this chapter
- Tool design is iterative -- observe how the model uses your tools, identify misuse patterns, and refine descriptions and schemas accordingly
- The concepts from this chapter apply directly to the file system tools, shell execution, and code search tools you will build in the following chapters
