---
title: Lessons from Pi
description: Analyze Pi's architecture and planning-first approach, extracting lessons about structured workflows and user-guided agent behavior.
---

# Lessons from Pi

> **What you'll learn:**
> - How Pi's plan-then-execute architecture differs from free-form agentic loops and the tradeoffs this creates for user control vs agent autonomy
> - What Pi's approach to change tracking, diff review, and user approval reveals about designing human-in-the-loop coding workflows
> - The lessons from Pi's versioned plan system about managing complex multi-step coding tasks with checkpoints and branching

Pi (originally Plandex) takes a fundamentally different approach to agent-assisted coding. Where Claude Code and OpenCode let the model freely choose its next action in an open-ended loop, Pi introduces an explicit planning phase. The agent first creates a structured plan, the user reviews and approves it, and then the agent executes the plan step by step. This planning-first architecture teaches valuable lessons about user control, predictability, and managing complex multi-file changes.

## Lesson 1: Plan-Then-Execute vs. Free-Form Loops

The core architectural difference between Pi and other coding agents is the separation of planning from execution. In Claude Code's model, the agent interleaves thinking and acting — it might read a file, think about what to change, write the change, run tests, and then read another file, all in a single continuous loop. In Pi's model, these phases are distinct:

**Planning Phase:**
1. The user describes the task
2. The agent analyzes the codebase and creates a plan
3. The plan lists specific files to modify, create, or delete
4. The user reviews the plan and can modify or approve it

**Execution Phase:**
1. The agent executes the approved plan step by step
2. Changes are tracked in a sandbox (not applied directly to files)
3. The user reviews the accumulated changes as diffs
4. The user applies the changes to the actual filesystem

This separation has profound implications for user experience. With a free-form loop, the agent acts on your codebase in real time — if it makes a mistake, you need to undo it. With Pi's approach, changes are staged in a sandbox until you explicitly apply them.

```rust
// Pi-inspired plan structure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Plan {
    pub id: String,
    pub description: String,
    pub steps: Vec<PlanStep>,
    pub status: PlanStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanStep {
    pub id: String,
    pub description: String,
    pub operation: PlannedOperation,
    pub status: StepStatus,
    pub dependencies: Vec<String>, // IDs of steps that must complete first
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PlannedOperation {
    ReadFile { path: PathBuf },
    ModifyFile { path: PathBuf, description: String },
    CreateFile { path: PathBuf, description: String },
    DeleteFile { path: PathBuf },
    RunCommand { command: String },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PlanStatus {
    Draft,
    Approved,
    InProgress,
    Completed,
    Failed { step_id: String, error: String },
}
```

The lesson is that planning adds predictability at the cost of spontaneity. A free-form agent can discover and fix unexpected issues mid-task. A plan-based agent only addresses what the plan covers. The right choice depends on the use case — routine refactoring benefits from planning's predictability, while exploratory debugging benefits from free-form flexibility.

::: python Coming from Python
If you've used Python task runners like `invoke` or `fabric`, Pi's plan model will feel familiar. The plan is like a makefile — a declared sequence of operations that you inspect before running. The free-form agentic loop is more like dropping into a Python REPL and figuring things out as you go. Both are valid approaches for different situations.
:::

## Lesson 2: Sandboxed Changes and Diff Review

Pi does not modify your files directly during plan execution. Instead, it accumulates changes in a sandbox — an in-memory representation of the modified filesystem. When execution completes, you review the changes as unified diffs before applying them.

This is architecturally significant because it separates *intent* (what the agent wants to change) from *effect* (what actually changes on disk). The user gets a complete picture of all modifications before any file is touched.

```rust
pub struct Sandbox {
    /// Original file contents (read from filesystem)
    originals: HashMap<PathBuf, String>,
    /// Modified file contents (produced by the agent)
    modifications: HashMap<PathBuf, String>,
    /// Files to be created (did not exist before)
    new_files: HashMap<PathBuf, String>,
    /// Files to be deleted
    deletions: HashSet<PathBuf>,
}

impl Sandbox {
    pub fn new() -> Self {
        Self {
            originals: HashMap::new(),
            modifications: HashMap::new(),
            new_files: HashMap::new(),
            deletions: HashSet::new(),
        }
    }

    /// Read a file — returns the sandboxed version if modified, otherwise reads from disk
    pub fn read_file(&mut self, path: &Path) -> anyhow::Result<String> {
        if let Some(content) = self.modifications.get(path) {
            return Ok(content.clone());
        }
        if let Some(content) = self.new_files.get(path) {
            return Ok(content.clone());
        }
        let content = std::fs::read_to_string(path)?;
        self.originals.insert(path.to_owned(), content.clone());
        Ok(content)
    }

    /// Write a file — stores in sandbox, does not touch filesystem
    pub fn write_file(&mut self, path: &Path, content: String) {
        if self.originals.contains_key(path) {
            self.modifications.insert(path.to_owned(), content);
        } else {
            self.new_files.insert(path.to_owned(), content);
        }
    }

    /// Generate diffs for all changes
    pub fn generate_diffs(&self) -> Vec<FileDiff> {
        let mut diffs = Vec::new();

        for (path, new_content) in &self.modifications {
            if let Some(original) = self.originals.get(path) {
                diffs.push(FileDiff {
                    path: path.clone(),
                    kind: DiffKind::Modified,
                    diff: create_unified_diff(original, new_content),
                });
            }
        }

        for (path, content) in &self.new_files {
            diffs.push(FileDiff {
                path: path.clone(),
                kind: DiffKind::Created,
                diff: create_unified_diff("", content),
            });
        }

        for path in &self.deletions {
            if let Some(original) = self.originals.get(path) {
                diffs.push(FileDiff {
                    path: path.clone(),
                    kind: DiffKind::Deleted,
                    diff: create_unified_diff(original, ""),
                });
            }
        }

        diffs
    }

    /// Apply all sandboxed changes to the actual filesystem
    pub fn apply(&self) -> anyhow::Result<ApplyResult> {
        let mut applied = 0;

        for (path, content) in &self.modifications {
            std::fs::write(path, content)?;
            applied += 1;
        }
        for (path, content) in &self.new_files {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, content)?;
            applied += 1;
        }
        for path in &self.deletions {
            std::fs::remove_file(path)?;
            applied += 1;
        }

        Ok(ApplyResult { files_changed: applied })
    }
}
```

