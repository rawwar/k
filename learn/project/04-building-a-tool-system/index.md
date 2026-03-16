---
title: "Chapter 4: Building a Tool System"
description: Design and implement a trait-based tool system with JSON schema definitions, a registry, and dispatch logic.
---

# Building a Tool System

This chapter gives your agent hands. The agentic loop from Chapter 3 can detect when the model wants to use a tool, but until now there was nothing to execute. Here you build the complete tool infrastructure: a Rust trait that every tool must implement, a JSON schema system that tells the model what tools are available and what arguments they accept, and a registry that dispatches tool calls to the right implementation at runtime.

The design is deliberately extensible. You define a `Tool` trait with methods for name, description, schema, and execution. Any struct that implements this trait can be registered in the tool registry and becomes immediately available to the model. This pattern means adding a new tool later -- a shell executor, a web search, a database query -- requires writing one struct and registering it, with no changes to the loop or dispatch logic.

You will also implement input validation against JSON schemas, proper error handling that distinguishes between tool failures and system errors, and the complete lifecycle of a tool call from the model's request through execution to the observation that feeds back into the conversation.

## Learning Objectives
- Design a Tool trait that defines the contract every tool must satisfy
- Generate JSON schema definitions that describe tool inputs to the model
- Build a tool registry that maps tool names to implementations
- Implement dispatch logic that routes tool calls and handles errors
- Validate tool inputs against their schemas before execution
- Understand the full tool lifecycle from request to observation

## Subchapters
1. [What Are Tools](/project/04-building-a-tool-system/01-what-are-tools)
2. [Tool Trait Design](/project/04-building-a-tool-system/02-tool-trait-design)
3. [JSON Schema Basics](/project/04-building-a-tool-system/03-json-schema-basics)
4. [Defining Tool Schemas](/project/04-building-a-tool-system/04-defining-tool-schemas)
5. [Tool Registry](/project/04-building-a-tool-system/05-tool-registry)
6. [Tool Dispatch](/project/04-building-a-tool-system/06-tool-dispatch)
7. [Input Validation](/project/04-building-a-tool-system/07-input-validation)
8. [Executing Tools](/project/04-building-a-tool-system/08-executing-tools)
9. [Error Handling](/project/04-building-a-tool-system/09-error-handling)
10. [Tool Results as Observations](/project/04-building-a-tool-system/10-tool-results-as-observations)
11. [Adding Tool Descriptions](/project/04-building-a-tool-system/11-adding-tool-descriptions)
12. [Testing Tools](/project/04-building-a-tool-system/12-testing-tools)
13. [The Tool Lifecycle](/project/04-building-a-tool-system/13-the-tool-lifecycle)
14. [Summary](/project/04-building-a-tool-system/14-summary)

## Prerequisites
- Chapter 3 completed (working agentic loop with tool-call detection and observation feeding)
- Understanding of Rust traits, dynamic dispatch, and serde serialization
