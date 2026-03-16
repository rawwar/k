---
title: "Chapter 13: Safety and Permissions"
description: Designing robust safety systems and permission architectures for coding agents that operate on real codebases.
---

# Safety and Permissions

Coding agents wield real power: they read files, write code, execute commands, and interact with external services. With that power comes serious risk. A misconfigured agent can delete files, leak secrets, run destructive commands, or silently introduce vulnerabilities into a codebase. This chapter tackles the problem of making agents safe by design rather than safe by luck.

We begin with threat modeling specific to coding agents, identifying the unique attack surfaces and failure modes that arise when an LLM drives a development workflow. From there, we build layered permission architectures that control what an agent can do, approval flows that keep a human in the loop for dangerous operations, and checkpoint/rollback systems that let you undo mistakes. We also explore sandboxing techniques that contain blast radius when things go wrong.

The chapter culminates with audit trails, rate limiting, and comprehensive testing strategies for safety systems. You will learn not just how to build these systems, but how to verify they actually work under adversarial conditions. Every safety mechanism we design follows a defense-in-depth philosophy: no single layer is trusted to catch every problem.

## Learning Objectives
- Identify and categorize threats specific to LLM-powered coding agents
- Design multi-layered permission architectures with allowlists, denylists, and capability scoping
- Implement human-in-the-loop approval flows for high-risk operations
- Build checkpoint and rollback systems that enable safe recovery from agent errors
- Apply sandboxing techniques to limit the blast radius of agent actions
- Construct audit trails and rate limiting systems for operational visibility

## Subchapters
1. [Threat Modeling](/linear/13-safety-and-permissions/01-threat-modeling)
2. [Permission Architectures](/linear/13-safety-and-permissions/02-permission-architectures)
3. [Approval Flows](/linear/13-safety-and-permissions/03-approval-flows)
4. [Checkpoint Systems](/linear/13-safety-and-permissions/04-checkpoint-systems)
5. [Rollback Mechanisms](/linear/13-safety-and-permissions/05-rollback-mechanisms)
6. [Allowlist Denylist Design](/linear/13-safety-and-permissions/06-allowlist-denylist-design)
7. [Sandboxing Deep Dive](/linear/13-safety-and-permissions/07-sandboxing-deep-dive)
8. [Plan Mode Design](/linear/13-safety-and-permissions/08-plan-mode-design)
9. [Audit Trails](/linear/13-safety-and-permissions/09-audit-trails)
10. [Rate Limiting Agents](/linear/13-safety-and-permissions/10-rate-limiting-agents)
11. [Testing Safety Systems](/linear/13-safety-and-permissions/11-testing-safety-systems)
12. [Summary](/linear/13-safety-and-permissions/12-summary)

## Prerequisites
- Chapter 7 (process management and sandboxing fundamentals)
- Chapter 12 (git operations for checkpointing and rollback)
