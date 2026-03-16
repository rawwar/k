---
title: "Chapter 2: Rust for Python Developers"
description: A bridge from Python to Rust covering ownership, error handling, traits, and async — everything you need to start building in Rust.
---

# Rust for Python Developers

If you are coming from Python, Rust can feel like a different planet. The compiler is strict, there is no garbage collector, and you must think about memory ownership in ways Python never required. But the reward is enormous: zero-cost abstractions, fearless concurrency, and a type system that catches entire classes of bugs at compile time. This chapter bridges the gap by mapping every major Rust concept to its Python equivalent.

We start with practical setup — installing Rust and understanding Cargo — then work through the language feature by feature. Each section presents the Python way first, then shows how Rust achieves the same goal with its own idioms. Ownership and borrowing are covered in depth because they are the biggest conceptual leap. We then move through structs, enums, traits, generics, and error handling, always drawing explicit parallels to Python classes, protocols, and exception handling.

By the end of this chapter, you will be able to read and write Rust at a level sufficient to build the coding agent. You will not be a Rust expert, but you will have a working mental model and enough fluency to follow the implementation chapters that come next.

## Learning Objectives
- Install Rust and use Cargo to create, build, and test projects
- Understand ownership, borrowing, and lifetimes as alternatives to garbage collection
- Map Python classes to Rust structs and Python protocols to Rust traits
- Handle errors idiomatically with Result and Option instead of try/except
- Write async Rust code using tokio and understand how it compares to Python's asyncio
- Use Rust's module system, collections, and iterators with confidence

## Subchapters
1. [Why Rust](/linear/02-rust-for-python-developers/01-why-rust)
2. [Installing and Setup](/linear/02-rust-for-python-developers/02-installing-and-setup)
3. [Cargo vs Pip](/linear/02-rust-for-python-developers/03-cargo-vs-pip)
4. [Hello World](/linear/02-rust-for-python-developers/04-hello-world)
5. [Variables Ownership vs Python GC](/linear/02-rust-for-python-developers/05-variables-ownership-vs-python-gc)
6. [Borrowing and References](/linear/02-rust-for-python-developers/06-borrowing-and-references)
7. [Structs vs Classes](/linear/02-rust-for-python-developers/07-structs-vs-classes)
8. [Enums and Pattern Matching](/linear/02-rust-for-python-developers/08-enums-and-pattern-matching)
9. [Error Handling Result vs Try Except](/linear/02-rust-for-python-developers/09-error-handling-result-vs-try-except)
10. [Traits vs Protocols](/linear/02-rust-for-python-developers/10-traits-vs-protocols)
11. [Generics](/linear/02-rust-for-python-developers/11-generics)
12. [Modules and Crates](/linear/02-rust-for-python-developers/12-modules-and-crates)
13. [Strings in Rust](/linear/02-rust-for-python-developers/13-strings-in-rust)
14. [Collections](/linear/02-rust-for-python-developers/14-collections)
15. [Iterators vs Comprehensions](/linear/02-rust-for-python-developers/15-iterators-vs-comprehensions)
16. [Async Await](/linear/02-rust-for-python-developers/16-async-await)
17. [Testing](/linear/02-rust-for-python-developers/17-testing)
18. [Summary](/linear/02-rust-for-python-developers/18-summary)

## Prerequisites
- Python programming experience — you should be comfortable writing functions, classes, and using pip
- A terminal you are familiar with (bash, zsh, PowerShell, etc.)
- No prior Rust experience required — this chapter starts from zero
