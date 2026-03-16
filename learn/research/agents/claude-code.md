# Claude Code

## Overview

Claude Code is Anthropic's terminal-native coding agent, invoked by running `claude` in a terminal session. It connects a conversational interface backed by Claude's reasoning capabilities directly to the user's filesystem and shell. There is no web browser, no sandboxed environment, and no visual editor. The agent operates in the user's actual development context, reading and writing real files, executing real commands, and interacting with real tools.

This simplicity is deceptive. Beneath the minimal terminal interface lies a sophisticated architecture that manages conversation state across long-running sessions, orchestrates tool execution with real-time streaming, enforces a tiered permission model that balances autonomy with safety, and recovers gracefully from errors. Claude Code represents a pure expression of the terminal-first philosophy: the agent works where the developer works, with the same tools the developer uses, at the speed of the developer's own thinking.

Claude Code's significance for agent builders extends beyond its capabilities. Its architecture demonstrates how a production-grade agent handles the hard problems that emerge at scale: context windows that fill up mid-task, tool calls that fail unexpectedly, permission decisions that must be made in milliseconds, and streaming responses that need to render coherently while the model is still generating them.

## Architecture

Claude Code is structured as a layered system with well-defined boundaries between components. At the outermost layer sits the terminal REPL, which accepts user input and renders streaming output. Below that, a conversation manager maintains the message history, handles context compaction when the conversation exceeds the model's context window, and constructs API requests with the appropriate system prompt, tools, and conversation state.

The API client communicates with the Anthropic API using server-sent events (SSE) for streaming. Responses from the API contain two types of content blocks: text blocks (natural language the user sees) and tool use blocks (structured requests to execute a tool). A tool dispatcher receives tool use blocks, routes them to the appropriate handler, and collects results. The permission system intercepts tool calls before execution, evaluating each call against the permission policy and requesting user approval when the operation exceeds the agent's granted autonomy.

The data flow follows a clear cycle. User input enters the conversation manager, which sends a request to the API. The streaming response is parsed in real time, with text rendered to the terminal as it arrives and tool calls dispatched as they complete. Tool results are appended to the conversation history as observations, and the updated conversation is sent back to the API for the next iteration. This cycle repeats until the model returns a response with no tool use blocks.

## Key Patterns

**Open-ended agentic loop.** Claude Code's loop has no hardcoded iteration limit. The model decides when the task is complete by returning a response without tool calls. If writing code and running tests takes three iterations, the loop runs three times. If debugging a complex issue requires twenty iterations of reading files, forming hypotheses, and testing fixes, the loop runs twenty times. This open-ended design is what gives the agent its ability to handle tasks that were never anticipated at design time. A safety heuristic pauses the loop after an unusually large number of tool calls and checks in with the user, preventing runaway loops where the model keeps trying variations of a broken approach.

**Diff-based file editing.** Rather than rewriting entire files, Claude Code uses a string-replacement approach to file editing. The model specifies what text to find and what to replace it with. This is more precise than full-file replacement, reduces the chance of accidentally destroying content, generates smaller payloads when files are large, and makes it easy for the user to review what changed.

**Tiered permissions.** The permission model distinguishes between three categories of operations. Read operations (file reads, searches, directory listings) are freely permitted because they do not change anything. Write operations require approval with exceptions for files the agent created during the current session. Shell commands are categorized by risk level, with safe commands like `ls` and `git status` running freely and potentially dangerous commands requiring explicit approval. Users can configure allow-lists for commands the agent may run without asking.

**Context compaction.** When the conversation grows too long for the model's context window, Claude Code summarizes earlier portions of the conversation. The model creates a compressed memory of what it has already done, preserving key facts and decisions while discarding raw details. This trades detail for continued capacity, allowing the agent to work on long tasks without losing the thread of its progress.

## Implementation Details

The streaming infrastructure parses SSE events from the Anthropic API in real time. Events encode partial text, tool use starts, tool use completions, and metadata. The client renders text to the terminal as it arrives (character by character for a responsive feel) and dispatches tool calls as they complete. This requires careful state management: the client must track which tool calls are in progress, buffer partial JSON for tool parameters, and handle the interleaving of text and tool content within a single response.

The tool system is statically defined. Every tool is known at startup and described in the system prompt. There is no dynamic tool registration or runtime tool discovery. Each tool has a name, a description, and a JSON Schema parameter specification. The model calls tools by name with structured arguments. The static approach means the system prompt is predictable and the model always knows exactly what it can do.

The system prompt itself is constructed dynamically at each API call, incorporating the available tools, the current working directory, project-specific context from configuration files, and behavioral guidelines. This prompt engineering is a critical part of the architecture: it shapes how the model reasons about the codebase, which tools it reaches for, and how aggressively it pursues multi-step plans.

Error handling follows the "error as information" pattern. When a tool call fails, the error message is returned to the model as the tool result observation. The model then decides how to proceed, often recovering by trying an alternative approach, adjusting its parameters, or asking the user for clarification.

## Cross-References

- [The Agentic Loop](/project/03-the-agentic-loop/01-what-is-an-agentic-loop) covers the core loop architecture that mirrors Claude Code's perceive-reason-act cycle
- [Building a Tool System](/project/04-building-a-tool-system/01-what-are-tools) explains tool registration, dispatch, and execution patterns used throughout Claude Code
- [File Operations Tools](/project/05-file-operations-tools/03-edit-tool-string-replace) details the diff-based editing approach Claude Code uses for file modifications
- [Shell Execution](/project/06-shell-execution/01-process-spawning) covers process spawning and output capture, the foundation of Claude Code's shell tool
- [Streaming Responses](/project/07-streaming-responses/02-sse-protocol) explains the SSE protocol and streaming state machine that powers real-time rendering
- [Context Management](/project/09-conversation-context-management/07-context-compaction-strategies) discusses compaction strategies similar to Claude Code's approach
- [Permission and Safety](/project/12-permission-and-safety/02-permission-levels) covers the tiered permission model that Claude Code implements
