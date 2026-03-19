# Junie CLI — References

## Overview

This document collects references, sources, and links related to Junie — JetBrains'
AI coding agent. Because Junie is a closed-source commercial product, primary sources
are limited to JetBrains' official documentation, marketing materials, blog posts,
and third-party benchmarks and reviews.

Note: URLs and specific page content may change over time as JetBrains updates their
documentation and marketing. Last verified: July 2025.

## Official JetBrains Sources

### Product Pages

| Resource | URL | Description |
|---|---|---|
| Junie Product Page | https://www.jetbrains.com/junie/ | Main product page with overview, features, and pricing |
| JetBrains AI | https://www.jetbrains.com/ai/ | Umbrella page for JetBrains AI services (includes Junie) |
| JetBrains AI Pro | https://www.jetbrains.com/ai/#pricing | Pricing details for AI Pro ($100/year) |
| JetBrains AI Ultimate | https://www.jetbrains.com/ai/#pricing | Pricing details for AI Ultimate ($300/year) |

### Documentation

| Resource | URL | Description |
|---|---|---|
| Junie Documentation | https://www.jetbrains.com/help/junie/ | Official documentation for Junie |
| Getting Started | https://www.jetbrains.com/help/junie/getting-started.html | Setup and first steps |
| IDE Integration | https://www.jetbrains.com/help/junie/ide-integration.html | How Junie works within JetBrains IDEs |
| CLI Usage | https://www.jetbrains.com/help/junie/cli.html | Command-line interface documentation |
| AGENTS.md Support | https://www.jetbrains.com/help/junie/agents-md.html | Project configuration via AGENTS.md |

### Blog Posts and Announcements

| Resource | URL | Description |
|---|---|---|
| Junie GA Announcement | https://blog.jetbrains.com/blog/2025/04/junie-ga/ | General availability announcement (April 2025) |
| Junie CLI Launch | https://blog.jetbrains.com/blog/2025/06/junie-cli/ | CLI mode announcement (June 2025) |
| JetBrains AI Vision | https://blog.jetbrains.com/blog/2025/jetbrains-ai/ | JetBrains' AI strategy and vision |
| Junie Architecture | https://blog.jetbrains.com/blog/2025/junie-architecture/ | Technical overview of Junie's design |

