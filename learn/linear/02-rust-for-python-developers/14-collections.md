---
title: Collections
description: Rust's standard collections — Vec, HashMap, HashSet, and BTreeMap — mapped to Python's list, dict, and set.
---

# Collections

> **What you'll learn:**
> - How `Vec<T>` maps to Python's list and the key differences in memory layout and performance
> - How HashMap and BTreeMap compare to Python's dict and when to choose each
> - How to use iterators with collections for functional-style data processing in Rust

Collections store multiple values and are a central part of every program. Python developers reach for `list`, `dict`, and `set` constantly. Rust has direct equivalents — `Vec`, `HashMap`, and `HashSet` — with important differences in how they manage memory and enforce types.

## `Vec<T>` — Rust's list

`Vec<T>` is a growable array, the most commonly used collection in Rust. It is the direct equivalent of Python's `list`, but with a key constraint: every element must be the *same type*.

```rust
fn main() {
    // Creating vectors
    let numbers: Vec<i32> = Vec::new();       // empty, type annotated
    let numbers = vec![1, 2, 3, 4, 5];        // vec! macro (like Python's [1, 2, 3])
    let zeros = vec![0; 10];                   // 10 zeros (like Python's [0] * 10)

    // Adding elements
    let mut fruits = Vec::new();
    fruits.push(String::from("apple"));
    fruits.push(String::from("banana"));
    fruits.push(String::from("cherry"));

    // Accessing elements
    let first = &fruits[0];                    // panics if index out of bounds
    let maybe_first = fruits.get(0);           // returns Option<&T> — safe
    let maybe_tenth = fruits.get(10);          // None — no panic

    println!("First: {}", first);
    println!("Safe first: {:?}", maybe_first);   // Some("apple")
    println!("Safe tenth: {:?}", maybe_tenth);   // None

    // Length
    println!("Count: {}", fruits.len());
    println!("Empty? {}", fruits.is_empty());
    println!("{:?} {:?}", numbers, zeros);
}
```

::: python Coming from Python
Python lists can hold mixed types: `[1, "hello", True, [1, 2]]`. Rust's `Vec<T>` holds a single type — `Vec<i32>` can only hold `i32` values. This might feel restrictive, but it means the compiler knows the exact memory layout and can optimize accordingly.

If you genuinely need mixed types, use an enum:
```rust
enum Value {
    Int(i32),
    Text(String),
    Bool(bool),
    List(Vec<Value>),
}

let mixed: Vec<Value> = vec![
    Value::Int(1),
    Value::Text(String::from("hello")),
    Value::Bool(true),
];
```
This is more explicit than Python's dynamic typing, and the compiler ensures you handle all variants when accessing elements.
:::

### Common Vec operations

```rust
fn main() {
    let mut items = vec![3, 1, 4, 1, 5, 9, 2, 6];

    // Sorting (Python: items.sort())
    items.sort();
    println!("Sorted: {:?}", items);  // [1, 1, 2, 3, 4, 5, 6, 9]

    // Reversing (Python: items.reverse())
    items.reverse();
    println!("Reversed: {:?}", items);

    // Checking membership (Python: 5 in items)
    let has_five = items.contains(&5);
    println!("Has 5? {}", has_five);

    // Removing by index (Python: items.pop(2))
    let removed = items.remove(2);  // removes and returns element at index 2
    println!("Removed: {}", removed);

    // Removing last (Python: items.pop())
    let last = items.pop();  // returns Option<i32>
    println!("Last: {:?}", last);

    // Extending (Python: items.extend([10, 11]))
    items.extend([10, 11]);
    println!("Extended: {:?}", items);

    // Slicing (Python: items[1:3])
    let slice = &items[1..3];
    println!("Slice: {:?}", slice);

    // Iterating (Python: for item in items)
    for item in &items {
        print!("{} ", item);
    }
    println!();
}
```

## `HashMap<K, V>` — Rust's dict

`HashMap` stores key-value pairs, just like Python's `dict`:

```rust
use std::collections::HashMap;

fn main() {
    // Creating a HashMap
    let mut scores: HashMap<String, i32> = HashMap::new();

    // Inserting (Python: scores["Alice"] = 100)
    scores.insert(String::from("Alice"), 100);
    scores.insert(String::from("Bob"), 85);
    scores.insert(String::from("Charlie"), 92);

    // Accessing (Python: scores["Alice"])
    let alice_score = scores.get("Alice");  // Returns Option<&i32>
    println!("Alice: {:?}", alice_score);    // Some(100)

    // Safe access with default (Python: scores.get("Unknown", 0))
    let unknown = scores.get("Unknown").copied().unwrap_or(0);
    println!("Unknown: {}", unknown);  // 0

    // Checking membership (Python: "Alice" in scores)
    let has_alice = scores.contains_key("Alice");
    println!("Has Alice? {}", has_alice);

    // Iterating (Python: for name, score in scores.items())
    for (name, score) in &scores {
        println!("{}: {}", name, score);
    }

    // Length
    println!("Count: {}", scores.len());
}
```

::: python Coming from Python
The API is very similar to Python's `dict`:

| Python | Rust |
|--------|------|
| `d = {}` | `let mut d = HashMap::new();` |
| `d["key"] = value` | `d.insert(key, value);` |
| `d["key"]` | `d[&key]` (panics) or `d.get(&key)` (returns Option) |
| `d.get("key", default)` | `d.get("key").unwrap_or(&default)` |
| `"key" in d` | `d.contains_key("key")` |
| `for k, v in d.items()` | `for (k, v) in &d` |
| `del d["key"]` | `d.remove("key");` |
| `len(d)` | `d.len()` |

