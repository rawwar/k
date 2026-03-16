---
title: Version Management
description: Implementing semantic versioning with automated version bumps, ensuring Cargo.toml, CLI output, and documentation stay in sync across releases.
---

# Version Management

> **What you'll learn:**
> - How to implement semantic versioning and decide when to bump major, minor, or patch versions
> - Techniques for keeping version numbers in Cargo.toml, CLI --version output, and docs in sync
> - How to automate version bumps with cargo-release or custom scripts in the CI pipeline

Version management seems simple until you have a version number in `Cargo.toml`, a `--version` flag in the CLI, a changelog heading, a Homebrew formula, and documentation pages that all need to agree. One mismatch and users lose trust. In this subchapter, you will set up a versioning workflow that keeps everything synchronized, from the initial commit through automated releases.

## Semantic Versioning for CLI Tools

[Semantic Versioning](https://semver.org/) (SemVer) uses a three-part version number: `MAJOR.MINOR.PATCH`.

For a CLI tool like your coding agent, the rules are:

| Bump | When | Examples |
|---|---|---|
| **PATCH** (0.1.0 -> 0.1.1) | Bug fixes, performance improvements | Fix timeout handling, fix config parsing |
| **MINOR** (0.1.0 -> 0.2.0) | New features, backward-compatible changes | Add new tool, add new provider, new CLI flag |
| **MAJOR** (0.2.0 -> 1.0.0) | Breaking changes | Change config format, rename CLI flags, remove features |

During early development (0.x.y), minor version bumps can include breaking changes. Once you release 1.0.0, you commit to stability.

::: python Coming from Python
Python uses PEP 440 for versioning, which supports SemVer plus additional specifiers like `.dev`, `.post`, and release candidates. Rust's Cargo strictly enforces SemVer for library crates (it uses SemVer compatibility for dependency resolution). For a binary crate like your agent, the version is more of a communication tool than a technical requirement -- but following SemVer builds trust with your users.
:::

## The Single Source of Truth

The version number lives in `Cargo.toml`. Everything else derives from it.

```toml
# Cargo.toml
[package]
name = "agent"
version = "0.2.0"
```

Your CLI already reads this automatically through clap's `#[command(version)]` attribute, which reads `CARGO_PKG_VERSION` at compile time:

```rust
use clap::Parser;

#[derive(Parser)]
#[command(version)]
pub struct Cli {
    // ... fields
}
```

Running `agent --version` outputs:

```
agent 0.2.0
```

If you have a build script that adds Git hash and build date (from the packaging subchapter), the version string is richer:

```rust
#[derive(Parser)]
#[command(
    version = format!(
        "{} ({} {})",
        env!("CARGO_PKG_VERSION"),
        env!("GIT_HASH"),
        env!("BUILD_DATE"),
    ),
)]
pub struct Cli {
    // ...
}
```

Output: `agent 0.2.0 (a3f2b1c 2026-03-16)`

## Automating Version Bumps with cargo-release

The `cargo-release` tool automates the entire release process: version bump, changelog update, Git tag, and crate publish.

```bash
cargo install cargo-release
```

Configure it in `Cargo.toml`:

```toml
[package.metadata.release]
# Files to search and replace version strings in
pre-release-replacements = [
    { file = "CHANGELOG.md", search = "## \\[Unreleased\\]", replace = "## [Unreleased]\n\n## [{{version}}] - {{date}}" },
    { file = "docs/src/installation.md", search = "version = \".*\"", replace = "version = \"{{version}}\"" },
]

# Commit message format
pre-release-commit-message = "chore: release v{{version}}"
tag-message = "v{{version}}"
tag-name = "v{{version}}"

# Publish to crates.io
publish = true

# Push the tag to trigger the release workflow
push-remote = "origin"
```

Now releasing is a single command:

```bash
# Bump patch version (0.2.0 -> 0.2.1)
cargo release patch

# Bump minor version (0.2.0 -> 0.3.0)
cargo release minor

# Bump major version (0.2.0 -> 1.0.0)
cargo release major

# Dry run to preview what would happen
cargo release patch --dry-run
```

`cargo release patch` does the following:

1. Bumps the version in `Cargo.toml` from 0.2.0 to 0.2.1.
2. Updates version references in `CHANGELOG.md` and documentation.
3. Commits the changes with message "chore: release v0.2.1".
4. Creates a Git tag `v0.2.1`.
5. Pushes the commit and tag to the remote.
6. Publishes to crates.io.

The tag push triggers your GitHub Actions release workflow, which builds binaries and creates the GitHub Release.

## Version Checking in Code

Sometimes you need to check the version at runtime, for example to include it in API request headers or telemetry:

```rust
/// Get the version string for API headers and logging.
pub fn version_string() -> String {
    format!("agent/{}", env!("CARGO_PKG_VERSION"))
}

/// Get detailed version information for diagnostics.
pub fn version_info() -> VersionInfo {
    VersionInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        git_hash: option_env!("GIT_HASH")
            .unwrap_or("unknown")
            .to_string(),
        build_date: option_env!("BUILD_DATE")
            .unwrap_or("unknown")
            .to_string(),
        target: env!("TARGET").to_string(),
        rust_version: env!("CARGO_PKG_RUST_VERSION")
            .to_string(),
    }
}

pub struct VersionInfo {
    pub version: String,
    pub git_hash: String,
    pub build_date: String,
    pub target: String,
    pub rust_version: String,
}

impl std::fmt::Display for VersionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "agent {}", self.version)?;
        writeln!(f, "git: {}", self.git_hash)?;
        writeln!(f, "built: {}", self.build_date)?;
        writeln!(f, "target: {}", self.target)?;
        write!(f, "rust: {}", self.rust_version)
    }
}
```

Add a `--version-verbose` flag or a `version` subcommand that shows the full details:

```rust
#[derive(Subcommand)]
pub enum Commands {
    /// Show detailed version information
    Version,
    // ... other subcommands
}

// In the command handler:
fn handle_version() {
    println!("{}", version_info());
}
```

Output:

```
agent 0.2.0
git: a3f2b1c
built: 2026-03-16
target: aarch64-apple-darwin
rust: 1.85.0
```

## Pre-release Versions

Use pre-release identifiers for testing releases before they go to the general audience:

```bash
# Release a beta version
# Manually set in Cargo.toml: version = "0.3.0-beta.1"
cargo release --tag-name "v0.3.0-beta.1" --no-publish

# Release a release candidate
# version = "0.3.0-rc.1"
cargo release --tag-name "v0.3.0-rc.1" --no-publish
```

Your release workflow already handles this -- remember the `prerelease` flag in the GitHub Release creation:

```yaml
prerelease: ${{ contains(github.ref, '-rc') || contains(github.ref, '-beta') }}
```

Beta and RC releases show up on the GitHub Releases page with a "Pre-release" badge, and `brew upgrade` will not install them unless the user explicitly opts in.

## The Complete Release Checklist

Here is the full release process, from decision to deployment:

```bash
# 1. Ensure all tests pass
cargo test --all-features

# 2. Ensure formatting and linting are clean
cargo fmt -- --check
cargo clippy --all-features -- -D warnings

# 3. Review the unreleased changes
git-cliff --unreleased

# 4. Decide the version bump based on changes
#    (patch for fixes, minor for features, major for breaking)

# 5. Run the release (dry-run first)
cargo release minor --dry-run

# 6. If everything looks good, run for real
cargo release minor --execute

# 7. Verify the GitHub Actions workflow runs successfully
gh run list --workflow=release.yml

# 8. Verify the Homebrew formula was updated
brew update && brew info yourusername/agent/agent

# 9. Verify the documentation was deployed
open https://yourusername.github.io/agent/
```

::: wild In the Wild
Claude Code follows Anthropic's internal versioning and release cadence. OpenCode uses Git tags and GitHub Releases for version management. The `cargo-release` tool is used by many prominent Rust projects including `serde`, `tokio`, and `clap` themselves. The key lesson from all these projects is that automation removes the human error from releases -- the fewer manual steps, the fewer things that can go wrong.
:::

## Key Takeaways

- Follow Semantic Versioning (MAJOR.MINOR.PATCH) and use `Cargo.toml` as the single source of truth for the version number -- all other references derive from it at compile time through `env!("CARGO_PKG_VERSION")`.
- Use `cargo-release` to automate the entire release process: version bump in `Cargo.toml`, changelog update, Git tag creation, remote push, and crate publish -- all in a single command.
- Configure `pre-release-replacements` in `cargo-release` to automatically update version references in changelogs, documentation, and other files when bumping versions.
- Include detailed version information (Git hash, build date, target platform, Rust version) accessible through a `--version` flag or `version` subcommand, giving users and support teams the context they need for debugging.
- Use pre-release identifiers (`-beta.1`, `-rc.1`) for testing releases, and mark them as pre-releases in GitHub so `brew upgrade` and stable installation methods skip them.
