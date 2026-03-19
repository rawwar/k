# Claude Code — References

> Official documentation, blog posts, public analyses, and community resources.

## Official Documentation

### Primary Docs (docs.anthropic.com)

| Page | URL | Coverage |
|------|-----|----------|
| Overview | https://docs.anthropic.com/en/docs/claude-code/overview | Installation, surfaces, capabilities |
| How Claude Code Works | https://docs.anthropic.com/en/docs/claude-code/how-claude-code-works | Agentic loop, tools, architecture |
| Best Practices | https://docs.anthropic.com/en/docs/claude-code/best-practices | Context management, prompt patterns, workflows |
| Common Workflows | https://docs.anthropic.com/en/docs/claude-code/common-workflows | Debugging, refactoring, testing, PR creation |
| Tools Reference | https://docs.anthropic.com/en/docs/claude-code/tools-reference | Complete tool catalog with permission requirements |
| Permissions | https://docs.anthropic.com/en/docs/claude-code/permissions | Permission system, modes, rule syntax, sandboxing |
| Settings | https://docs.anthropic.com/en/docs/claude-code/settings | Configuration scopes, all settings keys |
| Memory | https://docs.anthropic.com/en/docs/claude-code/memory | CLAUDE.md files, auto-memory, rules |
| Sub-Agents | https://docs.anthropic.com/en/docs/claude-code/sub-agents | Built-in and custom sub-agents, configuration |
| MCP | https://docs.anthropic.com/en/docs/claude-code/mcp | MCP server integration, transports, auth |
| Hooks | https://docs.anthropic.com/en/docs/claude-code/hooks | Lifecycle hook events, configuration |
| Skills | https://docs.anthropic.com/en/docs/claude-code/skills | On-demand knowledge packages |
| Plugins | https://docs.anthropic.com/en/docs/claude-code/plugins | Plugin system, marketplaces |
| VS Code | https://docs.anthropic.com/en/docs/claude-code/vs-code | VS Code extension |
| JetBrains | https://docs.anthropic.com/en/docs/claude-code/jetbrains | JetBrains plugin |
| GitHub Actions | https://docs.anthropic.com/en/docs/claude-code/github-actions | CI/CD integration |
| Headless Mode | https://docs.anthropic.com/en/docs/claude-code/headless | Non-interactive / scripting mode |
| Sandboxing | https://docs.anthropic.com/en/docs/claude-code/sandboxing | OS-level isolation |
| Model Config | https://docs.anthropic.com/en/docs/claude-code/model-config | Model selection, effort levels, thinking |

### Product Page
- **code.claude.com** — Product page with demos, pricing, and overview

### API / SDK
- **Agent SDK** — https://platform.claude.com/docs/en/agent-sdk/overview — For building custom agents

## GitHub Repository

| Resource | URL |
|----------|-----|
| Repository | https://github.com/anthropics/claude-code |
| Issues | https://github.com/anthropics/claude-code/issues |
| Plugins | https://github.com/anthropics/claude-code/tree/main/plugins |
| Example Settings | https://github.com/anthropics/claude-code/tree/main/examples/settings |

**Note**: The GitHub repo hosts the npm package distribution, plugins directory, example configurations, and issue tracker. The core source code is not open-source — the TypeScript source is not published.

## npm Package

| Resource | URL |
|----------|-----|
| Package | https://www.npmjs.com/package/@anthropic-ai/claude-code |
| Node.js requirement | 18+ |

Installation via npm is deprecated in favor of native installers.

## Anthropic Blog Posts

| Title | URL | Key Content |
|-------|-----|-------------|
| Claude Code announcement | https://www.anthropic.com/news/claude-code-announcement (approx.) | Initial launch as research preview (Feb 2025) |
| Claude 4 model family launch | https://www.anthropic.com/news/claude-4 (approx.) | SWE-bench scores for Claude 4 models |

**Note**: Exact blog post URLs may vary. Search https://www.anthropic.com/news for "Claude Code" for current listings.

## Benchmark Sources

| Benchmark | Source | Notes |
|-----------|--------|-------|
| Terminal-Bench 2.0 | https://terminal-bench.com (approx.) | Terminal agent benchmark; Claude Code ranks #39/#48/#69 |
| SWE-bench | https://www.swebench.com | Software engineering benchmark; model scores published by Anthropic |
| SWE-bench Verified | https://www.swebench.com | Curated 500-task subset |

## Community Resources

### Discord
- **Claude Developers Discord** — https://anthropic.com/discord — Official community for Claude Code users

### Settings Schema
- **JSON Schema** — https://json.schemastore.org/claude-code-settings.json — For IDE autocomplete in settings.json

### MCP Ecosystem
- **Model Context Protocol** — https://modelcontextprotocol.io — Open standard for AI-tool integrations
- **MCP Server Registry** — Various community-maintained lists of MCP servers

## Public Analyses & Third-Party Content

### System Prompt Analyses

Several community members and researchers have shared analyses of Claude Code's system prompt (obtained by asking Claude Code to reveal it, or through prompt injection techniques):

- System prompt analyses shared on Twitter/X, Reddit, and Hacker News
- Key observations from public analyses:
  - Very long system prompt (~15-20K+ tokens estimated)
  - Detailed tool-use instructions for each built-in tool
  - File editing instructions emphasizing surgical changes
  - Permission model rules embedded in the prompt
  - Context management instructions (when to compact, clear)
  - CLAUDE.md loading instructions
  - Git workflow instructions (commit messages, PR descriptions)
  - Sub-agent spawning instructions
  - Safety guidelines and content restrictions

**Caveat**: System prompts change frequently across versions. Public analyses reflect point-in-time snapshots.

### Community Discussions

| Platform | Search Terms |
|----------|-------------|
| Hacker News | "Claude Code" site:news.ycombinator.com |
| Reddit | r/ClaudeAI, r/LocalLLaMA — Claude Code discussions |
| Twitter/X | #ClaudeCode, @AnthropicAI — Claude Code announcements |
| YouTube | "Claude Code tutorial", "Claude Code review" — Video walkthroughs |

## Related Technologies

| Technology | Relationship to Claude Code |
|-----------|---------------------------|
| **Ink** | React renderer for terminal UIs — used for Claude Code's TUI (https://github.com/vadimdemedes/ink) |
| **MCP** | Open protocol for AI-tool integration — native in Claude Code (https://modelcontextprotocol.io) |
| **Claude API** | Anthropic's model API — powers the reasoning (https://docs.anthropic.com/en/docs/api) |

## Versioning and Updates

- **Release channel**: `stable` (one week old, skips regressions) or `latest` (most recent)
- **Auto-updates**: Configurable via `autoUpdatesChannel` setting
- **Changelog**: Check npm package versions and GitHub releases
- **Bug reports**: `/bug` command within Claude Code, or GitHub Issues

## Research Methodology Notes

This research was compiled from:

1. **Official documentation** at docs.anthropic.com (primary source)
2. **GitHub repository** at github.com/anthropics/claude-code (npm package, plugins, examples)
3. **Observable behavior** documented by community members
4. **Benchmark data** from Terminal-Bench and SWE-bench public leaderboards
5. **npm package metadata** (version history, dependencies, Node.js requirements)

The core Claude Code source is not open-source. Architecture details are inferred from documentation, the npm package manifest, and publicly observable behavior. Where something is inferred rather than confirmed, it is noted as such.
