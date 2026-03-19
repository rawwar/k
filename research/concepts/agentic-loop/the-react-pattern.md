---
title: The ReAct Pattern
status: complete
---

# The ReAct Pattern

The ReAct pattern — **Re**asoning + **Act**ing — is the foundational architecture of every coding agent studied in this research. Not "one of the foundations." The foundation. Every agent, from **mini-SWE-agent**'s 100-line Python script to **Codex CLI**'s thousands of lines of Rust, implements the same cycle: think about what to do, do it, observe the result, repeat. This document traces the pattern from its academic origins through to production implementations, with full pseudocode in four languages and detailed comparison across 17+ real-world agents.

---

## The Original Paper

**ReAct: Synergizing Reasoning and Acting in Language Models**
Shunyu Yao, Jeffrey Zhao, Dian Yu, Nan Du, Izhak Shafran, Karthik Narasimhan, Yuan Cao
arXiv:2210.03629 — Published at ICLR 2023

### The Core Insight

Before ReAct, there were two separate lines of work:

1. **Chain-of-thought (CoT) reasoning**: Let the model "think step by step" to solve problems. Good at reasoning, but hallucination-prone because the model has no way to verify its assumptions against reality.
2. **Action-only agents**: Let the model take actions in an environment. Good at interacting with the world, but lacking the internal reasoning to form plans, handle exceptions, or recover from errors.

ReAct's insight: **interleave them**. Let the model alternate between generating reasoning traces (Thoughts) and task-specific actions. Reasoning traces help the model induce, track, and update action plans, handle exceptions, and adjust its approach. Actions allow interfacing with external sources — knowledge bases, file systems, APIs, environments — for additional information that the model cannot hallucinate.

```
┌─────────────────────────────────────────────────────────────────────┐
│                     THE KEY DIAGRAM FROM THE PAPER                  │
│                                                                     │
│   Chain-of-Thought          Action-Only             ReAct           │
│   ─────────────────   ─────────────────────   ─────────────────── │
│   Thought 1            Action 1                Thought 1            │
│   Thought 2            Observation 1           Action 1             │
│   Thought 3            Action 2                Observation 1        │
│   ... (no grounding)   Observation 2           Thought 2            │
│   Answer               ... (no reasoning)      Action 2             │
│                        Answer                  Observation 2        │
│                                                Thought 3            │
│                                                Answer               │
│                                                                     │
│   ✗ Hallucination      ✗ No error recovery     ✓ Grounded          │
│   ✗ Error propagation  ✗ No planning           ✓ Adaptive           │
│   ✓ Internal reasoning ✓ Grounded              ✓ Both               │
└─────────────────────────────────────────────────────────────────────┘
```

### Benchmark Results

The paper tested ReAct on four benchmarks spanning knowledge-intensive reasoning and interactive decision-making:

| Benchmark | Domain | ReAct vs. Baselines | Key Finding |
|-----------|--------|---------------------|-------------|
| **HotpotQA** | Multi-hop QA | Competitive with CoT | Dramatically reduced hallucination rate |
| **Fever** | Fact verification | Outperformed Act-only | Reasoning traces helped identify when to stop searching |
| **ALFWorld** | Text game (household tasks) | **+34%** over imitation learning baseline | Reasoning enabled generalization to unseen tasks |
| **WebShop** | Web navigation + shopping | **+10%** over imitation learning baseline | Thoughts helped track progress toward goal |

### Why It Works: The Feedback Loop

The power of ReAct comes from closing the feedback loop between the model's internal world model and the external environment:

```
Model's internal state          External environment
┌──────────────────┐            ┌──────────────────┐
│  Current beliefs │◄───────────│  Observation     │
│  about the world │  feedback  │  (actual state)  │
├──────────────────┤            └──────────▲───────┘
│  Updated plan    │                       │
│  based on new    │─────────────────────►│
│  information     │     action           │
└──────────────────┘                      │
                                   ┌──────┴───────┐
                                   │  Environment │
                                   │  executes    │
                                   │  action      │
                                   └──────────────┘
```

- **Reasoning provides grounding**: The model articulates *why* it's taking an action, making it less likely to take random or hallucinated actions.
- **Actions provide information**: The model can check its assumptions against reality rather than reasoning from potentially incorrect premises.
- **Together they close the loop**: The model reasons → acts → observes → reasons better → acts better. This is the fundamental feedback cycle that makes agents work.

---

## The Thought → Action → Observation Cycle

The ReAct cycle has three atomic components. Every iteration of the loop produces exactly these three artifacts:

### Thought (Reasoning Trace)

The model's internal deliberation. In the original paper, this was a free-form text prefix like `Thought: I need to search for the population of Paris`. In modern implementations, this maps to:

