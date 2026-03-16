---
title: Agentic vs Chatbot
description: Drawing a clear line between conversational chatbots and agentic systems that can perceive, plan, and act autonomously.
---

# Agentic vs Chatbot

> **What you'll learn:**
> - The fundamental architectural differences between a chatbot and an agentic system
> - Why the ability to use tools and maintain state across iterations defines agentic behavior
> - How to evaluate where a system falls on the chatbot-to-agent autonomy spectrum

## A Distinction That Matters

At this point in the chapter, you've seen four production coding agents analyzed in detail and the common architectural patterns extracted. But there's a question that still trips people up: "Isn't a coding agent just a fancy chatbot?"

The answer is no, and the distinction isn't pedantic — it's architectural. The difference between a chatbot and an agent determines how you design the system, what capabilities it has, and what kind of tasks it can handle. Getting this distinction right shapes every engineering decision from here forward.

## What Makes a Chatbot

A chatbot is a system that takes a text input, processes it, and returns a text output. The canonical chatbot architecture looks like this:

1. User sends a message.
2. System constructs a prompt (system instructions + conversation history + user message).
3. System sends the prompt to an LLM.
4. LLM returns a text response.
5. System displays the response to the user.
6. Go to step 1.

This is a perfectly good architecture for many use cases — answering questions, explaining concepts, generating code snippets, brainstorming solutions. ChatGPT, in its default mode, is a chatbot. So is a basic integration of any LLM into a messaging interface.

The key limitation is in step 4: the LLM returns a **text response**. It can describe what you should do, generate code you could paste into your editor, or explain an error message you showed it. But it can't *do* anything. It can't read your files, run your tests, or modify your code. It can only produce text.

::: python Coming from Python
Think of it in Python terms. A chatbot is like a function that takes a string and returns a string:

```python
def chatbot(user_message: str, history: list[dict]) -> str:
    response = llm.complete(history + [{"role": "user", "content": user_message}])
    return response.text
```

An agent is like a function that takes a string and returns after performing side effects on the real world — reading files, writing files, running commands. The return value (if any) is just a summary of what it did.
:::

## What Makes an Agent

An agent extends the chatbot architecture with three additional capabilities: **tool use**, **observation**, and **autonomous iteration**.

1. User sends a message.
2. System constructs a prompt (system instructions + conversation history + user message + **tool definitions**).
3. System sends the prompt to an LLM.
4. LLM returns a response that may contain text **and/or tool calls**.
5. If the response contains tool calls:
   a. Execute each tool call.
   b. Append the tool results to the conversation history.
   c. **Go to step 3** (not step 1 — the loop continues without user input).
6. If the response contains only text, display it to the user and go to step 1.

The three additions are transformative:

**Tool use** means the model can do things, not just say things. When it decides a file needs to be read, it doesn't tell you "please read `src/main.rs`" — it invokes the `read_file` tool with the path `src/main.rs`, and the tool returns the file contents directly to the model.

**Observation** means the model can see the results of its actions. After it writes code and runs the tests, it sees the test output. After it modifies a file, it can read the file back to verify the change was applied correctly. The world state is visible to the model at every step.

**Autonomous iteration** means the model can take multiple actions in sequence without waiting for human input. Step 5c is the critical difference — the loop goes back to the LLM with the tool results, and the LLM decides whether to take more actions or produce a final response. The human doesn't need to be in the loop for every step.

## The Human-in-the-Loop Spectrum

The chatbot-agent distinction isn't binary — it's a spectrum defined by how much the human participates in the loop.

**Pure chatbot (human in every loop).** Every LLM response goes to the user. The user decides what to do next and manually performs any actions. The LLM has zero autonomy.

**Assisted chatbot (human reviews actions).** The LLM can suggest actions (like code changes), but the human must review and apply them. The human is still in the critical path. Many IDE chat features work this way.

**Supervised agent (human approves some actions).** The LLM can execute some actions autonomously (reading files, searching code) but needs human approval for risky operations (writing files, running commands). Claude Code in its default mode works this way.

**Autonomous agent (human reviews results).** The LLM executes all actions autonomously. The human sees the final result and decides whether to accept it. Codex works this way within its sandbox.

**Fully autonomous agent (human not in the loop).** The LLM completes entire tasks without human involvement. This is the theoretical end of the spectrum — used in CI/CD pipelines and automated refactoring systems where the agent commits directly.

The agent we build will operate primarily in the "supervised agent" mode — autonomous enough to iterate on a task without constant human input, but with safety checks for operations that could cause damage.

## The Architectural Consequences

