# Codex CLI — Agentic Loop Implementation

## Overview

Codex CLI's agentic loop is implemented in the `codex-core` crate and follows
the **SQ/EQ (Submission Queue / Event Queue)** pattern. The loop orchestrates:
model inference → tool call extraction → approval → sandboxed execution →
result injection → next model turn. It supports streaming (SSE + WebSocket),
parallel tool calls, automatic context compaction, and multi-agent sub-spawning.

## High-Level Flow

```
User Input (Op::UserTurn)
       │
       ▼
┌─────────────────────┐
│  Build Prompt        │ ◄── ContextManager.for_prompt()
│  (history + tools    │     + ToolRouter.model_visible_specs()
│   + system message)  │     + developer instructions
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│  Call Model API      │ ◄── ResponsesApiRequest → SSE stream
│  (streaming)         │     or WebSocket connection
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│  Parse Response      │ ◄── ResponseItem variants:
│  Items               │     Message, LocalShellCall, FunctionCall,
│                      │     CustomToolCall, WebSearchCall, Reasoning
└──────────┬──────────┘
           │
     ┌─────┴─────┐
     │            │
  Text         Tool Calls
     │            │
     ▼            ▼
  Emit        ┌─────────────────────┐
  AgentMsg    │  ToolRouter         │ ◄── build_tool_call()
              │  (dispatch)         │
              └──────────┬──────────┘
                         │
                         ▼
              ┌─────────────────────┐
              │  ToolOrchestrator   │ ◄── approval → sandbox → execute
              │  (approve + run)    │
              └──────────┬──────────┘
                         │
                         ▼
              ┌─────────────────────┐
              │  Inject Result      │ ◄── FunctionCallOutput / LocalShellOutput
              │  into Context       │
              └──────────┬──────────┘
                         │
                         ▼
              ┌─────────────────────┐
              │  Check Compaction   │ ◄── if tokens > auto_compact_limit
              │  Threshold          │     → trigger remote compaction
              └──────────┬──────────┘
                         │
                         ▼
                  Loop back to
                  "Call Model API"
                  (next turn)
```

## The Session Loop

The core loop runs as a Tokio task spawned by `Codex::spawn()`. It processes
submissions from the SQ channel and emits events to the EQ channel:

```rust
// Simplified from codex-core/src/codex.rs
async fn session_loop(
    mut rx_sub: Receiver<Submission>,
    tx_event: Sender<Event>,
    session: Arc<Session>,
) {
    while let Some(submission) = rx_sub.recv().await {
        match submission.op {
            Op::UserTurn { items, model, effort, .. } => {
                tx_event.send(EventMsg::TurnStarted(..)).await;

                // Build the model request
                let prompt = session.context_manager.for_prompt();
                let tools = session.tool_router.model_visible_specs();
                let request = build_responses_request(prompt, tools, model, effort);

                // Stream model response
                let stream = session.model_client.stream_responses(request).await;

                // Process streamed items
                loop {
                    match stream.next().await {
                        Some(ResponseEvent::ItemCreated(item)) => {
                            process_response_item(item, &session, &tx_event).await;
                        }
                        Some(ResponseEvent::Completed(usage)) => {
                            session.context_manager.update_token_usage(usage);
                            break;
                        }
                        None => break,
                    }
                }

                // Check if we need another model turn (tool calls pending)
                if session.has_pending_tool_results() {
                    // Continue the loop — submit results and call model again
                    continue;
                }

                tx_event.send(EventMsg::TurnComplete(..)).await;
            }
            Op::ExecApproval { id, decision } => {
                session.resolve_approval(id, decision).await;
            }
            Op::Compact { .. } => {
                session.run_compaction().await;
            }
            Op::Shutdown {} => break,
            // ... other ops
        }
    }
}
```

## Response Item Processing

When the model returns items, each is classified and dispatched:

