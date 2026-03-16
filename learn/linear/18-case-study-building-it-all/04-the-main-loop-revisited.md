---
title: The Main Loop Revisited
description: Re-examine the agentic loop with full knowledge of all subsystems, understanding how safety, streaming, context management, and tools integrate.
---

# The Main Loop Revisited

> **What you'll learn:**
> - How the agentic loop from Chapter 4 evolves when integrated with the full safety layer, provider abstraction, and context management system
> - The precise sequence of operations in a production loop iteration: context assembly, permission check, API call, stream processing, tool execution, and audit logging
> - How to handle the complex interactions between loop components — when a safety check fails mid-stream, when context exceeds limits during a turn, when a provider falls back

In Chapter 4, you built a minimal agentic loop: read input, call the LLM, check for tool calls, execute them, repeat. That loop worked, but it was a sketch. It had no safety checks, no streaming, no context management, no provider abstraction, and no error recovery. Now you know all those systems. Let's put them together and see what the production version of the loop actually looks like.

## The Chapter 4 Loop vs. The Production Loop

Here is the loop you built in Chapter 4, simplified:

```rust
// The Chapter 4 loop — minimal and instructive
loop {
    let response = client.send_messages(&messages).await?;

    match response {
        LlmResponse::Text(text) => {
            println!("{}", text);
            break; // No tool calls — we're done
        }
        LlmResponse::ToolCall(call) => {
            let result = execute_tool(&call)?;
            messages.push(Message::tool_result(call.id, result));
        }
    }
}
```

This is six lines. The production version is closer to sixty, because every one of those six lines expands to handle real-world concerns. Let's walk through a single iteration of the production loop.

## The Production Loop

