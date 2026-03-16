---
title: Tool Trait Design
description: Design a Rust trait that defines the interface every tool must implement for registration, schema, and execution.
---

# Tool Trait Design

> **What you'll learn:**
> - How to define an async `Tool` trait with methods for `name`, `description`, `schema`, and `execute`
> - How to use `#[async_trait]` to work around Rust's current limitations with async methods in traits
> - How to use `Box<dyn Tool>` for dynamic dispatch so the registry can hold tools of different concrete types

In the previous section you learned that tools are a contract between the model, the agent framework, and the tool implementation. Now you encode that contract in Rust. The central design decision is a trait called `Tool` that every tool struct must implement. If you get this trait right, every subsequent piece of the system -- the registry, the dispatcher, the schema generator -- falls into place naturally.

## Why a Trait?

Rust traits are the language's primary mechanism for defining shared behavior. A trait says "any type that implements me must provide these methods." This is exactly what you need: every tool must provide a name, a description, an input schema, and an execute function. The specific *implementation* differs -- a file reader tool behaves nothing like a shell executor -- but the *interface* is identical.

::: python Coming from Python
If you have used Python's `abc.ABC` or `typing.Protocol`, you already understand the concept. A Rust trait is like a Python abstract base class, but enforced at compile time rather than at runtime. Here is the Python equivalent:

```python
from abc import ABC, abstractmethod
from typing import Any

class Tool(ABC):
    @abstractmethod
    def name(self) -> str: ...

    @abstractmethod
    def description(self) -> str: ...

    @abstractmethod
    def input_schema(self) -> dict: ...

    @abstractmethod
    async def execute(self, input: dict[str, Any]) -> str: ...
```

The Rust version is more powerful because the compiler guarantees you cannot create a tool that forgets to implement one of these methods. In Python, you discover that at runtime.
:::

## The Tool Trait

Here is the trait definition you will use throughout the rest of the book. Start by creating a new file at `src/tools/mod.rs` (you will reorganize `main.rs` to use modules shortly):

```rust
use serde_json::Value;

/// The core trait that every tool must implement.
///
/// A tool has a name (used for dispatch), a description (sent to the LLM),
/// a JSON schema (defining valid inputs), and an execute method (performing
/// the actual work).
pub trait Tool: Send + Sync {
    /// Returns the unique name of this tool.
    /// This must match the name sent in the API's `tools` array.
    fn name(&self) -> &str;

    /// Returns a human-readable description of what this tool does.
    /// The LLM reads this to decide when to use the tool.
    fn description(&self) -> &str;

    /// Returns the JSON Schema that describes valid input for this tool.
    /// The LLM uses this to construct its `tool_use` arguments.
    fn input_schema(&self) -> Value;

    /// Execute the tool with the given input.
    /// Returns Ok(output_string) on success, Err(error_string) on failure.
    fn execute(&self, input: &Value) -> Result<String, ToolError>;
}
```

Let's walk through each part.

### `name(&self) -> &str`

This returns the tool's unique identifier. When the model emits a `tool_use` block, it includes a `name` field. Your dispatch logic uses this name to look up the right tool in the registry. The name must be stable -- changing it breaks any conversation that references the old name.

Convention: use `snake_case` names like `read_file`, `run_shell`, or `web_search`. These names appear in the API request and in the model's output, so keep them short and descriptive.

### `description(&self) -> &str`

This returns a natural-language description that the model uses to decide *when* to call the tool. A good description is specific: not just "reads a file" but "Read the contents of a file at the given path. Returns the file contents as a string with line numbers. Use this when you need to understand existing code before making changes."

You will refine these descriptions in subchapter 11. For now, a one-sentence summary is fine.

### `input_schema(&self) -> Value`

This returns a `serde_json::Value` representing the JSON Schema for the tool's input parameters. The schema tells the model what arguments the tool accepts, their types, which are required, and what each one means. You will learn how to build these schemas in subchapters 3 and 4.

### `execute(&self, input: &Value) -> Result<String, ToolError>`

This is where the work happens. The method receives the tool's input as a `serde_json::Value` (the parsed JSON object from the `tool_use` block) and returns either a success string or a `ToolError`. You accept `&Value` rather than a strongly-typed struct to keep the trait object-safe -- different tools have different input shapes, and the trait must accommodate all of them through a single interface.

## The ToolError Type

You need a dedicated error type to distinguish between different failure modes. Define it alongside the trait:

```rust
use std::fmt;

/// Represents the different ways a tool execution can fail.
#[derive(Debug)]
pub enum ToolError {
    /// The input did not match the tool's schema.
    InvalidInput(String),
    /// The tool ran but encountered an error (e.g., file not found).
    ExecutionFailed(String),
    /// A system-level error prevented the tool from running at all.
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

impl std::error::Error for ToolError {}
```

The three variants serve different purposes later in the dispatch pipeline:

