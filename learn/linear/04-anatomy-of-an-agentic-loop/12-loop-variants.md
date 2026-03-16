---
title: Loop Variants
description: Alternative agentic loop architectures including ReAct, plan-then-execute, and hierarchical multi-agent loops.
---

# Loop Variants

> **What you'll learn:**
> - How the ReAct (Reason + Act) pattern structures reasoning and action as alternating explicit steps
> - How plan-then-execute loops separate planning from execution for more predictable behavior
> - When to use single-loop, nested-loop, and multi-agent architectures based on task complexity

The agentic loop we have been describing -- call the LLM, check for tool use, execute tools, feed results back -- is the most common pattern, but it is not the only one. Different tasks and different constraints lead to different loop architectures. Some agents reason explicitly before acting. Some plan all steps upfront before executing any. Some use multiple nested loops for complex tasks.

Understanding these variants helps you choose the right architecture for your agent and recognize the trade-offs involved. You will also encounter these patterns when reading the source code of production agents, and knowing the vocabulary helps you navigate unfamiliar codebases.

## Variant 1: The Basic Tool Loop

This is what we have been describing throughout this chapter. Let's name it explicitly for comparison:

```text
Basic Tool Loop:
  repeat:
    response = call_llm(history)
    if response.has_tool_calls:
      results = execute_tools(response.tool_calls)
      history.append(response, results)
    else:
      return response.text
```

**Characteristics:**
- The model decides what to do at each step
- No explicit planning or reasoning phase
- Tool calls and reasoning are mixed together in the model's output
- The model can change its approach at any point

This is the loop that Claude Code, OpenCode, and most production coding agents use. It is simple, flexible, and works well with capable models that can reason and act simultaneously.

## Variant 2: The ReAct Loop

The ReAct (Reason + Act) pattern, introduced in a 2022 research paper, structures each iteration into three explicit phases: Thought, Action, and Observation. The key difference from the basic loop is that reasoning is made explicit -- the model must articulate its thinking before acting.

```text
ReAct Loop:
  repeat:
    thought = call_llm("What should I do next and why?", history)
    action = call_llm("Based on that reasoning, what tool should I call?", history + thought)
    observation = execute_tool(action)
    history.append(thought, action, observation)
    if action == "finish":
      return thought
```

In practice, modern LLM APIs do not require separate calls for thought and action. The model produces both in a single response. But the ReAct framing influences the system prompt:

```rust
const REACT_SYSTEM_PROMPT: &str = r#"
You are a coding assistant that follows the ReAct framework.
For each step, you must:

1. **Thought**: Explain your reasoning about what to do next.
   What information do you need? What have you learned so far?

2. **Action**: Choose a tool to execute, or respond with your final answer.
   Always explain why you chose this specific action.

3. After receiving the tool result, reflect on what you learned
   before deciding your next step.

Always show your reasoning explicitly. Never act without explaining why.
"#;
```

::: python Coming from Python
If you have used LangChain, you have likely encountered the ReAct agent -- it is one of LangChain's built-in agent types. LangChain implements ReAct by parsing the model's output into explicit "Thought:", "Action:", and "Observation:" sections using string parsing. Modern tool-use APIs make this parsing unnecessary (tool calls are structured), but the ReAct principle of explicit reasoning remains valuable as a system prompt strategy.
:::

**When to use ReAct:**
- When you need an audit trail of the model's reasoning (debugging, compliance)
- When working with less capable models that benefit from structured thinking
- When the task requires careful sequential reasoning where each step depends on the previous