The chatbot-agent distinction has concrete implications for system architecture. Let's compare:

| Concern | Chatbot | Agent |
|---------|---------|-------|
| Message format | Text only | Text + tool calls + tool results |
| Loop structure | Single turn | Multi-turn with autonomous iterations |
| State management | Conversation history | Conversation history + environment state |
| Error handling | Return error message to user | Return error to model for recovery |
| Context growth | Linear (one message per turn) | Rapid (tool calls + results add tokens quickly) |
| Permission model | Not needed | Essential |
| Side effects | None | File writes, shell commands, etc. |

Each of these differences drives engineering decisions. The message format means you need a structured representation that can hold tool use blocks alongside text. The loop structure means you need a dispatcher that can route tool calls and collect results. The context growth means you need compaction or summarization strategies. The side effects mean you need a permission system.

::: tip In the Wild
The architectural consequences of the chatbot-agent distinction are visible in how the tools handle errors. When ChatGPT generates code with a bug, it tells you "here's the code" and you discover the bug when you run it. When Claude Code generates code with a bug, it discovers the bug itself — because it runs the code, sees the error, and fixes it. The feedback loop between action and observation is what transforms error handling from a user problem into a system capability.
:::

## The "Tool Use" Inflection Point

If there's a single moment where a chatbot becomes an agent, it's the introduction of tool use. Without tools, the system can only produce text. With tools, it can interact with the real world. Everything else — the iterative loop, the permission system, the context management — follows from that single capability.

This is why tool use is such a central topic in this tutorial. When we build our tool system in Chapters 5-8, we're building the mechanism that transforms our system from a chatbot wrapper around an LLM into an actual agent. The tools are the hands and eyes of the agent — without them, the brain (the LLM) is isolated from the world.

Tool use also changes the model's behavior in subtle ways. When a model has tools available, it reasons differently. Instead of generating a complete answer in one shot, it plans a sequence of observations and actions. It asks itself "what do I need to know?" (perception), "what should I do?" (reasoning), and "how do I do it?" (tool selection). The presence of tools activates a more deliberate, investigative mode of operation.

## Common Misconceptions

Let's clear up some misconceptions that arise from the chatbot-agent confusion:

**"An agent is just a chatbot with extra prompting."** No. The difference isn't in the prompt — it's in the architecture. A chatbot with an elaborate system prompt is still a chatbot if it can't use tools or iterate autonomously. The prompt might make the chatbot *seem* more capable, but it doesn't give it the ability to act on the world.

**"Agents are always better than chatbots."** Not at all. For many tasks — answering questions, explaining concepts, generating standalone code snippets — a chatbot is perfectly adequate and simpler to build. You don't need an agentic loop to explain how Python decorators work. Agents shine for tasks that require multi-step interaction with a codebase: debugging, refactoring, feature implementation, test writing.

**"Agents replace developers."** Current coding agents are powerful assistants, not replacements. They handle routine tasks effectively and can tackle moderately complex problems, but they still need human judgment for architectural decisions, requirement interpretation, and quality assessment. The agent handles the typing; you handle the thinking.

**"More autonomy is always better."** Higher autonomy means less human oversight, which means more risk of the agent going in the wrong direction. The optimal autonomy level depends on the task, the stakes, and the user's trust in the model. Sometimes the best agent is one that stops and asks.

## Building the Mental Model

Here's the mental model to carry forward: a coding agent is a chatbot that has been given hands (tools), eyes (observation of tool results), and a permission to keep working without being asked (the iterative loop). It's the same brain (the LLM), but embedded in a body that can interact with the development environment.

When you build your agent, you'll start with the chatbot architecture — a REPL that sends messages to an LLM and displays responses. Then you'll add tools, one by one. Then you'll add the iterative loop. And at that moment, you'll watch your simple chatbot transform into an agent that can read files, write code, run tests, and fix its own mistakes.

That transformation is the core learning experience of this tutorial.

## Key Takeaways

- A chatbot processes text inputs and returns text outputs; an agent extends this with tool use, observation of results, and autonomous multi-step iteration — the distinction is architectural, not cosmetic.
- The human-in-the-loop spectrum ranges from pure chatbot (human in every loop) through supervised agent (human approves risky operations) to fully autonomous agent (human not in the loop).
- Tool use is the inflection point that transforms a chatbot into an agent — without tools, the model can only produce text; with tools, it can perceive and act on the real world.
- The chatbot-agent distinction drives concrete engineering decisions in message format, loop structure, state management, error handling, context growth, and permission systems.
- More autonomy isn't universally better — the optimal level depends on the task, the stakes, and the trust relationship between the user and the model.
