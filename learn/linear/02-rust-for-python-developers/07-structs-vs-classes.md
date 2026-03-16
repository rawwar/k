---
title: Structs vs Classes
description: Mapping Python classes to Rust structs and impl blocks, understanding how Rust separates data from behavior.
---

# Structs vs Classes

> **What you'll learn:**
> - How Rust structs define data and impl blocks attach methods, compared to Python's unified class model
> - The differences between named structs, tuple structs, and unit structs and when to use each
> - How to implement constructors, methods, and associated functions using idiomatic Rust patterns

If you know Python classes, you already understand the concept behind Rust structs. Both let you group related data together and attach behavior. But Rust splits what Python combines into a single `class` keyword into two separate constructs: **structs** define the data, and **impl blocks** define the behavior. This separation turns out to be a powerful design choice.

## Python classes vs Rust structs — side by side

Let's model a message in a coding agent's conversation:

**Python:**

```python
class Message:
    def __init__(self, role: str, content: str, token_count: int = 0):
        self.role = role
        self.content = content
        self.token_count = token_count

    def summary(self) -> str:
        return f"[{self.role}] {self.content[:50]}..."

    def is_from_user(self) -> bool:
        return self.role == "user"
```

**Rust:**

```rust
struct Message {
    role: String,
    content: String,
    token_count: usize,
}

impl Message {
    fn summary(&self) -> String {
        format!("[{}] {}...", self.role, &self.content[..50.min(self.content.len())])
    }

    fn is_from_user(&self) -> bool {
        self.role == "user"
    }
}

fn main() {
    let msg = Message {
        role: String::from("user"),
        content: String::from("Help me refactor this function"),
        token_count: 12,
    };

    println!("{}", msg.summary());
    println!("From user? {}", msg.is_from_user());
}
```

::: python Coming from Python
In Python, `class Message:` defines both the data (`self.role`, etc.) and the methods (`summary`, `is_from_user`) in one block. In Rust, the `struct Message {}` block defines *only* the fields, and a separate `impl Message {}` block adds methods. This separation means you can add methods to a struct in multiple `impl` blocks, even in different files. It is like being able to add methods to a Python class from outside the class definition — Rust's trait system (covered later) takes full advantage of this.
:::

## Creating instances (constructors)

Rust has no `__init__`. Instead, you use a conventional `new` associated function:

**Python:**

```python
msg = Message("user", "Hello", token_count=5)
```

**Rust:**

```rust
struct Message {
    role: String,
    content: String,
    token_count: usize,
}

impl Message {
    // Associated function (no &self) — called with Message::new()
    fn new(role: String, content: String) -> Self {
        Message {
            role,
            content,
            token_count: 0,
        }
    }

    // Another constructor with all fields
    fn with_token_count(role: String, content: String, token_count: usize) -> Self {
        Message {
            role,
            content,
            token_count,
        }
    }
}

fn main() {
    let msg1 = Message::new(String::from("user"), String::from("Hello"));
    let msg2 = Message::with_token_count(
        String::from("assistant"),
        String::from("I can help with that"),
        8,
    );

    println!("msg1 tokens: {}", msg1.token_count);  // 0
    println!("msg2 tokens: {}", msg2.token_count);  // 8
}
```