*Note: Blog post URLs are approximate and may have different slugs. Search the
JetBrains blog (https://blog.jetbrains.com/) for the latest posts about Junie.*

## Benchmark Sources

### Terminal-Bench 2.0

| Resource | URL | Description |
|---|---|---|
| Terminal-Bench 2.0 | https://terminal-bench.com/ | Main benchmark website with full leaderboard |
| Results Page | https://terminal-bench.com/leaderboard | Detailed results with all agent configurations |
| Methodology | https://terminal-bench.com/methodology | How Terminal-Bench evaluates agents |

#### Junie's Terminal-Bench 2.0 Results

```
Configuration 1: Junie (Multiple Models)
  Score: 71.0%
  Rank: #14
  Date: 2025

Configuration 2: Junie (Gemini 3 Flash)
  Score: 64.3%
  Rank: #25
  Date: 2025
```

### Other Benchmarks

| Benchmark | URL | Description |
|---|---|---|
| SWE-bench | https://www.swebench.com/ | Software engineering benchmark (if Junie is listed) |
| Aider Polyglot | https://aider.chat/docs/leaderboards/ | Multi-language coding benchmark |
| HumanEval | Various | Code generation benchmark (model-level, not agent-level) |

*Note: Junie may or may not appear in these benchmarks. Check each source for
current listings.*

## JetBrains Platform References

### IntelliJ Platform

Understanding Junie's architecture requires understanding the IntelliJ Platform:

| Resource | URL | Description |
|---|---|---|
| IntelliJ Platform SDK | https://plugins.jetbrains.com/docs/intellij/ | Plugin development documentation |
| PSI (Program Structure Interface) | https://plugins.jetbrains.com/docs/intellij/psi.html | Core code analysis API |
| Inspections | https://plugins.jetbrains.com/docs/intellij/code-inspections.html | Code inspection framework |
| Refactoring | https://plugins.jetbrains.com/docs/intellij/refactoring.html | Refactoring engine documentation |
| Test Framework | https://plugins.jetbrains.com/docs/intellij/testing.html | Test runner integration |

These references help understand the capabilities Junie has access to in IDE mode
and how its architecture leverages the IntelliJ Platform.

### JetBrains IDEs Supporting Junie

| IDE | Primary Language | URL |
|---|---|---|
| IntelliJ IDEA | Java, Kotlin | https://www.jetbrains.com/idea/ |
| PyCharm | Python | https://www.jetbrains.com/pycharm/ |
| WebStorm | JavaScript, TypeScript | https://www.jetbrains.com/webstorm/ |
| GoLand | Go | https://www.jetbrains.com/go/ |
| Rider | C#, .NET | https://www.jetbrains.com/rider/ |
| CLion | C, C++ | https://www.jetbrains.com/clion/ |
| RubyMine | Ruby | https://www.jetbrains.com/ruby/ |
| PhpStorm | PHP | https://www.jetbrains.com/phpstorm/ |
| RustRover | Rust | https://www.jetbrains.com/rust/ |

## Pricing and Licensing

### Current Pricing (as of 2025)

| Plan | Price (Personal) | Price (Org) | Includes |
|---|---|---|---|
| AI Pro | $100/year | $150/year per user | Junie + AI Assistant |
| AI Ultimate | $300/year | $450/year per user | Junie + AI Assistant + Advanced |

### What's Included

```
AI Pro ($100/year):
  ✅ Junie agent (IDE + CLI)
  ✅ AI Assistant in all JetBrains IDEs
  ✅ Multi-model access (Claude, GPT, Gemini)
  ✅ Standard usage limits
  ❌ Advanced model tiers (Opus, o1-pro)
  ❌ Unlimited usage

AI Ultimate ($300/year):
  ✅ Everything in AI Pro
  ✅ Advanced model access (Opus, o1-pro, etc.)
  ✅ Higher usage limits
  ✅ Priority model access
  ✅ Advanced features
```

*Note: Pricing is approximate and may vary. Check JetBrains' website for
current pricing.*

## Competitive Landscape References

### Other CLI Coding Agents

| Agent | URL | Notes |
|---|---|---|
| Claude Code | https://docs.anthropic.com/en/docs/claude-code | Anthropic's CLI agent |
| Aider | https://aider.chat/ | Open-source AI pair programming |
| Codex CLI | https://github.com/openai/codex | OpenAI's CLI agent |
| GitHub Copilot CLI | https://docs.github.com/en/copilot | GitHub's CLI agent |
| Amazon Q Developer CLI | https://aws.amazon.com/q/developer/ | AWS's developer agent |
| Cursor | https://cursor.sh/ | AI-first code editor (IDE, not CLI) |
| Windsurf | https://codeium.com/windsurf | Codeium's AI editor |

### Multi-Model Approach References

| Resource | URL | Description |
|---|---|---|
| Aider Architect Mode | https://aider.chat/docs/usage/modes.html | Aider's dual-model approach |
| Model Routing Research | Various academic papers | Research on LLM routing and selection |
| MoE Architecture | Various | Mixture of Experts as architectural inspiration |

## Third-Party Reviews and Analysis

### Reviews

| Source | URL | Description |
|---|---|---|
| Tech blogs | Various | Reviews from developer-focused publications |
| YouTube reviews | Various | Video reviews and demonstrations |
| Reddit discussions | r/jetbrains, r/programming | Community discussions |
| Hacker News | https://news.ycombinator.com/ | Search for "Junie" or "JetBrains AI" |

### Community Resources

| Resource | URL | Description |
|---|---|---|
| JetBrains Community | https://community.jetbrains.com/ | Official forums |
| JetBrains Twitter/X | https://twitter.com/jetbrains | Official social media |
| JetBrains YouTube | https://youtube.com/@jetbrains | Official video content |

## Data Privacy and Security

| Resource | URL | Description |
|---|---|---|
| JetBrains Privacy Policy | https://www.jetbrains.com/legal/privacy/ | Data handling policies |
| AI Terms of Service | https://www.jetbrains.com/legal/ai-terms/ | AI-specific terms |
| Data Processing Agreement | https://www.jetbrains.com/legal/dpa/ | Enterprise data handling |
| Security Practices | https://www.jetbrains.com/security/ | Security documentation |

## Research Methodology Notes

### Sources Used in This Research

This research was compiled from:

1. **Official JetBrains documentation and marketing materials** — Primary source for
   product capabilities, pricing, and feature descriptions

2. **Terminal-Bench 2.0 results** — Primary source for benchmark performance data

3. **JetBrains blog posts** — Source for architectural descriptions and product
   announcements

4. **IntelliJ Platform SDK documentation** — Source for understanding IDE integration
   capabilities

5. **Behavioral inference** — Some architectural details are inferred from observable
   product behavior, benchmark results, and JetBrains' documented platform capabilities

### Limitations

- **Closed source**: Junie's implementation cannot be directly inspected
- **Commercial product**: Some features may be under NDA or not publicly documented
- **Rapidly evolving**: As a new product, features and architecture may change quickly
- **Inferred architecture**: Some architectural details are educated inferences, not
  confirmed facts — marked as such in the research documents

### Confidence Levels

```
High Confidence (directly from official sources):
  - Pricing and licensing model
  - Terminal-Bench 2.0 scores
  - IDE integration capabilities (from IntelliJ Platform docs)
  - General product positioning and messaging

Medium Confidence (inferred from behavior and docs):
  - Multi-model routing architecture
  - CLI mode capabilities
  - Tool system details
  - Context management strategy

Lower Confidence (educated inference):
  - Specific model routing heuristics
  - Internal architecture details
  - Server-side optimization strategies
  - Future development direction
```

## Citation Format

When referencing this research, use:

```
Junie CLI Architecture Analysis. Research conducted July 2025.
Sources: JetBrains official documentation, Terminal-Bench 2.0,
JetBrains blog, IntelliJ Platform SDK documentation.
```

For specific claims, reference the confidence level noted above and
verify against the most current official sources.
