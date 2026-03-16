---
title: Claude Code Analysis
description: A deep technical analysis of Claude Code's architecture, tool system, permission model, and agentic loop design.
---

# Claude Code Analysis

> **What you'll learn:**
> - How Claude Code structures its agentic loop and manages conversation state
> - The design of Claude Code's tool system including file operations, shell execution, and search
> - How Claude Code's permission model balances autonomy with user safety

## The Terminal-First Agent

Claude Code is Anthropic's flagship coding agent, and it represents perhaps the purest expression of the terminal-native philosophy. When you run `claude` in your terminal, you get a conversational interface backed by Claude's full reasoning capabilities, with direct access to your filesystem and shell. There's no web browser, no sandboxed environment, no visual editor — just a powerful model connected to your actual development tools.

This simplicity is deceptive. Beneath the minimal interface lies a sophisticated architecture that manages conversation state, orchestrates tool execution, handles permissions, streams responses in real time, and recovers gracefully from errors. Let's examine each layer.

## The Agentic Loop

Claude Code's agentic loop follows the perceive-reason-act pattern we defined earlier, but with several refinements that make it practical at scale.

When you submit a prompt, Claude Code constructs a message containing your input plus a system prompt that describes the available tools, the current working directory, project context, and behavioral guidelines. It sends this to the Anthropic API, which returns a response. That response can contain two types of content: **text blocks** (natural language the user sees) and **tool use blocks** (structured requests to execute a tool).

If the response contains tool use blocks, Claude Code executes each tool, collects the results, appends them to the conversation as tool result messages, and sends the updated conversation back to the API. This cycle repeats until the model returns a response with no tool use blocks — signaling that it considers the task complete (or needs human input).

Here's the critical insight: **the model decides when to stop**. There's no hardcoded limit on the number of loop iterations. The model uses each tool result to assess whether the task is done. If it writes code and runs tests that pass, it might stop after three iterations. If it encounters a complex bug requiring multiple investigation steps, it might iterate twenty times. This open-ended loop is what gives Claude Code its ability to handle tasks that no one anticipated at design time.

::: tip In the Wild
Claude Code's loop includes a "stop after" heuristic for safety — if the agent has executed a very large number of tool calls without completing the task, it pauses and checks in with the user. This prevents runaway loops where the model keeps trying variations of a broken approach. When you build your own agent, you'll implement a similar circuit breaker.
:::

## The Tool System

Claude Code's tool system is where the rubber meets the road. The model can reason all day, but without tools, it can't change a single byte on your filesystem. Claude Code provides a carefully curated set of tools:

**File reading tools** allow the model to read file contents, search for patterns across files (similar to `grep` or `ripgrep`), list directory contents, and check file metadata. These are the perception tools — they let the agent understand the codebase it's working with.

**File writing tools** let the model create and modify files. Rather than rewriting entire files from scratch, Claude Code uses a diff-based editing approach — specifying what text to find and what to replace it with. This is more precise than overwriting, reduces the chance of accidentally destroying content, and generates smaller payloads when files are large.

**Shell execution** gives the model the ability to run arbitrary commands in your terminal. This is simultaneously the most powerful and most dangerous tool. With shell access, the agent can run tests (`pytest`), install dependencies (`pip install`), check git status, build projects, and perform any operation you could type at the prompt. But it can also delete files, modify system configurations, or run network-intensive operations.

**Search tools** let the model find relevant code across the codebase. Rather than reading every file (which would consume enormous amounts of context), the agent can search for function names, class definitions, import statements, and other patterns. This is how the agent efficiently navigates large projects.

The tool system is **statically defined** — every tool is known at startup, and the model is told about all available tools in the system prompt. There's no dynamic tool registration or runtime tool discovery. This simplicity means the system prompt is predictable and the model always knows what it can do.

::: python Coming from Python
If you've worked with Python decorators to register functions in a framework (like Flask's `@app.route`), you're familiar with the concept of a tool registry. Claude Code's tool system works similarly — each tool has a name, a description, and a parameter schema, and the model "calls" them by name with the appropriate arguments. The difference is that the "caller" is an LLM rather than a web request router.
:::

## The Permission Model

Claude Code operates on your local machine with your user permissions. It can read any file you can read, and it can execute any command you can execute. This power demands a permission model.

Claude Code's permission system operates on a tiered model:

