---
title: Binary Releases
description: Automate the creation and publication of prebuilt binary releases on GitHub with checksums, signatures, and platform-specific archives.
---

# Binary Releases

> **What you'll learn:**
> - How to set up GitHub Actions workflows that build, package, and publish binary releases for all platforms on git tag push
> - Techniques for creating platform-appropriate archives (tar.gz for Unix, zip for Windows) with checksums and optional GPG signatures
> - How to write install scripts (curl-pipe-bash style) that detect the user's platform and download the correct binary automatically

Prebuilt binary releases are the primary distribution mechanism for production CLI tools. Users do not want to install a Rust toolchain, wait for compilation, or debug build failures. They want to download a binary and run it. GitHub Releases provide the infrastructure for this: host platform-specific archives, attach checksums for verification, and let users download from a stable URL. This subchapter covers the complete pipeline from git tag to published release.

## The Release Pipeline Overview

The typical release pipeline for a Rust CLI tool follows these steps:

1. You push a git tag (`v0.5.2`)
2. CI triggers a matrix build across all target platforms
3. Each build produces an optimized binary (using the release profile from subchapter 1)
4. Binaries are packaged into platform-appropriate archives with completions and docs
5. SHA256 checksums are computed for every archive
6. A GitHub Release is created with all archives and checksums attached
7. Downstream distribution (Homebrew tap, install scripts) is updated automatically

Let's build this pipeline step by step.

## Packaging Binaries

Before uploading to GitHub Releases, package each binary into an archive with supporting files. The convention is `tar.gz` for Unix and `zip` for Windows:

```bash
#!/bin/bash
set -euo pipefail

BINARY_NAME="my-agent"
VERSION="${1:?Usage: package.sh <version>}"
TARGET="${2:?Usage: package.sh <version> <target>}"

BINARY="target/${TARGET}/release/${BINARY_NAME}"
if [[ "${TARGET}" == *"windows"* ]]; then
    BINARY="${BINARY}.exe"
fi

ARCHIVE_DIR="${BINARY_NAME}-${VERSION}-${TARGET}"
mkdir -p "${ARCHIVE_DIR}"

# Copy binary
cp "${BINARY}" "${ARCHIVE_DIR}/"

# Copy supporting files
cp README.md LICENSE "${ARCHIVE_DIR}/"

# Copy shell completions (if they exist)
if [ -d "completions" ]; then
    cp -r completions "${ARCHIVE_DIR}/"
fi

# Copy man page (if it exists)
if [ -d "man" ]; then
    cp -r man "${ARCHIVE_DIR}/"
fi

# Create archive
if [[ "${TARGET}" == *"windows"* ]]; then
    zip -r "${ARCHIVE_DIR}.zip" "${ARCHIVE_DIR}"
    echo "Created ${ARCHIVE_DIR}.zip"
else
    tar czf "${ARCHIVE_DIR}.tar.gz" "${ARCHIVE_DIR}"
    echo "Created ${ARCHIVE_DIR}.tar.gz"
fi

# Clean up
rm -rf "${ARCHIVE_DIR}"
```

The archive contents look like:

```
my-agent-0.5.2-x86_64-unknown-linux-musl/
  my-agent                    # The binary
  README.md                   # Project readme
  LICENSE                     # License file
  completions/
    my-agent.bash             # Bash completions
    _my-agent                 # Zsh completions
    my-agent.fish             # Fish completions
  man/
    my-agent.1                # Man page
```

::: python Coming from Python
Python distribution archives (wheels and sdists) have a strict format defined by PEP standards and include metadata files, dependency declarations, and platform tags. Rust release archives are simpler: just the binary and whatever supporting files you choose to include. There is no runtime dependency resolution, no package metadata format, and no compatibility tag negotiation. The binary runs or it does not.
:::

## SHA256 Checksums

Every release should include a checksum file that users can verify against. This catches corrupted downloads and provides a basic tamper detection mechanism:

```bash
# Generate checksums for all archives
shasum -a 256 my-agent-0.5.2-*.tar.gz my-agent-0.5.2-*.zip > SHA256SUMS.txt
```

The resulting file looks like:

