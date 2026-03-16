---
title: "Chapter 4: Anatomy of an Agentic Loop"
description: The core runtime pattern of every coding agent — from input processing through LLM invocation, tool dispatch, and observation collection.
---

# Anatomy of an Agentic Loop

The agentic loop is the beating heart of every coding agent. It is the runtime pattern that transforms a stateless language model into an autonomous system capable of multi-step reasoning and action. Understanding this loop in detail is the single most important concept in this entire tutorial track, because every feature you build later — tools, file operations, shell commands — plugs into this central loop.

This chapter dissects the agentic loop into its constituent phases: input processing, LLM invocation, tool call detection, tool dispatch, observation collection, and response generation. We model the loop as a state machine with well-defined transitions, making it possible to reason about error states, stop conditions, and recovery strategies. You will see how the abstract pattern maps to concrete implementations in production agents.

By the end of this chapter, you will have a complete mental model of the agentic loop and be ready to implement one in Rust. You will understand not just the happy path, but also the edge cases: what happens when a tool fails, when the model hallucinates a tool that does not exist, or when the conversation exceeds the context window mid-loop.

## Learning Objectives
- Model the agentic loop as a state machine with explicit states and transitions
- Identify the six phases of a single loop iteration and their responsibilities
- Understand how tool call detection bridges the LLM response and tool execution
- Design stop conditions that prevent infinite loops while allowing complex multi-step tasks
- Handle error states gracefully including tool failures, API errors, and context overflow
- Compare loop variants across production agents and understand their design trade-offs

## Subchapters
1. [The REPL Pattern](/linear/04-anatomy-of-an-agentic-loop/01-the-repl-pattern)
2. [From Chatbot to Agent](/linear/04-anatomy-of-an-agentic-loop/02-from-chatbot-to-agent)
3. [State Machine Model](/linear/04-anatomy-of-an-agentic-loop/03-state-machine-model)
4. [Input Processing](/linear/04-anatomy-of-an-agentic-loop/04-input-processing)
5. [LLM Invocation](/linear/04-anatomy-of-an-agentic-loop/05-llm-invocation)
6. [Tool Call Detection](/linear/04-anatomy-of-an-agentic-loop/06-tool-call-detection)
7. [Tool Dispatch](/linear/04-anatomy-of-an-agentic-loop/07-tool-dispatch)
8. [Observation Collection](/linear/04-anatomy-of-an-agentic-loop/08-observation-collection)
9. [Response Generation](/linear/04-anatomy-of-an-agentic-loop/09-response-generation)
10. [Stop Conditions](/linear/04-anatomy-of-an-agentic-loop/10-stop-conditions)
11. [Error States](/linear/04-anatomy-of-an-agentic-loop/11-error-states)
12. [Loop Variants](/linear/04-anatomy-of-an-agentic-loop/12-loop-variants)
13. [Real World Implementations](/linear/04-anatomy-of-an-agentic-loop/13-real-world-implementations)
14. [Summary](/linear/04-anatomy-of-an-agentic-loop/14-summary)

## Prerequisites
- Chapter 3 (understanding of LLM APIs and tool use)
