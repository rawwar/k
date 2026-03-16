---
title: Summary
description: A consolidated reference of Python-to-Rust mappings and the key mental model shifts needed to write idiomatic Rust.
---

# Summary

> **What you'll learn:**
> - A complete cheat sheet mapping Python concepts to their Rust equivalents
> - The three most important mental model shifts when moving from Python to Rust
> - How to continue deepening your Rust skills as you build the agent in the following chapters

You have covered a lot of ground. From installation to async programming, you now have a working map of how Rust concepts relate to what you already know from Python. This summary consolidates everything into a reference you can return to whenever you need a quick reminder.

## The three mental model shifts

If you remember nothing else from this chapter, remember these three shifts. They are the foundation of thinking in Rust.

### 1. Ownership replaces garbage collection

In Python, you create values and the runtime handles cleanup. In Rust, every value has exactly one owner, and when that owner goes out of scope, the value is dropped immediately.

```python
# Python — garbage collector handles everything
a = [1, 2, 3]
b = a          # Both point to the same list
b.append(4)
print(a)       # [1, 2, 3, 4] — shared mutable state
```

```rust
fn main() {
    let a = vec![1, 2, 3];
    let b = a;       // a is MOVED to b — a is no longer valid
    // println!("{:?}", a);  // compile error: value moved
    println!("{:?}", b);     // [1, 2, 3]
}
```

**Why this matters for the coding agent:** Long-running processes need predictable memory usage. Ownership guarantees deterministic cleanup — no GC pauses, no memory leaks, no surprise resource retention.

### 2. Errors are values, not exceptions

In Python, errors are invisible exceptions that propagate up the call stack. In Rust, errors are explicit `Result<T, E>` values that the compiler forces you to handle.

```python
# Python — errors are invisible in the signature
def read_config(path):
    with open(path) as f:     # might raise FileNotFoundError
        return json.loads(f.read())  # might raise JSONDecodeError
```

```rust
use std::io;

fn read_config(path: &str) -> Result<String, io::Error> {
    let content = std::fs::read_to_string(path)?;  // error is visible
    Ok(content)
}
```

**Why this matters for the coding agent:** An agent that crashes on an unhandled exception is worse than useless. `Result` makes every error path visible and forces handling, which means your agent is robust by construction.

### 3. Types are enforced, not hinted

In Python, type annotations are optional hints checked by external tools. In Rust, types are enforced by the compiler — if it compiles, the types are correct.

```python
# Python — type hints are suggestions
def greet(name: str) -> str:
    return f"Hello, {name}!"

greet(42)  # Type hint says str, but this runs fine (or mypy warns)
```

```rust
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

fn main() {
    // greet(42);  // compile error: expected &str, found integer
    println!("{}", greet("Alice"));
}
```

**Why this matters for the coding agent:** The type system catches mismatches between API responses and your data models, between tool inputs and outputs, and between every function boundary. Bugs that would surface at runtime in Python are caught at compile time in Rust.

## The complete Python-to-Rust cheat sheet

### Language basics

| Python | Rust | Notes |
|--------|------|-------|
| `x = 5` | `let x = 5;` | Immutable by default |
| `x = 5; x = 10` | `let mut x = 5; x = 10;` | Explicit mutability |
| `print(x)` | `println!("{}", x);` | Macro with format string |
| `f"Hello, {name}"` | `format!("Hello, {}", name)` | Format macro |
| `# comment` | `// comment` | Line comments |
| `if x > 0:` | `if x > 0 {` | Braces, no colon |
| `for i in range(10):` | `for i in 0..10 {` | Range syntax |
| `while True:` | `loop {` | Infinite loop |
| `None` | `None` (in `Option`) | Must be unwrapped |
| `True` / `False` | `true` / `false` | Lowercase |
| `pass` | `()` or `{}` | Unit type or empty block |

### Types

| Python | Rust | Notes |
|--------|------|-------|
| `int` | `i32`, `i64`, `u32`, `u64` | Fixed-size integers |
| `float` | `f64` | Default float type |
| `str` | `String` (owned) / `&str` (borrowed) | Two string types |
| `bool` | `bool` | Same concept |
| `list[int]` | `Vec<i32>` | Typed, growable array |
| `dict[str, int]` | `HashMap<String, i32>` | Import from std::collections |
| `set[int]` | `HashSet<i32>` | Import from std::collections |
| `tuple[int, str]` | `(i32, String)` | Fixed-size tuple |
| `Optional[int]` | `Option<i32>` | Compiler-enforced null safety |

