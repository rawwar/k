---
title: Summary
description: Review the complete packaging and distribution pipeline from optimized builds to auto-updating production binaries.
---

# Summary

> **What you'll learn:**
> - How release builds, cross-compilation, static linking, and packaging compose into a complete distribution pipeline
> - A checklist for production releases covering binary verification, platform testing, update path validation, and rollback readiness
> - How to maintain the distribution pipeline as new platforms, package managers, and compliance requirements emerge

You have now covered the entire journey from `cargo build` to a production distribution pipeline. Let's step back and see how all the pieces connect, walk through a production release checklist, and discuss how to maintain this pipeline over time.

## The Complete Pipeline

Here is how every topic in this chapter fits into the release pipeline, from source code to a binary running on a user's machine:

**Stage 1: Build**
You start with the release profile configuration from [subchapter 1](/linear/17-packaging-and-distribution/01-release-builds). Your `Cargo.toml` specifies `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, and `strip = "symbols"` to produce the smallest, fastest binary possible. The `vergen` build script embeds the git commit hash and build timestamp for traceability.

**Stage 2: Cross-Compile**
A CI matrix build from [subchapter 2](/linear/17-packaging-and-distribution/02-cross-compilation) compiles for every target platform. Native runners handle macOS and Windows builds; `cross` or `cargo-zigbuild` handles Linux ARM64 from x86_64 runners. Pure Rust dependencies (`rustls`, vendored `git2`) from [subchapter 3](/linear/17-packaging-and-distribution/03-static-linking) ensure the builds succeed without platform-specific system libraries.

**Stage 3: Verify**
Each binary is checked with `ldd`, `otool`, or `dumpbin` to confirm it has no unexpected dynamic dependencies. Musl builds are verified as fully static. The binary's `--version` output confirms the correct version and commit hash are embedded.

**Stage 4: Package**
Binaries are packaged into platform-appropriate archives (`tar.gz` for Unix, `zip` for Windows) alongside shell completions, man pages, README, and LICENSE. SHA256 checksums are computed for every archive.

**Stage 5: Publish**
A GitHub Release is created with all archives and checksums attached, as described in [subchapter 6](/linear/17-packaging-and-distribution/06-binary-releases). The Homebrew tap formula from [subchapter 5](/linear/17-packaging-and-distribution/05-homebrew-distribution) is automatically updated with new URLs and hashes. The crate is published to crates.io per [subchapter 4](/linear/17-packaging-and-distribution/04-cargo-install).

**Stage 6: Install**
Users install through whichever channel they prefer: `brew install`, the install script, `cargo install`, or manual download from GitHub Releases.

**Stage 7: Run**
On first run, the agent loads layered configuration from [subchapter 7](/linear/17-packaging-and-distribution/07-config-file-management), respecting global, project, and environment variable settings. The telemetry disclosure from [subchapter 9](/linear/17-packaging-and-distribution/09-telemetry-considerations) presents the opt-in prompt if this is the first launch.

**Stage 8: Update**
A background version check from [subchapter 8](/linear/17-packaging-and-distribution/08-auto-updates) queries GitHub Releases for the latest version. When an update is available, the user is notified (or the update is applied automatically, depending on their preference). The cycle returns to Stage 1 for the next release.

::: python Coming from Python
Compare this to distributing a Python CLI tool. You would need: `pyproject.toml` with build system metadata, a `setup.py` or `setup.cfg` for legacy compatibility, `cibuildwheel` or `manylinux` containers for platform-specific wheels, `twine` to upload to PyPI, `virtualenv` or `pipx` for isolated installation, a separate mechanism for shell completions, and usually a third-party tool for auto-updates. The Rust pipeline is more steps at the CI level, but the end result is dramatically simpler: one file per platform, no runtime, no dependency resolution.
:::

## Production Release Checklist

Use this checklist before every release. Automate as many steps as possible, but have a human review the output:

### Pre-Release

- [ ] All tests pass on the main branch (`cargo test`)
- [ ] Version number is bumped in `Cargo.toml` following semver
- [ ] CHANGELOG or release notes are drafted
- [ ] Breaking changes (if any) are documented with migration instructions
- [ ] `cargo publish --dry-run` succeeds

### Build and Package

- [ ] CI matrix build completes for all targets (macOS x86_64, macOS aarch64, Linux x86_64 glibc, Linux x86_64 musl, Linux aarch64 musl, Windows x86_64)
- [ ] All binaries pass static linking verification (`ldd`, `otool`, `dumpbin`)
- [ ] Binary size is within expected range (no accidental debug info)
- [ ] `--version` output shows correct version and commit hash
- [ ] Archives contain binary, completions, man page, README, LICENSE
- [ ] SHA256 checksums are generated and match the archives

### Publish

- [ ] GitHub Release is created with all archives and checksums
- [ ] Release notes accurately describe changes
- [ ] Pre-release tag is used for release candidates (`v0.6.0-rc1`)
- [ ] Homebrew tap formula is updated with correct URLs and hashes
- [ ] `cargo publish` succeeds
- [ ] Install script downloads the correct version

### Post-Release Verification

- [ ] `brew install yourname/tap/my-agent` works on macOS
- [ ] Install script works on a fresh Linux machine
- [ ] `cargo install my-agent` builds successfully
- [ ] Auto-update correctly detects the new version from the previous release
- [ ] Configuration migration works when upgrading from the previous version
- [ ] Telemetry events (if enabled) are received at the collection endpoint

### Rollback Readiness

- [ ] Previous release archives are still available on GitHub
- [ ] Homebrew formula can be pinned to the previous version: `brew pin my-agent`
- [ ] Users can downgrade with: `brew install yourname/tap/my-agent@0.5.2`

## Automating the Release

Combine the checklist into a release script that handles the common cases:

```bash
#!/bin/bash
set -euo pipefail

