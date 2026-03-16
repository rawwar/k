---
title: Defining Tool Schemas
description: Build JSON schema definitions for your tools using serde_json and validate them against the Anthropic API format.
---

# Defining Tool Schemas

> **What you'll learn:**
> - How to construct JSON schema objects programmatically using `serde_json::json!` macros
> - How to derive schemas from Rust types using the schemars crate as an alternative to manual definitions
> - How to format tool definitions as the `tools` array expected by the Anthropic Messages API

You now understand JSON Schema as a format. In this subchapter you put that knowledge to work by building schemas for your tools in Rust and assembling them into the `tools` array that the Anthropic API expects. You will see two approaches: manual construction with `serde_json::json!` and automatic derivation with the `schemars` crate. Each has trade-offs, and you will understand when to use which.

## Approach 1: Manual Schemas with `json!`

The most direct way to define a tool schema is to write it as a JSON literal using the `serde_json::json!` macro. This is what your `EchoTool` already does. Let's build a more realistic example -- a `ReadFile` tool schema.

```rust
use serde_json::{json, Value};

fn read_file_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "The absolute or relative path to the file to read."
            },
            "offset": {
                "type": "integer",
                "description": "The 1-based line number to start reading from. Defaults to 1."
            },
            "limit": {
                "type": "integer",
                "description": "The maximum number of lines to return. Defaults to all lines."
            }
        },
        "required": ["path"]
    })
}
```

**Advantages of manual schemas:**
- No extra dependencies. You already have `serde_json` in your project.
- Complete control over the output. What you write is exactly what gets sent.
- Easy to add descriptions, examples, and constraints exactly where you want them.

**Disadvantages:**
- No compile-time validation. A typo in `"properteis"` compiles fine and fails silently.
- The schema and the parsing code can drift. If you add a field to the schema but forget to handle it in `execute`, or vice versa, the compiler will not warn you.
- Repetitive for tools with many parameters.

For most tools in this book, manual schemas are the right choice. They are explicit, easy to read, and avoid extra dependencies. The schemas are small enough that the risk of drift is manageable.

## Approach 2: Deriving Schemas with schemars

The `schemars` crate can generate a JSON Schema from a Rust struct automatically. You define a struct for the tool's input, derive `JsonSchema` on it, and call `schema_for!` to get the schema. Here is what that looks like:

First, add `schemars` to your `Cargo.toml`:

```toml
[dependencies]
schemars = "0.8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

Then define your input struct:

```rust
use schemars::JsonSchema;
use serde::Deserialize;

/// Input parameters for the read_file tool.
#[derive(Deserialize, JsonSchema)]
struct ReadFileInput {
    /// The absolute or relative path to the file to read.
    path: String,

    /// The 1-based line number to start reading from.
    #[serde(default)]
    offset: Option<i64>,

    /// The maximum number of lines to return.
    #[serde(default)]
    limit: Option<i64>,
}
```

Now generate the schema:

```rust
use schemars::schema_for;

fn read_file_schema_derived() -> serde_json::Value {
    let schema = schema_for!(ReadFileInput);
    serde_json::to_value(schema).unwrap()
}
```

**Advantages of derived schemas:**
- The schema and the Rust type are always in sync. Add a field to the struct and the schema updates automatically.
- You can deserialize the input directly into the struct in your `execute` method, getting compile-time type checking.
- Doc comments on struct fields become `description` values in the schema.

**Disadvantages:**
- Adds a dependency (`schemars`).
- The generated schema may include extra keywords (`$schema`, `title`, `definitions`) that you do not need and that add noise to the API request.
- Less control over the exact output. You might need post-processing to match the API's expected format.

::: python Coming from Python
This is the Rust equivalent of Pydantic's `.model_json_schema()`. In Python, you define a Pydantic model and get the schema for free:

```python
from pydantic import BaseModel, Field
from typing import Optional

class ReadFileInput(BaseModel):
    path: str = Field(description="The file path to read.")
    offset: Optional[int] = Field(default=None, description="Start line.")
    limit: Optional[int] = Field(default=None, description="Max lines.")