### Functions and structures

| Python | Rust | Notes |
|--------|------|-------|
| `def func(x: int) -> str:` | `fn func(x: i32) -> String {` | Types in signature |
| `lambda x: x + 1` | `\|x\| x + 1` | Closure syntax |
| `class Dog:` | `struct Dog {}` + `impl Dog {}` | Data and behavior separated |
| `class Animal(ABC):` | `trait Animal {}` | No inheritance, just traits |
| `def __init__(self):` | `fn new() -> Self` | Convention, not magic |
| `def method(self):` | `fn method(&self)` | Explicit borrow |
| `@dataclass` | `#[derive(Debug, Clone, PartialEq)]` | Derived traits |
| `from enum import Enum` | `enum` keyword | Variants can carry data |
| `__str__` | `Display` trait | Manual implementation |
| `__repr__` | `Debug` trait | Usually derived |

### Error handling

| Python | Rust | Notes |
|--------|------|-------|
| `try: ... except:` | `match result { Ok(v) => ..., Err(e) => ... }` | Explicit matching |
| Implicit propagation | `?` operator | Explicit but ergonomic |
| `raise Exception("msg")` | `Err(MyError::new("msg"))` | Errors are values |
| `except FileNotFoundError:` | `Err(io::Error) if kind == NotFound` | Pattern matching on errors |
| `class MyError(Exception):` | `#[derive(Error)] enum MyError {}` | With thiserror crate |

### Modules and imports

| Python | Rust | Notes |
|--------|------|-------|
| `import os` | `use std::fs;` | Bring module into scope |
| `from os import path` | `use std::path;` | Same pattern |
| `from typing import List` | (not needed — built-in) | Type names are always available |
| `__init__.py` | `mod.rs` | Module root file |
| `_private` convention | No `pub` keyword | Privacy enforced by compiler |
| PyPI | crates.io | Package registry |
| `pip install` | `cargo add` | Add dependency |
| `requirements.txt` | `Cargo.lock` | Lock file |

### Testing

| Python (pytest) | Rust (cargo test) | Notes |
|----------------|-------------------|-------|
| `def test_foo():` | `#[test] fn test_foo() {` | Test discovery |
| `assert x == y` | `assert_eq!(x, y);` | Equality check |
| `pytest.raises(Exc)` | `#[should_panic]` | Expect a panic |
| `tests/test_*.py` | `tests/*.rs` | Integration tests |
| `conftest.py` fixtures | Helper functions | No fixture system |
| `@pytest.mark.asyncio` | `#[tokio::test]` | Async test support |
| `pytest -k pattern` | `cargo test pattern` | Filter tests |
| `pytest -s` | `cargo test -- --nocapture` | Show output |

### Async

| Python (asyncio) | Rust (tokio) | Notes |
|------------------|-------------|-------|
| `async def f():` | `async fn f() {` | Same keyword |
| `await coro` | `expr.await` | Postfix syntax in Rust |
| `asyncio.run(main())` | `#[tokio::main]` | Runtime setup |
| `asyncio.gather(a, b)` | `tokio::join!(a, b)` | Concurrent execution |
| `asyncio.create_task(c)` | `tokio::spawn(c)` | Background task |
| `asyncio.wait_for(c, t)` | `tokio::select!` with timeout | Racing futures |

## What you are ready for

You now have the vocabulary and mental models to read and write Rust. You understand:

- **Memory management** through ownership and borrowing instead of garbage collection
- **Type safety** through the compiler, not optional annotations
- **Error handling** through `Result` and `Option` instead of exceptions
- **Abstraction** through traits and generics instead of class inheritance
- **Concurrency** through async/await with tokio instead of asyncio
- **Project structure** through Cargo, modules, and crates instead of pip and packages

In the chapters ahead, you will apply all of this to build a real coding agent. The next chapter begins the implementation — creating the project structure, setting up dependencies, and writing the first lines of your agent's code.

When you encounter unfamiliar Rust concepts in the implementation chapters, come back to this chapter as a reference. The Python comparisons will help anchor new ideas to what you already know.

## Recommended resources for going deeper