- **Extended thinking / reasoning tokens** (Claude, OpenAI o-series): The model's chain-of-thought is computed in a separate "thinking" block before the visible response.
- **The assistant message text** before tool calls: When a model writes "Let me check the test file first..." before emitting a tool call, that text IS the thought.
- **System-prompt-induced THOUGHT prefixes**: mini-SWE-agent's system prompt instructs the model to write `THOUGHT:` before each response, making reasoning traces explicit and parseable.

The purpose of the Thought is threefold:
1. **Plan**: "I need to find the file that contains this function"
2. **Analyze**: "The error message suggests a null pointer, so the variable must not be initialized"
3. **Recover**: "That approach didn't work because the file is read-only. Let me try a different path"

### Action (Executable Operation)

The model's chosen interaction with the environment. In the original paper, actions were text strings like `Search[query]` or `Lookup[term]`. In modern coding agents, actions are:

- **Tool calls**: Structured function invocations — `read_file(path="src/main.py")`, `bash(command="npm test")`
- **Code execution**: Smolagents' code-as-action approach — the model writes executable Python code directly
- **API calls**: Direct invocations of external services
- **Shell commands**: The most common action in coding agents — nearly every agent wraps `subprocess.run()`

### Observation (Environment Feedback)

The result of executing the action, fed back to the model as new context. In modern LLM APIs, observations become `tool` role messages:

- **File contents**: The text of a file after a read operation
- **Command output**: stdout/stderr from a shell command
- **Error messages**: Stack traces, compiler errors, test failures
- **Search results**: Matching files, lines, or code symbols
- **Execution results**: Return values, screenshots, API responses

### How the Cycle Maps to Modern LLM APIs

The ReAct cycle maps directly to the message format used by every major LLM provider:

```
Original ReAct Paper              Modern LLM API (OpenAI/Anthropic/Google)
─────────────────────────         ──────────────────────────────────────────
                                  messages = [
System context                        {role: "system", content: "You are..."},
Task description                      {role: "user",   content: "Fix the bug..."},
                                  ]

Thought 1: I should look at...    → response = llm.chat(messages)
Action 1: Search[bug location]    → response.tool_calls = [{name: "grep", args: {...}}]

                                  messages.append(response)  // assistant message WITH tool_calls

Observation 1: Found in main.py   → messages.append({role: "tool", content: "main.py:42: ..."})

Thought 2: The bug is on line 42  → response = llm.chat(messages)
Action 2: Edit[main.py, ...]      → response.tool_calls = [{name: "edit", args: {...}}]
...                                ...
```

The API literally formalized the ReAct pattern. What Yao et al. described as text prefixes (`Thought:`, `Action:`, `Observation:`) became structured message roles (`assistant`, `tool_calls`, `tool`).

---

## Pseudocode Implementations

The agentic loop is language-agnostic. Below are complete, idiomatic implementations in four languages — each capturing the same ReAct cycle with language-specific patterns.

### Python — The Reference Implementation

```python
from typing import Any

def run_agent(llm, tools: dict[str, callable], system_prompt: str, task: str) -> str:
    """The ReAct loop in ~20 lines of Python."""
    messages = [
        {"role": "system", "content": system_prompt},
        {"role": "user", "content": task},
    ]

    while True:
        # THOUGHT + ACTION: Model reasons and optionally emits tool calls
        response = llm.chat(messages, tools=list(tools.keys()))
        messages.append(response.to_message())

        # Check for termination: no tool calls means the model is done
        if not response.tool_calls:
            return response.content  # Final answer

        # OBSERVATION: Execute each tool call, feed results back
        for call in response.tool_calls:
            try:
                result = tools[call.name](**call.arguments)
            except Exception as e:
                result = f"Error: {type(e).__name__}: {e}"

            messages.append({
                "role": "tool",
                "tool_call_id": call.id,
                "content": str(result),
            })
```

This is essentially what **mini-SWE-agent**, **Pi**, and **Aider** implement. The differences are in error handling, cost tracking, and context management — not in the loop structure itself.

### Go — With Context Cancellation and Streaming

