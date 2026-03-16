---
title: Borrowing and References
description: How Rust lets you temporarily access data without taking ownership, using shared and mutable references enforced at compile time.
---

# Borrowing and References

> **What you'll learn:**
> - The difference between shared references (&T) and mutable references (&mut T)
> - Why Rust enforces the rule of either one mutable reference or many shared references, but never both
> - How borrowing eliminates data races and use-after-free bugs that plague other languages

In the previous section, you learned that passing a value to a function *moves* ownership, making the original variable invalid. That is safe, but it is also annoying — you would have to clone everything just to let a function look at your data. Borrowing solves this problem by letting you *lend* data to a function without giving up ownership.

## The problem borrowing solves

Without borrowing, reading data is frustratingly awkward:

```rust
fn calculate_length(s: String) -> (String, usize) {
    let len = s.len();
    (s, len)  // have to return the String to give ownership back!
}

fn main() {
    let greeting = String::from("Hello, agent!");
    let (greeting, length) = calculate_length(greeting);
    println!("'{}' has {} characters", greeting, length);
}
```

You had to return the `String` alongside the result just to keep using it. This does not scale. Borrowing lets you pass a *reference* — a pointer to the data — without transferring ownership.

## Shared references with `&`

A shared reference lets you *read* data without owning it:

```rust
fn calculate_length(s: &String) -> usize {
    s.len()
}  // s goes out of scope, but since it's just a reference, nothing is dropped

fn main() {
    let greeting = String::from("Hello, agent!");
    let length = calculate_length(&greeting);  // pass a reference
    println!("'{}' has {} characters", greeting, length);  // greeting is still valid!
}
```

The `&` symbol creates a reference. `&greeting` means "borrow greeting" — the function can read the data, but `main` retains ownership. When `calculate_length` returns, the borrow ends and `main` can continue using `greeting` freely.

::: python Coming from Python
In Python, every variable is already a reference to an object on the heap, so you never think about this. Passing a value to a function always passes a reference — the function can read and even mutate the data through that reference. Rust makes this explicit: `&` means "I'm lending you read access" and `&mut` means "I'm lending you write access." This explicitness prevents entire classes of bugs that come from Python's implicit sharing.
:::

## Mutable references with `&mut`

Shared references (`&T`) are read-only. If you want a function to modify data, you need a mutable reference:

```rust
fn add_greeting(message: &mut String) {
    message.push_str(", welcome!");
}

fn main() {
    let mut greeting = String::from("Hello");  // must be declared mut
    add_greeting(&mut greeting);                // pass a mutable reference
    println!("{}", greeting);                   // "Hello, welcome!"
}
```

Notice two things:
1. The variable must be declared `let mut`
2. You pass `&mut greeting`, explicitly granting write access

Both the caller and the function signature agree that mutation will happen. There are no surprises.

## The borrowing rules

Rust enforces two rules at compile time that prevent data races and inconsistent state:

**Rule 1: You can have either one mutable reference OR any number of shared references, but not both at the same time.**

```rust
fn main() {
    let mut data = String::from("hello");

    // Multiple shared references — OK
    let r1 = &data;
    let r2 = &data;
    println!("{} and {}", r1, r2);

    // One mutable reference — OK (shared refs are no longer used above)
    let r3 = &mut data;
    r3.push_str(" world");
    println!("{}", r3);
}
```

```rust
fn main() {
    let mut data = String::from("hello");

    let r1 = &data;          // shared borrow
    let r2 = &mut data;      // ERROR: cannot borrow as mutable while also borrowed as shared
    println!("{}", r1);
}
```

**Rule 2: References must always be valid — no dangling references.**

```rust
// This would not compile — returning a reference to a local variable
// fn dangling() -> &String {
//     let s = String::from("hello");
//     &s  // ERROR: s is dropped when dangling() returns, so the reference would be invalid
// }
```

::: python Coming from Python
These rules prevent a real class of bugs. In Python, consider this scenario:
```python
data = {"key": "value"}
items = data.items()  # a "view" of the dict
data["new_key"] = "new_value"  # mutate while iterating
for k, v in items:  # RuntimeError in some cases, silent bug in others
    print(k, v)
```
Python catches *some* of these at runtime (dictionary changed size during iteration), but many mutation-through-aliasing bugs go undetected. Rust prevents all of them at compile time: if anyone has a shared reference to data, nobody can mutate it until those references are done.
:::

## How Rust tracks borrow lifetimes

