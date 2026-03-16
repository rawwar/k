---
title: Partial JSON Handling
description: Strategies for working with incomplete JSON payloads that arrive mid-stream, including incremental parsing, speculative completion, and structured extraction.
---

# Partial JSON Handling

> **What you'll learn:**
> - Why LLM streaming produces partial JSON fragments and the challenges of parsing incomplete structured data
> - Techniques for incremental JSON parsing that extract usable fields before the complete object arrives
> - Building a partial JSON accumulator that buffers and retries parsing as new chunks arrive

When you stream text content from an LLM, each SSE event carries a small, self-contained JSON payload like `{"type":"text_delta","text":"Hello"}`. Parsing is straightforward -- each event's data field is a complete JSON object. But when the LLM streams a **tool call**, the situation changes dramatically. Tool call arguments can be large JSON objects -- sometimes hundreds or thousands of characters -- and they arrive spread across many `content_block_delta` events. Your agent receives fragments like `{"file_pa`, then `th": "/src/m`, then `ain.rs",`, then `"content"`, and so on. At no point during streaming is the tool call arguments string valid JSON.

This is the partial JSON problem, and solving it correctly is essential for a responsive agent that can show tool call progress as it streams.

## Why Tool Calls Produce Partial JSON

When an LLM generates a tool call, the response structure looks like this (using Anthropic's format):

```json
{
  "type": "tool_use",
  "id": "toolu_01abc",
  "name": "write_file",
  "input": {
    "file_path": "/src/main.rs",
    "content": "fn main() {\n    println!(\"Hello, world!\");\n}"
  }
}
```

The `input` field is a JSON object containing the tool's arguments. During streaming, this object is generated token by token, and each token is delivered as a delta event. The deltas arrive something like this:

```
event: content_block_delta
data: {"type":"input_json_delta","partial_json":"{\"file_pa"}

event: content_block_delta
data: {"type":"input_json_delta","partial_json":"th\": \"/src/m"}

event: content_block_delta
data: {"type":"input_json_delta","partial_json":"ain.rs\", \"conten"}

event: content_block_delta
data: {"type":"input_json_delta","partial_json":"t\": \"fn main()"}
```

Each `partial_json` field contains a few characters of the tool call arguments. You must concatenate all of them to get valid JSON. But what if you want to show the user what file is being written *before* the complete tool call arrives? You need to extract information from partial JSON.

## The Accumulate-and-Retry Strategy

The simplest approach is to accumulate the JSON fragments and attempt to parse after each new chunk. When parsing fails (because the JSON is incomplete), you simply wait for more data:

```rust
use serde_json::Value;

pub struct JsonAccumulator {
    buffer: String,
    last_successful_parse: Option<Value>,
}

impl JsonAccumulator {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            last_successful_parse: None,
        }
    }

    /// Append a JSON fragment and attempt to parse.
    /// Returns Some(value) if the accumulated buffer is now valid JSON.
    pub fn feed(&mut self, fragment: &str) -> Option<&Value> {
        self.buffer.push_str(fragment);

        match serde_json::from_str::<Value>(&self.buffer) {
            Ok(value) => {
                self.last_successful_parse = Some(value);
                self.last_successful_parse.as_ref()
            }
            Err(_) => None,
        }
    }

    /// Get the complete accumulated string, whether or not it's valid JSON.
    pub fn raw(&self) -> &str {
        &self.buffer
    }

    /// Reset the accumulator for a new tool call.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.last_successful_parse = None;
    }
}
```

This works, but it has a problem: the parse attempt fails on every single delta until the very last one. For a tool call with 50 deltas, you run `serde_json::from_str` 50 times, and 49 of those fail. Is this expensive? Not really -- `serde_json` fails fast on invalid JSON, typically within microseconds. But there is a smarter approach.

## Speculative Completion

Instead of waiting for the JSON to become complete, you can try to *complete* it speculatively. The idea: look at the partial JSON, figure out what brackets and quotes are unclosed, and append closing characters to make it parseable:

```rust
/// Attempt to complete partial JSON by closing unclosed brackets and quotes.
/// Returns the completed JSON string if successful.
pub fn complete_partial_json(partial: &str) -> Option<String> {
    let trimmed = partial.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut completed = trimmed.to_string();
    let mut stack: Vec<char> = Vec::new();
    let mut in_string = false;
    let mut escape_next = false;

    for ch in trimmed.chars() {
        if escape_next {
            escape_next = false;
            continue;
        }

        if ch == '\\' && in_string {
            escape_next = true;
            continue;
        }

        if ch == '"' {
            in_string = !in_string;
            if in_string {
                stack.push('"');
            } else {
                // Pop the matching quote
                if stack.last() == Some(&'"') {
                    stack.pop();
                }
            }
            continue;
        }

        if !in_string {
            match ch {
                '{' => stack.push('}'),
                '[' => stack.push(']'),
                '}' | ']' => {
                    stack.pop();
                }
                _ => {}
            }
        }
    }

    // If we're mid-string, close it
    if in_string {
        completed.push('"');
        // Pop the matching opening quote
        if stack.last() == Some(&'"') {
            stack.pop();
        }
    }

    // Close any unclosed brackets in reverse order
    while let Some(closer) = stack.pop() {
        completed.push(closer);
    }

    // Try to parse the completed version
    match serde_json::from_str::<serde_json::Value>(&completed) {
        Ok(_) => Some(completed),
        Err(_) => None,
    }
}
```

Now you can extract partial information from incomplete tool calls:

```rust
fn demonstrate_speculative_completion() {
    let partial = r#"{"file_path": "/src/main.rs", "conten"#;

    if let Some(completed) = complete_partial_json(partial) {
        let value: serde_json::Value = serde_json::from_str(&completed).unwrap();
        // value = {"file_path": "/src/main.rs", "conten": ""}
        // We can extract file_path even though content is incomplete!
        if let Some(path) = value.get("file_path").and_then(|v| v.as_str()) {
            println!("Writing to: {}", path);
        }
    }
}
```

The speculative completion gives you a "best effort" parse that lets you show the user which file is being written, which tool is being called, and other partial information as the stream progresses.

::: python Coming from Python
In Python, you might use `json.JSONDecodeError` to detect incomplete JSON:
```python
import json

buffer = ""
for chunk in stream:
    buffer += chunk
    try:
        result = json.loads(buffer)
        process_complete(result)
    except json.JSONDecodeError:
        pass  # Wait for more data
```
The Rust approach is structurally identical -- `serde_json::from_str` returns `Err` for incomplete JSON, just like Python's `json.loads` raises `JSONDecodeError`. The speculative completion technique works in both languages, but Rust's lack of garbage collection makes the repeated parsing attempts cheaper since there is no GC pressure from the temporary `Value` objects that fail to parse.
:::

## A Streaming Tool Call Processor

Let's build a more complete tool call processor that combines accumulation with speculative completion and emits progress updates:

```rust
use serde::Deserialize;

#[derive(Debug, Clone)]
pub enum ToolCallProgress {
    /// We know the tool name but arguments are still streaming.
    Started { tool_name: String, tool_id: String },
    /// We have partial arguments with some fields extractable.
    PartialArguments {
        tool_id: String,
        extracted: serde_json::Value,
    },
    /// The tool call is complete with valid JSON arguments.
    Complete {
        tool_id: String,
        tool_name: String,
        arguments: serde_json::Value,
    },
}

pub struct ToolCallAccumulator {
    tool_id: String,
    tool_name: String,
    json_buffer: String,
    last_extracted: Option<serde_json::Value>,
}

impl ToolCallAccumulator {
    pub fn new(tool_id: String, tool_name: String) -> Self {
        Self {
            tool_id,
            tool_name,
            json_buffer: String::new(),
            last_extracted: None,
        }
    }

    /// Feed a JSON fragment and return progress if there's something new to report.
    pub fn feed(&mut self, fragment: &str) -> Option<ToolCallProgress> {
        self.json_buffer.push_str(fragment);

        // Try full parse first
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&self.json_buffer) {
            return Some(ToolCallProgress::Complete {
                tool_id: self.tool_id.clone(),
                tool_name: self.tool_name.clone(),
                arguments: value,
            });
        }

        // Try speculative completion
        if let Some(completed) = complete_partial_json(&self.json_buffer) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&completed) {
                // Only emit progress if we extracted something new
                let is_new = self
                    .last_extracted
                    .as_ref()
                    .map(|prev| prev != &value)
                    .unwrap_or(true);

                if is_new {
                    self.last_extracted = Some(value.clone());
                    return Some(ToolCallProgress::PartialArguments {
                        tool_id: self.tool_id.clone(),
                        extracted: value,
                    });
                }
            }
        }

        None
    }
}
```

This accumulator emits `PartialArguments` events as new fields become extractable, and a final `Complete` event when the full JSON is valid. Your rendering layer can use these to show progressive tool call information:

```rust
async fn handle_tool_call_stream(
    events: &mut tokio::sync::mpsc::Receiver<SseEvent>,
) {
    let mut accumulator: Option<ToolCallAccumulator> = None;

    while let Some(event) = events.recv().await {
        match event.event_type() {
            "content_block_start" => {
                let value: serde_json::Value =
                    serde_json::from_str(&event.data).unwrap();
                if let Some(block) = value.get("content_block") {
                    if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                        let tool_id = block["id"].as_str().unwrap_or("").to_string();
                        let tool_name = block["name"].as_str().unwrap_or("").to_string();
                        println!("Tool call starting: {}", tool_name);
                        accumulator = Some(ToolCallAccumulator::new(tool_id, tool_name));
                    }
                }
            }
            "content_block_delta" => {
                let value: serde_json::Value =
                    serde_json::from_str(&event.data).unwrap();
                if let Some(partial) = value
                    .get("delta")
                    .and_then(|d| d.get("partial_json"))
                    .and_then(|p| p.as_str())
                {
                    if let Some(ref mut acc) = accumulator {
                        if let Some(progress) = acc.feed(partial) {
                            match progress {
                                ToolCallProgress::PartialArguments { extracted, .. } => {
                                    if let Some(path) =
                                        extracted.get("file_path").and_then(|v| v.as_str())
                                    {
                                        println!("  Writing to: {}", path);
                                    }
                                }
                                ToolCallProgress::Complete { arguments, .. } => {
                                    println!("  Tool call complete: {}", arguments);
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            "content_block_stop" => {
                accumulator = None;
            }
            _ => {}
        }
    }
}
```

## Handling Edge Cases

Partial JSON parsing has several tricky edge cases:

**Strings with escaped quotes.** A value like `"path": "C:\\Users\\test"` produces backslash-quote sequences that confuse naive bracket counters. The `complete_partial_json` function above handles this with its `escape_next` flag.

**Nested objects.** Tool arguments often contain nested structures: `{"options": {"recursive": true, "depth": 3}}`. The stack-based approach handles nesting correctly by tracking each opening bracket.

**Large string values.** When a tool call writes file content, the `content` field might be thousands of characters. The speculative completion approach still works -- it closes the string and any open brackets -- but the truncated string value is not meaningful. For display purposes, you might choose to show only fields that have been fully received:

```rust
/// Extract only fully-formed key-value pairs from partial JSON.
fn extract_complete_fields(partial: &str) -> Vec<(String, serde_json::Value)> {
    let completed = match complete_partial_json(partial) {
        Some(c) => c,
        None => return vec![],
    };

    let value: serde_json::Value = match serde_json::from_str(&completed) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let obj = match value.as_object() {
        Some(o) => o,
        None => return vec![],
    };

    // A field is "complete" if it appears fully in the original partial string.
    // We check this by serializing each field and seeing if it appears in the original.
    obj.iter()
        .filter(|(key, val)| {
            let serialized = format!("\"{}\":{}", key, serde_json::to_string(val).unwrap());
            let key_appears = partial.contains(&format!("\"{}\":", key));
            // Heuristic: if the key appears and is followed by a complete value terminator
            key_appears && (partial.contains(&serialized) || val.is_null())
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}
```

::: wild In the Wild
Claude Code accumulates tool call arguments in a buffer and attempts speculative completion to show progress. When a `write_file` tool call starts streaming, Claude Code extracts the `file_path` early (often from the first few deltas) and displays "Writing to /src/main.rs..." before the file content has finished generating. This gives the user immediate feedback about what the agent is doing, even though the complete tool call might take several seconds to stream.
:::

## Key Takeaways

- Tool call arguments arrive as **partial JSON fragments** across many SSE events. You must accumulate fragments and handle incomplete JSON explicitly.
- The **accumulate-and-retry** strategy is simple and effective: buffer fragments, attempt `serde_json::from_str` after each chunk, and handle errors as "not yet complete."
- **Speculative completion** -- closing unclosed quotes and brackets -- lets you extract partial field values before the JSON is complete, enabling progressive UI updates like showing the target file path of a write operation.
- Edge cases include **escaped characters in strings**, **nested objects**, and **large string values** where truncation makes the speculatively-completed value meaningless.
- For the best user experience, extract and display only **fully-formed fields** rather than showing truncated speculative values that might confuse the user.
