---
title: Common Architecture Patterns
description: Extracting the shared architectural patterns found across all major coding agents, from the agentic loop to tool dispatch.
---

# Common Architecture Patterns

> **What you'll learn:**
> - The five core architectural components shared by every production coding agent
> - How the observe-think-act loop manifests in different agent implementations
> - The recurring patterns in tool design, state management, and context handling across agents

## The Universal Blueprint

Over the last four subchapters, we examined four very different coding agents — Claude Code (TypeScript, terminal-native, permission-based), OpenCode (Go, terminal UI, multi-provider), Pi (Rust, type-safe, trait-based), and Codex (sandboxed, network-isolated, async). They differ in language, deployment model, UI philosophy, and safety approach.

And yet, if you squint at their architectures, they're remarkably similar.

This isn't a coincidence. The problem they're solving — connecting a language model to a development environment through an iterative loop — imposes architectural constraints that any reasonable solution must satisfy. These constraints produce a common blueprint that we can extract, study, and use as the foundation for our own agent.

Let's identify the five components that appear in every agent we've studied.

## Component 1: The Agentic Loop

Every coding agent has a central loop that drives execution. The specifics vary, but the structure is always the same:

1. Construct a message for the LLM (including conversation history and tool results).
2. Send the message to the LLM and receive a response.
3. If the response contains tool calls, execute them and go to step 1 with the results.
4. If the response contains only text, display it to the user and wait for the next input.

This is the agentic loop — the heartbeat of the system. Claude Code implements it as a recursive function that calls the API, processes tool calls, and calls itself again. OpenCode implements it as a `for` loop that checks the response type at each iteration. Pi models it as a state machine with typed transitions. Codex runs it inside a container until no more tool calls are produced.

The loop has a few critical properties:

**It's model-driven.** The model decides whether to invoke tools or produce a final response. The runtime doesn't have a fixed number of iterations or a predetermined plan — it follows the model's lead.

**It's accumulative.** Each iteration adds to the conversation history. The model sees everything that's happened so far — all previous messages, tool calls, and tool results — when deciding what to do next. This growing context is how the model maintains coherence across a multi-step task.

**It terminates naturally.** The loop ends when the model produces a response without tool calls. This could mean the task is complete, the model is stuck and needs human input, or the model wants to report its progress. The runtime doesn't need explicit termination logic — the model's decision to stop calling tools is the termination signal.

```rust
// Simplified agentic loop structure
loop {
    let response = llm.send(&messages).await?;

    // Render any text content to the user
    for block in &response.content {
        if let ContentBlock::Text(text) = block {
            ui.render_text(text);
        }
    }

    // Collect tool calls from the response
    let tool_calls: Vec<_> = response.content.iter()
        .filter_map(|block| match block {
            ContentBlock::ToolUse(tc) => Some(tc),
            _ => None,
        })
        .collect();

    // If no tool calls, the turn is complete
    if tool_calls.is_empty() {
        break;
    }

    // Execute tools and add results to conversation
    for tool_call in tool_calls {
        let result = tool_dispatcher.execute(tool_call).await;
        messages.push(Message::tool_result(tool_call.id, result));
    }
}
```

::: wild In the Wild
Claude Code and OpenCode both include circuit breakers in their loops — if the agent has executed an unusually large number of tool calls without completing the task, the loop pauses and asks the user whether to continue. This prevents runaway loops where the model keeps trying variations of a broken approach. Pi handles this more subtly, tracking the number of consecutive failed tool calls and prompting the user if a threshold is exceeded.
:::

## Component 2: The LLM Integration Layer

Every agent needs to communicate with a language model. This layer handles constructing API requests, sending them, receiving responses (often streamed), and parsing the response into a structured format the rest of the system can work with.

The key design decision here is **abstraction level**. Some agents hardcode a single provider (Claude Code uses the Anthropic API). Others abstract across providers (OpenCode supports Anthropic, OpenAI, Google, and local models). The level of abstraction affects flexibility, complexity, and how tightly the agent can leverage provider-specific features.

Across all agents, this layer handles:

- **Message formatting:** Converting the conversation history into the format the provider's API expects.
- **Tool schema injection:** Including tool definitions in the API request so the model knows what tools are available.
- **Response parsing:** Extracting text blocks and tool use blocks from the response.
- **Streaming:** Processing server-sent events for real-time output.
- **Error handling:** Retrying on rate limits, handling network failures, and managing API-specific errors.

::: python Coming from Python
If you've used the `anthropic` or `openai` Python SDK, you've already interacted with this layer from the client side. In Python, you might write `client.messages.create(model="claude-sonnet-4-20250514", messages=messages, tools=tools)`. The agent's LLM integration layer wraps this call with conversation management, streaming handling, and tool schema generation. It's the same API call, but embedded in a larger system.
:::

## Component 3: The Tool System

Every agent provides the model with a set of tools — well-defined operations that the model can invoke to interact with the outside world. The tool system has three parts: tool **definitions** (what the model sees), tool **dispatch** (routing a tool call to the right handler), and tool **execution** (performing the actual operation).

**Tool definitions** are structured descriptions that tell the model what each tool does and what parameters it accepts. These are included in the API request, typically as JSON Schema objects. A file-reading tool might be defined as having a `path` parameter (string, required) and a `line_range` parameter (object with `start` and `end` integers, optional).

**Tool dispatch** routes a tool call from the model to the appropriate handler. The strategies differ:

