---
title: Single vs Multi Turn
description: Compare single-turn request-response patterns with multi-turn agentic conversations and their trade-offs.
---

# Single vs Multi Turn

> **What you'll learn:**
> - When a single LLM call is sufficient versus when the agentic loop must iterate multiple times
> - How multi-turn interactions accumulate context and why that affects both quality and cost
> - How to design your agent so users can switch between single-shot mode and full agentic mode

Not every user request needs the full agentic loop. Some questions have straightforward answers that the model can provide in a single call. Others require reading files, making changes, and verifying results across many turns. Understanding this spectrum helps you design an agent that is efficient for simple queries and powerful for complex tasks.

## The Single-Turn Case

A single-turn interaction is one where the model responds with `stop_reason: "end_turn"` on the first call. No tools, no loop iteration -- just a question and an answer.

Examples of single-turn interactions:
- "What does the `?` operator do in Rust?"
- "Write me a function that reverses a string."
- "Explain the difference between `&str` and `String`."

In these cases, the agentic loop runs exactly once:

```text
User message → LLM call → end_turn → return response
```

The loop overhead is minimal -- one API call, one match on `stop_reason`, one exit. Your loop handles this case naturally without any special code:

```rust
// The loop handles single-turn interactions automatically.
// If the model doesn't need tools, it responds with end_turn
// on the first iteration and the loop exits immediately.
loop {
    let response = self.call_api(state).await?;
    state.add_assistant_message(response.content.clone());

    match response.stop_reason.as_deref() {
        Some("end_turn") => {
            // Single turn: model answered directly
            return Ok(LoopResult::Complete(extract_text(&response.content)));
        }
        Some("tool_use") => {
            // Multi-turn: tools needed
            // ...
        }
        // ...
    }
}
```

## The Multi-Turn Case

Multi-turn interactions are where the agentic loop earns its keep. The model needs to interact with the world -- reading files, running commands, checking results -- before it can answer.

Consider "Add proper error handling to src/main.rs." This triggers a multi-turn sequence:

```text
Turn 1: User asks → Model calls read_file("src/main.rs")
Turn 2: Model sees file contents → Calls write_file("src/main.rs", new_content)
Turn 3: Model sees write confirmation → Calls run_command("cargo check")
Turn 4: Model sees compilation output → Responds with summary (end_turn)
```

