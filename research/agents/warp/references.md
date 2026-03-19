---
title: Warp References and Resources
status: complete
---

# References

> Comprehensive collection of URLs, documentation pages, blog posts, and resources
> related to Warp's terminal, agent platform (Oz), and architecture.

## Official Website and Documentation

### Primary URLs

| Resource | URL |
|----------|-----|
| **Warp Home** | https://warp.dev |
| **Documentation** | https://docs.warp.dev |
| **Blog** | https://warp.dev/blog |
| **Changelog** | https://docs.warp.dev/help/changelog |
| **Pricing** | https://warp.dev/pricing |

### Architecture and Engineering Blog Posts

| Post | URL | Key Content |
|------|-----|-------------|
| **How Warp Works** | https://warp.dev/blog/how-warp-works | Core architecture: Rust, Metal, PTY, blocks |
| **How Alacritty Works** | https://warp.dev/blog/how-alacritty-works | Understanding Warp's grid model heritage |
| **Using Rust and GPU** | https://warp.dev/blog/using-rust-and-gpu-to-render-user-interfaces-at-120-fps | GPU rendering deep dive |

### Agent and AI Documentation

| Page | URL | Key Content |
|------|-----|-------------|
| **Agent Overview** | https://docs.warp.dev/agent | Agent capabilities overview |
| **Agent Mode** | https://docs.warp.dev/agent/agent-mode | Conversation view for multi-turn workflows |
| **Terminal AI** | https://docs.warp.dev/agent/terminal-ai | Inline AI in terminal view |
| **Full Terminal Use** | https://docs.warp.dev/agent/full-terminal-use | PTY attachment, interactive process control |
| **AI Models** | https://docs.warp.dev/agent/ai-models | Model selection, auto modes, providers |
| **Active AI** | https://docs.warp.dev/agent/active-ai | Proactive error detection and fix suggestions |
| **Computer Use** | https://docs.warp.dev/agent/computer-use | Desktop interaction via screenshots |

### Context and Configuration Documentation

| Page | URL | Key Content |
|------|-----|-------------|
| **Context** | https://docs.warp.dev/agent/context | Multi-modal context system |
| **Codebase Context** | https://docs.warp.dev/agent/context/codebase-context | Semantic indexing, embeddings |
| **Rules** | https://docs.warp.dev/agent/rules | Global rules, AGENTS.md files |
| **Skills** | https://docs.warp.dev/agent/skills | SKILL.md files, parameterization |
| **Plans** | https://docs.warp.dev/agent/plans | /plan command, version history |
| **MCP** | https://docs.warp.dev/agent/mcp | Model Context Protocol integration |
| **Permissions** | https://docs.warp.dev/agent/permissions | Permission levels for agent actions |

### Conversation Management

| Page | URL | Key Content |
|------|-----|-------------|
| **Conversations** | https://docs.warp.dev/agent/conversations | Conversation lifecycle |
| **Forking** | https://docs.warp.dev/agent/conversations/forking | /fork, /fork-and-compact |
| **Compaction** | https://docs.warp.dev/agent/conversations/compaction | /compact, context summarization |

### Cloud Agents (Oz)

| Page | URL | Key Content |
|------|-----|-------------|
| **Oz Overview** | https://docs.warp.dev/oz | Cloud agent platform overview |
| **Oz Agents** | https://docs.warp.dev/oz/agents | Cloud agent configuration |
| **Triggers** | https://docs.warp.dev/oz/triggers | Slack, Linear, GitHub, webhooks, schedules |
| **Environments** | https://docs.warp.dev/oz/environments | Docker images, repos, startup commands |
| **Self-Hosting** | https://docs.warp.dev/oz/self-hosting | Managed daemon, unmanaged mode |
| **Oz CLI** | https://docs.warp.dev/oz/cli | Command-line interface for cloud agents |
| **Oz API/SDK** | https://docs.warp.dev/oz/api | Programmatic access to Oz platform |

### Terminal Features Documentation

| Page | URL | Key Content |
|------|-----|-------------|
| **Blocks** | https://docs.warp.dev/features/blocks | Block-based terminal model |
| **Warp Drive** | https://docs.warp.dev/features/warp-drive | Persistent shared storage |
| **Code Editor** | https://docs.warp.dev/features/editor | Built-in code editor with LSP |
| **Voice Input** | https://docs.warp.dev/features/voice-input | Speech-to-text input |
| **Code Review** | https://docs.warp.dev/agent/code-review | Interactive code review for agent diffs |
| **Task Lists** | https://docs.warp.dev/agent/task-lists | Structured task tracking |

## GitHub

| Resource | URL | Notes |
|----------|-----|-------|
| **Issues Repository** | https://github.com/warpdotdev/Warp | Issues-only repo (not source code) |
| **Warp Themes** | https://github.com/warpdotdev/themes | Community themes |

> **Note**: Warp's terminal source code is **not open source**. The GitHub repository
> (warpdotdev/Warp) is used exclusively for issue tracking and community feedback.
> Warp has expressed plans to open-source the Rust UI framework and potentially the
> client in the future.

## Benchmarks

