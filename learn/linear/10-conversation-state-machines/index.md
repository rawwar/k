---
title: "Chapter 10: Conversation State Machines"
description: Designing conversation state management with message history, token counting, compaction, persistence, and cost optimization for long-running agent sessions.
---

# Conversation State Machines

This chapter addresses one of the most complex challenges in building a coding agent: managing conversation state over time. Unlike a simple chatbot with short exchanges, a coding agent maintains long-running sessions where the conversation history grows to include thousands of messages spanning code changes, tool outputs, error logs, and reasoning traces. Managing this state correctly is essential for the agent to remain coherent, stay within context window limits, and avoid runaway costs.

We model conversation as a state machine where each message transition can trigger context window evaluation, compaction decisions, and persistence operations. You will learn how to count tokens accurately across different models, implement compaction algorithms that preserve the most relevant context while shedding redundant information, and design summarization techniques that maintain semantic fidelity. These are the same problems that production agents like Claude Code must solve.

The chapter also explores advanced conversation patterns: branching conversations for exploring alternatives, multi-agent conversations where multiple LLMs collaborate, and session persistence that allows conversations to resume across process restarts. We close with cost optimization strategies that help users manage API spending without sacrificing conversation quality.

## Learning Objectives
- Model conversation as a state machine with well-defined transitions and invariants
- Design message history data structures that efficiently support insertion, truncation, and search
- Implement accurate token counting using tiktoken and model-specific tokenizers
- Build compaction algorithms that respect context window limits while preserving essential context
- Persist conversation sessions to disk with storage formats that support efficient resume and search
- Apply cost optimization techniques including message pruning, caching, and model routing

## Subchapters
1. [Conversation as State](/linear/10-conversation-state-machines/01-conversation-as-state)
2. [Message History Design](/linear/10-conversation-state-machines/02-message-history-design)
3. [Token Counting Strategies](/linear/10-conversation-state-machines/03-token-counting-strategies)
4. [Context Window Management](/linear/10-conversation-state-machines/04-context-window-management)
5. [Compaction Algorithms](/linear/10-conversation-state-machines/05-compaction-algorithms)
6. [Summarization Techniques](/linear/10-conversation-state-machines/06-summarization-techniques)
7. [Session Persistence](/linear/10-conversation-state-machines/07-session-persistence)
8. [Storage Formats](/linear/10-conversation-state-machines/08-storage-formats)
9. [Branching Conversations](/linear/10-conversation-state-machines/09-branching-conversations)
10. [Multi Agent Conversations](/linear/10-conversation-state-machines/10-multi-agent-conversations)
11. [System Prompt Evolution](/linear/10-conversation-state-machines/11-system-prompt-evolution)
12. [Memory Patterns](/linear/10-conversation-state-machines/12-memory-patterns)
13. [Cost Optimization](/linear/10-conversation-state-machines/13-cost-optimization)
14. [Summary](/linear/10-conversation-state-machines/14-summary)

## Prerequisites
- Chapter 4 (agentic loop with conversation state and the fundamental request/response cycle)
- Chapter 3 (understanding of tokens, context windows, and message formats)
- Familiarity with Rust enums, structs, and trait-based design from Chapter 2
