---
title: Variables Ownership vs Python GC
description: The fundamental shift from Python's garbage-collected references to Rust's ownership model where every value has exactly one owner.
---

# Variables Ownership vs Python GC

> **What you'll learn:**
> - How Rust's ownership system replaces garbage collection with compile-time memory management
> - What move semantics mean and why assigning a variable in Rust can invalidate the original
> - How to think about scope and drop as the Rust equivalent of Python's reference counting and GC

This is the section that changes how you think about programming. Ownership is the concept that makes Rust unique, and it is the biggest mental shift you will make coming from Python. Everything in this section exists to answer one question: *who is responsible for freeing this memory, and when?*

In Python, you never ask this question. The garbage collector handles it. In Rust, the compiler answers it at compile time, and the rules are surprisingly simple.

## How Python manages memory

In Python, every value lives on the heap, and the runtime tracks how many variables point to it. When no variables point to a value anymore, the garbage collector frees it.

```python
# Python's memory model
a = [1, 2, 3]   # A list is created on the heap. a points to it. refcount = 1
b = a            # b also points to the same list. refcount = 2
a = None         # a no longer points to the list. refcount = 1
# b still points to the list, so it stays alive
b = None         # refcount = 0. The garbage collector frees the list.
```

This model is invisible and convenient. You create objects, pass them around, and they get cleaned up automatically. The downside is:
- **Non-deterministic cleanup** — you cannot predict exactly when memory is freed
- **GC pauses** — the garbage collector periodically scans for cycles, which pauses your program
- **Memory overhead** — every object carries a reference count and type information

::: python Coming from Python
Here is a subtle Python bug that ownership prevents. In Python, `a` and `b` point to the *same* list:
```python
a = [1, 2, 3]
b = a
b.append(4)
print(a)  # [1, 2, 3, 4] — surprise! a changed because b is the same object
```
This shared mutable state is a major source of bugs. In Rust, when you assign `b = a`, the value *moves* to `b` and `a` becomes invalid. There is no shared mutable state by default. To share, you must explicitly borrow (next section) or clone (explicit copy).
:::

## Rust's ownership rules

Rust has three ownership rules. They are simple, but their consequences are profound:

1. **Every value has exactly one owner** — a variable that holds the value
2. **There can only be one owner at a time** — assigning to another variable *moves* ownership
3. **When the owner goes out of scope, the value is dropped** — memory is freed immediately

Let's see each rule in action.

### Rule 1: Every value has one owner

```rust
fn main() {
    let name = String::from("Agent");  // `name` owns this String
    println!("{}", name);              // `name` is still the owner, we can use it
}
// `name` goes out of scope here — the String is dropped and its memory is freed
```

### Rule 2: Moving ownership

This is where Python developers get surprised:

```rust
fn main() {
    let a = String::from("hello");
    let b = a;  // ownership MOVES from a to b

    // println!("{}", a);  // ERROR: value used after move
    println!("{}", b);     // OK — b is the owner now
}
```

When you write `let b = a;`, Rust does *not* make a copy of the string. It does not increment a reference count. It *moves* the ownership from `a` to `b`. After the move, `a` is no longer valid. Trying to use `a` is a compile-time error.

::: python Coming from Python
In Python, `b = a` creates a second reference to the same object. Both `a` and `b` are valid and point to the same data. In Rust, `b = a` transfers ownership. After the move, only `b` is valid. Think of it like handing someone a physical book — you had it, now they have it, and you cannot read a book you no longer hold.
:::

### Why does this feel so different?

Let's trace through the mental model:

**Python — reference counting:**
```python
a = "hello"    # Object created, a references it (refcount=1)
b = a          # b also references it (refcount=2)
# Both a and b are valid. The object exists until refcount hits 0.
```

**Rust — ownership transfer:**
```rust
fn main() {
    let a = String::from("hello");  // a owns the String
    let b = a;                       // ownership moves to b. a is invalidated.
    // Only b is valid now. The String exists until b goes out of scope.
    println!("{}", b);
}
```

This feels restrictive, but it gives you a powerful guarantee: at any point in your program, there is *exactly one* variable responsible for each piece of data. No confusion about who "owns" what. No surprise mutations through aliases.

### Rule 3: Scope-based cleanup (drop)

When a variable goes out of scope, Rust immediately runs its *drop* code, freeing any resources it holds. This is deterministic — you know exactly when cleanup happens.

```rust
fn main() {
    {
        let data = String::from("temporary");
        println!("{}", data);
    }  // `data` is dropped RIGHT HERE — memory freed immediately

    // data is not accessible here — it no longer exists
    println!("data was already freed");
}
```

::: python Coming from Python
Python has a similar concept with context managers:
```python
with open("file.txt") as f:
    data = f.read()
# f is closed here (guaranteed by __exit__)
```
In Rust, *every* resource gets this deterministic cleanup behavior automatically, not just things you wrap in `with` blocks. File handles, network connections, memory allocations — they are all freed the instant their owner goes out of scope. No `with` statement needed.
:::