| Resource | URL | Notes |
|----------|-----|-------|
| **Terminal-Bench** | https://terminal-bench.com | Primary benchmark for terminal agents |
| **Terminal-Bench 2.0 Leaderboard** | https://terminal-bench.com/leaderboard | Current rankings |
| **Terminal-Bench GitHub** | https://github.com/terminal-bench/terminal-bench | Benchmark source |

### Warp Benchmark Entries

| Benchmark | Rank | Score | URL |
|-----------|------|-------|-----|
| Terminal-Bench 2.0 (config A) | #31 | 61.2% | https://terminal-bench.com (leaderboard) |
| Terminal-Bench 2.0 (config B) | #36 | 59.1% | https://terminal-bench.com (leaderboard) |
| Terminal-Bench 2.0 (config C) | #52 | 50.1% | https://terminal-bench.com (leaderboard) |
| Terminal-Bench 1.0 | #11 | 52.0% | https://terminal-bench.com (leaderboard) |

## Community and Social

| Resource | URL |
|----------|-----|
| **Warp Discord** | https://discord.gg/warpdotdev |
| **Warp Twitter/X** | https://x.com/warpdotdev |
| **Warp LinkedIn** | https://linkedin.com/company/warpdotdev |
| **Zach Lloyd (CEO) Twitter** | https://x.com/zachlloyd |

## Related Technologies

### Alacritty (Grid Model Heritage)

| Resource | URL | Relevance |
|----------|-----|-----------|
| **Alacritty GitHub** | https://github.com/alacritty/alacritty | Source of Warp's forked grid model |
| **Alacritty VTE** | https://github.com/alacritty/vte | Terminal escape sequence parser |

### Metal (GPU Rendering)

| Resource | URL | Relevance |
|----------|-----|-----------|
| **Metal Documentation** | https://developer.apple.com/metal/ | GPU API used by Warp on macOS |
| **Metal Shading Language** | https://developer.apple.com/metal/Metal-Shading-Language-Specification.pdf | Shader language for rendering |

### Rust Ecosystem

| Resource | URL | Relevance |
|----------|-----|-----------|
| **Rust Language** | https://www.rust-lang.org | Warp's implementation language |
| **wgpu** | https://github.com/gfx-rs/wgpu | Cross-platform GPU abstraction (potential Linux/Windows) |

### MCP (Model Context Protocol)

| Resource | URL | Relevance |
|----------|-----|-----------|
| **MCP Specification** | https://modelcontextprotocol.io | Protocol spec for tool integration |
| **MCP GitHub** | https://github.com/modelcontextprotocol | Reference implementations |

### Shell Hooks (Block Detection)

| Resource | URL | Relevance |
|----------|-----|-----------|
| **OSC 133 (Shell Integration)** | https://gitlab.freedesktop.org/Per_Bothner/specifications/blob/master/proposals/semantic-prompts.md | Escape sequences for prompt/command boundaries |
| **Zsh Hooks** | https://zsh.sourceforge.io/Doc/Release/Functions.html#Hook-Functions | precmd/preexec used by Warp |

## Competitors and Comparisons

| Agent | URL | Comparison Point |
|-------|-----|-----------------|
| **Claude Code** | https://docs.anthropic.com/en/docs/claude-code | CLI wrapper agent |
| **Codex CLI** | https://github.com/openai/codex | OpenAI's terminal agent |
| **Aider** | https://aider.chat | Pair programming tool |
| **Cursor** | https://cursor.com | IDE-native agent |
| **Windsurf** | https://windsurf.com | IDE-native agent |
| **Devin** | https://devin.ai | Cloud-only autonomous agent |
| **Zed** | https://zed.dev | Rust editor (shared UI framework heritage) |

## Funding and Company

| Resource | URL | Notes |
|----------|-----|-------|
| **Crunchbase** | https://www.crunchbase.com/organization/warp | Funding history ($170M+) |
| **Warp About** | https://warp.dev/about | Company info, team |
| **Warp Careers** | https://warp.dev/careers | Engineering positions |

## Security and Compliance

| Resource | URL | Notes |
|----------|-----|-------|
| **Security** | https://warp.dev/security | SOC 2, ZDR policies |
| **Privacy Policy** | https://warp.dev/privacy | Data handling practices |
| **Terms of Service** | https://warp.dev/terms | Usage terms |

## Key Blog Posts for Research

| Post Title | URL | Why Important |
|------------|-----|---------------|
| How Warp Works | https://warp.dev/blog/how-warp-works | **Essential** — core architecture explanation |
| Introducing Oz | https://warp.dev/blog/introducing-oz | Cloud agent platform launch |
| Full Terminal Use | https://warp.dev/blog/full-terminal-use | PTY attachment capability |
| Using Rust and GPU | https://warp.dev/blog/using-rust-and-gpu-to-render-user-interfaces-at-120-fps | Rendering pipeline details |
| How Alacritty Works | https://warp.dev/blog/how-alacritty-works | Grid model heritage |
| Agent Mode | https://warp.dev/blog/agent-mode | Conversation view launch |

> **Note**: URLs are based on documented patterns from Warp's public documentation and
> blog. Some specific paths may have changed since this research was compiled. Always
> verify against the live documentation at https://docs.warp.dev.
