# Capy — Tool System

> Model-agnostic platform with VM-based execution and GitHub integration. Detailed tool internals are not publicly available.

## Overview

Capy's tool system operates differently from open-source terminal agents. Rather than exposing a set of named tools (Read, Edit, Bash, Grep) to a single agent, Capy splits capabilities between two agents with hard boundaries — Captain gets read-only exploration tools, Build gets full execution tools.

Because Capy is a closed-source commercial product, the exact tool definitions and internal APIs are not publicly documented. What follows is reconstructed from blog posts, marketing materials, and observable behavior.

## Model-Agnostic Design

Capy's most notable tool-layer decision is **model agnosticism**. The platform supports multiple LLM providers:

| Provider | Model | Notes |
|----------|-------|-------|
| Anthropic | Claude Opus 4.6 | Best benchmark results (Terminal-Bench #7) |
| OpenAI | GPT-5.3 Codex | Code-optimized model |
| Google | Gemini 3 Pro | Large context window |
| xAI | Grok 4 Fast | Speed-optimized |
| Moonshot | Kimi K2 | — |
| Zhipu | GLM 4.7 | — |
| Alibaba | Qwen 3 Coder | Code-optimized |

Users can choose which model to use per task, enabling cost/quality tradeoffs. It's not publicly documented whether Captain and Build can use different models independently.

## Captain's Tool Set (Planning Phase)

Captain has **read-only** capabilities:

- **Codebase reading**: Can explore files, directory structures, and understand code patterns
- **Research**: Can investigate documentation and context relevant to the task
- **User interaction**: Can ask the user clarifying questions in a conversational loop
- **Spec writing**: Produces a structured specification as output

**Hard restrictions**: Captain cannot write files, run terminal commands, or interact with git. These restrictions are enforced at the platform level, not just via prompt instructions.

## Build's Tool Set (Execution Phase)

Build has **full execution** capabilities within a sandboxed Ubuntu VM:

- **File editing**: Create, modify, and delete files in the codebase
- **Terminal execution**: Full shell access with sudo privileges
- **Dependency management**: Install packages, run build tools
- **Test execution**: Run test suites and observe results
- **Git operations**: Commit changes, create branches (via worktrees)
- **PR creation**: Open pull requests on GitHub

**Hard restrictions**: Build cannot ask the user questions or request clarification. It must work from the spec alone.

## GitHub Integration

Capy integrates directly with GitHub for the full development lifecycle:

- **PR Management**: Build agent can create and update pull requests
- **Code Review**: Review and discuss code within the Capy interface
- **Issue Tracking**: Track and reference GitHub issues from tasks
- **Branch Management**: Automatic worktree creation for branch isolation

## Execution Environment

Each task runs in its own **sandboxed Ubuntu VM**:

- Full operating system environment
- Sudo access for the Build agent
- Isolated from other tasks and users
- Network access for package managers and external APIs
- Git worktree for branch isolation

This is more heavyweight than the sandboxing used by terminal agents (which typically use filesystem restrictions or Docker containers). Each Capy task gets a full VM, which enables true parallel execution without resource contention.

## What's Not Publicly Known

- Exact tool definitions and function signatures
- How tool calls are formatted and parsed per model provider
- Whether there's a tool correction or retry mechanism
- MCP (Model Context Protocol) support status
- Rate limiting or resource quotas per VM
- How the platform handles tool failures during Build execution
- Whether Build has access to web browsing or external documentation tools