```go
package agent

import (
    "context"
    "fmt"
)

// ReactLoop runs the core Thought→Action→Observation cycle.
// Cancellation propagates through ctx — user presses Ctrl+C,
// and every layer (stream, tool execution, loop) checks ctx.Done().
func ReactLoop(ctx context.Context, llm LLM, tools ToolRegistry, task string) (string, error) {
    messages := []Message{
        {Role: "system", Content: systemPrompt},
        {Role: "user", Content: task},
    }

    for {
        select {
        case <-ctx.Done():
            return "", ctx.Err()
        default:
        }

        // THOUGHT + ACTION: Stream the response token by token
        stream, err := llm.ChatStream(ctx, messages)
        if err != nil {
            return "", fmt.Errorf("llm.ChatStream: %w", err)
        }

        var response Message
        for event := range stream.Events() {
            switch e := event.(type) {
            case TextDelta:
                response.Content += e.Text
            case ToolCallDelta:
                response.ToolCalls = appendToolDelta(response.ToolCalls, e)
            case Done:
                response.Usage = e.Usage
            }
        }
        if err := stream.Err(); err != nil {
            return "", fmt.Errorf("stream: %w", err)
        }
        messages = append(messages, response)

        // Termination: no tool calls → model is done
        if len(response.ToolCalls) == 0 {
            return response.Content, nil
        }

        // OBSERVATION: Execute tools (parallel when safe)
        results := make(chan ToolResult, len(response.ToolCalls))
        for _, call := range response.ToolCalls {
            call := call
            go func() {
                result, execErr := tools.Execute(ctx, call.Name, call.Arguments)
                if execErr != nil {
                    result = fmt.Sprintf("Error: %v", execErr)
                }
                results <- ToolResult{CallID: call.ID, Content: result}
            }()
        }

        for range response.ToolCalls {
            r := <-results
            messages = append(messages, Message{
                Role:       "tool",
                ToolCallID: r.CallID,
                Content:    r.Content,
            })
        }
    }
}
```

This mirrors the architecture of **OpenCode** (Go), which uses `context.Context` for cancellation and channels for streaming event propagation. The key Go idiom: `select` on `ctx.Done()` at the top of the loop enables cooperative cancellation.

### TypeScript — Async/Await with Parallel Tool Execution

```typescript
interface Message {
  role: "system" | "user" | "assistant" | "tool";
  content: string;
  tool_calls?: ToolCall[];
  tool_call_id?: string;
}

async function reactLoop(
  llm: LLMClient,
  tools: Map<string, ToolFn>,
  systemPrompt: string,
  task: string,
  signal?: AbortSignal
): Promise<string> {
  const messages: Message[] = [
    { role: "system", content: systemPrompt },
    { role: "user", content: task },
  ];

  while (!signal?.aborted) {
    // THOUGHT + ACTION
    const response = await llm.chat(messages, {
      tools: [...tools.keys()],
      signal,
    });
    messages.push(response.toMessage());

    // Termination check
    const toolCalls = response.toolCalls ?? [];
    if (toolCalls.length === 0) {
      return response.content; // Done — return final answer
    }

    // OBSERVATION: Execute all tool calls in parallel
    const results = await Promise.all(
      toolCalls.map(async (call) => {
        const fn = tools.get(call.name);
        if (!fn) return { id: call.id, content: `Error: unknown tool '${call.name}'` };
        try {
          const result = await fn(call.arguments);
          return { id: call.id, content: String(result) };
        } catch (err) {
          return { id: call.id, content: `Error: ${err}` };
        }
      })
    );

    for (const result of results) {
      messages.push({
        role: "tool",
        tool_call_id: result.id,
        content: result.content,
      });
    }
  }

  throw new DOMException("Agent aborted", "AbortError");
}
```

This reflects the pattern used by **Gemini CLI** and **Warp**, both TypeScript-based agents. The `AbortSignal` pattern provides cancellation semantics similar to Go's `context.Context`. `Promise.all` parallelizes independent tool calls.

### Rust — Async/Tokio with Result Types

```rust
use anyhow::{Context, Result};
use tokio::select;
use tokio_util::sync::CancellationToken;

/// The ReAct loop in Rust. Cancellation via CancellationToken,
/// error propagation via Result, async execution via Tokio.
async fn react_loop(
    llm: &dyn LlmClient,
    tools: &ToolRegistry,
    system_prompt: &str,
    task: &str,
    cancel: CancellationToken,
) -> Result<String> {
    let mut messages = vec![
        Message::system(system_prompt),
        Message::user(task),
    ];

    loop {
        if cancel.is_cancelled() {
            anyhow::bail!("Agent cancelled");
        }

        // THOUGHT + ACTION: Query the model
        let response = select! {
            _ = cancel.cancelled() => anyhow::bail!("Agent cancelled"),
            resp = llm.chat(&messages) => resp.context("LLM query failed")?,
        };
        messages.push(response.to_message());

        // Termination: no tool calls → done
        let tool_calls = response.tool_calls();
        if tool_calls.is_empty() {
            return Ok(response.content().to_string());
        }

        // OBSERVATION: Execute tools concurrently with join_all
        let futures: Vec<_> = tool_calls
            .iter()
            .map(|call| {
                let tools = &tools;
                let cancel = cancel.clone();
                async move {
                    let result = select! {
                        _ = cancel.cancelled() => Err(anyhow::anyhow!("Cancelled")),
                        r = tools.execute(&call.name, &call.arguments) => r,
                    };
                    ToolResult {
                        call_id: call.id.clone(),
                        content: match result {
                            Ok(output) => output,
                            Err(e) => format!("Error: {e:#}"),
                        },
                    }
                }
            })
            .collect();

        let results = futures::future::join_all(futures).await;
        for result in results {
            messages.push(Message::tool(&result.call_id, &result.content));
        }
    }
}
```