```rust
pub async fn run_turn(
    user_input: &str,
    provider: &dyn Provider,
    tools: &ToolRegistry,
    safety: &SafetyLayer,
    context: &Arc<RwLock<ContextManager>>,
    renderer: &dyn Renderer,
) -> anyhow::Result<TurnOutcome> {
    // Phase 1: Add user message and manage context
    {
        let mut ctx = context.write().await;
        ctx.add_user_message(user_input);

        if ctx.approaching_limit() {
            tracing::info!("Context approaching limit, compacting...");
            ctx.compact()?;
            renderer.show_status("Context compacted to fit within token limits").await?;
        }
    } // Write lock released here

    // Phase 2: The inner agentic loop
    let mut iteration = 0;
    let max_iterations = 25; // Safety stop to prevent infinite loops

    loop {
        iteration += 1;
        if iteration > max_iterations {
            renderer.show_warning(
                "Reached maximum iteration limit. Stopping to prevent infinite loop."
            ).await?;
            return Ok(TurnOutcome::MaxIterationsReached);
        }

        // Phase 3: Assemble messages for the API call
        let messages = {
            let ctx = context.read().await;
            ctx.assemble_messages(tools.tool_definitions())
        };

        // Phase 4: Call the provider with streaming
        let mut stream = provider.stream_completion(&messages).await
            .map_err(|e| {
                tracing::error!("Provider error: {}", e);
                e
            })?;

        // Phase 5: Process the streamed response
        let mut full_response = AssistantResponse::new();
        renderer.begin_response().await?;

        while let Some(chunk) = stream.next().await {
            match chunk? {
                StreamChunk::Text(text) => {
                    full_response.push_text(&text);
                    renderer.render_text_chunk(&text).await?;
                }
                StreamChunk::ToolCallStart(call_info) => {
                    full_response.begin_tool_call(call_info);
                    renderer.show_tool_call_start(&call_info.name).await?;
                }
                StreamChunk::ToolCallDelta(delta) => {
                    full_response.append_tool_call_args(&delta);
                }
                StreamChunk::Done => break,
            }
        }

        renderer.end_response().await?;

        // Phase 6: Store the assistant message
        {
            let mut ctx = context.write().await;
            ctx.add_assistant_message(&full_response);
        }

        // Phase 7: If no tool calls, the turn is complete
        if full_response.tool_calls().is_empty() {
            return Ok(TurnOutcome::Complete);
        }

        // Phase 8: Execute tool calls through the safety layer
        let mut tool_results = Vec::new();

        for tool_call in full_response.tool_calls() {
            // Safety check before execution
            let permission = safety.check_tool_call(tool_call).await?;

            match permission {
                Permission::Allowed => {
                    // Execute the tool
                    let result = tools.execute(tool_call).await;
                    let tool_result = match result {
                        Ok(output) => ToolResult::success(tool_call.id.clone(), output),
                        Err(e) => {
                            tracing::warn!("Tool {} failed: {}", tool_call.name, e);
                            ToolResult::error(tool_call.id.clone(), e.to_string())
                        }
                    };
                    renderer.show_tool_result(&tool_result).await?;
                    tool_results.push(tool_result);
                }
                Permission::Denied(reason) => {
                    let denial = ToolResult::error(
                        tool_call.id.clone(),
                        format!("Permission denied: {}", reason),
                    );
                    renderer.show_permission_denied(&tool_call.name, &reason).await?;
                    tool_results.push(denial);
                }
                Permission::NeedsApproval => {
                    let approved = renderer
                        .prompt_tool_approval(tool_call)
                        .await?;

                    if approved {
                        let result = tools.execute(tool_call).await;
                        let tool_result = match result {
                            Ok(output) => ToolResult::success(tool_call.id.clone(), output),
                            Err(e) => ToolResult::error(tool_call.id.clone(), e.to_string()),
                        };
                        renderer.show_tool_result(&tool_result).await?;
                        tool_results.push(tool_result);
                    } else {
                        let denial = ToolResult::error(
                            tool_call.id.clone(),
                            "User denied this tool call".into(),
                        );
                        tool_results.push(denial);
                    }
                }
            }
        }

        // Phase 9: Add tool results to context and continue the loop
        {
            let mut ctx = context.write().await;
            ctx.add_tool_results(&tool_results);

            // Check context limits again after adding tool results
            if ctx.approaching_limit() {
                ctx.compact()?;
            }
        }

        // Loop continues — the model will see the tool results
        // and either make more tool calls or produce a final response
    }
}
```

That is the production loop. Let's break down what changed from the Chapter 4 version and why.

## Phase-by-Phase Walkthrough

### Phase 1: Context Management Before the Call

Before you call the LLM, you need to ensure the conversation fits within the context window. The user's new message might push the total token count past the limit. The context manager checks this and performs compaction if needed — summarizing older messages, dropping tool results from earlier turns, or truncating large outputs.

This was not in the Chapter 4 loop because at that point you had not built the context management system yet.

### Phase 2: Iteration Limiting

The production loop has a hard cap on iterations. Without it, a model that keeps making tool calls — perhaps in a loop of reading a file, modifying it, running tests, and finding they still fail — could run indefinitely. Twenty-five iterations is generous enough for complex tasks while preventing runaway loops.

### Phase 3: Message Assembly

The `assemble_messages` method does more than just return the conversation history. It prepends the system prompt, injects tool definitions, and may add context-specific instructions (like "you are working in directory /home/user/project"). This assembly step was implicit in Chapter 4 but becomes its own operation in production.

### Phase 4-5: Streamed Provider Calls

In Chapter 4, you called `send_messages` and got back a complete response. In production, you use streaming so the user sees tokens as they arrive. The stream produces chunks that can be text, tool call metadata, or tool call argument deltas. You accumulate these into a complete `AssistantResponse` while simultaneously rendering them.

### Phase 6: Recording the Assistant Message

After the full response is assembled, it goes into the context manager. This must happen *before* processing tool calls, because if a tool call fails and the agent crashes, you want the conversation state to be consistent — the assistant message is recorded even if tools were not executed.

### Phase 8: Safety-Gated Tool Execution