- [The Rust Programming Language](https://doc.rust-lang.org/book/) (the official book — free online)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/) (learn through annotated examples)
- [Rustlings](https://github.com/rust-lang/rustlings) (small exercises to practice Rust syntax)
- [Comprehensive Rust](https://google.github.io/comprehensive-rust/) (Google's Rust course)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial) (in-depth async Rust with tokio)

## Exercises

These exercises focus on deepening your understanding of the conceptual differences between Python and Rust. They emphasize reasoning about language design trade-offs rather than writing complete programs.

### Exercise 1: Ownership Mental Model Diagram (Easy)

Draw a diagram (or describe in text) showing what happens in memory for these three Python lines and the equivalent Rust lines:

```python
a = [1, 2, 3]
b = a
a.append(4)
```

Show: where the data lives, what `a` and `b` point to, and what happens at each step. Then do the same for the Rust equivalent using `Vec<i32>`, showing the move semantics. Explain why the Rust version prevents the shared mutable state that Python allows.

**Deliverable:** Two annotated diagrams (Python and Rust) and a one-paragraph explanation of the trade-off between Python's flexibility and Rust's safety.

### Exercise 2: Error Handling Comparison (Easy)

Take this Python function and translate its error handling approach to Rust. Do not write the full implementation -- instead, write only the function signature and the error type, and explain how each Python error scenario maps to a Rust `Result` variant:

```python
def parse_config(path: str) -> dict:
    with open(path) as f:           # FileNotFoundError, PermissionError
        text = f.read()             # UnicodeDecodeError
    config = json.loads(text)       # JSONDecodeError
    if "name" not in config:
        raise ValueError("missing 'name' field")
    return config
```

**Deliverable:** A Rust function signature, a custom error enum with variants for each failure, and a paragraph explaining why Rust's approach catches more bugs at compile time.

### Exercise 3: Trait vs. Duck Typing Design Analysis (Medium)

In Python, you can write `for item in collection` for any object that implements `__iter__`. In Rust, you implement the `Iterator` trait. Compare these two approaches by designing a "Searchable" abstraction -- something that can be searched with a query string and returns results.

**What to consider:** How would you define the interface in Python (using duck typing, `abc.ABC`, or `Protocol`)? How would you define it in Rust (using a trait)? What happens when someone passes an object that does not support the interface? When does each approach catch the error? What are the implications for a coding agent that dispatches tool calls based on tool names?

**Deliverable:** The Python and Rust interface definitions side by side, with analysis of the compile-time vs. runtime trade-offs for tool dispatch.

### Exercise 4: Translating Python Patterns to Rust (Medium)

For each of these common Python patterns, describe how you would express the same intent in Rust. Do not write full implementations -- write the type signatures and explain the key differences:

1. A dictionary with default values (`collections.defaultdict`)
2. A context manager (`with open(...) as f:`)
3. A generator function (`yield` keyword)
4. Multiple inheritance with mixins

**What to consider:** Some patterns translate directly, some require rethinking the approach entirely. For each, explain whether Rust offers a direct equivalent, a different mechanism that achieves the same goal, or a fundamentally different approach.

**Deliverable:** Four short descriptions with Rust type signatures and explanations of how each pattern maps (or does not map) across languages.

### Exercise 5: Designing a Bilingual Code Review Checklist (Hard)

Create a code review checklist for a team where some developers write Python and others write Rust, and both contribute to the same agent project. The checklist should identify: (a) common mistakes Python developers make in Rust, (b) common mistakes Rust developers make when designing Python-interop boundaries, and (c) patterns that look correct in one language but are subtly wrong in the other.

**What to consider:** Think about string handling (`str` vs `String`/`&str`), error propagation (exceptions vs `Result`), null safety (`None` vs `Option`), mutability defaults, and concurrency models. Reference specific examples from the cheat sheets in this chapter.

**Deliverable:** A structured checklist with at least 3 items per category, each with a concrete example and an explanation of why the mistake is easy to make.

## Key Takeaways

- The three critical mental shifts from Python to Rust: ownership replaces garbage collection, errors are values instead of exceptions, and types are enforced by the compiler instead of hinted
- Rust's strictness is not a burden — it is a feature. The compiler catches bugs that Python discovers at runtime, making your code more robust by construction
- Almost every Python concept has a direct Rust equivalent — the syntax differs but the intent is the same
- You now have enough Rust knowledge to build the coding agent — refer back to this chapter's cheat sheets as needed
- The Rust compiler is your best teacher — read its error messages carefully, and you will learn the language faster than any tutorial
