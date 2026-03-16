# The Agentic Loop

## Overview

The agentic loop is the central architectural pattern that transforms a chatbot into a coding agent. A chatbot exchanges single messages: the user speaks, the model responds, and the turn is over. An agent, by contrast, can take autonomous multi-step action within a single user turn. The mechanism enabling this is a loop that repeatedly invokes the LLM, detects tool calls in its response, executes those tools, feeds results back, and re-invokes the LLM until it decides the task is complete. Every production coding agent -- Claude Code, Cursor, Aider, OpenCode -- is built around some variant of this loop. Its design directly determines the agent's capabilities, cost profile, reliability, and user experience.

## The Pattern

The fundamental agentic loop follows a five-phase cycle embedded inside a user-facing REPL:

1. **User input.** The user types a natural-language message. This enters the outer REPL (Read-Eval-Print Loop) that every interactive agent exposes.
2. **LLM invocation.** The agent sends the full conversation history -- system prompt, prior messages, tool definitions -- to the language model.
3. **Tool call detection.** The agent inspects the LLM's response. If it contains tool calls (read file, run command, edit code), the agent extracts them. If the response contains only text and an `end_turn` stop reason, the loop terminates.
4. **Tool execution and observation.** The agent dispatches each tool call, runs it, and collects the results. These results are appended to the conversation history as tool result messages.
5. **Re-invocation.** The agent loops back to step 2, sending the now-longer conversation history (including tool results) to the LLM for another round of reasoning.

The outer REPL gives the user control -- they can interrupt, ask follow-ups, or change direction. The inner agentic loop gives the model autonomy -- it can take multi-step actions without waiting for user input at each step. This two-loop architecture is universal across coding agents.

## Implementation Approaches

Production agents implement several distinct loop variants, each with different trade-offs.

**The basic tool loop** is the most common. The LLM decides what to do at each step with no explicit planning phase. Tool calls and reasoning are interleaved in the model's output, and the model can change its approach at any point. Claude Code and OpenCode both use this pattern. It is simple, flexible, and works well with capable models.

**The ReAct (Reason + Act) loop** structures each iteration into explicit Thought, Action, and Observation phases. The model must articulate its reasoning before acting. This produces an audit trail and can improve accuracy for complex reasoning tasks, but it consumes more tokens and runs slower. In practice, modern tool-use APIs make the explicit parsing of ReAct sections unnecessary, but the principle of prompting for explicit reasoning before action remains valuable.

**The plan-then-execute loop** separates planning from execution entirely. The model first produces a complete step-by-step plan, then the agent executes each step. This gives users visibility and approval over the agent's intentions before changes are made. The trade-off is reduced flexibility -- the model cannot easily change course mid-execution, and plans can become stale if the environment changes during execution.

**Parallel tool execution** is an optimization to the basic loop. When the model requests multiple independent tools (for example, reading three files simultaneously), the agent runs them concurrently rather than sequentially. Read-only operations are safe to parallelize; write operations must remain sequential to avoid conflicts.

**Nested (hierarchical) loops** decompose complex tasks into subtasks, each running its own independent agentic loop with its own context and iteration budget. This provides isolation between subtasks and prevents one failure from corrupting the context of another, at the cost of higher token consumption and architectural complexity.

## Key Considerations

**Loop termination is critical.** Without well-designed stop conditions, an agent can spin indefinitely. Production agents implement four categories of stop conditions: model-initiated stops (the model decides it is done), iteration limits (a hard cap per user turn, typically defaulting around 50), token budgets (controlling cost, since each iteration adds to the context and the cost compounds -- a 10-iteration loop can cost 30x a single call), and user interrupts (Ctrl+C at any point). All four must be checked at the beginning of each iteration, and user interrupts must also be checked during long-running operations like API calls and tool execution.

**Error recovery determines robustness.** When a tool fails -- a file is not found, a command returns an error, a compilation fails -- the agent must decide whether to send the error back to the model for self-correction or handle it at the system level. Tool errors (bad inputs, missing files) should go back to the model with structured error messages that explain what failed, why, and what to try instead. System errors (disk full, network down) should be handled by the agent itself since the model cannot fix infrastructure problems. Retry budgets prevent the model from looping indefinitely on the same failing tool call.

**Graceful degradation shapes user experience.** When a stop condition fires mid-task, the agent must report what it accomplished before stopping. Silently discarding partial work -- file edits already applied, information already gathered -- is the worst possible outcome. Production agents summarize progress (actions taken, files modified, errors encountered) and offer the user a path to continue from where the agent left off.

**Cost grows non-linearly.** Each iteration appends tool calls and results to the conversation history, so input tokens grow with every call. Context management (compaction, summarization) becomes essential for long-running tasks, and the loop must integrate pre-call checks that evaluate whether the context is approaching its limit.

## Cross-References
- [The REPL Pattern](/linear/04-anatomy-of-an-agentic-loop/01-the-repl-pattern) -- How the classic REPL becomes the agent's outer loop
- [Stop Conditions](/linear/04-anatomy-of-an-agentic-loop/10-stop-conditions) -- Iteration limits, token budgets, and user interrupts
- [Loop Variants](/linear/04-anatomy-of-an-agentic-loop/12-loop-variants) -- ReAct, plan-then-execute, parallel, and hierarchical loops
- [Error States](/linear/04-anatomy-of-an-agentic-loop/11-error-states) -- Error recovery within the agentic loop
- [Context Window Management](/linear/10-conversation-state-machines/04-context-window-management) -- Managing growing context across loop iterations