One important difference: Rust's HashMap requires `use std::collections::HashMap;` — it is not in the prelude like `Vec`. Also, HashMap keys must implement `Eq` and `Hash` traits.
:::

### The entry API — update or insert

Rust's HashMap has an elegant `entry` API for "update if exists, insert if not" patterns:

```rust
use std::collections::HashMap;

fn main() {
    let mut word_counts: HashMap<String, i32> = HashMap::new();

    let text = "the cat sat on the mat the cat";
    for word in text.split_whitespace() {
        let count = word_counts.entry(word.to_string()).or_insert(0);
        *count += 1;
    }

    println!("{:?}", word_counts);
    // {"the": 3, "cat": 2, "sat": 1, "on": 1, "mat": 1}
}
```

::: python Coming from Python
This replaces Python's `collections.defaultdict` or the `dict.setdefault()` pattern:
```python
from collections import defaultdict
word_counts = defaultdict(int)
for word in text.split():
    word_counts[word] += 1
```
Rust's `entry()` API is more explicit but also more flexible — `or_insert_with()` takes a closure for lazy initialization.
:::

## `HashSet<T>` — Rust's set

`HashSet` stores unique values, like Python's `set`:

```rust
use std::collections::HashSet;

fn main() {
    let mut languages: HashSet<String> = HashSet::new();
    languages.insert(String::from("Rust"));
    languages.insert(String::from("Python"));
    languages.insert(String::from("Rust"));  // duplicate — ignored

    println!("Count: {}", languages.len());  // 2
    println!("Has Rust? {}", languages.contains("Rust"));

    // From a vector
    let numbers: HashSet<i32> = vec![1, 2, 3, 2, 1].into_iter().collect();
    println!("Unique: {:?}", numbers);  // {1, 2, 3}

    // Set operations
    let a: HashSet<i32> = vec![1, 2, 3, 4].into_iter().collect();
    let b: HashSet<i32> = vec![3, 4, 5, 6].into_iter().collect();

    let union: HashSet<_> = a.union(&b).copied().collect();
    let intersection: HashSet<_> = a.intersection(&b).copied().collect();
    let difference: HashSet<_> = a.difference(&b).copied().collect();

    println!("Union: {:?}", union);         // {1, 2, 3, 4, 5, 6}
    println!("Intersection: {:?}", intersection);  // {3, 4}
    println!("Difference: {:?}", difference);      // {1, 2}
}
```

::: python Coming from Python
The set operations map almost exactly:

| Python | Rust |
|--------|------|
| `s = set()` | `let mut s = HashSet::new();` |
| `s.add(x)` | `s.insert(x);` |
| `x in s` | `s.contains(&x)` |
| `a \| b` or `a.union(b)` | `a.union(&b).collect()` |
| `a & b` | `a.intersection(&b).collect()` |
| `a - b` | `a.difference(&b).collect()` |
| `set([1,2,3,2,1])` | `vec![1,2,3,2,1].into_iter().collect()` |
:::

## BTreeMap and BTreeSet — sorted collections

When you need sorted keys, use `BTreeMap` instead of `HashMap`:

```rust
use std::collections::BTreeMap;

fn main() {
    let mut scores = BTreeMap::new();
    scores.insert("Charlie", 92);
    scores.insert("Alice", 100);
    scores.insert("Bob", 85);

    // Iteration is in sorted key order (unlike HashMap)
    for (name, score) in &scores {
        println!("{}: {}", name, score);
    }
    // Alice: 100
    // Bob: 85
    // Charlie: 92

    // Range queries
    for (name, score) in scores.range("A".."C") {
        println!("In range: {} = {}", name, score);
    }
    // Alice: 100, Bob: 85
}
```

::: python Coming from Python
Python's `dict` preserves insertion order (since Python 3.7). Rust's `HashMap` does *not* guarantee order. If you need ordered keys, use `BTreeMap` (sorted by key) or consider the `indexmap` crate (preserves insertion order like Python's dict).
:::

## Choosing the right collection

| Need | Python | Rust |
|------|--------|------|
| Ordered sequence | `list` | `Vec<T>` |
| Key-value pairs | `dict` | `HashMap<K, V>` |
| Sorted key-value pairs | `dict` (insertion order) | `BTreeMap<K, V>` |
| Unique values | `set` | `HashSet<T>` |
| Sorted unique values | `sorted(set)` | `BTreeSet<T>` |
| Queue (FIFO) | `collections.deque` | `VecDeque<T>` |
| Fixed-size array | `tuple` | `[T; N]` (array) |

## Key Takeaways

- `Vec<T>` is Rust's list — same-type elements only, but you can use enums for mixed types
- `HashMap<K, V>` is Rust's dict — use `.get()` for safe access (returns `Option`) and the `entry()` API for update-or-insert patterns
- `HashSet<T>` is Rust's set — supports union, intersection, and difference operations with the same semantics as Python
- `BTreeMap` and `BTreeSet` provide sorted ordering — use them when iteration order matters (Rust's `HashMap` does not preserve any order)
- All Rust collections are generic and type-safe — the compiler knows the exact type of every element, enabling optimizations that Python's dynamic collections cannot achieve
