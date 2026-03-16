---
title: Cross Compilation
description: Set up cross-compilation toolchains to build your agent for macOS, Linux, and Windows from a single development machine.
---

# Cross Compilation

> **What you'll learn:**
> - How to configure Rust cross-compilation targets for macOS (x86_64 and aarch64), Linux (glibc and musl), and Windows using rustup and cross
> - Techniques for handling platform-specific dependencies (OpenSSL, libgit2, system libraries) during cross-compilation
> - How to set up CI matrix builds that produce binaries for all supported platforms in parallel using GitHub Actions

Your coding agent needs to run on your user's machine, and your users are not all on the same OS or architecture. Some are on macOS with Apple Silicon, others on Intel Macs, many on Linux x86_64 servers, and a growing number on Windows. Cross-compilation is the process of building binaries for a platform different from the one you are developing on. Rust has first-class support for this, and with the right tooling, you can produce binaries for every major platform from a single machine or CI runner.

## Understanding Targets

Rust uses "target triples" to identify platforms. A target triple encodes three pieces of information: architecture, vendor/OS, and ABI (application binary interface):

```
x86_64-unknown-linux-gnu
  ^         ^       ^
  |         |       |
  arch      OS    ABI/libc
```

Here are the targets that matter for a coding agent:

| Target Triple | Platform | Notes |
|---------------|----------|-------|
| `x86_64-apple-darwin` | macOS Intel | Older Macs (pre-2020) |
| `aarch64-apple-darwin` | macOS Apple Silicon | M1/M2/M3/M4 Macs |
| `x86_64-unknown-linux-gnu` | Linux x86_64 (glibc) | Most desktop/server Linux |
| `x86_64-unknown-linux-musl` | Linux x86_64 (musl) | Fully static binary |
| `aarch64-unknown-linux-gnu` | Linux ARM64 (glibc) | AWS Graviton, Raspberry Pi |
| `aarch64-unknown-linux-musl` | Linux ARM64 (musl) | Static ARM64 binary |
| `x86_64-pc-windows-msvc` | Windows x86_64 | Standard Windows build |

You can see all installed targets and add new ones with `rustup`:

```bash
# List installed targets
rustup target list --installed

# Add a new target
rustup target add x86_64-unknown-linux-musl
rustup target add aarch64-apple-darwin

# Build for a specific target
cargo build --release --target x86_64-unknown-linux-musl
```

::: python Coming from Python
Python's cross-platform story is completely different. You do not cross-compile Python code -- it is interpreted. But when your Python package has C extensions (numpy, cryptography, etc.), distributing across platforms becomes a nightmare. The `manylinux` project exists specifically to solve the "it compiled on my machine" problem for Linux wheels. The `cibuildwheel` tool orchestrates building wheels across platforms in CI. Rust sidesteps all of this: you cross-compile to a target triple and get a native binary. No wheels, no manylinux, no compatibility tags.
:::

## Simple Cross-Compilation with rustup

For pure Rust code with no C dependencies, cross-compilation is straightforward. Add the target and build:

```bash
# On an Apple Silicon Mac, build for Intel Mac
rustup target add x86_64-apple-darwin
cargo build --release --target x86_64-apple-darwin

# The binary lands in target/x86_64-apple-darwin/release/
ls target/x86_64-apple-darwin/release/my-agent
```

This works seamlessly within the same OS (macOS-to-macOS, Linux-to-Linux) because the system linker can handle both architectures. Cross-OS compilation (macOS-to-Linux) requires a cross-linker, which is where the tooling gets more involved.

## The `cross` Tool

