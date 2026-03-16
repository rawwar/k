---
title: What Is an Agentic Loop
description: Define the agentic loop pattern and explain how it differs from simple request-response chatbot interactions.
---

# What Is an Agentic Loop

> **What you'll learn:**
> - The fundamental difference between a chatbot (single turn) and an agent (multi-turn with tool use)
> - How the prompt-call-execute-observe cycle enables autonomous task completion
> - Why the loop pattern is the backbone of every modern coding agent from Claude Code to Cursor

In Chapter 2 you built something that can talk to an LLM: your CLI sends a message, the API returns a response, and you print it. That is a chatbot. It works, but it is fundamentally limited. If you ask it to "create a file called hello.rs with a main function," it will write out the file contents in its reply -- but it cannot actually create the file. It can describe what to do, but it cannot *do* anything.

An agentic loop changes this. Instead of a single request-response exchange, the agent runs a *loop*. It sends the user's prompt to the model. The model responds, and that response might contain a tool-use request -- "I want to call the `write_file` tool with these arguments." The agent executes that tool, collects the result (the *observation*), feeds it back into the conversation, and calls the model again. The model sees the observation, decides whether to call another tool or respond to the user, and the cycle repeats.

This is the **prompt-call-execute-observe** cycle, and it is the single most important pattern in this entire book.

## From Chatbot to Agent

Let's make this concrete. Imagine you type: "Read the file src/main.rs and add error handling to the unwrap calls."

A **chatbot** would respond with something like: "Here's how you could modify the file..." and show you code in its reply. You would then copy that code, paste it into your editor, and hope it works.

An **agent** running an agentic loop would:

1. Call the `read_file` tool to get the current contents of `src/main.rs`
2. Receive the file contents as an observation
3. Analyze the code and decide what changes to make
4. Call the `write_file` tool to write the updated version
5. Receive confirmation that the file was written
6. Optionally call a `run_command` tool to run `cargo check` and verify the changes compile
7. Receive the compilation output
8. Report back to the user: "I've updated src/main.rs to replace the unwrap calls with proper error handling. The project compiles successfully."

That sequence involved *four* LLM calls and *three* tool executions, all happening automatically within a single user request. The user typed one message and got back a completed task. That is the power of the agentic loop.

## The Cycle in Pseudocode

Before we touch any Rust, let's see the loop in plain pseudocode:

```text
messages = [system_prompt, user_message]

loop {
    response = call_llm(messages)
    append response to messages

    if response.stop_reason == "end_turn" {
        // Model is done, show result to user
        break
    }

    if response.stop_reason == "tool_use" {
        for each tool_call in response {
            result = execute_tool(tool_call.name, tool_call.input)
            append tool_result to messages
        }
        // Continue the loop -- model needs to see the results
    }
}
```

That is the entire pattern. Everything else in this chapter -- message types, conversation state, turn management, stop conditions -- is about implementing this loop correctly, efficiently, and robustly in Rust.

## Why a Loop and Not a Pipeline?

You might wonder: why not design this as a pipeline or a chain of fixed steps? Some frameworks do take that approach. LangChain, for example, lets you define "chains" where each step feeds into the next. But a chain requires you to know the steps in advance. The whole point of an agentic loop is that the *model* decides what to do next. It might need one tool call or ten. It might realize halfway through that it needs to read a different file than the one it started with. The loop gives the model the freedom to adapt.

This is also why the loop is *tool-agnostic*. The loop itself does not know about file operations or shell commands or code search. It only knows three things: how to call the LLM, how to detect tool-use requests in the response, and how to feed observations back. The specific tools are plugged in separately (that is Chapter 4). This separation is what makes the architecture extensible.

::: python Coming from Python
If you have used Python agent frameworks, the agentic loop might feel familiar. In LangChain you would write an `AgentExecutor` that calls `.invoke()` in a loop. In a raw Python implementation, you would write:
```python
while True:
    response = client.messages.create(model="claude-sonnet-4-20250514", messages=messages)
    messages.append({"role": "assistant", "content": response.content})
    if response.stop_reason == "end_turn":
        break
    # extract tool calls, execute them, append results
```
The Rust version follows the same structure, but with stronger typing. Instead of dictionaries and string matching, you will use enums and pattern matching, which means the compiler catches errors like forgetting to handle a message type.
:::

## The Loop in Every Production Agent

This is not an academic exercise. Every production coding agent uses some variation of this loop:

::: wild In the Wild
Claude Code, Anthropic's own CLI coding agent, runs an agentic loop at its core. When you give it a task, it enters a loop that calls the Claude API, checks for tool use in the response, executes tools (file reads, writes, shell commands), feeds the results back, and continues until the model signals it is done. OpenCode, an open-source Go-based agent, uses the same fundamental pattern with a `for` loop that alternates between LLM calls and tool dispatch. Codex from OpenAI follows the identical cycle. The specifics differ -- how they handle streaming, what safety checks they add, how they manage context -- but the core loop is the same everywhere.
:::

## What You Are Building

Over the next eleven subchapters, you will implement the complete agentic loop for your coding agent. Here is the roadmap:

- **Architecture**: How the loop is structured as a state machine with clear phases
- **Message types**: The Rust enums and structs that represent every kind of message in the conversation
- **Conversation state**: The `Vec<Message>` that grows with each iteration and forms the model's context
- **Turn management**: Counting iterations and enforcing limits so the loop cannot run forever
- **Stop conditions**: Detecting when the model is done, when limits are hit, or when something goes wrong
- **The core loop**: The actual `async fn agent_loop` implementation in Rust
- **Tool call handling**: Extracting tool-use requests from the model's response and routing them
- **Observation feeding**: Packaging tool results and injecting them back into the conversation
- **Single vs multi-turn**: Understanding when the loop iterates once versus many times
- **Debugging**: Logging, tracing, and diagnosing when the loop goes wrong

By the end of this chapter, your agent will have a complete, working agentic loop. The tool execution will be stubbed -- returning placeholder results -- but the loop structure will be fully operational. Chapter 4 will fill in the stubs with real tool implementations.

Let's start by looking at the architecture of the loop.

## Key Takeaways

- An agentic loop transforms a single-turn chatbot into a multi-turn autonomous agent that can take actions and observe their results
- The core cycle is **prompt-call-execute-observe**: send messages to the LLM, detect tool-use requests, execute tools, feed results back, repeat
- The loop is tool-agnostic by design -- it only knows how to call the LLM and detect whether the response requires action, making it extensible to any set of tools
- Every production coding agent (Claude Code, OpenCode, Codex) uses this same fundamental pattern
- The model, not the programmer, decides what to do next -- the loop gives the model freedom to adapt its approach as it discovers new information
