# Sliding Window & Truncation Strategies

## 1. Introduction

The simplest family of context compaction strategies operates on a single principle:
drop or truncate content based on its position in the conversation or its age.
These strategies require **zero LLM calls**, introduce **near-zero latency**, and
are trivial to implement. Every production coding agent uses at least one of them.

The fundamental trade-off is stark: **high information loss in exchange for
simplicity and speed**. A sliding window doesn't understand what it's discarding.
It cannot distinguish a critical architectural decision from a routine `ls` output,
relying solely on recency as a proxy for relevance.

Despite this, position-based strategies form the backbone of context management.
They are the first line of defense against context overflow, and more sophisticated
strategies (summarization, semantic compression) are layered on top rather than
replacing them. This document surveys the major variants.

---

## 2. Simple Sliding Window

The most basic approach: maintain a fixed-size window of recent messages and
discard everything older.

**Mechanism:**
- Define a window size `W` (measured in messages or tokens).
- When the conversation exceeds `W`, drop the oldest messages until it fits.
- The window slides forward as new messages arrive.

**Implementations:** LangChain's `ConversationBufferWindowMemory` with configurable
`k` (number of interaction pairs to retain). Most chat UIs that pass "the last N
messages" to the API.

**Configuration choices:**
- **Message count**: simple but imprecise — one message might be 10 tokens or 10,000.
- **Token count**: accurate budget control but requires a tokenizer pass.
- **Turn count**: each user-assistant pair counts as one turn. Good middle ground.

**The critical problem:** you lose ALL context about what happened before the window.
The agent cannot recall original instructions, early decisions, or files it modified
twenty turns ago — leading to repeated mistakes, contradicted decisions, and lost
track of the original user request.

---

## 3. Anchor-Preserving Window

**Key insight:** the initial context — system prompt, first user message, setup
instructions — should almost **never** be dropped. These messages define the entire
task and agent behavior.

OpenHands implements this in `ConversationWindowCondenser`:

```python
class ConversationWindowCondenser(RollingCondenser):
    max_events: int = 100
    keep_first: int = 5   # Preserve initial setup/instructions

    def condense(self, events):
        if len(events) <= self.max_events:
            return events
        # Keep first K events + last (max - K) events; forget the middle
        head = events[:self.keep_first]
        tail = events[-(self.max_events - self.keep_first):]
        return head + tail
```

**The "first K + last N" pattern:** preserve bookends, drop the middle.

**Why the middle is safe(r) to drop:** "Lost in the middle" research (Liu et al.,
2023) showed LLMs pay most attention to context beginnings and endings. Information
buried in the middle is already partially "lost" even when present. Dropping it
removes content the model was least likely to leverage.

The `keep_first` parameter typically captures: system prompt (1 event), initial
user request (1 event), and environment setup (2-3 events). This small anchor
provides remarkable continuity — the agent retains awareness of its task even as
intermediate work scrolls out of the window.

---

## 4. Recent Events Only

OpenHands provides `RecentEventsCondenser` — even simpler:

```python
class RecentEventsCondenser(RollingCondenser):
    keep_events: int = 50

    def condense(self, events):
        if len(events) <= self.keep_events:
            return events
        return events[-self.keep_events:]
```

No anchoring. No bookends. Just the last N events.

**Best for:** short sessions where the task fits within the window, repetitive tasks
where earlier context is irrelevant, stateless tool-use patterns.
**Risk:** losing initial instructions entirely. If the system prompt scrolls out,
the agent drifts from its objective. Unsuitable for long-running autonomous sessions.

---

## 5. Amortized Forgetting

OpenHands' `AmortizedForgettingCondenser` probabilistically retains events with a
bias toward recency — instead of a sharp cutoff, it gradually forgets.

- Each event gets a retention probability based on its age.
- More recent events have higher probability of being kept.
- On each compaction pass, events are independently sampled for retention.
- Older events are gradually forgotten, not abruptly discarded.

```python
class AmortizedForgettingCondenser(RollingCondenser):
    max_events: int = 100
    keep_first: int = 5
    # Each non-anchored event survives with probability
    # proportional to its recency rank.
```

**Retention probability vs age:**

