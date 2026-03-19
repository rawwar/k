# LLM-Based Conversation Summarization for Coding Agents

## 1. Introduction

Summarization is the most common compaction strategy employed by LLM-based coding agents today. When a conversation grows too long to fit within a model's context window—or when the cost of processing the full history becomes prohibitive—agents turn to the most intuitive solution: ask an LLM to produce a shorter version.

The core idea is deceptively simple. Take the older portion of a conversation, pass it to an LLM with instructions to summarize, and replace the verbose original with the compact result. Future turns see the summary instead of the full history, reclaiming context window space while (ideally) preserving the information needed to continue the task.

But this creates a fascinating meta-challenge: **using an LLM to compress context for another LLM**. The summarizer must understand what information the downstream model will need—which details are load-bearing and which are expendable. This is not general-purpose summarization; it is summarization in service of continued agentic reasoning. A detail that seems minor in isolation (a specific error code, an exact file path, a user's stated preference) can be the linchpin of future decisions. Losing it silently is worse than losing it loudly, because the agent proceeds with false confidence.

---

## 2. How Agents Use LLMs to Summarize Their Own History

### OpenCode's Approach

OpenCode takes the most straightforward approach. When context usage crosses a threshold (configurable, defaulting to 80%), it fires a summarization call with a focused prompt:

```
Provide a detailed but concise summary of our conversation above.
Focus on:
- What was done so far
- What we're currently working on
- Which files we're working on and their current state
- What we're going to do next
- Any important decisions or context that would be needed to continue
```

The summary message **replaces ALL prior history**. OpenCode stores a `summary_message_id` in its SQLite session store; everything before that ID is discarded from the active context. Subsequent turns start from the summary, not the full conversation:

```
Before:  [system] [user₁] [assistant₁] [user₂] [assistant₂] ... [userₙ] [assistantₙ]
After:   [system] [summary_message] [userₙ₊₁] ...
```

The system prompt is always preserved. The summary becomes the new conversational anchor. This is clean but irreversible—once summarized, the original messages are gone from the LLM's perspective.

### Claude Code's `/compact`

Claude Code offers both automatic compaction and a manual `/compact` command that accepts focus instructions:

```
/compact Focus on the API changes we discussed
/compact Keep the test results and error messages
/compact Preserve the database schema decisions
```

This directed compaction is a powerful UX pattern. The user knows what matters for their next steps and can guide the summarizer accordingly. Without direction, the LLM must guess what future turns will need.

A **key design decision**: after compaction, Claude Code re-reads `CLAUDE.md` from disk. This ensures that project-level instructions—coding conventions, architecture decisions, preferred patterns—are never lost through summarization. Even if the summary omits a convention, re-injecting the project configuration file restores it.

```
Compaction flow:
  1. Summarize conversation history
  2. Replace history with summary
  3. Re-read CLAUDE.md from disk        ← critical step
  4. Re-inject project instructions
  5. Continue with refreshed context
```

Most other agents do not replicate this pattern, risking the loss of project-level context through repeated compaction cycles. The insight: some information exists outside the conversation and should be re-sourced from its canonical location.

### OpenHands' Two LLM-Based Condensers

OpenHands provides two distinct summarization strategies:

**LLMSummarizingCondenser** produces a free-form summary of forgotten events:

```python
condenser_config:
  type: llm
  model: gpt-4o-mini        # cheaper model for summarization
  max_size: 100              # max events before triggering
  keep_first: 2              # preserve initial events (system setup)
```

**StructuredSummaryCondenser** produces categorized sections:

```python
condenser_config:
  type: structured_summary
  model: gpt-4o-mini
  sections:
    - "Key Findings"
    - "Files Modified"
    - "Current Approach"
    - "Open Issues"
    - "User Preferences"
```

The structured approach yields more organized summaries. When an agent needs to recall which files were modified, it can look at a dedicated section rather than parsing a narrative paragraph. The trade-off is rigidity—information may fit awkwardly into predefined categories.

### Droid's Ambitious Approach

Droid takes summarization further with incremental compression around anchor points:

```
Full fidelity:    [key decision] [key decision] [key decision]
                       |              |              |
Compressed:    ~~~compressed~~~  ~~~compressed~~~  ~~~compressed~~~
```

Key decisions are preserved at full fidelity. Surrounding context is compressed progressively. Each context piece carries a **confidence score** determining compression aggressiveness:

- **High confidence** (routine exploration): compress aggressively, one-line summary
- **Medium confidence** (implementation details): moderate compression, preserve key facts
- **Low confidence** (critical decisions, errors): preserve at full fidelity

This reportedly enables **multi-week continuous sessions**, far exceeding simple summarization.

### Goose's Tool-Pair Summarization

Goose summarizes individual **tool_call + tool_result pairs** in the background. Originals are preserved but marked invisible; summaries are inserted as visible replacements:

```rust
fn mark_invisible(message: &mut Message) {
    message.metadata.insert(
        "agent_visible".to_string(),
        serde_json::Value::Bool(false),
    );
}

fn insert_summary(messages: &mut Vec<Message>, summary: String, position: usize) {
    let summary_msg = Message {
        role: Role::System,
        content: Content::Text(summary),
        metadata: {
            let mut m = HashMap::new();
            m.insert("is_summary".to_string(), serde_json::Value::Bool(true));
            m
        },
    };
    messages.insert(position, summary_msg);
}
```

Goose operates at three levels of urgency:

| Level | Trigger | Threshold | Behavior |
|-------|---------|-----------|----------|
| **Proactive** | Context reaches 80% | Before overflow | Summarize oldest tool pairs |
| **Reactive** | Context overflow error | After API rejection | Emergency summarization, retry |
| **Background** | Every turn | Per-turn async | Pre-compute summaries for future use |

The background level pre-computes summaries asynchronously so they're ready when needed—swapping in pre-computed summaries instantly rather than making blocking LLM calls.

---

## 3. Prompt Engineering for Summarization

### What to Keep vs Discard

**Keep**: current task state, exact file paths, key decisions, specific error messages, environment details, pending actions.
**Discard**: intermediate exploration, failed attempts (unless informative), verbose tool output, conversational pleasantries, redundant information.

### Structured vs Free-Form Prompts

Free-form: `Summarize the conversation, focusing on what matters for continuing the coding task.`

Structured prompts consistently produce more useful summaries:

```
Summarize the conversation in these sections:
## Task — What the user wants to accomplish.
## Progress — What has been done so far.
## Current State — Files modified, current errors, where we left off.
## Next Steps — What we planned to do next.
## Key Decisions — Important choices made and their rationale.
```

### The "What Are We Doing Next" Pattern

Nearly every effective summarization prompt includes a forward-looking component. OpenCode asks for "what we're going to do next." This is critical because summarization triggers mid-task. Without forward-looking context, the agent faces a cold-start problem: it knows what was done but not what was planned, leading to redundant re-analysis.

### Junie's Approach

Junie preserves task context across model transitions. When switching between planning and execution phases—potentially using different models—it preserves task definition, acceptance criteria, and current progress as structured metadata rather than relying solely on conversational summary.

---

## 4. Multi-Pass Summarization

### When Summaries Themselves Get Too Long

For long-running tasks, the summary itself can grow to consume significant context. The naive approach—summarize the summary—works but compounds losses:

```
Pass 0: Full conversation (50K tokens)
Pass 1: Summary₁ + recent (12K tokens)
Pass 2: Summary₂ + recent (8K tokens)    ← fidelity degrading
Pass 3: Summary₃ + recent (6K tokens)    ← significant drift
```

This is analogous to repeatedly JPEG-compressing an image—each generation degrades quality.

### Droid's Incremental Approach

Instead of summarizing summaries, Droid maintains a single layer of compression with varying fidelity. When space is needed, the oldest compressed sections are compressed further while anchors are preserved. This avoids the telephone game because each pass works from original content around anchors, not from previous summaries.

### LangChain's Summary Buffer Memory

LangChain's `ConversationSummaryBufferMemory` keeps recent messages verbatim and maintains a running summary of older messages:

```python
class SummaryBufferMemory:
    def add_message(self, message):
        self.buffer.append(message)
        while token_count(self.buffer) > self.max_token_limit:
            oldest = self.buffer.pop(0)
            self.running_summary = self.llm.summarize(
                existing_summary=self.running_summary,
                new_content=oldest
            )
```

The summary is incrementally updated rather than regenerated. Each call is small (existing summary + few messages), keeping costs low. The risk: incremental updates can drift since the LLM never sees the full original context.

---

## 5. Quality vs Cost Trade-offs

Each summarization call has a token cost. For a conversation at 100K tokens summarized to 5K, the call costs ~100K input + 5K output tokens. A common optimization: use a cheaper model for summarization:

| Summarization Model | Cost per 100K→5K | Quality | Latency |
|---------------------|-------------------|---------|---------|
| GPT-4o | ~$0.28 | High | ~8s |
| GPT-4o-mini | ~$0.02 | Medium | ~3s |
| Claude 3.5 Sonnet | ~$0.33 | High | ~10s |
| Claude 3.5 Haiku | ~$0.04 | Medium | ~2s |
| Gemini 1.5 Flash | ~$0.01 | Medium-Low | ~2s |

OpenHands explicitly supports this: its condenser config accepts a `model` parameter independent of the main agent model. Cheaper models produce less reliable summaries—missing nuance, structural issues, increased hallucination risk, and mangled code identifiers. The practical sweet spot: summarize with a model one tier down from the main task model, reducing costs 80-90% with acceptable quality.

---

## 6. Risks and Failure Modes

### Information Loss
Summaries inevitably lose detail. A summary that drops a specific error message (`ECONNREFUSED on port 5432`) or an exact file path (`src/utils/auth/tokenRefresh.ts`) can silently derail subsequent reasoning. The agent doesn't know what it doesn't know.

**Mitigation**: Structured summaries with explicit categories for file paths and error messages. Reference-backed summarization—instruct the LLM to include identifiers verbatim.

### Hallucinated Summaries
The summarizing LLM can invent details not in the original conversation:

```
Original: "We discussed refactoring the auth module"
Hallucinated: "We decided to move auth from src/auth.ts to src/lib/auth/index.ts
               and the user approved this restructuring"
```

The agent now believes a specific plan was agreed upon when it was merely discussed. Particularly dangerous: hallucinated file paths, function names, or user approvals.

**Mitigation**: Include identifiers verbatim in prompts. Use structured output forcing explicit citation.

### Anchor Dilution
System prompts and initial instructions can be weakened through summarization. After 50 turns, "Always write tests first" might be summarized away entirely.

**Mitigations**: Claude Code re-reads `CLAUDE.md`. OpenHands' `keep_first: K` preserves initial events. Most agents have no explicit mitigation.

### Summary Drift
After multiple rounds, semantic content drifts from the original—the telephone game:

```
Round 1: "Add cursor-based pagination to the /users endpoint"
Round 2: "Adding pagination to the API"
Round 3: "Working on API improvements"
```

**Mitigation**: Periodic full resets. Track summarization depth, force fresh start after N rounds. Maintain a "core facts" section that is never summarized.

---

## 7. Implementation Patterns

### The Replace Pattern
Delete old messages, insert summary. OpenCode uses SQLite:

```sql
UPDATE sessions SET summary_message_id = ? WHERE id = ?;
SELECT * FROM messages WHERE session_id = ? AND (id >= ? OR role = 'system') ORDER BY created_at;
```

**Pro**: Simple, minimal memory. **Con**: Irreversible, no fallback.

### The Shadow Pattern
Keep originals marked invisible. Goose filters by visibility when building LLM requests:

```rust
fn build_llm_messages(history: &[Message]) -> Vec<LLMMessage> {
    history.iter()
        .filter(|msg| msg.metadata.get("agent_visible")
            .and_then(|v| v.as_bool()).unwrap_or(true))
        .map(|msg| msg.to_llm_message())
        .collect()
}
```

**Pro**: Full history preserved, reversible. **Con**: Unbounded memory growth.

### The Checkpoint Pattern
Save state before summarization; restore if quality is poor. Gemini CLI's shadow git approach checkpoints the entire working state via git branches.

**Pro**: Safety net against poor summaries. **Con**: Storage overhead, quality assessment complexity.

---

## 8. Comparison Table

| Agent | Type | Model | Trigger | Preserves Anchors | Structured | Reversible |
|-------|------|-------|---------|-------------------|------------|------------|
| **OpenCode** | Full replace | Same as main | 80% context | System prompt only | No | No |
| **Claude Code** | Full replace | Same as main | Auto + `/compact` | Yes (re-reads CLAUDE.md) | User-directed | No |
| **OpenHands (LLM)** | Head/tail + summary | Configurable | Event count | Yes (`keep_first`) | No | No |
| **OpenHands (Structured)** | Head/tail + summary | Configurable | Event count | Yes (`keep_first`) | Yes | No |
| **Droid** | Incremental | Not disclosed | Continuous | Yes (anchors) | Partial | Partial |
| **Goose** | Tool-pair shadow | Background | 80% + async | System prompt | No | Yes |
| **Junie** | Task-preserving | Same as main | Phase transitions | Yes (metadata) | Yes | No |
| **Gemini CLI** | Replace + checkpoint | Same as main | Context threshold | System prompt | No | Yes |

**Key observations**: No agent uses one strategy for all situations. Anchor preservation is inconsistent—Claude Code's re-read is the gold standard. Structured summaries are underused despite clear benefits. Reversibility is rare.

---

## 9. Best Practices

**Always preserve system prompt and initial instructions.** Never include them in summarization input:

```python
def build_context(system_prompt, summary, recent_messages):
    return [
        {"role": "system", "content": system_prompt},   # never summarized
        {"role": "assistant", "content": summary},
        *recent_messages
    ]
```

**Include forward-looking context.** Every summarization prompt should capture what comes next:

```
Bad:  "We refactored the auth module and fixed three bugs."
Good: "We refactored the auth module and fixed three bugs.
       Next: implement rate limiting on /api/login using token bucket."
```

**Use structured summaries for complex tasks.** For multi-file, multi-step work, categorized sections prevent critical details from being buried in prose.

**Re-inject project configuration after compaction.** Follow Claude Code: re-read project config files from disk. The canonical source is the file system, not the conversation.

**Set conservative compaction thresholds.** Trigger at 70-80%, not 95%. Leave headroom for the summarization call itself and post-summarization turns.

**Monitor summary quality.** Log both originals and summaries during development. Review for dropped details, hallucinations, structural degradation, and drift. Automated metrics (ROUGE, BERTScore) measure textual similarity, not functional completeness—manual review remains essential.

**Consider hybrid approaches.** The best implementations combine summarization with other strategies: truncation for tool outputs, retrieval for specific details on demand, structured state objects that are updated rather than summarized. No single strategy handles all cases.
