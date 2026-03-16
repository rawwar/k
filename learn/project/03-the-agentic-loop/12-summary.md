---
title: Summary
description: Review the agentic loop architecture and prepare for building the tool system that plugs into it.
---

# Summary

> **What you'll learn:**
> - How the agentic loop, conversation state, and stop conditions work together as a complete system
> - How your implementation compares to the loop architecture used in production coding agents
> - What gaps remain (tool execution is stubbed) and how Chapter 4 fills them with a real tool system

You have built the heart of a coding agent. The agentic loop you implemented in this chapter transforms a simple chatbot into something that can plan, act, observe, and adapt. Let's step back and review the full system you have constructed.

## What You Built

Over the course of this chapter, you implemented each piece of the agentic loop and wired them together:

**Message types** (`ContentBlock`, `Message`, `ApiResponse`) -- The typed foundation for all communication between your agent and the LLM. Content blocks distinguish between text, tool-use requests, and tool results. The serde annotations handle serialization to and from the Anthropic API format automatically.

**Conversation state** (`ConversationState`) -- The growing `Vec<Message>` that forms the model's context window. You added methods for appending messages, tracking token usage, estimating token counts, and basic truncation to stay within context limits.

**Turn management** (`TurnConfig`, `AgentTurnState`) -- Inner turn counts that prevent runaway loops, outer turn counts for session limits, and environment-variable-based configuration so users can adjust limits without recompiling.

**Stop conditions** (`LoopResult`, `LoopAction`) -- An enum that models every way the loop can exit: normal completion, max tokens, turn limit, context overflow. The decision function maps the API's `stop_reason` to a concrete action with a fallback for edge cases like missing stop reasons.

**The core loop** (`Agent::run`) -- The `async fn` that ties everything together. A `loop` with a `match` on `LoopAction` that calls the LLM, inspects the response, dispatches tool calls, feeds results back, and repeats. The structure is tool-agnostic -- it does not know or care what the tools do.

**Tool call handling** -- Extraction of `ToolUse` blocks from the response, sequential execution, and result collection. Currently stubbed with placeholder responses, but the contract (input `Vec<ContentBlock>`, output `Vec<ContentBlock>`) is finalized.

**Observation feeding** -- Tool results packaged as `ToolResult` content blocks in a user message, with proper ID linking, error flags, and formatting. The model sees these results on its next turn and uses them to decide what to do next.

**Debugging** -- Structured logging with the `tracing` crate, conversation history dumps, and REPL commands for inspecting state at runtime. Detection of common failure modes like repetitive tool calls and missing results.

## The Complete Data Flow

Here is the full picture of how data flows through one iteration of the loop:

```text
1. User types a message
       |
       v
2. Message appended to ConversationState.messages
       |
       v
3. ConversationState serialized into ApiRequest
   (system prompt + all messages + model config)
       |
       v
4. HTTP POST to Anthropic API
       |
       v
5. ApiResponse deserialized
   (content blocks + stop_reason + usage)
       |
       v
6. Assistant message appended to ConversationState
       |
       v
7. stop_reason matched to LoopAction:
   - ReturnToUser → extract text, return LoopResult::Complete
   - ExecuteTools → extract ToolUse blocks, execute each one
   - MaxTokensReached → return LoopResult::MaxTokens
   - UnexpectedReason → return AgentError
       |
       v  (if ExecuteTools)
8. Tool results collected as Vec<ContentBlock>
       |
       v
9. Tool results wrapped in a user Message
   and appended to ConversationState
       |
       v
10. Loop back to step 3
```

Every step is logged. Every error path returns a typed result. Every message is preserved in the conversation history for the next iteration.

## How This Compares to Production Agents

The loop you built follows the same fundamental pattern used by production coding agents. Here are the specific parallels:

::: wild In the Wild
**Claude Code** runs a loop that is structurally identical to yours: call the API, check for tool use, execute tools, feed results back. It adds streaming (you will add this in Chapter 7), a permission system (Chapter 12), and sophisticated context compaction (Chapter 9), but the core loop logic is the same.

**OpenCode** (Go) implements the loop as a `for` loop in its `agent.Run()` method. The message history is a slice of message structs, tool dispatch uses a registry pattern, and stop conditions include turn limits and context size checks -- the same building blocks you have built in Rust.

**Codex** (Python) uses a similar `while True` loop with the OpenAI API. Its stop conditions include turn limits, token budgets, and a "stuck detection" system that catches repetitive tool calls -- a pattern you implemented in the debugging subchapter.

