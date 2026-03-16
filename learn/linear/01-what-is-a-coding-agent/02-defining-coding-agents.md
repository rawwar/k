---
title: Defining Coding Agents
description: A precise definition of coding agents, distinguishing them from code assistants, copilots, and general-purpose chatbots.
---

# Defining Coding Agents

> **What you'll learn:**
> - The formal characteristics that define a coding agent versus other AI coding tools
> - The three essential capabilities every coding agent must possess: perception, reasoning, and action
> - How autonomy, tool use, and iterative execution separate agents from simpler systems

## What Exactly Is a Coding Agent?

The term "coding agent" gets thrown around loosely. Marketing departments apply it to everything from a glorified autocomplete plugin to a fully autonomous system that can implement features end-to-end. Before we spend an entire tutorial track building one, let's pin down exactly what we mean.

A **coding agent** is a software system that uses a large language model to autonomously perform multi-step programming tasks by perceiving the state of a codebase, reasoning about what actions to take, and executing those actions through tools — iterating until the task is complete or the system determines it cannot proceed.

That's a dense definition, so let's unpack it piece by piece.

## The Three Pillars: Perceive, Reason, Act

Every coding agent, regardless of its implementation language or target use case, must have three core capabilities. Think of these as the pillars that hold up the entire system.

### Perception: Reading the World

A coding agent must be able to observe its environment. In practice, this means reading files from the filesystem, examining directory structures, searching through code for patterns, inspecting git history, reading terminal output, and understanding error messages.

If you think of a Python developer working on a bug fix, the perception phase is everything they do before they start typing. They read the stack trace, open the relevant source file, check what the function is supposed to do, look at the test that's failing, maybe search the codebase for other callers of the function. A coding agent does all of this, but through tools — a `read_file` tool, a `search` tool, a `list_directory` tool.

The quality of perception directly determines the quality of everything that follows. An agent that reads only the file it intends to modify, without understanding the broader context, will produce code that compiles in isolation but breaks the system. This is why every production coding agent invests heavily in perception tools.

### Reasoning: Deciding What to Do

The language model at the center of the agent is the reasoning engine. It takes in everything the agent has perceived — file contents, error messages, conversation history, the user's original request — and decides what to do next. Should it read another file to gather more context? Should it write a fix? Should it run the test suite to verify its changes? Should it ask the user a clarifying question?

This is where the magic of large language models comes in. The model isn't executing a fixed algorithm or following a decision tree. It's applying the patterns it learned during training across billions of lines of code to reason about the specific situation in front of it. It can recognize that a `NullPointerException` in Java often means a missing null check, that a failing import in Python usually means a missing dependency, that a type error in Rust often means you need to convert between `&str` and `String`.

::: tip In the Wild
Claude Code's reasoning is particularly visible in its "extended thinking" mode, where the model explicitly works through its chain of thought before acting. You can watch it reason: "The user wants pagination. Let me check the existing route handlers to understand the pattern. I see they use an `offset` and `limit` parameter in the database queries. I'll follow the same pattern." This externalized reasoning makes the agent's decision-making transparent and debuggable.
:::

### Action: Changing the World

Perception without action is just reading. Reasoning without action is just thinking. A coding agent must be able to *do things* — write files, create directories, execute shell commands, run tests, install dependencies, make git commits.

Each action is implemented as a **tool**. A tool is a well-defined operation that the language model can invoke by name, passing structured parameters. When the model decides it needs to modify a file, it doesn't generate the raw text of a file write system call. Instead, it produces a structured tool call like: "call the `write_file` tool with path `src/handlers/users.rs` and the following content." The agent's runtime executes this tool call and returns the result to the model.

This separation between deciding what to do (the model's job) and executing the operation (the runtime's job) is fundamental. It means the runtime can add safety checks, request user permission, log operations, enforce sandboxing — all without the model needing to know about these concerns.

## The Loop That Makes It Agentic

The three pillars alone don't make an agent. What makes a system truly agentic is that perceive-reason-act happens in a **loop**. The agent doesn't just read, think, and write once. It reads, thinks, writes, then *observes the result of its writing*, thinks again, and possibly acts again.

Consider a concrete scenario. You ask an agent to fix a failing test. Here's what the loop might look like:

1. **Perceive:** Read the test failure output. Read the test file. Read the source file being tested.
2. **Reason:** The test expects a function to return a list, but the function returns a single item. The fix is to wrap the return value in a list.
3. **Act:** Modify the source file.
4. **Perceive:** Run the test suite. Read the new output.
5. **Reason:** The original test passes now, but a different test broke because it was relying on the single-item return value.
6. **Act:** Modify the second test to account for the list return type.
7. **Perceive:** Run the test suite again. All tests pass.
8. **Reason:** Task is complete. Report the result to the user.

