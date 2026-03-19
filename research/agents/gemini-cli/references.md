# Gemini CLI — References

> Source links, documentation, packages, and related tools.

## Primary Sources

### GitHub Repository

- **Repository**: https://github.com/google-gemini/gemini-cli
- **License**: Apache 2.0
- **Language**: TypeScript
- **Organization**: google-gemini (Google's Gemini open-source org)

Key source directories for architecture research:
```
packages/core/src/
├── agent/              # Agent orchestration
├── agents/             # Sub-agent support
├── core/               # Core modules
│   ├── baseLlmClient.ts
│   ├── client.ts
│   ├── contentGenerator.ts
│   ├── geminiChat.ts
│   ├── geminiRequest.ts
│   ├── turn.ts
│   ├── tokenLimits.ts
│   ├── prompts.ts
│   ├── coreToolScheduler.ts
│   └── coreToolHookTriggers.ts
├── tools/              # Built-in tool implementations
├── sandbox/            # Sandbox backends
├── config/             # Configuration management
├── mcp/                # MCP integration
├── skills/             # Skills system
├── policy/             # Security policy
├── confirmation-bus/   # Confirmation system
├── safety/             # Safety filters
├── routing/            # Model routing
├── scheduler/          # Tool scheduling
├── hooks/              # Lifecycle hooks
├── voice/              # Voice input
├── ide/                # IDE integration
├── output/             # Output formatting
├── telemetry/          # Telemetry
├── fallback/           # Error recovery
└── billing/            # Usage tracking
```

### Official Documentation

- **Getting Started**: https://github.com/google-gemini/gemini-cli#readme
- **Google AI for Developers**: https://ai.google.dev/gemini-api/docs
- **Gemini API Reference**: https://ai.google.dev/api
- **Vertex AI Documentation**: https://cloud.google.com/vertex-ai/docs

### npm Package

- **Package**: Published to npm registry
- **Install**: `npm install -g @google/gemini-cli` (or `npx`)
- **Registry**: https://www.npmjs.com/search?q=gemini-cli

### Homebrew

- **Install**: Available via Homebrew

## API and Model Documentation

### Gemini Models

- **Gemini 3 Flash**: Latest Flash model, best performance in Terminal-Bench 2.0
  - Context window: 1M tokens
  - Optimized for speed and function calling

- **Gemini 2.5 Pro**: Reasoning-focused model
  - Context window: 1M tokens
  - Stronger on reasoning benchmarks, weaker on agentic tasks

- **Gemini 2.5 Flash**: Previous generation Flash
  - Lighter weight, fast responses
  - Good for simpler tasks

### Gemini API

- **Base URL**: https://generativelanguage.googleapis.com/
- **Authentication**: API Key or OAuth
- **Features**: Streaming, function calling, grounding, token caching
- **Documentation**: https://ai.google.dev/gemini-api/docs

### Vertex AI

- **Console**: https://console.cloud.google.com/vertex-ai
- **Endpoints**: Region-specific (us-central1, europe-west4, etc.)
- **Authentication**: Service account or application default credentials
- **Features**: Higher rate limits, VPC security, data residency

## Related Tools and Protocols

### Model Context Protocol (MCP)

- **Specification**: https://modelcontextprotocol.io/
- **GitHub**: https://github.com/modelcontextprotocol
- **SDK (TypeScript)**: https://github.com/modelcontextprotocol/typescript-sdk
- **Server examples**: https://github.com/modelcontextprotocol/servers
- **Gemini CLI MCP integration**: packages/core/src/mcp/

### Competing Terminal Agents

- **Claude Code** (Anthropic)
  - https://docs.anthropic.com/en/docs/claude-code
  - The most direct competitor — first-party agent from Anthropic

- **Codex CLI** (OpenAI)
  - https://github.com/openai/codex
  - OpenAI's terminal coding agent

- **Aider**
  - https://github.com/paul-gauthier/aider
  - Open-source, model-agnostic terminal coding agent

- **Cursor** (IDE-based)
  - https://cursor.com
  - IDE-native agent with terminal features

### Sandbox Technologies

- **macOS Seatbelt**
  - Apple's built-in sandboxing framework
  - `sandbox-exec` command-line tool
  - SBPL (Seatbelt Profile Language) for policy definition
  - Documentation: Apple developer docs (limited public docs)

- **Docker**
  - https://www.docker.com/
  - https://docs.docker.com/engine/security/
  - Container-based isolation

- **Podman**
  - https://podman.io/
  - Daemonless container engine, Docker-compatible

- **gVisor**
  - https://gvisor.dev/
  - https://github.com/google/gvisor
  - User-space kernel for container sandboxing
  - Developed by Google (relevant connection to Gemini CLI)

- **LXC/LXD**
  - https://linuxcontainers.org/
  - System container manager

## Benchmarks

### Terminal-Bench 2.0

- **Leaderboard**: Check terminal-bench.com or related benchmark sites
- **Methodology**: Real-world terminal coding tasks
- **Gemini CLI entries**:
  - Gemini 3 Flash: Rank #55, Score 47.4%
  - Gemini 2.5 Pro: Rank #105, Score 19.6%

### Other Relevant Benchmarks

- **SWE-bench**: Software engineering benchmark (separate from terminal agents)
  - https://www.swebench.com/
  - Measures code generation quality

- **HumanEval**: Code generation benchmark
  - Measures function-level code generation
  - Not specific to terminal agents

## Community and Support

### Issue Tracking

- **GitHub Issues**: https://github.com/google-gemini/gemini-cli/issues
- Bug reports, feature requests, and community discussion

### Contributing

- **Contributing Guide**: https://github.com/google-gemini/gemini-cli/blob/main/CONTRIBUTING.md
- Apache 2.0 license — open to contributions

### Release Channels

- **Stable**: Weekly Tuesday releases
- **Preview**: Weekly Tuesday pre-releases
- **Nightly**: Continuous from main branch
- Release notes on GitHub Releases page

## Configuration Reference

### Settings Files

```
~/.gemini/
├── GEMINI.md              # Global context file
├── settings.json          # Global settings
└── skills/                # Global skills
    └── <skill-name>/
        ├── skill.md
        └── metadata.json

.gemini/
├── settings.json          # Project settings
└── skills/                # Project skills
    └── <skill-name>/
        ├── skill.md
        └── metadata.json

./GEMINI.md                # Workspace context file
```

### Environment Variables

```bash
GEMINI_API_KEY            # API key for authentication
GEMINI_SANDBOX            # Sandbox backend (seatbelt, docker, podman, gvisor, lxc)
SANDBOX_FLAGS             # Additional sandbox flags
GOOGLE_CLOUD_PROJECT      # Vertex AI project ID
GOOGLE_CLOUD_REGION       # Vertex AI region
```

### Key Configuration Options (settings.json)

```json
{
  "model": "gemini-3-flash",
  "sandbox": "seatbelt",
  "sandboxFlags": [],
  "contextFile": "GEMINI.md",
  "mcpServers": {},
  "toolConfirmation": {
    "autoApprove": []
  },
  "tools": {
    "discoveryCommand": ""
  }
}
```

## Research Methodology

This research was conducted by:
1. Analyzing the Gemini CLI source code at github.com/google-gemini/gemini-cli
2. Reviewing official Google AI documentation
3. Examining the TypeScript monorepo structure (packages/core, packages/cli)
4. Studying the tool implementations and sandbox backends
5. Reviewing Terminal-Bench 2.0 benchmark results
6. Comparing with competing agents (Claude Code, Codex CLI)

## Version Information

- **Research date**: 2025
- **Agent version**: Based on source code analysis (check releases for exact version)
- **Models evaluated**: Gemini 3 Flash, Gemini 2.5 Pro
- **Benchmark version**: Terminal-Bench 2.0
