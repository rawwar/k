---
title: Static Linking
description: Produce fully self-contained binaries with no runtime dependencies by statically linking system libraries and using musl on Linux.
---

# Static Linking

> **What you'll learn:**
> - How to statically link common dependencies (OpenSSL via rustls, libgit2 via the git2 crate's vendored feature) to eliminate runtime library requirements
> - Techniques for building fully static Linux binaries using musl libc, including the tradeoffs with glibc compatibility
> - How to verify that your binary is truly self-contained using tools like ldd, otool, and file to check for dynamic library dependencies

When you hand someone a binary, you want it to work. Not "works if you install libssl 1.1" or "works if your glibc is at least 2.28." A statically linked binary contains everything it needs to run -- all library code is baked into the single executable file. The user downloads it, marks it executable, and runs it. No package manager, no dependency resolution, no "it works on my machine." This is one of Rust's greatest practical advantages for CLI tools, and this subchapter shows you how to achieve it.

## Dynamic vs. Static Linking

By default, Rust statically links all Rust code but dynamically links system libraries. On Linux, even a "hello world" binary dynamically links to libc:

```bash
# Build a default release binary
cargo build --release

# Check dynamic dependencies on Linux
ldd target/release/my-agent
#   linux-vdso.so.1 (0x00007ffcc3bfe000)
#   libgcc_s.so.1 => /lib/x86_64-linux-gnu/libgcc_s.so.1
#   libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6
#   /lib64/ld-linux-x86-64.so.2

# Check on macOS
otool -L target/release/my-agent
#   /usr/lib/libSystem.B.dylib
#   /usr/lib/libresolv.9.dylib
```

Those dynamic dependencies mean your binary needs compatible versions of those libraries on the target system. For libc, this is usually fine -- every Linux system has it. But if your agent links to OpenSSL, libgit2, or libsqlite3, you are asking users to install specific library versions.

::: python Coming from Python
The Python equivalent of this problem is the infamous `pip install cryptography` failure, where the user doesn't have OpenSSL development headers, or has the wrong version. The `manylinux` wheel standard exists to bundle pre-compiled shared libraries inside Python wheels. Rust's approach is simpler: statically link everything and ship a single binary. No shared library search paths, no `LD_LIBRARY_PATH` hacks, no `delocate` or `auditwheel` post-processing.
:::

## Strategy 1: Use Pure Rust Alternatives

The cleanest way to eliminate dynamic dependencies is to avoid C libraries entirely. The Rust ecosystem provides pure-Rust replacements for the most common C dependencies:

| C Library | Rust Alternative | How to Switch |
|-----------|-----------------|---------------|
| OpenSSL (libssl) | `rustls` | Use `reqwest` with `rustls-tls` feature |
| libcurl | `reqwest` / `hyper` | Already pure Rust (with rustls) |
| libgit2 | `gitoxide` (`gix`) | Replace `git2` crate with `gix` |
| libsqlite3 | `rusqlite` bundled | Enable `bundled` feature flag |
| libz (zlib) | `flate2` with `rust_backend` | Feature flag on `flate2` |

For our coding agent, the key change is the HTTP client:

```toml
[dependencies]
# Instead of this (links to system OpenSSL):
# reqwest = { version = "0.12", features = ["json"] }

# Use this (pure Rust TLS):
reqwest = { version = "0.12", default-features = false, features = [
    "rustls-tls",
    "json",
    "stream",
    "http2",
] }
```

The `default-features = false` is important. Without it, `reqwest` may pull in `native-tls`, which links to OpenSSL on Linux or Security.framework on macOS. With `rustls-tls`, all TLS is handled by pure Rust code -- no system libraries needed.

## Strategy 2: Vendor C Dependencies

When a pure Rust alternative is not available or not mature enough, you can vendor the C dependency. Vendoring means compiling the C source code as part of your Rust build, producing a static library that gets linked into your binary:

```toml
[dependencies]
# Vendor libgit2: compiles from C source during cargo build
git2 = { version = "0.19", features = ["vendored"] }

# Vendor SQLite: compiles from the amalgamation C file
rusqlite = { version = "0.32", features = ["bundled"] }

# Vendor zlib
flate2 = { version = "1", features = ["zlib-ng"], default-features = false }
```

With vendored builds, the C code is compiled by the `cc` crate using whatever C compiler is available on the build system. For native builds, this is your system's `gcc` or `clang`. For cross-compilation, you need a cross-compiler (which is why `cross` and `cargo-zigbuild` are so useful -- they provide this automatically).

## Strategy 3: Musl libc for Fully Static Linux Binaries

The most impactful step for Linux distribution is switching from glibc to musl libc. Musl is a lightweight, static-linking-friendly libc implementation. When you target `x86_64-unknown-linux-musl`, the Rust compiler statically links musl libc into your binary, producing a truly standalone executable:

```bash
# Add the musl target
rustup target add x86_64-unknown-linux-musl

# Install musl tools (Ubuntu/Debian)
sudo apt install musl-tools

# Build a fully static binary
cargo build --release --target x86_64-unknown-linux-musl

# Verify it is fully static
ldd target/x86_64-unknown-linux-musl/release/my-agent
#   not a dynamic executable

file target/x86_64-unknown-linux-musl/release/my-agent
#   ELF 64-bit LSB executable, x86-64, version 1 (SYSV),
#   statically linked, stripped
```

That `not a dynamic executable` output from `ldd` is what you want to see. This binary runs on any Linux system regardless of the installed glibc version, distribution, or architecture compatibility layer.

### Musl Tradeoffs

Musl is not a drop-in replacement for glibc in all cases. Here are the tradeoffs:

| Aspect | glibc | musl |
|--------|-------|------|
| DNS resolution | Full NSS support (LDAP, NIS, mDNS) | Simple resolver (DNS only) |
| Locale support | Full locale/i18n support | Minimal locale handling |
| Performance | Optimized allocator (ptmalloc2) | Simpler allocator |
| Binary size | Dynamic linking reduces per-binary size | Larger binaries (libc is baked in) |
| Compatibility | Standard on virtually all Linux distros | Runs anywhere, but some edge cases |

For a coding agent, the DNS and locale limitations rarely matter. Your agent makes HTTPS requests to API endpoints (DNS works fine) and processes code files (locale-dependent string sorting is not a concern). The portability benefit far outweighs the edge cases.

### Combining Musl with Pure Rust Dependencies

The ideal configuration for maximum portability combines musl with pure-Rust dependencies:

```toml
[dependencies]
reqwest = { version = "0.12", default-features = false, features = [
    "rustls-tls", "json", "stream"
] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"
```

```bash
cargo build --release --target x86_64-unknown-linux-musl
```

This produces a binary with zero dynamic dependencies. No libc, no libssl, no libcrypto, nothing. Copy it to any x86_64 Linux machine and it runs.

## Verifying Your Binary

Always verify that your release binary is truly self-contained before distributing it. Here are the platform-specific tools:

### Linux

```bash
# Check for dynamic dependencies
ldd target/x86_64-unknown-linux-musl/release/my-agent
# Expected: "not a dynamic executable" or "statically linked"

# Check binary type
file target/x86_64-unknown-linux-musl/release/my-agent
# Expected: "ELF 64-bit LSB executable, ... statically linked"

# List symbols (if not stripped)
nm target/release/my-agent | head -20
```

### macOS

```bash
# Check dynamic library dependencies
otool -L target/release/my-agent
# Minimal: just /usr/lib/libSystem.B.dylib (always needed on macOS)

# Check if the binary is signed
codesign -dv target/release/my-agent
```

On macOS, fully static binaries are not possible -- Apple requires linking to `libSystem.B.dylib`. However, this library is guaranteed to be present on every macOS installation, so it is not a practical concern.

### Windows

```powershell
# Check dependencies using dumpbin (from Visual Studio)
dumpbin /dependents target\release\my-agent.exe
# Expected: KERNEL32.dll, ntdll.dll (always present on Windows)
```

## Automated Verification in CI

Add a verification step to your CI pipeline that catches accidental dynamic dependencies:

```yaml
- name: Verify static binary (Linux musl)
  if: contains(matrix.target, 'musl')
  run: |
    # This fails if the binary has any dynamic dependencies
    ldd target/${{ matrix.target }}/release/my-agent 2>&1 | \
      grep -q "not a dynamic executable" || \
      (echo "ERROR: Binary has dynamic dependencies!" && \
       ldd target/${{ matrix.target }}/release/my-agent && \
       exit 1)

- name: Verify binary type
  run: |
    file target/${{ matrix.target }}/release/my-agent
```

::: wild In the Wild
Production CLI tools in the Rust ecosystem almost universally use musl for Linux distribution. Tools like ripgrep, fd, bat, and delta all ship static musl binaries. The pattern is well-established: musl for Linux portability, pure-Rust TLS for zero OpenSSL dependency, and vendored C libraries for anything that must remain in C.
:::

## A Complete Static Build Script

Here is a build script that produces verified static binaries for Linux:

```bash
#!/bin/bash
set -euo pipefail

BINARY_NAME="my-agent"
TARGET="x86_64-unknown-linux-musl"

echo "Building static binary for ${TARGET}..."
cargo build --release --target "${TARGET}"

BINARY="target/${TARGET}/release/${BINARY_NAME}"

echo "Verifying binary..."
file "${BINARY}"

# Check it is static
if ldd "${BINARY}" 2>&1 | grep -q "not a dynamic executable"; then
    echo "Binary is fully static"
else
    echo "WARNING: Binary has dynamic dependencies:"
    ldd "${BINARY}"
    exit 1
fi

# Show size
ls -lh "${BINARY}"

echo "Build complete: ${BINARY}"
```

## Key Takeaways

- Static linking produces self-contained binaries with zero runtime dependencies -- users download one file and it works.
- Prefer pure Rust alternatives to C libraries: `rustls` over OpenSSL, `gix` over `git2`, `bundled` features for SQLite.
- Target `x86_64-unknown-linux-musl` (or `aarch64-unknown-linux-musl`) for fully static Linux binaries that run on any distribution.
- Musl has minor limitations (simplified DNS resolver, minimal locale support) that rarely affect CLI tools.
- Always verify your binary with `ldd` (Linux), `otool` (macOS), or `dumpbin` (Windows) before distribution to catch accidental dynamic dependencies.
