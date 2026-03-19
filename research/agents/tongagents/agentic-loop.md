# TongAgents — Agentic Loop

> ⚠️ **Limited information available.** No source code or technical documentation has been published. This analysis infers loop behavior from Terminal-Bench 2.0 performance patterns and BIGAI's research background.

## What Terminal-Bench Requires

Terminal-Bench 2.0 evaluates agents across 89 tasks in realistic CLI environments. To achieve 80.2% (Rank #3), TongAgents must handle:

- **Multi-step command sequences** — tasks that require 5-20+ sequential shell commands
- **Error recovery** — commands that fail and require diagnosis and retry
- **State tracking** — understanding the current system state across commands
- **Conditional execution** — choosing different paths based on command output
- **Task completion detection** — knowing when a task is actually done

This implies a sophisticated execution loop, not a simple prompt-then-execute cycle.

## Inferred Loop Structure

Given BIGAI's emphasis on cognitive architecture and planning, the loop likely follows a **plan-execute-verify** pattern:

```
1. ANALYZE — Parse the task description, identify requirements
2. PLAN    — Generate a high-level plan with ordered steps
3. EXECUTE — Run the next step (shell command, file edit, etc.)
4. OBSERVE — Capture and interpret the output
5. EVALUATE — Did the step succeed? Is the plan still valid?
6. ADAPT   — If needed, revise the plan based on new information
7. VERIFY  — Check if the overall task is complete
8. REPEAT  — Go to step 3 if more work remains
```

### Why Plan-Execute-Verify?

- BIGAI's cognitive science research emphasizes **deliberative planning** over reactive behavior
- The high accuracy (80.2%) suggests the agent doesn't just try things randomly — it has a strategy
- Terminal-Bench tasks often require understanding the *goal state* before acting

## Iteration and Error Recovery

The 80.2% score with Gemini 3.1 Pro vs ~71.9% with Claude Opus 4.6 suggests:

- The loop includes **retry logic** — when a command fails, the agent can diagnose and retry
- Error recovery may depend on model reasoning quality, explaining the performance gap
- The agent likely has a **maximum iteration limit** to prevent infinite loops on unsolvable tasks

## Multi-Agent Loop Hypothesis

If TongAgents uses multiple agents (as the name suggests), the loop may involve **inter-agent communication**:

```
Orchestrator: "Configure nginx with SSL"
  → Planner: "Steps: 1) install nginx, 2) generate cert, 3) configure, 4) test"
  → Executor: runs step 1, returns output
  → Verifier: "nginx installed successfully, proceed"
  → Executor: runs step 2, returns error
  → Planner: "Revise step 2: install certbot first"
  → Executor: runs revised step, returns output
  → Verifier: "cert generated, proceed"
  ... continues until task complete
```

This separation of concerns could explain the strong performance — each agent is focused on what it does best.

## What We Don't Know

- The actual iteration limits or timeout mechanisms
- How the agent handles ambiguous task descriptions
- Whether there is a "reflection" or "self-critique" step (common in recent agent research)
- The specific prompting strategy for each loop phase
- How context is managed across loop iterations (see [context-management.md](context-management.md))
- Whether the loop is fixed or dynamically adapted per task type