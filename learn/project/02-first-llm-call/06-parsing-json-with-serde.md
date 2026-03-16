---
title: Parsing JSON with Serde
description: Deserialize JSON API responses into strongly-typed Rust structs using serde and serde_json.
---

# Parsing JSON with Serde

> **What you'll learn:**
> - How serde's `Deserialize` derive macro maps JSON fields to Rust struct fields automatically
> - How to handle optional fields, renamed fields, and nested objects with serde attributes
> - How to use enums with `#[serde(tag = "type")]` to model the different content block types in API responses

In the previous subchapter, you got your first response from Claude. You used `response.json::<ChatResponse>()` to parse the JSON into a Rust struct, but we glossed over *how* that works. Serde is one of the most important crates in the Rust ecosystem, and understanding it well will save you hours of debugging as your agent evolves. Let's dig in.

## What Serde Does

Serde (short for **ser**ialize/**de**serialize) is a framework for converting Rust data structures to and from various formats. The `serde` crate itself is format-agnostic -- it defines traits (`Serialize` and `Deserialize`) that describe how a type can be converted. The `serde_json` crate provides the JSON-specific implementation.

When you write:

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}
```

The `#[derive(Deserialize)]` macro generates an implementation of the `Deserialize` trait for `Usage`. This generated code knows how to take a JSON object like `{"input_tokens": 15, "output_tokens": 10}` and produce a `Usage { input_tokens: 15, output_tokens: 10 }` value. You did not write any parsing code -- the derive macro inspected your struct's field names and types and wrote it for you.

::: python Coming from Python
In Python, you would typically use a dictionary or a dataclass with manual conversion:
```python
import json

data = json.loads(response_text)
input_tokens = data["input_tokens"]   # Runtime KeyError if missing
output_tokens = data["output_tokens"] # No type checking at all

# Or with Pydantic:
from pydantic import BaseModel

class Usage(BaseModel):
    input_tokens: int
    output_tokens: int

usage = Usage.model_validate_json(response_text)
```
Serde is conceptually similar to Pydantic: you define a typed model, and the framework handles parsing and validation. The key difference is that serde runs at compile time (the derive macro generates the parsing code), so there is zero reflection overhead at runtime. If your struct does not match the JSON shape, you get a clear runtime error during deserialization -- not a silent `None` or a missing key exception.
:::

## Basic Deserialization

Here is the simplest possible example -- parsing a JSON string into a struct:

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

fn main() {
    let json_str = r#"{"input_tokens": 15, "output_tokens": 10}"#;

    let usage: Usage = serde_json::from_str(json_str).unwrap();
    println!("{:?}", usage);
    // Output: Usage { input_tokens: 15, output_tokens: 10 }
}
```

`serde_json::from_str` takes a JSON string and returns a `Result<T, serde_json::Error>`. If the JSON does not match the struct's shape -- missing fields, wrong types, malformed JSON -- you get a descriptive error.

What happens if the JSON has extra fields that your struct does not define? **By default, serde ignores them.** This is important because the Anthropic API response includes many fields you might not care about yet. You can define a struct with just the fields you need, and serde will happily skip the rest.

```rust
#[derive(Debug, Deserialize)]
struct MinimalResponse {
    id: String,
    // We only want the id -- everything else is silently ignored
}

fn main() {
    let json_str = r#"{
        "id": "msg_01XFDUDYJgAACzvnptvVoYEL",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "text", "text": "Hello!"}],
        "model": "claude-sonnet-4-20250514",
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 10, "output_tokens": 5}
    }"#;

    let response: MinimalResponse = serde_json::from_str(json_str).unwrap();
    println!("Message ID: {}", response.id);
}
```

## Optional Fields

Not every field is guaranteed to be present (or non-null) in every response. Wrap those fields in `Option<T>`:

```rust
#[derive(Debug, Deserialize)]
struct ChatResponse {
    id: String,
    content: Vec<ContentBlock>,
    stop_reason: Option<String>,    // Can be null in streaming responses
    stop_sequence: Option<String>,  // Only present if a stop sequence was hit
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,           // Not present on tool_use blocks
}
```

When a JSON field is `null` or absent, `Option<T>` becomes `None`. When it is present and valid, it becomes `Some(value)`. This maps cleanly to Rust's explicit handling of nullable values.

If you want to make a field truly optional (allowed to be absent from the JSON entirely, not just null), you need:

```rust
#[derive(Debug, Deserialize)]
struct ChatResponse {
    id: String,
    #[serde(default)]
    stop_sequence: Option<String>,
}
```

The `#[serde(default)]` attribute tells serde to use the type's `Default` value (which is `None` for `Option`) when the field is missing from the JSON.

## Renaming Fields

Rust's naming convention uses `snake_case`, but JSON APIs sometimes use `camelCase` or have field names that collide with Rust keywords. The `#[serde(rename = "...")]` attribute handles both:

```rust
#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]   // "type" is a Rust keyword
    block_type: String,
    text: Option<String>,
}
```

If an API uses camelCase extensively, you can apply a renaming rule to the entire struct instead of individual fields:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiResponse {
    request_id: String,      // Matches "requestId" in JSON
    created_at: String,      // Matches "createdAt" in JSON
}
```

The Anthropic API uses `snake_case` for most fields, so you rarely need this. But `type` -> `block_type` is a rename you will use frequently.

## Nested Structs

JSON objects nest naturally, and so do serde structs. The Anthropic response is a good example:

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ChatResponse {
    id: String,
    #[serde(rename = "type")]
    response_type: String,
    role: String,
    model: String,
    content: Vec<ContentBlock>,
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

fn main() {
    let json_str = r#"{
        "id": "msg_01XFDUDYJgAACzvnptvVoYEL",
        "type": "message",
        "role": "assistant",
        "model": "claude-sonnet-4-20250514",
        "content": [
            {"type": "text", "text": "Hello! How can I help you today?"}
        ],
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": {"input_tokens": 12, "output_tokens": 10}
    }"#;

    let response: ChatResponse = serde_json::from_str(json_str).unwrap();

    println!("Model: {}", response.model);
    println!("Stop reason: {:?}", response.stop_reason);
    println!("Tokens: {} in, {} out",
        response.usage.input_tokens,
        response.usage.output_tokens
    );

    for block in &response.content {
        if let Some(text) = &block.text {
            println!("Response: {text}");
        }
    }
}
```

Each nested JSON object maps to a nested Rust struct. Serde handles the recursion automatically.

## Tagged Enums for Content Blocks

The Anthropic API returns content blocks that can be different types -- `text` blocks, `tool_use` blocks, and more. The `type` field determines which kind of block it is. Serde's tagged enum feature models this perfectly:

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

fn main() {
    let text_json = r#"{"type": "text", "text": "Hello!"}"#;
    let tool_json = r#"{
        "type": "tool_use",
        "id": "toolu_01A09q90qw90lq917835lq9",
        "name": "read_file",
        "input": {"path": "src/main.rs"}
    }"#;

    let text_block: ContentBlock = serde_json::from_str(text_json).unwrap();
    let tool_block: ContentBlock = serde_json::from_str(tool_json).unwrap();

    println!("{:?}", text_block);
    // Output: Text { text: "Hello!" }

    println!("{:?}", tool_block);
    // Output: ToolUse { id: "toolu_01A09q90qw90lq917835lq9", name: "read_file", input: Object {...} }

    // Pattern matching makes it easy to handle each variant
    match &text_block {
        ContentBlock::Text { text } => println!("Got text: {text}"),
        ContentBlock::ToolUse { name, .. } => println!("Got tool call: {name}"),
    }
}
```

The `#[serde(tag = "type")]` attribute tells serde to look at the `"type"` field in the JSON to decide which enum variant to deserialize into. The `#[serde(rename = "text")]` on each variant maps the JSON value to the Rust variant name.

This is a powerful pattern you will use throughout the agent. When the API adds new content block types, you add a new variant to the enum. Rust's exhaustive `match` ensures you handle every variant -- the compiler tells you if you forget one.

::: details What is serde_json::Value?
For the `tool_use` input field, we used `serde_json::Value` instead of a specific struct. `Value` is a dynamic JSON type -- it can hold any valid JSON: objects, arrays, strings, numbers, booleans, or null. It is useful when you do not know the exact shape of the data ahead of time (tool inputs vary by tool). You can access fields on a `Value` with indexing: `input["path"].as_str()`.
:::

## Serialization: Rust Structs to JSON

You also need to go the other direction -- converting Rust structs to JSON for request bodies. The `Serialize` derive macro handles this:

```rust
use serde::Serialize;

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

fn main() {
    let request = ChatRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        max_tokens: 1024,
        messages: vec![Message {
            role: "user".to_string(),
            content: "Hello!".to_string(),
        }],
        system: None,
    };

    let json = serde_json::to_string_pretty(&request).unwrap();
    println!("{json}");
}
```

This outputs:

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 1024,
  "messages": [
    {
      "role": "user",
      "content": "Hello!"
    }
  ]
}
```

Notice that the `system` field is absent from the output because of `#[serde(skip_serializing_if = "Option::is_none")]`. This is important -- if you sent `"system": null`, the API might interpret that differently than omitting the field entirely.

## Key Takeaways

- Serde's `#[derive(Deserialize)]` generates JSON parsing code at compile time based on your struct's field names and types, with zero runtime reflection overhead.
- Use `Option<T>` for fields that might be null or absent, `#[serde(rename = "...")]` for keyword collisions and naming convention mismatches, and `#[serde(default)]` for truly optional fields.
- Tagged enums with `#[serde(tag = "type")]` model the Anthropic content block types cleanly, and Rust's exhaustive `match` ensures you handle every variant.
- Extra JSON fields not defined in your struct are silently ignored, letting you start with a minimal struct and add fields as you need them.
- For serialization, `#[serde(skip_serializing_if = "Option::is_none")]` omits `None` fields from the output, which matters for APIs that distinguish between null and absent.
