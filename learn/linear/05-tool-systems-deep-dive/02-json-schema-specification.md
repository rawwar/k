---
title: JSON Schema Specification
description: Using JSON Schema to define tool input parameters, types, constraints, and documentation that LLMs can reliably interpret.
---

# JSON Schema Specification

> **What you'll learn:**
> - The subset of JSON Schema used by LLM APIs to define tool parameters and their types
> - How to specify required fields, enums, nested objects, and arrays in tool definitions
> - Common schema patterns for coding agent tools like file paths, line numbers, and code content

When you define a tool for a language model, you need a way to describe exactly what inputs the tool expects. What parameters does it take? What types are they? Which ones are required? Are there constraints on the values? JSON Schema is the standard answer to all of these questions. It is the language that LLM APIs (Claude, GPT, and others) use to describe tool parameters.

You do not need to be a JSON Schema expert to build a coding agent, but you do need to understand the subset that LLM APIs support. This subchapter covers that subset in detail with practical examples drawn from the tools you will build in later chapters.

## JSON Schema Basics

JSON Schema is a vocabulary for annotating and validating JSON documents. When you define a tool, the parameter schema describes the JSON object that the model should produce when calling that tool. Here is the simplest possible tool definition, as you would send it to Claude's API:

```json
{
  "name": "read_file",
  "description": "Read the contents of a file at the given path.",
  "input_schema": {
    "type": "object",
    "properties": {
      "path": {
        "type": "string",
        "description": "Absolute path to the file to read."
      }
    },
    "required": ["path"]
  }
}
```

The `input_schema` field contains a JSON Schema object. Let's break down the key elements.

### The `type` Field

Every schema node has a `type` that declares what kind of JSON value is expected. The types you will use most often are:

- `"string"` -- text values like file paths, code content, search patterns
- `"integer"` -- whole numbers like line numbers, byte offsets, limits
- `"number"` -- floating-point numbers (rarely needed in coding agent tools)
- `"boolean"` -- true/false flags like `create_if_missing` or `recursive`
- `"object"` -- nested JSON objects for grouped parameters
- `"array"` -- lists of items, such as multiple file paths or search results

The top-level schema for a tool's input is always `"type": "object"`, because the model produces a JSON object containing the named parameters.

### The `properties` Field

Inside an object schema, `properties` maps parameter names to their individual schemas:

```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Absolute path to the file."
    },
    "line_number": {
      "type": "integer",
      "description": "1-based line number to start reading from."
    },
    "count": {
      "type": "integer",
      "description": "Number of lines to read. Defaults to the entire file."
    }
  },
  "required": ["path"]
}
```

Notice that `line_number` and `count` are not in the `required` array. This means the model can omit them, and your tool implementation should provide sensible defaults.

### The `required` Field

The `required` array lists which properties must be present in the input. If the model omits a required field, the validation layer catches it before execution. Be thoughtful about what you make required versus optional -- every required field is something the model must get right on every invocation.

::: python Coming from Python
If you have used Pydantic in Python, JSON Schema will feel familiar. Pydantic models *generate* JSON Schema under the hood. A Pydantic model like this:
```python
from pydantic import BaseModel

class ReadFileInput(BaseModel):
    path: str
    line_number: int | None = None
    count: int | None = None
```
produces a JSON Schema nearly identical to the one above. In Rust, the `schemars` crate fills the same role -- you derive `JsonSchema` on a struct and it generates the schema for you.
:::

## String Constraints

Strings are the most common parameter type in coding agent tools. JSON Schema provides several ways to constrain them.

### Enums

When a parameter should only accept specific values, use `enum`:

```json
{
  "name": "search_files",
  "description": "Search for a pattern across files in the project.",
  "input_schema": {
    "type": "object",
    "properties": {
      "pattern": {
        "type": "string",
        "description": "The regex pattern to search for."
      },
      "file_type": {
        "type": "string",
        "enum": ["rust", "python", "javascript", "typescript", "all"],
        "description": "Restrict search to files of this type. Defaults to 'all'."
      }
    },
    "required": ["pattern"]
  }
}
```

Enums are powerful because they tell the model exactly what values are valid. Without the enum, the model might pass `"rs"`, `"Rust"`, `".rs"`, or `"rust"` -- all reasonable guesses but potentially broken. With the enum, it knows the exact set of valid options.

### String Length Constraints

You can limit string length with `minLength` and `maxLength`:

```json
{
  "pattern": {
    "type": "string",
    "description": "The search pattern.",
    "minLength": 1,
    "maxLength": 500
  }
}
```

These constraints are especially useful for preventing accidental empty strings or absurdly long inputs.

## Numeric Constraints

For integers and numbers, you can set bounds with `minimum`, `maximum`, `exclusiveMinimum`, and `exclusiveMaximum`:

```json
{
  "line_number": {
    "type": "integer",
    "description": "1-based line number.",
    "minimum": 1
  },
  "count": {
    "type": "integer",
    "description": "Number of lines to read.",
    "minimum": 1,
    "maximum": 10000
  }
}
```