Four turns, three tool calls. Each turn adds at least two messages to the conversation state (the assistant's response and the tool result), so the message history grows from 1 message to 9 messages.

## The Cost Curve

Multi-turn interactions are more expensive in two ways: API cost and latency. Here is how the costs scale:

```text
Turn 1: Input tokens = system_prompt + user_message
Turn 2: Input tokens = system_prompt + user_message + assistant_1 + tool_result_1
Turn 3: Input tokens = system_prompt + all_previous_messages
Turn N: Input tokens = system_prompt + ALL previous messages
```

Because the full history is sent with every call, input tokens grow *quadratically* with the number of turns. A 4-turn interaction does not cost 4x a single turn -- it costs roughly 1x + 2x + 3x + 4x = 10x in input tokens (the exact numbers depend on message sizes, but the growth pattern is real).

Let's quantify this with a concrete example:

```rust
/// Estimate the cumulative input token cost of a multi-turn interaction.
fn estimate_cumulative_cost(
    system_tokens: usize,
    message_tokens: &[usize],  // tokens per message
) -> usize {
    let mut total = 0;
    for turn in 0..message_tokens.len() {
        // Each API call sends the system prompt plus all messages so far
        let input_for_this_call = system_tokens
            + message_tokens[..=turn].iter().sum::<usize>();
        total += input_for_this_call;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_growth() {
        let system = 500; // 500-token system prompt
        // 8 messages: user, asst+tool, result, asst+tool, result, asst+tool, result, asst_final
        let messages = vec![100, 200, 500, 150, 30, 200, 800, 100];

        let total = estimate_cumulative_cost(system, &messages);
        // Much more than 8x a single message due to cumulative history
        println!("Total input tokens across all calls: {}", total);
        assert!(total > messages.iter().sum::<usize>() * 2);
    }
}
```

::: python Coming from Python
If you have built chatbots in Python, you have probably noticed that API costs are manageable for single-turn use. The moment you add tool use and the conversation grows, costs jump significantly. This is the same in Rust -- the language does not affect the API cost. What Rust *can* help with is efficient token counting and context management, because you can track the exact size of your serialized messages without relying on estimated character counts.
:::

## When Single-Turn Is Enough

Not every agent request needs tools. Design your agent to handle simple questions efficiently. Here are guidelines for when single-turn is appropriate:

**Knowledge questions**: "What does this Rust syntax mean?" The model has this knowledge and does not need to read any files.

**Code generation from a description**: "Write a function that parses a CSV file." The model can generate this from its training data.

**Explanations of provided code**: If the user pastes code directly into their message, the model can analyze it without tool calls.

The key insight: if the model *already has all the information it needs* in the conversation history, it does not need tools and will respond in a single turn.

## When Multi-Turn Is Required

Tool calls are needed when the model must interact with the user's specific environment:

**Reading the codebase**: "Explain what the `Agent` struct does." The model needs to read `src/agent.rs` to answer this accurately.

**Making changes**: "Fix the compilation error." The model needs to read the error, read the relevant file, make a change, and verify it compiles.

**Gathering information**: "What dependencies does this project use?" The model needs to read `Cargo.toml`.

**Verification**: "Refactor this function and make sure the tests still pass." The model needs to make changes and then run the test suite.

## Designing for Both Modes

Your agent should handle both cases without the user needing to switch modes. The agentic loop already does this naturally -- if no tool calls are needed, it exits on the first iteration. But there are some design considerations:

**System prompt guidance**: Your system prompt can influence whether the model uses tools. A prompt that says "Always read the file before making changes" will cause more tool calls than one that says "Answer directly when possible, use tools only when needed."

```rust
// Encourage efficient tool use in the system prompt
let system_prompt = "\
You are a coding assistant with access to file and shell tools. \
Answer questions directly from your knowledge when possible. \
Use tools only when you need to interact with the user's actual codebase — \
reading files, writing changes, running commands, or checking results.";
```

**Display feedback**: In multi-turn mode, the user is waiting while the agent works. Good agents show progress during tool execution:

```rust
fn display_progress(call: &ToolCall) {
    match call.name.as_str() {
        "read_file" => {
            let path = call.input["path"].as_str().unwrap_or("unknown");
            println!("  Reading {}...", path);
        }
        "write_file" => {
            let path = call.input["path"].as_str().unwrap_or("unknown");
            println!("  Writing {}...", path);
        }
        "run_command" => {
            let cmd = call.input["command"].as_str().unwrap_or("unknown");
            println!("  Running `{}`...", cmd);
        }
        other => {
            println!("  Running tool '{}'...", other);
        }
    }
}
```

**Cost transparency**: Consider showing the user how many turns and tokens a request consumed:

```rust
impl LoopResult {
    pub fn summary(&self, turns: usize, usage: &Usage) -> String {
        let status = match self {
            LoopResult::Complete(_) => "Completed",
            LoopResult::MaxTokens(_) => "Truncated (max tokens)",
            LoopResult::TurnLimitReached(_) => "Stopped (turn limit)",
            LoopResult::ContextOverflow => "Stopped (context overflow)",
        };

        format!(
            "[{} in {} turn(s), {} input + {} output tokens]",
            status, turns, usage.input_tokens, usage.output_tokens,
        )
    }
}
```

::: wild In the Wild
Claude Code visually distinguishes between single-turn and multi-turn interactions. For simple questions, it shows the response immediately. For tool-using interactions, it displays each tool call as it happens with a progress indicator -- "Reading src/main.rs... Writing src/main.rs... Running cargo check..." This gives the user confidence that the agent is making progress and lets them interrupt if it goes off track. OpenCode takes a similar approach, showing tool calls inline as they execute.
:::

## The Spectrum in Practice

Real-world agent usage spans the full spectrum:

| Request type | Turns | Tool calls | Token cost |
|---|---|---|---|
| "What is a lifetime in Rust?" | 1 | 0 | Low |
| "Read src/main.rs" | 2 | 1 | Low-medium |
| "Add logging to the agent module" | 3-5 | 2-4 | Medium |
| "Refactor the error handling across all files" | 8-15 | 6-12 | High |
| "Set up a new feature with tests and docs" | 15-25 | 10-20 | Very high |

Your turn limit (default 25) should accommodate the "very high" category while preventing truly runaway loops. If a user regularly hits the limit, they can increase it. If they mostly do simple queries, the loop exits on the first turn and the limit is irrelevant.

## Key Takeaways

- Single-turn interactions (no tools needed) are handled naturally by the agentic loop -- the model returns `end_turn` on the first iteration and the loop exits immediately
- Multi-turn interactions grow cost quadratically because the full message history is sent with every API call -- a 4-turn interaction costs roughly 10x a single turn in input tokens
- Design the system prompt to encourage efficient tool use: answer from knowledge when possible, use tools only when interacting with the actual codebase
- Show progress during multi-turn interactions so the user knows the agent is working and can interrupt if needed
- The same loop architecture handles both modes seamlessly; the model decides whether to use tools based on the request and context
