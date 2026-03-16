---
title: Strings in Rust
description: Navigating Rust's string types — String vs &str — and understanding UTF-8 encoding compared to Python's str.
---

# Strings in Rust

> **What you'll learn:**
> - Why Rust has two main string types (String and &str) and when to use each
> - How Rust enforces valid UTF-8 encoding and what that means for string manipulation
> - Common string operations like formatting, slicing, and concatenation compared to Python equivalents

Strings in Rust confuse every Python developer at first. In Python, there is one string type: `str`. It holds Unicode text, you can slice it, concatenate it, and never think about encoding. In Rust, there are two main string types, and understanding why they exist is essential.

## The two string types

| Type | Owned? | Growable? | Where it lives | Python equivalent |
|------|--------|-----------|----------------|-------------------|
| `String` | Yes | Yes | Heap | `str` (most similar) |
| `&str` | No (borrowed) | No | Anywhere | A read-only view of a `str` |

**`String`** is an owned, heap-allocated, growable string. You use it when you need to create, modify, or own string data.

**`&str`** (pronounced "string slice") is a *reference* to string data. It is a view into a `String`, a string literal, or any contiguous UTF-8 bytes. You use it when you just need to read string data.

```rust
fn main() {
    // String — owned, on the heap, can be modified
    let mut owned = String::from("Hello");
    owned.push_str(", world!");
    println!("{}", owned);  // "Hello, world!"

    // &str — a borrowed view, read-only
    let slice: &str = "Hello, world!";  // string literals are &str
    println!("{}", slice);

    // A &str can borrow from a String
    let view: &str = &owned;
    println!("{}", view);
}
```

::: python Coming from Python
Think of it this way:
- `String` is like Python's `str` — you can create it, modify it, pass it around, and it manages its own memory
- `&str` is like a `memoryview` or a read-only slice of a string — it points to data owned by someone else

In Python, you never make this distinction because Python handles memory automatically. In Rust, the distinction matters because it tells the compiler who is responsible for the memory and what operations are safe.

The good news: once you internalize that function parameters should usually take `&str` and function return values should usually return `String`, most string decisions become automatic.
:::

## Creating strings

There are several ways to create `String` values:

```rust
fn main() {
    // From a string literal
    let s1 = String::from("hello");
    let s2 = "hello".to_string();  // equivalent

    // From formatting
    let name = "Agent";
    let s3 = format!("Hello, {}!", name);

    // Empty string, then build it up
    let mut s4 = String::new();
    s4.push_str("Hello");
    s4.push(' ');  // push a single char
    s4.push_str("world");

    // From other types
    let s5 = 42.to_string();
    let s6 = true.to_string();

    println!("{} | {} | {} | {} | {} | {}", s1, s2, s3, s4, s5, s6);
}
```

::: python Coming from Python
In Python, you create strings with literals, f-strings, `str()`, and `.join()`. The Rust equivalents:

| Python | Rust |
|--------|------|
| `"hello"` | `"hello"` (this is `&str`) or `String::from("hello")` |
| `f"Hello, {name}!"` | `format!("Hello, {}!", name)` |
| `str(42)` | `42.to_string()` |
| `"".join(parts)` | `parts.join("")` or `parts.concat()` |
| `s = ""; s += "hello"` | `let mut s = String::new(); s.push_str("hello");` |
:::

## Concatenation

String concatenation in Rust looks different from Python:

```rust
fn main() {
    // Using format! (most common and readable)
    let first = "Hello";
    let second = "world";
    let combined = format!("{}, {}!", first, second);
    println!("{}", combined);

    // Using push_str (mutating an existing String)
    let mut greeting = String::from("Hello");
    greeting.push_str(", ");
    greeting.push_str("world!");
    println!("{}", greeting);

    // Using + operator (takes ownership of left side)
    let hello = String::from("Hello");
    let world = String::from(", world!");
    let combined = hello + &world;  // hello is moved, world is borrowed
    // println!("{}", hello);  // ERROR: hello was moved
    println!("{}", combined);

    // Joining a collection (like Python's "sep".join(list))
    let parts = vec!["one", "two", "three"];
    let joined = parts.join(", ");
    println!("{}", joined);  // "one, two, three"
}
```

