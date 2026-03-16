---
title: "Chapter 15: Extensibility Patterns"
description: Design plugin architectures, event systems, and extension points that let users and third parties extend your coding agent's capabilities.
---

# Extensibility Patterns

A coding agent that can only do what its original authors built is fundamentally limited. The most successful developer tools -- VS Code, Neovim, Emacs -- thrive because they are extensible platforms, not just fixed applications. This chapter teaches you how to design your coding agent as an extensible platform where new tools, providers, workflows, and integrations can be added without modifying the core codebase.

We begin with plugin architecture patterns and loading strategies, then build the foundational infrastructure: an event bus for decoupled communication and a hook system for intercepting and modifying agent behavior. A major focus is the Model Context Protocol (MCP), the emerging standard for connecting agents to external tool and resource servers. You will implement MCP client support for both tool servers and resource servers, giving your agent access to a growing ecosystem of capabilities.

The chapter also covers skill systems (higher-level abstractions over tools), configuration-driven extensions, hot reload for development workflows, and the critical security and versioning concerns that arise when running third-party code. By the end, you will understand how to build an agent that grows more capable over time through community contributions rather than just core development.

## Learning Objectives
- Design plugin architectures that support dynamic loading, lifecycle management, and clean isolation
- Implement an event bus and hook system that lets extensions observe and modify agent behavior at key points
- Build MCP client support for connecting to external tool servers and resource servers
- Design a skill system that packages tools, prompts, and workflows into reusable higher-level capabilities
- Implement hot reload for rapid plugin development without restarting the agent
- Address security, isolation, and versioning challenges that arise with third-party extensions

## Subchapters
1. [Plugin Architecture Patterns](/linear/15-extensibility-patterns/01-plugin-architecture-patterns)
2. [Loading Strategies](/linear/15-extensibility-patterns/02-loading-strategies)
3. [Event Bus Design](/linear/15-extensibility-patterns/03-event-bus-design)
4. [Hook Systems](/linear/15-extensibility-patterns/04-hook-systems)
5. [Model Context Protocol](/linear/15-extensibility-patterns/05-model-context-protocol)
6. [MCP Tool Servers](/linear/15-extensibility-patterns/06-mcp-tool-servers)
7. [MCP Resource Servers](/linear/15-extensibility-patterns/07-mcp-resource-servers)
8. [Skill System Design](/linear/15-extensibility-patterns/08-skill-system-design)
9. [Configuration Driven Extensions](/linear/15-extensibility-patterns/09-configuration-driven-extensions)
10. [Hot Reload Patterns](/linear/15-extensibility-patterns/10-hot-reload-patterns)
11. [Isolation and Security](/linear/15-extensibility-patterns/11-isolation-and-security)
12. [Versioning Plugins](/linear/15-extensibility-patterns/12-versioning-plugins)
13. [Extension Testing](/linear/15-extensibility-patterns/13-extension-testing)
14. [Summary](/linear/15-extensibility-patterns/14-summary)

## Prerequisites
- Chapter 5 (tool system architecture and registration patterns)
- Chapter 14 (provider abstraction and trait-based design)