The [`cross`](https://github.com/cross-rs/cross) project provides Docker-based cross-compilation that "just works" for most targets. It wraps Cargo and transparently runs your build inside a Docker container that has the right cross-compilation toolchain:

```bash
# Install cross
cargo install cross --git https://github.com/cross-rs/cross

# Build for Linux from macOS -- no manual toolchain setup
cross build --release --target x86_64-unknown-linux-gnu
cross build --release --target x86_64-unknown-linux-musl
cross build --release --target aarch64-unknown-linux-gnu
```

`cross` handles the hard parts: it provides the right GCC/musl cross-compiler, sets up sysroots with the right headers, and configures the linker. You use it exactly like `cargo` -- same flags, same behavior, just a different command name.

The main requirement is Docker (or Podman). If your CI runners have Docker available, `cross` works beautifully. If Docker is not an option, you need `cargo-zigbuild`.

## `cargo-zigbuild`: Cross-Compile Without Docker

[`cargo-zigbuild`](https://github.com/rust-cross/cargo-zigbuild) uses the Zig compiler's cross-compilation toolchain as a drop-in C/C++ cross-compiler. Zig bundles sysroots for many platforms, so you do not need Docker containers or manual toolchain installation:

```bash
# Install cargo-zigbuild
cargo install cargo-zigbuild

# Install Zig (macOS)
brew install zig

# Cross-compile for Linux from macOS
cargo zigbuild --release --target x86_64-unknown-linux-gnu
cargo zigbuild --release --target x86_64-unknown-linux-musl
cargo zigbuild --release --target aarch64-unknown-linux-gnu
```

A major advantage of `cargo-zigbuild` is glibc version targeting. You can specify the minimum glibc version your binary requires:

```bash
# Target glibc 2.17 (CentOS 7 / Amazon Linux 2 compatibility)
cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.17
```

This ensures your binary runs on older Linux distributions without the "GLIBC_2.28 not found" errors that plague many Rust binaries built on modern systems.

## Handling C Dependencies

The challenge with cross-compilation is C dependencies. If your coding agent uses crates that link to system C libraries, you need those libraries compiled for the target platform. Common offenders:

| Crate | C Dependency | Cross-Compile Solution |
|-------|-------------|----------------------|
| `openssl` | libssl, libcrypto | Use `rustls` instead (pure Rust) |
| `git2` | libgit2, libssh2 | Enable `vendored` feature |
| `reqwest` | (depends on TLS backend) | Use `rustls-tls` feature |
| `sqlite` | libsqlite3 | Use `bundled` feature |

The best strategy is to eliminate C dependencies wherever possible. For our coding agent:

```toml
[dependencies]
# Use rustls instead of OpenSSL for TLS
reqwest = { version = "0.12", default-features = false, features = [
    "rustls-tls", "json", "stream"
] }

# Vendor libgit2 so it compiles from source during the build
git2 = { version = "0.19", features = ["vendored"] }

# Bundled SQLite compiles from C source -- no system library needed
rusqlite = { version = "0.32", features = ["bundled"] }
```

The `vendored` and `bundled` features tell these crates to compile their C dependencies from source rather than linking to the system library. This makes cross-compilation work because the C code is compiled with the cross-compiler alongside your Rust code.

::: details Why not always vendor everything?
Vendoring increases build times (you are compiling C code from source) and can introduce version mismatches if the bundled version has known vulnerabilities that the system version has patched. For distribution builds, vendoring is the right call. For development, linking to system libraries is fine.
:::

## Configuring Linkers Per Target

When you cross-compile without `cross` or `cargo-zigbuild`, you need to tell Cargo which linker to use for each target. This is configured in `.cargo/config.toml`:

```toml
[target.x86_64-unknown-linux-gnu]
linker = "x86_64-linux-gnu-gcc"

[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"

[target.x86_64-unknown-linux-musl]
linker = "x86_64-linux-musl-gcc"

[target.x86_64-pc-windows-msvc]
linker = "lld-link"
```

On Ubuntu, install the cross-compilers:

```bash
sudo apt install gcc-x86-64-linux-gnu gcc-aarch64-linux-gnu musl-tools
```

On macOS, the Xcode toolchain handles both `x86_64-apple-darwin` and `aarch64-apple-darwin` natively. For Linux targets from macOS, use `cross` or `cargo-zigbuild` rather than manually installing cross-linkers.

## CI Matrix Builds with GitHub Actions

The practical approach for production is to build each platform on its native runner in CI. Here is a GitHub Actions workflow that builds for all major targets:

```yaml
name: Release

on:
  push:
    tags: ["v*"]

jobs:
  build:
    strategy:
      matrix:
        include:
          - target: x86_64-apple-darwin
            os: macos-latest
          - target: aarch64-apple-darwin
            os: macos-latest
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
          - target: x86_64-unknown-linux-musl
            os: ubuntu-latest
          - target: aarch64-unknown-linux-gnu
            os: ubuntu-latest
            use_cross: true
          - target: x86_64-pc-windows-msvc
            os: windows-latest

    runs-on: ${{ matrix.os }}

    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Install cross
        if: matrix.use_cross
        run: cargo install cross --git https://github.com/cross-rs/cross

      - name: Install musl-tools
        if: contains(matrix.target, 'musl')
        run: sudo apt install -y musl-tools

      - name: Build
        run: |
          if [ "${{ matrix.use_cross }}" = "true" ]; then
            cross build --release --target ${{ matrix.target }}
          else
            cargo build --release --target ${{ matrix.target }}
          fi
        shell: bash

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: binary-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/my-agent*
```

This workflow uses native runners for platforms that support direct compilation (macOS builds macOS targets, Ubuntu builds Linux x86_64 targets) and `cross` for targets that need a cross-compiler (ARM64 Linux from x86_64 Ubuntu).

::: wild In the Wild
Production coding agents like Claude Code build for multiple platforms in CI pipelines. The typical approach is a matrix build that produces binaries for macOS (both architectures), Linux (both glibc and musl), and sometimes Windows. The musl builds are particularly important because they produce fully static binaries that run on any Linux distribution without glibc version conflicts.
:::

## Universal Binaries on macOS

macOS supports "universal binaries" (also called "fat binaries") that contain code for both Intel and Apple Silicon. You can create one with the `lipo` tool:

```bash
# Build for both architectures
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin

# Combine into a universal binary
lipo -create \
  target/x86_64-apple-darwin/release/my-agent \
  target/aarch64-apple-darwin/release/my-agent \
  -output target/my-agent-universal

# Verify
file target/my-agent-universal
# target/my-agent-universal: Mach-O universal binary with 2 architectures:
#   Mach-O 64-bit executable x86_64
#   Mach-O 64-bit executable arm64
```

Universal binaries are larger (roughly double the size of a single-architecture binary) but simplify distribution: you ship one file that works on all Macs. Homebrew formulas often use universal binaries to avoid architecture detection logic.

## Key Takeaways

- Rust identifies platforms with target triples (e.g., `x86_64-unknown-linux-musl`) and you can add targets with `rustup target add`.
- Use `cross` (Docker-based) or `cargo-zigbuild` (Zig-based) to cross-compile for Linux from macOS without manual toolchain setup.
- Eliminate C dependencies where possible -- use `rustls` instead of OpenSSL, enable `vendored`/`bundled` features for unavoidable C libraries.
- CI matrix builds on native runners are the most reliable approach: let macOS runners build macOS binaries, Linux runners build Linux binaries.
- Use `lipo` to create macOS universal binaries that work on both Intel and Apple Silicon.
