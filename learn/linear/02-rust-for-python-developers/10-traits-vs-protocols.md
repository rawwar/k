---
title: Traits vs Protocols
description: Understanding Rust traits as the equivalent of Python protocols and abstract base classes, enabling polymorphism without inheritance.
---

# Traits vs Protocols

> **What you'll learn:**
> - How Rust traits define shared behavior, analogous to Python's Protocol and ABC classes
> - The difference between static dispatch (generics) and dynamic dispatch (dyn Trait) for trait objects
> - How to implement standard library traits like Display, Debug, Clone, and From for your types

In Python, you define shared behavior through class inheritance, abstract base classes (ABCs), or the newer `Protocol` type. Rust has no inheritance. Instead, it uses *traits* — named sets of methods that types can implement. Traits are the foundation of polymorphism in Rust, and they are more flexible than inheritance because a type can implement any number of traits without forming a hierarchy.

## Python ABCs and Protocols vs Rust traits

**Python — abstract base class:**

```python
from abc import ABC, abstractmethod

class Tool(ABC):
    @abstractmethod
    def name(self) -> str:
        ...

    @abstractmethod
    def execute(self, input: str) -> str:
        ...

    # Concrete method using abstract methods
    def describe(self) -> str:
        return f"Tool: {self.name()}"

class ShellTool(Tool):
    def name(self) -> str:
        return "shell"

    def execute(self, input: str) -> str:
        import subprocess
        result = subprocess.run(input, shell=True, capture_output=True, text=True)
        return result.stdout
```

**Rust — trait:**

```rust
trait Tool {
    fn name(&self) -> &str;
    fn execute(&self, input: &str) -> String;

    // Default method (like a concrete method in Python ABC)
    fn describe(&self) -> String {
        format!("Tool: {}", self.name())
    }
}

struct ShellTool;

impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn execute(&self, input: &str) -> String {
        // Simplified — real implementation would use std::process::Command
        format!("Executing: {}", input)
    }
}

fn main() {
    let tool = ShellTool;
    println!("{}", tool.describe());    // "Tool: shell"
    println!("{}", tool.execute("ls")); // "Executing: ls"
}
```

::: python Coming from Python
The structure maps directly:
- Python's `@abstractmethod` = Rust's method without a body in the trait
- Python's concrete methods on an ABC = Rust's default method implementations
- Python's `class ShellTool(Tool)` = Rust's `impl Tool for ShellTool`

The key difference: Python classes inherit *data and behavior* from parent classes. Rust traits define *only behavior*. There is no data inheritance, no `super().__init__()`, and no diamond inheritance problem.
:::

## Implementing multiple traits

A Rust type can implement as many traits as you want — this is like a Python class implementing multiple protocols:

```rust
use std::fmt;

trait Tool {
    fn name(&self) -> &str;
    fn execute(&self, input: &str) -> String;
}

trait Describable {
    fn describe(&self) -> String;
}

struct ShellTool {
    working_dir: String,
}

impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn execute(&self, input: &str) -> String {
        format!("[{}] $ {}", self.working_dir, input)
    }
}

impl Describable for ShellTool {
    fn describe(&self) -> String {
        format!("Shell tool in {}", self.working_dir)
    }
}

impl fmt::Display for ShellTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ShellTool({})", self.working_dir)
    }
}

fn main() {
    let tool = ShellTool {
        working_dir: String::from("/home/user"),
    };
    println!("{}", tool.execute("ls"));   // Tool trait
    println!("{}", tool.describe());       // Describable trait
    println!("{}", tool);                  // Display trait
}
```

::: python Coming from Python
This is like implementing multiple Python protocols or ABCs:
```python
class ShellTool(Tool, Describable):
    def __init__(self, working_dir: str):
        self.working_dir = working_dir
    # ... implement all required methods
```
In Rust, you implement each trait in a separate `impl` block instead of listing parents in the class definition. This has a subtle advantage: you can implement traits for types defined in *other* crates (with some restrictions), which is impossible with Python class inheritance on third-party classes.
:::

## Standard library traits you will use

Rust has several traits that you will implement constantly. These are the equivalent of Python's dunder methods:

| Python dunder | Rust trait | Purpose |
|---------------|-----------|---------|
| `__str__` | `Display` | User-facing string representation |
| `__repr__` | `Debug` | Developer-facing string representation |
| `__eq__` | `PartialEq` | Equality comparison with `==` |
| `__lt__`, `__gt__` | `PartialOrd` | Ordering comparisons |
| `__hash__` | `Hash` | Hashing for use in collections |
| `__iter__` | `IntoIterator` | Making a type iterable |
| `__len__` | (no direct equiv) | Use a `.len()` method |
| `__enter__`/`__exit__` | `Drop` | Cleanup when value is dropped |

