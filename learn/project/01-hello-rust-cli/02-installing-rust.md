---
title: Installing Rust
description: Install the Rust toolchain with rustup and configure your editor for a productive development workflow with rust-analyzer.
---

# Installing Rust

> **What you'll learn:**
> - How to install Rust on macOS, Linux, and Windows using rustup
> - How to verify your installation and understand stable, beta, and nightly channels
> - How to set up rust-analyzer in your editor for autocompletion and inline error reporting

Before you write a single line of Rust, you need the toolchain on your machine. The good news: Rust has one of the smoothest installation experiences in the compiled-language world. A single command gets you the compiler, the build tool, the package manager, and the documentation — all managed by a version manager called **rustup**.

## Installing with rustup

`rustup` is the official Rust toolchain installer and version manager. Think of it as the equivalent of `pyenv` for Python — it lets you install multiple Rust versions, switch between them, and keep everything up to date.

### macOS and Linux

Open your terminal and run:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

The installer walks you through a few options. Choose the default installation (option 1) unless you have a specific reason to customize. When it finishes, it adds Rust's binaries to your shell's PATH. You may need to restart your terminal or run:

```bash
source "$HOME/.cargo/env"
```

### Windows

On Windows, download and run `rustup-init.exe` from [https://rustup.rs](https://rustup.rs). The installer handles everything, including setting up the MSVC build tools that Rust needs on Windows.

If you prefer the Windows Subsystem for Linux (WSL), use the macOS/Linux instructions above inside your WSL terminal.

::: python Coming from Python
If you have used `pyenv` to manage Python versions, `rustup` fills the same role. The key difference: `rustup` is the *official* tool blessed by the Rust project, so there is no confusion about which installer to use. Unlike Python's ecosystem — where you might choose between `pyenv`, `conda`, `asdf`, or system packages — Rust has one answer: `rustup`.
:::

## Verifying Your Installation

Once the installer finishes, verify that everything is working:

```bash
rustc --version
cargo --version
rustup --version
```

You should see output similar to:

```
rustc 1.84.0 (9fc6b43 2025-01-07)
cargo 1.84.0 (fb7c438 2024-12-20)
rustup 1.27.1 (54dd3d0 2024-04-24)
```

The exact version numbers will differ depending on when you install, but all three commands should succeed. Here is what each tool does:

- **`rustc`** — the Rust compiler. You rarely call it directly (Cargo handles that), but it is the engine under the hood.
- **`cargo`** — the build tool and package manager. You use this constantly. It builds your code, runs tests, manages dependencies, and more.
- **`rustup`** — the toolchain manager. It installs and updates `rustc`, `cargo`, and other components.

## Understanding Toolchain Channels

Rust ships on three release channels:

| Channel | Release Cadence | Use Case |
|---------|----------------|----------|
| **stable** | Every 6 weeks | Production code — this is your default |
| **beta** | Every 6 weeks (previews the next stable) | Testing upcoming features |
| **nightly** | Every night | Experimental features, bleeding-edge crates |

As a beginner, you should stay on **stable**. Every example in this book works on stable Rust. The only reason to install nightly is if a specific crate requires it (we will note when that happens — it is rare).

To update your toolchain to the latest stable release:

```bash
rustup update stable
```

To check which toolchain is currently active:

```bash
rustup show
```

## Installing Helpful Components

The Rust toolchain comes with several optional components that make development smoother. Install these now:

```bash
rustup component add clippy rustfmt
```

- **`clippy`** — a linter that catches common mistakes and suggests idiomatic improvements. Think of it as `pylint` or `flake8` for Rust.
- **`rustfmt`** — an automatic code formatter. It is the equivalent of `black` for Python. Run it with `cargo fmt` and never argue about code style again.

You can try them immediately:

```bash
# Format all code in your project
cargo fmt

# Run the linter
cargo clippy
```

(These won't do much until you have a project, but they will be ready when you do.)

## Setting Up Your Editor

Rust has excellent editor support through **rust-analyzer**, the official language server. It gives you:

- Real-time error highlighting as you type
- Autocompletion with type information
- Inline type hints for variables where Rust infers the type
- Go-to-definition, find-all-references, and rename refactoring
- Code actions to automatically apply compiler suggestions

### VS Code

Install the **rust-analyzer** extension from the VS Code marketplace. Search for "rust-analyzer" (the publisher is "rust-lang"). That is it — no extra configuration needed. When you open a Rust project, rust-analyzer automatically detects the `Cargo.toml` and starts analyzing your code.

Recommended additional extensions:

- **Even Better TOML** — syntax highlighting for `Cargo.toml`
- **Error Lens** — shows error messages inline next to the problematic line
- **CodeLLDB** — debugger support for Rust

### Zed

Zed has built-in Rust support powered by rust-analyzer. Open a Rust project and it works out of the box.

### Neovim

If you use Neovim with the built-in LSP client, add rust-analyzer to your LSP configuration. If you use a plugin manager like `lazy.nvim`, the `mason.nvim` plugin can install rust-analyzer for you automatically.

### JetBrains (RustRover or IntelliJ + Rust Plugin)

JetBrains offers **RustRover**, a dedicated Rust IDE, or you can use the Rust plugin with IntelliJ IDEA. Both provide excellent Rust support with their own analysis engine.

::: python Coming from Python
If you currently use VS Code with Pylance for Python, switching to rust-analyzer for Rust feels very similar. The big upgrade: Rust's type system is fully static, so rust-analyzer can give you *exact* type information everywhere — no guessing, no `# type: ignore`. Autocompletion is precise because every type is known at compile time.
:::

## Your First Compile Test

Let's verify that everything works end-to-end. Create a temporary file and compile it:

```bash
echo 'fn main() { println!("Rust is ready!"); }' > /tmp/test_rust.rs
rustc /tmp/test_rust.rs -o /tmp/test_rust
/tmp/test_rust
```

You should see:

```
Rust is ready!
```

This compiles a Rust source file directly with `rustc`. In practice, you almost never do this — you use `cargo` instead, which handles compilation, linking, and dependency management for you. But it is useful to know that `rustc` is there and that your toolchain works.

Clean up:

```bash
rm /tmp/test_rust.rs /tmp/test_rust
```

## Keeping Rust Up to Date

Rust releases a new stable version every six weeks. To update:

```bash
rustup update
```

This updates all installed channels (stable, beta, nightly) to their latest versions. Get in the habit of running this periodically — Rust is backward-compatible, so updating stable never breaks your existing code.

## Key Takeaways

- Install Rust with `rustup`, the official toolchain manager — one command gives you the compiler (`rustc`), build tool (`cargo`), and version manager (`rustup`).
- Stay on the **stable** channel for all the code in this book. Update regularly with `rustup update`.
- Install `clippy` and `rustfmt` immediately — they catch bugs and format your code automatically, just like `flake8` and `black` do in Python.
- Set up **rust-analyzer** in your editor for real-time error checking, autocompletion, and inline type hints — it is the single biggest productivity boost for Rust development.
- Verify your installation with `rustc --version`, `cargo --version`, and a quick compile test before moving on.
