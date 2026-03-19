# OpenCode — References

## Primary Sources

### GitHub Repository
- **Original repo (archived)**: https://github.com/opencode-ai/opencode
- **Successor project (Crush)**: https://github.com/charmbracelet/crush
- **License**: MIT

### Documentation
- **Official website**: https://opencode.ai
- **Documentation site**: https://opencode.ai/docs
- **Configuration reference**: https://opencode.ai/docs/config
- **Provider directory**: https://opencode.ai/docs/providers
- **Theme gallery**: https://opencode.ai/docs/themes
- **Custom commands**: https://opencode.ai/docs/commands

### Installation
- **Install script**: `curl -fsSL https://opencode.ai/install | bash`
- **Homebrew**: `brew install anomalyco/tap/opencode`
- **npm**: `npm install -g opencode-ai`
- **Go**: `go install github.com/opencode-ai/opencode@latest`

## Key Source Code Files

| File | Description |
|------|-------------|
| [`internal/llm/agent/agent.go`](https://github.com/opencode-ai/opencode/blob/main/internal/llm/agent/agent.go) | Core agentic loop implementation |
| [`internal/llm/agent/tools.go`](https://github.com/opencode-ai/opencode/blob/main/internal/llm/agent/tools.go) | Tool registration for coder and task agents |
| [`internal/llm/agent/agent-tool.go`](https://github.com/opencode-ai/opencode/blob/main/internal/llm/agent/agent-tool.go) | Sub-agent tool implementation |
| [`internal/llm/agent/mcp-tools.go`](https://github.com/opencode-ai/opencode/blob/main/internal/llm/agent/mcp-tools.go) | MCP tool discovery and registration |
| [`internal/llm/provider/provider.go`](https://github.com/opencode-ai/opencode/blob/main/internal/llm/provider/provider.go) | Provider interface and factory |
| [`internal/llm/provider/anthropic.go`](https://github.com/opencode-ai/opencode/blob/main/internal/llm/provider/anthropic.go) | Anthropic Claude provider |
| [`internal/llm/provider/openai.go`](https://github.com/opencode-ai/opencode/blob/main/internal/llm/provider/openai.go) | OpenAI provider |
| [`internal/llm/provider/gemini.go`](https://github.com/opencode-ai/opencode/blob/main/internal/llm/provider/gemini.go) | Google Gemini provider |
| [`internal/llm/provider/copilot.go`](https://github.com/opencode-ai/opencode/blob/main/internal/llm/provider/copilot.go) | GitHub Copilot provider |
| [`internal/llm/tools/tools.go`](https://github.com/opencode-ai/opencode/blob/main/internal/llm/tools/tools.go) | BaseTool interface |
| [`internal/llm/tools/bash.go`](https://github.com/opencode-ai/opencode/blob/main/internal/llm/tools/bash.go) | Bash tool with safety controls |
| [`internal/llm/tools/edit.go`](https://github.com/opencode-ai/opencode/blob/main/internal/llm/tools/edit.go) | File editing tool |
| [`internal/llm/tools/view.go`](https://github.com/opencode-ai/opencode/blob/main/internal/llm/tools/view.go) | File reading tool |
| [`internal/llm/tools/patch.go`](https://github.com/opencode-ai/opencode/blob/main/internal/llm/tools/patch.go) | Diff patch tool |
| [`internal/llm/tools/sourcegraph.go`](https://github.com/opencode-ai/opencode/blob/main/internal/llm/tools/sourcegraph.go) | Sourcegraph integration |
| [`internal/llm/models/models.go`](https://github.com/opencode-ai/opencode/blob/main/internal/llm/models/models.go) | Model definitions and registry |
| [`internal/llm/prompt/coder.go`](https://github.com/opencode-ai/opencode/blob/main/internal/llm/prompt/coder.go) | System prompts |
| [`internal/llm/prompt/summarizer.go`](https://github.com/opencode-ai/opencode/blob/main/internal/llm/prompt/summarizer.go) | Summarization prompt |
| [`internal/tui/tui.go`](https://github.com/opencode-ai/opencode/blob/main/internal/tui/tui.go) | Bubble Tea TUI main model |
| [`internal/app/app.go`](https://github.com/opencode-ai/opencode/blob/main/internal/app/app.go) | Application initialization |
| [`internal/session/session.go`](https://github.com/opencode-ai/opencode/blob/main/internal/session/session.go) | Session management |
| [`internal/message/message.go`](https://github.com/opencode-ai/opencode/blob/main/internal/message/message.go) | Message persistence |
| [`internal/pubsub/broker.go`](https://github.com/opencode-ai/opencode/blob/main/internal/pubsub/broker.go) | Generic pub/sub broker |
| [`internal/permission/permission.go`](https://github.com/opencode-ai/opencode/blob/main/internal/permission/permission.go) | Permission system |
| [`cmd/root.go`](https://github.com/opencode-ai/opencode/blob/main/cmd/root.go) | CLI setup |
| [`go.mod`](https://github.com/opencode-ai/opencode/blob/main/go.mod) | Go module dependencies |

## Dependencies

### Key Go Libraries
- **Bubble Tea**: https://github.com/charmbracelet/bubbletea — Terminal UI framework
- **Lip Gloss**: https://github.com/charmbracelet/lipgloss — Declarative terminal styling
- **Glamour**: https://github.com/charmbracelet/glamour — Markdown rendering
- **Cobra**: https://github.com/spf13/cobra — CLI framework
- **Viper**: https://github.com/spf13/viper — Configuration management
- **anthropic-sdk-go**: https://github.com/anthropics/anthropic-sdk-go — Anthropic Go SDK
- **openai-go**: https://github.com/openai/openai-go — OpenAI Go SDK
- **google/genai**: https://pkg.go.dev/google.golang.org/genai — Google GenAI SDK
- **mcp-go**: https://github.com/mark3labs/mcp-go — MCP client library
- **go-sqlite3**: https://github.com/ncruces/go-sqlite3 — SQLite via Wasm
- **goose**: https://github.com/pressly/goose — Database migrations
- **sqlc**: https://sqlc.dev — SQL query code generation
- **Catppuccin**: https://github.com/catppuccin/go — Color palette themes

## Videos

- **OpenCode + Gemini 2.5 Pro demo**: https://www.youtube.com/watch?v=P8luPmEa1QI
  - "BYE Claude Code! I'm SWITCHING To the FASTEST AI Coder!"

## Benchmarks
- **Terminal-Bench 2.0**: https://terminal-bench.com (OpenCode ranks #50, Claude Opus 4.5, 51.7%)

## Related Projects

### Successor
- **Crush** (by Charm team): https://github.com/charmbracelet/crush
  - Continuation of OpenCode under the Charm umbrella
  - Same core team, maintained by original author + Charm

### Competing Agents
- **Claude Code** (Anthropic): https://docs.anthropic.com/en/docs/agents-and-tools/claude-code/overview
- **Aider**: https://github.com/paul-gauthier/aider
- **Cline**: https://github.com/cline/cline
- **Continue**: https://github.com/continuedev/continue
- **Cursor**: https://cursor.sh
- **Codex CLI** (OpenAI): https://github.com/openai/codex

### Charm Ecosystem
The Charm team's suite of Go terminal libraries that OpenCode builds on:
- **Bubble Tea**: https://github.com/charmbracelet/bubbletea
- **Lip Gloss**: https://github.com/charmbracelet/lipgloss
- **Glamour**: https://github.com/charmbracelet/glamour
- **Bubbles**: https://github.com/charmbracelet/bubbles (component library)
- **BubbleZone**: https://github.com/lrstanley/bubblezone (mouse support)

## Organization

### Anomaly Innovations
- **Website**: https://anomaly.co (or anomalyco)
- **Key people**: Jay V (jayair) — SST/Serverless Stack creator
- **Other projects**: SST (https://sst.dev), Ion

### Charm
- **Website**: https://charm.sh
- **GitHub**: https://github.com/charmbracelet
- **Role**: Maintaining the Crush successor, providing the Bubble Tea ecosystem

## Configuration Reference

### Config File Locations
1. `$HOME/.opencode.json` — Global config
2. `$XDG_CONFIG_HOME/opencode/.opencode.json` — XDG config
3. `./.opencode.json` — Project-local config

### JSON Schema
- https://github.com/opencode-ai/opencode/blob/main/opencode-schema.json

### Environment Variables
| Variable | Purpose |
|----------|---------|
| `ANTHROPIC_API_KEY` | Anthropic Claude |
| `OPENAI_API_KEY` | OpenAI |
| `GEMINI_API_KEY` | Google Gemini |
| `GITHUB_TOKEN` | GitHub Copilot |
| `GROQ_API_KEY` | Groq |
| `AWS_ACCESS_KEY_ID` | AWS Bedrock |
| `AWS_SECRET_ACCESS_KEY` | AWS Bedrock |
| `AWS_REGION` | AWS Bedrock |
| `AZURE_OPENAI_ENDPOINT` | Azure OpenAI |
| `AZURE_OPENAI_API_KEY` | Azure OpenAI |
| `LOCAL_ENDPOINT` | Self-hosted models |
| `SHELL` | Default shell |
