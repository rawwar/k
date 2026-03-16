---
title: Enums and Pattern Matching
description: Rust's powerful algebraic data types and exhaustive pattern matching, compared to Python's enums and if/elif chains.
---

# Enums and Pattern Matching

> **What you'll learn:**
> - How Rust enums carry data in their variants, making them far more powerful than Python's Enum class
> - How match expressions enforce exhaustive handling of every variant at compile time
> - When to use if let, while let, and nested patterns for concise control flow

Rust enums are one of the language's superpowers. If you are coming from Python, you might think of enums as simple label constants — `Color.RED`, `Status.ACTIVE`. Rust enums are fundamentally different. Each variant can carry its own data, making them closer to tagged unions or algebraic data types. Combined with pattern matching, they let you model complex states with absolute type safety.

## Python enums vs Rust enums

**Python — simple enums (labels only):**

```python
from enum import Enum

class Role(Enum):
    USER = "user"
    ASSISTANT = "assistant"
    SYSTEM = "system"

role = Role.USER
print(role.value)  # "user"
```

**Rust — enums with the same simplicity:**

```rust
#[derive(Debug)]
enum Role {
    User,
    Assistant,
    System,
}

fn main() {
    let role = Role::User;
    println!("{:?}", role);  // User
}
```

So far, they look similar. But here is where Rust pulls ahead dramatically.

## Enums with data

Rust enum variants can carry different types and amounts of data:

```rust
#[derive(Debug)]
enum Message {
    Text(String),
    Image { url: String, width: u32, height: u32 },
    ToolCall { name: String, arguments: String },
    ToolResult { tool_call_id: String, output: String },
    Empty,
}

fn main() {
    let msg1 = Message::Text(String::from("Hello, can you help me?"));

    let msg2 = Message::ToolCall {
        name: String::from("read_file"),
        arguments: String::from(r#"{"path": "src/main.rs"}"#),
    };

    let msg3 = Message::Empty;

    println!("{:?}", msg1);
    println!("{:?}", msg2);
    println!("{:?}", msg3);
}
```

Each variant is like a different struct, but they all live under the same type `Message`. This means a function that takes `Message` can handle any of these variants — and the compiler ensures you handle all of them.

::: python Coming from Python
To achieve something similar in Python, you would use a union of dataclasses or `typing.Union`:
```python
from dataclasses import dataclass
from typing import Union

@dataclass
class TextMessage:
    content: str

@dataclass
class ToolCallMessage:
    name: str
    arguments: str

@dataclass
class EmptyMessage:
    pass

Message = Union[TextMessage, ToolCallMessage, EmptyMessage]
```
This is more verbose, the type checker only *suggests* you handle all variants (it does not *enforce* it), and at runtime there is no connection between these types. Rust's enum makes the relationship explicit and the compiler guarantees exhaustive handling.
:::

## Pattern matching with `match`

The `match` expression is how you work with enums. It is like Python's `match` statement (3.10+) but with compile-time exhaustiveness checking:

```rust
#[derive(Debug)]
enum Message {
    Text(String),
    ToolCall { name: String, arguments: String },
    ToolResult { tool_call_id: String, output: String },
    Empty,
}

fn describe_message(msg: &Message) -> String {
    match msg {
        Message::Text(content) => format!("Text: {}", content),
        Message::ToolCall { name, arguments } => {
            format!("Tool call: {}({})", name, arguments)
        }
        Message::ToolResult { tool_call_id, output } => {
            format!("Result for {}: {}", tool_call_id, output)
        }
        Message::Empty => String::from("(empty message)"),
    }
}

fn main() {
    let msg = Message::ToolCall {
        name: String::from("shell"),
        arguments: String::from("ls -la"),
    };
    println!("{}", describe_message(&msg));
}
```

The compiler enforces that every variant is handled. If you forget one:

```rust
fn describe_message(msg: &Message) -> String {
    match msg {
        Message::Text(content) => format!("Text: {}", content),
        Message::ToolCall { name, .. } => format!("Tool: {}", name),
        // ERROR: non-exhaustive patterns: `Empty` and `ToolResult { .. }` not covered
    }
}
```

::: python Coming from Python
Python 3.10 added structural pattern matching with `match`/`case`, but it does *not* enforce exhaustiveness:
```python
match msg:
    case TextMessage(content=c):
        print(f"Text: {c}")
    case ToolCallMessage(name=n):
        print(f"Tool: {n}")
    # Forgetting EmptyMessage — no warning, no error. It just falls through silently.
```
Rust's exhaustiveness checking is one of the most valuable features of the type system. It means that when you add a new variant to an enum, the compiler tells you every place in your codebase that needs to handle it. You will never have an unhandled case silently doing nothing.
:::

## The wildcard pattern `_`

Sometimes you want to handle specific variants and group everything else:

```rust
#[derive(Debug)]
enum Message {
    Text(String),
    ToolCall { name: String, arguments: String },
    ToolResult { tool_call_id: String, output: String },
    Empty,
}

fn is_actionable(msg: &Message) -> bool {
    match msg {
        Message::ToolCall { .. } => true,
        _ => false,  // everything else is not actionable
    }
}

fn main() {
    let msg = Message::Empty;
    println!("Actionable? {}", is_actionable(&msg));
}
```

