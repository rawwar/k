---
title: The Landscape Today
description: A survey of the current coding agent ecosystem including commercial products, open-source projects, and emerging research prototypes.
---

# The Landscape Today

> **What you'll learn:**
> - The major categories of coding agents available today and their target use cases
> - How commercial and open-source agents differ in architecture, capabilities, and trade-offs
> - The current state of the market and where the technology is heading next

## A Crowded and Fast-Moving Field

The coding agent landscape in 2025 and 2026 is vibrant, competitive, and evolving at a pace that makes any snapshot partially obsolete within months. New agents launch regularly, existing agents gain significant capabilities with each update, and the boundary between "coding agent" and "AI-enhanced IDE" continues to blur.

Despite this rapid evolution, the landscape has settled into recognizable categories. Understanding these categories helps you make sense of new entrants and — more importantly for our purposes — helps you identify the architectural patterns that persist across all of them.

## Category 1: Terminal-Native Agents

Terminal-native agents run in your shell. You launch them from the command line, they operate in your working directory, and they interact with your actual filesystem and toolchain. There's no browser, no cloud sandbox, no GUI — just a conversation in your terminal that happens to be backed by an LLM with access to your tools.

**Claude Code** (Anthropic) is the most prominent example. You run `claude` in your terminal, describe a task, and it goes to work — reading files, writing changes, running commands, and iterating. It operates with the full permissions of your user account, which gives it maximum power but also maximum responsibility. Its permission system asks for approval before dangerous operations, but by default it can read any file you can read and run any command you can run.

**OpenCode** is an open-source terminal agent written in Go. It offers a rich terminal UI using the Bubble Tea framework, supports multiple LLM providers (Anthropic, OpenAI, Google, and local models), and implements the full agentic loop with tools for file operations and shell execution. Because it's open source, you can read every line of its architecture — which makes it an excellent reference for building your own.

The terminal-native approach has a philosophical appeal: it meets developers where they already work. If you spend your day in a terminal with tmux, vim, and git, a terminal agent feels like a natural extension of your workflow rather than a context switch.

::: python Coming from Python
If you're a Python developer who works primarily in VS Code or PyCharm, the terminal-native approach might feel unfamiliar. But think of it this way: terminal agents are like sophisticated Python scripts that can run anywhere you have a shell. There's no IDE plugin to install, no compatibility to worry about, and the agent has the same access to your environment that you do when you type commands at the prompt. Many Python developers find that once they try a terminal agent, they appreciate the simplicity and power.
:::

## Category 2: Sandboxed Cloud Agents

Sandboxed cloud agents run your code in an isolated environment rather than on your local machine. They typically clone your repository into a container, perform their work there, and present the results as a set of changes (often as a pull request or a diff).

**Codex** (OpenAI) is the flagship example. Each Codex task runs in a cloud container with no network access. The agent can read and write files, execute commands, and run tests — but only within the sandbox. When it's done, you review the changes and decide whether to apply them. This architecture prioritizes safety: even if the agent generates a malicious command, it can't reach your production systems.

The trade-off is responsiveness. Because the agent works in a copy of your code rather than on your actual files, there's a disconnect between what it does and what you see. You submit a task, wait, and review the result — rather than watching the agent work in real time on your codebase. For some workflows (background code review, bulk refactoring), this is perfectly fine. For interactive development, it can feel slow.

## Category 3: IDE-Integrated Agents

IDE-integrated agents embed the agentic capability directly into your editor. Rather than a separate terminal session or cloud service, the agent lives in a sidebar or panel within the IDE, with direct access to the editor's file system, language services, and debugging tools.

**Cursor** and **Windsurf** are the most prominent examples. They started as AI-enhanced editors built on VS Code, offering chat-based code generation. Over time, they've added increasingly agentic features: the ability to apply multi-file changes, run commands, and iterate on test results. They blur the line between "IDE with AI features" and "coding agent with an editor UI."

The advantage of IDE integration is discoverability and visual feedback. You can see changes as diffs in your editor, approve them per-file, and maintain your existing editor workflow. The disadvantage is coupling — these agents are tied to a specific editor and can be harder to script, automate, or use in CI/CD pipelines.

## Category 4: Open-Source and Community Agents

The open-source agent ecosystem is where you find the greatest variety and the most transparency. These projects let you read the source code, understand every architectural decision, and customize the agent to your needs.

