---
title: Changelog Management
description: Maintaining a changelog that tracks user-facing changes, using conventional commits and tools like git-cliff to automate changelog generation for each release.
---

# Changelog Management

> **What you'll learn:**
> - How to maintain a CHANGELOG.md following the Keep a Changelog conventions
> - How to use conventional commits and git-cliff to automate changelog generation
> - Techniques for categorizing changes as added, changed, fixed, or removed for clear communication

Users want to know what changed in each release. Did you fix the bug they reported? Is there a new feature they can try? Will anything break when they upgrade? A well-maintained changelog answers these questions at a glance. In this subchapter, you will set up both the conventions and the tooling to keep your changelog accurate with minimal effort.

## The Keep a Changelog Format

The [Keep a Changelog](https://keepachangelog.com/) convention is the most widely used format. Here is what it looks like:

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Shell completion generation for bash, zsh, and fish

### Fixed
- Config file parsing now handles comments correctly

## [0.2.0] - 2026-03-10

### Added
- Multi-provider support (Anthropic, OpenAI, Ollama)
- Project-level configuration with `.agent.toml`
- Structured logging with the `tracing` crate

### Changed
- Default model updated to claude-sonnet-4-20250514
- Config file format migrated from JSON to TOML

### Fixed
- Agent no longer crashes on malformed LLM responses
- Shell command timeout now works correctly on Linux

### Removed
- Deprecated `--api-key` CLI flag (use environment variable instead)

## [0.1.0] - 2026-02-15

### Added
- Initial release with Anthropic provider support
- Interactive REPL mode
- File read/write tools
- Shell command execution
- Basic error handling
```

The structure is simple: each version has dated sections grouped by change type. The `[Unreleased]` section at the top collects changes that have not been released yet.

::: python Coming from Python
Python projects use the same Keep a Changelog format (or similar). If you have maintained a `CHANGELOG.md` or `HISTORY.rst` for a Python package, this will be familiar. The difference in the Rust ecosystem is that tools like `git-cliff` can fully automate changelog generation from commit messages, whereas Python's equivalents like `towncrier` require separate "news fragments" for each change.
:::

## Conventional Commits

Automated changelog generation requires structured commit messages. The [Conventional Commits](https://www.conventionalcommits.org/) specification gives you that structure:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

Common types and how they map to changelog categories:

| Commit Type | Changelog Category | Example |
|---|---|---|
| `feat` | Added | `feat(tools): add grep tool for code search` |
| `fix` | Fixed | `fix(shell): handle timeout on long-running commands` |
| `perf` | Changed | `perf(parser): reduce JSON parsing allocations by 40%` |
| `refactor` | Changed | `refactor(config): simplify layered config merging` |
| `docs` | (usually excluded) | `docs: update installation guide for Homebrew` |
| `test` | (usually excluded) | `test: add integration tests for multi-turn conversations` |
| `chore` | (usually excluded) | `chore: update dependencies` |
| `feat!` or `BREAKING CHANGE` | (highlighted) | `feat!: change config format from JSON to TOML` |

The scope is optional but useful for filtering changes to specific components. The `!` after the type (or `BREAKING CHANGE` in the footer) marks breaking changes that warrant a major version bump.

## Setting Up git-cliff

`git-cliff` reads your commit history, applies conventional commit parsing, and generates a changelog. Install it:

```bash
cargo install git-cliff
```

Create a configuration file `cliff.toml` in your project root:

```toml
[changelog]
header = """
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

"""
body = """
{% if version %}\
    ## [{{ version | trim_start_matches(pat="v") }}] - {{ timestamp | date(format="%Y-%m-%d") }}
{% else %}\
    ## [Unreleased]
{% endif %}\
{% for group, commits in commits | group_by(attribute="group") %}
    ### {{ group | upper_first }}
    {% for commit in commits %}
        - {% if commit.scope %}**{{ commit.scope }}**: {% endif %}\
            {{ commit.message | upper_first }}\
            {% if commit.breaking %} (**BREAKING**){% endif %}\
    {% endfor %}
{% endfor %}\n
"""
trim = true

[git]
conventional_commits = true
filter_unconventional = true
split_commits = false
commit_parsers = [
    { message = "^feat", group = "Added" },
    { message = "^fix", group = "Fixed" },
    { message = "^perf", group = "Changed" },
    { message = "^refactor", group = "Changed" },
    { message = "^doc", group = "Documentation" },
    { message = "^style", skip = true },
    { message = "^test", skip = true },
    { message = "^chore", skip = true },
    { message = "^ci", skip = true },
]
protect_breaking_commits = false
filter_commits = false
tag_pattern = "v[0-9].*"
sort_commits = "oldest"
```

Generate the changelog:

```bash
# Generate full changelog
git-cliff -o CHANGELOG.md

# Generate changelog for just the latest release
git-cliff --latest -o CHANGELOG.md

# Preview what the next release would look like
git-cliff --unreleased

# Generate changelog between two tags
git-cliff v0.1.0..v0.2.0
```

## Integrating with the Release Pipeline

Add changelog generation to your GitHub Actions release workflow:

```yaml
# Add this step to your release job in .github/workflows/release.yml
- name: Generate release notes
  run: |
    cargo install git-cliff
    # Generate notes for just this release
    git-cliff --latest --strip header > release-notes.md
    cat release-notes.md

- name: Update CHANGELOG.md
  run: |
    git-cliff -o CHANGELOG.md
    git config user.name "github-actions[bot]"
    git config user.email "github-actions[bot]@users.noreply.github.com"
    git add CHANGELOG.md
    git commit -m "chore: update CHANGELOG.md for ${{ github.ref_name }}" || true
    git push origin HEAD:main || true
```

This generates release notes for the GitHub Release page and updates the `CHANGELOG.md` file in the repository.

## Writing Good Commit Messages

The quality of your changelog depends entirely on the quality of your commit messages. Here are practical examples:

```bash
# Good: specific, describes the user-facing change
git commit -m "feat(tools): add grep tool for searching code with regex patterns"

# Bad: vague, describes implementation not behavior
git commit -m "feat: add new tool"

# Good: explains what was broken and how it is fixed
git commit -m "fix(shell): prevent command timeout from killing the agent process

The shell tool was using SIGKILL to terminate timed-out commands, which
also killed the agent's child process group. Now uses SIGTERM with a
grace period before escalating to SIGKILL."

# Good: breaking change clearly marked
git commit -m "feat!(config): migrate configuration from JSON to TOML

BREAKING CHANGE: Config files must be renamed from .agent.json to
.agent.toml and converted to TOML format. Run 'agent config migrate'
to convert automatically."
```

## Enforcing Commit Conventions

Use a Git hook to validate commit messages before they are accepted:

```bash
#!/bin/sh
# .githooks/commit-msg

commit_msg=$(cat "$1")

# Conventional commit pattern
pattern="^(feat|fix|docs|style|refactor|perf|test|chore|ci|build|revert)(\(.+\))?(!)?: .{1,}"

if ! echo "$commit_msg" | grep -qE "$pattern"; then
    echo "ERROR: Commit message does not follow Conventional Commits format."
    echo ""
    echo "Expected: <type>(<scope>): <description>"
    echo "Example:  feat(tools): add grep tool for code search"
    echo ""
    echo "Valid types: feat, fix, docs, style, refactor, perf, test, chore, ci, build, revert"
    exit 1
fi
```

Enable the hook:

```bash
git config core.hooksPath .githooks
```

::: wild In the Wild
Many Rust CLI tools including `ripgrep` and `bat` maintain hand-written changelogs that are meticulously organized by category. Larger projects with more contributors tend to automate with tools like `git-cliff` or `release-plz`. Claude Code's changelog is maintained as part of its release process within Anthropic's internal systems. The consensus in the ecosystem is that automated changelogs from conventional commits produce good-enough results for most projects, with the option to manually edit before publishing.
:::

## Key Takeaways

- Follow the Keep a Changelog format with sections for Added, Changed, Fixed, and Removed under each version heading -- this is the format users expect and understand.
- Adopt Conventional Commits (`feat:`, `fix:`, `perf:`, etc.) for structured commit messages that enable automated changelog generation and semantic version bumping.
- Use `git-cliff` to generate changelogs from commit history, configured with a `cliff.toml` that maps commit types to changelog categories and filters out non-user-facing changes.
- Integrate changelog generation into your CI release pipeline so that every tagged release automatically updates `CHANGELOG.md` and generates release notes for the GitHub Release page.
- Enforce commit message conventions with a Git hook that rejects non-conforming messages, ensuring the raw material for your changelog is consistently structured.