**Read operations** are generally permitted without explicit approval. The agent can freely read files, search codebases, and list directories. The reasoning is that reading doesn't change anything — the risk is low, and requiring approval for every file read would make the agent unusably slow.

**Write operations** require more care. Claude Code asks for user approval before writing to files, with some exceptions for files the agent created during the current session. This balances productivity (the agent can iterate quickly on files it just created) with safety (it won't silently modify existing files without your knowledge).

**Shell commands** are the most sensitive. Claude Code categorizes commands by risk level. Safe commands (like `ls`, `cat`, `git status`) can execute freely. Potentially dangerous commands (like those involving `rm`, `sudo`, or pipe chains) require explicit approval. The user can also configure an allow-list of commands the agent can run without asking.

This tiered approach reflects a design philosophy: **optimize for flow while preserving safety at the boundaries**. The agent should be able to read, think, and iterate rapidly. But before it changes something important, it checks with the human.

## Context and State Management

Managing context in a long-running agent session is one of the hardest engineering problems. Claude Code's conversation can grow to tens of thousands of tokens as the model reads files, executes commands, and receives results. Eventually, the conversation exceeds the model's context window.

Claude Code addresses this with a **context compaction** strategy. When the conversation grows too long, it summarizes earlier portions — preserving the key facts and decisions while discarding the raw details. The model effectively creates a compressed memory of what it's already done, freeing up context space for new operations.

This is a pragmatic solution to a fundamental limitation. Models have finite context windows, but real-world tasks have unbounded information needs. Compaction lets the agent work on long tasks without losing the thread of what it's been doing.

## Streaming and User Experience

Claude Code streams responses to the terminal as they're generated, giving you real-time visibility into the agent's thinking and actions. When the model generates text, you see it appear character by character. When it invokes a tool, you see the tool name and parameters before execution begins. When a tool returns its result, you see the output.

This streaming approach serves both practical and psychological purposes. Practically, it means you don't wait for the entire response to generate before seeing the first word. Psychologically, it builds trust — you can see what the agent is doing at every step and intervene if something looks wrong.

The streaming infrastructure is non-trivial. The Anthropic API returns server-sent events (SSE) that encode partial text, tool use starts, tool use completions, and metadata. Claude Code's client parses this event stream in real time, rendering text to the terminal and dispatching tool calls as they arrive.

## Architecture Summary

Pulling it all together, Claude Code's architecture looks like this:

1. **Interface layer:** Terminal REPL that accepts user input and renders streaming output.
2. **Conversation manager:** Maintains the message history, handles context compaction, and constructs API requests.
3. **API client:** Communicates with the Anthropic API, sending messages and receiving streamed responses.
4. **Tool dispatcher:** Receives tool use blocks from the API response, routes them to the appropriate tool handler, and collects results.
5. **Tool implementations:** Individual tools for file reading, file writing, shell execution, search, and other operations.
6. **Permission system:** Intercepts tool calls before execution, evaluates them against the permission policy, and requests user approval when needed.

Each of these components maps to something you'll build in this tutorial. The interface layer is your REPL (Chapter 2). The conversation manager is your state management (Chapter 4). The API client is your LLM integration (Chapter 3). The tool dispatcher and implementations are your tool system (Chapters 5-8). The permission system is your safety layer (Chapter 12).

## What We'll Borrow

From Claude Code, we'll adopt several patterns:

- **The open-ended agentic loop** where the model decides when the task is complete.
- **Diff-based file editing** rather than full file replacement.
- **Tiered permissions** that distinguish between read, write, and execute operations.
- **Streaming response rendering** for real-time user feedback.
- **Context compaction** to handle long-running sessions.

These aren't Claude Code-specific innovations — as we'll see, they appear across multiple agents. But Claude Code implements them in a particularly clean, well-documented way that makes them easy to study and adapt.

## Key Takeaways

- Claude Code's architecture centers on an open-ended agentic loop where the model iterates through tool calls until it determines the task is complete, with no hardcoded iteration limit.
- The tool system is statically defined with tools for file reading, diff-based file editing, shell execution, and code search — each categorized by risk level in the permission model.
- Claude Code's permission model uses a tiered approach: reads are freely permitted, writes require approval with exceptions for agent-created files, and shell commands are categorized by risk.
- Context compaction allows Claude Code to handle long sessions by summarizing earlier conversation history, trading raw detail for continued context capacity.
- The streaming architecture provides real-time visibility into the agent's actions, building user trust and enabling early intervention when something goes wrong.
