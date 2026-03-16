---
title: "Chapter 11: Git Integration"
description: Version control as a safety net for coding agents, covering git operations, worktree isolation, and repository analysis.
---

# Git Integration

Version control is the coding agent's most powerful safety net. When an agent modifies files across a codebase, the ability to track changes, create atomic commits, and roll back mistakes transforms risky operations into reversible experiments. This chapter builds a comprehensive git integration layer that gives your agent the same version control superpowers that experienced developers rely on daily.

You will implement tools that let the agent inspect repository state through status and diff commands, manage branches for isolated work, and create well-structured commits with meaningful messages. Beyond basic operations, you will build worktree isolation so the agent can work on multiple tasks simultaneously without interference, and repository analysis tools that help the agent understand project history and code ownership.

By the end of this chapter, your agent will treat git not as an afterthought but as a core part of its workflow -- checking status before making changes, creating checkpoints during complex refactors, and using blame and log data to make better decisions about how to modify code.

## Learning Objectives
- Implement git status and diff tools that give the agent awareness of repository state
- Build branch management capabilities for isolated feature work
- Create a commit tool that produces clean, well-messaged commits
- Use worktree isolation to enable parallel agent tasks without conflicts
- Develop repository analysis tools using git log and blame
- Ensure all git operations follow safe, non-destructive patterns

## Subchapters
1. [Git Basics for Agents](/project/11-git-integration/01-git-basics-for-agents)
2. [Git Status and Diff](/project/11-git-integration/02-git-status-diff)
3. [Branch Management](/project/11-git-integration/03-branch-management)
4. [Creating Commits](/project/11-git-integration/04-creating-commits)
5. [Worktree Isolation](/project/11-git-integration/05-worktree-isolation)
6. [Diff Generation](/project/11-git-integration/06-diff-generation)
7. [Safe Operations](/project/11-git-integration/07-safe-operations)
8. [Merge Conflict Detection](/project/11-git-integration/08-merge-conflict-detection)
9. [Repo Analysis](/project/11-git-integration/09-repo-analysis)
10. [Log and Blame](/project/11-git-integration/10-log-and-blame)
11. [Git Tool Implementation](/project/11-git-integration/11-git-tool-implementation)
12. [Summary](/project/11-git-integration/12-summary)

## Prerequisites
- Chapter 4: Tool system fundamentals and the tool trait pattern
- Chapter 5: File operation tools and path handling
- Chapter 6: Shell execution with safety constraints
