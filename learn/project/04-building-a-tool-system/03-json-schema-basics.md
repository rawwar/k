---
title: JSON Schema Basics
description: Understand JSON Schema fundamentals needed to describe tool input parameters to the LLM.
---

# JSON Schema Basics

> **What you'll learn:**
> - How JSON Schema describes the shape, types, and constraints of a JSON object
> - The key schema keywords you need for tool definitions: type, properties, required, description, and enum
> - How the Anthropic API uses the input_schema field to tell the model what arguments a tool accepts

Before you can define tool schemas in Rust code, you need to understand the format itself. JSON Schema is a specification for describing the structure of JSON data. It is what the Anthropic API uses to tell the model what arguments each tool accepts. If you get the schema wrong, the model will send malformed inputs. If you get it right, the model will reliably produce exactly the JSON your tool expects.

## What JSON Schema Does

JSON Schema answers one question: "Is this JSON value valid?" You write a schema that describes what valid data looks like -- its types, required fields, allowed values, constraints. Then you (or a validation library) check whether a given JSON value conforms to that schema.

For tool definitions, the schema serves a dual purpose. First, it tells the model what arguments to produce: "this tool takes a `path` string and an optional `line_number` integer." Second, your agent code can validate the model's output against the schema before executing the tool, catching errors early.

Here is a minimal example. This schema describes an object with one required string field:

```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "The file path to read."
    }
  },
  "required": ["path"]
}
```

A JSON value like `{"path": "src/main.rs"}` is valid. A value like `{"path": 42}` is invalid (wrong type). A value like `{}` is invalid (missing required field).

## The Keywords You Need

JSON Schema has dozens of keywords, but tool definitions only use a handful. Here are the ones you will use throughout this book.

### `type`

Specifies the expected JSON type. The values relevant to tool schemas are:

| Type | JSON Example | Rust Equivalent |
|------|-------------|-----------------|
| `"string"` | `"hello"` | `String` or `&str` |
| `"number"` | `3.14` | `f64` |
| `"integer"` | `42` | `i64` |
| `"boolean"` | `true` | `bool` |
| `"object"` | `{"key": "val"}` | `serde_json::Map` |
| `"array"` | `[1, 2, 3]` | `Vec<T>` |
| `"null"` | `null` | `Option::None` |

Every tool's top-level schema should have `"type": "object"` because the Anthropic API sends tool inputs as JSON objects.

### `properties`

Defines the fields an object can contain. Each property has its own sub-schema:

```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "The file path to read."
    },
    "offset": {
      "type": "integer",
      "description": "Line number to start reading from (1-based)."
    },
    "limit": {
      "type": "integer",
      "description": "Maximum number of lines to return."
    }
  }
}
```

The model reads the `description` for each property to understand what value to pass. Good property descriptions are concise but specific -- "The file path to read" beats "path" and "The absolute or relative path to the file you want to read, as a string" is unnecessarily verbose.

### `required`

An array of field names that must be present. Fields not listed in `required` are optional:

```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string", "description": "File path." },
    "offset": { "type": "integer", "description": "Start line." }
  },
  "required": ["path"]
}
```

Here, `path` must be provided. `offset` is optional -- if the model does not include it, your tool should use a sensible default (like starting from line 1).

### `description`

A human-readable (and LLM-readable) explanation. You can use `description` at the property level and at the top level of the schema:

```json
{
  "type": "object",
  "description": "Input for the read_file tool.",
  "properties": {
    "path": {
      "type": "string",
      "description": "Absolute or relative path to the file."
    }
  },
  "required": ["path"]
}
```

