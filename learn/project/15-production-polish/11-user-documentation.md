---
title: User Documentation
description: Writing user-facing documentation including installation guides, quick start tutorials, command references, and configuration guides using mdBook or similar tools.
---

# User Documentation

> **What you'll learn:**
> - How to structure user documentation that covers installation, quick start, and reference material
> - Techniques for generating command-line help and configuration references from code
> - How to set up mdBook or similar tools for building and deploying documentation sites

Your agent is fast, well-tested, and easy to install. But none of that matters if users cannot figure out how to use it. Good documentation is not a nice-to-have -- it is the difference between a tool that people adopt and one they abandon after five minutes. In this subchapter, you will build comprehensive user documentation that covers installation, first steps, command reference, configuration, and troubleshooting.

## Documentation Structure

Effective documentation follows the Diataxis framework, which organizes content into four categories:

1. **Tutorials** -- learning-oriented, step-by-step guides ("Getting Started")
2. **How-to guides** -- task-oriented instructions ("How to configure a custom provider")
3. **Reference** -- information-oriented, comprehensive descriptions ("Command Reference")
4. **Explanation** -- understanding-oriented, conceptual discussions ("How the Agent Loop Works")

For a CLI tool, here is a practical structure:

```
docs/
  src/
    SUMMARY.md           # mdBook table of contents
    introduction.md      # What the agent does and who it is for
    installation.md      # All installation methods
    quickstart.md        # First session walkthrough
    configuration.md     # Config file format and options
    commands.md          # Full command and flag reference
    providers.md         # LLM provider setup guides
    tools.md             # Built-in tool documentation
    troubleshooting.md   # Common problems and solutions
    changelog.md         # Link to or embed the changelog
  book.toml              # mdBook configuration
```

## Setting Up mdBook

mdBook is the standard documentation tool in the Rust ecosystem. It compiles Markdown into a static website.

```bash
# Install mdBook
cargo install mdbook

# Initialize the documentation structure
mkdir docs && cd docs
mdbook init
```

Configure `docs/book.toml`:

```toml
[book]
title = "Agent Documentation"
authors = ["Your Name"]
language = "en"
multilingual = false
src = "src"

[output.html]
default-theme = "coal"
preferred-dark-theme = "coal"
git-repository-url = "https://github.com/yourusername/agent"
edit-url-template = "https://github.com/yourusername/agent/edit/main/docs/src/{path}"

[output.html.search]
enable = true
```

Define the table of contents in `docs/src/SUMMARY.md`:

```markdown
# Summary

- [Introduction](./introduction.md)
- [Installation](./installation.md)
- [Quick Start](./quickstart.md)
- [Configuration](./configuration.md)
- [Command Reference](./commands.md)
- [LLM Providers](./providers.md)
- [Built-in Tools](./tools.md)
- [Troubleshooting](./troubleshooting.md)
- [Changelog](./changelog.md)
```

::: python Coming from Python
Python projects typically use Sphinx with reStructuredText or MkDocs with Markdown for documentation. mdBook is Rust's equivalent to MkDocs -- it compiles Markdown to a static site with search, theming, and navigation. The experience is similar, but mdBook is a single binary with no Python dependency chain to manage. If you have written MkDocs documentation, the transition to mdBook is straightforward.
:::

## Writing the Installation Guide

The installation guide should cover every method a user might try:

```markdown
<!-- docs/src/installation.md -->
# Installation

## Homebrew (macOS and Linux)

The easiest way to install on macOS:

\```bash
brew tap yourusername/agent
brew install agent
\```

## Cargo (any platform with Rust)

If you have Rust installed:

\```bash
cargo install agent
\```

To install with only specific provider support:

\```bash
# Only Anthropic (default)
cargo install agent

# Only OpenAI
cargo install agent --no-default-features --features openai

# All providers
cargo install agent --features full
\```

## Pre-built Binaries

Download the latest binary for your platform from the
[GitHub Releases](https://github.com/yourusername/agent/releases) page.

### macOS

\```bash
# Apple Silicon (M1/M2/M3/M4)
curl -LO https://github.com/yourusername/agent/releases/latest/download/agent-aarch64-apple-darwin.tar.gz
tar xzf agent-aarch64-apple-darwin.tar.gz
sudo mv agent /usr/local/bin/

# Intel
curl -LO https://github.com/yourusername/agent/releases/latest/download/agent-x86_64-apple-darwin.tar.gz
tar xzf agent-x86_64-apple-darwin.tar.gz
sudo mv agent /usr/local/bin/
\```

### Linux

\```bash
# x86_64
curl -LO https://github.com/yourusername/agent/releases/latest/download/agent-x86_64-unknown-linux-musl.tar.gz
tar xzf agent-x86_64-unknown-linux-musl.tar.gz
sudo mv agent /usr/local/bin/

# ARM64
curl -LO https://github.com/yourusername/agent/releases/latest/download/agent-aarch64-unknown-linux-musl.tar.gz
tar xzf agent-aarch64-unknown-linux-musl.tar.gz
sudo mv agent /usr/local/bin/
\```

## Verify Installation

\```bash
agent --version
\```
```

