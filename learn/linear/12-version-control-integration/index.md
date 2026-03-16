---
title: "Chapter 12: Version Control Integration"
description: Integrating with Git for status, diff, commit, worktree, and automation workflows that make the agent a safe and effective collaborator on version-controlled code.
---

# Version Control Integration

This chapter covers the essential integration between a coding agent and Git, the version control system that underpins virtually all modern software development. An agent that modifies code must understand the state of the repository — which files are modified, what changes are staged, whether there are conflicts — and must be able to create commits, manage branches, and provide safety nets that let users confidently undo agent-generated changes.

We start with the Git object model: blobs, trees, commits, and refs. Understanding these fundamentals helps you reason about what Git operations actually do at the data level, which is essential for implementing features like intelligent diff display, conflict resolution, and repository analysis. You will then learn how to drive Git from Rust, both by spawning git commands (building on Chapter 7's process management) and by using the git2 library for direct access to the Git object database.

The chapter covers the full range of version control operations an agent needs: status checking, diff generation, commit creation, branch management, worktree mechanics for parallel workstreams, and merge conflict handling. We close with automation patterns — how to use Git as a safety net that makes agent modifications reversible, and how to build undo strategies that restore the repository to known-good states.

## Learning Objectives
- Understand the Git object model (blobs, trees, commits, refs) and how it enables version control operations
- Drive Git operations from Rust using both process spawning and the git2 (libgit2) library
- Implement status, diff, and commit operations that integrate smoothly into the agent workflow
- Work with Git worktrees to enable parallel agent workstreams on the same repository
- Handle merge conflicts programmatically and present resolution options to the user
- Build Git-based safety nets and undo strategies for reversible agent modifications

## Subchapters
1. [Git Object Model](/linear/12-version-control-integration/01-git-object-model)
2. [Working with Git in Rust](/linear/12-version-control-integration/02-working-with-git-in-rust)
3. [Status and Diff](/linear/12-version-control-integration/03-status-and-diff)
4. [Commit Operations](/linear/12-version-control-integration/04-commit-operations)
5. [Branch Strategies](/linear/12-version-control-integration/05-branch-strategies)
6. [Worktree Mechanics](/linear/12-version-control-integration/06-worktree-mechanics)
7. [Merge and Conflict](/linear/12-version-control-integration/07-merge-and-conflict)
8. [Repo Analysis Techniques](/linear/12-version-control-integration/08-repo-analysis-techniques)
9. [Git as Safety Net](/linear/12-version-control-integration/09-git-as-safety-net)
10. [Undo Strategies](/linear/12-version-control-integration/10-undo-strategies)
11. [Automation Patterns](/linear/12-version-control-integration/11-automation-patterns)
12. [Summary](/linear/12-version-control-integration/12-summary)

## Prerequisites
- Chapter 7 (process management for running git commands as child processes)