This mirrors **Codex CLI** and **Goose**, both Rust-based agents. The key Rust idioms: `Result<T>` for error propagation, `CancellationToken` for cooperative cancellation (replacing Go's `context.Context`), `tokio::select!` for racing cancellation against async operations, and `join_all` for parallel tool execution.

---

## Why ReAct Dominates Coding Agents

Every coding agent studied in this research — all 17+ agents across five languages — implements ReAct at its core. This is not coincidence. ReAct dominates because it maps directly to the constraints of the problem:

### 1. LLMs Are Stateless Functions

An LLM takes a sequence of messages and produces a response. It has no persistent memory, no ability to maintain state between calls. The ReAct loop provides the statefulness: the `messages` array IS the state, and each iteration appends to it.

### 2. Tool Use Is the Only Bridge

Language models cannot read files, run commands, search codebases, or modify code. They can only produce text (or structured outputs). Tool calls are the sole mechanism by which a model can affect the external world. ReAct's Action step is this bridge.

### 3. Observations Close the Feedback Loop

Without feeding results back to the model, it's reasoning in a vacuum. A model that edits a file but never sees the compiler output doesn't know if its edit was correct. Observations — the `tool` messages — are what make the agent adaptive rather than open-loop.

### 4. Termination Is Natural

When the model has gathered enough information, made its changes, and verified the results, it simply responds with text and no tool calls. No explicit "STOP" token is needed, no state machine transition, no external signal. The absence of tool calls IS the termination signal. This is elegant and robust.

### 5. The Pattern Maps to API Design

Modern LLM APIs (OpenAI, Anthropic, Google) were designed with this pattern in mind. The `tool_calls` field on assistant messages, the `tool` role for observations, the structured function definitions — these are ReAct formalized into an API spec. Using ReAct isn't a design choice; it's the path of least resistance.

### The Universal Pseudocode

Every agent we studied reduces to this:

```
messages = [system_prompt, user_task]

while true:
    response = llm.generate(messages)
    messages.append(response)

    tool_calls = parse_tool_calls(response)
    if tool_calls is empty:
        break                          # model is done

    for call in tool_calls:
        result = execute(call)
        messages.append(observation(result))
```

The differences between agents — and they are substantial — are all **layered on top** of this core: streaming, state machines, message queues, multi-agent orchestration, context compaction, permission gates, verification enforcement. But peel back every layer and you find this loop.

---

## Variations: ReAct vs. Function Calling vs. Structured Output

The original ReAct pattern and modern implementations differ in how actions are represented. Understanding this evolution is important because it explains why different agents make different choices.

### 1. Text-Based ReAct (Original Paper, 2022)

The model generates actions as plain text, parsed by the harness:

```
Thought: I need to search for information about the Eiffel Tower's height.
Action: Search[Eiffel Tower height]
Observation: The Eiffel Tower is 330 metres (1,083 ft) tall...
Thought: I found the answer.
Action: Finish[330 metres]
```

**Pros**: Maximum flexibility — the model can express any action format. Easy to prototype.
**Cons**: Parsing is fragile. The model might write `Action: search(Eiffel Tower height)` or `Action: Search["Eiffel Tower height"]` — subtle format variations break the parser. No schema validation.

**Who still uses this**: **mini-SWE-agent** uses a variant — actions are bash commands embedded in markdown code blocks, parsed by the harness. The simplicity is deliberate: it keeps the agent minimal and the trajectory human-readable.

### 2. Function Calling (OpenAI 2023, Anthropic 2024, Google 2024)

The API provider handles the structured output:

```json
{
  "role": "assistant",
  "content": "Let me search for that information.",
  "tool_calls": [{
    "id": "call_abc123",
    "type": "function",
    "function": {
      "name": "search",
      "arguments": "{\"query\": \"Eiffel Tower height\"}"
    }
  }]
}
```

**Pros**: Reliable parsing (the API guarantees valid JSON). Schema validation at the API level. The model is trained specifically to emit tool calls. IDs enable matching responses to calls.
**Cons**: Constrained to the provider's tool format. Can't express novel action types without defining them upfront. Slight overhead from schema enforcement.

**Who uses this**: The majority of agents — **Claude Code**, **Codex CLI**, **OpenCode**, **Goose**, **Gemini CLI**, **Warp**, **Droid**, **ForgeCode**, **Ante**, **OpenHands**, **Junie**, **Aider** (with function-calling-capable models), **Sage**, **Capy**.

### 3. Code-as-Action (Smolagents / HuggingFace, 2024)

Instead of structured tool calls, the model writes executable Python code:

```python
# Thought: I need to search for the Eiffel Tower's height
result = search(query="Eiffel Tower height")
print(result)
# Thought: Parse the result
height = result.split("tall")[0].split()[-2]
final_answer(height)
```

**Pros**: **30% fewer steps** than JSON tool calling (Smolagents paper). The model can compose tools naturally — variables, loops, conditionals, error handling — within a single action. Dramatically more expressive.
**Cons**: Security concerns (executing arbitrary code). Harder to sandbox. The model can write buggy code that crashes the agent. Requires a code execution runtime.

**Who uses this**: **Smolagents**, and conceptually **TongAgents** (which generates tool-using code).

### The Evolution

```
2022: Text-based ReAct ─────► Flexible but fragile
          │
          ▼
2023: Function Calling ─────► Structured but constrained
          │                     (APIs formalized the pattern)
          ▼
2024: Code-as-Action ───────► Expressive but risky
                                (full programming language as action space)
```

Each step traded flexibility for reliability or vice versa. Function calling is the dominant equilibrium — reliable enough for production, expressive enough for most tasks. Code-as-action is gaining traction where step efficiency matters.

---

## mini-SWE-agent: The Purest ReAct Implementation

**mini-SWE-agent** is the purest distillation of the ReAct pattern into a working coding agent. The entire agent is ~100 lines of Python. It scores **65% on SWE-bench Verified** — competitive with agents that are 10–100× more complex.

### Why It Matters

mini-SWE-agent proves that the ReAct loop, with nothing added, is sufficient for competitive coding performance. Every feature that other agents add — planning, reflection, retrieval, multi-agent orchestration — is an optimization, not a requirement.

### The Complete Architecture

```
┌─────────────────────────────────────────────────┐
│                  DefaultAgent                    │
│                                                  │
│  State: messages[], cost, n_calls               │
│                                                  │
│  run(task)                                       │
│    ├── Initialize [system_msg, user_msg]         │
│    └── while True:                               │
│          ├── step()                              │
│          │    ├── query()      ← call the LLM   │
│          │    └── execute_actions() ← run tools  │
│          └── if exit: break                      │
│                                                  │
│  Dependencies: Model, Environment               │
│  Config: system_template, instance_template,     │
│          step_limit, cost_limit, output_path     │
└─────────────────────────────────────────────────┘
```

### The Two-Line Heart

```python
def step(self):
    """Query the LM, execute actions."""
    return self.execute_actions(self.query())
```

That's it. The entire step method. Two function calls composed: ask the model what to do (`query`), then do it (`execute_actions`). This is the ReAct cycle in its most compressed form:

- `query()` = Thought + Action (the model reasons and emits actions)
- `execute_actions()` = Observation (the environment runs the actions and feeds results back)

### The Annotated Components

**AgentConfig** — Only 5 fields:

```python
class AgentConfig(BaseModel):
    system_template: str         # Jinja2 template for system prompt
    instance_template: str       # Jinja2 template for task description
    step_limit: int = 0          # Max steps (0 = unlimited)
    cost_limit: float = 3.0      # Max cost in dollars
    output_path: Path | None     # Where to save trajectory
```

Compare this to SWE-agent's dozens of config options for tools, history processing, guardrails, etc.

**DefaultAgent.__init__** — A model and an environment, that's it:

```python
def __init__(self, model: Model, env: Environment, *, config_class=AgentConfig, **kwargs):
    self.config = config_class(**kwargs)
    self.messages: list[dict] = []
    self.model = model
    self.env = env
    self.cost = 0.0
    self.n_calls = 0
```

No tool registry, no memory store, no planner. The entire mutable state is `self.messages`, `self.cost`, and `self.n_calls`.

**run()** — The outer loop with exception-based control flow:

```python
def run(self, task: str = "", **kwargs) -> dict:
    self.messages = []
    self.add_messages(
        self.model.format_message(role="system", content=self._render_template(...)),
        self.model.format_message(role="user", content=self._render_template(...)),
    )
    while True:
        try:
            self.step()
        except InterruptAgentFlow as e:
            self.add_messages(*e.messages)
        finally:
            self.save(self.config.output_path)
        if self.messages[-1].get("role") == "exit":
            break
    return self.messages[-1].get("extra", {})
```

**query()** — Check limits, call the model, track cost:

```python
def query(self) -> dict:
    if 0 < self.config.step_limit <= self.n_calls or 0 < self.config.cost_limit <= self.cost:
        raise LimitsExceeded(...)
    self.n_calls += 1
    message = self.model.query(self.messages)   # Passes ENTIRE history — no truncation
    self.cost += message.get("extra", {}).get("cost", 0.0)
    self.add_messages(message)
    return message
```

**execute_actions()** — Extract actions, run them, format observations:

```python
def execute_actions(self, message: dict) -> list[dict]:
    outputs = [self.env.execute(action) for action in message.get("extra", {}).get("actions", [])]
    return self.add_messages(*self.model.format_observation_messages(message, outputs, ...))
```

**add_messages()** — Just `list.extend`:

```python
def add_messages(self, *messages: dict) -> list[dict]:
    self.messages.extend(messages)
    return list(messages)
```

No filtering, no summarization, no priority queue. Messages go in, they never come out.

### What's Deliberately Missing

| Feature | Present in other agents | Why mini-SWE-agent omits it |
|---------|------------------------|----------------------------|
| Planning step | ForgeCode, Capy, Droid | The LM plans implicitly via system prompt's "Recommended Workflow" |
| Reflection step | OpenHands, Goose | The LM sees full history — it can naturally reflect on failures |
| Tool selection | Gemini CLI, Warp | Only one tool: bash. No selection needed. |
| Context management | Codex CLI, Claude Code | Messages go in, they never come out. Context fills up? Hit the limit. |
| Multi-agent | ForgeCode, Ante, Claude Code | One agent, one loop, one model. |
| Streaming | OpenCode, Goose, Gemini CLI | Synchronous calls. Wait for completion. |
| Permission system | Claude Code, Codex CLI | Research agent — no user in the loop. |

### Why the Trajectory IS the Training Data

Because mini-SWE-agent never compacts, summarizes, or drops messages, the trajectory saved to disk is **byte-for-byte identical** to what the model saw at the last step. This makes it perfect for:

- **Fine-tuning**: The trajectory is a supervised training example
- **Reinforcement learning**: Attach reward signals (SWE-bench pass/fail) directly to trajectories
- **Debugging**: Replay the exact prompt the model received at any step
- **Analysis**: Study reasoning patterns across thousands of trajectories

This design choice is not accidental — mini-SWE-agent was built as a research platform for studying and improving coding agents.

---

## From Paper to Production

The path from Yao et al.'s 2022 paper to today's production coding agents involved several key evolutions:

### Evolution 1: Text-Based → API-Native Tool Calling

The paper used text parsing (`Action: Search[query]`). When OpenAI introduced function calling in June 2023, the pattern became first-class in the API. Agents no longer needed fragile regex parsers — the API guaranteed structured tool calls. This was the single biggest accelerant for agent development.

### Evolution 2: Single-Turn → Multi-Turn

The paper demonstrated ReAct on tasks that typically took 3–7 steps. Modern coding agents routinely run for 20–100+ steps on complex tasks. This required solving problems the paper didn't address:

- **Context window management**: When the conversation exceeds the context window, what do you drop? Codex CLI compacts at 90% capacity. OpenHands uses a CondensationAgent. mini-SWE-agent just... fills up.
- **Cost management**: 50 steps × $0.10/step = $5.00 per task. Agents need cost limits, step limits, and cost-aware model selection.
- **Error recovery**: In a 50-step trajectory, the model will inevitably make mistakes. The loop must be resilient — errors from tool execution feed back as observations, and the model must learn to recover.

### Evolution 3: Simple Tasks → Complex Coding

The paper tested on QA and web navigation. Coding agents face a fundamentally harder domain:

- **State is complex**: The "environment" is an entire codebase — thousands of files, build systems, test suites, version control.
- **Actions have side effects**: Editing a file can break other files. Running a command can modify system state. The agent must reason about cascading consequences.
- **Verification is multi-dimensional**: A "correct" code change must compile, pass tests, maintain style, not break unrelated functionality, and actually solve the problem.

The ReAct pattern itself didn't change. What changed was the sophistication of the tools (from `Search[query]` to full IDE-like capabilities) and the complexity of the system prompts (from one paragraph to thousands of tokens of coding-specific guidance).

### Evolution 4: Layered Complexity

Modern agents layer capabilities ON TOP of the ReAct core:

```
Layer 5: Multi-agent orchestration      (ForgeCode, Ante, Claude Code)
Layer 4: State machines / event systems  (OpenHands, Codex CLI)
Layer 3: Streaming + real-time rendering (OpenCode, Goose, Gemini CLI)
Layer 2: Context management / compaction (Codex CLI, OpenHands, Aider)
Layer 1: Error handling, retries, cost   (all agents)
Layer 0: THE REACT LOOP                  (all agents — the universal foundation)
```

Every layer is optional. mini-SWE-agent implements only Layer 0 and scores competitively. But production agents need Layers 1–5 for reliability, user experience, and cost control.

---

## ReAct in the Wild: Implementation Comparison

The following table compares how 17 agents implement the ReAct pattern. Despite surface differences, every agent follows the same Thought → Action → Observation cycle.

| Agent | Language | ReAct Style | Tool Interface | Planning | Notable Twist |
|-------|----------|-------------|----------------|----------|---------------|
| **mini-SWE-agent** | Python | Text (bash in code blocks) | Single tool: bash via subprocess | Implicit (system prompt) | 2-line step(); trajectory = training data |
| **Claude Code** | TypeScript | API function calling | Read/Write/Bash/Grep/Glob + sub-agents | Adaptive 3-phase | Sub-agent delegation; user always in loop |
| **Codex CLI** | Rust | API function calling | Sandboxed shell + file ops | Implicit | SQ/EQ message passing; auto-compaction at 90% |
| **OpenCode** | Go | API function calling | Shell + file + LSP tools | Implicit | Go channels + context.Context for cancellation |
| **Goose** | Rust | API function calling | Extension-provided tools | Implicit | MOIM context injection; tokio::select! merging |
| **ForgeCode** | TypeScript | API function calling | Full IDE tools | Explicit (Muse/Forge) | Verification enforcement; progressive thinking |
| **Ante** | Rust | API function calling | File + shell + web tools | Explicit (meta-agent) | Fan-out/fan-in; lock-free scheduler |
| **OpenHands** | Python | API function calling | EventStream + runtime | Explicit (microagents) | State machine; CondensationAgent; stuck detector |
| **Gemini CLI** | TypeScript | API function calling | Batched tool scheduler | Implicit | Parallel read-only tools; token caching |
| **Warp** | TypeScript | API function calling | Shell + file tools | Implicit | Integrated terminal; context from IDE |
| **Droid** | TypeScript | API function calling | MCP tool servers | Explicit (plan-exec-verify) | Multi-repo; shadow workspace |
| **Aider** | Python | API function calling + edit formats | Whole/diff/udiff edit formats | Implicit | Edit-apply-verify; repo map for context |
| **Pi** | TypeScript | API function calling | Extensible plugin tools | Implicit | Event hooks (tool:start, tool:complete) |
| **Junie CLI** | Kotlin | API function calling | IDE-integrated tools | Explicit (plan, then execute) | JetBrains integration; stage gates |
| **Sage** | Python | API function calling | AST-aware code tools | Implicit | Code intelligence; semantic search |
| **Capy** | TypeScript | API function calling | Shell + file tools | Explicit (Captain/Build) | Hard phase constraints; Captain can't code |
| **TongAgents** | Python | Code-as-action | Generated Python code | Explicit (pipeline graph) | Tool-creating agents; nested tool composition |

### Key Observations from the Table

1. **Every agent uses the ReAct cycle** — the variation is in what wraps it, not whether it exists.
2. **Function calling dominates** — only mini-SWE-agent (text-based) and TongAgents (code-as-action) deviate.
3. **Planning is evenly split** — roughly half use implicit planning (system prompt guidance), half use explicit planning (separate planning step or agent).
4. **Language doesn't determine architecture** — Python, TypeScript, Rust, Go, and Kotlin agents all implement the same pattern.
5. **Complexity correlates with production-readiness**, not with benchmark performance — mini-SWE-agent is simplest AND competitive.

---

## Limitations and Extensions

The ReAct pattern, for all its universality, has well-known limitations. Each has spawned extensions that real agents implement.

### Limitation 1: Context Window Exhaustion

On long tasks (50+ steps), the message history exceeds the context window. The model loses access to early reasoning and observations. This is the most common failure mode for agentic coding.

**Solutions observed in practice:**

| Strategy | Agent | How It Works |
|----------|-------|-------------|
| Auto-compaction | **Codex CLI** | Summarizes conversation at 90% capacity via remote endpoint; preserves GhostSnapshot for undo |
| Condensation agent | **OpenHands** | Runs a CondensationAgent that intelligently summarizes, keeping current file state and test results |
| Sliding window | **Goose** | Drops oldest messages beyond threshold; MOIM re-injects critical context each turn |
| Repo map | **Aider** | Compressed AST-based view of codebase structure included in every prompt; reduces need for exploration |
| Accept the limit | **mini-SWE-agent** | Simply fills up and hits cost/step limits; sufficient for SWE-bench's 20–40 step tasks |

### Limitation 2: No Cross-Task Learning

Each agent invocation starts fresh. The model doesn't learn from previous tasks — if it solved a similar bug yesterday, it doesn't remember that today.

**Solutions observed in practice:**
- **CLAUDE.md / memory files**: Claude Code persists learnings in `.claude/` files loaded into each session's system prompt. Not true learning, but effective knowledge transfer.
- **Fine-tuning on trajectories**: mini-SWE-agent's design enables this — collect trajectories from many tasks, fine-tune the base model. The next invocation benefits from the training.
- **Prompt libraries**: Goose and Droid support custom instructions that encode domain knowledge.
- **Project conventions**: Many agents read config files (`.forgecode/`, `.goose/`, `CONVENTIONS.md`) that persist team knowledge.

### Limitation 3: Action Loops (Stuck Detection)

The model can get stuck repeating the same failed action: "Try X → X fails → Try X again → X fails again." This is especially common when the model lacks the knowledge to try an alternative approach.

**Solutions observed in practice:**
- **Stuck detector**: **OpenHands** monitors for repeated actions or identical observations and intervenes — suggesting an alternative or terminating.
- **Cost/step limits**: Every production agent implements hard limits. When exceeded, the loop terminates gracefully.
- **Temperature variation**: Some systems increase temperature after repeated failures, encouraging exploration.
- **Error escalation**: When a tool fails repeatedly, escalate: "WARNING: This approach has failed 3 times. Try a fundamentally different approach."
- **Backoff prompting**: **Goose** adds progressively stronger hints when the model appears stuck, eventually suggesting it reconsider its strategy entirely.

### Limitation 4: No Hierarchical Reasoning

The flat Thought → Action → Observation cycle doesn't naturally support hierarchical planning (break a task into subtasks, solve each, compose results).

**Solutions observed in practice:**
- **Multi-agent orchestration**: ForgeCode (Muse plans, Forge executes), Ante (meta-agent decomposes), Capy (Captain plans, Build executes).
- **System prompt structure**: mini-SWE-agent's "Recommended Workflow" provides a soft hierarchy without architectural support.
- **Sub-agent delegation**: Claude Code spawns specialized sub-agents for exploration or complex subtasks, each running their own ReAct loop in a separate context window.

### Extension: ReAct + Reflection

Add an explicit reflection step after observations:

```
Thought → Action → Observation → Reflection → Thought → ...
```

The reflection asks: "Did that work? What did I learn? Should I change my approach?" This is what **Reflexion** (Shinn et al., 2023) formalized. In practice, agents like **OpenHands** implement this via microagent prompts that trigger self-evaluation after key milestones.

### Extension: ReAct + Planning

Add a planning phase before (or interleaved with) the ReAct loop:

```
Plan → [Thought → Action → Observation]* → Verify → [Replan if needed]
```

**ForgeCode** implements this with its Muse/Forge split — Muse creates a detailed plan, Forge executes it step by step. **Capy** implements it with Captain/Build — Captain creates the plan (and can ask clarifying questions), Build executes it autonomously. **Droid** uses an explicit plan-execute-verify pipeline.

The planning extension helps on tasks where exploration is expensive and wrong paths are costly. It hurts on tasks where the plan must adapt quickly — the plan can become stale before execution completes.

### Extension: ReAct + Retrieval (RAG-ReAct)

Augment observations with retrieved context:

```
Thought → Action → Observation + Retrieved Context → Thought → ...
```

**Goose**'s MOIM (Model-Oriented Information Management) injects dynamic context from extensions before each LLM call. **Aider**'s repository map provides a compressed structural view of the codebase included in every prompt. **ForgeCode**'s semantic entry-point discovery finds relevant files before any agent starts working.

### Extension: ReAct + Verification Enforcement

Add a mandatory verification step before task completion:

```
[Thought → Action → Observation]* → Verification Required → Done
```

**ForgeCode Services** implements this programmatically — the runtime requires a verification pass (run tests, check output) before marking any task complete. This was their key insight: prompting "please verify" doesn't reliably produce verification. **Enforcement** does. The agent cannot claim it's done until the system confirms the verification step has actually executed.

### Alternative: Tree of Thoughts (ToT)

Instead of a linear Thought → Action chain, Tree of Thoughts (Yao et al., 2023) explores multiple reasoning paths in parallel, evaluating each and selecting the best:

```
                    ┌── Thought A1 → Action → Obs → Thought A2 → ...
                    │
Initial State ──────┼── Thought B1 → Action → Obs → Thought B2 → ...
                    │
                    └── Thought C1 → Action → Obs → (pruned)
```

**Why it's rare in coding agents**: Each branch requires separate LLM calls and separate environment state (you'd need to fork the codebase for each branch). The cost is multiplicative — 3 branches × 10 steps × $0.10/step = $3.00 vs. $1.00 for linear ReAct. For most coding tasks, the linear approach with error recovery is more cost-effective.

---

## The ReAct Pattern as Universal Primitive

ReAct is to coding agents what the event loop is to Node.js, what the request-response cycle is to web servers, what map-reduce is to distributed computing. It is the **universal primitive** — the irreducible core that every system implements, even when buried under layers of abstraction.

```
┌──────────────────────────────────────────────────────────────┐
│                                                              │
│    ReAct is not one pattern among many.                      │
│    It is THE pattern.                                        │
│                                                              │
│    Everything else — streaming, state machines,              │
│    multi-agent orchestration, context management,            │
│    verification enforcement — is built on top of it.         │
│                                                              │
│    An agent without ReAct is not an agent.                   │
│    It's a chatbot.                                           │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

The Thought → Action → Observation cycle will remain the foundation as long as we have:
1. **Stateless models** that need external state management (the message list)
2. **Tool use** as the bridge between model reasoning and world interaction
3. **Feedback** as the mechanism for grounding model behavior in reality

When any of these three constraints changes — models with persistent memory, models that can directly execute code, models that can perceive the world without tool mediation — the pattern may evolve. Until then, ReAct is the law of the land.