This is the biggest change from Chapter 4. Every tool call goes through the safety layer before execution. The safety layer can return three verdicts:

- **Allowed**: Execute immediately. Common for read-only operations like file reads.
- **Denied**: Block the call and tell the model why. The model receives the denial as a tool result and can adjust its approach.
- **NeedsApproval**: Pause and ask the user. This is the human-in-the-loop checkpoint for sensitive operations like file writes or shell commands.

::: python Coming from Python
If you have used Python frameworks like LangChain, you might have seen similar concepts as "callbacks" or "tool middlewares." The Rust approach encodes the three permission states as an enum, which the compiler forces you to handle exhaustively. In Python, it is easy to forget to handle the denied case. In Rust, the `match` statement will not compile until you cover all variants.
:::

### Phase 9: Context Update and Re-check

After tool execution, the results go back into context. Tool results can be large — a file read might return thousands of lines. The context manager checks limits again and compacts if needed. Then the loop continues.

## Edge Cases in the Production Loop

### The Model Hallucinates a Tool

If the model calls a tool that does not exist in the registry, `tools.execute()` returns an error. The loop converts this to a `ToolResult::error` and feeds it back. The model sees "Tool 'nonexistent_tool' not found" and typically corrects itself on the next iteration.

### A Safety Denial Mid-Turn

When the safety layer denies a tool call, the loop does not abort the entire turn. It sends the denial back as a tool result. The model receives "Permission denied: cannot write to files outside project directory" and adjusts. This is critical — a denial is information, not a crash.

### Context Overflow During a Turn

If tool results push the conversation past the token limit mid-turn, the context manager compacts before the next LLM call. This might mean the model loses details from earlier in the conversation, but the alternative (crashing or sending an oversized request) is worse.

### Provider Failures

If the streaming call to the provider fails (network error, rate limit, invalid request), the error propagates up via `?`. The caller can choose to retry or report the error to the user. The loop itself does not retry — that responsibility belongs to the provider or a retry wrapper around it.

::: wild In the Wild
Claude Code handles the iteration limit with a graduated approach: after a configurable number of iterations, it pauses and asks the user "I've been working for a while. Should I continue?" rather than hard-stopping. This gives the user control over long-running tasks without an arbitrary cutoff. OpenCode tracks token usage per turn and warns the user when a turn is consuming an unusually large portion of the context budget, giving them the option to interrupt before context gets compacted.
:::

## The Turn Outcome

The `TurnOutcome` enum communicates what happened during the turn:

```rust
pub enum TurnOutcome {
    /// The model produced a final text response with no tool calls
    Complete,
    /// Reached the maximum iteration limit
    MaxIterationsReached,
    /// The user requested to stop (Ctrl+C during a tool approval prompt)
    UserInterrupted,
    /// A fatal error occurred that ended the turn
    Error(anyhow::Error),
}
```

The outer REPL loop uses this to decide what to do next. `Complete` and `MaxIterationsReached` both return to the prompt. `UserInterrupted` may ask if the user wants to continue or start a new conversation. `Error` is displayed and then the prompt returns.

## Key Takeaways

- The production agentic loop is the Chapter 4 loop expanded with context management, streaming, safety checks, iteration limits, and structured error handling — every feature from the intervening chapters plugs into this central loop.
- Every tool call passes through the safety layer before execution, with three possible outcomes (allowed, denied, needs approval) that the loop must handle exhaustively — denials are fed back to the model as information, not treated as crashes.
- Context management happens at two points in each iteration: before the LLM call (to ensure the request fits) and after tool results are added (because large tool outputs can push past the limit).
- The loop communicates outcomes through a typed enum (`TurnOutcome`) that the outer REPL uses to decide what happens next — this prevents the common bug of silently dropping errors or not handling edge cases.
- Streaming transforms the loop from request-response into a progressive rendering pipeline where the user sees tokens as they arrive, tool calls as they are invoked, and results as they complete.
