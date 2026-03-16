---
title: Testing Tools
description: Write unit and integration tests for individual tools, the registry, and the dispatch pipeline.
---

# Testing Tools

> **What you'll learn:**
> - How to unit test a tool's execute method with mock inputs and assert on the output structure
> - How to integration test the full dispatch pipeline from a raw tool_use block through to the observation
> - How to use test fixtures and temporary directories to test file-system tools without side effects

A tool system has many moving parts: the trait implementation, the schema, the registry, the dispatch, the validation, and the observation formatting. Each part can break independently. Testing gives you confidence that the pieces work individually and together. In this subchapter you write tests at three levels: unit tests for individual tools, integration tests for the dispatch pipeline, and property-based checks for schema correctness.

## Unit Testing a Tool's Execute Method

The most basic test verifies that a tool produces the expected output for a given input. Here is how to test the `EchoTool`:

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
    fn description(&self) -> &str { "Echoes the input message back." }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_echo_tool_success() {
        let tool = EchoTool;
        let input = json!({"message": "hello"});
        let result = tool.execute(&input);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Echo: hello");
    }

    #[test]
    fn test_echo_tool_missing_message() {
        let tool = EchoTool;
        let input = json!({});
        let result = tool.execute(&input);

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            ToolError::InvalidInput(msg) => {
                assert!(msg.contains("message"), "Error should mention the missing field");
            }
            _ => panic!("Expected InvalidInput, got {:?}", err),
        }
    }

    #[test]
    fn test_echo_tool_wrong_type() {
        let tool = EchoTool;
        let input = json!({"message": 42});
        let result = tool.execute(&input);

        // as_str() returns None for a number, so this should be InvalidInput
        assert!(result.is_err());
    }

    #[test]
    fn test_echo_tool_name() {
        let tool = EchoTool;
        assert_eq!(tool.name(), "echo");
    }

    #[test]
    fn test_echo_tool_schema_is_object() {
        let tool = EchoTool;
        let schema = tool.input_schema();
        assert_eq!(schema["type"], "object");
    }
}

fn main() {
    // Run tests with: cargo test
    println!("Run 'cargo test' to execute the test suite.");
}
```

These tests cover the happy path, the missing-field case, the wrong-type case, and basic metadata. Each test is focused on one behavior.

::: python Coming from Python
If you have used pytest, this will feel familiar. Rust's built-in test framework uses `#[test]` annotations instead of function name prefixes:

```python
# Python with pytest
def test_echo_tool_success():
    tool = EchoTool()
    result = tool.execute({"message": "hello"})
    assert result == "Echo: hello"
```

The Rust version uses `assert!`, `assert_eq!`, and pattern matching instead of Python's bare `assert`. The `#[cfg(test)]` module is compiled only during `cargo test`, so test code does not affect your release binary.
:::

## Testing File Tools with Temporary Directories

Tools that interact with the file system need isolated environments. Use the `tempfile` crate to create temporary directories that are automatically cleaned up after each test.

Add `tempfile` to your dev dependencies in `Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
```

Here is how to test a file-reading tool:

```rust
#[cfg(test)]
mod file_tool_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// A minimal read tool for testing purposes.
    struct TestReadFileTool;

    impl Tool for TestReadFileTool {
        fn name(&self) -> &str { "read_file" }
        fn description(&self) -> &str { "Read a file." }
        fn input_schema(&self) -> Value {
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path." }
                },
                "required": ["path"]
            })
        }
        fn execute(&self, input: &Value) -> Result<String, ToolError> {
            let path = input.get("path").and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::InvalidInput("Missing 'path'".into()))?;
            fs::read_to_string(path)
                .map_err(|e| ToolError::ExecutionFailed(format!("{}", e)))
        }
    }

    #[test]
    fn test_read_existing_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "hello, world").unwrap();

        let tool = TestReadFileTool;
        let input = json!({"path": file_path.to_str().unwrap()});
        let result = tool.execute(&input);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello, world");
    }

    #[test]
    fn test_read_nonexistent_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("does_not_exist.txt");

        let tool = TestReadFileTool;
        let input = json!({"path": file_path.to_str().unwrap()});
        let result = tool.execute(&input);

        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::ExecutionFailed(msg) => {
                assert!(msg.contains("No such file") || msg.contains("not found"),
                    "Error message should indicate file not found, got: {}", msg);
            }
            other => panic!("Expected ExecutionFailed, got {:?}", other),
        }
    }

    #[test]
    fn test_read_empty_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("empty.txt");
        fs::write(&file_path, "").unwrap();

        let tool = TestReadFileTool;
        let input = json!({"path": file_path.to_str().unwrap()});
        let result = tool.execute(&input);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }
}
```

