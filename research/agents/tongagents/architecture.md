# TongAgents — Architecture

> ⚠️ **No source code is publicly available.** Everything in this file is inferred from benchmark results, the agent's name, and BIGAI's broader research portfolio. Speculative claims are clearly marked.

## What We Can Infer

### Multi-Agent System (High Confidence)

The "Agents" suffix (plural) strongly suggests a **multi-agent architecture** rather than a single-agent loop. This is consistent with:

- BIGAI's extensive research on multi-agent systems and coordination
- The institute's work on cognitive architectures that decompose tasks into specialized modules
- The naming convention: "TongAgents" not "TongAgent" or "TongCoder"

A plausible architecture would involve specialized agents for different task phases — planning, execution, verification — coordinated by an orchestrator.

### Model-Agnostic Design (High Confidence)

TongAgents was tested with both Gemini 3.1 Pro and Claude Opus 4.6, achieving strong results with both. This indicates:

- The system is not hard-coded to a specific model's API or capabilities
- Prompts and tool interfaces are likely abstracted behind a model-neutral layer
- The agent framework handles model-specific details (token limits, tool calling formats) separately from task logic

### Cognitive Architecture Influence (Moderate Confidence)

BIGAI's core research theme is **cognitive architecture** — building AI systems that mirror human cognitive processes. It would be surprising if TongAgents did *not* incorporate ideas from this research:

- **Hierarchical task decomposition** — breaking complex terminal tasks into subtasks
- **Planning before execution** — generating a plan before running commands
- **Self-monitoring** — evaluating progress and adjusting strategy
- **Memory/knowledge management** — maintaining structured context about the task state

### Performance Characteristics

The benchmark results reveal architectural clues:

- **80.2% with Gemini 3.1 Pro** (Rank #3) — very strong, near the top of the leaderboard
- **~71.9% with Claude Opus 4.6** (Rank #13) — still solid but notably lower
- The **~8pp gap** suggests the architecture may rely on capabilities where Gemini excels (potentially: longer context utilization, structured output, or specific reasoning patterns)

## Speculative Architecture Diagram

```
┌─────────────────────────────────────────┐
│            Orchestrator Agent            │
│  (task decomposition, plan management)  │
├─────────────┬───────────┬───────────────┤
│  Planning   │ Execution │  Verification │
│   Agent     │   Agent   │    Agent      │
│             │           │               │
│ - Analyze   │ - Run     │ - Check       │
│   task      │   commands│   output      │
│ - Generate  │ - Handle  │ - Validate    │
│   plan      │   errors  │   state       │
└─────────────┴───────────┴───────────────┘
         │           │            │
         └───────────┼────────────┘
                     ▼
          ┌─────────────────────┐
          │   Tool Interface    │
          │  (shell, files,     │
          │   model-agnostic)   │
          └─────────────────────┘
```

*This diagram is entirely speculative and based on common multi-agent patterns combined with BIGAI's research focus.*

## What We Don't Know

- The actual number and roles of agents in the system
- How agents communicate (shared memory? message passing? blackboard?)
- Whether the system uses fine-tuned models or relies entirely on prompting
- The specific prompt engineering techniques employed
- Whether there is any training data or few-shot examples specific to terminal tasks
- How the system handles the different tool-calling conventions across models

## Comparison to Known Architectures

Based on the multi-agent hypothesis, TongAgents likely shares structural similarities with:

- **CAMEL** — multi-agent conversation framework (but more task-specialized)
- **AutoGen** — multi-agent orchestration (but potentially with more cognitive structure)
- **MetaGPT** — role-based multi-agent system (another Chinese AI project with similar philosophy)

The key differentiator would be BIGAI's cognitive architecture influence, potentially making the agent decomposition more principled than ad-hoc role assignment.