---
title: Input Processing
description: How agent systems receive, validate, and prepare user input before sending it to the language model.
---

# Input Processing

> **What you'll learn:**
> - How user input is transformed into a properly formatted message for the LLM API
> - The role of input preprocessing including command detection, context injection, and message assembly
> - How conversation history is managed and truncated to fit within context window limits

Input processing is the first phase of every agentic loop iteration. It is the transition from Idle to Processing in our state machine -- the moment when raw user input becomes a structured API request. This phase seems simple on the surface (just send the text to the model, right?), but production agents do significant work here: detecting slash commands, injecting system context, assembling the full message array, and managing context window limits.

Getting input processing right determines whether the model receives clear, well-structured prompts or a jumbled mess that degrades response quality. Let's walk through each step.

## From Keystrokes to Messages

When a user types at the agent's prompt, the raw input is a string. Before it reaches the LLM, it passes through several processing stages:

```text
User keystroke input
    |
    v
[1. Raw input capture]  -- Read the line from stdin
    |
    v
[2. Command detection]  -- Check for slash commands (/help, /clear, /exit)
    |
    v
[3. Input validation]   -- Reject empty input, check length limits
    |
    v
[4. Message creation]   -- Wrap in a Message struct with role="user"
    |
    v
[5. History append]     -- Add to the conversation history vector
    |
    v
[6. Context assembly]   -- Build the full API request: system prompt + history + tools
    |
    v
[7. Context management] -- Truncate or compact if exceeding token limits
    |
    v
Ready for LLM invocation
```

Let's examine each stage.

## Stage 1: Raw Input Capture

At the simplest level, you read a line from stdin. But production agents need more than `read_line`. They need line editing (arrow keys, backspace), history recall (up arrow for previous commands), and sometimes multi-line input. In Rust, the `rustyline` crate provides readline-style input handling:

```rust
use rustyline::DefaultEditor;

fn read_user_input(editor: &mut DefaultEditor) -> Result<String, InputError> {
    match editor.readline("> ") {
        Ok(line) => {
            let trimmed = line.trim().to_string();
            if !trimmed.is_empty() {
                editor.add_history_entry(&trimmed)?;
            }
            Ok(trimmed)
        }
        Err(rustyline::error::ReadlineError::Interrupted) => {
            Err(InputError::Interrupted)
        }
        Err(rustyline::error::ReadlineError::Eof) => {
            Err(InputError::Eof)
        }
        Err(e) => Err(InputError::Other(e.to_string())),
    }
}

enum InputError {
    Interrupted,
    Eof,
    Other(String),
}
```

Notice the explicit handling of Ctrl+C (`Interrupted`) and Ctrl+D (`Eof`). These are not errors -- they are user signals that your agent must handle gracefully. An interrupted read should cancel the current input, not crash the agent. An EOF should exit the REPL cleanly.

::: python Coming from Python
Python's `input()` function handles readline automatically if the `readline` module is imported. In Rust, you need to explicitly pull in a crate like `rustyline`. The tradeoff: more setup, but you get full control over key bindings, completion, and history persistence. `rustyline` is the Rust ecosystem's equivalent of Python's `readline` module -- it is the standard choice for interactive CLI input.
:::

## Stage 2: Command Detection

Most coding agents support slash commands -- special inputs that are handled by your code rather than sent to the LLM. Common examples:

- `/help` -- Show available commands
- `/clear` -- Clear conversation history
- `/exit` or `/quit` -- Exit the agent
- `/compact` -- Summarize and compress conversation history
- `/model` -- Switch the active model

These are intercepted before the input reaches the message assembly stage:

```rust
enum InputAction {
    SendToLlm(String),
    SlashCommand { name: String, args: String },
    Empty,
}

fn classify_input(input: &str) -> InputAction {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return InputAction::Empty;
    }

    if trimmed.starts_with('/') {
        let mut parts = trimmed[1..].splitn(2, ' ');
        let name = parts.next().unwrap_or("").to_string();
        let args = parts.next().unwrap_or("").to_string();
        return InputAction::SlashCommand { name, args };
    }

    InputAction::SendToLlm(trimmed.to_string())
}
```

