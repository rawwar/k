# Pi — References

## Primary Sources

### Repository & Documentation

- **Monorepo**: [github.com/badlogic/pi-mono](https://github.com/badlogic/pi-mono) — All seven packages in one repository
- **Website**: [shittycodingagent.ai](https://shittycodingagent.ai) — Official site (tongue-in-cheek domain name)
- **npm packages**: Published under the `@mariozechner` scope
  - [`@mariozechner/pi-coding-agent`](https://www.npmjs.com/package/@mariozechner/pi-coding-agent) — The CLI
  - [`@mariozechner/pi-ai`](https://www.npmjs.com/package/@mariozechner/pi-ai) — Unified LLM API
  - [`@mariozechner/pi-agent-core`](https://www.npmjs.com/package/@mariozechner/pi-agent-core) — Agent runtime
  - [`@mariozechner/pi-tui`](https://www.npmjs.com/package/@mariozechner/pi-tui) — Terminal UI framework
  - [`@mariozechner/pi-web-ui`](https://www.npmjs.com/package/@mariozechner/pi-web-ui) — Web components
  - [`@mariozechner/pi-mom`](https://www.npmjs.com/package/@mariozechner/pi-mom) — Slack bot
  - [`@mariozechner/pi-pods`](https://www.npmjs.com/package/@mariozechner/pi-pods) — GPU pod management

### Author

- **Mario Zechner** (@badlogic)
  - GitHub: [github.com/badlogic](https://github.com/badlogic)
  - Blog: [mariozechner.at](https://mariozechner.at)
  - Known for: libGDX (popular open-source Java game framework)
  - Pi origin story blog post on mariozechner.at describing frustrations with Claude Code and the motivation for building Pi

### Standards

- **Agent Skills standard**: [agentskills.io](https://agentskills.io) — The standard Pi's skills system follows

## Community Resources

### Discord

- **Pi Discord server**: Active community for discussion, support, and package sharing
- Primary channel for real-time help and extension development discussion

### Ecosystem Projects

- **awesome-pi-agent**: Curated list of Pi resources, packages, extensions, and integrations
- **pi-vs-claude-code**: Comparison repository benchmarking Pi against Claude Code on real-world tasks
- **pi-skills**: Community-maintained collection of common skills for various frameworks and tools

### Notable Third-Party Packages

| Package | Description | Type |
|---------|-------------|------|
| pi-skills | Common skills collection | Skills |
| pi-messenger | Messaging platform integrations | Extension |
| pi-mcp-adapter | MCP protocol support for Pi | Extension |
| pi-web-access | Web browsing capabilities | Extension |

### Multi-Agent Integrations

- **Overstory**: Multi-agent orchestrator with Pi integration
- **Agent of Empires**: Orchestration framework supporting Pi as a component agent

## Key Architectural References

### Monorepo Packages

| Package | npm Name | Role |
|---------|----------|------|
| `packages/ai/` | `@mariozechner/pi-ai` | Unified multi-provider LLM API |
| `packages/agent/` | `@mariozechner/pi-agent-core` | Agent runtime, tool calling, state |
| `packages/coding-agent/` | `@mariozechner/pi-coding-agent` | The CLI application |
| `packages/tui/` | `@mariozechner/pi-tui` | Terminal UI with differential rendering |
| `packages/web-ui/` | `@mariozechner/pi-web-ui` | Web components for chat UIs |
| `packages/mom/` | `@mariozechner/pi-mom` | Slack bot delegating to pi |
| `packages/pods/` | `@mariozechner/pi-pods` | CLI for vLLM on GPU pods |

### Key Configuration Files

| File | Purpose |
|------|---------|
| `AGENTS.md` | Project instructions (like CLAUDE.md) |
| `SYSTEM.md` | System prompt customization (append or replace) |
| `SKILL.md` | Skill definition files |
| `~/.pi/agent/skills/` | Global skills directory |
| `~/.pi/agent/prompts/` | Prompt templates directory |

## Supported LLM Providers (via pi-ai)

### API Protocols

| Protocol | Description |
|----------|-------------|
| OpenAI Completions | Standard chat completions API |
| OpenAI Responses | Newer OpenAI responses API |
| Anthropic Messages | Claude models API |
| Google Generative AI | Gemini models API |

### Providers (15+)

Anthropic, OpenAI, Google, Azure OpenAI, AWS Bedrock, Mistral, Groq, Cerebras, xAI, OpenRouter, Hugging Face, Kimi, MiniMax, and others.

## Package Discovery

- **npm keyword**: `pi-package` — Search npm for community packages
- **Installation**: `pi install npm:@package/name` or `pi install git:github.com/user/repo`

## Related Projects by Author

- **libGDX**: [github.com/libgdx/libgdx](https://github.com/libgdx/libgdx) — Cross-platform Java game framework (Mario Zechner's most well-known project, demonstrating his track record in developer tooling and open-source community building)

## Research Notes

- Pi is intentionally un-Google-able (the name "pi" is generic) — use "pi coding agent" or "pi-coding-agent" for searches
- The website domain (shittycodingagent.ai) is tongue-in-cheek, reflecting the anti-marketing philosophy
- The project is MIT licensed
- Active development with regular releases as of 2025
