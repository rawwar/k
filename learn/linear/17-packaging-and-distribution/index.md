---
title: "Chapter 17: Packaging and Distribution"
description: Ship your coding agent as a polished binary product, covering release builds, cross-compilation, package managers, auto-updates, and telemetry.
---

# Packaging and Distribution

You have built a coding agent that works on your machine. Now it needs to work on everyone else's machine too. This chapter covers the entire journey from development build to production distribution: optimizing release builds, cross-compiling for multiple platforms, static linking for dependency-free binaries, and packaging for distribution through Homebrew, cargo install, and GitHub releases.

We also tackle the ongoing concerns that arise after the initial release. Configuration file management ensures users can customize the agent without conflicts across updates. Auto-update mechanisms keep users on the latest version without manual intervention. Telemetry -- done respectfully -- gives you the data you need to improve the product while respecting user privacy.

Each topic addresses the specific challenges of distributing a Rust binary that depends on system libraries (OpenSSL, libgit2), needs to work across macOS, Linux, and Windows, and must handle graceful upgrades without disrupting active sessions. By the end, you will have a complete release pipeline that produces professional, installable binaries for all major platforms.

::: python Coming from Python
If you have ever tried to distribute a Python CLI tool, you know the pain: creating wheels, dealing with `manylinux` compatibility, bundling native extensions, writing `setup.py` or `pyproject.toml`, uploading to PyPI, hoping the user has the right Python version, creating virtual environments, and praying that `pip install` doesn't hit a C extension compilation error on the user's machine. Rust eliminates almost all of this. You compile once per platform and ship a single binary with zero runtime dependencies. No interpreter, no virtual environment, no dependency resolution at install time. This chapter is where Rust's practical advantage over Python becomes most dramatically clear.
:::

## Learning Objectives
- Configure Cargo for optimized release builds with appropriate LTO, codegen, and stripping settings
- Cross-compile Rust binaries for macOS (Intel and Apple Silicon), Linux (glibc and musl), and Windows
- Set up static linking to produce self-contained binaries without runtime dependencies
- Package and distribute through Homebrew taps, cargo install, and GitHub binary releases
- Implement platform-aware configuration file management using XDG conventions and OS-specific paths
- Implement an auto-update system that checks for and applies updates without disrupting the user
- Design privacy-respecting telemetry that provides actionable product insights

## Subchapters
1. [Release Builds](/linear/17-packaging-and-distribution/01-release-builds)
2. [Cross Compilation](/linear/17-packaging-and-distribution/02-cross-compilation)
3. [Static Linking](/linear/17-packaging-and-distribution/03-static-linking)
4. [Cargo Install](/linear/17-packaging-and-distribution/04-cargo-install)
5. [Homebrew Distribution](/linear/17-packaging-and-distribution/05-homebrew-distribution)
6. [Binary Releases](/linear/17-packaging-and-distribution/06-binary-releases)
7. [Config File Management](/linear/17-packaging-and-distribution/07-config-file-management)
8. [Auto Updates](/linear/17-packaging-and-distribution/08-auto-updates)
9. [Telemetry Considerations](/linear/17-packaging-and-distribution/09-telemetry-considerations)
10. [Summary](/linear/17-packaging-and-distribution/10-summary)

## Prerequisites
- Chapter 2 (Rust fundamentals and Cargo basics)
- Familiarity with Cargo workspace configuration and build profiles
- A working coding agent binary from previous chapters
