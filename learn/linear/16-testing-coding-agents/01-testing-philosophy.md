---
title: Testing Philosophy
description: Develop a testing mindset for LLM-powered systems that distinguishes between deterministic, probabilistic, and behavioral testing strategies.
---

# Testing Philosophy

> **What you'll learn:**
> - How to categorize agent components into deterministic (tools, parsers) and non-deterministic (LLM interactions) tiers with different testing strategies for each
> - The testing pyramid for coding agents — what belongs in unit tests, integration tests, and end-to-end evaluation benchmarks
> - Why traditional code coverage metrics are insufficient for agent testing and what alternative quality metrics to track instead

If you have ever written a test for a web server, you know the drill: set up the request, call the handler, assert on the response. The same input always produces the same output. Coding agents break this contract. The LLM at the center of your system is a black box that gives slightly different answers every time you ask. Temperature settings, model updates, and even prompt phrasing changes shift the output. So how do you write tests you can trust?

The answer is to stop thinking about your agent as a single thing to test and start thinking of it as layers. Some layers are completely deterministic and testable with traditional techniques. Other layers are inherently stochastic and require different strategies. The art of testing coding agents is knowing which strategy to apply where.

## The Deterministic and Non-Deterministic Split

Every coding agent has two broad categories of code:

**Deterministic code** — given the same input, it always produces the same output. This includes your tool implementations (file read, file write, shell execution), your message parser, your token counter, your permission checker, your prompt builder, and your configuration loader. This code makes up the majority of your codebase, and you test it exactly like you would test any other Rust program.

**Non-deterministic code** — anything that depends on the LLM's response. This includes the agentic loop's decision-making (which tool does the model pick?), the quality of generated code, the phrasing of explanations, and the number of turns needed to solve a problem. You cannot write an `assert_eq!` for these because the answer changes on every run.

The crucial insight: you do not need to test that the LLM gives the right answer. You need to test that your code handles whatever answer the LLM gives correctly. This shifts the focus from "does the model respond well?" to "does my code parse, route, execute, and present model responses correctly?"

```rust
// This is deterministic and fully testable:
fn parse_tool_call(raw: &str) -> Result<ToolCall, ParseError> {
    let value: serde_json::Value = serde_json::from_str(raw)?;
    let name = value["name"]
        .as_str()
        .ok_or(ParseError::MissingField("name"))?;
    let arguments = value["arguments"].clone();
    Ok(ToolCall {
        name: name.to_string(),
        arguments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_tool_call() {
        let raw = r#"{"name": "read_file", "arguments": {"path": "src/main.rs"}}"#;
        let call = parse_tool_call(raw).unwrap();
        assert_eq!(call.name, "read_file");
    }

    #[test]
    fn parse_missing_name_returns_error() {
        let raw = r#"{"arguments": {"path": "src/main.rs"}}"#;
        assert!(parse_tool_call(raw).is_err());
    }
}
```

This test will pass today, tomorrow, and a year from now. It does not call any LLM. It verifies that your parsing logic handles well-formed and malformed input correctly. This is where the bulk of your testing effort should go.

## The Agent Testing Pyramid

Traditional testing uses a pyramid: many unit tests at the base, fewer integration tests in the middle, and a handful of end-to-end tests at the top. Agent testing uses a similar pyramid, but the layers have different meanings:

**Base: Unit tests (deterministic).** Test every tool, every parser, every utility function. These run in milliseconds, require no network, and catch the vast majority of bugs. Target hundreds of these.

**Middle: Integration tests with mocked LLM.** Wire up the full agentic loop but replace the real LLM provider with a mock that returns scripted responses. This lets you test the conversation flow, tool dispatch, error handling, and turn management without calling real APIs. Target dozens of these covering your critical paths.

**Upper middle: Replay tests.** Record real LLM interactions once, then replay them in tests. This gives you real-world fidelity without ongoing API costs. Re-record periodically when you change models or prompts. Target a handful for your most important workflows.

**Top: Live evaluation benchmarks.** Run the agent against a set of coding tasks using the real API and score the results. This is expensive and non-deterministic, so you run it rarely — on release candidates or weekly CI schedules. Target five to twenty benchmark tasks.

