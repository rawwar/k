---
title: Homebrew Distribution
description: Create and maintain a Homebrew tap that lets macOS and Linux users install your agent with a single brew install command.
---

# Homebrew Distribution

> **What you'll learn:**
> - How to create a Homebrew formula that downloads prebuilt binaries and installs them with proper shell completions and man pages
> - Techniques for setting up a Homebrew tap repository with automated formula updates triggered by new GitHub releases
> - How to handle Homebrew-specific requirements including bottle generation, dependency declarations, and version pinning

Homebrew is the dominant package manager on macOS and has strong adoption on Linux as well. For CLI tools targeting developers, `brew install` is the gold standard of distribution. A single command downloads, installs, and configures your agent with shell completions and proper PATH setup. This subchapter walks through creating a Homebrew formula from scratch and automating updates as you release new versions.

## How Homebrew Works

Homebrew installs software into its own prefix (`/opt/homebrew` on Apple Silicon, `/usr/local` on Intel Mac, `/home/linuxbrew/.linuxbrew` on Linux) and symlinks binaries into a directory on the user's PATH. A **formula** is a Ruby script that tells Homebrew how to download, build (or just unpack), and install your software.

There are two ways to distribute through Homebrew:

1. **Homebrew core** -- Submit your formula to the official `homebrew/homebrew-core` repository. This requires meeting strict criteria (notable project, active maintenance, no unnecessary dependencies) and going through a review process.
2. **Custom tap** -- Host your own formula repository. Users install with `brew tap yourname/tap && brew install my-agent` or `brew install yourname/tap/my-agent`. This is the practical choice for most projects.

We will focus on the custom tap approach because you control the entire pipeline and can start distributing immediately.

::: python Coming from Python
Homebrew can install Python tools too -- many CLI tools distributed through `pip` also have Homebrew formulas. But Homebrew formulas for Python tools are complex: they need to declare Python as a dependency, manage virtual environments, and handle Python version upgrades. Rust binaries are radically simpler: the formula downloads a prebuilt binary and copies it into place. No interpreter, no virtual environment, no dependency isolation.
:::

## Creating a Homebrew Tap

A tap is a GitHub repository with a specific naming convention. Create a repository named `homebrew-tap` (the `homebrew-` prefix is required):

```bash
# Create the tap repository on GitHub
gh repo create homebrew-tap --public --description "Homebrew formulas"

# Clone it
git clone https://github.com/yourname/homebrew-tap.git
cd homebrew-tap

# Create the Formula directory (required by Homebrew convention)
mkdir Formula
```

The repository structure is:

```
homebrew-tap/
  Formula/
    my-agent.rb    # Your formula file
  README.md
```

## Writing the Formula

A formula for prebuilt binaries is straightforward. Create `Formula/my-agent.rb`:

```ruby
class MyAgent < Formula
  desc "A CLI coding agent powered by large language models"
  homepage "https://github.com/yourname/my-agent"
  version "0.5.2"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/yourname/my-agent/releases/download/v0.5.2/my-agent-aarch64-apple-darwin.tar.gz"
      sha256 "abc123...full-sha256-hash-here..."
    end
    on_intel do
      url "https://github.com/yourname/my-agent/releases/download/v0.5.2/my-agent-x86_64-apple-darwin.tar.gz"
      sha256 "def456...full-sha256-hash-here..."
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/yourname/my-agent/releases/download/v0.5.2/my-agent-aarch64-unknown-linux-musl.tar.gz"
      sha256 "789abc...full-sha256-hash-here..."
    end
    on_intel do
      url "https://github.com/yourname/my-agent/releases/download/v0.5.2/my-agent-x86_64-unknown-linux-musl.tar.gz"
      sha256 "012def...full-sha256-hash-here..."
    end
  end

  def install
    bin.install "my-agent"

    # Install shell completions if included in the archive
    bash_completion.install "completions/my-agent.bash" => "my-agent"
    zsh_completion.install "completions/_my-agent"
    fish_completion.install "completions/my-agent.fish"

    # Install man page if included
    man1.install "man/my-agent.1" if File.exist? "man/my-agent.1"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/my-agent --version")
  end
end
```

Let's break down the key parts:

- **`on_macos` / `on_linux` / `on_arm` / `on_intel`** -- Conditional blocks that select the right binary for the user's platform and architecture. Homebrew evaluates these at install time.
- **`sha256`** -- A cryptographic hash of each archive. Homebrew verifies this to ensure the download has not been tampered with. Generate it with `shasum -a 256 filename.tar.gz`.
- **`bin.install`** -- Copies the binary into the Homebrew bin directory and creates the symlink.
- **`bash_completion.install`** -- Installs shell completions to the right location so they are automatically loaded.
- **`test`** -- A simple smoke test that Homebrew runs with `brew test my-agent`.

## Generating Shell Completions

Most Rust CLI tools use `clap` for argument parsing. Clap can generate shell completions at build time. Add this to your `build.rs`:

```rust
use clap::CommandFactory;
use clap_complete::{generate_to, shells};
use std::fs;

// Assuming your CLI struct is defined in src/cli.rs
include!("src/cli.rs");

fn main() {
    let out_dir = std::path::PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").unwrap()
    ).join("completions");

    fs::create_dir_all(&out_dir).unwrap();

    let mut cmd = Cli::command();
    generate_to(shells::Bash, &mut cmd, "my-agent", &out_dir).unwrap();
    generate_to(shells::Zsh, &mut cmd, "my-agent", &out_dir).unwrap();
    generate_to(shells::Fish, &mut cmd, "my-agent", &out_dir).unwrap();
}
```

