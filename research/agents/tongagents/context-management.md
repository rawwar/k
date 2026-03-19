# TongAgents — Context Management

> ⚠️ **Limited information available.** This analysis focuses on what can be inferred from the significant performance gap between models, which is one of the most informative signals about TongAgents' context management approach.

## The Model Performance Gap

The most revealing data point about TongAgents' context management is the performance difference:

| Model | Score | Context Window |
|-------|-------|---------------|
| Gemini 3.1 Pro | 80.2% ±2.6 | Very large (1M+ tokens) |
| Claude Opus 4.6 | ~71.9% | 200K tokens |

The **~8 percentage point gap** is substantial. Several hypotheses could explain it:

### Hypothesis 1: Context Window Utilization
If TongAgents maintains extensive execution history (command outputs, file contents, previous reasoning), it may benefit significantly from Gemini's larger context window. Agents that accumulate context without aggressive summarization would naturally perform better with larger windows.

### Hypothesis 2: Reasoning Style Alignment
The agent's prompts may be structured in a way that aligns better with Gemini's reasoning patterns. Different models respond differently to:
- Chain-of-thought formatting
- System prompt structure
- Tool call schemas
- Multi-turn conversation flow

### Hypothesis 3: Tool Calling Fidelity
Gemini 3.1 Pro may produce more reliable structured outputs for tool calls, leading to fewer parsing errors and wasted iterations.

### Hypothesis 4: Development Model Bias
TongAgents may have been primarily developed and tested with Gemini, with Claude support added later. Prompt engineering is often model-specific, and subtle optimizations for one model may not transfer.

## Inferred Context Strategy

For Terminal-Bench tasks (which involve multi-step CLI interactions), TongAgents likely manages:

### Execution History
- **Command log** — what commands were run and their outputs
- **State summary** — current understanding of system state
- **Error log** — what went wrong and how it was addressed

### Task Context
- **Original task description** — preserved throughout execution
- **Current plan** — the active plan with completed/pending steps
- **Goal state** — what "done" looks like for this task

### Potential Approaches

**Full History (Simple):**
Append everything to context. Works with large windows (Gemini), degrades with smaller ones (Claude). This would neatly explain the performance gap.

**Sliding Window:**
Keep recent N interactions in full, summarize older ones. More robust across model context sizes.

**Structured State:**
Maintain a structured representation of system state rather than raw command history. More token-efficient but harder to implement correctly.

## Multi-Agent Context Sharing

If TongAgents uses multiple agents, context management becomes more complex:
- Each agent needs access to relevant context but not necessarily everything
- The orchestrator must decide what context to pass to each specialized agent
- Shared memory or a blackboard pattern could enable efficient context sharing

## What We Don't Know

- Whether the agent uses context summarization or compression
- How much of the context window is typically consumed per task
- Whether there are different context strategies for different task types
- If the agent tracks token usage and adapts its strategy accordingly
- Whether the performance gap is primarily a context issue or reflects other architectural factors