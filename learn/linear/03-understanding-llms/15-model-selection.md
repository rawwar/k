---
title: Model Selection
description: Choosing the right model for your agent based on capability, cost, speed, and context window trade-offs.
---

# Model Selection

> **What you'll learn:**
> - How to evaluate models across dimensions that matter for agents: tool use, code quality, instruction following, and speed
> - The trade-offs between frontier models and smaller models for different agent tasks
> - How to implement model routing so your agent uses the right model for each operation

Choosing a model is not a one-time decision. The best model for complex multi-file refactoring is not the best model for a simple file read. The model that produces the highest quality code is not the fastest or the cheapest. A well-designed agent selects the right model for each operation, balancing capability, cost, and speed. This subchapter helps you reason about model selection and implement it in your agent.

## Dimensions That Matter for Agents

Not all model capabilities matter equally for coding agents. Here are the dimensions to evaluate, ranked by importance:

### 1. Tool Use Reliability

The most critical capability for an agent. A model with unreliable tool use -- generating malformed arguments, calling the wrong tool, or failing to use tools when it should -- breaks the entire agent loop. No amount of code quality matters if the model cannot reliably call `read_file` with a valid path.

Test tool use reliability by running tasks that require specific sequences of tool calls and measuring:
- Does the model call the right tool?
- Are the arguments well-formed JSON?
- Does the model use tools proactively (without being prompted to)?
- Does the model correctly chain tool calls (read, then edit, then verify)?

### 2. Code Generation Quality

For a coding agent, the quality of generated code directly affects user experience. Evaluate:
- Syntactic correctness (does the code compile?)
- Semantic correctness (does it do what was asked?)
- Idiomatic style (does it follow language conventions?)
- Appropriate error handling
- Correct use of libraries and APIs

### 3. Instruction Following

The model needs to follow system prompt instructions reliably. A model that ignores "always run tests after editing" or "never modify files outside the project directory" is a liability for agent use, even if its raw code generation is excellent.

### 4. Context Utilization

With large context windows, models need to effectively use information spread across the entire context. A model with a 200K context window that only attends to the last 10K tokens is functionally equivalent to a 10K model for agent purposes.

### 5. Speed (Time to First Token and Tokens per Second)

Agent UX depends on responsiveness. Latency compounds across multiple tool use cycles -- if each API call takes 5 seconds, a 10-step task takes nearly a minute just in API wait time. Evaluate:
- **Time to first token (TTFT):** How quickly does streaming begin?
- **Tokens per second (TPS):** How fast does the model generate output?

### 6. Cost

As covered in the [Rate Limits and Pricing](/linear/03-understanding-llms/13-rate-limits-and-pricing) subchapter, costs compound in agent loops. A model that is 10x more expensive but only marginally better may not be the right default choice.

## Current Model Landscape

Here is a practical assessment of models available for coding agents as of early 2025:

### Anthropic Models

**Claude Opus 4** -- The most capable model for complex reasoning, multi-file changes, and difficult debugging. Best for tasks that require deep understanding of codebases. High cost and slower than Sonnet.

**Claude Sonnet 4 / Claude 3.5 Sonnet** -- The sweet spot for most coding agent operations. Excellent tool use, strong code generation, fast enough for interactive use, and reasonable cost. This is the default model for most production coding agents.

**Claude 3.5 Haiku** -- Fast and cheap. Good for simple operations like file reads, straightforward edits, and classification tasks. Tool use is reliable but code generation is less sophisticated for complex tasks.

### OpenAI Models

**GPT-4o** -- Strong general-purpose model with good tool use support. Competitive with Claude Sonnet for many coding tasks. Good speed-to-capability ratio.

**GPT-4o mini** -- Very fast and cheap. Good for simple tool calls and classification. Code generation quality drops on complex tasks.

**GPT-4.1** -- Large context window (1M tokens) and strong coding performance. Good for tasks that involve very large codebases. The extended context is valuable for agents that need to analyze many files simultaneously.

**o3 / o4-mini** -- Reasoning models that spend more time "thinking" before responding. Can produce better results for complex logical problems but are slower and more expensive. The thinking tokens add to output costs.

## Model Routing

Model routing is the practice of selecting different models for different operations within the same agent session. This optimizes the cost-speed-capability trade-off:

```rust
enum TaskComplexity {
    Simple,   // File reads, simple edits, status checks
    Medium,   // Bug fixes, feature additions, test writing
    Complex,  // Multi-file refactoring, architecture changes, complex debugging
}

fn select_model(complexity: TaskComplexity) -> &'static str {
    match complexity {
        TaskComplexity::Simple => "claude-3-5-haiku-20241022",
        TaskComplexity::Medium => "claude-sonnet-4-20250514",
        TaskComplexity::Complex => "claude-opus-4-20250514",
    }
}
```

The challenge is determining complexity automatically. Approaches include:

**Keyword heuristics:** If the user mentions "refactor," "redesign," or "architecture," use a more capable model. If they say "fix typo" or "update version," use a cheaper model.