::: python Coming from Python
Python's `+` creates a new string from two existing strings, and both originals remain valid. Rust's `+` is different — it takes ownership of the left operand and borrows the right. This is efficient (it can reuse the left string's buffer) but surprising.

For readability, prefer `format!()` — it is the Rust equivalent of f-strings and the most idiomatic way to combine strings:
```python
# Python
result = f"{first}, {second}!"

# Rust
let result = format!("{}, {}!", first, second);
```
:::

## String slicing and indexing

Here is a critical difference: you **cannot index a Rust string by position**.

```rust
fn main() {
    let hello = String::from("Hello");

    // This does NOT work:
    // let h = hello[0];  // ERROR: String cannot be indexed by integer

    // Use .chars() to iterate over characters
    let first_char = hello.chars().next();
    println!("{:?}", first_char);  // Some('H')

    // Use byte slicing (careful — must be at character boundaries)
    let slice = &hello[0..5];  // "Hello" — works because all chars are 1 byte
    println!("{}", slice);

    // Iterating over characters
    for c in hello.chars() {
        print!("{} ", c);
    }
    println!();  // H e l l o

    // Getting the nth character
    let third = hello.chars().nth(2);
    println!("{:?}", third);  // Some('l')
}
```

Why no indexing? Because Rust strings are UTF-8, and characters can be 1 to 4 bytes. The string `"cafe"` has different byte offsets than `"caf\u{00e9}"` even though they look similar. Indexing by byte position could land in the middle of a multi-byte character.

::: python Coming from Python
In Python, `"hello"[0]` gives you `"h"` and it seems simple. But Python strings are stored as Unicode code points, so indexing is O(1). Rust strings are stored as UTF-8 bytes, so finding the nth character requires scanning from the beginning — it would be O(n). Rust refuses to hide this cost behind an innocent-looking `[0]` syntax.

If you need O(1) indexing, convert to a `Vec<char>` first:
```rust
let chars: Vec<char> = "hello".chars().collect();
let first = chars[0];  // 'h' — O(1) access
```
This is like Python's `list("hello")`.
:::

## Common string methods

Here is a reference of Python string methods and their Rust equivalents:

```rust
fn main() {
    let text = String::from("  Hello, World!  ");

    // Trimming (Python: strip/lstrip/rstrip)
    let trimmed = text.trim();                        // "Hello, World!"
    let left_trimmed = text.trim_start();              // "Hello, World!  "
    let right_trimmed = text.trim_end();               // "  Hello, World!"

    // Case conversion
    let upper = "hello".to_uppercase();                // "HELLO"
    let lower = "HELLO".to_lowercase();                // "hello"

    // Checking content
    let starts = "Hello".starts_with("He");            // true
    let ends = "Hello".ends_with("lo");                // true
    let has = "Hello World".contains("World");         // true
    let empty = "".is_empty();                         // true

    // Splitting (Python: split)
    let parts: Vec<&str> = "a,b,c".split(',').collect();  // ["a", "b", "c"]
    let words: Vec<&str> = "hello world".split_whitespace().collect();

    // Replacing (Python: replace)
    let replaced = "hello world".replace("world", "Rust");  // "hello Rust"

    // Length
    let byte_len = "hello".len();          // 5 (byte length!)
    let char_count = "hello".chars().count();  // 5 (character count)

    println!("{} | {} | {} | {} | {} | {} | {} | {:?} | {:?} | {} | {} | {}",
        trimmed, left_trimmed, right_trimmed, upper, lower,
        starts, ends, parts, words, replaced, byte_len, char_count);
    println!("{} {}", has, empty);
}
```

::: python Coming from Python
Most operations have direct equivalents:

| Python | Rust | Note |
|--------|------|------|
| `s.strip()` | `s.trim()` | Returns `&str` |
| `s.upper()` | `s.to_uppercase()` | Returns new `String` |
| `s.startswith("x")` | `s.starts_with("x")` | Returns `bool` |
| `s.split(",")` | `s.split(',').collect::<Vec<_>>()` | Lazy iterator, must collect |
| `s.replace("a", "b")` | `s.replace("a", "b")` | Same name! |
| `len(s)` | `s.len()` | **Bytes**, not chars |
| `len(s)` (char count) | `s.chars().count()` | Character count |

The biggest gotcha: `.len()` in Rust returns byte length, not character count. For ASCII strings they are the same, but for emoji or accented characters they differ.
:::

## When to use `String` vs `&str`

Here is a practical guide:

```rust
// Function parameters: prefer &str — accepts both String and &str
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

// Struct fields that own data: use String
struct Message {
    role: String,
    content: String,
}

// Returning new string data: return String
fn build_prompt(messages: &[Message]) -> String {
    let mut prompt = String::new();
    for msg in messages {
        prompt.push_str(&format!("[{}]: {}\n", msg.role, msg.content));
    }
    prompt
}

fn main() {
    // Works with &str (string literal)
    println!("{}", greet("World"));

    // Works with &String (reference to owned String)
    let name = String::from("Agent");
    println!("{}", greet(&name));
}
```

The rule of thumb:
- **Function parameters**: `&str` (accepts both `String` and string literals)
- **Struct fields**: `String` (the struct needs to own the data)
- **Return values**: `String` (the caller needs owned data)
- **Constants**: `&str` or `&'static str` (compile-time string data)

## Key Takeaways

- Rust has two main string types: `String` (owned, growable, heap-allocated) and `&str` (borrowed, read-only view) — use `&str` for function parameters and `String` for owned data
- You cannot index strings by position because Rust strings are UTF-8 and characters can be multiple bytes — use `.chars()` to iterate over characters
- `.len()` returns byte length, not character count — use `.chars().count()` for character count
- `format!()` is the idiomatic way to build strings in Rust, equivalent to Python's f-strings
- Most Python string methods have direct Rust equivalents: `trim`, `to_uppercase`, `starts_with`, `split`, `replace`, and `contains` all work similarly
