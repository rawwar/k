---
title: Release Automation
description: Building a CI/CD release pipeline with GitHub Actions that builds, tests, signs, and publishes binaries and creates GitHub Releases on tag push.
---

# Release Automation

> **What you'll learn:**
> - How to design a GitHub Actions workflow that triggers on version tags and produces release artifacts
> - How to build and upload platform-specific binaries and generate checksums for verification
> - Techniques for creating GitHub Releases with auto-generated release notes from commit history

Manual releases are error-prone. You forget to build for one platform, or you upload the wrong binary, or you forget to update the Homebrew formula. A good release pipeline handles all of this automatically: push a Git tag, and everything else happens without human intervention. GitHub Actions is the natural choice for projects hosted on GitHub, and in this subchapter you will build a complete release workflow.

## The Release Workflow Overview

Here is the flow triggered by pushing a version tag:

1. **Tag push** (`v0.1.0`) triggers the workflow.
2. **Test** -- run the full test suite to catch last-minute issues.
3. **Build** -- cross-compile for all target platforms.
4. **Package** -- create tarballs and generate SHA256 checksums.
5. **Release** -- create a GitHub Release with binaries, checksums, and release notes.
6. **Notify** -- update the Homebrew formula and optionally publish to crates.io.

## The Complete GitHub Actions Workflow

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v[0-9]+.*'

permissions:
  contents: write

env:
  CARGO_TERM_COLOR: always
  BINARY_NAME: agent

