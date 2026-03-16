# Tool Systems

## Overview

Tools are what give a coding agent its agency. Without tools, an LLM is a text generator -- it can describe how to fix a bug but cannot actually read the file, edit the code, or run the tests. Tool systems encompass the full lifecycle: defining schemas so the model knows what is available, registering tools at startup or dynamically, dispatching calls to the correct implementation, executing safely, formatting results, and handling errors for self-correction. The design of the tool system directly determines what an agent can do, how reliably it does it, and how safely it operates.

## The Pattern

A tool system has five core stages that form a pipeline from LLM request to LLM-consumable result.

**Schema definition.** Each tool is described by a JSON Schema that specifies its name, a natural-language description, and an `input_schema` defining the parameters. The schema uses `type`, `properties`, `required`, `enum`, `minimum`/`maximum`, and `items` to constrain what the model can pass. In Rust, schemas are generated from structs using `schemars` with `#[derive(JsonSchema)]`, where doc comments become `description` fields and `Option<T>` fields become non-required properties. Go uses struct tags (OpenCode), TypeScript uses Zod (Claude Code). The goal is the same: define the schema once in the native type system and generate JSON Schema automatically.

**Tool registration.** At startup, the agent builds a registry of available tools. Static registration means the full set of tools is known at compile time or startup. Dynamic registration allows tools to be added or removed during a session based on context -- for example, database tools might only be registered when the agent detects a database configuration file. The registry maps tool names to their implementations and schemas, and the full list of tool definitions is sent to the LLM with every API call so it knows what actions are available.

**Tool dispatch.** When the LLM response contains tool calls, the agent routes each to the correct implementation. A match statement maps names to handlers at compile time, providing exhaustiveness checking but requiring code changes to add tools. A HashMap lookup maps name strings to trait objects at runtime, enabling dynamic registration but losing compile-time guarantees. Most production agents use the HashMap approach, wrapping each tool behind a common trait interface that standardizes invocation.

**Tool execution.** Execution happens through one of four models. In-process execution runs the tool as a direct function call -- fast and simple, used for file reads and code search. Subprocess execution spawns a child process with isolation, used for shell commands and compilers. Sandboxed execution adds security constraints to defend against prompt injection. Remote execution sends calls to an external service over HTTP. Most CLI agents use in-process execution for the majority of tools, with subprocess for shell commands.

**Result formatting.** Tool results are sent back as structured messages paired with the original call via a shared ID. Successful results contain the output as text. Failed results include an `is_error` flag and a descriptive error message.

## Implementation Approaches

**Schema design trade-offs.** Flat schemas with simple parameter types are easiest for models to fill correctly. Deeply nested objects increase the chance of malformed calls. Enums are valuable because they tell the model exactly which values are valid (for example, `"enum": ["rust", "python", "javascript"]` prevents it from passing `"rs"` or `"Rust"`). Boolean flags should be used sparingly -- each one doubles the behavior space. When a tool accumulates many flags, splitting it into focused tools is better.

**Validation strategies.** Input validation happens at two levels. Schema-level validation checks types, required fields, and bounds before the tool runs. Semantic validation checks domain constraints inside the implementation -- verifying that a file path is within the project directory, that a line number exists in the file, or that an edit target actually appears. Both levels should produce descriptive error messages that help the model self-correct.

**Error propagation.** Tool failures (file not found, permission denied, compilation error) go back to the LLM as error-flagged results so it can adjust. System errors (disk full, network unreachable) are handled by the agent directly since the model cannot fix them. Well-formatted error messages follow a three-part structure: what failed, why, and what to do next. Proactive suggestions (like similar file names when a path is not found) dramatically improve recovery rates.

**Sending definitions to the LLM.** Tool definitions are included in every API request as a `tools` array. The descriptions are critical -- they are the model's only documentation for each tool. Vague descriptions lead to incorrect calls. Production-quality descriptions specify purpose, expected parameter values, edge case behavior, and return format. The token cost of tool definitions is fixed overhead that reduces conversation space, so monitoring this cost matters as the tool set grows.

## Key Considerations

**Schema quality determines tool reliability.** Imprecise descriptions, missing constraints, and ambiguous names increase failure rates. Investing in schema quality pays compound returns across all future invocations.

**Dynamic vs. static registration is a spectrum.** Most agents start static and add dynamic capabilities as needed. Dynamic registration enables contextual tools and plugins but introduces complexity: the LLM must be informed when the tool set changes, and the agent must handle calls to tools that were removed between planning and execution.

**Security is a tool system concern.** Every tool that takes LLM-generated input is a prompt injection vector. Sandboxed execution, filesystem restrictions, command deny-lists, and user confirmation prompts are defenses in the tool system layer. Each tool should have only the access it needs.

**Tool composition and batching.** When the model calls multiple tools in one response, independent operations (multiple reads) can run in parallel, but dependent operations (read then edit) must be sequenced correctly.

## Cross-References
- [JSON Schema Specification](/linear/05-tool-systems-deep-dive/02-json-schema-specification) -- Defining tool parameters with JSON Schema
- [Execution Models](/linear/05-tool-systems-deep-dive/05-execution-models) -- In-process, subprocess, sandboxed, and remote execution
- [Error Propagation](/linear/05-tool-systems-deep-dive/07-error-propagation) -- Routing errors to the model vs. the agent
- [Tool Description Design](/linear/05-tool-systems-deep-dive/03-tool-description-design) -- Writing descriptions that models can follow
- [Designing for LLMs](/linear/05-tool-systems-deep-dive/13-designing-for-llms) -- How tool definitions are sent to and interpreted by models