VERSION="${1:?Usage: release.sh <version>}"

echo "=== Releasing v${VERSION} ==="

# Verify we are on main and clean
git diff --quiet || (echo "Working directory is dirty" && exit 1)
BRANCH=$(git branch --show-current)
[ "$BRANCH" = "main" ] || (echo "Not on main branch" && exit 1)

# Bump version
cargo set-version "${VERSION}"

# Run tests
cargo test

# Dry run publish
cargo publish --dry-run

# Commit and tag
git add Cargo.toml Cargo.lock
git commit -m "release: v${VERSION}"
git tag "v${VERSION}"

# Push (triggers CI release workflow)
git push origin main
git push origin "v${VERSION}"

echo "=== Tag pushed. CI will build and publish. ==="
echo "Monitor: https://github.com/yourname/my-agent/actions"
```

The tag push triggers the CI workflow that builds all binaries, creates the GitHub Release, updates the Homebrew tap, and publishes to crates.io. The human's job is to write release notes and verify the post-release checklist.

## Maintaining the Pipeline

A distribution pipeline is not a set-and-forget system. Here are the ongoing maintenance tasks:

### New Platforms

When a new platform becomes popular (RISC-V Linux, Windows ARM64), add it to the CI matrix:

```yaml
- target: riscv64gc-unknown-linux-gnu
  os: ubuntu-latest
  use_cross: true