**Turn-based escalation:** Start with a cheaper model and escalate to a more capable one if the agent fails (compilation errors, test failures) after N attempts.

**User preference:** Let the user select the model tier ("fast mode" vs. "thorough mode") and map that to model selection.

```rust
fn estimate_complexity(user_message: &str) -> TaskComplexity {
    let complex_keywords = ["refactor", "redesign", "architecture", "migrate",
                            "rewrite", "optimize across", "multi-file"];
    let simple_keywords = ["typo", "rename", "update version", "add import",
                           "fix indent", "format"];

    let msg_lower = user_message.to_lowercase();

    if complex_keywords.iter().any(|k| msg_lower.contains(k)) {
        TaskComplexity::Complex
    } else if simple_keywords.iter().any(|k| msg_lower.contains(k)) {
        TaskComplexity::Simple
    } else {
        TaskComplexity::Medium
    }
}
```

::: python Coming from Python
Python's OpenAI and Anthropic SDKs make it trivial to switch models -- just change the `model` parameter. In Rust, you set the model string in your request struct. The more interesting challenge in both languages is building the routing logic that decides which model to use. The Rust type system helps here -- using an enum for complexity levels ensures every case is handled in the match statement.
:::

## Trade-off Analysis

Here is a decision framework for common scenarios:

| Scenario | Recommended Model | Reasoning |
|---|---|---|
| Interactive coding session | Claude Sonnet 4 / GPT-4o | Balance of speed, capability, and cost |
| Quick file operations | Claude 3.5 Haiku / GPT-4o mini | Speed matters, task is simple |
| Complex debugging | Claude Opus 4 | Deep reasoning needed |
| Large codebase analysis | GPT-4.1 / Claude with caching | Large context window needed |
| CI/CD integration (batch) | Claude 3.5 Haiku | Cost matters, speed is secondary |
| Planning phase | Claude Sonnet 4 | Good reasoning at reasonable cost |
| Code review | Claude Sonnet 4 | Needs code understanding + instruction following |

## Multi-Provider Strategy

Supporting multiple providers gives you resilience and flexibility:

**Failover:** If Anthropic's API is overloaded (HTTP 529), fall back to OpenAI.

**Cost arbitrage:** Use whichever provider is cheaper for the specific model tier you need.

**Capability matching:** Some providers are stronger for certain languages or frameworks. You can route based on the detected language of the project.

Implementing this requires the provider abstraction discussed in [Message Formats](/linear/03-understanding-llms/06-message-formats) and [API Anatomy](/linear/03-understanding-llms/11-api-anatomy-anthropic). Your agent stores conversation history in a provider-agnostic format and serializes to the appropriate API format at call time.

```rust
struct ModelConfig {
    provider: Provider,
    model_id: String,
    max_tokens: u32,
    temperature: f32,
}

fn get_model_config(complexity: TaskComplexity, primary: Provider) -> ModelConfig {
    match (complexity, primary) {
        (TaskComplexity::Simple, Provider::Anthropic) => ModelConfig {
            provider: Provider::Anthropic,
            model_id: "claude-3-5-haiku-20241022".to_string(),
            max_tokens: 4096,
            temperature: 0.0,
        },
        (TaskComplexity::Medium, Provider::Anthropic) => ModelConfig {
            provider: Provider::Anthropic,
            model_id: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 8192,
            temperature: 0.0,
        },
        // ... other combinations
        _ => default_config(),
    }
}
```

::: wild In the Wild
Claude Code naturally uses Anthropic's Claude models, but OpenCode supports multiple providers through a provider abstraction layer. Users can configure their preferred provider and model, and the agent handles the differences in API format transparently. Some open-source agents implement automatic failover -- if the primary provider returns an error, the agent retries with a secondary provider. This resilience is important for agents used in professional settings where downtime is unacceptable.
:::

## Benchmarking Your Agent

Rather than relying on general model benchmarks, create a test suite specific to your agent's use cases:

1. **Define 10-20 representative tasks** covering the range of operations your agent handles
2. **Run each task with each candidate model** and record: success rate, token usage, latency, cost
3. **Weight the results** by how frequently each task type occurs in real usage
4. **Re-evaluate periodically** as new models are released

This empirical approach is more reliable than general benchmarks because it accounts for your specific tool definitions, system prompt, and use patterns. A model that scores well on HumanEval might perform poorly with your particular tool use protocol.

## Key Takeaways

- Tool use reliability is the most critical dimension for agent model selection -- a model that generates malformed tool calls or ignores tools breaks the entire agent loop
- The Claude Sonnet tier (3.5 or 4) and GPT-4o represent the sweet spot for most coding agent operations, balancing capability, speed, and cost
- Model routing -- using different models for different task complexities -- optimizes cost without sacrificing capability where it matters, and can be implemented with keyword heuristics, turn-based escalation, or user preference
- Multi-provider support provides failover resilience and cost flexibility, but requires a provider-agnostic message format and serialization layer
- Build a task-specific benchmark suite rather than relying on general model benchmarks, and re-evaluate with each new model release