```
a1b2c3d4...  my-agent-0.5.2-aarch64-apple-darwin.tar.gz
e5f6a7b8...  my-agent-0.5.2-x86_64-apple-darwin.tar.gz
c9d0e1f2...  my-agent-0.5.2-x86_64-unknown-linux-musl.tar.gz
a3b4c5d6...  my-agent-0.5.2-aarch64-unknown-linux-musl.tar.gz
e7f8a9b0...  my-agent-0.5.2-x86_64-pc-windows-msvc.zip
```

Users verify a download with:

```bash
# Download the binary and checksums
curl -LO https://github.com/yourname/my-agent/releases/download/v0.5.2/my-agent-0.5.2-x86_64-unknown-linux-musl.tar.gz
curl -LO https://github.com/yourname/my-agent/releases/download/v0.5.2/SHA256SUMS.txt

# Verify
shasum -a 256 -c SHA256SUMS.txt --ignore-missing
# my-agent-0.5.2-x86_64-unknown-linux-musl.tar.gz: OK
```

## The Complete GitHub Actions Workflow

Here is a production-grade release workflow that builds, packages, and publishes for all platforms:

```yaml
name: Release

on:
  push:
    tags: ["v*"]

permissions:
  contents: write  # Required to create GitHub Releases

env:
  BINARY_NAME: my-agent

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: aarch64-apple-darwin
            os: macos-latest
          - target: x86_64-apple-darwin
            os: macos-latest
          - target: x86_64-unknown-linux-musl
            os: ubuntu-latest
          - target: aarch64-unknown-linux-musl
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
        if: matrix.target == 'x86_64-unknown-linux-musl'
        run: sudo apt install -y musl-tools

      - name: Build
        shell: bash
        run: |
          if [ "${{ matrix.use_cross }}" = "true" ]; then
            cross build --release --target ${{ matrix.target }}
          else
            cargo build --release --target ${{ matrix.target }}
          fi

      - name: Package
        shell: bash
        run: |
          VERSION="${GITHUB_REF_NAME#v}"
          TARGET="${{ matrix.target }}"
          ARCHIVE_DIR="${BINARY_NAME}-${VERSION}-${TARGET}"
          mkdir -p "${ARCHIVE_DIR}"

          if [[ "${TARGET}" == *"windows"* ]]; then
            cp "target/${TARGET}/release/${BINARY_NAME}.exe" "${ARCHIVE_DIR}/"
          else
            cp "target/${TARGET}/release/${BINARY_NAME}" "${ARCHIVE_DIR}/"
          fi

          cp README.md LICENSE "${ARCHIVE_DIR}/" 2>/dev/null || true

          if [[ "${TARGET}" == *"windows"* ]]; then
            7z a "${ARCHIVE_DIR}.zip" "${ARCHIVE_DIR}"
          else
            tar czf "${ARCHIVE_DIR}.tar.gz" "${ARCHIVE_DIR}"
          fi

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: archive-${{ matrix.target }}
          path: ${{ env.BINARY_NAME }}-*-${{ matrix.target }}.*

  release:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts
          merge-multiple: true

      - name: Generate checksums
        run: |
          cd artifacts
          shasum -a 256 *.tar.gz *.zip 2>/dev/null > SHA256SUMS.txt
          cat SHA256SUMS.txt

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          files: |
            artifacts/*.tar.gz
            artifacts/*.zip
            artifacts/SHA256SUMS.txt
          generate_release_notes: true
          draft: false
          prerelease: ${{ contains(github.ref_name, '-rc') || contains(github.ref_name, '-beta') }}
```

Key design decisions in this workflow:

- **`fail-fast: false`** -- If one platform fails, the others continue building. You can fix the failing platform and re-run just that job.
- **`permissions: contents: write`** -- Required to create releases. Use the minimum permissions necessary.
- **`generate_release_notes: true`** -- GitHub automatically generates release notes from merged PRs since the last release.
- **Prerelease detection** -- Tags containing `-rc` or `-beta` are automatically marked as pre-releases.

## Using `cargo-dist`

