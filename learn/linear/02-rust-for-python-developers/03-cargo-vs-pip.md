---
title: Cargo vs Pip
description: Understanding Cargo as Rust's build system and package manager, mapped to familiar Python concepts like pip, venv, and pyproject.toml.
---

# Cargo vs Pip

> **What you'll learn:**
> - How Cargo combines the roles of pip, setuptools, venv, and make into one unified tool
> - The structure of Cargo.toml compared to pyproject.toml and requirements.txt
> - How dependency resolution, versioning, and lock files work in the Rust ecosystem

In Python, building and distributing software involves a constellation of tools: pip for installing packages, venv or virtualenv for isolation, setuptools or poetry for packaging, pyproject.toml or setup.py for configuration, and requirements.txt or poetry.lock for pinning dependencies. Each tool handles one piece of the puzzle, and getting them all to work together is a rite of passage.

Rust's answer is Cargo. One tool. It handles creating projects, compiling code, managing dependencies, running tests, building documentation, and publishing packages. Once you understand Cargo, you understand the entire Rust build story.

## What Cargo replaces

Here is a mapping of Python tools to their Cargo equivalents:

| Python tool/concept | Cargo equivalent | Command |
|---------------------|-----------------|---------|
| `python -m venv` | (not needed — see below) | — |
| `pip install requests` | Adding a dependency | `cargo add reqwest` |
| `pip install -e .` | Building the project | `cargo build` |
| `python script.py` | Running the project | `cargo run` |
| `pytest` | Running tests | `cargo test` |
| `pyproject.toml` | `Cargo.toml` | — |
| `requirements.txt` / `poetry.lock` | `Cargo.lock` | — |
| `pip install build && python -m build` | Building a release | `cargo build --release` |
| `twine upload` | Publishing a crate | `cargo publish` |

::: python Coming from Python
There is no virtual environment in Rust because there is no need for one. Each Rust project has its own `target/` directory where compiled dependencies live. Dependencies are compiled per-project, not installed globally. You never have the "wrong virtualenv activated" problem because the concept does not exist. Each `Cargo.toml` file defines a self-contained project with its own dependency tree.
:::

## Cargo.toml — your project manifest

Every Rust project has a `Cargo.toml` at its root. This is the equivalent of `pyproject.toml`. Let's compare them side by side.

**Python — pyproject.toml:**

```toml
[project]
name = "my-agent"
version = "0.1.0"
description = "A CLI coding agent"
requires-python = ">=3.11"
dependencies = [
    "httpx>=0.25.0",
    "click>=8.0.0",
    "rich>=13.0.0",
]

[project.optional-dependencies]
dev = ["pytest", "ruff"]
```

**Rust — Cargo.toml:**

```toml
[package]
name = "my-agent"
version = "0.1.0"
edition = "2021"
description = "A CLI coding agent"

[dependencies]
reqwest = { version = "0.12", features = ["json"] }
clap = { version = "4", features = ["derive"] }
tokio = { version = "1", features = ["full"] }

[dev-dependencies]
assert_cmd = "2"
```

The structure is similar, but there are a few Rust-specific concepts to notice:

- **edition** — Rust editions (2015, 2018, 2021, 2024) allow the language to evolve without breaking old code. Always use the latest edition for new projects. This is roughly analogous to specifying `requires-python`.
- **features** — Rust crates can have optional features that you opt into. `reqwest = { version = "0.12", features = ["json"] }` means "install reqwest, and also enable its JSON serialization support." This is like Python extras (`httpx[http2]`), but far more granular.
- **dev-dependencies** — dependencies only used for testing, equivalent to putting pytest in `[project.optional-dependencies] dev`.

## Adding dependencies

In Python, you run `pip install requests` and then manually add it to your requirements file. In Rust, there is one command:

```bash
# Add a dependency — updates Cargo.toml automatically
cargo add serde --features derive

# Add a dev-only dependency
cargo add --dev mockall

# Remove a dependency
cargo remove serde
```

This is equivalent to `poetry add` or `pip install` + editing `pyproject.toml`, but built directly into the standard tool.

::: python Coming from Python
If you use Poetry, you are already familiar with the `poetry add` workflow. Cargo's `cargo add` works the same way. If you use plain pip, note that Cargo always updates the manifest file — you never have a disconnect between what is installed and what is declared. The "I forgot to update requirements.txt" problem does not exist.
:::

## Cargo.lock — deterministic builds

