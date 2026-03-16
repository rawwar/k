---
title: Summary
description: Consolidate the testing strategies into a comprehensive test plan that gives confidence in agent correctness without excessive API costs.
---

# Summary

> **What you'll learn:**
> - How unit tests, mocks, snapshots, property tests, recording/replay, and benchmarks compose into a complete agent testing strategy
> - A decision framework for choosing the right testing technique for each type of agent behavior you need to verify
> - How to maintain and evolve your test suite as the agent grows, models change, and new features are added

You have now covered the full spectrum of testing techniques for a coding agent. Each technique addresses a specific challenge, and together they form a comprehensive strategy that gives you confidence in your agent's correctness without draining your API budget. Let's bring it all together.

## The Complete Testing Strategy

Here is every technique from this chapter and where it fits in your agent's test suite:

**Unit tests** cover your deterministic code: tool implementations, parsers, validators, formatters, token counters, and configuration loaders. These are your foundation — they run in milliseconds, catch the most bugs, and cost nothing. You should have hundreds of them.

**Mock providers** let you test the agentic loop without calling real APIs. You script LLM responses and verify that your loop dispatches tools correctly, handles errors, manages turn limits, and constructs proper conversation histories. These tests run in seconds and cover your critical code paths.

**Snapshot tests** catch unintended changes in outputs that matter: tool result formatting, API request bodies, system prompts, and error messages. They are not for testing correctness — they are for detecting change. Use them for outputs consumed by external systems (the LLM, the API, the user).

**Property-based tests** explore the space of inputs your code might encounter. They verify invariants like "parsing never panics," "path validation always rejects traversal," and "token counting never decreases when messages are added." They catch the edge cases you would never think to write by hand.

**Recording and replay** bridges the gap between mocked tests and reality. You record a real API interaction once, sanitize it, and replay it in your test suite forever. This gives you real-world fidelity at test-suite speed. Re-record when you change models, prompts, or tools.

**Benchmark evaluations** measure what actually matters: can the agent complete real coding tasks? They call the real API, so they are expensive and non-deterministic. Run them on a schedule or before releases, and track results over time to detect regressions.

**Safety tests** verify that your security boundaries hold under adversarial conditions. They test path traversal prevention, command injection blocking, permission enforcement, and prompt injection defense. They run on every commit and are never skipped.

**Test fixtures** provide the reusable building blocks that make all the above tests clean and maintainable. Workspace fixtures scaffold temp directories, conversation fixtures set up mock interactions, and data fixtures load recordings and expected outputs.

## Decision Framework: Which Technique for Which Behavior

When you need to test something new, use this decision framework:

| What you are testing | Technique | Why |
|---|---|---|
| A tool's core logic (read, write, shell) | Unit test with real temp directory | Deterministic, fast, tests the real thing |
| Input validation for tool arguments | Unit test + property test | Unit test for known cases, property test for edge cases |
| Agentic loop flow (tool dispatch, turn management) | Integration test with mock provider | Tests the loop without API costs |
| Error recovery (tool fails, model retries) | Integration test with scripted mock | Script the failure and recovery sequence |
| Output formatting (what the LLM sees) | Snapshot test | Detects any change in formatting |
| API request construction | Snapshot test with redactions | Catches field changes, ignores dynamic values |
| Security boundaries | Dedicated safety test suite | Must run on every commit, never skipped |
| Parser robustness | Property test | Explores random inputs, catches panics |
| Real-world agent quality | Benchmark evaluation | Expensive but necessary ground truth |
| Prompt changes | Replay test (before change) + benchmark (after) | Replay verifies no regression, benchmark measures improvement |

::: tip Coming from Python
If you are coming from pytest, here is how Rust's testing tools map to what you know:

| Python (pytest) | Rust equivalent |
|---|---|
| `def test_foo():` | `#[test] fn test_foo()` |
| `@pytest.fixture` | Helper function or builder struct |
| `@pytest.mark.parametrize` | `test-case` crate or loop in test |
| `unittest.mock.MagicMock` | Implement the trait yourself (`MockProvider`) |
| `pytest-snapshot` / `syrupy` | `insta` crate |
| `hypothesis` | `proptest` crate |
| `vcrpy` (HTTP recording) | Custom `RecordingProvider` / `ReplayProvider` |
| `pytest.mark.slow` | `#[ignore]` attribute |
| `conftest.py` | `tests/common/mod.rs` or test helper module |
| `pytest -m "not slow"` | `cargo test` (ignores `#[ignore]` by default) |
| `pytest -m slow` | `cargo test -- --ignored` |
| `tox` / `nox` | GitHub Actions workflow tiers |