**Trade-offs:**
- More tokens consumed (explicit reasoning takes space)
- Potentially slower (more verbose output)
- Better debuggability (you can read the model's reasoning)
- Can improve accuracy for complex reasoning tasks

## Variant 3: Plan-Then-Execute

In this variant, the model first creates a complete plan, then executes each step. The planning and execution are separate loops:

```text
Plan-Then-Execute:
  plan = call_llm("Create a step-by-step plan for this task", user_request)

  for step in plan.steps:
    result = execute_step(step)
    if result.failed:
      revised_plan = call_llm("Step failed. Revise the plan.", plan, result)
      continue with revised_plan

  summary = call_llm("Summarize what was accomplished", plan, all_results)
  return summary
```

In Rust, this looks like:

```rust
struct Plan {
    steps: Vec<PlanStep>,
}

struct PlanStep {
    description: String,
    tool_name: String,
    expected_input: serde_json::Value,
}

fn plan_then_execute(
    user_request: &str,
    registry: &ToolRegistry,
    history: &mut ConversationHistory,
) -> Result<String, AgentError> {
    // Phase 1: Planning
    history.add_user_message(format!(
        "Create a step-by-step plan for: {}. \
         Respond with a numbered list of specific actions.",
        user_request
    ));
    let plan_response = call_llm(history)?;
    let plan = parse_plan(&plan_response.text);

    // Phase 2: Execution
    let mut results: Vec<StepResult> = Vec::new();

    for (i, step) in plan.steps.iter().enumerate() {
        println!("Step {}/{}: {}", i + 1, plan.steps.len(), step.description);

        // Execute the step using the basic tool loop for each step
        let step_result = execute_plan_step(step, registry, history)?;
        results.push(step_result);

        // Check if the step failed and we need to re-plan
        if results.last().map_or(false, |r| r.failed) {
            let revised = revise_plan(&plan, &results, history)?;
            // Continue with revised plan
            // (simplified -- real implementation would restructure remaining steps)
            break;
        }
    }

    // Phase 3: Summary
    let summary = summarize_execution(&results, history)?;
    Ok(summary)
}

struct StepResult {
    step_index: usize,
    output: String,
    failed: bool,
}

fn parse_plan(text: &str) -> Plan {
    // Parse numbered steps from the model's response
    let steps = text.lines()
        .filter(|line| {
            line.trim_start().starts_with(|c: char| c.is_ascii_digit())
        })
        .map(|line| PlanStep {
            description: line.trim().to_string(),
            tool_name: String::new(), // Determined during execution
            expected_input: serde_json::Value::Null,
        })
        .collect();

    Plan { steps }
}
```

**When to use plan-then-execute:**
- Complex multi-step tasks where the order of operations matters
- When you want to show the user what the agent intends to do before it starts
- When you want user approval of the plan before execution begins
- Long-running tasks where progress tracking is important

**Trade-offs:**
- More predictable behavior (the plan provides a roadmap)
- User can review and approve before execution
- Less flexible -- the model cannot easily change course mid-execution
- Plans can become stale if the environment changes during execution
- Requires an additional LLM call for planning

## Variant 4: Parallel Tool Execution

The basic loop executes tools sequentially. When the model requests multiple independent tools, you can run them in parallel:

```text
Parallel Tool Loop:
  repeat:
    response = call_llm(history)
    if response.has_tool_calls:
      independent_groups = classify_tools(response.tool_calls)
      results = execute_parallel(independent_groups)
      history.append(response, results)
    else:
      return response.text
```

```rust
async fn execute_parallel_groups(
    calls: &[ToolCall],
    registry: &ToolRegistry,
) -> Vec<ToolResult> {
    // Group 1: Read-only operations (safe to parallelize)
    let reads: Vec<&ToolCall> = calls.iter()
        .filter(|c| is_read_only(&c.name))
        .collect();

    // Group 2: Write operations (run sequentially after reads)
    let writes: Vec<&ToolCall> = calls.iter()
        .filter(|c| !is_read_only(&c.name))
        .collect();

    // Execute reads in parallel
    let read_futures = reads.iter().map(|call| {
        let result = registry.dispatch(&call.name, &call.input);
        async move { result.with_id(call.id.clone()) }
    });
    let mut results = futures::future::join_all(read_futures).await;

    // Execute writes sequentially
    for call in writes {
        let result = registry.dispatch(&call.name, &call.input)
            .with_id(call.id.clone());
        results.push(result);
    }

    results
}

fn is_read_only(tool_name: &str) -> bool {
    matches!(tool_name, "read_file" | "list_directory" | "search_files" | "grep")
}
```

**When to use parallel execution:**
- When the model frequently requests multiple independent reads
- When tool latency is high (network calls, slow I/O)
- When you want to minimize total turn time

::: tip In the Wild
Claude Code executes tool calls from the model's response. When the model requests multiple file reads or multiple search operations simultaneously, these can run in parallel since they are independent read-only operations. OpenCode also supports parallel tool execution for independent operations, significantly reducing the total time for turns that involve multiple file reads.
:::

## Variant 5: Nested Loops (Hierarchical)

For very complex tasks, a single loop is not enough. You can nest loops:

```text
Outer Loop (Task-level):
  decompose task into subtasks
  for each subtask:
    Inner Loop (Step-level):
      run basic tool loop on subtask
      collect subtask result
  synthesize all subtask results into final response
```

This is the architecture behind "multi-agent" systems where a planner agent delegates work to specialized worker agents:

```rust
async fn hierarchical_loop(
    task: &str,
    registry: &ToolRegistry,
) -> Result<String, AgentError> {
    // Outer loop: task decomposition
    let subtasks = decompose_task(task)?;
    let mut subtask_results: Vec<String> = Vec::new();

    for subtask in &subtasks {
        // Inner loop: each subtask gets its own agentic loop
        // with its own history and iteration budget
        let mut subtask_history = ConversationHistory::new();
        let result = run_inner_loop(
            &subtask.description,
            &mut subtask_history,
            registry,
            &LoopConfig { max_iterations: 20 }, // Tighter limits per subtask
        )?;
        subtask_results.push(result);
    }

    // Synthesis: combine all subtask results
    let synthesis = synthesize_results(&subtask_results)?;
    Ok(synthesis)
}

struct Subtask {
    description: String,
    tools_needed: Vec<String>,
}
```

**When to use nested loops:**
- Very large tasks that can be naturally decomposed (e.g., "set up an entire project")
- When different subtasks need different tools or configurations
- When you want isolation between subtasks (one failure does not corrupt the context of others)

**Trade-offs:**
- Most complex architecture
- Highest token consumption (each inner loop has overhead)
- Best for tasks that are naturally decomposable
- Each inner loop has its own context, preventing cross-contamination

## Choosing the Right Variant

Here is a decision framework:

| Task Type | Best Variant | Why |
|-----------|-------------|-----|
| Simple Q&A, single file edits | Basic tool loop | Minimal overhead, fast |
| Multi-step reasoning tasks | ReAct | Explicit reasoning improves accuracy |
| Large planned refactors | Plan-then-execute | User can review plan before execution |
| Multiple independent reads | Parallel tool execution | Reduces latency |
| Full project setup | Nested loops | Isolates subtasks, manages complexity |

For a general-purpose coding agent, start with the basic tool loop. It handles the vast majority of tasks well. Add parallel execution for read-only tools as a straightforward performance optimization. Consider plan-then-execute for tasks that the user explicitly flags as large or risky. Reserve nested loops for advanced use cases.

## Key Takeaways

- The basic tool loop (call LLM, detect tools, execute, observe, repeat) is the most common and most flexible variant, used by most production coding agents
- ReAct structures each iteration into explicit Thought/Action/Observation phases, improving debuggability and reasoning quality at the cost of more tokens
- Plan-then-execute separates planning from execution, giving users visibility and approval over the agent's intended actions before any changes are made
- Parallel tool execution runs independent tools concurrently (especially read-only operations), reducing total turn time without changing the loop structure
- Nested loops decompose complex tasks into independent subtasks, each with its own agentic loop and context, providing isolation at the cost of complexity and token usage