Slash commands bypass the agentic loop entirely. They are handled in the outer REPL, not the inner loop. This is an important design decision: slash commands are user-to-agent communication (configuring the agent itself), while normal messages are user-to-model communication (asking the model to do something).

::: tip In the Wild
Claude Code supports a rich set of slash commands including `/clear`, `/compact`, `/cost`, `/doctor`, `/help`, `/init`, `/login`, `/logout`, `/model`, and `/review`. OpenCode has a similar set with `/clear`, `/compact`, `/model`, and `/help`. Both agents process these commands in their outer REPL before the input reaches the agentic loop. Claude Code's `/compact` command is particularly interesting -- it summarizes the conversation history to free up context window space without losing important context.
:::

## Stage 3: Input Validation

Before creating a message, validate the input:

```rust
const MAX_INPUT_LENGTH: usize = 100_000; // Characters, not tokens

fn validate_input(input: &str) -> Result<(), InputError> {
    if input.trim().is_empty() {
        return Err(InputError::Empty);
    }

    if input.len() > MAX_INPUT_LENGTH {
        return Err(InputError::TooLong {
            length: input.len(),
            max: MAX_INPUT_LENGTH,
        });
    }

    Ok(())
}
```

The length limit is a sanity check, not a token budget enforcement. A user could accidentally paste a massive file into the prompt; catching this early avoids wasting an API call that will fail due to context limits.

## Stage 4-5: Message Creation and History

Once validated, the input becomes a message and joins the conversation history:

```rust
#[derive(Clone)]
struct Message {
    role: String,
    content: String,
}

struct ConversationHistory {
    messages: Vec<Message>,
}

impl ConversationHistory {
    fn add_user_message(&mut self, content: String) {
        self.messages.push(Message {
            role: "user".to_string(),
            content,
        });
    }

    fn add_assistant_message(&mut self, content: String) {
        self.messages.push(Message {
            role: "assistant".to_string(),
            content,
        });
    }

    fn all_messages(&self) -> &[Message] {
        &self.messages
    }

    fn clear(&mut self) {
        self.messages.clear();
    }
}
```

The conversation history is the agent's memory. It contains every user message, every assistant response, every tool call, and every tool result from the entire session. This is what gives the LLM context about previous interactions.

## Stage 6: Context Assembly

The API request is more than just the conversation history. It combines several components:

```rust
struct ApiRequest {
    model: String,
    system: String,
    messages: Vec<Message>,
    tools: Vec<ToolDefinition>,
    max_tokens: u32,
}

fn assemble_request(
    history: &ConversationHistory,
    system_prompt: &str,
    tools: &[ToolDefinition],
    model: &str,
) -> ApiRequest {
    ApiRequest {
        model: model.to_string(),
        system: system_prompt.to_string(),
        messages: history.all_messages().to_vec(),
        tools: tools.to_vec(),
        max_tokens: 4096,
    }
}
```

The **system prompt** is critical. It tells the model who it is, what it can do, and how it should behave. For a coding agent, the system prompt typically includes:

- The agent's identity and purpose ("You are a coding assistant...")
- Instructions for tool use ("When you need to read a file, use the read_file tool...")
- The current working directory and project context
- Behavioral guidelines ("Always explain what you are about to do before doing it...")

The system prompt is not part of the conversation history. It is sent with every API call but does not appear as a user or assistant message. This means you can change it between calls without polluting the conversation -- for example, updating the working directory if the user navigates to a different folder.

## Stage 7: Context Window Management

Every LLM has a context window limit -- a maximum number of tokens it can process in a single request. Claude's context windows range from 200K tokens for standard models. Your system prompt, conversation history, tool definitions, and the model's response all count against this limit.

As the conversation grows, you will eventually approach the limit. At that point, you have several options:

**Truncation** -- Remove the oldest messages from history, keeping the system prompt and the most recent exchanges. This is simple but can lose important context from earlier in the conversation.

**Compaction** -- Summarize older messages into a single condensed message. The model itself can do this summarization. This preserves the key information while dramatically reducing token count.