Most of these can be auto-derived:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ToolCall {
    id: String,
    name: String,
    arguments: String,
}

fn main() {
    let call = ToolCall {
        id: String::from("tc_1"),
        name: String::from("shell"),
        arguments: String::from("ls -la"),
    };

    println!("{:?}", call);  // Debug
    let call2 = call.clone();  // Clone
    assert_eq!(call, call2);   // PartialEq
}
```

`Display` cannot be derived — you must implement it manually:

```rust
use std::fmt;

#[derive(Debug)]
struct ToolCall {
    id: String,
    name: String,
    arguments: String,
}

impl fmt::Display for ToolCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.name, self.arguments)
    }
}

fn main() {
    let call = ToolCall {
        id: String::from("tc_1"),
        name: String::from("shell"),
        arguments: String::from("ls -la"),
    };

    println!("{}", call);    // Display: "shell(ls -la)"
    println!("{:?}", call);  // Debug: ToolCall { id: "tc_1", name: "shell", arguments: "ls -la" }
}
```

## The From trait — type conversions

The `From` trait defines how to convert between types. It is like having a constructor that accepts another type:

```rust
#[derive(Debug)]
struct AgentError {
    message: String,
}

impl From<std::io::Error> for AgentError {
    fn from(e: std::io::Error) -> Self {
        AgentError {
            message: format!("I/O error: {}", e),
        }
    }
}

impl From<String> for AgentError {
    fn from(s: String) -> Self {
        AgentError { message: s }
    }
}

fn main() {
    // From is called automatically by .into()
    let err: AgentError = String::from("something went wrong").into();
    println!("{:?}", err);
}
```

::: python Coming from Python
`From` is similar to Python's convention of accepting different types in `__init__`:
```python
class AgentError(Exception):
    def __init__(self, source):
        if isinstance(source, IOError):
            super().__init__(f"I/O error: {source}")
        elif isinstance(source, str):
            super().__init__(source)
```
Rust's `From` trait makes these conversions type-safe and discoverable. The `?` operator uses `From` automatically — when an error type implements `From`, `?` converts it for you.
:::

## Static dispatch vs dynamic dispatch

There are two ways to use traits for polymorphism:

### Static dispatch (generics) — resolved at compile time

```rust
trait Tool {
    fn name(&self) -> &str;
}

struct ShellTool;
impl Tool for ShellTool {
    fn name(&self) -> &str { "shell" }
}

struct ReadFileTool;
impl Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }
}

// Static dispatch — the compiler generates a specialized version for each type
fn print_tool_name(tool: &impl Tool) {
    println!("Tool: {}", tool.name());
}

fn main() {
    print_tool_name(&ShellTool);     // compiler generates print_tool_name::<ShellTool>
    print_tool_name(&ReadFileTool);  // compiler generates print_tool_name::<ReadFileTool>
}
```

### Dynamic dispatch (trait objects) — resolved at runtime

```rust
trait Tool {
    fn name(&self) -> &str;
}

struct ShellTool;
impl Tool for ShellTool {
    fn name(&self) -> &str { "shell" }
}

struct ReadFileTool;
impl Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }
}

fn main() {
    // Dynamic dispatch — different types in one collection
    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(ShellTool),
        Box::new(ReadFileTool),
    ];

    for tool in &tools {
        println!("Tool: {}", tool.name());
    }
}
```

::: python Coming from Python
Python always uses dynamic dispatch — method lookups happen at runtime through the MRO (method resolution order). Rust gives you both options:
- **Static dispatch** (`impl Trait` or generics) — zero runtime overhead, the compiler generates specialized code. Use this when you know the type at compile time.
- **Dynamic dispatch** (`dyn Trait`) — small runtime overhead for a vtable lookup. Use this when you need a collection of different types that implement the same trait, like a `Vec<Box<dyn Tool>>`.

For our coding agent, we will use `dyn Tool` to store different tool implementations in a single registry.
:::

## Key Takeaways

- Rust traits define shared behavior like Python's ABCs and Protocols, but without data inheritance — no `super().__init__()`, no diamond problem
- Types can implement any number of traits, with each implementation in its own `impl` block — more flexible than Python's class inheritance
- Standard library traits (`Display`, `Debug`, `Clone`, `PartialEq`) map to Python's dunder methods, and most can be auto-derived with `#[derive]`
- Static dispatch (`impl Trait`) has zero overhead and is preferred when the type is known at compile time; dynamic dispatch (`dyn Trait`) is needed for heterogeneous collections
- The `From` trait enables type conversions and works automatically with the `?` operator for error propagation