The commonality is not coincidence. The prompt-call-execute-observe cycle is the minimal viable architecture for an LLM agent. Every production implementation converges on this pattern because it is the simplest structure that enables autonomous, tool-using behavior.
:::

## What Is Missing

Your loop works, but it has intentional gaps that the next chapters fill:

| Gap | Impact | Filled in |
|---|---|---|
| **Tool execution is stubbed** | Every tool call returns an error placeholder | Chapter 4: Building a Tool System |
| **No real tools** | No file read/write, no shell commands | Chapters 5-6: File Operations, Shell Execution |
| **No streaming** | The full response arrives at once; the user waits | Chapter 7: Streaming Responses |
| **Basic UI** | Just `println!` for output | Chapter 8: Terminal UI |
| **Naive truncation** | Might break tool-use/tool-result pairs | Chapter 9: Context Management |
| **No permissions** | Tools execute without user approval | Chapter 12: Permission and Safety |

The critical gap is tool execution. Right now, if the model asks to read a file, your agent responds with "[Stub] Tool execution not yet implemented." The model sees this error and either gives up or tries a different approach. Chapter 4 replaces the stub with a real tool registry where you define tools as Rust traits, register them, and dispatch calls to actual implementations.

## The Architecture Is Stable

Here is the important thing: **the loop itself does not change**. When you add tools in Chapter 4, you replace the `handle_tool_calls` method. When you add streaming in Chapter 7, you modify `call_api`. When you add permissions in Chapter 12, you add a check before tool execution. The `match` on `LoopAction`, the message history management, and the stop condition logic remain the same.

This is the payoff of the architectural decisions you made in this chapter:

- **Separation of concerns** means each new capability plugs in at a defined boundary
- **Exhaustive matching** means the compiler tells you when a new variant needs handling
- **Typed message history** means adding new content block types is safe -- serde handles serialization, and the compiler catches missing match arms
- **Tool-agnostic loop** means the loop works the same whether you have 1 tool or 50

## Exercises

1. **(Easy)** Add a turn counter display to the REPL that shows the current turn number during multi-turn interactions: "Turn 3/25: Reading src/main.rs..."

2. **(Medium)** Implement a `/replay` REPL command that re-displays the conversation history in a user-friendly format, showing which messages were from the user, which from the assistant, and which were tool results.

3. **(Hard)** Implement a "continuation" feature: when the model returns `stop_reason: "max_tokens"`, automatically send a follow-up message asking it to continue from where it left off, and concatenate the responses. Be careful to handle the case where continuation also hits max_tokens.

::: python Coming from Python
If you want to verify your understanding, try implementing the same agentic loop in Python and compare:
```python
class Agent:
    def __init__(self, client, system_prompt, max_turns=25):
        self.client = client
        self.system_prompt = system_prompt
        self.max_turns = max_turns

    def run(self, messages, user_input):
        messages.append({"role": "user", "content": user_input})
        for turn in range(self.max_turns):
            response = self.client.messages.create(
                model="claude-sonnet-4-20250514",
                system=self.system_prompt,
                messages=messages,
                max_tokens=4096,
            )
            messages.append({
                "role": "assistant",
                "content": response.content,
            })
            if response.stop_reason == "end_turn":
                return response.content[0].text
            elif response.stop_reason == "tool_use":
                results = self.execute_tools(response.content)
                messages.append({
                    "role": "user",
                    "content": results,
                })
        return "Turn limit reached"
```
Notice how the Rust version forces you to handle every stop reason, every error path, and every content block type. The Python version works but silently ignores edge cases like `"max_tokens"` and `None` stop reasons. That difference in rigor is what makes the Rust version more robust for production use.
:::

## Looking Ahead

In Chapter 4, you will give your agent hands. You will design a `Tool` trait that defines the contract every tool must satisfy, build a registry that maps tool names to implementations, and implement dispatch logic that routes the model's tool calls to the correct handler. The agentic loop will go from a working skeleton to a fully functioning agent that can read files, write files, and eventually execute shell commands.

The loop is ready. It is time to fill in the tools.

## Key Takeaways

- The agentic loop is a complete system: message types, conversation state, turn management, stop conditions, tool dispatch, observation feeding, and debugging all work together
- The core pattern -- prompt, call, execute, observe, repeat -- is the same in every production coding agent, regardless of language or framework
- Your loop is tool-agnostic by design: the `handle_tool_calls` stub can be replaced with a real tool system without changing the loop's control flow
- Rust's type system (enums, pattern matching, `Result`) provides compile-time guarantees that every stop condition, every content block type, and every error path is handled
- The architecture is stable: tools, streaming, UI, context management, and permissions all plug into defined extension points without restructuring the loop