## Generating Command Reference from Code

Rather than maintaining the command reference by hand, generate it from your clap definitions. Add a build step that produces Markdown from clap's help output:

```rust
// src/bin/generate_docs.rs
use clap::CommandFactory;
use std::io::Write;

fn main() {
    let cmd = agent::Cli::command();
    let mut output = String::new();

    output.push_str("# Command Reference\n\n");
    output.push_str("## Global Options\n\n");
    output.push_str("```\n");

    let mut buf = Vec::new();
    cmd.clone().write_long_help(&mut buf).unwrap();
    output.push_str(&String::from_utf8(buf).unwrap());
    output.push_str("```\n\n");

    // Document each subcommand
    for subcmd in cmd.get_subcommands() {
        output.push_str(&format!("## `agent {}`\n\n", subcmd.get_name()));
        if let Some(about) = subcmd.get_long_about().or(subcmd.get_about()) {
            output.push_str(&format!("{}\n\n", about));
        }
        output.push_str("```\n");
        let mut buf = Vec::new();
        subcmd.clone().write_long_help(&mut buf).unwrap();
        output.push_str(&String::from_utf8(buf).unwrap());
        output.push_str("```\n\n");
    }

    std::fs::write("docs/src/commands.md", output).unwrap();
    println!("Generated docs/src/commands.md");
}
```

Run this as part of your documentation build process:

```bash
cargo run --bin generate_docs
mdbook build docs/
```

## Writing the Configuration Reference

Document every configuration option with its type, default value, and environment variable override:

```markdown
<!-- docs/src/configuration.md -->
# Configuration

Agent reads configuration from three sources, in order of priority:

1. **Global config**: `~/.config/agent/config.toml`
2. **Project config**: `.agent.toml` in your project root
3. **Environment variables**: `AGENT_*` prefixed variables
4. **CLI flags**: override everything

## Configuration Reference

### `[provider]`

| Key | Type | Default | Env Var | Description |
|-----|------|---------|---------|-------------|
| `name` | string | `"anthropic"` | `AGENT_PROVIDER` | LLM provider |
| `model` | string | `"claude-sonnet-4-20250514"` | `AGENT_MODEL` | Model identifier |
| `api_url` | string | (provider default) | `AGENT_API_URL` | API base URL |
| `max_tokens` | integer | `4096` | `AGENT_MAX_TOKENS` | Max response tokens |
| `temperature` | float | `0.0` | - | Response temperature |

### `[tools]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `allowed_commands` | string[] | `["cargo", "git", "ls", "cat"]` | Auto-approved commands |
| `command_timeout_secs` | integer | `30` | Max command execution time |
| `allowed_read_paths` | string[] | `["."]` | Readable directories |
| `allowed_write_paths` | string[] | `["."]` | Writable directories |

### `[logging]`

| Key | Type | Default | Env Var | Description |
|-----|------|---------|---------|-------------|
| `level` | string | `"info"` | `AGENT_LOG_LEVEL` | Log verbosity |
| `file` | string | (none) | - | Log file path |
| `json` | boolean | `false` | - | JSON log format |
```

## Deploying Documentation

Use GitHub Actions to build and deploy documentation automatically:

```yaml
# .github/workflows/docs.yml
name: Documentation

on:
  push:
    branches: [main]
    paths:
      - 'docs/**'
      - 'src/**'  # Rebuild if code changes (for generated docs)

permissions:
  pages: write
  id-token: write

jobs:
  deploy:
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable

      - name: Install mdBook
        run: cargo install mdbook

      - name: Generate command reference
        run: cargo run --bin generate_docs

      - name: Build documentation
        run: mdbook build docs

      - name: Upload pages artifact
        uses: actions/upload-pages-artifact@v3
        with:
          path: docs/book

      - name: Deploy to GitHub Pages
        id: deployment
        uses: actions/deploy-pages@v4
```

Enable GitHub Pages in your repository settings (Settings > Pages > Source: GitHub Actions).

::: wild In the Wild
Claude Code provides built-in help via the `/help` command and documents its full API and configuration in Anthropic's developer documentation. OpenCode includes an `opencode --help` command with detailed subcommand documentation. Both tools follow the principle that the most important documentation is accessible from within the tool itself -- `--help` should be comprehensive enough for daily use, with the website serving as the in-depth reference.
:::

## Key Takeaways

- Structure documentation using the Diataxis framework: tutorials (Getting Started), how-to guides (task-oriented), reference (comprehensive), and explanation (conceptual) -- each serves a different user need.
- Use mdBook for building documentation sites -- it is the standard in the Rust ecosystem, compiles Markdown to a searchable static site, and deploys easily to GitHub Pages.
- Generate command references from your clap definitions rather than writing them by hand, ensuring documentation always matches the actual CLI interface.
- Document every configuration option with its type, default value, environment variable override, and a clear description -- this is the page users will visit most often.
- Deploy documentation automatically through CI on every push to main, so documentation stays current with the code without manual publishing steps.
