# OpenCode

## Overview

OpenCode is an open-source, terminal-native coding agent written in Go. Unlike commercial agents whose internals must be inferred from behavior, OpenCode's entire architecture is visible in its source code, its design trade-offs are documented in its commit history, and every pattern is available for study and adaptation. For agent builders, this transparency makes OpenCode an invaluable reference implementation.

OpenCode provides a rich text-based user interface built with the Bubble Tea framework, supports multiple LLM providers through a clean abstraction layer, and implements the full agentic loop with tools for file operations, shell execution, and code navigation. Its Go implementation demonstrates how the agent patterns that originate in research and commercial products translate into a practical, community-maintained open-source project.

What sets OpenCode apart architecturally is the combination of its provider abstraction and its terminal UI sophistication. The provider layer means users are not locked into a single LLM vendor, and the Bubble Tea interface means the agent presents information clearly even when generating high volumes of streaming output, tool call notifications, and command results.

## Architecture

OpenCode's architecture is organized around a central agent loop that coordinates between the LLM provider layer, the tool dispatch system, the session database, and the terminal UI.

The provider abstraction layer sits between the agent core and the LLM APIs. It defines a common interface that any provider must implement: methods for sending prompts and receiving responses, handling streaming, and listing available models. Concrete implementations exist for Anthropic, OpenAI, Google, and local models through Ollama. The agent core communicates exclusively through this interface, so switching providers requires no changes to the loop, tool dispatch, or UI code.

The terminal UI is built with Bubble Tea, which implements The Elm Architecture: a unidirectional data flow pattern consisting of a Model (application state), an Update function (state transitions triggered by messages), and a View function (rendering the current state to the screen). When the agent receives new content from the LLM, it dispatches an update message that modifies the state. The view function re-renders based on the new state. This pattern eliminates two-way data binding, implicit state mutations, and race conditions between UI updates.

The session database persists conversations to local storage, allowing users to resume previous sessions. Messages, tool calls, tool results, and metadata like timestamps and token counts are all stored in structured form. This enables both session resumption and conversation replay for debugging.

## Key Patterns

**Provider abstraction through interfaces.** Go's structural typing makes the provider abstraction lightweight. A type satisfies an interface if it has the right methods, without explicit declaration. The provider interface includes methods for chat completion, streaming, and model listing. Adding a new provider means implementing these methods on a new struct. This decoupling is a critical lesson for agent builders: the model landscape changes fast, and a provider-agnostic core means swapping models requires implementing an interface, not rewriting the agent's brain.

**Dynamic tool registry.** OpenCode's tool system uses a registry pattern. Tools register themselves at startup by providing a name, description, parameter schema, and execution function. When the model requests a tool call, the dispatcher looks up the tool by name, validates parameters against the schema, and calls the execution function. This makes adding new tools straightforward: implement the tool, register it at startup, and update the system prompt. Community contributors can add tools without understanding the core loop. The trade-off is that the full set of tools depends on runtime initialization code rather than being visible in a single dispatch statement.

**LSP integration for semantic code navigation.** Beyond simple text search, OpenCode integrates with the Language Server Protocol to provide semantic code understanding. The agent can ask for symbol definitions, references, and type information. This moves code navigation from string matching to structural understanding, allowing the agent to find where a function is defined, what calls it, and what type it returns. Not all agents offer this capability, and it represents a meaningful improvement in code comprehension for complex projects.

**Error transparency.** When a tool call fails, OpenCode returns the error message to the model as the tool result rather than crashing or silently retrying. The model then decides how to proceed. This pattern leverages the model's reasoning capabilities for error recovery. The model might try a different file path, use a different command, or ask the user for help. Passing errors back as information rather than hiding them is a key insight that improves agent resilience.

## Implementation Details

The Elm Architecture in Bubble Tea structures the entire UI around immutable state transitions. Each user action, LLM event, or tool completion generates a message that flows through the Update function, producing a new state. The View function renders this state to the terminal. For a coding agent that produces diverse output types (streaming text, syntax-highlighted code, tool notifications, command output, status updates), this architecture keeps the UI predictable and testable. Individual components like the conversation view, status bar, and input box are composed as nested models, each handling their own messages and rendering.

OpenCode's session persistence stores the full conversation in a local database, including message content, tool call parameters, tool results, timestamps, and token counts. This structured storage enables analytics: how many tool calls does the average task require? Which tools are most frequently used? Where do failures tend to occur? For agent developers iterating on their system's performance, this data is invaluable.

Configuration is handled through a TOML file. Users set their preferred provider, model, API keys, tool permissions, and UI preferences. The configuration extends to the system prompt: users can provide custom instructions that are prepended to the default system prompt, allowing project-specific conventions, preferred coding styles, or domain knowledge to be injected without modifying the agent's code. This configuration-driven approach means the agent adapts to the user's environment at runtime.

File operations in OpenCode use full-file replacement rather than diff-based editing. When the model modifies a file, it writes the entire new content. This is simpler to implement but less efficient for large files and more error-prone when the model must reproduce unchanged portions of the file accurately. Diff-based editing, as used by Claude Code, avoids these issues at the cost of more complex tool implementation.

Shell execution streams command output back to the model in real time rather than waiting for the command to complete. This is important for long-running commands like test suites: the agent sees output as it is produced and can reason about partial results before the command finishes.

## Cross-References

- [Multi-Provider Support](/project/13-multi-provider-support/01-why-multiple-providers) covers the provider abstraction pattern that is central to OpenCode's architecture
- [Terminal UI with Ratatui](/project/08-terminal-ui-with-ratatui/04-elm-architecture) explains The Elm Architecture pattern that Bubble Tea and Ratatui both implement
- [Tool Registry](/project/04-building-a-tool-system/05-tool-registry) details the registry pattern that OpenCode uses for dynamic tool dispatch
- [Search and Code Intelligence](/project/10-search-and-code-intelligence/05-tree-sitter-intro) covers AST-based code navigation, related to OpenCode's LSP integration
- [Session Persistence](/project/09-conversation-context-management/05-session-persistence) discusses conversation storage patterns similar to OpenCode's session database
- [Configuration Management](/project/15-production-polish/03-config-file-management) covers TOML-based configuration approaches matching OpenCode's design
- [Error Handling in Tools](/project/04-building-a-tool-system/09-error-handling) explains the error-as-information pattern that OpenCode implements