jobs:
  # First, run tests to make sure everything passes
  test:
    name: Test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --all-features

  # Build for each target platform
  build:
    name: Build ${{ matrix.target }}
    needs: test
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: x86_64-unknown-linux-musl
            os: ubuntu-latest
            use_cross: true
          - target: aarch64-unknown-linux-musl
            os: ubuntu-latest
            use_cross: true
          - target: x86_64-apple-darwin
            os: macos-latest
            use_cross: false
          - target: aarch64-apple-darwin
            os: macos-latest
            use_cross: false
          - target: x86_64-pc-windows-msvc
            os: windows-latest
            use_cross: false

    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.target }}

      # Install cross for Linux builds
      - name: Install cross
        if: matrix.use_cross
        run: cargo install cross --git https://github.com/cross-rs/cross

      # Build the binary
      - name: Build
        run: |
          if [ "${{ matrix.use_cross }}" = "true" ]; then
            cross build --release --target ${{ matrix.target }}
          else
            cargo build --release --target ${{ matrix.target }}
          fi
        shell: bash

      # Package the binary
      - name: Package (Unix)
        if: runner.os != 'Windows'
        run: |
          cd target/${{ matrix.target }}/release
          tar czf ../../../${{ env.BINARY_NAME }}-${{ matrix.target }}.tar.gz ${{ env.BINARY_NAME }}
        shell: bash

      - name: Package (Windows)
        if: runner.os == 'Windows'
        run: |
          cd target/${{ matrix.target }}/release
          7z a ../../../${{ env.BINARY_NAME }}-${{ matrix.target }}.zip ${{ env.BINARY_NAME }}.exe
        shell: bash

      # Upload the artifact for the release job
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ env.BINARY_NAME }}-${{ matrix.target }}
          path: |
            ${{ env.BINARY_NAME }}-${{ matrix.target }}.tar.gz
            ${{ env.BINARY_NAME }}-${{ matrix.target }}.zip
          if-no-files-found: error

  # Create the GitHub Release
  release:
    name: Create Release
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0  # Full history for release notes

      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts

      - name: Collect release files
        run: |
          mkdir release-files
          find artifacts -type f \( -name "*.tar.gz" -o -name "*.zip" \) \
            -exec mv {} release-files/ \;
          ls -la release-files/

      - name: Generate checksums
        run: |
          cd release-files
          shasum -a 256 * > checksums-sha256.txt
          cat checksums-sha256.txt

      - name: Get version from tag
        id: version
        run: echo "VERSION=${GITHUB_REF#refs/tags/v}" >> $GITHUB_OUTPUT

      - name: Generate release notes
        id: notes
        run: |
          # Get commits since the last tag
          PREV_TAG=$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || echo "")
          if [ -n "$PREV_TAG" ]; then
            NOTES=$(git log --pretty=format:"- %s (%h)" "$PREV_TAG"..HEAD)
          else
            NOTES=$(git log --pretty=format:"- %s (%h)" HEAD~10..HEAD)
          fi
          # Write to a file to handle multiline content
          echo "$NOTES" > release-notes.md

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          name: v${{ steps.version.outputs.VERSION }}
          body_path: release-notes.md
          files: release-files/*
          draft: false
          prerelease: ${{ contains(github.ref, '-rc') || contains(github.ref, '-beta') }}
```

::: python Coming from Python
Python release pipelines typically involve `twine upload` to PyPI and building wheels for different platforms. Rust's release pipeline is more involved because you are distributing native binaries -- there is no runtime to handle platform differences for you. The upside is that your users get a zero-dependency binary that starts instantly, unlike Python tools that may require installing Python itself and managing virtual environments.
:::

## Adding Homebrew Formula Updates

Extend the release workflow with a job that updates your Homebrew tap:

```yaml
  # Update Homebrew formula
  homebrew:
    name: Update Homebrew
    needs: release
    runs-on: ubuntu-latest
    steps:
      - name: Download checksums
        uses: actions/download-artifact@v4
        with:
          path: artifacts

      - name: Compute checksums
        id: checksums
        run: |
          cd artifacts
          # Find the checksum file from the release
          for target in \
            "aarch64-apple-darwin" \
            "x86_64-apple-darwin" \
            "aarch64-unknown-linux-musl" \
            "x86_64-unknown-linux-musl"; do
            file="agent-${target}/agent-${target}.tar.gz"
            if [ -f "$file" ]; then
              sha=$(shasum -a 256 "$file" | awk '{print $1}')
              echo "${target}_SHA256=${sha}" >> $GITHUB_OUTPUT
            fi
          done

      - name: Get version from tag
        id: version
        run: echo "VERSION=${GITHUB_REF#refs/tags/v}" >> $GITHUB_OUTPUT

      - name: Update Homebrew formula
        uses: mislav/bump-homebrew-formula-action@v3
        with:
          formula-name: agent
          homebrew-tap: yourusername/homebrew-agent
          tag-name: ${{ github.ref_name }}
          download-url: https://github.com/${{ github.repository }}/releases/download/${{ github.ref_name }}/agent-x86_64-apple-darwin.tar.gz
        env:
          COMMITTER_TOKEN: ${{ secrets.HOMEBREW_TAP_TOKEN }}
```

The `HOMEBREW_TAP_TOKEN` secret needs to be a GitHub personal access token with `repo` scope that can push to your `homebrew-agent` repository.

## Continuous Integration for Every Push

Beyond release automation, you want a CI workflow that runs on every push and pull request:

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  check:
    name: Check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo check --all-features

  test:
    name: Test
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --all-features

  fmt:
    name: Format
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --all -- --check

  clippy:
    name: Clippy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy --all-features -- -D warnings
```

This workflow catches formatting issues, linting violations, test failures, and compilation errors before code reaches the main branch.

## Triggering a Release

With the pipeline in place, releasing a new version is a single command:

```bash
# Tag the release
git tag v0.1.0
git push origin v0.1.0
```

The workflow automatically:
1. Runs the test suite.
2. Builds binaries for Linux (x86_64, ARM64), macOS (Intel, Apple Silicon), and Windows.
3. Creates tarballs with checksums.
4. Publishes a GitHub Release with auto-generated release notes.
5. Updates the Homebrew formula.

If any step fails, the release is not published, and you get a notification to investigate.

::: wild In the Wild
Claude Code's release process is automated through Anthropic's internal CI systems. OpenCode uses a similar GitHub Actions approach to build multi-platform binaries on every tagged release. The `cargo-dist` tool is an emerging option that automates much of this setup -- it generates GitHub Actions workflows and Homebrew formulas from your `Cargo.toml` metadata, reducing the boilerplate to a single `cargo dist init` command.
:::

## Key Takeaways

- Trigger release workflows on version tag pushes (`v*`) rather than manual triggers -- this makes the release process a single `git tag` + `git push` command.
- Use a build matrix to compile for all target platforms in parallel, combining `cross` for Linux targets and native builds for macOS and Windows.
- Generate SHA256 checksums for every release artifact and include them in the GitHub Release, giving users a way to verify download integrity.
- Maintain a separate CI workflow for every push and pull request that checks formatting, linting, and runs tests on multiple operating systems.
- Automate Homebrew formula updates as part of the release pipeline so that `brew upgrade` picks up new versions without manual intervention.
