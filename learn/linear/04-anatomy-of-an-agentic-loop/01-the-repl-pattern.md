---
title: The REPL Pattern
description: Starting from the familiar Read-Eval-Print Loop and understanding why it serves as the foundation for agent architecture.
---

# The REPL Pattern

> **What you'll learn:**
> - How the classic REPL pattern (Read, Eval, Print, Loop) maps to the structure of an interactive agent
> - Why the REPL is the natural starting point for building a coding agent's runtime
> - How to extend the basic REPL with state persistence and multi-step evaluation to approach agentic behavior

Every programmer has used a REPL. You open a Python interpreter, type an expression, see the result, and type another. This deceptively simple pattern -- Read, Evaluate, Print, Loop -- is the oldest and most intuitive model for interactive computation. It is also the architectural ancestor of every coding agent you will build.

Before we dive into the complexities of agentic loops, tool dispatch, and state machines, let's ground ourselves in this familiar territory. Understanding what a REPL does well and where it falls short gives you the clearest path to understanding why agents are built the way they are.

## The Classic REPL

The REPL pattern has four phases that execute in a continuous cycle:

1. **Read** -- Accept input from the user. In a Python REPL, this is the `>>>` prompt waiting for you to type an expression. In a coding agent, this is the chat prompt waiting for a natural-language instruction.

2. **Evaluate** -- Process the input. In Python, this means parsing and executing the expression. In a coding agent, this means sending the user's message to an LLM and processing the response.

3. **Print** -- Display the result. Python prints the return value; a coding agent displays the LLM's response text.

4. **Loop** -- Go back to step 1 and wait for the next input.

Here is the simplest possible REPL in Rust:

```rust
use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("> ");
        stdout.flush().unwrap();

        let mut input = String::new();
        stdin.lock().read_line(&mut input).unwrap();

        let input = input.trim();
        if input.is_empty() {
            continue;
        }
        if input == "exit" {
            break;
        }

        // Evaluate: for now, just echo
        let response = format!("You said: {}", input);

        // Print
        println!("{}", response);
    }
}
```

This is a working REPL. It reads a line, "evaluates" it (in this case, just echoing), prints the result, and loops. Every interactive CLI tool you have ever used -- Python's interpreter, Node's REPL, irb for Ruby, your shell itself -- follows this same skeleton.

::: python Coming from Python
Python's built-in REPL is literally this pattern, implemented in C inside `PyRun_InteractiveLoopFlags()`. When you type `python3` with no arguments, you enter a REPL where each expression is compiled, evaluated, and its result printed. The `code.InteractiveConsole` class lets you build custom REPLs in Python:
```python
import code
console = code.InteractiveConsole()
console.interact("Welcome to my REPL")
```
In Rust, there is no built-in REPL infrastructure -- you build it from `stdin`/`stdout` primitives. This gives you more control but requires more setup.
:::

## From Echo to LLM

The echo REPL above is not very interesting. Let's make the "Evaluate" step call an LLM instead of echoing. Conceptually, the change is small:

```rust
use std::io::{self, BufRead, Write};

struct Message {
    role: String,
    content: String,
}

// Imagine this function calls the Claude API and returns the response text
fn call_llm(messages: &[Message]) -> Result<String, Box<dyn std::error::Error>> {
    // ... HTTP request to the API ...
    todo!("We will implement this in the project track")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut history: Vec<Message> = Vec::new();

    loop {
        print!("> ");
        stdout.flush()?;

        let mut input = String::new();
        stdin.lock().read_line(&mut input)?;
        let input = input.trim().to_string();

        if input.is_empty() {
            continue;
        }
        if input == "exit" {
            break;
        }

        // Read: add user message to history
        history.push(Message {
            role: "user".to_string(),
            content: input,
        });

        // Evaluate: call the LLM
        let response = call_llm(&history)?;

        // Store assistant response in history
        history.push(Message {
            role: "assistant".to_string(),
            content: response.clone(),
        });

        // Print
        println!("{}", response);
    }

    Ok(())
}
```

This is now a chatbot. It reads user input, sends the full conversation history to an LLM, prints the response, and loops. The critical difference from the echo REPL is the `history` vector -- by accumulating messages and sending them all on each turn, the chatbot maintains conversational context. The LLM sees everything that has been said before and can refer back to it.

## What the REPL Gets Right