```
Retention Prob.
1.0 |                                      ************
0.8 |                            **********
0.6 |                  **********
0.4 |        **********
0.2 |********
0.0 +------------------------------------------------->
    Oldest                                    Newest
```

**Benefits:** smoother degradation (no sharp cliff), probabilistic coverage (some
old events survive), and repeated compaction as implicit importance filtering.
**Trade-off:** non-deterministic — same conversation may produce different compacted
histories on different runs, complicating debugging and reproducibility.

---

## 6. Observation Masking

A strategy operating on **content** rather than **position**: replace verbose
observation payloads with compact structural placeholders.

**Before masking** (~2,000 tokens):
```
[CmdOutputObservation]
$ cat src/auth/middleware.py
import jwt
from functools import wraps
from flask import request, jsonify
... (500 lines of Python code) ...
def verify_token(token):
    try:
        payload = jwt.decode(token, SECRET_KEY, algorithms=['HS256'])
        return payload
    except jwt.ExpiredSignatureError:
        return None
```

**After masking** (~15 tokens):
```
[Previous observation: CmdOutputObservation (cat src/auth/middleware.py) - truncated]
```

**Preserved:** what command ran, output type, chronological position.
**Removed:** actual file contents, command outputs, search results, stack traces.

**BrowserOutputCondenser** is a specialized variant for web content — browser
observations (full page DOM, screenshot descriptions) are among the largest payloads.

Zero LLM cost, significant token savings, and the conversation's structural skeleton
remains intact. The agent knows "I read that file" and can re-read if needed.

---

## 7. Output Truncation Strategies

Rather than dropping entire messages, truncation **trims individual outputs** to a
maximum size while preserving the most informative portions.

### Per-Item Truncation (Codex)

```rust
const TRUNCATION_LIMIT: usize = 10_000;  // ~10KB per output

fn truncate_output(output: &str, limit: usize) -> String {
    if output.len() <= limit {
        return output.to_string();
    }
    let prefix_size = limit * 2 / 3;   // ~67% from start
    let suffix_size = limit / 3;        // ~33% from end
    let truncated_chars = output.len() - prefix_size - suffix_size;
    let prefix = &output[..prefix_size];
    let suffix = &output[output.len() - suffix_size..];
    format!("{prefix}\n\n... {truncated_chars} chars truncated ...\n\n{suffix}")
}
```

**Why asymmetric (67/33)?** Error messages cluster at the end (compiler errors, stack
traces), while headers and structure appear at the start (signatures, imports). The
elision marker tells the model content was removed.

### Head + Tail (mini-SWE-agent)

```python
MAX_OUTPUT_CHARS = 10_000

def truncate(output: str) -> str:
    if len(output) <= MAX_OUTPUT_CHARS:
        return output
    half = MAX_OUTPUT_CHARS // 2
    head = output[:half]
    tail = output[-half:]
    warning = (
        "\n\n[Output truncated. "
        "Use more targeted commands (e.g., grep -n, head, tail) "
        "to get specific content.]\n\n"
    )
    return head + warning + tail
```

The **warning message** teaches the model to use more precise commands next time,
creating a feedback loop that reduces future truncation.

### Search Result Limits (ForgeCode)

```
FORGE_MAX_SEARCH_RESULT_BYTES = 10240  # 10KB default
```

Prevents a single `grep -r "TODO"` from flooding context with thousands of matches.
Applied at the tool level before results enter the conversation, encouraging more
specific search queries.

### Global Truncation Pass (Codex)

When total context exceeds the budget after per-item truncation, Codex iterates
over ALL function call outputs and truncates each by increasing amounts until the
total fits. A blunt emergency valve — rarely triggered if per-item limits are
well-calibrated, but prevents catastrophic overflow.

---

## 8. Hybrid Approaches

No production agent relies on a single strategy. The standard pattern is a
**pipeline of progressively more aggressive compaction**:

```
1. Per-item truncation     (always on, ~0 cost)
       ↓
2. Observation masking     (cheap, structural preservation)
       ↓
3. Sliding window          (medium: loses context)
       ↓
4. LLM summarization       (expensive: tokens + latency)
```

**OpenHands pipeline:**
1. `ObservationMasking` — replace old observation content with placeholders.
2. `ConversationWindowCondenser` — drop middle events, preserve anchors.
3. `LLMSummarizingCondenser` — LLM-generated summary as final fallback.