Alternatively, many CLI tools generate completions at runtime with a `--completions` subcommand:

```rust
use clap::{Command, CommandFactory};
use clap_complete::{generate, Shell};

fn print_completions(shell: Shell) {
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "my-agent", &mut std::io::stdout());
}
```

Include the generated completion files in your release archive alongside the binary.

## Computing SHA256 Hashes

When updating the formula, you need the SHA256 hash of each platform's archive. Compute them from your release artifacts:

```bash
# After building release archives
shasum -a 256 my-agent-*-*.tar.gz

# Output:
# abc123...  my-agent-aarch64-apple-darwin.tar.gz
# def456...  my-agent-x86_64-apple-darwin.tar.gz
# 789abc...  my-agent-aarch64-unknown-linux-musl.tar.gz
# 012def...  my-agent-x86_64-unknown-linux-musl.tar.gz
```

Paste these hashes into the formula. If a hash does not match at install time, Homebrew refuses to install -- this is a critical security feature.

## Automating Formula Updates

Manually updating the formula for every release is tedious and error-prone. Automate it with GitHub Actions. Add this job to your release workflow:

```yaml
update-homebrew:
  needs: [build, release]  # Run after binaries are published
  runs-on: ubuntu-latest
  steps:
    - name: Checkout tap repository
      uses: actions/checkout@v4
      with:
        repository: yourname/homebrew-tap
        token: ${{ secrets.TAP_GITHUB_TOKEN }}
        path: homebrew-tap

    - name: Download release assets and compute hashes
      run: |
        VERSION="${GITHUB_REF_NAME#v}"  # Strip 'v' prefix
        BASE_URL="https://github.com/yourname/my-agent/releases/download/${GITHUB_REF_NAME}"

        declare -A TARGETS=(
          ["aarch64-apple-darwin"]="on_macos.*on_arm"
          ["x86_64-apple-darwin"]="on_macos.*on_intel"
          ["aarch64-unknown-linux-musl"]="on_linux.*on_arm"
          ["x86_64-unknown-linux-musl"]="on_linux.*on_intel"
        )

        for target in "${!TARGETS[@]}"; do
          ARCHIVE="my-agent-${target}.tar.gz"
          curl -sL "${BASE_URL}/${ARCHIVE}" -o "${ARCHIVE}"
          HASH=$(shasum -a 256 "${ARCHIVE}" | awk '{print $1}')
          echo "${target}_hash=${HASH}" >> "$GITHUB_ENV"
          echo "${target}: ${HASH}"
        done

        echo "version=${VERSION}" >> "$GITHUB_ENV"

    - name: Update formula
      run: |
        cd homebrew-tap
        # Use sed or a templating tool to update version and hashes
        # in Formula/my-agent.rb
        python3 scripts/update-formula.py \
          --version "${{ env.version }}" \
          --aarch64-darwin "${{ env.aarch64-apple-darwin_hash }}" \
          --x86-64-darwin "${{ env.x86_64-apple-darwin_hash }}" \
          --aarch64-linux "${{ env.aarch64-unknown-linux-musl_hash }}" \
          --x86-64-linux "${{ env.x86_64-unknown-linux-musl_hash }}"

    - name: Commit and push
      run: |
        cd homebrew-tap
        git config user.name "github-actions[bot]"
        git config user.email "github-actions[bot]@users.noreply.github.com"
        git add Formula/my-agent.rb
        git commit -m "my-agent ${{ env.version }}"
        git push
```

Alternatively, the [`homebrew-releaser`](https://github.com/Justintime50/homebrew-releaser) action handles all of this with less custom scripting.

## Testing Your Formula

Before pushing to users, test the formula locally:

```bash
# Add your tap
brew tap yourname/tap

# Install from the tap
brew install yourname/tap/my-agent

# Verify the installation
my-agent --version

# Run the formula's test block
brew test my-agent

# Check for formula issues
brew audit --strict --new Formula/my-agent.rb
```

The `brew audit` command checks for common formula problems: missing descriptions, incorrect URLs, style violations, and more. Fix any issues it reports before publishing.

::: wild In the Wild
Many Rust CLI tools distribute through Homebrew taps. The pattern is well-established: a release workflow builds binaries, uploads them to GitHub Releases, then updates the tap formula with new URLs and hashes. Tools like `cargo-dist` and `cargo-release` can automate the entire pipeline including tap updates, making it a one-command release process.
:::

## Key Takeaways

- A Homebrew tap is a GitHub repository named `homebrew-tap` containing Ruby formula files in a `Formula/` directory.
- Formulas for Rust binaries are simple: they download a prebuilt archive, verify its SHA256 hash, and copy the binary into place.
- Use `on_macos`/`on_linux` and `on_arm`/`on_intel` blocks to serve the correct binary for each platform and architecture.
- Include shell completions (bash, zsh, fish) and man pages in your release archive so the formula can install them alongside the binary.
- Automate formula updates in your release CI pipeline so every new tag automatically updates the tap repository with new version numbers and hashes.