For projects that want a turnkey solution, [`cargo-dist`](https://opensource.axo.dev/cargo-dist/) generates the entire release pipeline:

```bash
# Install cargo-dist
cargo install cargo-dist

# Initialize -- interactive setup
cargo dist init

# Generate CI configuration
cargo dist generate
```

`cargo-dist` creates a `.github/workflows/release.yml` that handles building, packaging, checksum generation, and GitHub Release creation. It also generates install scripts and Homebrew formulas. It is opinionated but saves significant time if its defaults match your needs.

## Install Scripts

A "curl pipe bash" install script gives users a one-liner to install your tool. Create an `install.sh` that detects the platform and downloads the right binary:

```bash
#!/bin/bash
set -euo pipefail

REPO="yourname/my-agent"
BINARY_NAME="my-agent"
INSTALL_DIR="${HOME}/.local/bin"

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}" in
    Linux)  OS_TAG="unknown-linux-musl" ;;
    Darwin) OS_TAG="apple-darwin" ;;
    *)      echo "Unsupported OS: ${OS}"; exit 1 ;;
esac

case "${ARCH}" in
    x86_64|amd64)  ARCH_TAG="x86_64" ;;
    aarch64|arm64) ARCH_TAG="aarch64" ;;
    *)             echo "Unsupported architecture: ${ARCH}"; exit 1 ;;
esac

TARGET="${ARCH_TAG}-${OS_TAG}"

# Get latest version
VERSION=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | \
    grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')

echo "Installing ${BINARY_NAME} v${VERSION} for ${TARGET}..."

# Download and extract
ARCHIVE="${BINARY_NAME}-${VERSION}-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/v${VERSION}/${ARCHIVE}"

TMPDIR=$(mktemp -d)
trap "rm -rf ${TMPDIR}" EXIT

curl -sL "${URL}" -o "${TMPDIR}/${ARCHIVE}"

# Verify checksum
CHECKSUMS_URL="https://github.com/${REPO}/releases/download/v${VERSION}/SHA256SUMS.txt"
curl -sL "${CHECKSUMS_URL}" -o "${TMPDIR}/SHA256SUMS.txt"
(cd "${TMPDIR}" && shasum -a 256 -c SHA256SUMS.txt --ignore-missing)

# Extract and install
tar xzf "${TMPDIR}/${ARCHIVE}" -C "${TMPDIR}"
mkdir -p "${INSTALL_DIR}"
cp "${TMPDIR}/${BINARY_NAME}-${VERSION}-${TARGET}/${BINARY_NAME}" "${INSTALL_DIR}/"
chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

echo "Installed ${BINARY_NAME} to ${INSTALL_DIR}/${BINARY_NAME}"

# Check if install dir is in PATH
if [[ ":${PATH}:" != *":${INSTALL_DIR}:"* ]]; then
    echo ""
    echo "Add ${INSTALL_DIR} to your PATH:"
    echo "  export PATH=\"\${HOME}/.local/bin:\${PATH}\""
fi
```

Users install with:

```bash
curl -sSf https://raw.githubusercontent.com/yourname/my-agent/main/install.sh | bash
```

Host the install script in your repository so it is always up to date. The script automatically fetches the latest release, verifies the checksum, and installs to `~/.local/bin`.

::: wild In the Wild
The "curl pipe bash" pattern is controversial in the security community because it runs arbitrary code from the internet. Production tools mitigate this by verifying checksums, using HTTPS exclusively, and hosting the script on a domain they control. Many Rust CLI tools (rustup itself uses this pattern) accept the tradeoff because the alternative -- asking users to manually download, extract, and move binaries -- has much lower adoption.
:::

## Key Takeaways

- Package binaries into `tar.gz` (Unix) or `zip` (Windows) archives containing the binary, shell completions, man pages, README, and LICENSE.
- Always generate and publish SHA256 checksums alongside your release archives so users can verify download integrity.
- Use a GitHub Actions matrix build to compile for all platforms in parallel, then aggregate artifacts into a single release.
- Consider `cargo-dist` for a turnkey release pipeline that generates CI workflows, install scripts, and Homebrew formulas automatically.
- Provide a platform-detecting install script for users who want a one-command installation without a package manager.