**Sliding window** -- Keep the first few messages (which often contain the task definition) and the most recent N messages, dropping everything in between.

```rust
fn manage_context(
    history: &mut ConversationHistory,
    system_prompt: &str,
    tools: &[ToolDefinition],
    max_context_tokens: usize,
) {
    let total_tokens = estimate_tokens(system_prompt, history.all_messages(), tools);

    if total_tokens <= max_context_tokens {
        return; // No management needed
    }

    // Strategy: keep first 2 messages and last N messages
    let messages = history.all_messages();
    if messages.len() <= 4 {
        return; // Too few messages to truncate
    }

    let first_two: Vec<Message> = messages[..2].to_vec();
    let mut recent = messages[2..].to_vec();

    // Remove oldest messages from the middle until we fit
    while estimate_tokens(system_prompt, &combine(&first_two, &recent), tools)
        > max_context_tokens
        && recent.len() > 2
    {
        recent.remove(0);
    }

    history.replace(combine(&first_two, &recent));
}

fn combine(a: &[Message], b: &[Message]) -> Vec<Message> {
    let mut result = a.to_vec();
    result.extend_from_slice(b);
    result
}

fn estimate_tokens(
    system: &str,
    messages: &[Message],
    tools: &[ToolDefinition],
) -> usize {
    // Rough estimate: 1 token per 4 characters
    let mut chars = system.len();
    for msg in messages {
        chars += msg.content.len();
    }
    // Tool definitions also consume tokens
    for tool in tools {
        chars += tool.estimated_token_size();
    }
    chars / 4
}
```

Token estimation is inherently approximate -- the exact count depends on the model's tokenizer. Production agents use the model's actual tokenizer for precise counts, but a character-based heuristic (roughly 4 characters per token for English text) is good enough for context management decisions.

::: tip In the Wild
Claude Code implements context management through its `/compact` command and automatic context compaction. When the conversation approaches the context limit, Claude Code summarizes the history using the model itself, replacing detailed tool call sequences with a condensed summary. OpenCode also implements automatic compaction -- when the context window fills up, it summarizes the conversation and continues with the compressed history. Both agents keep the system prompt and the most recent messages intact while compressing the middle of the conversation.
:::

## The Complete Input Pipeline

Putting it all together, here is the complete input processing function:

```rust
enum TurnAction {
    RunAgentLoop(ApiRequest),
    HandleCommand { name: String, args: String },
    Skip,
    Exit,
}

fn process_input(
    editor: &mut DefaultEditor,
    history: &mut ConversationHistory,
    system_prompt: &str,
    tools: &[ToolDefinition],
    model: &str,
    max_context_tokens: usize,
) -> Result<TurnAction, InputError> {
    let input = read_user_input(editor)?;

    match classify_input(&input) {
        InputAction::Empty => Ok(TurnAction::Skip),

        InputAction::SlashCommand { name, args } => {
            if name == "exit" || name == "quit" {
                Ok(TurnAction::Exit)
            } else {
                Ok(TurnAction::HandleCommand { name, args })
            }
        }

        InputAction::SendToLlm(text) => {
            validate_input(&text)?;
            history.add_user_message(text);
            manage_context(history, system_prompt, tools, max_context_tokens);
            let request = assemble_request(history, system_prompt, tools, model);
            Ok(TurnAction::RunAgentLoop(request))
        }
    }
}
```

This function is the complete transition from Idle to Processing. It handles every input type -- empty lines, slash commands, and messages to the model -- and produces a clear action for the outer REPL to execute.

## Key Takeaways

- Input processing transforms raw user text into a structured API request through seven stages: capture, command detection, validation, message creation, history append, context assembly, and context management
- Slash commands are intercepted before reaching the LLM and handled by the outer REPL, not the agentic loop
- The conversation history is the agent's memory -- every message, tool call, and tool result is stored here and sent with each API request
- Context window management is essential for long conversations; strategies include truncation (drop old messages), compaction (summarize old messages), and sliding windows (keep first and last, drop middle)
- The system prompt, tool definitions, and conversation history together form the complete context that the LLM sees -- getting this assembly right directly affects response quality