## Move semantics in functions

Ownership also transfers when you pass values to functions:

```rust
fn print_greeting(name: String) {
    println!("Hello, {}!", name);
}  // `name` is dropped here — the String is freed

fn main() {
    let my_name = String::from("Alice");
    print_greeting(my_name);  // ownership moves into the function

    // println!("{}", my_name);  // ERROR: my_name was moved
}
```

Passing `my_name` to `print_greeting` moves ownership into the function's parameter. After the call, `my_name` is invalid. The function now owns the data and will free it when the function returns.

::: python Coming from Python
In Python, passing a value to a function just copies the reference:
```python
def print_greeting(name):
    print(f"Hello, {name}!")

my_name = "Alice"
print_greeting(my_name)
print(my_name)  # Still works — Python just passed a reference
```
In Rust, you "give away" the value when you pass it. If you want to keep using it after the function call, you need to either *borrow* it (next section) or *clone* it (make an explicit copy).
:::

## Copy vs Move: the simple types exception

Not everything moves. Simple, fixed-size types that live on the stack are *copied* instead of moved:

```rust
fn main() {
    let x: i32 = 42;
    let y = x;  // x is COPIED, not moved (integers implement Copy)
    println!("x={}, y={}", x, y);  // Both are valid!

    let a = true;
    let b = a;  // booleans are also Copy
    println!("a={}, b={}", a, b);  // Both are valid!
}
```

Types that implement the `Copy` trait — integers, floats, booleans, characters, and tuples of Copy types — are automatically copied on assignment. These types are small, cheap to copy, and have no resources to manage.

`String`, `Vec`, and other heap-allocated types do *not* implement `Copy`. They *move* on assignment because copying them would be expensive (duplicating heap data).

```rust
fn main() {
    // Copy types — assignment copies
    let x = 42;        // i32 → Copy
    let y = x;         // y is a copy, x still valid
    println!("{} {}", x, y);

    // Move types — assignment moves
    let s1 = String::from("hello");  // String → Move
    let s2 = s1;                     // s1 is moved to s2, s1 invalid
    // println!("{}", s1);  // ERROR: use of moved value
    println!("{}", s2);
}
```

## Explicit cloning

When you genuinely need a copy of heap data, use `.clone()`:

```rust
fn main() {
    let s1 = String::from("hello");
    let s2 = s1.clone();  // Deep copy — both s1 and s2 are valid

    println!("s1={}, s2={}", s1, s2);
}
```

::: python Coming from Python
`clone()` in Rust is like `copy.deepcopy()` in Python. The key difference is that Python makes shallow copies by default (`b = a` copies the reference), while Rust moves by default. In Rust, you opt into copying with `.clone()`. In Python, you opt into deep copying with `copy.deepcopy()`. The defaults are inverted, which pushes Rust developers toward sharing data through borrowing (zero-cost) rather than copying (expensive).
:::

## Returning ownership

Functions can return ownership back to the caller:

```rust
fn create_greeting(name: &str) -> String {
    let greeting = format!("Hello, {}!", name);
    greeting  // ownership of the String moves to the caller
}

fn main() {
    let message = create_greeting("Alice");
    println!("{}", message);  // message owns the String now
}
```

You can also take ownership in a function and return it:

```rust
fn add_exclamation(mut text: String) -> String {
    text.push('!');
    text  // return ownership to the caller
}

fn main() {
    let greeting = String::from("Hello");
    let excited = add_exclamation(greeting);
    // greeting is moved, but excited holds the modified value
    println!("{}", excited);  // "Hello!"
}
```

This pattern of "take ownership, modify, return" works but is verbose. The next section introduces *borrowing*, which lets you access data without taking ownership — a much more ergonomic approach.

## The ownership mental model

Here is a summary you can use as a reference:

| Situation | Python | Rust |
|-----------|--------|------|
| Assign `b = a` (heap type) | Both `a` and `b` reference the same object | `a` is moved to `b`; `a` is invalid |
| Assign `b = a` (int/bool) | Both valid (same object ref) | Both valid (value is copied) |
| Pass to function | Reference copied; caller can still use value | Ownership moves; caller cannot use value |
| Return from function | Reference returned | Ownership returned to caller |
| Variable goes out of scope | Refcount decremented; GC may collect later | Value dropped immediately; memory freed |
| Explicit copy | `copy.deepcopy(a)` | `a.clone()` |

## Key Takeaways

- Every value in Rust has exactly one owner, and when that owner goes out of scope, the value is immediately freed — no garbage collector needed
- Assignment of heap types (`String`, `Vec`) *moves* ownership, making the original variable invalid — this prevents aliased mutable state
- Simple stack types (integers, booleans, floats) implement `Copy` and are duplicated on assignment — both variables remain valid
- Use `.clone()` when you genuinely need a deep copy, but prefer borrowing (next section) for zero-cost data access
- Rust's ownership model trades the convenience of automatic reference counting for compile-time guarantees about memory safety and deterministic resource cleanup
