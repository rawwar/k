---
title: "Chapter 5: Tool Systems Deep Dive"
description: How agents interact with the world through tools — schema design, validation, execution models, composition, and security.
---

# Tool Systems Deep Dive

Tools are how a coding agent escapes the confines of text generation and actually does things in the world. A tool might read a file, execute a shell command, search a codebase, or write code to disk. The tool system is the bridge between the language model's intentions and real-world side effects. Getting this system right is what separates a useful agent from a dangerous one.

This chapter covers the complete tool lifecycle from definition to execution. We start with JSON Schema, the lingua franca for describing tool interfaces to language models. We then examine how to design tool descriptions that models understand reliably, how to validate inputs before execution, and how to handle the results. You will learn the difference between synchronous and asynchronous execution, how to compose simple tools into complex workflows, and how to implement security boundaries.

By the end of this chapter, you will have a thorough understanding of tool system design and be ready to implement specific tools in the following chapters. You will know not just how tools work mechanically, but how to design them so that language models use them correctly — a subtly different and critically important skill.

## Learning Objectives
- Design tool schemas using JSON Schema that language models can interpret accurately
- Write tool descriptions that minimize misuse and maximize correct invocations
- Implement validation strategies that catch invalid inputs before execution
- Choose between synchronous and asynchronous execution models based on tool characteristics
- Compose simple tools into higher-level operations while maintaining error transparency
- Apply security boundaries including sandboxing, permission models, and output sanitization

## Subchapters
1. [Why Tools Matter](/linear/05-tool-systems-deep-dive/01-why-tools-matter)
2. [JSON Schema Specification](/linear/05-tool-systems-deep-dive/02-json-schema-specification)
3. [Tool Description Design](/linear/05-tool-systems-deep-dive/03-tool-description-design)
4. [Validation Strategies](/linear/05-tool-systems-deep-dive/04-validation-strategies)
5. [Execution Models](/linear/05-tool-systems-deep-dive/05-execution-models)
6. [Sync vs Async Execution](/linear/05-tool-systems-deep-dive/06-sync-vs-async-execution)
7. [Error Propagation](/linear/05-tool-systems-deep-dive/07-error-propagation)
8. [Tool Result Formats](/linear/05-tool-systems-deep-dive/08-tool-result-formats)
9. [Tool Categories](/linear/05-tool-systems-deep-dive/09-tool-categories)
10. [Tool Composition](/linear/05-tool-systems-deep-dive/10-tool-composition)
11. [Tool Discovery](/linear/05-tool-systems-deep-dive/11-tool-discovery)
12. [Security Considerations](/linear/05-tool-systems-deep-dive/12-security-considerations)
13. [Designing for LLMs](/linear/05-tool-systems-deep-dive/13-designing-for-llms)
14. [Summary](/linear/05-tool-systems-deep-dive/14-summary)

## Prerequisites
- Chapter 4 (understanding the agentic loop)