Use `_` when you have handled the cases you care about and want a catch-all. The `..` inside a variant ignores specific fields you do not need.

## `if let` — matching a single pattern

When you only care about one variant, `match` can feel heavy. Use `if let` instead:

```rust
#[derive(Debug)]
enum Message {
    Text(String),
    ToolCall { name: String, arguments: String },
    Empty,
}

fn main() {
    let msg = Message::Text(String::from("Hello!"));

    // Full match — overkill when you only care about one variant
    match &msg {
        Message::Text(content) => println!("Got text: {}", content),
        _ => {}
    }

    // if let — cleaner for single-pattern matching
    if let Message::Text(content) = &msg {
        println!("Got text: {}", content);
    }

    // if let with else
    if let Message::ToolCall { name, .. } = &msg {
        println!("Calling tool: {}", name);
    } else {
        println!("Not a tool call");
    }
}
```

::: python Coming from Python
`if let` is like using `isinstance()` checks in Python:
```python
if isinstance(msg, TextMessage):
    print(f"Got text: {msg.content}")
```
The difference is that `if let` also *destructures* the value — it extracts the data inside the variant into variables in one step, where Python requires a separate attribute access.
:::

## Option and Result — enums you will use everywhere

The two most important enums in Rust are defined in the standard library:

```rust
// Option — a value that might be absent (replaces None/null)
enum Option<T> {
    Some(T),
    None,
}

// Result — an operation that might fail (replaces exceptions)
enum Result<T, E> {
    Ok(T),
    Err(E),
}
```

You will use these constantly:

```rust
fn find_tool(name: &str) -> Option<String> {
    match name {
        "shell" => Some(String::from("Execute shell commands")),
        "read_file" => Some(String::from("Read file contents")),
        _ => None,
    }
}

fn main() {
    // Handling Option with match
    match find_tool("shell") {
        Some(description) => println!("Found: {}", description),
        None => println!("Tool not found"),
    }

    // Handling Option with if let
    if let Some(desc) = find_tool("read_file") {
        println!("Read file tool: {}", desc);
    }

    // Using unwrap_or for a default value (like Python's dict.get(key, default))
    let desc = find_tool("unknown").unwrap_or(String::from("No description"));
    println!("{}", desc);
}
```

::: python Coming from Python
`Option<T>` replaces Python's pattern of returning `None`:
```python
def find_tool(name: str) -> str | None:
    tools = {"shell": "Execute commands", "read_file": "Read files"}
    return tools.get(name)

# In Python, you might forget to check for None:
desc = find_tool("unknown")
print(desc.upper())  # AttributeError: 'NoneType' has no attribute 'upper'
```
In Rust, the compiler *refuses* to let you access the value inside an `Option` without first checking if it is `Some`. The `NoneType has no attribute` error is structurally impossible.
:::

## Nested pattern matching

Patterns can be deeply nested:

```rust
#[derive(Debug)]
enum ToolOutput {
    Success(String),
    Error(String),
}

#[derive(Debug)]
enum AgentAction {
    Respond(String),
    UseTool { name: String, result: Option<ToolOutput> },
    Stop,
}

fn describe_action(action: &AgentAction) -> String {
    match action {
        AgentAction::Respond(text) => format!("Responding: {}", text),
        AgentAction::UseTool { name, result: Some(ToolOutput::Success(output)) } => {
            format!("Tool {} succeeded: {}", name, output)
        }
        AgentAction::UseTool { name, result: Some(ToolOutput::Error(err)) } => {
            format!("Tool {} failed: {}", name, err)
        }
        AgentAction::UseTool { name, result: None } => {
            format!("Tool {} pending", name)
        }
        AgentAction::Stop => String::from("Agent stopping"),
    }
}

fn main() {
    let action = AgentAction::UseTool {
        name: String::from("shell"),
        result: Some(ToolOutput::Success(String::from("file.txt created"))),
    };
    println!("{}", describe_action(&action));
}
```

This kind of nested matching would require multiple `isinstance` checks and `if/elif` chains in Python. Rust handles it in a single, readable `match` expression with compile-time exhaustiveness guarantees.

## Match with guards

You can add conditions to match arms with `if` guards:

```rust
fn classify_score(score: i32) -> &'static str {
    match score {
        s if s >= 90 => "excellent",
        s if s >= 70 => "good",
        s if s >= 50 => "passing",
        _ => "failing",
    }
}

fn main() {
    println!("{}", classify_score(85));  // "good"
    println!("{}", classify_score(45));  // "failing"
}
```

## Key Takeaways

- Rust enums are algebraic data types — each variant can carry different data, making them far more expressive than Python's `Enum` class
- `match` expressions enforce exhaustive handling of all variants at compile time — you can never forget to handle a case
- `Option<T>` replaces Python's `None` with compile-time safety — you cannot access a value without first checking if it exists
- `if let` provides a concise syntax for matching a single variant, similar to Python's `isinstance()` but with built-in destructuring
- Nested pattern matching handles complex data structures in a single expression that would require chains of `isinstance` and `if/elif` in Python