Borrows are not permanent — they last only as long as the reference is being used. The compiler tracks this automatically:

```rust
fn main() {
    let mut data = String::from("hello");

    let r1 = &data;           // shared borrow starts
    println!("{}", r1);       // shared borrow ends here (last use of r1)

    let r2 = &mut data;      // mutable borrow starts — OK, no active shared borrows
    r2.push_str(" world");
    println!("{}", r2);       // mutable borrow ends here
}
```

The compiler uses "non-lexical lifetimes" (NLL) — a borrow lasts until its last use, not until the end of the scope. This makes the rules much more ergonomic than they might seem at first.

## Common borrowing patterns

### Pattern 1: Functions that read data

Most functions just need to *look* at data. Use `&T`:

```rust
fn is_long(text: &str) -> bool {
    text.len() > 100
}

fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

fn main() {
    let message = String::from("Hello world from Rust");
    println!("Long? {}", is_long(&message));
    println!("Words: {}", count_words(&message));
    println!("Message: {}", message);  // still valid — we only borrowed
}
```

::: python Coming from Python
Notice that the functions take `&str` (a string slice reference) rather than `&String`. In Rust, `&str` is the preferred type for functions that read strings because it can accept both `&String` and string literals. This is similar to how Python functions accept `str` and you never write a function that specifically requires `"a string created with str()"` — you just accept any string.
:::

### Pattern 2: Functions that modify data

When a function needs to change data, use `&mut T`:

```rust
fn to_uppercase_first(text: &mut String) {
    if let Some(first) = text.chars().next() {
        let upper = first.to_uppercase().to_string();
        text.replace_range(..first.len_utf8(), &upper);
    }
}

fn main() {
    let mut name = String::from("alice");
    to_uppercase_first(&mut name);
    println!("{}", name);  // "Alice"
}
```

### Pattern 3: Iterating with references

When iterating over a collection, you typically borrow its elements:

```rust
fn sum_values(numbers: &[i32]) -> i32 {
    let mut total = 0;
    for n in numbers {
        total += n;
    }
    total
}

fn main() {
    let scores = vec![85, 92, 78, 95, 88];
    let total = sum_values(&scores);
    println!("Total: {}", total);
    println!("Scores: {:?}", scores);  // scores is still valid
}
```

## Borrowing vs Python's reference model

Let's compare the two models directly with a concrete example:

**Python — shared mutable access (source of bugs):**

```python
def process(items):
    items.append("new")   # mutates the caller's list!
    return len(items)

my_list = [1, 2, 3]
count = process(my_list)
print(my_list)  # [1, 2, 3, 'new'] — surprised?
```

**Rust — explicit about mutation:**

```rust
fn process(items: &mut Vec<String>) -> usize {
    items.push(String::from("new"));
    items.len()
}

fn main() {
    let mut my_list = vec!["a".to_string(), "b".to_string()];
    let count = process(&mut my_list);  // explicitly passing mutable access
    println!("{:?} — count: {}", my_list, count);
}
```

In Rust, the `&mut` in both the function signature and the call site make it crystal clear that this function will modify the data. In Python, you have to read the function body to know whether it mutates its arguments.

## A brief note on lifetimes

Sometimes the compiler needs help understanding how long a reference is valid. This is where *lifetime annotations* come in:

```rust
fn longer<'a>(s1: &'a str, s2: &'a str) -> &'a str {
    if s1.len() > s2.len() {
        s1
    } else {
        s2
    }
}

fn main() {
    let s1 = String::from("long string");
    let result;
    {
        let s2 = String::from("short");
        result = longer(&s1, &s2);
        println!("{}", result);
    }
}
```

The `'a` annotations tell the compiler that the returned reference lives as long as the shortest-lived input reference. You will not need to write lifetime annotations often — the compiler infers them in most cases. We will revisit lifetimes when they naturally come up in the agent code. For now, just know they exist to help the compiler verify reference safety.

## Key Takeaways

- Borrowing lets you access data without taking ownership: `&T` for reading, `&mut T` for modifying
- Rust enforces that you can have either one mutable reference *or* any number of shared references at a time — this prevents data races at compile time
- References automatically end at their last use (non-lexical lifetimes), making the rules more ergonomic than they appear
- Prefer `&str` over `&String` in function signatures, and `&[T]` over `&Vec<T>` — these slice types are more flexible and idiomatic
- Lifetime annotations (`'a`) help the compiler verify reference validity in complex cases, but most lifetimes are inferred automatically
