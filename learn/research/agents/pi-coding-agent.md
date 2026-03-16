# Pi Coding Agent

## Overview

Pi is a coding agent written in Rust, and it occupies a distinctive position in the agent landscape. While Claude Code demonstrates polished commercial agent behavior and OpenCode provides a transparent Go implementation, Pi shows what it looks like to build an agent that leverages Rust's type system, ownership model, and trait system as core architectural tools. For anyone building a coding agent in Rust, Pi is the most directly relevant reference implementation.

Pi is an open-source terminal agent that connects to multiple LLM providers and provides the standard agentic capabilities: file reading, file writing, shell execution, and code search. Its design philosophy centers on making illegal states unrepresentable at compile time. Rather than relying on runtime checks and assertions to catch invalid state transitions, Pi encodes valid transitions into the type system so that entire categories of bugs become compile-time errors. This approach trades upfront verbosity for long-term reliability, a trade-off that pays dividends in autonomous systems that run without constant human supervision.

Pi's lightweight approach to dependencies and its focus on speed reflect Rust's broader ecosystem values. The agent compiles to a single binary with minimal runtime overhead, starts quickly, and uses memory efficiently. For developers accustomed to agent frameworks that pull in hundreds of dependencies and require managed runtimes, Pi demonstrates that a capable coding agent can be built with a small, controlled dependency footprint.

## Architecture

Pi's architecture is organized into six well-defined layers, each communicating through typed interfaces that the compiler can verify.

The configuration layer reads and validates settings at startup using serde for deserialization. Configuration files in TOML format specify the preferred provider, model, API credentials, tool settings, and UI preferences. Validation happens once at startup: if a required field is missing or a value is invalid, Pi reports the error immediately rather than failing mid-session. After validation, typed configuration structs are passed through the system, guaranteed to be well-formed.

The provider layer implements the LLM client interface for each supported provider. Like OpenCode, Pi defines a common trait that all providers must implement, enabling model switching without changes to the core loop. The provider trait covers chat completion, streaming, and model enumeration.

The tool layer contains individual tool implementations behind a common `Tool` trait. Each tool (file reading, file writing, shell execution, search) implements methods for returning its name, description, parameter schema, and an async execute function. The tool dispatcher holds a collection of trait objects and routes calls by matching on the tool name.

The agent core implements the agentic loop: send messages to the provider, parse the response for tool calls, dispatch tools, feed results back as observations, and repeat until the model signals completion. State transitions within the loop are encoded using the typestate pattern, where different stages of the loop are represented by distinct types with methods available only for valid transitions.

The UI layer handles terminal rendering for conversation display and user interaction. The error infrastructure defines custom error types for different failure categories, with conversions and formatting that allow errors to propagate cleanly through the entire call chain.

## Key Patterns

**Typestate pattern for state machine safety.** Pi uses Rust's type system to model the agent's state transitions. Different states (waiting for input, processing a response, executing a tool) are represented by distinct types. Methods are implemented only on the types where those operations are valid. You cannot call "execute tool" from the "waiting for input" state because the type system does not expose that method on that type. Invalid transitions become compile-time errors rather than runtime bugs. This pattern requires more types and more boilerplate than a simple enum-and-match approach, but for a system where an invalid state transition could mean executing a tool call the model never requested, the compile-time guarantee is worth the extra code.

**Trait-based tool dispatch.** The `Tool` trait defines a common interface: `name()`, `description()`, `parameters_schema()`, and an async `execute()` method. Every tool implements this trait, and the dispatcher works with trait objects (`Box<dyn Tool>`). This design strikes a balance between OpenCode's fully dynamic registry and a static dispatch approach. The tool set is determined at startup, but adding a new tool is as simple as implementing the trait and registering an instance. The trait guarantees that every tool provides the required methods, so the dispatcher never encounters a missing implementation at runtime.

**Explicit error propagation.** Rust's `Result<T, E>` type runs deep through Pi's architecture. Every fallible operation returns a Result, and errors propagate explicitly through the call chain using the `?` operator. Pi defines custom error types for API errors, tool execution errors, filesystem errors, and parse errors. These types implement Rust's `Error` trait, enabling composition and conversion. When you read Pi's tool dispatch code, you can trace exactly what happens when a tool fails: the error propagates up, gets formatted as a tool result observation, and goes back to the model. There are no hidden exception handlers and no silent error swallowing.

**Async-first design with Tokio.** Pi uses the Tokio runtime for all I/O operations. LLM API calls, file reads, shell execution, and UI event handling are all non-blocking. This enables streaming response rendering (the stream processor and the terminal renderer cooperate without blocking each other), tool execution timeouts (long-running commands can be cancelled without blocking the program), and concurrent tool execution (multiple independent tool calls can be dispatched in parallel using Tokio task spawning).

## Implementation Details

The trait-based tool system uses `#[async_trait]` to enable async methods in trait definitions, which is necessary because Rust's native async trait support has historically required this crate for trait objects. Each tool's `execute` method takes a `serde_json::Value` for parameters and returns `Result<ToolResult, ToolError>`. The dispatcher deserializes the model's JSON arguments, looks up the tool by name in its collection of trait objects, and calls execute. The `Send + Sync` bounds on the `Tool` trait ensure that tools can be safely shared across threads, which is required for concurrent dispatch.

Configuration validation at startup follows Rust's "make illegal states unrepresentable" philosophy applied at the application level. Serde's derive macros handle deserialization, and custom validation logic checks for semantic correctness (valid API keys, supported model names, consistent provider-model combinations). After validation, the configuration is frozen into immutable structs that flow through the rest of the system. There is no global mutable state for configuration; each component receives the configuration it needs through its constructor.

Error types are defined using `thiserror` for derive-based error definitions and `anyhow` for ad-hoc error contexts in application code. The distinction matters: library-like code within Pi (tool implementations, provider clients) uses specific error types for precise handling, while application-level code (the main loop, CLI parsing) uses `anyhow::Result` for convenience. This two-tier approach avoids the verbosity of custom error types everywhere while maintaining precision where it matters.

The async runtime handles streaming responses by processing SSE events as they arrive. Partial text is rendered to the terminal immediately. Tool call blocks are assembled from streaming chunks (since a single tool call may arrive across multiple SSE events) and dispatched once complete. The UI event loop runs concurrently with the stream processor, handling keyboard input and terminal resize events without blocking response rendering.

## Cross-References

- [Why Rust](/project/01-hello-rust-cli/01-why-rust) discusses the language-level advantages that Pi leverages throughout its architecture
- [The Agentic Loop](/project/03-the-agentic-loop/02-loop-architecture) covers the loop pattern that Pi implements with typestate-enforced transitions
- [Tool Registration API](/project/14-extensibility-and-plugins/02-tool-registration-api) explains the trait-based registration pattern Pi uses for tool dispatch
- [Streaming Responses](/project/07-streaming-responses/09-streaming-state-machine) covers the streaming state machine similar to Pi's async stream processing
- [Error Recovery](/project/15-production-polish/01-error-recovery) discusses error handling strategies that mirror Pi's explicit propagation approach
- [Multi-Provider Support](/project/13-multi-provider-support/01-why-multiple-providers) covers the provider abstraction that Pi shares with OpenCode
- [Elm Architecture](/project/08-terminal-ui-with-ratatui/04-elm-architecture) explains the UI architecture pattern used in Pi's terminal rendering layer
