---
title: "Chapter 3: The Agentic Loop"
description: Implement the core loop pattern that transforms a simple chatbot into an autonomous coding agent.
---

# The Agentic Loop

This chapter introduces the most important architectural pattern in the entire book: the agentic loop. A chatbot sends one message and gets one reply. An agent, by contrast, runs a loop -- it sends a prompt, receives a response that may include tool-use requests, executes those tools, feeds the results back as observations, and repeats until the task is complete or a stop condition is met.

You will build this loop from scratch in Rust. The implementation starts simple -- a while loop with a match statement -- but the design decisions you make here ripple through every later chapter. You will learn how to manage conversation state as a growing vector of messages, how to detect when the model wants to call a tool versus when it wants to respond to the user, and how to decide when the loop should stop.

By the end of this chapter your agent will be able to carry on multi-turn conversations and, critically, it will have the scaffolding to execute tools in the next chapter. The loop itself is tool-agnostic; it simply knows that some assistant responses require action and observation before continuing.

## Learning Objectives
- Understand the prompt-call-execute-observe cycle that defines agentic behavior
- Implement a loop that alternates between LLM calls and tool execution
- Manage conversation state as a typed message history
- Distinguish between tool-use responses and end-of-turn responses
- Implement stop conditions based on turn count, token budget, and explicit stop signals
- Debug agentic loops by logging message flow and state transitions

## Subchapters
1. [What Is an Agentic Loop](/project/03-the-agentic-loop/01-what-is-an-agentic-loop)
2. [Loop Architecture](/project/03-the-agentic-loop/02-loop-architecture)
3. [Message Types](/project/03-the-agentic-loop/03-message-types)
4. [Conversation State](/project/03-the-agentic-loop/04-conversation-state)
5. [Turn Management](/project/03-the-agentic-loop/05-turn-management)
6. [Stop Conditions](/project/03-the-agentic-loop/06-stop-conditions)
7. [Implementing the Core Loop](/project/03-the-agentic-loop/07-implementing-the-core-loop)
8. [Handling Tool Calls](/project/03-the-agentic-loop/08-handling-tool-calls)
9. [Observation Feeding](/project/03-the-agentic-loop/09-observation-feeding)
10. [Single vs Multi Turn](/project/03-the-agentic-loop/10-single-vs-multi-turn)
11. [Debugging the Loop](/project/03-the-agentic-loop/11-debugging-the-loop)
12. [Summary](/project/03-the-agentic-loop/12-summary)

## Prerequisites
- Chapter 2 completed (working LLM API integration with typed request/response structs)
- Familiarity with Rust enums, pattern matching, and the Result type