Setting `minimum: 1` on a line number prevents the model from passing 0 or negative values. Setting a `maximum` on count prevents the model from asking to read millions of lines.

## Arrays

Arrays let you accept lists of values. Use the `items` field to define the schema for each element:

```json
{
  "name": "batch_read",
  "description": "Read multiple files at once.",
  "input_schema": {
    "type": "object",
    "properties": {
      "paths": {
        "type": "array",
        "items": {
          "type": "string",
          "description": "Absolute path to a file."
        },
        "description": "List of file paths to read.",
        "minItems": 1,
        "maxItems": 20
      }
    },
    "required": ["paths"]
  }
}
```

Use `minItems` and `maxItems` to constrain the list length. Without `maxItems`, the model might pass hundreds of paths and overwhelm your tool.

## Nested Objects

Sometimes a tool needs structured input that goes beyond flat parameters. You can nest objects:

```json
{
  "name": "edit_file",
  "description": "Apply an edit to a file by replacing old content with new content.",
  "input_schema": {
    "type": "object",
    "properties": {
      "path": {
        "type": "string",
        "description": "Absolute path to the file to edit."
      },
      "old_string": {
        "type": "string",
        "description": "The exact string to find and replace. Must match exactly."
      },
      "new_string": {
        "type": "string",
        "description": "The replacement string."
      }
    },
    "required": ["path", "old_string", "new_string"]
  }
}
```

This flat structure works well for a simple find-and-replace tool. But if you needed to apply multiple edits at once, you might nest an array of edit objects:

```json
{
  "name": "multi_edit",
  "description": "Apply multiple edits to a single file.",
  "input_schema": {
    "type": "object",
    "properties": {
      "path": {
        "type": "string",
        "description": "Absolute path to the file."
      },
      "edits": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "old_string": {
              "type": "string",
              "description": "The exact text to find."
            },
            "new_string": {
              "type": "string",
              "description": "The replacement text."
            }
          },
          "required": ["old_string", "new_string"]
        },
        "description": "The list of edits to apply in order.",
        "minItems": 1
      }
    },
    "required": ["path", "edits"]
  }
}
```

A word of caution: deeply nested schemas are harder for models to fill correctly. Keep nesting to one or two levels when possible.

## Generating Schemas in Rust

In your Rust agent, you will not write JSON Schema by hand. Instead, you will use the `schemars` crate to derive schemas from Rust structs:

```rust
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadFileInput {
    /// Absolute path to the file to read.
    pub path: String,

    /// 1-based line number to start reading from.
    /// If omitted, reads from the beginning of the file.
    pub line_number: Option<u32>,

    /// Number of lines to read.
    /// If omitted, reads the entire file.
    pub count: Option<u32>,
}
```

The `schemars` crate reads the doc comments (`///`) and turns them into `description` fields in the generated schema. The `Option<u32>` fields become non-required properties. This is a pattern you will use repeatedly -- define your input as a Rust struct, derive `JsonSchema`, and let the crate handle the translation.

To generate the schema at runtime:

```rust
use schemars::schema_for;

fn main() {
    let schema = schema_for!(ReadFileInput);
    let json = serde_json::to_string_pretty(&schema).unwrap();
    println!("{}", json);
}
```

::: wild In the Wild
OpenCode generates tool schemas from Go structs using struct tags, much like Rust's derive approach. Each tool defines an `Input` struct with JSON tags and description tags, and the framework generates the JSON Schema from the struct definition at startup. Claude Code, being a TypeScript application, uses Zod schemas that are converted to JSON Schema. Both approaches share the same goal: define the schema once in the native language's type system and generate JSON Schema automatically.
:::

## Common Patterns for Coding Agent Tools

Here are schema patterns you will encounter repeatedly when building coding agent tools:

**File path parameter:** Always use `"type": "string"` with a description specifying whether relative or absolute paths are expected. Enforce this in validation, not the schema itself.

**Optional with default:** Use `Option<T>` in Rust (which becomes a non-required property). Document the default value in the description: `"Number of context lines to show. Defaults to 3."`

**Mutually exclusive parameters:** JSON Schema supports `oneOf` for this, but in practice it confuses models. Instead, use a single enum parameter to select the mode, and validate that the correct fields are present for that mode.

**Boolean flags:** Use sparingly. Each boolean flag doubles the behavior space of your tool, which makes descriptions harder to write and models more likely to make mistakes. If you find yourself adding many boolean flags, consider splitting the tool.

## Key Takeaways

- JSON Schema is the standard for describing tool parameters to LLMs -- the `input_schema` field in your tool definition
- Use `type`, `properties`, `required`, `enum`, `minimum`/`maximum`, and `items` to precisely constrain what the model can pass to your tools
- In Rust, derive `JsonSchema` from `schemars` on your input structs to generate schemas automatically from your type definitions
- Keep schemas as flat as possible -- deeply nested objects are harder for models to fill correctly
- Document defaults in descriptions, use enums to constrain string values, and set numeric bounds to prevent edge-case inputs