When you build your project, Cargo resolves all dependency versions and writes the exact resolved versions to `Cargo.lock`. This is equivalent to `poetry.lock` or `pip freeze > requirements.txt`.

The rules for committing `Cargo.lock`:

- **Binary projects (applications):** Always commit `Cargo.lock`. You want reproducible builds.
- **Libraries:** Do not commit `Cargo.lock`. Let downstream users resolve versions.

Since our coding agent is a binary application, you will always commit `Cargo.lock`.

```bash
# After any dependency change, Cargo.lock is automatically updated
cargo build
git add Cargo.lock
git commit -m "update dependencies"
```

## The build process

Here is where Cargo diverges most from Python. Python is interpreted — `python script.py` parses and executes your code directly. Rust is compiled — Cargo compiles your source code into a native binary before you can run it.

```bash
# Compile in debug mode (fast compilation, slow binary, with debug info)
cargo build
# Binary lands at: target/debug/my-agent

# Compile in release mode (slow compilation, fast binary, optimized)
cargo build --release
# Binary lands at: target/release/my-agent

# Compile and run in one step (most common during development)
cargo run

# Compile and run with release optimizations
cargo run --release
```

::: python Coming from Python
The closest Python equivalent to `cargo build --release` is using PyInstaller or Nuitka to bundle your Python application into a standalone binary. But those tools wrap the Python interpreter inside the binary. Rust compiles to actual machine code — no interpreter, no runtime, no dependencies. The resulting binary is a single file you can copy to any compatible machine and run directly.
:::

## Common Cargo commands cheat sheet

Here are the Cargo commands you will use daily, mapped to their Python equivalents:

```bash
# Create a new project (like `mkdir project && cd project && poetry init`)
cargo new my-project
cargo new my-library --lib  # creates a library instead of binary

# Build the project (like `python -m build` but for compilation)
cargo build

# Run the project (like `python main.py`)
cargo run

# Run tests (like `pytest`)
cargo test

# Check if code compiles without producing a binary (faster than build)
cargo check

# Format code (like `black .` or `ruff format .`)
cargo fmt

# Lint code (like `flake8 .` or `ruff check .`)
cargo clippy

# Generate and open documentation (like `pdoc` or `sphinx`)
cargo doc --open

# Update dependencies to latest compatible versions (like `pip install --upgrade`)
cargo update
```

The command you will use most during development is `cargo check`. It runs all the compiler checks without actually producing a binary, which is significantly faster than `cargo build`. Use it constantly to verify your code compiles as you work.

## Workspaces — Cargo's monorepo support

For larger projects, Cargo supports *workspaces* — a way to manage multiple related packages in one repository. This is similar to Python's monorepo patterns with tools like `hatch` or having multiple packages in one directory with a shared virtualenv.

```toml
# Root Cargo.toml
[workspace]
members = [
    "agent-core",
    "agent-cli",
    "agent-tools",
]
```

Each member is a full Cargo project with its own `Cargo.toml`, but they share a single `target/` directory and `Cargo.lock`. This means dependencies are resolved once for the entire workspace, and a `cargo build` at the root builds everything.

We will use a workspace structure for our coding agent as the project grows.

## The crates.io registry

Rust packages are called *crates*, and the central registry is [crates.io](https://crates.io). This is the equivalent of [PyPI](https://pypi.org). When you add a dependency in `Cargo.toml`, Cargo downloads it from crates.io by default.

You can search for crates:

```bash
cargo search serde
```

Or browse [crates.io](https://crates.io) and [lib.rs](https://lib.rs) (a community-curated alternative frontend) to find packages.

::: python Coming from Python
One cultural difference: Rust crate names on crates.io follow a strict one-name-per-crate policy with no namespacing. In Python, you have organizational packages like `google-cloud-storage` or `azure-storage-blob`. In Rust, names are globally unique and often short: `serde`, `tokio`, `clap`. Check crates.io before naming your own crate to avoid conflicts.
:::

## Key Takeaways

- Cargo is a single tool that replaces pip, venv, setuptools, poetry, pytest runner, and make — there is one way to do each operation
- `Cargo.toml` is equivalent to `pyproject.toml`, and `Cargo.lock` is equivalent to `poetry.lock` — always commit `Cargo.lock` for binary applications
- `cargo check` is your most-used command during development — it verifies your code compiles without producing a binary, which is faster than `cargo build`
- Rust has no virtual environments because each project is self-contained with its own compiled dependencies in the `target/` directory
- Crates (Rust packages) live on crates.io, equivalent to PyPI, and are managed entirely through `Cargo.toml`
