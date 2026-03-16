---
title: Iterators vs Comprehensions
description: Rust's lazy iterator chains as the powerful equivalent of Python's list comprehensions, generator expressions, and itertools.
---

# Iterators vs Comprehensions

> **What you'll learn:**
> - How Rust's iterator adaptors (map, filter, fold) replace Python's list comprehensions and generator expressions
> - Why Rust iterators are lazy by default and how collect() triggers evaluation into a concrete collection
> - How to chain iterator operations for expressive, zero-cost data transformations

If Python list comprehensions are your favorite feature, you will love Rust iterators. They are more powerful, always lazy (like Python generators), and compile down to code that is as fast as hand-written loops. Iterator chains are the idiomatic way to process data in Rust, and mastering them makes your code both readable and performant.

## Python comprehensions vs Rust iterators

Let's start with a direct comparison:

**Python — list comprehension:**

```python
numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]

# Filter even numbers and square them
result = [x ** 2 for x in numbers if x % 2 == 0]
print(result)  # [4, 16, 36, 64, 100]
```

**Rust — iterator chain:**

```rust
fn main() {
    let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

    let result: Vec<i32> = numbers
        .iter()
        .filter(|&&x| x % 2 == 0)
        .map(|&x| x * x)
        .collect();

    println!("{:?}", result);  // [4, 16, 36, 64, 100]
}
```

The Rust version is slightly more verbose, but each step is a clear, named operation. Let's break down the pieces.

::: python Coming from Python
The mapping between Python comprehension syntax and Rust iterator methods:

| Python comprehension | Rust iterator chain |
|---------------------|---------------------|
| `[expr for x in items]` | `items.iter().map(\|x\| expr).collect()` |
| `[x for x in items if cond]` | `items.iter().filter(\|x\| cond).collect()` |
| `[expr for x in items if cond]` | `items.iter().filter(\|x\| cond).map(\|x\| expr).collect()` |
| `sum(items)` | `items.iter().sum()` |
| `any(cond for x in items)` | `items.iter().any(\|x\| cond)` |
| `all(cond for x in items)` | `items.iter().all(\|x\| cond)` |

Notice that every Rust chain ends with `.collect()` or a terminal operation like `.sum()`. Without that final step, nothing happens — the iterator is lazy.
:::

## Laziness — iterators do nothing until consumed

This is a crucial difference. Python list comprehensions are *eager* — they evaluate immediately and produce a full list. Rust iterators are *lazy* — they do nothing until you call a consuming method like `.collect()`, `.sum()`, `.count()`, or `.for_each()`.

```rust
fn main() {
    let numbers = vec![1, 2, 3, 4, 5];

    // This creates an iterator but does NOT process anything yet
    let iterator = numbers.iter().map(|&x| {
        println!("Processing {}", x);  // this won't print yet!
        x * 2
    });

    println!("Iterator created, nothing processed yet");

    // NOW it processes — collect() drives the iterator
    let doubled: Vec<i32> = iterator.collect();
    println!("Result: {:?}", doubled);
}
```

Output:
```
Iterator created, nothing processed yet
Processing 1
Processing 2
Processing 3
Processing 4
Processing 5
Result: [2, 4, 6, 8, 10]
```

::: python Coming from Python
Rust iterators are like Python generator expressions, not list comprehensions:
```python
# Eager — processes immediately, builds full list
eager = [x * 2 for x in numbers]

# Lazy — nothing happens until you iterate
lazy = (x * 2 for x in numbers)  # generator expression
result = list(lazy)  # NOW it processes — like Rust's .collect()
```
The advantage of laziness: if you chain many operations, Rust processes each element through the entire chain in a single pass. No intermediate lists are allocated.
:::

## Core iterator methods

### map — transform each element

```rust
fn main() {
    let names = vec!["alice", "bob", "charlie"];

    let uppercased: Vec<String> = names
        .iter()
        .map(|name| name.to_uppercase())
        .collect();

    println!("{:?}", uppercased);  // ["ALICE", "BOB", "CHARLIE"]
}
```

### filter — keep elements that match a condition

```rust
fn main() {
    let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

    let evens: Vec<&i32> = numbers
        .iter()
        .filter(|&&n| n % 2 == 0)
        .collect();

    println!("{:?}", evens);  // [2, 4, 6, 8, 10]
}
```

### filter_map — filter and transform in one step

```rust
fn main() {
    let strings = vec!["42", "not_a_number", "7", "oops", "13"];

    // Parse strings to numbers, keeping only successful parses
    let numbers: Vec<i32> = strings
        .iter()
        .filter_map(|s| s.parse::<i32>().ok())
        .collect();

    println!("{:?}", numbers);  // [42, 7, 13]
}
```

::: python Coming from Python
`filter_map` combines Python's pattern of:
```python
numbers = [int(s) for s in strings if s.isdigit()]
```
But it is more flexible because it uses `Option` — `filter_map` keeps `Some` values and discards `None` values.
:::

### fold — accumulate into a single value

```rust
fn main() {
    let numbers = vec![1, 2, 3, 4, 5];

    // Sum using fold (like Python's functools.reduce)
    let total = numbers.iter().fold(0, |acc, &x| acc + x);
    println!("Total: {}", total);  // 15

    // Build a string
    let csv = numbers.iter().fold(String::new(), |acc, &x| {
        if acc.is_empty() {
            x.to_string()
        } else {
            format!("{},{}", acc, x)
        }
    });
    println!("CSV: {}", csv);  // "1,2,3,4,5"

    // But for summing, just use .sum()
    let total: i32 = numbers.iter().sum();
    println!("Sum: {}", total);
}
```