```rust
async fn process_response_item(
    item: ResponseItem,
    session: &Session,
    tx_event: &Sender<Event>,
) {
    // Record in context manager
    session.context_manager.record_items(vec![item.clone()]);

    match &item {
        // Text response — emit to UI
        ResponseItem::Message { content, .. } => {
            tx_event.send(EventMsg::AgentMessage(..)).await;
        }

        // Reasoning summary
        ResponseItem::Reasoning { summary, .. } => {
            tx_event.send(EventMsg::Reasoning(..)).await;
        }

        // Shell command — route through tool system
        ResponseItem::LocalShellCall { call_id, command, .. } => {
            let tool_call = tool_router.build_tool_call(&item);
            let result = tool_orchestrator.run(tool_call, ..).await;
            // Inject result into context
            session.context_manager.record_items(vec![result.into()]);
        }

        // Function call (MCP or built-in)
        ResponseItem::FunctionCall { name, call_id, arguments, .. } => {
            let tool_call = tool_router.build_tool_call(&item);
            let result = tool_orchestrator.run(tool_call, ..).await;
            session.context_manager.record_items(vec![result.into()]);
        }

        // Web search
        ResponseItem::WebSearchCall { .. } => {
            // Handled by web search tool
        }

        // Context compaction marker
        ResponseItem::Compaction { .. } => {
            tx_event.send(EventMsg::ContextCompacted(..)).await;
        }

        _ => {}
    }
}
```

## Tool Router — Dispatch Logic

The `ToolRouter` maps model-generated items to executable tool calls:

```rust
// codex-core/src/tools/router.rs
pub struct ToolCall {
    pub tool_name: String,
    pub tool_namespace: Option<String>,
    pub call_id: String,
    pub payload: ToolPayload,
}

pub struct ToolRouter {
    registry: ToolRegistry,
    specs: Vec<ConfiguredToolSpec>,
    model_visible_specs: Vec<ToolSpec>,
}

impl ToolRouter {
    pub fn build_tool_call(&self, item: &ResponseItem) -> ToolCall {
        match item {
            ResponseItem::FunctionCall { name, .. } => {
                // Check MCP tools first, then built-in functions
                if let Some(mcp_tool) = self.registry.find_mcp(name) {
                    // Route to MCP server
                } else {
                    // Route to built-in function handler
                }
            }
            ResponseItem::LocalShellCall { .. } => {
                // Route to local_shell tool
            }
            ResponseItem::CustomToolCall { .. } => {
                // Route to custom tool dispatcher
            }
            _ => unreachable!(),
        }
    }
}
```

## Tool Orchestrator — Approval → Sandbox → Execute

The orchestrator drives the critical path for every tool execution:

```rust
// codex-core/src/tools/orchestrator.rs
pub(crate) struct ToolOrchestrator {
    sandbox: SandboxManager,
}

impl ToolOrchestrator {
    pub async fn run<Rq, Out, T>(
        &mut self,
        tool: &T,
        req: Rq,
        tool_ctx: ToolContext,
        turn_ctx: TurnContext,
        approval_policy: AskForApproval,
    ) -> ToolResult<Out>
    where
        T: ToolRuntime<Rq, Out>,
    {
        // 1. Check approval requirement
        let requirement = tool.approval_requirement(&req, &tool_ctx);

        match requirement {
            ExecApprovalRequirement::Skip => {
                // Auto-approved — proceed directly
            }
            ExecApprovalRequirement::Forbidden { reason } => {
                return Err(ToolError::Rejected(reason));
            }
            ExecApprovalRequirement::NeedsApproval { .. } => {
                // Emit ExecApprovalRequest event to UI
                // Block until user responds via Op::ExecApproval
                let decision = tool.start_approval_async(req, ..).await;
                match decision {
                    ReviewDecision::Denied => return Err(ToolError::Rejected(..)),
                    ReviewDecision::Abort => return Err(ToolError::Aborted),
                    ReviewDecision::Approved => { /* continue */ }
                    ReviewDecision::ApprovedExecpolicyAmendment(amendment) => {
                        // Update execpolicy rules for future commands
                    }
                }
            }
        }

        // 2. Select sandbox level
        let initial_sandbox = self.sandbox.select_initial(
            &tool_ctx.sandbox_policy,
            tool.requested_permissions(&req),
        );

        // 3. First execution attempt
        let result = Self::run_attempt(tool, &req, initial_sandbox, ..).await;

        // 4. Handle sandbox denial — retry with escalation
        match result {
            Err(SandboxErr::Denied { .. }) if tool.escalate_on_failure() => {
                // Ask user for approval to retry without sandbox
                let escalated = SandboxAttempt { sandbox: None, .. };
                Self::run_attempt(tool, &req, escalated, ..).await
            }
            other => other,
        }
    }
}
```