The lesson is that sandboxing gives users confidence. When the agent proposes changes to fifteen files across your project, you want to see every modification before it happens. You want to spot the one file where the agent made an incorrect assumption. The sandbox pattern makes this review workflow natural.

::: tip In the Wild
Pi presents changes as color-coded diffs in the terminal, with additions in green and deletions in red — the same format developers see in `git diff`. Users can apply all changes at once, apply them selectively (file by file), or reject the entire batch and ask the agent to try again. This workflow mirrors the code review process that developers already know, making it intuitive even for first-time users.
:::

## Lesson 3: Version Control for Agent Actions

Pi versions its plans. When you modify a plan (adding steps, reordering operations, changing the approach), the previous version is preserved. You can compare versions, revert to an earlier plan, or branch from a checkpoint.

This is an underappreciated architectural pattern. Agent-assisted coding is inherently iterative — the first attempt often is not right. Without versioning, each retry starts from scratch. With versioning, you build on previous attempts.

```rust
pub struct PlanHistory {
    versions: Vec<PlanVersion>,
}

pub struct PlanVersion {
    pub version: usize,
    pub plan: Plan,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub change_description: String,
}

impl PlanHistory {
    pub fn current(&self) -> &Plan {
        &self.versions.last().expect("History is never empty").plan
    }

    pub fn add_version(&mut self, plan: Plan, description: String) {
        let version = self.versions.len() + 1;
        self.versions.push(PlanVersion {
            version,
            plan,
            timestamp: chrono::Utc::now(),
            change_description: description,
        });
    }

    pub fn revert_to(&mut self, version: usize) -> anyhow::Result<()> {
        let target = self.versions.iter()
            .find(|v| v.version == version)
            .ok_or_else(|| anyhow::anyhow!("Version {} not found", version))?;

        let reverted = target.plan.clone();
        self.add_version(
            reverted,
            format!("Reverted to version {}", version),
        );
        Ok(())
    }
}
```

The lesson is that agent actions benefit from the same version control patterns we use for code. If you can undo, branch, and compare, you can iterate more freely because mistakes are recoverable.

## Lesson 4: Multi-File Coherence

One of the hardest problems in coding agents is maintaining coherence across multiple files. When the agent modifies a function signature in one file, it must update all call sites in other files. Pi's planning phase helps with this because the plan is holistic — it lists all files that need to change and describes the relationship between changes.

This is harder in a free-form loop where the agent modifies files one at a time and might forget to update a related file. Pi's plan makes the scope of changes explicit and reviewable before execution begins.

## Lesson 5: When Planning Fails

The plan-based approach has a failure mode that free-form loops do not: the plan itself can be wrong. The agent might plan to modify a file that does not exist, or plan changes based on an incorrect understanding of the codebase. When execution starts and a step fails, the plan needs to be revised.

Pi handles this by allowing mid-execution replanning. If step 3 of 10 fails, the agent can revise the remaining steps based on what it learned from the failure. This is a compromise between full planning rigidity and full free-form flexibility.

The lesson is that no planning system survives contact with reality unchanged. Build in replanning capabilities from the start. Let the agent adapt when its assumptions prove wrong.

```rust
pub enum ExecutionResult {
    /// Step completed successfully
    Success(StepOutput),
    /// Step failed — plan needs revision from this point
    NeedsReplan {
        failed_step: String,
        error: String,
        remaining_steps: Vec<String>,
    },
    /// Step failed fatally — cannot continue
    Fatal(String),
}
```

## Lesson 6: The Spectrum of Agent Autonomy

The deepest lesson from comparing Pi with Claude Code and OpenCode is that coding agents exist on a spectrum of autonomy:

- **High autonomy (Claude Code)**: The agent acts freely, making and executing decisions in real time. Maximum speed, minimum user control during execution.
- **Medium autonomy (OpenCode)**: The agent acts freely but with a robust approval system and session controls. Balanced speed and control.
- **Low autonomy (Pi)**: The agent plans, the user approves, then execution proceeds. Maximum predictability, slower interaction cycle.

None of these is universally "best." The right choice depends on the task, the user's risk tolerance, and how well they trust the model. An ideal agent might offer all three modes, letting users choose their comfort level.

## Key Takeaways

- The plan-then-execute architecture separates intent from action, giving users a complete picture of proposed changes before any files are modified — this adds predictability at the cost of the spontaneous problem-solving that free-form loops enable.
- Sandboxed changes with diff review mirror the code review workflow developers already know, building confidence in the agent's modifications and preventing unreviewed changes from reaching the filesystem.
- Versioned plans enable iterative refinement — users can compare, revert, and branch plans just as they do with code, making the agent's decision-making process transparent and recoverable.
- Build replanning capabilities into plan-based architectures because no plan survives contact with reality unchanged — when a step fails, the agent should be able to revise the remaining steps.
- Coding agents exist on a spectrum from high autonomy (free-form loops) to low autonomy (plan-then-execute), and the ideal agent may offer multiple modes to match different tasks and user preferences.
