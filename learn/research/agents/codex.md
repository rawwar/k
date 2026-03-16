# Codex CLI

## Overview

OpenAI's Codex CLI is a coding agent that takes a fundamentally different approach to the safety-autonomy trade-off than terminal-native agents like Claude Code or OpenCode. Built with Node.js and TypeScript, Codex runs generated code in a sandboxed environment with restricted network access and filesystem isolation. This architectural choice cascades through the entire design, affecting how the agent perceives code, executes commands, delivers results, and interacts with users.

Codex is open-source under the Apache 2.0 license, which means its architecture, sandboxing implementation, and tool system are all available for study and adaptation. The project demonstrates how to build an agent that maximizes safety through isolation rather than through permission checks. Where Claude Code trusts the user to approve dangerous operations and OpenCode lets users control tool permissions through configuration, Codex eliminates the need for these decisions by ensuring that nothing the agent does can affect the world outside its sandbox.

Codex also supports multimodal input, accepting images alongside text prompts. This enables workflows where a user provides a screenshot of a bug, a design mockup, or a visual diagram, and the agent reasons about both the visual and textual context. Combined with tight integration to the OpenAI API ecosystem and its responses API, Codex represents a different point in the design space: one that prioritizes safety guarantees and API ecosystem integration over the interactive flexibility of terminal-native agents.

## Architecture

Codex CLI's architecture centers on the sandbox as the primary safety mechanism. On macOS, sandboxing is achieved through the `seatbelt` system (Apple's sandboxing framework using `sandbox-exec`), which restricts the agent's process to a controlled set of filesystem paths and network capabilities. On Linux, Docker containers provide equivalent isolation. In both cases, the agent operates within an environment where it can read and write files within a designated workspace but cannot access arbitrary filesystem locations or make unrestricted network connections.

The core runtime is implemented in Node.js with TypeScript. The agentic loop follows the same perceive-reason-act pattern as other agents: send a prompt to the OpenAI API, receive a response that may contain tool calls, execute those tools within the sandbox, feed the results back as observations, and repeat until the model signals completion. The TypeScript implementation leverages Node's async/await for non-blocking I/O and its event-driven architecture for streaming response processing.

The OpenAI API integration uses the responses API, which structures interactions around tool calls and model reasoning. Codex supports multiple OpenAI models and can be configured to use different models for different tasks. The API client handles authentication, request construction, streaming response parsing, and error recovery.

The tool system inside the sandbox provides file reading, file writing, and shell execution. These tools operate without permission checks because the sandbox itself is the permission model. The agent can read and write any file in the workspace and execute any available command. Since the sandbox is disposable, there is no risk of persistent damage from a bad operation.

## Key Patterns

**Safety through isolation rather than permission.** This is Codex's defining architectural pattern. Rather than classifying operations as safe or dangerous and requiring approval for dangerous ones (as Claude Code does), Codex makes all operations safe by isolating their effects. The sandbox ensures that even a completely unrestricted agent cannot damage the user's real environment. This eliminates the need for a complex permission system and removes the cognitive burden of approving or denying each operation. The trade-off is that the agent cannot interact with the real environment: it cannot install new dependencies from the network, run integration tests against live services, or access files outside its workspace.

**Platform-specific sandboxing.** Codex adapts its sandboxing mechanism to the host operating system. On macOS, it uses `seatbelt` profiles that declare which filesystem paths and system calls the sandboxed process may access. On Linux, it uses Docker containers with restricted capabilities. This platform-aware approach means the same agent codebase provides consistent safety guarantees across different operating systems, even though the underlying isolation mechanism differs. The abstraction teaches an important design lesson: define your safety contract at a high level (the agent cannot affect the outside world) and implement it differently per platform.

**Multimodal input handling.** Codex accepts images alongside text in prompts, enabling the model to reason about visual content. A user can paste a screenshot of a UI bug, a diagram of a system architecture, or a photo of handwritten pseudocode, and the agent incorporates this visual information into its reasoning. This requires the API client to handle mixed content types in message construction and the UI to support image input workflows.

**Open-source with permissive licensing.** Released under Apache 2.0, Codex's codebase is available for commercial use, modification, and redistribution. This licensing choice has fostered community contributions and enabled other projects to study and adapt Codex's sandboxing approach, tool system, and API integration patterns. For agent builders, the permissive license means any pattern found in Codex can be freely incorporated into their own projects.

## Implementation Details

The sandboxing implementation on macOS uses Apple's Sandbox framework, invoked through `sandbox-exec` with a custom profile. The profile specifies which filesystem paths the process may read and write, which network connections are permitted (typically none or severely restricted), and which system calls are allowed. The profile is generated dynamically based on the workspace location and the agent's configured permissions. This approach provides strong isolation without requiring a container runtime, which makes it lightweight and fast to initialize.

On Linux, Codex uses Docker containers with a minimal base image. The workspace is mounted as a volume, giving the agent access to the code while isolating it from the host filesystem. Network access is controlled through Docker's network configuration, typically set to `none` for full isolation. The container is created at task start and destroyed at task completion, ensuring no state persists between sessions.

The TypeScript implementation structures the agentic loop as an async generator that yields events (text chunks, tool calls, tool results, completion signals). The consumer of this generator handles rendering, user interaction, and session management. This generator pattern provides a clean separation between the loop's logic and its side effects, making the loop testable in isolation by substituting a mock consumer.

Codex integrates with the OpenAI responses API, which provides structured tool call handling, model configuration, and usage tracking. The API client manages conversation state, handles token counting for context window management, and implements retry logic for transient API errors. Model selection is configurable, allowing users to choose between different OpenAI models based on capability and cost requirements.

The tool system within the sandbox is minimal by design. File operations support reading and writing with path validation to ensure the agent stays within its workspace. Shell execution runs commands through the system shell with output capture and timeout handling. There is no separate search tool; the agent uses shell commands like `grep` and `find` for code navigation. This minimalism reflects the philosophy that a smaller tool surface area is easier to secure and reason about, even if it means the agent must compose basic tools to achieve complex operations.

## Cross-References

- [Sandboxing Deep Dive](/project/12-permission-and-safety/09-sandboxing-deep-dive) covers sandbox implementation strategies including container-based and OS-level approaches similar to Codex
- [Permission Levels](/project/12-permission-and-safety/02-permission-levels) contrasts the permission-based approach with Codex's isolation-based safety model
- [Threat Model](/project/12-permission-and-safety/01-threat-model) discusses the security considerations that motivate Codex's sandboxing architecture
- [Streaming Responses](/project/07-streaming-responses/01-why-streaming) covers the streaming patterns that Codex implements through its async generator design
- [Shell Execution](/project/06-shell-execution/08-sandboxing-basics) explains sandboxing fundamentals for shell command execution
- [Multi-Provider Support](/project/13-multi-provider-support/04-openai-adapter) discusses OpenAI API integration patterns relevant to Codex's implementation
- [Plugin Architecture](/project/14-extensibility-and-plugins/11-plugin-isolation) covers plugin isolation techniques related to Codex's sandboxed execution model