**Codex pipeline:**
1. Per-item truncation (10KB limit) on every tool output immediately.
2. Global truncation pass when total context approaches the limit.
3. Soft-cap rollover: new conversation with summary when context is exhausted.

The principle: **exhaust cheap strategies before spending on expensive ones**.

---

## 9. Anchor Points: Messages That Should Never Be Dropped

Certain messages carry disproportionate importance regardless of age:

- **System prompt:** defines agent behavior, tools, constraints. Losing this causes
  the agent to "forget what it is" — a catastrophic failure mode.
- **First user message:** the original task description. Without it, the agent
  cannot verify whether current work aligns with the user's intent.
- **Key decision points:** when the user approved/rejected an approach. "Don't use
  Redux, use Zustand" must persist even 50 turns later.
- **Error recovery records:** when the agent changed approach after a dead end.
- **File modification records:** what was changed and why.

**How to identify anchors:**

| Method | Approach | Trade-off |
|--------|----------|-----------|
| Positional | First K messages always kept | Simple but inflexible |
| Role-based | System/user messages preserved, tool outputs trimmed | Reasonable default |
| Content-based | Messages with keywords ("constraint", "don't") flagged | Fragile heuristic |
| LLM-scored | LLM rates importance of each message | Expensive, adds latency |

Production default: **positional anchoring** (keep first K) combined with
**role-based priority** (prefer user messages over tool outputs when compacting).

---

## 10. Comparison Table

| Strategy | Info Loss | LLM Cost | Latency | Preserves Anchors | Best For |
|---|---|---|---|---|---|
| Simple window | High | None | ~0ms | No | Ephemeral chat sessions |
| Anchor-preserving | Medium | None | ~0ms | Yes | Long autonomous sessions |
| Recent events | High | None | ~0ms | No | Short, focused tasks |
| Amortized forgetting | Medium | None | ~0ms | Configurable | Balanced long sessions |
| Observation masking | Low-Medium | None | ~0ms | Yes | Tool-heavy workflows |
| Per-item truncation | Low | None | ~0ms | N/A | Large individual outputs |
| Head+tail split | Low | None | ~0ms | N/A | Command/build outputs |
| Global truncation | Medium | None | ~0ms | N/A | Emergency overflow |

Every strategy here has zero LLM cost — that is their defining characteristic.
The differences lie in information loss and whether initial context survives.

---

## 11. When to Use Sliding Window vs Summarization

```
                    Sliding Window          LLM Summarization
                    ──────────────          ─────────────────
Cost per use:       Free                    $0.001 - $0.05
Latency:            < 1ms                   500ms - 5s
Info preservation:  Low (positional)        High (semantic)
Deterministic:      Yes*                    No
Implementation:     Trivial                 Moderate
Failure modes:      Loses key context       Bad summary, hallucination
```
*Amortized forgetting is non-deterministic.

**Use sliding window when:**
- Sessions are short (< 30 turns) and recent context suffices.
- Cost sensitivity is high and every API call matters.
- Latency is critical (real-time interactive use).
- The task is repetitive and earlier context is genuinely irrelevant.

**Use LLM summarization when:**
- Sessions are long (50+ turns) with complex state evolution.
- Multiple interrelated decisions need to be preserved.
- The agent needs to recall *why* it made earlier choices.

**Use both (hybrid) when:**
- Building production agents handling variable-length sessions.
- Cheap strategies as first defense, summarization as fallback.
- Pipeline should degrade gracefully: fast/cheap for simple sessions,
  slower/thorough for complex ones.

The trend in production agents is clearly toward **hybrid pipelines** where
sliding window and truncation handle the common case while LLM summarization
activates only when cheap strategies cause unacceptable information loss.

---

## Summary

Sliding window and truncation strategies are the **workhorses** of context
management — not because they're sophisticated, but because they're fast, free,
and reliable. The art lies in composing a pipeline that applies the right strategy
at each stage of context pressure. The key design decisions are:
1. **Where to anchor**: which messages must survive all compaction.
2. **How to split**: asymmetric (67/33) vs symmetric (50/50) for truncation.
3. **When to escalate**: thresholds for moving from cheap to expensive strategies.
4. **What to preserve**: structure (masking) vs content (summarization).
