---
title: Property Based Testing
description: Use property-based testing to validate agent invariants across randomly generated inputs, catching edge cases that example-based tests miss.
---

# Property Based Testing

> **What you'll learn:**
> - How to identify testable properties of agent components (e.g., "parsing never panics", "tool output is always valid JSON", "permissions are never escalated")
> - Techniques for using proptest or quickcheck to generate random tool inputs, conversation histories, and configuration values
> - How to write shrinkable generators for agent-specific types so that failing test cases are minimized to the simplest reproducing example

Example-based tests verify specific scenarios: "when I pass this input, I get this output." They are great for documenting expected behavior, but they only cover the cases you think of. Property-based tests flip the approach: you define a property that should always hold (an invariant), generate thousands of random inputs, and verify the property holds for all of them.

For a coding agent, property-based testing catches the edge cases that slip through example-based tests: strange Unicode in file paths, empty strings where you expect content, deeply nested JSON in tool arguments, and integer overflows in token counting. These are exactly the kinds of inputs an LLM might produce.

## Getting Started with Proptest

The `proptest` crate is the go-to property-based testing library in Rust. Add it to your dev dependencies:

```toml
[dev-dependencies]
proptest = "1"
```

A proptest test looks like a regular test but uses the `proptest!` macro to generate random inputs:

```rust
use proptest::prelude::*;

fn count_tokens(text: &str) -> usize {
    // Simplified token counter: split on whitespace
    text.split_whitespace().count()
}

proptest! {
    #[test]
    fn token_count_is_never_negative(text in "\\PC*") {
        // Property: token count is always >= 0
        // (This is trivially true for usize, but the point is the pattern)
        let count = count_tokens(&text);
        prop_assert!(count <= text.len());
    }

    #[test]
    fn empty_string_has_zero_tokens(text in "\\s*") {
        // Property: whitespace-only strings have zero tokens
        let count = count_tokens(&text);
        prop_assert_eq!(count, 0);
    }
}
```

The `"\\PC*"` is a regex strategy that generates random strings of printable characters. Proptest generates hundreds of test cases, and if any fail, it shrinks the input to the minimal failing case.

::: tip Coming from Python
Python has `hypothesis`, which works similarly:
```python
from hypothesis import given
from hypothesis import strategies as st

@given(st.text())
def test_token_count_never_negative(text):
    count = count_tokens(text)
    assert count <= len(text)
```
Proptest and hypothesis share the same core idea: define strategies for generating random inputs, then assert properties. The key difference is that proptest integrates with Rust's type system — you can derive strategies for your own types, and the compiler ensures your generators produce valid typed data. Hypothesis uses Python's dynamic typing which is more flexible but offers fewer compile-time guarantees.
:::

## Properties Worth Testing in an Agent

Identifying the right properties is the hardest part. Here are the properties that matter most for a coding agent:

### 1. Parsing Never Panics

Your message parser processes LLM output, which can be anything. It should never panic, regardless of input:

```rust
use proptest::prelude::*;

#[derive(Debug)]
struct ToolCall {
    name: String,
    arguments: serde_json::Value,
}

fn parse_tool_call(raw: &str) -> Result<ToolCall, String> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let name = value["name"]
        .as_str()
        .ok_or_else(|| "missing name field".to_string())?
        .to_string();
    let arguments = value.get("arguments").cloned().unwrap_or(serde_json::Value::Null);
    Ok(ToolCall { name, arguments })
}

proptest! {
    #[test]
    fn parsing_never_panics(input in "\\PC{0,1000}") {
        // Property: parse_tool_call never panics, only returns Ok or Err
        let _ = parse_tool_call(&input);
    }
}
```

This test generates thousands of random strings and feeds them to your parser. If any input causes a panic (index out of bounds, unwrap on None, etc.), proptest catches it and shrinks it to the minimal failing input.

### 2. Tool Input Validation Rejects All Invalid Paths

```rust
fn validate_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("empty path".into());
    }
    if path.contains("..") {
        return Err("path traversal".into());
    }
    if path.starts_with('/') {
        return Err("absolute paths not allowed".into());
    }
    Ok(())
}

proptest! {
    #[test]
    fn path_traversal_always_rejected(
        prefix in "[a-z/]{0,10}",
        suffix in "[a-z/]{0,10}"
    ) {
        // Property: any path containing ".." is rejected
        let malicious = format!("{}../{}", prefix, suffix);
        prop_assert!(validate_path(&malicious).is_err());
    }

    #[test]
    fn valid_relative_paths_accepted(
        segments in prop::collection::vec("[a-z][a-z0-9_]{0,20}", 1..5)
    ) {
        // Property: paths made of valid segments are accepted
        let path = segments.join("/");
        prop_assert!(validate_path(&path).is_ok());
    }
}
```

### 3. Token Counting Is Consistent

```rust
fn estimate_tokens(text: &str) -> usize {
    // Rough estimate: 1 token per 4 characters
    (text.len() + 3) / 4
}

fn fits_in_context(messages: &[String], max_tokens: usize) -> bool {
    let total: usize = messages.iter().map(|m| estimate_tokens(m)).sum();
    total <= max_tokens
}

proptest! {
    #[test]
    fn adding_messages_never_decreases_token_count(
        messages in prop::collection::vec("[a-zA-Z ]{1,100}", 1..10),
        extra in "[a-zA-Z ]{1,100}"
    ) {
        let count_before: usize = messages.iter().map(|m| estimate_tokens(m)).sum();
        let mut with_extra = messages.clone();
        with_extra.push(extra);
        let count_after: usize = with_extra.iter().map(|m| estimate_tokens(m)).sum();

        prop_assert!(count_after >= count_before);
    }
}
```