schema = ReadFileInput.model_json_schema()
```

The `schemars` crate fills the same role in Rust. The generated schemas are similar but not identical -- Pydantic uses `$defs` for nested models, while schemars uses `definitions`.
:::

## Which Approach to Use

For this book, you will use **manual schemas with `json!`** as the primary approach. The reasons:

1. **Fewer dependencies.** You are learning Rust; every new crate is a new concept to track.
2. **Transparency.** When you read the tool's `input_schema` method, you see exactly what gets sent to the API. No hidden transformations.
3. **Tool schemas are small.** Most tools have 1-5 parameters. The overhead of writing them by hand is minimal.

If you later build a tool with 10+ parameters or you want to guarantee schema-struct consistency, switch to `schemars` for that tool. The trait does not care how you produce the `Value` -- both approaches return the same type.

## Assembling the Tools Array

The Anthropic API expects a `tools` array where each element has `name`, `description`, and `input_schema`. You need a function that takes your registered tools and produces this array. Here is how:

```rust
use serde_json::{json, Value};

/// Represents a tool definition as the API expects it.
fn tool_definition(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "input_schema": input_schema
    })
}

/// Given a collection of tools, produce the tools array for the API request.
fn build_tools_array(tools: &[Box<dyn Tool>]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            tool_definition(tool.name(), tool.description(), tool.input_schema())
        })
        .collect()
}
```

This function iterates over all registered tools, calls the trait methods to get the name, description, and schema, and assembles them into the format the API expects. You will integrate this into the registry in subchapter 5.

Let's see a complete working example that assembles definitions from multiple tools:

```rust
use serde_json::{json, Value};
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

struct EchoTool;

impl Tool for EchoTool {
    fn name(&self) -> &str { "echo" }
    fn description(&self) -> &str {
        "Echoes the input message back. Useful for testing."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The message to echo back."
                }
            },
            "required": ["message"]
        })
    }
    fn execute(&self, input: &Value) -> Result<String, ToolError> {
        let msg = input.get("message").and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'message'".into()))?;
        Ok(format!("Echo: {}", msg))
    }
}

struct ReadFileTool;

impl Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }
    fn description(&self) -> &str {
        "Read the contents of a file at the given path."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The file path to read."
                }
            },
            "required": ["path"]
        })
    }
    fn execute(&self, input: &Value) -> Result<String, ToolError> {
        let path = input.get("path").and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'path'".into()))?;
        std::fs::read_to_string(path)
            .map_err(|e| ToolError::ExecutionFailed(format!("Cannot read {}: {}", path, e)))
    }
}

fn build_tools_array(tools: &[Box<dyn Tool>]) -> Vec<Value> {
    tools.iter().map(|tool| {
        json!({
            "name": tool.name(),
            "description": tool.description(),
            "input_schema": tool.input_schema()
        })
    }).collect()
}

fn main() {
    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(EchoTool),
        Box::new(ReadFileTool),
    ];

    let tools_array = build_tools_array(&tools);
    let output = serde_json::to_string_pretty(&tools_array).unwrap();
    println!("{}", output);
}
```

Running this produces a properly formatted `tools` array ready to include in an API request. Each element contains the `name`, `description`, and `input_schema` exactly as the API expects.

## Validating Your Schemas

Before sending a schema to the API, you should sanity-check it. Common mistakes include:

1. **Forgetting `"type": "object"` at the top level.** The API expects tool input schemas to be objects.
2. **Listing a field in `required` that is not in `properties`.** This is valid JSON Schema but confusing.
3. **Missing `description` on properties.** The schema will work, but the model will have less guidance for producing correct arguments.

A simple validation function catches the first two issues:

```rust
fn validate_tool_schema(schema: &Value) -> Result<(), String> {
    // Top level must be "object"
    if schema.get("type").and_then(|t| t.as_str()) != Some("object") {
        return Err("Tool input_schema must have \"type\": \"object\"".into());
    }

    // Every required field must exist in properties
    if let (Some(required), Some(properties)) = (
        schema.get("required").and_then(|r| r.as_array()),
        schema.get("properties").and_then(|p| p.as_object()),
    ) {
        for field in required {
            if let Some(name) = field.as_str() {
                if !properties.contains_key(name) {
                    return Err(format!(
                        "Required field '{}' not found in properties", name
                    ));
                }
            }
        }
    }

    Ok(())
}
```

Call this in your tests or at tool registration time. It is a lightweight safeguard that prevents subtle bugs from reaching the API.

## Key Takeaways

- Manual schemas with `serde_json::json!` are the simplest approach: no extra dependencies, full control, and transparent output.
- The `schemars` crate can derive schemas from Rust structs, keeping the schema and types in sync automatically -- useful for tools with many parameters.
- The API expects each tool definition to have `name`, `description`, and `input_schema` fields. The `build_tools_array` function assembles these from your trait methods.
- Always validate your schemas: ensure `"type": "object"` at the top level, check that `required` fields exist in `properties`, and include `description` on every property.
- Both manual and derived schemas produce `serde_json::Value`, so the trait does not care which approach you use. Choose per tool based on complexity.