- **`InvalidInput`** means the model sent malformed arguments. The error message should be descriptive enough for the model to self-correct on retry.
- **`ExecutionFailed`** means the tool ran but something went wrong -- a file did not exist, a command returned a non-zero exit code. This is a normal tool error, not a bug in your code.
- **`SystemError`** means something unexpected happened -- a panic, a timeout, a resource exhaustion. This might warrant logging or alerting, not just a retry.

## The Send + Sync Bounds

Notice the `Tool: Send + Sync` bound on the trait definition. This is crucial. Your tool registry will store tools as `Box<dyn Tool>`, and the agentic loop runs in an async runtime (tokio). The `Send` bound means a tool can be transferred between threads. The `Sync` bound means a tool can be referenced from multiple threads simultaneously. Without these bounds, the compiler will reject your code when you try to use tools across `.await` points.

In practice, this means your tool structs should not contain `Rc`, `Cell`, or other non-thread-safe types. If a tool needs mutable state, use `Arc<Mutex<T>>`. Most tools you will build are stateless, so these bounds cost nothing.

## Dynamic Dispatch with `Box<dyn Tool>`

The registry needs to store tools of different concrete types in a single collection. A `HashMap<String, Box<dyn Tool>>` accomplishes this through dynamic dispatch. Each `Box<dyn Tool>` is a fat pointer: one pointer to the data and one pointer to the vtable (a table of function pointers for the trait methods).

```rust
use std::collections::HashMap;

// This is what the registry will hold internally:
type ToolMap = HashMap<String, Box<dyn Tool>>;
```

When you call `tool.execute(input)`, the runtime looks up `execute` in the vtable and jumps to the correct implementation. This adds a tiny cost (one pointer indirection per call) compared to static dispatch, but it is negligible for tool execution -- the tool itself will do I/O that dwarfs any vtable overhead.

::: python Coming from Python
Dynamic dispatch in Rust is what Python does by default. When you call `tool.execute(input)` in Python, the interpreter looks up `execute` in the object's method resolution order at runtime. Rust makes you opt into this with `dyn Trait` -- by default, Rust uses static dispatch (monomorphization), which is faster but requires knowing the concrete type at compile time. Since your registry holds tools of different types, dynamic dispatch is the right choice.
:::

## A Concrete Example: The EchoTool

To make this tangible, here is a minimal tool that simply echoes its input back. This is not useful in production, but it lets you verify that the trait, registry, and dispatch pipeline work before you build real tools.

```rust
use serde_json::{json, Value};

/// A simple tool that echoes its input back. Useful for testing the tool pipeline.
struct EchoTool;

impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Echoes the input message back. Useful for testing the tool system."
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
        let message = input
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidInput("Missing required field: message".to_string())
            })?;

        Ok(format!("Echo: {}", message))
    }
}
```

Notice how `execute` manually extracts the `message` field from the JSON value. This pattern -- `input.get("field").and_then(|v| v.as_str())` -- shows up constantly in tool implementations. The `ok_or_else` call converts a `None` (missing field) into a `ToolError::InvalidInput`, which will eventually flow back to the model as an error observation so it can retry.

## Putting It Together

Here is the complete module that you will build on throughout the chapter. Place this in `src/tools.rs` for now; you will split it into submodules as the system grows.

```rust
use serde_json::{json, Value};
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

impl std::error::Error for ToolError {}

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    fn execute(&self, input: &Value) -> Result<String, ToolError>;
}

pub struct EchoTool;

impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Echoes the input message back. Useful for testing the tool system."
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
        let message = input
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidInput("Missing required field: message".to_string())
            })?;
        Ok(format!("Echo: {}", message))
    }
}

fn main() {
    // Quick sanity check
    let tool = EchoTool;
    let input = json!({"message": "hello, tool system!"});

    println!("Tool name: {}", tool.name());
    println!("Tool description: {}", tool.description());
    println!("Schema: {}", serde_json::to_string_pretty(&tool.input_schema()).unwrap());

    match tool.execute(&input) {
        Ok(result) => println!("Result: {}", result),
        Err(e) => println!("Error: {}", e),
    }
}
```

Running this prints:

```
Tool name: echo
Tool description: Echoes the input message back. Useful for testing the tool system.
Schema: {
  "properties": {
    "message": {
      "description": "The message to echo back.",
      "type": "string"
    }
  },
  "required": ["message"],
  "type": "object"
}
Result: Echo: hello, tool system!
```

You now have a working trait that defines the contract, a concrete implementation that satisfies it, and an error type that categorizes failures. In the next subchapter, you will learn the JSON Schema format so you can write schemas that accurately describe any tool's inputs.

## Key Takeaways

- The `Tool` trait defines four methods: `name`, `description`, `input_schema`, and `execute`. Every tool struct must implement all four.
- `ToolError` has three variants -- `InvalidInput`, `ExecutionFailed`, and `SystemError` -- so the dispatch layer can handle each failure mode differently.
- The `Send + Sync` bounds on the trait are required because tools are stored in a shared registry and called from an async runtime.
- `Box<dyn Tool>` enables dynamic dispatch, letting the registry hold tools of different concrete types in a single `HashMap`.
- The `EchoTool` serves as a minimal reference implementation you can use to test the pipeline before building real tools.
