---
title: OpenCode
category: agents
status: complete
---

# OpenCode

> Open-source Go-based CLI coding agent with a rich terminal UI, built by Anomaly Innovations (now maintained by the Charm team as "Crush").

## What Is OpenCode?

OpenCode is a **Go-based terminal coding agent** that provides an interactive TUI (Terminal User Interface) for AI-assisted software development. It supports multiple LLM providers (Anthropic, OpenAI, Google Gemini, AWS Bedrock, Groq, Azure, OpenRouter, GitHub Copilot, xAI, and local models) and includes a comprehensive set of coding tools—file editing, shell execution, code search, LSP integration, and sub-agent delegation.

Unlike many coding agents built in Python or TypeScript, OpenCode is written entirely in **Go** using the [Bubble Tea](https://github.com/charmbracelet/bubbletea) TUI framework from Charm. This gives it a distinctive niche: fast startup, a single static binary, and a polished terminal-native experience with vim-like keybindings.

## Who Built It

OpenCode was created by **Anomaly Innovations** (anomalyco). The project was originally authored by Jay V (GitHub: jayair), who is also known for creating SST (Serverless Stack). The project has since been **archived** and continued under the name **[Crush](https://github.com/charmbracelet/crush)** by the original author and the Charm team (charmbracelet).

## Why It Matters

1. **Go in the agent space**: Most coding agents are Python/TypeScript. OpenCode demonstrates that Go is a viable language for building agentic systems, with advantages in binary distribution, startup time, and concurrency.

2. **TUI-first design**: While most agents are CLI-first with basic text output, OpenCode built a full Bubble Tea TUI with session management, model switching dialogs, permission dialogs, file diff views, and vim-like editor integration.

3. **Provider abstraction**: Clean multi-provider support with a unified `Provider` interface, supporting 10+ providers through native SDKs (not just OpenAI-compatible APIs).

4. **Community adoption**: The website claims 120K+ GitHub stars, 800+ contributors, and 5M+ monthly developers (across the broader OpenCode/Crush ecosystem).

## Tech Stack

| Component | Technology |
|-----------|------------|
| Language | Go 1.24 |
| TUI Framework | Bubble Tea (charmbracelet/bubbletea) |
| Styling | Lip Gloss (charmbracelet/lipgloss) |
| Database | SQLite (ncruces/go-sqlite3 via Wasm) |
| Migrations | Goose (pressly/goose) |
| SQL Generation | sqlc |
| CLI Framework | Cobra (spf13/cobra) |
| Config | Viper (spf13/viper) |
| LLM SDKs | anthropic-sdk-go, openai-go, google/genai |
| MCP | mark3labs/mcp-go |
| Markdown | Glamour (charmbracelet/glamour) |
| Themes | Catppuccin |

## Current Status

- **Archived**: The opencode-ai/opencode repository is archived
- **Successor**: Continued as [Crush](https://github.com/charmbracelet/crush) under Charm
- **License**: MIT
- **Last active version**: v0.x (early development)

## Benchmark Performance

- **Terminal-Bench 2.0**: Rank #50 (Claude Opus 4.5, 51.7%)
- **Note**: As an early-stage project, benchmark scores reflect its nascent tooling maturity rather than architectural limitations

## Key Files for Understanding the Codebase

| File | Purpose |
|------|---------|
| `internal/llm/agent/agent.go` | Core agentic loop |
| `internal/llm/agent/tools.go` | Tool registration per agent type |
| `internal/llm/provider/provider.go` | Provider abstraction interface |
| `internal/llm/tools/tools.go` | Tool interface definition |
| `internal/tui/tui.go` | Main TUI application |
| `internal/app/app.go` | Application initialization |
| `internal/session/session.go` | Session management |
| `internal/message/message.go` | Message persistence |
| `internal/pubsub/broker.go` | Generic pub/sub event system |
| `internal/permission/permission.go` | Permission request/grant flow |
