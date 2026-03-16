---
title: What Are Tools
description: Define what tools mean in the context of LLM agents and why they are necessary for real-world task completion.
---

# What Are Tools

> **What you'll learn:**
> - How tools extend an LLM's capabilities beyond text generation to interact with the real world
> - The difference between tools the model calls (function calling) and tools a human uses
> - Why a coding agent needs file, shell, and search tools as its minimum viable set

In Chapter 3 you built an agentic loop that can detect when the model wants to call a tool. But "wanting" is not "doing." Right now, when your loop encounters a `tool_use` block, it has nowhere to send it. The model asks to read a file and your agent shrugs. This chapter changes that. Before you write any code, though, you need a clear mental model of what tools actually are and why they matter.

## The LLM's Limitation

A large language model is, at its core, a text predictor. Given a sequence of tokens, it produces the next most likely token. This means it can reason about code, explain algorithms, draft documentation, and carry on a conversation. What it cannot do is *act*. It cannot read a file on your disk. It cannot run a shell command. It cannot open a network connection. It cannot check the current time.

This is not a bug -- it is a fundamental architectural boundary. The model runs in a stateless inference environment. It has no file system, no process table, no network stack. Every "fact" it produces comes from patterns in its training data or from what you put in its context window.

Tools are the bridge across this boundary. A tool is a function that your agent code provides, which the model can request to invoke. When the model decides it needs to read a file, it does not somehow reach out to your operating system. Instead, it emits a structured message -- a `tool_use` block -- that says "I want to call the `read_file` tool with the argument `path: src/main.rs`." Your agent code receives this message, executes the actual file read, and feeds the result back to the model as an observation.

Think of it this way: the model is the brain, and tools are its arms and eyes.

## Function Calling: The Mechanism

The specific mechanism that enables tools is called **function calling** (or tool use, depending on the API). Here is how it works with the Anthropic Messages API:

1. **You declare tools.** When you send a request to the API, you include a `tools` array that describes each tool: its name, a description of what it does, and a JSON schema defining its input parameters.

2. **The model decides to use one.** Based on the conversation and the tool descriptions, the model may produce a response with `stop_reason: "tool_use"` and one or more `tool_use` content blocks. Each block contains the tool name, a unique `id`, and the input arguments as a JSON object.

3. **You execute it.** Your code receives the `tool_use` block, looks up the tool by name, validates the input, runs the tool's logic, and collects the result.

4. **You feed the result back.** You append a `tool_result` content block to the conversation, referencing the `tool_use_id`, and send the updated conversation back to the API. The model now sees the result and can reason about it.

This cycle repeats inside the agentic loop you built in Chapter 3. The model may call tools several times in succession -- read a file, then edit it, then run a test -- before it decides the task is complete and produces a final text response.

```json
{
  "tools": [
    {
      "name": "read_file",
      "description": "Read the contents of a file at the given path.",
      "input_schema": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "The absolute or relative path to the file."
          }
        },
        "required": ["path"]
      }
    }
  ]
}
```

This JSON snippet shows a single tool declaration. The `input_schema` is a JSON Schema object that tells the model exactly what arguments the tool accepts. You will learn how to construct these schemas in Rust in the upcoming subchapters.

::: python Coming from Python
If you have used the OpenAI or Anthropic Python SDKs, you may have declared tools using dictionaries or Pydantic models. The concept is identical in Rust -- the only difference is that you construct the JSON schema using `serde_json::json!` macros or derive it from Rust structs. The structured typing actually makes Rust a natural fit for this: your tool's input types are checked at compile time.
:::

## Tools vs. Prompts

A common question is: why not just ask the model to "imagine" it can read files and include the file contents in the prompt? Some early agent prototypes did exactly that. The problem is reliability. When the model "imagines" a file read, it is generating plausible-looking content from its training data. It might hallucinate file contents, invent function signatures that do not exist, or produce output that looks correct but is subtly wrong.

Tools give you **grounded** information. When the model calls `read_file`, it gets the actual bytes on disk, not a guess. When it calls `run_shell`, it gets real stdout and stderr, not simulated output. This grounding is what makes the difference between an agent that *sounds* helpful and one that *is* helpful.

## The Minimum Viable Toolset

What tools does a coding agent need at minimum? Look at what a human developer does:

1. **Read files** -- You open a file to understand the current code.
2. **Write/edit files** -- You make changes to implement a feature or fix a bug.
3. **Run commands** -- You execute tests, build the project, run linters.
4. **Search** -- You grep through a codebase to find relevant code.

Every production coding agent has some version of these four categories. Claude Code has `Read`, `Write`, `Edit`, `Bash`, and `Grep` (among others). OpenAI's Codex provides `shell`, `read`, `write`, and `patch`. The specific tool names differ, but the capabilities map to the same developer workflow.

In this chapter you will build the *framework* that makes these tools possible. You will define the trait that every tool must implement, the registry that stores them, and the dispatch logic that routes tool calls. In Chapter 5 you will implement the actual file tools, and in Chapter 6 you will add shell execution.

::: wild In the Wild
Claude Code ships with approximately a dozen tools, including `Read`, `Write`, `Edit`, `Bash`, `Glob`, `Grep`, and `WebFetch`. Each tool is a self-contained module that registers itself with the tool system at startup. OpenCode takes a similar approach with tools like `shell`, `read`, `write`, and `patch`. Both agents structure their tools around what a developer does at a terminal, not around abstract capabilities.
:::

## Tools as a Contract

From a software design perspective, a tool is a contract between three parties:

1. **The model** sees a name, a description, and an input schema. It uses this information to decide when to call the tool and what arguments to pass.
2. **The agent code** sees an interface (in Rust, a trait) that it can call with the provided arguments. It does not care about the tool's internal logic -- it just calls `execute` and collects the result.
3. **The tool implementation** does the actual work. It reads a file, runs a command, queries a database. It returns a result string that becomes the observation.

This three-way contract is the design principle behind everything you will build in this chapter. The `Tool` trait encodes the contract. The JSON schema formalizes the input side. The tool result formalizes the output side. Get this contract right, and adding new tools later is trivial.

## Key Takeaways

- Tools bridge the gap between the model's reasoning ability and the real world -- they let the model read files, run commands, and observe results instead of guessing.
- Function calling is the API mechanism: you declare tools in the request, the model emits `tool_use` blocks, you execute them and feed `tool_result` observations back.
- A coding agent needs at minimum: file reading, file writing/editing, shell execution, and search. Everything else is an enhancement.
- Tools are a three-way contract between the model (which sees descriptions and schemas), the agent framework (which calls `execute`), and the implementation (which does the work).
- This chapter builds the framework; Chapters 5 and 6 fill it with concrete tool implementations.