| Agent | Dispatch Strategy |
|-------|------------------|
| Claude Code | Static match on tool name |
| OpenCode | Runtime registry lookup by name |
| Pi | Trait object dispatch with name matching |
| Codex | Internal dispatch within sandbox runtime |

**Tool execution** performs the actual operation — reading a file, writing content, running a shell command. Every agent wraps execution in error handling that converts failures into informative messages the model can reason about.

The tools themselves converge across agents. Every production coding agent includes at minimum:

- **File read** — Read the contents of a file by path.
- **File write** — Create or modify a file (full replacement or diff-based).
- **Shell execute** — Run a command and capture stdout/stderr.
- **Search/grep** — Find patterns across files in the codebase.
- **List directory** — Enumerate files in a directory.

Most also include additional tools for specific tasks — git operations, web search, code navigation via LSP, and sometimes specialized tools like image generation or diagram creation.

## Component 4: Context Management

Language models have finite context windows. A coding agent's conversation — including system prompt, user messages, tool calls, and tool results — can easily grow to hundreds of thousands of tokens during a complex task. Every agent needs a strategy for managing this growth.

The strategies fall into a few categories:

**Truncation.** The simplest approach: when the context gets too long, drop the oldest messages. This risks losing important early context (like the user's original request) but is easy to implement.

**Summarization/Compaction.** Claude Code's approach: when the context grows too long, ask the model to summarize the conversation so far, then replace the detailed history with the summary. This preserves the essential information while dramatically reducing token count.

**Selective inclusion.** Rather than including everything in the conversation, only include messages that are relevant to the current step. This requires more sophisticated logic to determine relevance but can be very token-efficient.

**Token counting.** All strategies require knowing how many tokens the current context consumes. This involves either using the model provider's tokenizer or approximating token counts based on character/word counts.

Context management is invisible when it works and catastrophic when it doesn't. If the agent loses track of what it's been doing because important context was evicted, it might repeat work, contradict earlier decisions, or lose track of the original goal. Getting this right is one of the most important engineering challenges in agent development.

## Component 5: Safety and Permissions

Every agent needs some mechanism to prevent dangerous operations. The approaches differ drastically — from Codex's sandbox isolation to Claude Code's tiered permission system — but the underlying need is universal.

Permission systems typically address three questions:

1. **What operations are allowed?** Some agents categorize tools by risk level (read = safe, write = needs approval, shell = dangerous). Others allow everything within a sandbox.

2. **Who decides?** In interactive agents, the user approves or denies specific operations. In async agents, the sandbox provides the safety boundary. Some agents allow pre-configured allow-lists that grant blanket approval for certain operations.

3. **What happens when an operation is denied?** The agent must handle denial gracefully — typically by informing the model that the operation was blocked and letting it find an alternative approach.

::: wild In the Wild
The permission models reveal interesting philosophical differences. Claude Code's model trusts the user to make good decisions about what to allow. Codex's model trusts the sandbox to contain any damage. OpenCode gives users fine-grained control through configuration. Pi encodes permission checks in the type system, making it a compile-time concern rather than a runtime one. Each approach reflects a different assumption about where trust should live in the system.
:::

## The Meta-Pattern: Separation of Concerns

Zooming out from the five components, the meta-pattern is **separation of concerns**. Every well-designed agent separates:

- **Reasoning** (the model) from **execution** (the tools).
- **Interface** (the UI) from **logic** (the loop and dispatch).
- **Provider specifics** (API format, auth, streaming) from **agent logic** (message management, tool orchestration).
- **Policy** (what's allowed) from **mechanism** (how it's executed).

This separation makes agents modular, testable, and adaptable. You can swap the UI without touching the loop. You can add a tool without changing the LLM integration. You can switch providers without rewriting the tool system. Each component has a clear responsibility and a well-defined interface to the others.

When we build our agent, we'll follow this separation rigorously. Each chapter in this tutorial corresponds roughly to one component, and the interfaces between components will be explicit and typed.

## The Blueprint We'll Follow

Here's how the five components map to the agent we'll build:

| Component | Our Implementation | Chapter(s) |
|-----------|-------------------|------------|
| Agentic Loop | Async loop with model-driven termination | Ch 4 |
| LLM Integration | Anthropic API client with streaming | Ch 3 |
| Tool System | Trait-based dispatch with enum tools | Ch 5-8 |
| Context Management | Token counting with compaction | Ch 9 |
| Safety/Permissions | Tiered permission system | Ch 12 |

This is the blueprint. Every architectural decision we make in the coming chapters traces back to the patterns we've observed across Claude Code, OpenCode, Pi, and Codex. You're not learning arbitrary design choices — you're learning the patterns that the industry has converged on through hard-won experience.

## Key Takeaways

- Every production coding agent shares five core components: the agentic loop, the LLM integration layer, the tool system, context management, and a safety/permission model.
- The agentic loop is model-driven, accumulative, and naturally terminating — the model decides when to call tools and when to stop, and the conversation history grows with each iteration.
- Tool systems share a three-part structure (definition, dispatch, execution) but differ in dispatch strategy — from static matching to dynamic registries to trait-based dispatch.
- Context management is the invisible engineering challenge that determines whether an agent can handle complex, multi-step tasks without losing coherence.
- The meta-pattern across all agents is separation of concerns: reasoning from execution, interface from logic, provider specifics from agent logic, and policy from mechanism.