::: python Coming from Python
In Python's pytest ecosystem, you might organize these tiers with markers: `@pytest.mark.unit`, `@pytest.mark.integration`, `@pytest.mark.slow`. Rust uses a different mechanism. Unit tests live in `#[cfg(test)]` modules inside each source file. Integration tests live in the `tests/` directory at the crate root. You control which tests run using `cargo test`, `cargo test --test integration`, or by using the `#[ignore]` attribute for expensive tests that only run when explicitly requested with `cargo test -- --ignored`.
:::

## What Not to Mock

A common mistake is mocking too much. If you mock the filesystem when testing your file-read tool, you are testing your mock, not your tool. The tool's entire purpose is to interact with the filesystem.

Instead, use real but isolated environments:

- **Filesystem tools**: create a temporary directory with `tempfile::tempdir()`, populate it with known files, and run your tool against it.
- **Shell tools**: execute real commands in a sandboxed temp directory.
- **Git tools**: initialize a real git repository in a temp directory with known commits.

Reserve mocking for the expensive, non-deterministic boundary: the LLM provider. Everything else should use the real implementation in an isolated environment.

```rust
#[test]
fn read_file_tool_returns_contents() {
    // Use a real temp directory, not a mock filesystem
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("hello.txt");
    std::fs::write(&file_path, "Hello, world!").unwrap();

    let result = read_file_tool(file_path.to_str().unwrap()).unwrap();
    assert_eq!(result, "Hello, world!");
}
```

## Beyond Code Coverage

Traditional code coverage measures what percentage of lines, branches, or functions your tests execute. For a coding agent, 100% code coverage does not mean your agent works. You could cover every line and still have an agent that picks the wrong tool, generates broken code, or enters an infinite loop of retries.

Better metrics for agent test quality:

- **Tool coverage**: does every tool have tests for success, failure, invalid input, and edge cases?
- **Scenario coverage**: do your integration tests cover the critical user workflows — file editing, bug fixing, code generation, multi-step tasks?
- **Safety coverage**: do your safety tests cover every attack vector you have identified — prompt injection, path traversal, command injection?
- **Regression coverage**: when a bug is found in production, is there a test that reproduces it and prevents recurrence?

Track these as checklists, not percentages. A coding agent with 60% line coverage and complete scenario coverage is better tested than one with 95% line coverage that never tests a multi-turn conversation.

::: wild In the Wild
Claude Code maintains an extensive suite of deterministic tests for its tool implementations and parser logic, paired with a smaller set of evaluation benchmarks that run against the real API on a schedule. The evaluation suite covers specific coding tasks (fix a bug, add a feature, refactor a module) and scores the agent on whether the task was completed correctly. This two-tier approach — fast deterministic tests for correctness, slow evaluations for quality — is the industry standard for production coding agents.
:::

## The Testing Mindset for Agents

Here is the mental model that ties everything together:

1. **Test your code, not the model.** Your code is deterministic. The model is not. Focus your testing effort on the code you control.
2. **Isolate the boundary.** The LLM provider is the one non-deterministic dependency. Put it behind a trait so you can swap in mocks, replays, or the real thing depending on the test tier.
3. **Test behaviors, not outputs.** Instead of asserting that the agent produces exact text, assert that it calls the right tools in the right order, handles errors gracefully, and respects safety constraints.
4. **Budget for reality.** Some testing against the real API is necessary. Budget for it, schedule it, and track it as a first-class quality metric.

With this philosophy in hand, the rest of this chapter gives you the concrete techniques to implement each tier of the pyramid.

## Key Takeaways

- Split your agent into deterministic components (tools, parsers, permissions) and non-deterministic components (LLM interactions), and apply different testing strategies to each
- Follow the agent testing pyramid: many unit tests, several mock-based integration tests, a few replay tests, and occasional live benchmarks
- Mock only the LLM provider — use real but isolated environments (temp directories, real git repos) for everything else
- Measure test quality by scenario coverage, safety coverage, and regression coverage rather than relying solely on line coverage percentages
- Test your code's handling of LLM responses, not the quality of the LLM's responses themselves