**Pi** is a Rust-based coding agent that emphasizes type safety and composable tool design. It leverages Rust's type system to model agent state transitions, ensuring at compile time that invalid states are unrepresentable. Its tool system uses traits to define a common interface that all tools implement, making it straightforward to add new capabilities.

**Aider** is a Python-based terminal agent focused on git-integrated code editing. It works by generating diffs that it applies to your codebase, committing each change automatically. Its architecture is simpler than some others — it focuses on the edit-commit cycle rather than general tool use — but this simplicity makes it approachable and effective for many tasks.

**Goose** (by Block) is another open-source agent focused on extensibility through a modular tool system. It supports custom tool providers, allowing you to extend the agent with domain-specific capabilities.

::: tip In the Wild
The open-source agents are the richest learning resources for aspiring agent builders. OpenCode's Go codebase shows clean separation between the UI layer (Bubble Tea), the agent core (provider-agnostic LLM interface), and the tool system (file operations, shell execution). Pi's Rust codebase demonstrates how to use Rust's trait system and enums to build a type-safe tool dispatch layer. When you build your own agent in this tutorial, you'll draw on patterns from both.
:::

## Commercial vs. Open-Source: The Real Differences

The divide between commercial and open-source agents isn't just about price. It reflects genuinely different design philosophies.

**Commercial agents** (Claude Code, Codex, Cursor) invest heavily in polish, safety, and integration. They have dedicated teams working on permission models, content filtering, abuse prevention, and seamless onboarding. They tend to use proprietary models and may include features like extended thinking, model-specific optimizations, and usage analytics. Their architecture is typically opaque — you can observe the behavior but not the implementation.

**Open-source agents** (OpenCode, Pi, Aider) invest in transparency, flexibility, and community. Their architectures are fully visible, which means you can learn from them, fork them, and adapt them. They typically support multiple LLM providers, so you're not locked into one vendor. But they may lack the polish, safety features, and scale of commercial tools.

For this tutorial, we draw from both worlds. We study the architectural patterns visible in commercial agents (through their documentation, behavior, and published research) and the implementation details visible in open-source agents (through their actual code). The agent you'll build combines the best ideas from across the landscape.

## The Convergence Thesis

Despite their surface-level differences, these agents are converging on a remarkably similar architecture. Every agent in every category has:

1. **An LLM integration layer** that sends prompts and receives responses (with tool calls).
2. **A tool system** that translates model requests into real-world actions.
3. **An agentic loop** that orchestrates perception, reasoning, and action in cycles.
4. **A context management strategy** for handling conversation history and token limits.
5. **A permission or safety model** that controls what the agent is allowed to do.

The next four subchapters examine how four specific agents — Claude Code, OpenCode, Pi, and Codex — implement these components. In the subchapter that follows, we'll extract the common patterns into a unified architectural model that will serve as the blueprint for what we build.

## Where Is It All Going?

Several trends are shaping the near future of coding agents:

**Multi-agent systems.** Rather than one agent doing everything, systems of specialized agents collaborate — one for code generation, one for testing, one for code review. The orchestration of these agents is itself an active research area.

**Persistent memory.** Current agents mostly start fresh with each session. Future agents will remember your codebase conventions, your preferred patterns, your past decisions — building a project-specific knowledge base over time.

**Self-improving tool use.** Agents that can write and register their own tools, extending their capabilities at runtime based on the task at hand.

**Smaller, specialized models.** While current agents rely on large frontier models, the trend toward smaller models fine-tuned for specific tasks could make agents faster, cheaper, and more predictable.

Understanding the current landscape in depth — not just what each agent does, but *how* it does it — positions you to build agents that incorporate today's best patterns while remaining open to tomorrow's advances.

## Key Takeaways

- The coding agent landscape divides into four categories: terminal-native (Claude Code, OpenCode), sandboxed cloud (Codex), IDE-integrated (Cursor, Windsurf), and open-source community (Pi, Aider, Goose) — each with distinct trade-offs between power, safety, and user experience.
- Despite surface-level differences in language, UI, and deployment model, all production coding agents converge on the same five core components: LLM integration, tool system, agentic loop, context management, and permission model.
- Open-source agents provide the most valuable learning resources for agent builders because their architecture is fully transparent and their design decisions are documented in code.
- The field is moving toward multi-agent collaboration, persistent memory, and self-improving tool use — trends that the foundational architecture you'll learn in this track positions you to adopt.
