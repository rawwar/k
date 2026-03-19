# Context Management

> mini-SWE-agent's context strategy is radical in its simplicity: append everything, truncate nothing, summarize nothing.

## Linear History -- The Anti-Pattern That Works

Most agent frameworks implement sophisticated context management:
- **Sliding windows** that drop old messages
- **Summarization** that compresses past interactions
- **Retrieval** that selectively pulls relevant context
- **Priority queues** that rank message importance

mini-SWE-agent does **none of this**. Every message -- system prompt, task description, assistant reasoning, tool outputs -- is appended to a single list and passed in its entirety to the LM at every step.

```python
def add_messages(self, *messages: dict) -> list[dict]:
    self.messages.extend(messages)
    return list(messages)
```

Messages go in. They never come out.

## Why This Works

### 1. Modern Context Windows Are Enormous

When mini-SWE-agent was designed (2025), frontier models offered 128K-200K+ token context windows. A typical SWE-bench trajectory of 30-40 steps generates roughly 30K-60K tokens -- well within the window of any modern model.

### 2. No Information Loss

History compaction always loses something. A summary might omit the exact error message the LM needs. A sliding window might drop the file listing that showed the project structure. Linear history means the LM always has complete information.

### 3. Trajectory = Prompt = Training Data

Because the message list IS the prompt, and the prompt IS the trajectory:
- **Debugging**: Open the saved JSON, and you see exactly what the LM saw
- **Fine-tuning**: The trajectory is already in the correct format for SFT
- **RL training**: Reward signals map directly to trajectory steps
- **Reproducibility**: Re-running with the same model and trajectory prefix reproduces behavior

This property is broken the moment you add summarization or compaction -- the saved trajectory no longer matches what the LM actually saw.

### 4. No Compaction Bugs

History compaction is a common source of agent bugs:
- Summarizer hallucinates or omits critical details
- Window boundary cuts a multi-part message
- Priority scoring misjudges relevance
- Compaction changes message roles or structure

mini-SWE-agent sidesteps all of these failure modes.

## Output Truncation -- The One Concession

The only place where content is modified is in the **observation template** for long command outputs. If output exceeds 10,000 characters, the template shows head + tail with an elision notice:

```yaml
{% if output.output | length < 10000 %}
<output>{{ output.output }}</output>
{% else %}
<warning>
The output of your last command was too long.
Please try a different command that produces less output.
</warning>
<output_head>{{ output.output[:5000] }}</output_head>
<elided_chars>{{ elided_chars }} characters elided</elided_chars>
<output_tail>{{ output.output[-5000:] }}</output_tail>
{% endif %}
```

Key points about this truncation:
- It happens at the **observation level**, not the history level
- The truncation is **transparent** -- the LM sees the warning and knows content was elided
- It **encourages better behavior** -- the warning tells the LM to use more targeted commands
- It preserves **head and tail** -- usually the most informative parts of long output

## Environment Variables for Output Control

The default configuration sets environment variables that suppress pagers and progress bars:

```yaml
environment:
  env:
    PAGER: cat
    MANPAGER: cat
    LESS: -R
    PIP_PROGRESS_BAR: 'off'
    TQDM_DISABLE: '1'
```

This prevents commands from hanging (waiting for pager input in a non-interactive subprocess) and reduces noise in the output.

## Context Window Budget

A rough breakdown of token usage in a typical SWE-bench run:

| Component | Tokens (approx.) | Notes |
|-----------|-------------------|-------|
| System prompt | ~800 | Fixed; includes workflow guidance |
| Task description | ~200-2000 | Varies by issue complexity |
| Per step (assistant) | ~200-500 | THOUGHT + command |
| Per step (observation) | ~100-2000 | Depends on command output |
| **Total (30 steps)** | **~10K-50K** | Well within 128K+ windows |

## Comparison with Other Approaches

| Agent | Context Strategy | Trade-off |
|-------|-----------------|-----------|
| **mini-SWE-agent** | Linear (no compaction) | Simple, faithful; uses more tokens per call |
| **SWE-agent** | History processors (summarize, truncate) | Saves tokens; loses information fidelity |
| **Claude Code** | Sliding window + summarization | Handles very long sessions; complex implementation |
| **Devin** | Multi-agent with shared memory | Sophisticated; more failure modes |
| **OpenHands** | Configurable history management | Flexible; more complexity |

## When Linear History Breaks Down

Linear history has known limitations:

1. **Very long tasks** (100+ steps) -- may hit context window limits even with 200K token models
2. **Large file outputs** -- even with truncation, repeatedly viewing large files consumes significant tokens
3. **Multi-file exploration** -- browsing many files can fill context with potentially irrelevant content

For SWE-bench tasks, these limitations rarely matter -- most issues are solved in 20-40 steps. For longer tasks (e.g., multi-hour autonomous coding), linear history would eventually need augmentation.

The mini-SWE-agent team's position is clear: **start with linear history, and only add compaction if you empirically need it.** For their target use cases, they haven't needed it yet.

## Implications for Fine-Tuning and RL

Linear history is a significant advantage for training:

1. **Direct SFT data**: Each trajectory is a complete training example -- `messages[:-1]` is input, `messages[-1]` is target
2. **No distribution shift**: The training data matches the inference-time prompt format exactly
3. **Step-level attribution**: You can assign rewards to individual steps without worrying about compaction changing the context
4. **Easy filtering**: Bad trajectories (wrong answer, timeout) can be identified and excluded without complex analysis

This is why mini-SWE-agent is specifically recommended for "FT or RL" use cases -- the linear history design makes training data collection trivial.