## Streaming Model Communication

### SSE (Server-Sent Events) — Primary

```rust
// codex-api/src/lib.rs
pub struct ResponsesApiRequest {
    pub model: String,
    pub input: Vec<ResponseInputItem>,
    pub tools: Vec<ToolSpec>,
    pub instructions: Option<String>,
    pub reasoning: Option<ReasoningConfig>,
    pub stream: bool,  // always true
    pub previous_response_id: Option<String>,
    // ...
}

// Returns a stream of:
pub enum ResponseEvent {
    ResponseCreated { id: String },
    ItemCreated(ResponseItem),
    ItemUpdated(ResponseItem),
    ItemCompleted(ResponseItem),
    ResponseCompleted { usage: TokenUsage },
    Error { message: String },
}
```

### WebSocket — For Realtime

The model client supports WebSocket connections for realtime conversation
mode. WebSocket events are mapped to the same `ResponseEvent` enum for
unified processing by the session loop.

## Parallel Tool Calls

When the model emits multiple tool calls in a single response, Codex processes
them concurrently:

```rust
// Multiple tool calls from model response
let tool_calls: Vec<ToolCall> = response_items
    .iter()
    .filter_map(|item| tool_router.build_tool_call(item))
    .collect();

// Execute in parallel with join_all
let results = futures::future::join_all(
    tool_calls.into_iter().map(|tc| {
        tool_orchestrator.run(tc, ..)
    })
).await;

// Inject all results into context
for result in results {
    context_manager.record_items(vec![result.into()]);
}
```

Whether parallel tool calls are enabled depends on the model's
`supports_parallel_tool_calls` flag in `ModelInfo`.

## Auto-Compaction

When estimated token usage approaches the context window limit, automatic
compaction is triggered:

```rust
// codex-core/src/context_manager/compact_remote.rs
async fn run_remote_compact_task_inner_impl(
    session: &Session,
    turn_context: TurnContext,
) {
    // 1. Clone current history
    let history = session.context_manager.items.clone();

    // 2. Trim function call items that overflow context window
    let trimmed = trim_function_call_history_to_fit_context_window(history);

    // 3. Build compaction request
    let compaction_input = CompactionInput {
        model: turn_context.model.clone(),
        items: trimmed,
        instructions: turn_context.instructions.clone(),
    };

    // 4. Call model's compaction endpoint
    let compacted = model_client.compact_conversation_history(compaction_input).await;

    // 5. Replace history (preserving GhostSnapshot items for undo)
    session.context_manager.replace_with_compacted(compacted);

    // 6. Emit ContextCompacted event
    tx_event.send(EventMsg::ContextCompacted(..)).await;
}
```

Compaction threshold: `auto_compact_token_limit` defaults to **90% of the
model's context window** (which itself defaults to 272,000 tokens).

## Turn Lifecycle Events

A complete turn emits this event sequence:

```
TurnStarted
  ├── AgentMessage (streaming text chunks)
  ├── Reasoning (if reasoning model)
  ├── ExecApprovalRequest (if tool needs approval)
  │   └── [blocked until Op::ExecApproval received]
  ├── AgentMessage (tool result interpretation)
  ├── TokenUsage (per API call)
  ├── ContextCompacted (if auto-compaction triggered)
  └── ...repeats for multi-turn tool use...
TurnComplete (or TurnAborted)
```

## Interrupt Handling

Users can interrupt a running turn via `Op::Interrupt`:

```rust
Op::Interrupt => {
    // Cancel the current model stream
    session.cancel_active_stream().await;
    // Abort any pending tool executions
    session.abort_pending_tools().await;
    // Emit TurnAborted event
    tx_event.send(EventMsg::TurnAborted(..)).await;
}
```

Ctrl+C in the TUI triggers `Op::Interrupt`. Double Ctrl+C exits the program.

## Undo / Thread Rollback

Codex supports undoing the last turn(s) via `Op::Undo`:

```rust
Op::Undo { num_turns } => {
    // Remove last N user+assistant turn pairs from context
    context_manager.drop_last_n_user_turns(num_turns);
    // GhostSnapshot items are preserved across undo for redo support
}
```

`Op::ThreadRollback` provides deeper rollback by replaying from a persisted
rollout file up to a specified point.

## Sub-Agent Spawning (Multi-Agent)

When the model calls the `codex` tool or the user uses `/fork`, a sub-agent
is spawned:

```rust
// agent/control.rs
pub async fn spawn_agent(
    &self,
    prompt: String,
    role: Option<String>,
    model: Option<String>,
    cwd: Option<PathBuf>,
) -> Result<AgentHandle, SpawnError> {
    // 1. Reserve a spawn slot (CAS on atomic counter)
    let reservation = self.state.reserve_spawn_slot()?;

    // 2. Create config for sub-agent (apply role overrides)
    let mut sub_config = self.base_config.clone();
    if let Some(role) = role {
        apply_role_to_config(&mut sub_config, Some(&role)).await?;
    }

    // 3. Spawn new CodexThread
    let thread = ThreadManager::create_thread(sub_config, prompt, ..).await?;

    // 4. Return handle for monitoring
    Ok(AgentHandle { thread_id, reservation })
}
```

Sub-agents have their own context, model calls, and tool execution — but share
the parent's sandbox manager and approval policy (unless overridden by role).

## Non-Interactive Execution (`codex exec`)

The exec crate (`codex-rs/exec/`) wraps the same core loop with a simpler
event processor:

```rust
// exec/src/lib.rs
pub async fn run_exec(prompt: String, config: Config) -> ExitCode {
    let codex = Codex::spawn(config, ..).await?;
    codex.submit(Op::UserTurn { items: vec![prompt], .. }).await;

    loop {
        match codex.next_event().await {
            EventMsg::TurnComplete(_) => break ExitCode::SUCCESS,
            EventMsg::TurnAborted(_) => break ExitCode::FAILURE,
            EventMsg::AgentMessage(msg) => print_to_stdout(msg),
            EventMsg::ExecApprovalRequest(req) => {
                // Auto-approve based on --ask-for-approval flag
                codex.submit(Op::ExecApproval { decision: auto_decision }).await;
            }
            _ => {} // Other events handled silently
        }
    }
}
```

Output modes:
- **Human-readable** (default): Formatted text with syntax highlighting
- **JSONL** (`--json`): Structured events for machine consumption
- **Ephemeral** (`--ephemeral`): No rollout persistence

## Event Processing in Exec Mode

```rust
// exec/src/exec_events.rs
pub enum ThreadEvent {
    ThreadStarted,
    TurnStarted,
    TurnCompleted,
    TurnFailed,
    ItemStarted,
    ItemUpdated,
    ItemCompleted,
    Error,
}

pub enum ThreadItemDetails {
    AgentMessage,        // text response
    Reasoning,           // reasoning summary
    CommandExecution,    // shell command
    FileChange,          // file patch
    McpToolCall,         // MCP tool invocation
    CollabToolCall,      // sub-agent tool call
    WebSearch,           // web search
    TodoList,            // agent's internal to-do list
    Error,               // non-fatal error
}
```

## Resume Flow

`codex resume` replays a persisted rollout to reconstruct session state:

1. Load JSONL rollout from `~/.codex/sessions/<session_id>/`
2. Deserialize each line into `ResponseItem` or `Op`
3. Replay through `ContextManager` to rebuild history
4. Recursively resume sub-agent rollouts
5. Accept new user input on the reconstructed session

This enables seamless continuation across terminal restarts.