While the top-level `description` on the schema is less important (the tool's own `description` field handles that), property-level descriptions are critical. They are the model's primary guide for constructing correct arguments.

### `enum`

Restricts a string (or other type) to a fixed set of values:

```json
{
  "type": "string",
  "enum": ["read", "write", "append"],
  "description": "The file operation mode."
}
```

This tells the model it must choose one of the three listed values. Enums are useful for tools that have a mode parameter, like a file tool that can read or write, or a search tool that can search by name or by content.

### `items`

Describes the elements of an array:

```json
{
  "type": "object",
  "properties": {
    "paths": {
      "type": "array",
      "items": { "type": "string" },
      "description": "List of file paths to process."
    }
  }
}
```

You will use this less often, but it matters for tools that accept multiple inputs, like a batch file reader.

## How the Anthropic API Uses Schemas

When you send a request to the Anthropic Messages API, the `tools` array contains objects with three fields:

```json
{
  "tools": [
    {
      "name": "read_file",
      "description": "Read the contents of a file at the given path.",
      "input_schema": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "The file path to read."
          }
        },
        "required": ["path"]
      }
    }
  ]
}
```

The `input_schema` is the JSON Schema you just learned about. The model uses `name` and `description` to decide *whether* to call the tool, and `input_schema` to decide *what arguments* to pass.

When the model decides to call this tool, it produces a `tool_use` content block:

```json
{
  "type": "tool_use",
  "id": "toolu_01A2B3C4D5",
  "name": "read_file",
  "input": {
    "path": "src/main.rs"
  }
}
```

Notice that `input` conforms to the `input_schema` -- it is an object with a `path` string. The model learned this shape from the schema you provided. This is why accurate schemas matter: an incorrect schema leads to incorrect inputs.

::: python Coming from Python
In Python, you might use Pydantic to define tool input models and then call `.model_json_schema()` to get the JSON Schema automatically:

```python
from pydantic import BaseModel, Field

class ReadFileInput(BaseModel):
    path: str = Field(description="The file path to read.")

schema = ReadFileInput.model_json_schema()
```

Rust has a similar option with the `schemars` crate, which derives JSON Schema from Rust types. You will see both approaches -- manual construction with `serde_json::json!` and automatic derivation with `schemars` -- in the next subchapter.
:::

## Schemas in Rust with serde_json

You do not need a separate JSON Schema library to *construct* schemas. The `serde_json::json!` macro lets you write JSON literals directly in Rust code:

```rust
use serde_json::{json, Value};

fn read_file_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "The file path to read."
            },
            "offset": {
                "type": "integer",
                "description": "Line number to start reading from (1-based)."
            },
            "limit": {
                "type": "integer",
                "description": "Maximum number of lines to return."
            }
        },
        "required": ["path"]
    })
}

fn main() {
    let schema = read_file_schema();
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
```

The `json!` macro returns a `serde_json::Value`, which is exactly the type your `Tool::input_schema()` method returns. This approach is straightforward: what you see in the macro is what gets sent to the API. The downside is that the schema is not validated at compile time -- a typo in a keyword like `"properteis"` will not cause a compiler error. You catch those during testing.

## A Complete Schema Example

Here is a more realistic schema for a shell execution tool. It demonstrates several keywords working together:

```json
{
  "type": "object",
  "properties": {
    "command": {
      "type": "string",
      "description": "The shell command to execute."
    },
    "working_directory": {
      "type": "string",
      "description": "Directory to run the command in. Defaults to the project root."
    },
    "timeout_seconds": {
      "type": "integer",
      "description": "Maximum time in seconds before the command is killed. Defaults to 30."
    }
  },
  "required": ["command"]
}
```

This schema tells the model: you *must* provide a `command` string; you *may* provide a `working_directory` and a `timeout_seconds`. The descriptions guide the model's choices -- it knows the default timeout is 30 seconds, so it will only override it for commands expected to run longer.

::: wild In the Wild
Claude Code's tool schemas are notably detailed. The `Bash` tool's schema includes descriptions like "The command to execute in the shell. Must be a valid bash command." and constraints on maximum output length. These detailed schemas reduce tool-call errors significantly -- the model rarely sends malformed input when the schema clearly describes what is expected.
:::

## Key Takeaways

- JSON Schema describes the structure of JSON data using keywords like `type`, `properties`, `required`, `description`, and `enum`.
- Every tool schema has `"type": "object"` at the top level because tool inputs are always JSON objects.
- The `required` array distinguishes mandatory fields from optional ones. Optional fields should have sensible defaults in your tool implementation.
- Property-level `description` values are critical -- they guide the model in constructing correct arguments, functioning as mini-prompts.
- The `serde_json::json!` macro lets you write schemas as JSON literals in Rust code, returning the `serde_json::Value` type your trait expects.
