---
title: Benchmark Testing
description: Design evaluation benchmarks that measure your agent's ability to complete real coding tasks, providing quantitative quality metrics over time.
---

# Benchmark Testing

> **What you'll learn:**
> - How to design coding task benchmarks with clear success criteria, covering file editing, bug fixing, code generation, and multi-step workflows
> - Techniques for scoring agent performance beyond pass/fail, including token efficiency, tool call count, and time to completion
> - How to track benchmark results over time to detect regressions when updating models, prompts, or agent logic

Unit tests tell you if your code is correct. Integration tests tell you if the pieces work together. Benchmark tests tell you if your agent is any good. They measure the agent's ability to complete real coding tasks — the thing your users actually care about. A benchmark suite answers questions like: "Can the agent fix this bug?", "How many tokens does it use to create a new file?", and "Did our latest prompt change make things better or worse?"

Benchmarks are the only test tier that calls the real LLM API. They are expensive and non-deterministic, so you run them sparingly — on release candidates, weekly schedules, or when you make significant changes to the model, prompts, or agent logic.

## Designing a Benchmark Task

A benchmark task has four components: a setup (the initial state of the workspace), a prompt (what the user asks the agent to do), success criteria (how you determine if the task was completed), and scoring rubrics (how you quantify the quality of the completion).

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct BenchmarkTask {
    pub name: String,
    pub description: String,
    pub setup: WorkspaceSetup,
    pub prompt: String,
    pub success_criteria: Vec<Criterion>,
    pub max_turns: usize,
    pub timeout_secs: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WorkspaceSetup {
    pub files: Vec<FileSetup>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FileSetup {
    pub path: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum Criterion {
    FileContains { path: String, expected: String },
    FileExists { path: String },
    CommandSucceeds { command: String },
    FileDoesNotContain { path: String, forbidden: String },
}
```

Here is a concrete benchmark task that asks the agent to fix a bug:

```rust
fn fix_off_by_one_benchmark() -> BenchmarkTask {
    BenchmarkTask {
        name: "fix_off_by_one".to_string(),
        description: "Fix an off-by-one error in a loop".to_string(),
        setup: WorkspaceSetup {
            files: vec![
                FileSetup {
                    path: "src/lib.rs".to_string(),
                    content: r#"
pub fn sum_range(start: usize, end: usize) -> usize {
    let mut total = 0;
    // Bug: should be ..= for inclusive range
    for i in start..end {
        total += i;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sum_range() {
        assert_eq!(sum_range(1, 5), 15); // 1+2+3+4+5 = 15
    }
}
"#
                    .to_string(),
                },
            ],
        },
        prompt: "The test in src/lib.rs is failing. Fix the bug.".to_string(),
        success_criteria: vec![
            Criterion::CommandSucceeds {
                command: "cargo test".to_string(),
            },
            Criterion::FileContains {
                path: "src/lib.rs".to_string(),
                expected: "..=".to_string(),
            },
        ],
        max_turns: 10,
        timeout_secs: 120,
    }
}
```

## The Benchmark Runner

The benchmark runner sets up the workspace, runs the agent, and evaluates the results:

```rust
use std::time::Instant;

#[derive(Debug, Serialize)]
pub struct BenchmarkResult {
    pub task_name: String,
    pub passed: bool,
    pub criteria_results: Vec<CriterionResult>,
    pub metrics: BenchmarkMetrics,
}

#[derive(Debug, Serialize)]
pub struct CriterionResult {
    pub criterion: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct BenchmarkMetrics {
    pub total_turns: usize,
    pub tool_calls: usize,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub wall_time_secs: f64,
}

pub async fn run_benchmark(
    task: &BenchmarkTask,
    provider: &dyn LlmProvider,
) -> BenchmarkResult {
    // Set up the workspace
    let dir = tempfile::tempdir().unwrap();
    for file in &task.setup.files {
        let path = dir.path().join(&file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, &file.content).unwrap();
    }

    // Initialize cargo project if needed
    let cargo_toml = dir.path().join("Cargo.toml");
    if !cargo_toml.exists() {
        std::fs::write(
            &cargo_toml,
            "[package]\nname = \"benchmark\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
    }

    // Run the agent
    let start = Instant::now();
    let tools = create_tools(dir.path());
    let agent = AgentLoop::new(
        std::sync::Arc::new(provider),
        tools,
        task.max_turns,
    );
    let actions = agent.run(&task.prompt).await;
    let wall_time = start.elapsed().as_secs_f64();

    // Evaluate criteria
    let criteria_results: Vec<_> = task
        .success_criteria
        .iter()
        .map(|c| evaluate_criterion(c, dir.path()))
        .collect();

    let passed = criteria_results.iter().all(|r| r.passed);

    // Collect metrics
    let tool_calls = actions
        .iter()
        .filter(|a| matches!(&a.kind, ActionKind::ToolCall { .. }))
        .count();

    BenchmarkResult {
        task_name: task.name.clone(),
        passed,
        criteria_results,
        metrics: BenchmarkMetrics {
            total_turns: actions.iter().map(|a| a.turn).max().unwrap_or(0) + 1,
            tool_calls,
            input_tokens: 0,  // Collected from provider
            output_tokens: 0, // Collected from provider
            wall_time_secs: wall_time,
        },
    }
}

fn evaluate_criterion(criterion: &Criterion, workspace: &std::path::Path) -> CriterionResult {
    match criterion {
        Criterion::FileContains { path, expected } => {
            let full_path = workspace.join(path);
            match std::fs::read_to_string(&full_path) {
                Ok(content) => CriterionResult {
                    criterion: format!("{} contains '{}'", path, expected),
                    passed: content.contains(expected.as_str()),
                    detail: if content.contains(expected.as_str()) {
                        "Found".to_string()
                    } else {
                        format!("Not found in file content ({} bytes)", content.len())
                    },
                },
                Err(e) => CriterionResult {
                    criterion: format!("{} contains '{}'", path, expected),
                    passed: false,
                    detail: format!("Cannot read file: {}", e),
                },
            }
        }
        Criterion::FileExists { path } => {
            let exists = workspace.join(path).exists();
            CriterionResult {
                criterion: format!("{} exists", path),
                passed: exists,
                detail: if exists { "Exists" } else { "Not found" }.to_string(),
            }
        }
        Criterion::CommandSucceeds { command } => {
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .current_dir(workspace)
                .output();
            match output {
                Ok(o) => CriterionResult {
                    criterion: format!("'{}' succeeds", command),
                    passed: o.status.success(),
                    detail: if o.status.success() {
                        "Exit code 0".to_string()
                    } else {
                        String::from_utf8_lossy(&o.stderr).to_string()
                    },
                },
                Err(e) => CriterionResult {
                    criterion: format!("'{}' succeeds", command),
                    passed: false,
                    detail: format!("Failed to run: {}", e),
                },
            }
        }
        Criterion::FileDoesNotContain { path, forbidden } => {
            let full_path = workspace.join(path);
            match std::fs::read_to_string(&full_path) {
                Ok(content) => CriterionResult {
                    criterion: format!("{} does not contain '{}'", path, forbidden),
                    passed: !content.contains(forbidden.as_str()),
                    detail: if content.contains(forbidden.as_str()) {
                        "Forbidden content found".to_string()
                    } else {
                        "Clean".to_string()
                    },
                },
                Err(_) => CriterionResult {
                    criterion: format!("{} does not contain '{}'", path, forbidden),
                    passed: true,
                    detail: "File does not exist (vacuously true)".to_string(),
                },
            }
        }
    }
}
```

::: python Coming from Python
Python benchmark suites for LLM agents often use pytest with custom fixtures and markers:
```python
@pytest.mark.benchmark
@pytest.mark.timeout(120)
def test_fix_off_by_one(real_provider, tmp_workspace):
    setup_workspace(tmp_workspace, files={"src/lib.rs": BUGGY_CODE})
    agent = Agent(real_provider, workspace=tmp_workspace)
    agent.run("Fix the failing test")
    assert run_command("cargo test", cwd=tmp_workspace).returncode == 0
```
Rust's approach is structurally similar. The main difference is that Rust benchmarks are typically run with `#[ignore]` and activated explicitly (`cargo test -- --ignored`), while Python uses pytest markers (`pytest -m benchmark`). Both approaches achieve the same goal: keeping expensive tests out of the normal test loop.
:::

## Tracking Results Over Time

A single benchmark run is a snapshot. Tracking results over time reveals trends — is the agent getting better or worse?

```rust
#[derive(Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub timestamp: String,
    pub agent_version: String,
    pub model: String,
    pub results: Vec<BenchmarkResult>,
    pub summary: BenchmarkSummary,
}

#[derive(Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub total_tasks: usize,
    pub passed: usize,
    pub failed: usize,
    pub pass_rate: f64,
    pub avg_turns: f64,
    pub avg_tool_calls: f64,
    pub total_tokens: u64,
}

pub fn generate_report(results: Vec<BenchmarkResult>) -> BenchmarkReport {
    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let avg_turns = results.iter().map(|r| r.metrics.total_turns as f64).sum::<f64>() / total as f64;
    let avg_tools = results.iter().map(|r| r.metrics.tool_calls as f64).sum::<f64>() / total as f64;

    BenchmarkReport {
        timestamp: chrono::Utc::now().to_rfc3339(),
        agent_version: env!("CARGO_PKG_VERSION").to_string(),
        model: "claude-sonnet-4-20250514".to_string(),
        results,
        summary: BenchmarkSummary {
            total_tasks: total,
            passed,
            failed: total - passed,
            pass_rate: passed as f64 / total as f64,
            avg_turns,
            avg_tool_calls: avg_tools,
            total_tokens: 0,
        },
    }
}
```

Save reports as JSON files named by date and compare them across runs. When the pass rate drops or token usage spikes, investigate the change.

::: wild In the Wild
Claude Code runs a benchmark suite called an "eval suite" that covers dozens of real coding tasks across different languages, task types (bug fixes, new features, refactoring), and complexity levels. Each task has automated success criteria (tests pass, lint passes, file exists). The results are tracked over time so the team can see the impact of model upgrades, prompt changes, and agent logic improvements on real-world task completion rates.
:::

## A Minimal Benchmark Suite

Start with five to ten tasks covering your most important use cases:

1. **Read and explain**: read a file and answer a question about it
2. **Simple edit**: make a targeted change to one file
3. **Bug fix**: fix a failing test
4. **Create new file**: generate a complete source file from a description
5. **Multi-step**: read one file, edit another, run tests to verify

Each task should be completable in under two minutes and have unambiguous success criteria. Resist the urge to create complex, open-ended tasks — they are harder to score and produce noisier results.

## Key Takeaways

- Benchmark tests measure the agent's ability to complete real coding tasks using the real LLM API, providing the ground-truth quality signal that no other test tier can give
- Design tasks with clear setup, prompt, success criteria, and scoring rubrics so results are reproducible and comparable across runs
- Track metrics beyond pass/fail — token efficiency, tool call count, turn count, and wall time reveal whether the agent is improving or regressing
- Run benchmarks sparingly (release candidates, weekly CI) because they are expensive and non-deterministic, and track results over time to catch regressions
- Start with five to ten well-designed tasks covering your core use cases rather than trying to cover every possible scenario
