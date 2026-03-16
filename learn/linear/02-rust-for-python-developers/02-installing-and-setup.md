---
title: Installing and Setup
description: Getting Rust installed via rustup, configuring your editor, and verifying your development environment is ready.
---

# Installing and Setup

> **What you'll learn:**
> - How to install Rust using rustup and manage toolchain versions
> - How to configure VS Code or your preferred editor with rust-analyzer for a productive workflow
> - How to verify your installation and troubleshoot common setup issues

Before you write a single line of Rust, you need a working development environment. If you have installed Python, you are used to downloading an installer or using pyenv to manage versions. Rust has its own version manager called `rustup`, and it is significantly simpler than the Python toolchain story.

## Installing Rust with rustup

Rust is installed through `rustup`, a command-line tool that manages Rust toolchain versions. Open your terminal and run:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

On Windows, download and run `rustup-init.exe` from [rustup.rs](https://rustup.rs).

The installer will ask you to choose an installation option. Press `1` for the default installation. This installs three things:

- **rustc** — the Rust compiler (analogous to the `python` interpreter)
- **cargo** — the build system and package manager (we will compare this to pip in the next section)
- **rustup** — the toolchain manager itself (analogous to pyenv)

After installation, restart your terminal or run:

```bash
source "$HOME/.cargo/env"
```

Verify the installation by checking versions:

```bash
rustc --version
# rustc 1.83.0 (90b35a623 2024-11-26)

cargo --version
# cargo 1.83.0 (5ffbef321 2024-10-29)

rustup --version
# rustup 1.27.1 (54dd3d00f 2024-04-24)
```

::: python Coming from Python
In Python, managing versions is a whole ordeal — pyenv, deadsnakes PPA, conda, the system Python you should never touch. Rust's `rustup` is like pyenv, virtualenv, and pip all rolled into one installer. There is exactly one blessed way to install Rust, and it works the same on every platform. No more "which Python am I using?" confusion.
:::

## Understanding the toolchain

A Rust "toolchain" is a specific version of the compiler plus its associated tools. By default, you are on the `stable` channel. Rust has three channels:

| Channel | Description | Python equivalent |
|---------|-------------|-------------------|
| **stable** | Production-ready releases every 6 weeks | Latest stable Python release |
| **beta** | Next stable release, for testing | Python release candidates |
| **nightly** | Bleeding-edge, may contain unstable features | Python dev builds |

For this course, you will use `stable`. You can update to the latest stable release at any time:

```bash
rustup update stable
```

To see which toolchains you have installed:

```bash
rustup toolchain list
# stable-aarch64-apple-darwin (default)
```

::: python Coming from Python
Unlike Python where version 3.11 and 3.12 can have meaningful behavioral differences, Rust's stability guarantee means that code that compiles on stable Rust 1.70 will compile on Rust 1.83 with the same behavior. Rust never breaks backward compatibility on the stable channel. This is a remarkable guarantee that the Python ecosystem does not offer.
:::

## Configuring your editor

The single most important thing you can do for Rust productivity is install **rust-analyzer**. It provides real-time feedback from the compiler as you type — autocompletion, type hints, error highlighting, and one-click fixes.

### VS Code (recommended for beginners)

1. Install the [rust-analyzer extension](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
2. That is it. Seriously. Open a Rust project and it starts working.

rust-analyzer gives you:
- **Inline type hints** — hover over any variable to see its inferred type
- **Error highlighting** — red squiggles appear as you type, before you even save
- **Go to definition** — Ctrl/Cmd+click on any function, type, or module
- **Code actions** — automatic imports, derive macro additions, match arm generation
- **Integrated terminal** — run `cargo build` and `cargo test` without leaving the editor

### Other editors

- **Neovim/Vim** — use `nvim-lspconfig` with the `rust_analyzer` server
- **Emacs** — use `lsp-mode` or `eglot` with rust-analyzer
- **IntelliJ/CLion** — use the official Rust plugin (includes its own analysis engine)
- **Zed** — has built-in Rust support with rust-analyzer
- **Helix** — has built-in LSP support, just install rust-analyzer

Whichever editor you choose, **make sure rust-analyzer is working before you proceed**. Open a Rust file, make a deliberate type error, and verify that it highlights the error. The quality of your learning experience depends heavily on this real-time feedback.

::: python Coming from Python
If you use Pylance or Pyright in VS Code for Python, rust-analyzer provides a similar experience but with much deeper analysis. Because Rust's type system is richer and enforced by the compiler, rust-analyzer can give you more precise information. Where Pylance might say "this could be `str | None`", rust-analyzer will tell you the exact type at every point in your code.
:::

## Useful Cargo components

Install a couple of useful Cargo components that you will use throughout this course:

```bash
# Clippy — a linter with hundreds of lint rules (think flake8/pylint for Rust)
rustup component add clippy

# Rustfmt — automatic code formatting (think black/ruff for Rust)
rustup component add rustfmt
```

Now you can:

```bash
# Format all code in a project (like running `black .`)
cargo fmt

# Lint your code (like running `flake8 .` or `ruff check .`)
cargo clippy
```

::: python Coming from Python
In Python, you choose and configure your own formatter (black, ruff, autopep8) and linter (flake8, pylint, ruff). In Rust, `rustfmt` and `clippy` are the official, community-standard tools. There is no debate about formatting style — `cargo fmt` produces the one true format, similar to how Go has `gofmt`. This means every Rust project you encounter will be formatted identically.
:::

## Verifying everything works

Let's verify your full setup with a quick test. Run these commands:

```bash
# Create a new project
cargo new hello-verify
cd hello-verify

# Build and run
cargo run
```

You should see:

```
   Compiling hello-verify v0.1.0 (/path/to/hello-verify)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.50s
     Running `target/debug/hello-verify`
Hello, world!
```

Now verify your tooling:

```bash
# Format the code
cargo fmt

# Run the linter
cargo clippy
```

If all three commands succeed without errors, your environment is ready.

## Troubleshooting common issues

**"command not found: cargo"** — Your shell configuration was not reloaded. Run `source "$HOME/.cargo/env"` or restart your terminal. On macOS/Linux, check that `$HOME/.cargo/bin` is in your `PATH`.

**"linker 'cc' not found"** — Rust needs a C linker to produce binaries. On macOS, install Xcode Command Line Tools with `xcode-select --install`. On Ubuntu/Debian, install `build-essential` with `sudo apt install build-essential`. On Fedora, `sudo dnf install gcc`.

**rust-analyzer not working** — Make sure you opened a *folder* containing a `Cargo.toml` file, not just a loose `.rs` file. rust-analyzer needs the Cargo project structure to function.

**"error: could not find `Cargo.toml`"** — You are running a Cargo command outside a Cargo project. Make sure you `cd` into a directory that contains a `Cargo.toml` file.

## Clean up

You can delete the test project now. We will create proper projects in the upcoming sections:

```bash
cd ..
rm -rf hello-verify
```

Your Rust development environment is ready. In the next section, you will learn how Cargo compares to the Python tools you already know.

## Key Takeaways

- Install Rust with `rustup`, which manages the compiler, Cargo, and toolchain versions in one tool — far simpler than Python's pyenv/pip/venv story
- rust-analyzer is essential for a productive Rust experience — install it in your editor before writing any code
- `cargo fmt` and `cargo clippy` are the standard formatting and linting tools, equivalent to black and flake8 in the Python ecosystem
- Rust's stability guarantee means code that compiles today will compile on future stable releases without breaking changes
