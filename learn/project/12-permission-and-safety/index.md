---
title: "Chapter 12: Permission and Safety"
description: Comprehensive safety architecture for coding agents, covering threat models, permission systems, sandboxing, and audit trails.
---

# Permission and Safety

A coding agent that can read files, execute shell commands, and modify a codebase is inherently powerful and inherently dangerous. This chapter builds the safety infrastructure that makes that power trustworthy. You will design and implement a layered defense system that prevents the agent from performing harmful operations while keeping it productive for legitimate tasks.

The chapter begins with threat modeling specific to coding agents — understanding what can go wrong when an LLM has access to a filesystem and a shell. From there, you will implement permission levels that categorize operations by risk, an approval system that gates dangerous actions on human confirmation, and file checkpoints that enable reliable undo. You will also build allowlists and denylists for command filtering, a plan mode for previewing changes before execution, dangerous operation detection heuristics, and sandboxing to contain agent operations within safe boundaries. The chapter concludes with audit logging for accountability and strategies for testing that your safety systems actually work.

Safety is not a single feature but a philosophy that permeates the entire agent. By the end of this chapter, your agent will have defense in depth: multiple independent safety layers that each catch different categories of risk, ensuring that no single failure can lead to catastrophic outcomes.

## Learning Objectives
- Design a threat model specific to LLM-powered coding agents
- Implement a tiered permission system that categorizes operations by risk level
- Build an approval workflow that pauses for human confirmation on dangerous actions
- Create file checkpoints and undo capabilities for reliable rollback
- Construct allowlists and denylists for filtering shell commands and file paths
- Implement plan mode for previewing changes before execution
- Detect dangerous operations through pattern matching and heuristic scoring
- Apply sandboxing techniques to constrain filesystem and network access
- Record structured audit logs for debugging and post-incident analysis
- Test safety systems with red-team scenarios and property-based testing

## Subchapters
1. [Threat Model](/project/12-permission-and-safety/01-threat-model)
2. [Permission Levels](/project/12-permission-and-safety/02-permission-levels)
3. [Approval System](/project/12-permission-and-safety/03-approval-system)
4. [File Checkpoints](/project/12-permission-and-safety/04-file-checkpoints)
5. [Undo and Revert](/project/12-permission-and-safety/05-undo-revert)
6. [Allowlists and Denylists](/project/12-permission-and-safety/06-allowlists-denylists)
7. [Plan Mode](/project/12-permission-and-safety/07-plan-mode)
8. [Dangerous Operation Detection](/project/12-permission-and-safety/08-dangerous-operation-detection)
9. [Sandboxing Deep Dive](/project/12-permission-and-safety/09-sandboxing-deep-dive)
10. [Audit Logging](/project/12-permission-and-safety/10-audit-logging)
11. [Testing Safety](/project/12-permission-and-safety/11-testing-safety)
12. [Summary](/project/12-permission-and-safety/12-summary)

## Prerequisites
- Chapter 6: Shell execution with process spawning, timeouts, and basic command filtering
- Chapter 11: Git integration for repository operations, checkpointing, and safe rollback
- Familiarity with Rust enums, traits, and the `Result` type