Notice:
- `new` does not take `&self` — it is an *associated function* (like a Python `@staticmethod` or `@classmethod`)
- You call it with `Message::new(...)`, not `msg.new(...)`
- `Self` is an alias for the struct type (like Python's `self.__class__`)
- When a field name matches the variable name, you can use shorthand: `role` instead of `role: role`

::: python Coming from Python
Python's `__init__` is special — it is called automatically when you create an instance. Rust has no such magic. `new` is just a convention, not a language feature. You could name the constructor `create` or `build` and it would work the same way. The upside is transparency — there is no hidden behavior during construction.
:::

## Methods: `&self`, `&mut self`, and `self`

Methods in Rust explicitly declare how they access the struct:

```rust
struct Counter {
    value: i32,
    name: String,
}

impl Counter {
    // Borrows self immutably — can read but not modify
    fn get_value(&self) -> i32 {
        self.value
    }

    // Borrows self mutably — can read and modify
    fn increment(&mut self) {
        self.value += 1;
    }

    // Takes ownership of self — consumes the struct
    fn into_name(self) -> String {
        self.name  // self is moved, Counter is consumed
    }
}

fn main() {
    let mut counter = Counter {
        value: 0,
        name: String::from("requests"),
    };

    println!("Value: {}", counter.get_value());  // borrows immutably
    counter.increment();                          // borrows mutably
    counter.increment();
    println!("Value: {}", counter.get_value());  // 2

    let name = counter.into_name();  // counter is consumed — can't use it anymore
    println!("Name was: {}", name);
    // println!("{}", counter.value);  // ERROR: counter was moved
}
```

::: python Coming from Python
In Python, every method takes `self` and can do whatever it wants — read, write, delete attributes. Rust forces you to declare your intent:
- `&self` — "I will only read" (like a method on a frozen dataclass)
- `&mut self` — "I will modify" (like a normal method)
- `self` — "I will consume this" (like a method that calls `del self` at the end — not common in Python)

This explicitness is why Rust code is easier to reason about. When you see `fn summary(&self)`, you know it cannot modify the struct. The compiler guarantees it.
:::

## Tuple structs and unit structs

Rust has three kinds of structs:

```rust
// Named struct — fields have names (most common)
struct Point {
    x: f64,
    y: f64,
}

// Tuple struct — fields accessed by position
struct Color(u8, u8, u8);

// Unit struct — no fields (used as markers or for trait implementations)
struct Placeholder;

fn main() {
    let origin = Point { x: 0.0, y: 0.0 };
    println!("({}, {})", origin.x, origin.y);

    let red = Color(255, 0, 0);
    println!("R={}, G={}, B={}", red.0, red.1, red.2);

    let _p = Placeholder;  // exists only as a type, no data
}
```

Tuple structs are useful for newtype patterns — wrapping a single value to create a distinct type:

```rust
struct UserId(u64);
struct MessageId(u64);

fn send_message(user: UserId, message: MessageId) {
    println!("Sending message {} to user {}", message.0, user.0);
}

fn main() {
    let user = UserId(42);
    let msg = MessageId(100);
    send_message(user, msg);
    // send_message(msg, user);  // ERROR: types don't match
}
```

::: python Coming from Python
Python's `typing.NewType` does something similar but only at the type-checker level — at runtime, a `UserId` is just an `int`. Rust's newtype pattern creates an actual distinct type that the compiler enforces. You cannot accidentally pass a `MessageId` where a `UserId` is expected.
:::

## Deriving common behavior

In Python, classes inherit useful methods through `__repr__`, `__eq__`, `__hash__`, etc. In Rust, you *derive* these with a `#[derive]` attribute:

```rust
#[derive(Debug, Clone, PartialEq)]
struct Message {
    role: String,
    content: String,
    token_count: usize,
}

fn main() {
    let msg = Message {
        role: String::from("user"),
        content: String::from("Hello"),
        token_count: 3,
    };

    // Debug printing (like Python's __repr__)
    println!("{:?}", msg);
    // Message { role: "user", content: "Hello", token_count: 3 }

    // Clone (like copy.deepcopy)
    let msg2 = msg.clone();

    // Equality comparison (like __eq__)
    assert_eq!(msg, msg2);
}
```

| Python | Rust derive | What it does |
|--------|-------------|-------------|
| `__repr__` | `Debug` | Debug formatting with `{:?}` |
| `__str__` | `Display` (manual impl) | User-facing formatting with `{}` |
| `__eq__` | `PartialEq` | Equality with `==` |
| `__hash__` | `Hash` | Hashing for use in HashMaps |
| `copy.deepcopy` | `Clone` | Explicit deep copy with `.clone()` |
| `copy.copy` | `Copy` | Implicit copy on assignment (only for simple types) |

## Putting it together — a real-world struct

Here is a struct you might actually use in a coding agent, with constructors, methods, and derives:

```rust
#[derive(Debug, Clone)]
struct ToolResult {
    tool_name: String,
    output: String,
    success: bool,
    duration_ms: u64,
}

impl ToolResult {
    fn new(tool_name: String, output: String, success: bool, duration_ms: u64) -> Self {
        ToolResult {
            tool_name,
            output,
            success,
            duration_ms,
        }
    }

    fn is_error(&self) -> bool {
        !self.success
    }

    fn truncated_output(&self, max_len: usize) -> &str {
        if self.output.len() <= max_len {
            &self.output
        } else {
            &self.output[..max_len]
        }
    }

    fn into_output(self) -> String {
        self.output
    }
}

fn main() {
    let result = ToolResult::new(
        String::from("shell"),
        String::from("Hello from the subprocess!"),
        true,
        42,
    );

    println!("Tool: {}", result.tool_name);
    println!("Error? {}", result.is_error());
    println!("Preview: {}", result.truncated_output(15));
    println!("{:?}", result);
}
```

## Key Takeaways

- Rust structs define data and `impl` blocks attach methods — this separation enables adding behavior from multiple locations and through traits
- Methods explicitly declare how they access data: `&self` (read), `&mut self` (modify), or `self` (consume) — the compiler enforces these guarantees
- Constructors are conventional (`new`, `with_*`) not magical — there is no equivalent of Python's `__init__`
- `#[derive(...)]` generates common trait implementations automatically, similar to Python's `@dataclass` or dunder methods
- Tuple structs and the newtype pattern create distinct types from existing ones, providing type safety that Python's `NewType` only offers at the type-checker level
