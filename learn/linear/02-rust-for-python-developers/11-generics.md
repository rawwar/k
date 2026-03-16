---
title: Generics
description: Writing type-parameterized code in Rust with generics and trait bounds, compared to Python's type variables and generic types.
---

# Generics

> **What you'll learn:**
> - How to write functions, structs, and enums that are generic over types using angle bracket syntax
> - How trait bounds constrain generic types, ensuring the compiler knows what operations are available
> - How Rust's monomorphization compares to Python's duck typing and runtime generic behavior

Generics let you write code that works with many different types without sacrificing type safety. If you have used Python's `typing.TypeVar` or `list[T]` annotations, you have seen this idea — but in Python, generics are hints for the type checker. In Rust, generics are a core language feature that the compiler uses to generate specialized, efficient code.

## The problem generics solve

Without generics, you would need separate functions for each type:

```rust
fn largest_i32(list: &[i32]) -> &i32 {
    let mut largest = &list[0];
    for item in list {
        if item > largest {
            largest = item;
        }
    }
    largest
}

fn largest_f64(list: &[f64]) -> &f64 {
    let mut largest = &list[0];
    for item in list {
        if item > largest {
            largest = item;
        }
    }
    largest
}
```

These two functions have identical logic — only the types differ. Generics let you write it once:

```rust
fn largest<T: PartialOrd>(list: &[T]) -> &T {
    let mut largest = &list[0];
    for item in list {
        if item > largest {
            largest = item;
        }
    }
    largest
}

fn main() {
    let numbers = vec![34, 50, 25, 100, 65];
    println!("Largest: {}", largest(&numbers));

    let floats = vec![1.5, 3.7, 2.1, 4.8];
    println!("Largest: {}", largest(&floats));
}
```

The `<T: PartialOrd>` says: "this function works with any type `T` that can be compared with `>` (implements the `PartialOrd` trait)."

::: python Coming from Python
In Python, you would just write one function and rely on duck typing:
```python
def largest(items):
    return max(items)
```
This works because Python resolves `>` at runtime for whatever type happens to be in the list. If you pass a list of objects that do not support comparison, you get a `TypeError` at runtime.

Rust's generics give you the same "write once" benefit, but with compile-time type checking. If you try to call `largest` with a type that does not implement `PartialOrd`, the compiler rejects it before the code runs.
:::

## Generic functions

The basic syntax uses angle brackets after the function name:

```rust
fn first_element<T>(items: &[T]) -> Option<&T> {
    items.first()
}

fn repeat_value<T: Clone>(value: &T, count: usize) -> Vec<T> {
    let mut result = Vec::new();
    for _ in 0..count {
        result.push(value.clone());
    }
    result
}

fn main() {
    let nums = vec![10, 20, 30];
    let first = first_element(&nums);
    println!("{:?}", first);  // Some(10)

    let words = vec!["hello", "world"];
    let first = first_element(&words);
    println!("{:?}", first);  // Some("hello")

    let repeated = repeat_value(&String::from("agent"), 3);
    println!("{:?}", repeated);  // ["agent", "agent", "agent"]
}
```

## Trait bounds — constraining generic types

Without a trait bound, you can only do things that work with *all* types (which is almost nothing). Trait bounds tell the compiler what capabilities `T` must have:

```rust
use std::fmt::Display;

// T must implement Display (can be converted to a string with {})
fn print_labeled<T: Display>(label: &str, value: T) {
    println!("{}: {}", label, value);
}

// Multiple bounds with +
fn print_debug_and_display<T: Display + std::fmt::Debug>(value: T) {
    println!("Display: {}", value);
    println!("Debug: {:?}", value);
}

// where clause for cleaner syntax with many bounds
fn process<T>(item: T) -> String
where
    T: Display + Clone + PartialEq,
{
    let cloned = item.clone();
    if item == cloned {
        format!("Identical: {}", item)
    } else {
        format!("Different")
    }
}

fn main() {
    print_labeled("count", 42);
    print_labeled("name", "Alice");
    println!("{}", process(String::from("hello")));
}
```

::: python Coming from Python
Trait bounds are like Python's `typing.Protocol` constraints, but enforced at compile time:
```python
from typing import TypeVar, Protocol

class Displayable(Protocol):
    def __str__(self) -> str: ...

T = TypeVar('T', bound=Displayable)

def print_labeled(label: str, value: T) -> None:
    print(f"{label}: {value}")
```
In Python, `bound=Displayable` is a hint for mypy. In Rust, `T: Display` is a hard constraint — the code will not compile if `T` does not implement `Display`. The guarantee is absolute.
:::

## Generic structs

Structs can be parameterized over types just like functions:

```rust
#[derive(Debug)]
struct Pair<T> {
    first: T,
    second: T,
}

impl<T> Pair<T> {
    fn new(first: T, second: T) -> Self {
        Pair { first, second }
    }
}

impl<T: PartialOrd + std::fmt::Display> Pair<T> {
    fn larger(&self) -> &T {
        if self.first >= self.second {
            &self.first
        } else {
            &self.second
        }
    }
}

fn main() {
    let pair = Pair::new(10, 20);
    println!("Larger: {}", pair.larger());  // 20

    let pair = Pair::new(String::from("alpha"), String::from("beta"));
    println!("Larger: {}", pair.larger());  // "beta"
}
```

