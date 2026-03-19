# Codex CLI — References & Resources

## Primary Sources

### GitHub Repository
- **Main repo**: [github.com/openai/codex](https://github.com/openai/codex)
- **License**: Apache-2.0
- **Stars**: 30k+ (as of mid-2025)

### Key Files in Repository

| File | Content |
|---|---|
| `README.md` | Project overview, installation, quickstart |
| `AGENTS.md` | Coding conventions and development guidelines |
| `CHANGELOG.md` | Release notes |
| `codex-rs/README.md` | Rust implementation overview, code organization |
| `codex-cli/README.md` | Legacy TypeScript CLI docs (deprecated) |
| `docs/config.md` | Configuration reference |
| `docs/contributing.md` | Contribution guide |
| `docs/install.md` | Installation instructions |
| `docs/sandbox.md` | Sandbox documentation |
| `docs/exec.md` | Non-interactive execution docs |
| `docs/execpolicy.md` | Execution policy docs |
| `docs/skills.md` | Skills system docs |
| `docs/agents_md.md` | AGENTS.md format spec |
| `docs/prompts.md` | Prompt engineering guidance |
| `docs/js_repl.md` | JavaScript REPL documentation |

### Key Source Directories

| Directory | Content |
|---|---|
| `codex-rs/core/src/` | Core engine (~120 modules) |
| `codex-rs/protocol/src/` | SQ/EQ protocol, types, model metadata |
| `codex-rs/tui/` | Ratatui terminal UI |
| `codex-rs/exec/src/` | Non-interactive execution mode |
| `codex-rs/linux-sandbox/src/` | Linux sandbox (bubblewrap + seccomp) |
| `codex-rs/windows-sandbox-rs/src/` | Windows sandbox (ACLs + Firewall) |
| `codex-rs/process-hardening/src/` | Pre-main security hardening |
| `codex-rs/execpolicy/src/` | Execution policy engine |
| `codex-rs/shell-command/src/` | Shell command parser (84KB main file) |
| `codex-rs/mcp-server/src/` | Codex as MCP server |
| `codex-rs/codex-api/` | Typed OpenAI API client |
| `codex-rs/config/src/` | Layered config system |
| `codex-rs/hooks/src/` | Hook system |
| `codex-rs/skills/src/` | Skills system |

## Official Documentation

### Developer Docs
- **Codex docs home**: [developers.openai.com/codex](https://developers.openai.com/codex)
- **CLI guide**: [developers.openai.com/codex/cli](https://developers.openai.com/codex/cli)
- **CLI features**: [developers.openai.com/codex/cli/features](https://developers.openai.com/codex/cli/features)
- **Sandbox & security**: [developers.openai.com/codex/sandbox](https://developers.openai.com/codex/sandbox)
- **Sandboxing concepts**: [developers.openai.com/codex/concepts/sandboxing](https://developers.openai.com/codex/concepts/sandboxing)
- **MCP integration**: [developers.openai.com/codex/mcp](https://developers.openai.com/codex/mcp)
- **Sub-agents**: [developers.openai.com/codex/subagents](https://developers.openai.com/codex/subagents)
- **Models**: [developers.openai.com/codex/models](https://developers.openai.com/codex/models)
- **Pricing**: [developers.openai.com/codex/pricing](https://developers.openai.com/codex/pricing)
- **Best practices**: [developers.openai.com/codex/learn/best-practices](https://developers.openai.com/codex/learn/best-practices)
- **Changelog**: [developers.openai.com/codex/changelog](https://developers.openai.com/codex/changelog)
- **SDK**: [developers.openai.com/codex/sdk](https://developers.openai.com/codex/sdk)

### Configuration
- **Basic config**: [developers.openai.com/codex/config-basic](https://developers.openai.com/codex/config-basic)
- **Advanced config**: [developers.openai.com/codex/config-advanced](https://developers.openai.com/codex/config-advanced)
- **Config reference**: [developers.openai.com/codex/config-reference](https://developers.openai.com/codex/config-reference)
- **Windows setup**: [developers.openai.com/codex/windows](https://developers.openai.com/codex/windows)
- **Slash commands**: [developers.openai.com/codex/guides/slash-commands](https://developers.openai.com/codex/guides/slash-commands)

### Security
- **Codex security white paper**: [trust.openai.com](https://trust.openai.com/?itemUid=382f924d-54f3-43a8-a9df-c39e6c959958)
- **Agent approvals & security**: [developers.openai.com/codex/agent-approvals-security](https://developers.openai.com/codex/agent-approvals-security)
- **SECURITY.md**: [github.com/openai/codex/blob/main/SECURITY.md](https://github.com/openai/codex/blob/main/SECURITY.md)

## API References

### OpenAI APIs Used
- **Responses API**: [platform.openai.com/docs/api-reference/responses](https://platform.openai.com/docs/api-reference/responses)
- **Web search tool**: [platform.openai.com/docs/guides/tools-web-search](https://platform.openai.com/docs/guides/tools-web-search)

### MCP (Model Context Protocol)
- **MCP specification**: [modelcontextprotocol.io](https://modelcontextprotocol.io)
- **MCP Inspector**: [github.com/modelcontextprotocol/inspector](https://github.com/modelcontextprotocol/inspector)

## Ecosystem & Dependencies

### Key Rust Crates
- **tokio**: Async runtime
- **ratatui**: Terminal UI framework — [ratatui.rs](https://ratatui.rs)
- **clap**: Command-line argument parsing
- **reqwest**: HTTP client
- **serde**: Serialization/deserialization
- **landlock**: Linux Landlock LSM bindings
- **seccompiler**: seccomp BPF filter generation

### Linux Sandbox Dependencies
- **bubblewrap**: [github.com/containers/bubblewrap](https://github.com/containers/bubblewrap) — Mount namespace isolation
- **Landlock LSM**: [landlock.io](https://landlock.io) — Kernel-level filesystem access control
- **seccomp**: Linux kernel syscall filtering

### macOS Sandbox
- **Seatbelt / sandbox-exec**: Apple's application sandboxing framework
- Documented in Apple's sandbox design guide (private API)

### Windows Sandbox Technologies
- **Windows ACLs**: [docs.microsoft.com](https://docs.microsoft.com/en-us/windows/win32/secauthz/access-control-lists)
- **Windows Firewall API**: `INetFwPolicy2` COM interface
- **DPAPI**: Data Protection API for credential storage

## Installation

### Package Managers
```bash
# npm (primary)
npm install -g @openai/codex

# Homebrew (macOS)
brew install --cask codex

# Direct download
# See github.com/openai/codex/releases/latest
```

### Supported Platforms
| Platform | Architecture | Binary |
|---|---|---|
| macOS | Apple Silicon (arm64) | `codex-aarch64-apple-darwin.tar.gz` |
| macOS | Intel (x86_64) | `codex-x86_64-apple-darwin.tar.gz` |
| Linux | x86_64 | `codex-x86_64-unknown-linux-musl.tar.gz` |
| Linux | arm64 | `codex-aarch64-unknown-linux-musl.tar.gz` |
| Windows | x86_64 | Via WSL or native (experimental) |

## Related Projects

### OpenAI Codex Family
- **Codex CLI**: This project — terminal agent
- **Codex IDE**: VS Code / Cursor / Windsurf extension
- **Codex Cloud**: Cloud-hosted agent at [chatgpt.com/codex](https://chatgpt.com/codex)
- **Codex App**: Desktop app (`codex app` command)

### Competing Agents
- **Claude Code**: [docs.anthropic.com/en/docs/agents-and-tools/claude-code](https://docs.anthropic.com/en/docs/agents-and-tools/claude-code)
- **Aider**: [aider.chat](https://aider.chat) — [github.com/Aider-AI/aider](https://github.com/Aider-AI/aider)
- **Cline**: [github.com/cline/cline](https://github.com/cline/cline)
- **Continue**: [continue.dev](https://continue.dev)
- **Cursor**: [cursor.com](https://cursor.com)
- **Windsurf**: [windsurf.com](https://windsurf.com)

## Benchmarks
- **Terminal-Bench 2.0**: Terminal agent evaluation benchmark
- **SWE-bench**: [swe-bench.github.io](https://swe-bench.github.io) — Software engineering tasks

## Research Timeline

| Date | Event |
|---|---|
| May 2025 | Codex CLI open-sourced (TypeScript) |
| Jun 2025 | Rust rewrite announced as primary |
| Mid 2025 | Windows native sandbox added |
| Mid 2025 | MCP server mode added |
| Mid 2025 | Sub-agent system added |
| Jul 2025 | GPT-5.4 model support |
| Ongoing | Active development, frequent releases |

## Config File Locations

| File | Purpose |
|---|---|
| `~/.codex/config.toml` | User configuration |
| `~/.codex/sessions/` | Session rollout persistence |
| `~/.codex/skills/` | Installed skills |
| `~/.codex/memories/` | Agent memory files |
| `~/.codex/themes/` | Custom `.tmTheme` files |
| `codex-rs/core/config.schema.json` | Generated JSON Schema for config |

## Environment Variables

| Variable | Purpose |
|---|---|
| `OPENAI_API_KEY` | API key for OpenAI |
| `CODEX_HOME` | Override default config home |
| `CODEX_SQLITE_HOME` | Override SQLite state DB location |
| `CODEX_CA_CERTIFICATE` | Custom CA bundle path |
| `SSL_CERT_FILE` | Fallback CA certificate |
| `CODEX_SANDBOX_ENV_VAR` | Internal sandbox detection |
| `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR` | Internal network status |
| `CODEX_OSS_PORT` | Override Ollama/LM Studio port |
| `CODEX_OSS_BASE_URL` | Override Ollama/LM Studio URL |
| `RUST_LOG` | Rust logging level |