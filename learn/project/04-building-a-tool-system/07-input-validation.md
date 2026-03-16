---
title: Input Validation
description: Validate tool inputs against their JSON schemas before execution to catch malformed arguments early.
---

# Input Validation

> **What you'll learn:**
> - Why input validation is necessary even though the model usually produces valid JSON for the schema
> - How to use the jsonschema crate to validate a `serde_json::Value` against a tool's input schema
> - How to produce clear validation error messages that can be fed back as observations to help the model self-correct

The dispatch function from the previous subchapter takes the model's input and passes it straight to `tool.execute()`. That works when the model sends well-formed arguments -- and it usually does. But "usually" is not "always." Models hallucinate, produce partial JSON, misremember parameter names, and send strings where integers are expected. Input validation catches these errors *before* the tool runs, giving you cleaner error messages and preventing tools from receiving garbage data.

## Why Validate?

You might wonder: if the model sees the JSON Schema and generally follows it, why add validation? Three reasons:

**1. Models are not perfect.** Even the best models occasionally produce invalid tool inputs. This happens more often with complex schemas, when the model is under token pressure, or when it is trying to use a tool in an unusual way. A missing required field or a wrong type can cause confusing runtime errors inside the tool. Validation catches these at the boundary.

**2. Defense in depth.** Your tool's `execute` method should not have to worry about whether its input is valid. If validation runs first, the tool can assume its input conforms to the schema. This simplifies tool implementations: you extract fields with confidence rather than checking every access.

**3. Better error messages.** A schema validation error says "property 'path' is required but missing." A runtime error deep inside the tool says "called `Option::unwrap()` on a `None` value." The first helps the model self-correct. The second is opaque.

## Adding the jsonschema Crate

The `jsonschema` crate validates `serde_json::Value` against a JSON Schema. Add it to your `Cargo.toml`:

```toml
[dependencies]
jsonschema = "0.28"
serde_json = "1"
```

The crate compiles a schema into a validator object, which you then use to check input values. Here is the basic usage:

```rust
use jsonschema::draft7;
use serde_json::json;

fn main() {
    let schema = json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "The file path to read."
            }
        },
        "required": ["path"]
    });

    // Compile the schema into a validator
    let validator = draft7::new(&schema)
        .expect("Invalid schema");

    // Valid input
    let good_input = json!({"path": "src/main.rs"});
    assert!(validator.validate(&good_input).is_ok());

    // Invalid input: missing required field
    let bad_input = json!({});
    let result = validator.validate(&bad_input);
    assert!(result.is_err());

    // Invalid input: wrong type
    let wrong_type = json!({"path": 42});
    let result = validator.validate(&wrong_type);
    assert!(result.is_err());

    println!("All validation checks passed!");
}
```

The `draft7::new` function compiles the schema according to JSON Schema Draft 7, which is the version the Anthropic API uses. The resulting validator can be reused for multiple inputs without recompiling the schema.

## Collecting Validation Errors

When validation fails, you want all the errors, not just the first one. The model may have gotten multiple things wrong at once. The `jsonschema` crate can provide detailed error information:

```rust
use jsonschema::draft7;
use serde_json::json;

fn validate_input(schema: &serde_json::Value, input: &serde_json::Value) -> Result<(), String> {
    let validator = draft7::new(schema)
        .map_err(|e| format!("Invalid schema: {}", e))?;

    let result = validator.validate(input);
    if let Err(errors) = result {
        let error_messages: Vec<String> = errors
            .map(|e| format!("- {}", e))
            .collect();

        return Err(format!(
            "Input validation failed:\n{}",
            error_messages.join("\n")
        ));
    }

    Ok(())
}

fn main() {
    let schema = json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "File path."
            },
            "offset": {
                "type": "integer",
                "description": "Start line."
            }
        },
        "required": ["path"]
    });

    // Test with multiple errors
    let bad_input = json!({"offset": "not a number"});
    match validate_input(&schema, &bad_input) {
        Ok(()) => println!("Valid!"),
        Err(e) => println!("{}", e),
    }
}
```

This produces output like:

```
Input validation failed:
- "path" is a required property
- "not a number" is not of type "integer"
```

Each error message is clear enough for the model to understand what it needs to fix. The model can read this observation and retry with corrected arguments.

::: python Coming from Python
In Python, you might use the `jsonschema` package (same name, different ecosystem):

```python
import jsonschema

schema = {
    "type": "object",
    "properties": {"path": {"type": "string"}},
    "required": ["path"],
}

try:
    jsonschema.validate(instance={}, schema=schema)
except jsonschema.ValidationError as e:
    print(f"Validation error: {e.message}")
```

The Rust `jsonschema` crate works similarly but returns an iterator of errors rather than throwing the first one. Both libraries implement the same JSON Schema specification, so validation behavior is consistent.
:::

## Integrating Validation into Dispatch

Now let's wire validation into the dispatch function. Validation should run *after* the tool is found in the registry but *before* `execute` is called:

```rust
use jsonschema::draft7;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
pub enum ToolError {
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

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    fn execute(&self, input: &Value) -> Result<String, ToolError>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self { ToolRegistry { tools: HashMap::new() } }
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }
}

pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
}

pub struct ToolUse {
    pub id: String,
    pub name: String,
    pub input: Value,
}

/// Validate input against the tool's schema.
fn validate_tool_input(tool: &dyn Tool, input: &Value) -> Result<(), String> {
    let schema = tool.input_schema();

    let validator = match draft7::new(&schema) {
        Ok(v) => v,
        Err(e) => {
            // Schema itself is invalid -- this is a system error, not a user error
            return Err(format!("Internal error: invalid tool schema: {}", e));
        }
    };

    let result = validator.validate(input);
    if let Err(errors) = result {
        let messages: Vec<String> = errors
            .map(|e| format!("- {}", e))
            .collect();

        return Err(format!(
            "Invalid input for tool '{}':\n{}",
            tool.name(),
            messages.join("\n")
        ));
    }

    Ok(())
}

/// Dispatch with validation: look up, validate, then execute.
pub fn dispatch_tool_call(
    registry: &ToolRegistry,
    tool_use: &ToolUse,
) -> ToolResult {
    // Step 1: Look up the tool
    let tool = match registry.get(&tool_use.name) {
        Some(t) => t,
        None => {
            return ToolResult {
                tool_use_id: tool_use.id.clone(),
                content: format!("Error: Unknown tool '{}'", tool_use.name),
                is_error: true,
            };
        }
    };

    // Step 2: Validate input against schema
    if let Err(validation_error) = validate_tool_input(tool, &tool_use.input) {
        return ToolResult {
            tool_use_id: tool_use.id.clone(),
            content: validation_error,
            is_error: true,
        };
    }

    // Step 3: Execute the tool
    match tool.execute(&tool_use.input) {
        Ok(output) => ToolResult {
            tool_use_id: tool_use.id.clone(),
            content: output,
            is_error: false,
        },
        Err(e) => ToolResult {
            tool_use_id: tool_use.id.clone(),
            content: e.to_string(),
            is_error: true,
        },
    }
}
```

The dispatch function now has three stages: **lookup**, **validate**, **execute**. Each stage can produce a `ToolResult` with `is_error: true` if something goes wrong. The error flows back to the model as an observation, and the model can adjust its approach.

## Performance Consideration: Caching Validators

Compiling a schema into a validator takes time. If your agentic loop dispatches thousands of tool calls (unlikely, but possible with heavy search tools), recompiling the schema on every call is wasteful. You can optimize by caching compiled validators in the registry:

```rust
use jsonschema::draft7;
use serde_json::Value;

/// A pre-compiled validator paired with the tool name, for error messages.
pub struct CachedValidator {
    tool_name: String,
    // Store the compiled validator once
    inner: jsonschema::Draft7Validator,
}

impl CachedValidator {
    pub fn new(tool_name: &str, schema: &Value) -> Result<Self, String> {
        let inner = draft7::new(schema)
            .map_err(|e| format!("Invalid schema for '{}': {}", tool_name, e))?;
        Ok(CachedValidator {
            tool_name: tool_name.to_string(),
            inner,
        })
    }

    pub fn validate(&self, input: &Value) -> Result<(), String> {
        let result = self.inner.validate(input);
        if let Err(errors) = result {
            let messages: Vec<String> = errors
                .map(|e| format!("- {}", e))
                .collect();
            return Err(format!(
                "Invalid input for '{}':\n{}",
                self.tool_name,
                messages.join("\n")
            ));
        }
        Ok(())
    }
}
```

You would compile the validator once at registration time and store it alongside the tool in the registry. For now, the simpler approach of compiling on each validation call is fine -- tool calls in an agentic loop typically number in the tens, not thousands.

## Validation as a Safety Net

Validation is not just about catching model errors. It also catches bugs in your own tool schemas. If you define a schema that says a field is `"type": "integer"` but your `execute` method tries to read it as a string, validation will never flag this mismatch -- but the tool will fail at runtime. By validating inputs and writing tests that exercise the validation layer, you build confidence that the schema and the implementation agree.

In the testing subchapter (subchapter 12), you will write tests that send both valid and invalid inputs through the dispatch pipeline and verify that validation produces the expected errors.

## Key Takeaways

- Input validation catches malformed arguments before they reach `execute`, producing clearer error messages that help the model self-correct.
- The `jsonschema` crate compiles schemas into validators that check `serde_json::Value` inputs against JSON Schema Draft 7.
- Validation errors are collected into a single message listing all issues, not just the first one, so the model can fix everything in one retry.
- Validation integrates between tool lookup and execution in the dispatch pipeline: lookup, validate, execute.
- For high-throughput scenarios, compile validators once at registration time and cache them alongside the tools.
