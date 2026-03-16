---
title: Modules and Crates
description: Organizing Rust code with modules and crates, mapped to Python's packages, modules, and import system.
---

# Modules and Crates

> **What you'll learn:**
> - How Rust's module system uses mod, use, and pub to control visibility and organize code
> - The difference between library crates and binary crates and when to use each
> - How to structure a multi-file Rust project compared to Python packages with __init__.py

As your Rust projects grow beyond a single file, you need a way to organize code into logical units. Python uses packages (directories with `__init__.py`) and modules (`.py` files) with an `import` statement. Rust uses a module system with `mod`, `use`, and `pub` keywords that controls both organization and visibility in ways Python does not.

## Python packages vs Rust modules — the mental map

| Python concept | Rust concept | Notes |
|----------------|-------------|-------|
| `.py` file | module | A file is a module |
| Package (directory with `__init__.py`) | module (directory with `mod.rs`) | Nested modules |
| `import` / `from ... import` | `use` | Bringing items into scope |
| `__all__` / `_private_prefix` convention | `pub` keyword | Controlling visibility |
| PyPI package | Crate | External dependency |
| `pip install` | `cargo add` | Adding a crate |

## Inline modules

The simplest module form is an inline module within a single file:

```rust
mod tools {
    pub fn shell_execute(cmd: &str) -> String {
        format!("Executing: {}", cmd)
    }

    pub fn read_file(path: &str) -> String {
        format!("Reading: {}", path)
    }

    // Private function — not accessible outside this module
    fn validate_command(cmd: &str) -> bool {
        !cmd.contains("rm -rf")
    }
}

fn main() {
    println!("{}", tools::shell_execute("ls -la"));
    println!("{}", tools::read_file("main.rs"));

    // tools::validate_command("test");  // ERROR: function is private
}
```

::: python Coming from Python
In Python, everything in a module is public by default. You *conventionally* prefix private names with `_`, but nothing enforces it — anyone can import `_private_function` and use it.

In Rust, everything is *private by default*. You must explicitly add `pub` to make items accessible from outside their module. This is not a convention — the compiler enforces it. If it is not `pub`, external code cannot see it.
:::

## File-based modules

In real projects, each module lives in its own file. Here is how to structure a multi-file project:

```
src/
  main.rs          # Binary entry point
  tools.rs         # The "tools" module
  config.rs        # The "config" module
```

**src/main.rs:**

```rust
mod tools;   // Tells Rust to look for src/tools.rs
mod config;  // Tells Rust to look for src/config.rs

fn main() {
    let cmd_output = tools::shell_execute("ls");
    let api_key = config::load_api_key();
    println!("{}", cmd_output);
    println!("Key: {}", api_key);
}
```

**src/tools.rs:**

```rust
pub fn shell_execute(cmd: &str) -> String {
    format!("Executing: {}", cmd)
}

pub fn read_file(path: &str) -> String {
    format!("Reading: {}", path)
}
```

**src/config.rs:**

```rust
pub fn load_api_key() -> String {
    String::from("sk-placeholder")
}
```

::: python Coming from Python
In Python, files are automatically importable:
```python
# Just works — Python finds tools.py and config.py
from tools import shell_execute
from config import load_api_key
```
In Rust, you must declare modules with `mod tools;` in the parent file. This explicit declaration is required — Rust does not automatically discover files. This might feel tedious, but it means you always know exactly what modules exist by reading the parent file.
:::

## Directory modules (nested modules)

For deeper organization, you create a directory with a `mod.rs` file:

```
src/
  main.rs
  tools/
    mod.rs           # The tools module root (like Python's __init__.py)
    shell.rs         # tools::shell submodule
    file_ops.rs      # tools::file_ops submodule
```

**src/main.rs:**

```rust
mod tools;

fn main() {
    let output = tools::shell::execute("ls -la");
    let content = tools::file_ops::read("main.rs");
    println!("{}", output);
    println!("{}", content);
}
```

**src/tools/mod.rs:**

```rust
pub mod shell;
pub mod file_ops;
```

**src/tools/shell.rs:**

```rust
pub fn execute(cmd: &str) -> String {
    format!("$ {}", cmd)
}
```

**src/tools/file_ops.rs:**

```rust
pub fn read(path: &str) -> String {
    format!("Contents of {}", path)
}
```

::: python Coming from Python
This maps directly to Python's package structure:
```
tools/
  __init__.py     # like mod.rs — declares what the package exports
  shell.py        # like shell.rs
  file_ops.py     # like file_ops.rs
```
With `__init__.py`:
```python
from .shell import execute
from .file_ops import read
```
The Rust version uses `pub mod shell;` and `pub mod file_ops;` in `mod.rs` instead. The concept is identical — the parent module declares its children.
:::

## The `use` keyword — importing items

`use` brings items into scope so you do not need full paths:

```rust
mod tools {
    pub mod shell {
        pub fn execute(cmd: &str) -> String {
            format!("$ {}", cmd)
        }
    }
}

// Bring a specific item into scope
use tools::shell::execute;

// Or bring the module into scope
use tools::shell;

fn main() {
    // With `use tools::shell::execute`:
    println!("{}", execute("ls"));

    // With `use tools::shell`:
    println!("{}", shell::execute("ls"));
}
```