## Custom Strategies for Agent Types

Proptest lets you build custom strategies that generate realistic agent-specific data:

```rust
use proptest::prelude::*;
use serde_json::json;

fn tool_name_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("read_file".to_string()),
        Just("write_file".to_string()),
        Just("shell".to_string()),
        Just("list_files".to_string()),
    ]
}

fn tool_input_strategy() -> impl Strategy<Value = serde_json::Value> {
    prop_oneof![
        "[a-z/]{1,50}".prop_map(|path| json!({"path": path})),
        ("[a-z/]{1,50}", "[a-zA-Z0-9 \\n]{0,200}")
            .prop_map(|(path, content)| json!({"path": path, "content": content})),
        "[a-z ]{1,100}".prop_map(|cmd| json!({"command": cmd})),
    ]
}

fn conversation_message_strategy() -> impl Strategy<Value = (String, String)> {
    prop_oneof![
        "[a-zA-Z ?!.]{1,200}".prop_map(|text| ("user".to_string(), text)),
        "[a-zA-Z .]{1,500}".prop_map(|text| ("assistant".to_string(), text)),
    ]
}

proptest! {
    #[test]
    fn tool_dispatch_handles_all_tool_names(
        name in tool_name_strategy(),
        input in tool_input_strategy()
    ) {
        // Property: dispatch never panics for known tool names
        let result = dispatch_tool(&name, &input);
        // result is either Ok or Err, but never a panic
        let _ = result;
    }
}

fn dispatch_tool(name: &str, input: &serde_json::Value) -> Result<String, String> {
    match name {
        "read_file" | "write_file" | "shell" | "list_files" => {
            Ok(format!("Executed {}", name))
        }
        _ => Err(format!("Unknown tool: {}", name)),
    }
}
```

## Shrinking: Finding the Minimal Failing Case

When proptest finds a failing input, it automatically shrinks it — tries progressively simpler inputs that still trigger the failure. This is invaluable for debugging. Instead of seeing a 200-character random string that causes a panic, you see the 3-character string that triggers the same bug.

```rust
fn truncate_to_limit(text: &str, limit: usize) -> &str {
    // Bug: doesn't handle multi-byte UTF-8 characters
    &text[..limit.min(text.len())]
}

proptest! {
    #[test]
    fn truncate_never_panics(
        text in "\\PC{0,200}",
        limit in 0..200usize
    ) {
        // This will find inputs where slicing at `limit` splits a
        // multi-byte character, causing a panic
        let _ = truncate_to_limit(&text, limit);
    }
}
```

When this test finds a multi-byte character that causes a panic, proptest shrinks the input to something like `("a\u{0080}", 1)` — the simplest case that demonstrates the bug: a two-byte character with a limit that slices in the middle.

## Combining Property Tests with Deterministic Tests

Property tests do not replace example-based tests — they complement them. Use example-based tests for documenting known scenarios and property tests for exploring unknown edge cases:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Example-based: documents specific behavior
    #[test]
    fn truncate_empty_string() {
        assert_eq!(truncate_safe("", 10), "");
    }

    #[test]
    fn truncate_within_limit() {
        assert_eq!(truncate_safe("hello", 10), "hello");
    }

    // Property-based: explores the space
    proptest! {
        #[test]
        fn truncate_output_never_exceeds_limit(
            text in "\\PC{0,500}",
            limit in 0..500usize
        ) {
            let result = truncate_safe(&text, limit);
            prop_assert!(result.len() <= limit);
        }

        #[test]
        fn truncate_preserves_valid_utf8(
            text in "\\PC{0,500}",
            limit in 0..500usize
        ) {
            let result = truncate_safe(&text, limit);
            // If this compiles and runs, the result is valid UTF-8
            prop_assert!(result.len() <= text.len());
        }
    }
}

fn truncate_safe(text: &str, limit: usize) -> &str {
    if limit >= text.len() {
        return text;
    }
    // Find the largest valid UTF-8 boundary at or before `limit`
    let mut end = limit;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
```

::: info In the Wild
Production coding agents face a constant stream of unexpected inputs from the LLM. Models sometimes produce malformed JSON, Unicode control characters in file paths, or tool arguments with missing fields. Property-based testing is especially valuable here because it exercises exactly these kinds of edge cases that a developer might not think to test manually. The proptest library's shrinking capability makes it practical to debug the failures it finds.
:::

## Key Takeaways

- Property-based testing generates thousands of random inputs and verifies that invariants hold for all of them, catching edge cases that example-based tests miss
- The most valuable properties for coding agents are "never panics" (parsers, validators), "always rejects" (security checks), and "always preserves" (data integrity)
- Use proptest's built-in strategies for primitives and compose custom strategies for agent-specific types like tool names, tool inputs, and conversation messages
- Proptest automatically shrinks failing inputs to the minimal reproducing case, making it practical to debug the edge cases it discovers
- Combine property tests with example-based tests — use examples to document known behavior and properties to explore unknown edge cases