Each test creates a fresh `TempDir`, writes any needed fixtures, runs the tool, and asserts on the result. When the `TempDir` goes out of scope, the directory and its contents are deleted. This means tests do not interfere with each other and do not leave artifacts on disk.

## Integration Testing the Dispatch Pipeline

Unit tests verify individual tools. Integration tests verify that the entire pipeline works together: registry lookup, validation, execution, and result formatting.

```rust
use std::collections::HashMap;

struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    fn new() -> Self { ToolRegistry { tools: HashMap::new() } }
    fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }
    fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }
}

struct ToolUse {
    id: String,
    name: String,
    input: Value,
}

struct ToolResult {
    tool_use_id: String,
    content: String,
    is_error: bool,
}

fn dispatch_tool_call(registry: &ToolRegistry, tool_use: &ToolUse) -> ToolResult {
    let tool = match registry.get(&tool_use.name) {
        Some(t) => t,
        None => return ToolResult {
            tool_use_id: tool_use.id.clone(),
            content: format!("Unknown tool: {}", tool_use.name),
            is_error: true,
        },
    };

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

#[cfg(test)]
mod integration_tests {
    use super::*;

    fn setup_registry() -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));
        registry
    }

    #[test]
    fn test_dispatch_known_tool() {
        let registry = setup_registry();
        let tool_use = ToolUse {
            id: "toolu_01ABC".to_string(),
            name: "echo".to_string(),
            input: json!({"message": "hello"}),
        };

        let result = dispatch_tool_call(&registry, &tool_use);

        assert_eq!(result.tool_use_id, "toolu_01ABC");
        assert!(!result.is_error);
        assert_eq!(result.content, "Echo: hello");
    }

    #[test]
    fn test_dispatch_unknown_tool() {
        let registry = setup_registry();
        let tool_use = ToolUse {
            id: "toolu_02DEF".to_string(),
            name: "nonexistent".to_string(),
            input: json!({}),
        };

        let result = dispatch_tool_call(&registry, &tool_use);

        assert_eq!(result.tool_use_id, "toolu_02DEF");
        assert!(result.is_error);
        assert!(result.content.contains("nonexistent"));
    }

    #[test]
    fn test_dispatch_invalid_input() {
        let registry = setup_registry();
        let tool_use = ToolUse {
            id: "toolu_03GHI".to_string(),
            name: "echo".to_string(),
            input: json!({}), // Missing required "message"
        };

        let result = dispatch_tool_call(&registry, &tool_use);

        assert_eq!(result.tool_use_id, "toolu_03GHI");
        assert!(result.is_error);
    }

    #[test]
    fn test_tool_use_id_preserved() {
        let registry = setup_registry();
        let test_id = "toolu_custom_id_12345";
        let tool_use = ToolUse {
            id: test_id.to_string(),
            name: "echo".to_string(),
            input: json!({"message": "test"}),
        };

        let result = dispatch_tool_call(&registry, &tool_use);
        assert_eq!(result.tool_use_id, test_id,
            "tool_use_id must be preserved from request to result");
    }
}
```

The integration tests verify the full path: registry lookup, tool execution, result construction, and ID preservation. The `test_tool_use_id_preserved` test is particularly important -- if the ID does not match, the API will reject your response.