Notice that `larger` is only available when `T` implements `PartialOrd + Display`. You can have different `impl` blocks with different trait bounds, providing different methods depending on the capabilities of `T`.

## Generics with multiple type parameters

You can use multiple type parameters when the types differ:

```rust
#[derive(Debug)]
struct KeyValue<K, V> {
    key: K,
    value: V,
}

impl<K: std::fmt::Display, V: std::fmt::Display> KeyValue<K, V> {
    fn display(&self) -> String {
        format!("{} => {}", self.key, self.value)
    }
}

fn main() {
    let item = KeyValue {
        key: String::from("model"),
        value: String::from("claude-3"),
    };
    println!("{}", item.display());

    let item = KeyValue {
        key: 1,
        value: 99.5,
    };
    println!("{}", item.display());
}
```

::: python Coming from Python
This maps to Python's generic classes with `TypeVar`:
```python
from typing import TypeVar, Generic

K = TypeVar('K')
V = TypeVar('V')

class KeyValue(Generic[K, V]):
    def __init__(self, key: K, value: V):
        self.key = key
        self.value = value
```
In Python, `KeyValue[str, int]` is a type hint. In Rust, `KeyValue<String, i32>` is a concrete type — the compiler generates specialized code for each combination of types used.
:::

## Monomorphization — zero-cost generics

Rust's generics have *zero runtime cost* thanks to monomorphization. When you write:

```rust
fn add<T: std::ops::Add<Output = T>>(a: T, b: T) -> T {
    a + b
}

fn main() {
    let x = add(1_i32, 2_i32);
    let y = add(1.0_f64, 2.0_f64);
    println!("{} {}", x, y);
}
```

The compiler generates two specialized functions at compile time:

```rust
// What the compiler actually produces (conceptually):
fn add_i32(a: i32, b: i32) -> i32 { a + b }
fn add_f64(a: f64, b: f64) -> f64 { a + b }
```

There is no runtime dispatch, no boxing, no vtable lookup. The generated code is as fast as if you had written specialized functions by hand.

::: python Coming from Python
In Python, generics are purely a type-checking concept — at runtime, a `list[int]` and a `list[str]` are both just `list`. There is no specialization. Every operation goes through Python's dynamic dispatch (attribute lookups, dunder methods).

Rust's monomorphization means generic code runs at the same speed as hand-specialized code. This is what "zero-cost abstractions" means — you get the *abstraction* (write once, works with many types) without any *cost* (no runtime overhead).
:::

## Common generic patterns in the standard library

You are already using generics every time you use Rust's standard library:

```rust
fn main() {
    // Vec<T> — a generic growable array
    let numbers: Vec<i32> = vec![1, 2, 3];
    let names: Vec<String> = vec![String::from("Alice"), String::from("Bob")];

    // Option<T> — a generic "maybe a value"
    let some_number: Option<i32> = Some(42);
    let no_number: Option<i32> = None;

    // Result<T, E> — a generic "success or error"
    let ok: Result<i32, String> = Ok(42);
    let err: Result<i32, String> = Err(String::from("oops"));

    // HashMap<K, V> — a generic key-value store
    use std::collections::HashMap;
    let mut scores: HashMap<String, i32> = HashMap::new();
    scores.insert(String::from("Alice"), 100);

    println!("{:?} {:?} {:?} {:?} {:?}", numbers, names, some_number, no_number, ok);
    println!("{:?} {:?}", err, scores);
}
```

Every container and utility type in Rust's standard library is generic. You have been using generics since the first `Vec` you created.

## The `impl Trait` shorthand

For function parameters and return types, `impl Trait` is a convenient shorthand for generics:

```rust
use std::fmt::Display;

// These two function signatures are equivalent:
fn print_it_generic<T: Display>(item: T) {
    println!("{}", item);
}

fn print_it_impl(item: impl Display) {
    println!("{}", item);
}

// impl Trait in return position means "returns some type that implements Display"
fn make_greeting(name: &str) -> impl Display {
    format!("Hello, {}!", name)
}

fn main() {
    print_it_generic(42);
    print_it_impl("hello");
    println!("{}", make_greeting("Agent"));
}
```

The `impl Trait` syntax is preferred when you do not need to name the type parameter (such as when the function only uses it once).

## Key Takeaways

- Generics let you write functions and structs that work with any type, similar to Python's duck typing but with compile-time type safety
- Trait bounds (`T: Display + Clone`) constrain what types are accepted, ensuring the compiler knows exactly what operations are available on `T`
- Monomorphization generates specialized code for each concrete type, giving generics zero runtime cost — as fast as hand-written specialized code
- `impl Trait` is a convenient shorthand for simple generic parameters and return types
- The entire Rust standard library is built on generics — `Vec<T>`, `Option<T>`, `Result<T, E>`, and `HashMap<K, V>` are all generic types you use daily