The REPL pattern gives us several properties that are essential for agent architecture:

**Interactivity.** The user stays in control. They can steer the conversation, ask follow-up questions, or change direction at any time. This is the outer loop that every agent needs.

**Statefulness through history.** By accumulating messages, the REPL maintains a conversation state that persists across turns. The LLM does not remember anything between API calls -- all memory is in the message history that your code manages.

**Simplicity.** The control flow is linear and easy to reason about: read, evaluate, print, repeat. There are no callbacks, no event systems, no complex state transitions. You can trace through the code in your head.

**Graceful termination.** The loop exits when the user says so (or when an unrecoverable error occurs). This gives you a natural place to clean up resources.

## Where the REPL Falls Short

But the basic REPL has a fundamental limitation: **the evaluation step is a single, atomic operation**. The user asks something, the LLM responds, and we are done. There is no way for the LLM to take an action, observe the result, and then decide what to do next -- all within a single user turn.

Consider this interaction with a plain chatbot:

```text
User: Read the file src/main.rs and fix any compilation errors
Assistant: I'd be happy to help! Please paste the contents of src/main.rs
           and I'll identify any errors.
```

The model cannot actually read the file. It can only respond with text. To fix compilation errors, you would need to manually copy the file contents, paste them, get the LLM's suggestions, manually apply them, run the compiler, paste any new errors, and repeat. You are the one doing all the looping -- the REPL just bounces messages back and forth.

This is the gap that agentic architecture fills. An agent keeps the REPL as its outer loop (the user can always type something new), but adds an **inner loop** where the LLM can call tools, observe results, and continue reasoning -- all before returning control to the user.

## The REPL as Outer Loop

Here is the key insight: the REPL does not go away when you build an agent. It becomes the **outer loop** -- the user-facing interaction cycle. Inside the "Evaluate" step, you embed a second loop where the LLM and tools interact autonomously:

```text
Outer Loop (REPL):
  Read  -> user types a message
  Eval  -> [inner agentic loop runs, possibly many LLM calls and tool executions]
  Print -> display final result to user
  Loop  -> wait for next message
```

The outer REPL gives the user control: they can interrupt, ask follow-ups, or change course. The inner loop gives the agent autonomy: it can take multi-step actions without waiting for user input at each step.

This two-loop architecture is universal across coding agents. Claude Code, OpenCode, Cursor, Aider -- they all have an outer REPL where the user types messages and an inner loop where the agent works autonomously.

::: tip In the Wild
Claude Code's outer loop is its main REPL that accepts user input at the `>` prompt. When you type a message, the inner agentic loop takes over -- potentially reading files, running commands, editing code, and making multiple LLM calls -- before returning control to the prompt. OpenCode follows the same pattern with its TUI (terminal UI) input box serving as the Read step and its `agent.Run()` method implementing the inner loop.
:::

## State: The Missing Ingredient

The basic REPL stores one piece of state: the conversation history. But an agent needs more. It needs to track:

- **Tool definitions** available to the model
- **System prompt** with instructions for behavior
- **Working directory** and project context
- **Token count** to know when the context window is filling up
- **Iteration count** to prevent infinite loops

This state lives outside the REPL loop and persists across turns. In Rust, you would gather it into a struct:

```rust
struct AgentState {
    history: Vec<Message>,
    system_prompt: String,
    tools: Vec<ToolDefinition>,
    working_directory: std::path::PathBuf,
    total_tokens_used: usize,
    max_iterations: usize,
}
```

We will flesh out this state structure as the chapter progresses. For now, the key point is that the REPL pattern naturally accommodates state -- you just define what you need before the loop starts and pass it through each iteration.

## Key Takeaways

- The REPL (Read, Eval, Print, Loop) is the foundational pattern for every interactive coding agent -- it does not disappear when you add agent capabilities, it becomes the outer loop
- A chatbot is a REPL where the Evaluate step calls an LLM with conversation history -- this gives you multi-turn conversation but not autonomous action
- The critical limitation of a plain REPL is that evaluation is atomic: the LLM cannot take actions, observe results, and decide what to do next within a single user turn
- Agents solve this by nesting an inner loop (the agentic loop) inside the REPL's Evaluate step, giving the LLM autonomy while keeping the user in control through the outer loop
- Agent state goes beyond conversation history to include tool definitions, system prompts, token budgets, and iteration limits