The biggest conceptual difference: Python fixtures are injected by name (the framework resolves them), while Rust fixtures are called explicitly (you construct them yourself). Rust's approach is more verbose but more transparent — there is no hidden resolution order or scope confusion.
:::

## The CI Pipeline Recap

Your CI pipeline runs these tests in three tiers:

**Every push** (seconds):
- All unit tests (`cargo test --lib`)
- All safety tests
- Property tests
- Clippy and formatting checks

**Every pull request** (minutes):
- Everything from Tier 1
- Integration tests with mock providers (`cargo test --test '*'`)
- Replay tests using recorded fixtures
- Snapshot verification

**Scheduled or release** (minutes, costs money):
- Everything from Tiers 1 and 2
- Benchmark evaluations against real API (`cargo test -- --ignored`)
- Results saved as artifacts for trend tracking

## Evolving Your Test Suite

Your test suite is not static. It grows and changes with your agent. Here is how to keep it healthy:

**When you add a new tool:** Write unit tests for input validation, execution, error handling, and output formatting. Add the tool to existing integration test scenarios where appropriate. Add safety tests if the tool interacts with the filesystem or shell.

**When you change the system prompt:** Run existing replay tests to see if the change breaks any recorded interactions. If it does, evaluate whether the break is acceptable. Run benchmarks to measure whether the change improves task completion. Re-record affected fixtures.

**When you switch models:** Re-record all replay fixtures with the new model. Run the full benchmark suite and compare results to the previous model. Watch for changes in tool calling patterns — a new model might use tools differently.

**When you find a bug in production:** Write a unit or integration test that reproduces the bug before fixing it. This becomes a regression test that prevents the bug from returning. If the bug was a security issue, add it to the safety test suite.

**When tests become slow:** Profile which tests take the longest. Move slow-but-important tests behind `#[ignore]`. Look for tests that create unnecessary temp directories or run redundant setup. Consider sharing workspace fixtures across related tests using `lazy_static` or `once_cell`.

## What You Built in This Chapter

Over these twelve subchapters, you built a complete testing infrastructure:

1. A **testing philosophy** that separates deterministic from non-deterministic code and tests each appropriately
2. **Unit tests** for every tool covering validation, execution, errors, and formatting
3. A **mock provider** with response builders for fast, deterministic loop testing
4. **Integration tests** that verify the full agentic loop including tool dispatch and error recovery
5. **Snapshot tests** that catch unintended changes in tool output formatting and API requests
6. **Property-based tests** that explore edge cases in parsers, validators, and data transformations
7. A **recording and replay** system that captures real API interactions for test fixtures
8. **Benchmark tasks** with success criteria and scoring for measuring real-world agent quality
9. A **safety test suite** that verifies security boundaries under adversarial conditions
10. A **CI/CD pipeline** with three tiers that balances coverage, speed, and cost
11. **Reusable test fixtures** for workspaces, conversations, and fixture data

This testing infrastructure lets you iterate on your agent with confidence. You can change prompts, swap models, add tools, and refactor code knowing that your test suite will catch regressions in correctness, security, and quality.

::: info In the Wild
The most successful production coding agents — Claude Code, Cursor, and similar tools — all invest heavily in testing infrastructure. They maintain thousands of deterministic tests for their tool implementations and core logic, hundreds of mock-based integration tests for conversation flows, and curated benchmark suites that measure real-world task completion. The common lesson: the testing infrastructure is as important as the agent code itself. Spending a week building good test fixtures, mock providers, and CI pipelines pays for itself many times over in bugs caught before they reach users.
:::

## Key Takeaways

- A complete agent testing strategy uses all eight techniques: unit tests, mock providers, snapshot tests, property tests, recording/replay, benchmarks, safety tests, and fixtures — each addressing a specific testing challenge
- Use the decision framework to choose the right technique: unit tests for deterministic logic, mocks for loop testing, snapshots for output stability, property tests for edge cases, benchmarks for real-world quality, and safety tests for security boundaries
- The CI pipeline runs in three tiers — fast deterministic tests on every push, integration tests on PRs, and benchmark evaluations on schedule — balancing coverage with speed and cost
- Evolve your test suite alongside your agent: add regression tests for production bugs, re-record fixtures when changing models, and run benchmarks when changing prompts
- The testing infrastructure is as valuable as the agent code itself — invest in good fixtures, mock providers, and CI pipelines from the start
