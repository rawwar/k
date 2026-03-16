---
title: "Chapter 16: Testing Coding Agents"
description: Master the unique testing challenges of non-deterministic LLM-powered systems, from mocking model responses to benchmark-driven evaluation.
---

# Testing Coding Agents

Testing a coding agent is fundamentally harder than testing a traditional application. The core of your system — the LLM — is non-deterministic, expensive to call, and changes behavior when the provider updates their model. A test that passes today may fail tomorrow, not because your code changed, but because the model responded slightly differently. This chapter equips you with the techniques and mindset needed to build a robust test suite despite this uncertainty.

We start with the testing philosophy specific to agent systems: what to test deterministically, what to test probabilistically, and what to test by recording real interactions. You will learn how to unit test tools in isolation, mock LLM responses for fast iteration, and integration test the full agentic loop. We cover snapshot testing for catching regressions in tool output, property-based testing for validating invariants across random inputs, and recording/replay for capturing real model interactions as reusable test fixtures.

The chapter also addresses benchmark testing (evaluating agent performance on standardized tasks), testing your safety systems under adversarial conditions, and setting up CI/CD pipelines that balance test coverage with API cost. By the end, you will have a comprehensive testing strategy that gives you confidence in your agent's correctness without breaking the bank on API calls.

## Learning Objectives
- Develop a testing philosophy that accounts for the non-deterministic nature of LLM-powered systems
- Write effective unit tests for tools, parsers, and other deterministic agent components
- Mock LLM responses to enable fast, repeatable integration tests of the agentic loop
- Implement recording and replay infrastructure for capturing real model interactions as test fixtures
- Apply property-based testing to validate agent invariants across diverse inputs
- Set up CI/CD pipelines that run the right tests at the right time to balance coverage and cost

## Subchapters
1. [Testing Philosophy](/linear/16-testing-coding-agents/01-testing-philosophy)
2. [Unit Testing Tools](/linear/16-testing-coding-agents/02-unit-testing-tools)
3. [Mocking LLM Responses](/linear/16-testing-coding-agents/03-mocking-llm-responses)
4. [Integration Testing the Loop](/linear/16-testing-coding-agents/04-integration-testing-the-loop)
5. [Snapshot Testing](/linear/16-testing-coding-agents/05-snapshot-testing)
6. [Property Based Testing](/linear/16-testing-coding-agents/06-property-based-testing)
7. [Recording and Replay](/linear/16-testing-coding-agents/07-recording-and-replay)
8. [Benchmark Testing](/linear/16-testing-coding-agents/08-benchmark-testing)
9. [Testing Safety Systems](/linear/16-testing-coding-agents/09-testing-safety-systems)
10. [CI CD for Agents](/linear/16-testing-coding-agents/10-ci-cd-for-agents)
11. [Test Fixtures](/linear/16-testing-coding-agents/11-test-fixtures)
12. [Summary](/linear/16-testing-coding-agents/12-summary)

## Prerequisites
- Chapter 4 (the agentic loop that you will be testing)
- Chapter 5 (the tool system that you will be testing)