## Testing Schema Correctness

Your schemas should be valid JSON Schema and should match the tool's actual behavior. Here are tests that verify schema properties:

```rust
#[cfg(test)]
mod schema_tests {
    use super::*;

    #[test]
    fn test_schema_has_object_type() {
        let tool = EchoTool;
        let schema = tool.input_schema();
        assert_eq!(
            schema.get("type").and_then(|t| t.as_str()),
            Some("object"),
            "Tool schema must have type: object"
        );
    }

    #[test]
    fn test_schema_has_properties() {
        let tool = EchoTool;
        let schema = tool.input_schema();
        assert!(
            schema.get("properties").is_some(),
            "Tool schema must have a properties field"
        );
    }

    #[test]
    fn test_required_fields_exist_in_properties() {
        let tool = EchoTool;
        let schema = tool.input_schema();

        let required = schema.get("required")
            .and_then(|r| r.as_array())
            .expect("Schema should have a required array");

        let properties = schema.get("properties")
            .and_then(|p| p.as_object())
            .expect("Schema should have properties object");

        for field in required {
            let field_name = field.as_str().unwrap();
            assert!(
                properties.contains_key(field_name),
                "Required field '{}' must exist in properties",
                field_name
            );
        }
    }

    #[test]
    fn test_all_properties_have_descriptions() {
        let tool = EchoTool;
        let schema = tool.input_schema();

        let properties = schema.get("properties")
            .and_then(|p| p.as_object())
            .expect("Schema should have properties");

        for (name, prop) in properties {
            assert!(
                prop.get("description").is_some(),
                "Property '{}' must have a description",
                name
            );
        }
    }
}
```

These schema tests are reusable. You can write a helper function that runs all of them for any tool:

```rust
#[cfg(test)]
fn assert_valid_tool_schema(tool: &dyn Tool) {
    let schema = tool.input_schema();

    // Must be an object type
    assert_eq!(schema["type"], "object", "{}: schema must be object type", tool.name());

    // Required fields must exist in properties
    if let (Some(required), Some(properties)) = (
        schema.get("required").and_then(|r| r.as_array()),
        schema.get("properties").and_then(|p| p.as_object()),
    ) {
        for field in required {
            let name = field.as_str().unwrap();
            assert!(properties.contains_key(name),
                "{}: required field '{}' missing from properties", tool.name(), name);
        }
    }

    // All properties should have descriptions
    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        for (name, prop) in properties {
            assert!(prop.get("description").is_some(),
                "{}: property '{}' missing description", tool.name(), name);
        }
    }
}
```

Call `assert_valid_tool_schema(&EchoTool)` in any tool's test module to run the full suite of schema checks. When you add a new tool in Chapter 5, you get schema validation for free.

## Test Organization

As your tool system grows, organize tests into separate files:

```
src/
  tools/
    mod.rs          // Tool trait + ToolError
    echo.rs         // EchoTool implementation
    registry.rs     // ToolRegistry
    dispatch.rs     // dispatch_tool_call
tests/
  tool_echo.rs      // Unit tests for EchoTool
  tool_registry.rs  // Registry tests
  dispatch.rs       // Integration tests
```

Rust's `tests/` directory is for integration tests that access your crate's public API. The `#[cfg(test)]` modules inside `src/` are for unit tests that can access private internals. Use both: unit tests for individual tool logic, integration tests for the full pipeline.

## Key Takeaways

- Unit test each tool's `execute` method with valid inputs, missing fields, wrong types, and edge cases like empty inputs.
- Use the `tempfile` crate for file-system tools to create isolated test environments that clean up automatically.
- Integration tests exercise the full dispatch pipeline: registry lookup, execution, result formatting, and ID preservation.
- Schema tests verify structural correctness: `type: object` at top level, required fields in properties, descriptions on all properties.
- Write a reusable `assert_valid_tool_schema` helper that you can apply to every tool as you build them in subsequent chapters.