This multi-turn loop is the heartbeat of a coding agent. Without it, you just have a chatbot that generates code in one shot and hopes for the best. The loop is what allows the agent to recover from mistakes, handle unexpected complications, and converge on a working solution.

::: python Coming from Python
If you've used Python's `subprocess` module to run commands and check their output in a script, you've written something conceptually similar to an agentic loop — a program that takes an action, observes the result, and decides what to do next. The difference is that in an agent, the "deciding what to do next" part is handled by a language model rather than hardcoded `if/else` logic. Think of it as replacing your control flow with an LLM.
:::

## The Spectrum of Autonomy

Not all coding agents are equally autonomous. It's useful to think of autonomy as a spectrum rather than a binary property.

At the **low-autonomy end**, you have tools like GitHub Copilot. It perceives (reads surrounding code), reasons (predicts the next tokens), and acts (inserts a suggestion), but only within a single completion. It doesn't iterate, doesn't run tests, and doesn't recover from errors. You might call it a "micro-agent" — agentic for one step, but not across multiple steps.

In the **middle of the spectrum**, you have tools like Cursor or Windsurf in their chat mode. They can read files you point them to, generate code changes, and show you diffs. But they typically require you to approve each change, run the tests yourself, and paste back any errors. They're agentic in their reasoning but rely on you for the loop.

At the **high-autonomy end**, you have Claude Code, Codex, and similar agents that can execute the full perceive-reason-act loop many times without human intervention. You give them a task, and they iterate until they believe they're done. You supervise the process, and you can intervene at any point, but the default mode is autonomous execution.

The agent we'll build in this tutorial sits firmly at the high-autonomy end. It will be able to read files, write files, execute commands, observe results, and iterate — all orchestrated by the language model, with you supervising rather than steering.

## Formal Characteristics

Let's distill the definition into a checklist. A system qualifies as a coding agent if it has all of the following:

1. **An LLM as the reasoning core.** The system uses a language model (not rule-based logic) to decide what to do at each step.

2. **Tool access.** The system can perform operations on the external environment — reading and writing files, executing commands, searching code — through structured tool interfaces.

3. **An iterative execution loop.** The system operates in a loop where each cycle involves perception, reasoning, and action. The result of one cycle informs the next.

4. **Autonomous multi-step execution.** The system can chain multiple perceive-reason-act cycles together without requiring human input between each step.

5. **Task-oriented behavior.** The system works toward completing a user-defined goal rather than simply responding to a single prompt.

If a system lacks any one of these, it's something other than a coding agent. It might be an excellent tool — Copilot is incredibly useful — but it occupies a different category.

::: tip In the Wild
OpenCode demonstrates these five characteristics clearly. Its LLM core can be swapped between providers (Anthropic, OpenAI, or local models). Its tool system gives the model access to file reading, file writing, shell execution, and LSP-powered code navigation. Its main loop runs tool calls until the model produces a response with no tool invocations, signaling task completion. And it operates autonomously — you ask it to do something, and it works through the steps until it's done.
:::

## Why Precision Matters

You might wonder why we need such a precise definition. After all, won't we just build the thing and see what it does?

The precision matters because it guides our architecture. When we design the system in later chapters, every architectural decision maps back to these characteristics. The agentic loop is the main execution path. The tool system is the action layer. The LLM integration is the reasoning layer. The file and command tools are the perception layer. By defining these clearly now, we have a blueprint that tells us what to build and why.

It also helps you evaluate other agents. When a new tool launches and claims to be an "AI coding agent," you can test it against these five characteristics. Does it have tool access? Does it iterate? Can it chain steps together autonomously? If yes, it's an agent. If not, it's something else — possibly valuable, but architecturally different.

## Key Takeaways

- A coding agent is defined by five characteristics: LLM-based reasoning, tool access, an iterative execution loop, autonomous multi-step execution, and task-oriented behavior.
- The perceive-reason-act cycle is the fundamental unit of agent behavior, and it's the loop — not any single step — that makes a system truly agentic.
- Autonomy exists on a spectrum from single-step suggestion (Copilot) to fully autonomous multi-step execution (Claude Code, Codex), and our agent targets the high-autonomy end.
- The separation between reasoning (what the model does) and execution (what the runtime does) is a core architectural principle that enables safety, logging, and user control.
- This precise definition serves as an architectural blueprint: every component we build maps directly to one of the five characteristics.
