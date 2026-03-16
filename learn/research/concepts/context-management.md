# Context Management

## Overview

Context management is the set of strategies an agent uses to stay within the model's context window while preserving information needed to complete tasks. It matters for cost and quality. Each agentic loop iteration appends tool calls and results, so input tokens grow with every LLM invocation -- a 10-iteration loop can cost 30x a single call. Exceeding the context window causes API rejection; filling it too full degrades attention to relevant information. Context management threads the needle between running out of space and losing track of what matters.

## The Pattern

Context management operates as a budget system with three layers: counting, budgeting, and compaction.

**Token counting** determines how much space the current conversation occupies. The most accurate approach uses the provider's tokenizer library (like `tiktoken` for OpenAI or Anthropic's token counting endpoint). When exact counting is too expensive, agents use estimation heuristics -- roughly 4 characters per token for English, 3.5 for code. Production agents combine both: exact counts from the API's `usage` field in responses, supplemented by estimates for new messages added between calls. The count must cover all components: system prompt, tool definitions, message history, and framing overhead.

**Context window budgeting** allocates the fixed capacity across competing needs. A 200,000-token model typically reserves 8,192 tokens for the response, 500 as a safety margin, and the remainder for input. Within that input budget, the system prompt and tool definitions are fixed costs. When they take nearly 10% of the budget before the first message, monitoring this overhead matters.

**Compaction** reduces the conversation's token footprint when the budget is exhausted. A three-threshold policy governs timing: a soft threshold (around 75%) signals compaction at the next convenient point, a hard threshold (around 90%) forces immediate compaction, and a target threshold (around 50%) defines where utilization should land after compaction to prevent thrashing.

## Implementation Approaches

**Sliding window compaction** is the simplest algorithm. It keeps the N most recent messages and drops everything older, preserving the system prompt. This works because conversations have temporal locality -- recent messages are almost always more relevant than old ones. It is fast (O(n) in removed messages), never fails, and is trivial to implement. Its weakness is that it is content-blind: a critical architecture decision from 30 turns ago is just as expendable as a "yes, that looks good" from the same era.

**Importance-scored compaction** ranks messages by heuristics and removes the least important first. Scoring combines recency, role (system prompts untouchable, tool results expendable), content patterns (error messages valuable, large file dumps not), and token cost (removing one 5,000-token message beats removing fifty 100-token messages). This preserves critical context regardless of age but requires tuning the heuristics.

**Semantic deduplication** identifies and removes redundant content. Coding agents repeatedly read the same files; each read produces a near-identical tool output. Deduplication detects these groups using similarity measures (such as Jaccard similarity on line sets) and replaces each with only the most recent occurrence. A 200-line file appearing five times can be reduced from 5,000+ tokens to around 1,000 with zero information loss.

**Summarization-based compaction** uses an LLM call to condense older conversation segments into a compact summary preserving key decisions and current state. This is the highest-quality strategy but the most expensive and slowest. It works best at the soft threshold rather than as an emergency measure.

**Hybrid compaction** combines strategies in order of increasing aggressiveness. First, remove semantic duplicates (zero information loss). Then, apply importance scoring to remove low-value messages (minimal loss). Finally, use the sliding window as a last resort (highest loss but guaranteed to free space). This layered approach gives the best balance between context quality and space efficiency.

**Session persistence and restoration** enables conversations to survive process restarts. Sessions are stored as JSON Lines (JSONL) files where each line is a self-contained message, paired with a metadata file. JSONL is ideal because appending a message is a single operation, and a crash mid-write loses at most one incomplete line. On resume, the agent validates loaded data (role alternation, tool call pairing), repairs inconsistencies, and handles schema migrations. Atomic writes (write to temp, sync, rename) protect metadata from corruption.

## Key Considerations

**Response reserve is easily overlooked.** If the agent fills the context window to 100% with input tokens, the model has zero space for its response. Reserving a fixed 8,192 tokens is a common default (roughly 400 lines of code), but adaptive reserves that track the 90th percentile of recent response sizes avoid wasting space on over-provisioned budgets during turns that only need short answers.

**Tool call/result pairing must survive compaction.** Removing a tool call without its corresponding result (or vice versa) creates an invalid message sequence that confuses the model. Compaction algorithms must treat tool call/result pairs as atomic units, removing or keeping them together.

**The pre-call check is non-negotiable.** Every LLM invocation must pass through a budget check that either proceeds, triggers compaction, or rejects the call if the request cannot fit even after compaction. The reject case handles when the system prompt and tool definitions alone consume so much of the window that deleting all conversation messages would not help.

**Cost optimization compounds.** Aggressive compaction (targeting 30% utilization) keeps costs low but risks losing context. Conservative compaction (targeting 65%) preserves context but lets costs grow. The right setting depends on the task -- short Q&A tasks benefit from aggressive compaction, while long refactoring sessions need every bit of context they can get.

## Cross-References
- [Context Window Management](/linear/10-conversation-state-machines/04-context-window-management) -- Token budgeting, compaction thresholds, and pre-call checks
- [Compaction Algorithms](/linear/10-conversation-state-machines/05-compaction-algorithms) -- Sliding window, importance scoring, and semantic deduplication
- [Summarization Techniques](/linear/10-conversation-state-machines/06-summarization-techniques) -- LLM-based conversation summarization
- [Session Persistence](/linear/10-conversation-state-machines/07-session-persistence) -- Saving and restoring sessions across restarts
- [Token Counting Strategies](/linear/10-conversation-state-machines/03-token-counting-strategies) -- Exact vs. estimated token counting approaches
- [Cost Optimization](/linear/10-conversation-state-machines/13-cost-optimization) -- Controlling costs through context management
