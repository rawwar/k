---
title: Homebrew Formula
description: Creating a Homebrew formula and tap for easy macOS installation, including SHA256 verification, dependency declarations, and automated formula updates.
---

# Homebrew Formula

> **What you'll learn:**
> - How to write a Homebrew formula that downloads and installs the agent binary
> - How to set up a Homebrew tap repository for distributing the formula
> - Techniques for automating formula updates when new versions are released

For macOS users, `brew install` is the expected way to install developer tools. If your agent is not on Homebrew, many potential users will not bother installing it. Setting up a Homebrew tap and formula is simpler than it looks, and once it is in place, new releases can update the formula automatically through your CI pipeline.

## Homebrew Concepts

Before writing the formula, let's clarify a few Homebrew terms:

- **Formula** -- a Ruby script that tells Homebrew how to download, build (if needed), and install your software.
- **Tap** -- a Git repository of formulas. Your tap is separate from Homebrew's core repository, which means you do not need to go through the Homebrew review process.
- **Bottle** -- a pre-built binary package. For binary-only distributions (which is our case), the formula simply downloads the binary.
- **Cask** -- used for macOS GUI applications. CLI tools use formulas, not casks.

## Creating a Homebrew Tap Repository

A tap is a GitHub repository with a specific naming convention: `homebrew-<name>`. For the agent:

1. Create a repository named `homebrew-agent` under your GitHub account.
2. Inside it, create a `Formula` directory.
3. Place your formula Ruby file in `Formula/agent.rb`.

Users will install from your tap with:

```bash
brew tap yourusername/agent
brew install yourusername/agent/agent
```

## Writing the Formula

Here is a complete formula that downloads pre-built binaries for Intel and Apple Silicon Macs:

```ruby
# Formula/agent.rb
class Agent < Formula
  desc "A CLI coding agent powered by LLMs"
  homepage "https://github.com/yourusername/agent"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/yourusername/agent/releases/download/v#{version}/agent-aarch64-apple-darwin.tar.gz"
      sha256 "abc123...replace_with_actual_sha256_for_arm64..."
    else
      url "https://github.com/yourusername/agent/releases/download/v#{version}/agent-x86_64-apple-darwin.tar.gz"
      sha256 "def456...replace_with_actual_sha256_for_x86_64..."
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/yourusername/agent/releases/download/v#{version}/agent-aarch64-unknown-linux-musl.tar.gz"
      sha256 "789abc...replace_with_actual_sha256_for_linux_arm64..."
    else
      url "https://github.com/yourusername/agent/releases/download/v#{version}/agent-x86_64-unknown-linux-musl.tar.gz"
      sha256 "012def...replace_with_actual_sha256_for_linux_x86_64..."
    end
  end

  def install
    bin.install "agent"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/agent --version")
  end
end
```

The formula does the following:

1. Detects the platform and architecture.
2. Downloads the correct pre-built binary from your GitHub release.
3. Verifies the SHA256 checksum (critical for security).
4. Installs the binary into Homebrew's `bin` directory.
5. Provides a test that verifies the binary outputs the correct version.

::: python Coming from Python
If you have published Python packages, you know that `pip install` handles everything. Homebrew is macOS's equivalent system-level package manager. The Ruby formula is roughly analogous to a `setup.py` or `pyproject.toml` -- it declares metadata, download URLs, and installation steps. The key difference is that Homebrew formulas download *pre-built binaries* rather than building from source (unless you set up a source formula), so you must produce and host the binaries yourself.
:::

## Generating SHA256 Checksums

The SHA256 checksums are essential -- they verify that the downloaded binary has not been tampered with. You generate them from your release artifacts:

```bash
# Generate checksums for all release archives
shasum -a 256 agent-*.tar.gz

# Or using openssl
openssl dgst -sha256 agent-aarch64-apple-darwin.tar.gz
```

In your release automation (covered in the next subchapter), you will generate these automatically and use them to update the formula.

## Building a Source Formula

If you prefer to let Homebrew build from source (which avoids hosting binaries but requires users to have Rust installed), the formula looks different:

```ruby
class Agent < Formula
  desc "A CLI coding agent powered by LLMs"
  homepage "https://github.com/yourusername/agent"
  url "https://github.com/yourusername/agent/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "source_tarball_sha256_here..."
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/agent --version")
  end
end
```