Common `use` patterns:

```rust
// Import a specific item
use std::collections::HashMap;

// Import multiple items from the same module
use std::io::{self, Read, Write};

// Import everything from a module (use sparingly)
use std::collections::*;

// Rename on import (like Python's `import numpy as np`)
use std::collections::HashMap as Map;
```

::: python Coming from Python
Rust's `use` maps directly to Python's `from ... import`:

| Python | Rust |
|--------|------|
| `from os.path import join` | `use std::path::join;` |
| `from os import path` | `use std::path;` |
| `import os.path as osp` | `use std::path as p;` |
| `from collections import *` | `use std::collections::*;` |
| `from io import StringIO, BytesIO` | `use std::io::{self, Read, Write};` |

The main difference: in Python, imports are resolved at runtime. In Rust, `use` is resolved at compile time — if the path is wrong, you get a compile error immediately.
:::

## Visibility rules

Rust's privacy model is strict and module-based:

```rust
mod database {
    pub struct Connection {
        pub host: String,
        port: u16,           // private — only accessible within this module
    }

    impl Connection {
        pub fn new(host: String, port: u16) -> Self {
            Connection { host, port }
        }

        pub fn url(&self) -> String {
            format!("{}:{}", self.host, self.port)
        }

        fn reconnect(&self) {
            // private method — only this module can call it
            println!("Reconnecting to {}", self.url());
        }
    }
}

fn main() {
    let conn = database::Connection::new(String::from("localhost"), 5432);
    println!("Host: {}", conn.host);     // OK — host is pub
    // println!("Port: {}", conn.port);  // ERROR — port is private
    println!("URL: {}", conn.url());     // OK — url() is pub
    // conn.reconnect();                  // ERROR — reconnect() is private
}
```

Visibility modifiers:
- **(no modifier)** — private to the current module and its children
- `pub` — visible everywhere
- `pub(crate)` — visible within the current crate but not to external users
- `pub(super)` — visible to the parent module

## Library crates vs binary crates

A Rust project can be a binary (an executable), a library (code for others to use), or both.

**Binary crate** — has a `src/main.rs`:
```
my-agent/
  Cargo.toml
  src/
    main.rs    # Entry point: fn main()
```

**Library crate** — has a `src/lib.rs`:
```
my-library/
  Cargo.toml
  src/
    lib.rs     # Entry point: pub functions, structs, traits
```

**Both** — has both files:
```
my-agent/
  Cargo.toml
  src/
    main.rs    # The binary — can use items from the library
    lib.rs     # The library — reusable code
```

For our coding agent, we will use the "both" pattern. Core logic lives in `lib.rs` (testable, reusable), and `main.rs` is a thin wrapper that starts the application.

## A practical project structure

Here is a realistic structure for a coding agent project:

```
coding-agent/
  Cargo.toml
  src/
    main.rs              # Entry point
    lib.rs               # Library root — re-exports public API
    agent/
      mod.rs             # Agent module
      loop.rs            # Agentic loop
      context.rs         # Context management
    tools/
      mod.rs             # Tools module
      shell.rs           # Shell execution tool
      file_read.rs       # File reading tool
      file_write.rs      # File writing tool
    api/
      mod.rs             # API module
      client.rs          # HTTP client
      types.rs           # API request/response types
  tests/
    integration_test.rs  # Integration tests
```

::: python Coming from Python
The equivalent Python structure would be:
```
coding_agent/
  __main__.py             # Entry point
  __init__.py             # Package root
  agent/
    __init__.py
    loop.py
    context.py
  tools/
    __init__.py
    shell.py
    file_read.py
    file_write.py
  api/
    __init__.py
    client.py
    types.py
tests/
  test_integration.py
```
The key structural difference: Python's `__init__.py` can contain arbitrary code and often re-exports symbols. Rust's `mod.rs` typically just declares submodules with `pub mod` and occasionally re-exports with `pub use`.
:::

## Re-exports with `pub use`

You can re-export items to simplify the public API:

```rust
// In src/tools/mod.rs
pub mod shell;
pub mod file_ops;

// Re-export commonly used items so users can write `tools::execute`
// instead of `tools::shell::execute`
pub use shell::execute;
pub use file_ops::read;
```

This is equivalent to Python's pattern of importing in `__init__.py`:

```python
# In tools/__init__.py
from .shell import execute
from .file_ops import read
```

## Key Takeaways

- Rust modules are declared explicitly with `mod` — the compiler does not auto-discover files like Python does
- Everything is private by default; use `pub` to expose items — this is enforced by the compiler, not by naming convention
- File-based modules (`tools.rs`) and directory modules (`tools/mod.rs`) correspond to Python's modules and packages with `__init__.py`
- `use` is Rust's import statement, resolved at compile time — it maps directly to Python's `from ... import` syntax
- Structure projects with `lib.rs` for reusable logic and `main.rs` as a thin entry point — this enables testing and reuse of your core code