### enumerate — get index and value

```rust
fn main() {
    let tools = vec!["shell", "read_file", "write_file"];

    // Like Python's enumerate()
    for (index, tool) in tools.iter().enumerate() {
        println!("{}. {}", index + 1, tool);
    }
}
```

### zip — combine two iterators

```rust
fn main() {
    let names = vec!["Alice", "Bob", "Charlie"];
    let scores = vec![95, 87, 92];

    // Like Python's zip()
    let results: Vec<(&str, &i32)> = names.iter().copied().zip(scores.iter()).collect();
    println!("{:?}", results);  // [("Alice", 95), ("Bob", 87), ("Charlie", 92)]

    // Zip and process
    for (name, score) in names.iter().zip(scores.iter()) {
        println!("{}: {}", name, score);
    }
}
```

## Chaining operations

The real power comes from chaining multiple operations. Each step transforms the iterator, and the whole chain executes in a single pass:

```rust
#[derive(Debug)]
struct ToolResult {
    name: String,
    success: bool,
    duration_ms: u64,
}

fn main() {
    let results = vec![
        ToolResult { name: String::from("shell"), success: true, duration_ms: 150 },
        ToolResult { name: String::from("read_file"), success: false, duration_ms: 5 },
        ToolResult { name: String::from("write_file"), success: true, duration_ms: 23 },
        ToolResult { name: String::from("shell"), success: true, duration_ms: 3200 },
        ToolResult { name: String::from("read_file"), success: true, duration_ms: 8 },
    ];

    // Find successful results that took over 100ms, get their names
    let slow_successes: Vec<&str> = results
        .iter()
        .filter(|r| r.success)
        .filter(|r| r.duration_ms > 100)
        .map(|r| r.name.as_str())
        .collect();

    println!("Slow successes: {:?}", slow_successes);  // ["shell", "shell"]

    // Total duration of all operations
    let total_ms: u64 = results.iter().map(|r| r.duration_ms).sum();
    println!("Total time: {}ms", total_ms);  // 3386ms

    // Count failures
    let failure_count = results.iter().filter(|r| !r.success).count();
    println!("Failures: {}", failure_count);  // 1

    // Average duration
    let avg = total_ms as f64 / results.len() as f64;
    println!("Average: {:.1}ms", avg);
}
```

::: python Coming from Python
The equivalent Python would be:
```python
slow_successes = [r.name for r in results if r.success and r.duration_ms > 100]
total_ms = sum(r.duration_ms for r in results)
failure_count = sum(1 for r in results if not r.success)
```
Python comprehensions are more concise for simple cases. Rust iterator chains are more readable for complex multi-step transformations because each operation is named and on its own line. For the coding agent, we will frequently use iterator chains to process tool results, messages, and API responses.
:::

## Collecting into different types

`.collect()` is generic — it can produce different collection types based on the type annotation:

```rust
use std::collections::{HashMap, HashSet};

fn main() {
    let words = vec!["apple", "banana", "cherry", "apple", "banana"];

    // Collect into Vec
    let word_list: Vec<&str> = words.iter().copied().collect();
    println!("List: {:?}", word_list);

    // Collect into HashSet (deduplicates)
    let unique_words: HashSet<&str> = words.iter().copied().collect();
    println!("Unique: {:?}", unique_words);

    // Collect into HashMap
    let word_lengths: HashMap<&str, usize> = words
        .iter()
        .copied()
        .map(|w| (w, w.len()))
        .collect();
    println!("Lengths: {:?}", word_lengths);

    // Collect into String
    let joined: String = vec!["Hello", " ", "World"].into_iter().collect();
    println!("{}", joined);
}
```

## The three types of iteration

Rust provides three ways to iterate over a collection, controlling ownership:

```rust
fn main() {
    let names = vec![
        String::from("Alice"),
        String::from("Bob"),
        String::from("Charlie"),
    ];

    // &names or names.iter() — borrows each element (&String)
    for name in &names {
        println!("Borrowed: {}", name);
    }
    // names is still valid here

    // &mut names or names.iter_mut() — mutably borrows each element
    let mut scores = vec![85, 92, 78];
    for score in &mut scores {
        *score += 5;  // add 5 to each score
    }
    println!("Adjusted: {:?}", scores);

    // names or names.into_iter() — takes ownership of each element
    for name in names {
        println!("Owned: {}", name);
    }
    // names is no longer valid — it was consumed
}
```

::: python Coming from Python
Python only has one way to iterate: `for item in collection`. The collection is never consumed. In Rust, `for item in collection` *consumes* the collection (takes ownership). Use `for item in &collection` to borrow, which is the equivalent of Python's behavior.
:::

## Key Takeaways

- Rust iterator chains replace Python list comprehensions with named operations like `.map()`, `.filter()`, `.fold()`, and `.collect()`
- Iterators are lazy by default — nothing executes until a consuming method like `.collect()`, `.sum()`, or `.for_each()` is called
- `.collect()` is generic and can produce `Vec`, `HashMap`, `HashSet`, or `String` depending on the type annotation
- Iterator chains compile to code as fast as hand-written loops thanks to Rust's optimizer — there is zero overhead for the abstraction
- Use `&collection` for borrowing iteration (Python-like), `&mut collection` for mutation, and `collection` for consuming iteration