The `std_cargo_args` method is a Homebrew helper that passes `--root=#{prefix}` and other standard Cargo installation flags. The `depends_on "rust" => :build` declaration ensures Rust is installed before building.

For a binary distribution, the pre-built binary formula is preferred because it avoids the 5-10 minute compile time.

## Installing Shell Completions

You can include shell completions in the formula installation:

```ruby
def install
  bin.install "agent"

  # Generate and install shell completions
  generate_completions_from_executable(bin/"agent", "completions")

  # Or install pre-generated completion files
  # bash_completion.install "completions/agent.bash" => "agent"
  # zsh_completion.install "completions/_agent"
  # fish_completion.install "completions/agent.fish"
end
```

The `generate_completions_from_executable` helper runs your binary with the completions subcommand and places the output in the correct locations. This is why the `agent completions <shell>` command you built in the CLI subchapter matters -- Homebrew can call it directly.

## Automating Formula Updates

When you release a new version, the formula needs to be updated with the new version number, download URLs, and checksums. Here is a script that automates this:

```bash
#!/bin/bash
# scripts/update-homebrew.sh
set -euo pipefail

VERSION="$1"
TAP_REPO="yourusername/homebrew-agent"

# Download the release archives and compute checksums
declare -A CHECKSUMS
for target in \
    "aarch64-apple-darwin" \
    "x86_64-apple-darwin" \
    "aarch64-unknown-linux-musl" \
    "x86_64-unknown-linux-musl"; do

    archive="agent-${target}.tar.gz"
    url="https://github.com/yourusername/agent/releases/download/v${VERSION}/${archive}"

    echo "Downloading ${archive}..."
    curl -sLO "$url"
    CHECKSUMS[$target]=$(shasum -a 256 "$archive" | awk '{print $1}')
    rm "$archive"
done

# Clone the tap repository and update the formula
git clone "https://github.com/${TAP_REPO}.git" homebrew-tap
cd homebrew-tap

# Use sed to update version and checksums in the formula
sed -i '' "s/version \".*\"/version \"${VERSION}\"/" Formula/agent.rb

echo "Updated formula to version ${VERSION}"
echo "Checksums:"
for target in "${!CHECKSUMS[@]}"; do
    echo "  ${target}: ${CHECKSUMS[$target]}"
done

# The CI pipeline would commit and push these changes
git add Formula/agent.rb
git commit -m "Update agent to ${VERSION}"
git push origin main
```

In the next subchapter on release automation, you will integrate this into your GitHub Actions workflow so that pushing a version tag automatically updates the Homebrew formula.

## Testing the Formula Locally

Before publishing, test the formula locally:

```bash
# Install from the local formula file
brew install --formula ./Formula/agent.rb

# Test it
brew test agent

# Audit the formula for style issues
brew audit --strict --formula ./Formula/agent.rb

# Uninstall when done testing
brew uninstall agent
```

The `brew audit --strict` command checks for common formula issues like missing descriptions, incorrect license formats, or missing test blocks.

::: wild In the Wild
Claude Code is distributed through npm rather than Homebrew, leveraging the Node.js ecosystem's cross-platform package management. Many Rust CLI tools (like `ripgrep`, `bat`, and `fd`) maintain Homebrew formulas in the official homebrew-core repository, which requires a formal review process but reaches a much larger audience. Starting with your own tap is the right first step -- you can submit to homebrew-core later once the tool is mature and has enough users.
:::

## Key Takeaways

- Set up a GitHub repository named `homebrew-<name>` as your Homebrew tap, with a `Formula` directory containing the Ruby formula file -- users install with `brew tap` + `brew install`.
- Write platform-aware formulas using `on_macos`/`on_linux` and `Hardware::CPU.arm?` blocks to download the correct pre-built binary for each user's system.
- Always include SHA256 checksums in the formula for security verification, and regenerate them automatically when releasing new versions.
- Include a `test` block in the formula that validates the installed binary (at minimum, check that `--version` output matches the formula version).
- Automate formula updates in your CI pipeline so that pushing a version tag triggers a new release, generates checksums, and updates the tap repository without manual intervention.