```

Add the corresponding block to the Homebrew formula and update the install script's platform detection.

### Dependency Updates

Keep your CI actions, cross-compilation tools, and Rust toolchain current. Pin GitHub Actions to specific versions and update them regularly:

```yaml
# Pin to a specific version, not @main
- uses: dtolnay/rust-toolchain@stable
- uses: actions/upload-artifact@v4
```

### Package Manager Changes

Homebrew occasionally changes its formula DSL or API. Subscribe to the Homebrew announcements and test your formula after major Homebrew releases.

### Security Response

When a security vulnerability is discovered:

1. Fix the vulnerability on a private branch
2. Prepare builds for all platforms
3. Publish the fix and a security advisory simultaneously
4. The auto-update system ensures users are notified immediately
5. If the vulnerability is critical, consider pushing a force notification regardless of the user's update preference

::: wild In the Wild
Production coding agents like Claude Code maintain robust release pipelines that can ship updates quickly. The ability to push a security fix to users within hours (not days or weeks) is a significant advantage of the single-binary distribution model. When every user runs a self-contained binary with auto-update capabilities, the time from "fix merged" to "fix deployed" is as fast as your CI pipeline.
:::

## What You Have Built

Over the course of this chapter, you have assembled a complete distribution system:

| Component | Purpose | Subchapter |
|-----------|---------|------------|
| Release profile | Optimized binary builds | [1](/linear/17-packaging-and-distribution/01-release-builds) |
| Cross-compilation | Multi-platform support | [2](/linear/17-packaging-and-distribution/02-cross-compilation) |
| Static linking | Zero runtime dependencies | [3](/linear/17-packaging-and-distribution/03-static-linking) |
| crates.io | Source-based distribution | [4](/linear/17-packaging-and-distribution/04-cargo-install) |
| Homebrew tap | macOS/Linux package manager | [5](/linear/17-packaging-and-distribution/05-homebrew-distribution) |
| GitHub Releases | Binary download hosting | [6](/linear/17-packaging-and-distribution/06-binary-releases) |
| Config management | User customization | [7](/linear/17-packaging-and-distribution/07-config-file-management) |
| Auto-updates | Version currency | [8](/linear/17-packaging-and-distribution/08-auto-updates) |
| Telemetry | Product insights | [9](/linear/17-packaging-and-distribution/09-telemetry-considerations) |

This is the full picture. Your coding agent is no longer a project that "works on my machine." It is a distributable product that installs cleanly, updates itself, respects user preferences, and runs on every major platform. That transformation -- from development artifact to production software -- is what this chapter is about, and it is where Rust's single-binary, no-runtime model pays its biggest dividends.

## Exercises

### Exercise 1: Cross-Compilation Strategy for a New Target (Easy)

Your agent needs to support Linux on ARM64 (aarch64) for deployment on AWS Graviton instances. Walk through the decisions you need to make: which Rust target triple to use, whether to use `cross`, `cargo-zigbuild`, or a native ARM64 runner, how to handle the OpenSSL dependency (vendored `rustls` vs. cross-compiled OpenSSL), and how to verify the resulting binary works. What would you add to your CI matrix, and how would you test the binary if you only have x86_64 CI runners?

### Exercise 2: Binary Size Optimization Analysis (Medium)

Your agent's release binary is 45MB, and you want to get it under 20MB for faster downloads and auto-updates. List every optimization technique from this chapter (LTO, codegen-units, panic=abort, strip, opt-level) and estimate the size reduction each provides. Beyond compiler settings, identify three architectural changes that could reduce binary size: removing unused features via Cargo features, replacing heavy dependencies with lighter alternatives, and using `UPX` compression. For each approach, discuss the trade-off -- what do you lose in exchange for the smaller binary? At what binary size is further optimization not worth the effort?

### Exercise 3: Update Mechanism Design (Hard)

Design a self-update mechanism for your agent that handles: (a) checking for updates without blocking startup (background check), (b) downloading the new binary while the current session continues, (c) atomically replacing the running binary (write-to-temp-then-rename), (d) verifying the new binary's integrity (SHA256 checksum), and (e) rolling back if the new version crashes on first launch. Consider platform-specific challenges: on Windows, you cannot replace a running binary; on macOS, Gatekeeper may quarantine the downloaded binary. How do you handle the case where the user installed via Homebrew -- should your self-update mechanism defer to `brew upgrade`?

### Exercise 4: Installation UX Audit (Medium)

Evaluate the first-run experience for three installation methods: `brew install`, the shell install script (`curl | sh`), and `cargo install`. For each method, document: how long installation takes on a fresh machine, what prerequisites the user needs (Xcode CLI tools, Rust toolchain, nothing), what error messages the user sees if something goes wrong, and how the user discovers that the tool was installed successfully. Design an ideal first-run experience that detects the user's shell, installs completions, presents a brief getting-started guide, and runs a diagnostic check. What information should `my-agent doctor` verify?

## Key Takeaways

- The distribution pipeline flows through eight stages: build, cross-compile, verify, package, publish, install, run, and update -- each stage builds on the previous chapter topics.
- Automate as much of the release process as possible (CI builds, Homebrew updates, checksum generation) but keep a human in the loop for release notes, version decisions, and post-release verification.
- Maintain the pipeline actively: add new platform targets as they become popular, keep CI dependencies current, and test the install path after package manager updates.
- The single-binary distribution model is Rust's greatest practical advantage over interpreted languages -- embrace it fully with static linking, cross-compilation, and auto-updates.
- Plan for security response from day one: the ability to push a fix to all users quickly depends on having a reliable, automated release pipeline already in place.
