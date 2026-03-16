---
title: Tool Composition
description: Building higher-level operations by composing simple tools, including pipelines, conditional chains, and transactional tool groups.
---

# Tool Composition

> **What you'll learn:**
> - How to compose atomic tools into higher-level operations like "find and replace across files"
> - The difference between model-driven composition (the LLM chains tools) and system-driven composition (the agent orchestrates)
> - Patterns for transactional tool groups where a sequence of operations should succeed or fail atomically

Individual tools are useful, but real development tasks usually require multiple tools working together. When a developer says "find every file that imports `OldName` and rename it to `NewName`," they are describing a composition of search, read, and edit operations. How your agent handles this composition has a major impact on its reliability and efficiency.

## Two Approaches to Composition

There are two fundamentally different ways to compose tools, and understanding the distinction is critical.

### Model-Driven Composition

In model-driven composition, the language model chains tools itself. It decides the sequence, handles intermediate results, and adapts to errors at each step. This is the natural approach in an agentic loop.

The model might approach a rename task like this:

1. Call `search_files` with pattern `OldName` to find all usages
2. For each file in the results, call `read_file` to see the context
3. For each location that needs changing, call `edit_file` to make the replacement
4. Call `shell` to run the compiler and check for errors
5. If errors, call `read_file` to examine the error locations and call `edit_file` to fix them

The model is the orchestrator. It sees each result, reasons about it, and decides the next step.

**Advantages:**
- Maximum flexibility -- the model can adapt to unexpected situations
- No orchestration code to write -- the model handles the logic
- Handles edge cases naturally -- if a file needs a different kind of edit, the model adjusts

**Disadvantages:**
- Slow -- each step requires a round-trip to the LLM API
- Token-expensive -- the model consumes tokens reasoning about each intermediate step
- Inconsistent -- the model might handle similar files differently
- Can get lost -- on large refactors, the model might forget which files it has already processed

### System-Driven Composition

In system-driven composition, the agent provides a higher-level tool that handles the multi-step workflow internally. The model calls a single tool, and the agent orchestrates the steps.

```rust
use serde::Deserialize;
use schemars::JsonSchema;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindAndReplaceInput {
    /// Regex pattern to find files containing the target text.
    pub search_pattern: String,

    /// The exact text to replace in each matching file.
    pub old_text: String,

    /// The replacement text.
    pub new_text: String,

    /// Glob pattern to filter files. Defaults to all files.
    pub file_glob: Option<String>,

    /// If true, show what would change without making changes.
    pub dry_run: Option<bool>,
}

pub fn execute_find_and_replace(input: FindAndReplaceInput) -> Result<String, String> {
    let dry_run = input.dry_run.unwrap_or(false);

    // Step 1: Search for files containing the pattern
    let matching_files = search_for_files(&input.search_pattern, &input.file_glob)?;

    if matching_files.is_empty() {
        return Ok(format!(
            "No files found matching pattern '{}'.",
            input.search_pattern
        ));
    }

    // Step 2: Apply replacement in each file
    let mut results = Vec::new();
    let mut errors = Vec::new();

    for file_path in &matching_files {
        match apply_replacement(file_path, &input.old_text, &input.new_text, dry_run) {
            Ok(count) => {
                if count > 0 {
                    results.push(format!("  {}: {} replacement(s)", file_path, count));
                }
            }
            Err(e) => {
                errors.push(format!("  {}: {}", file_path, e));
            }
        }
    }

    // Step 3: Format the report
    let mut report = String::new();

    if dry_run {
        report.push_str("DRY RUN — no changes made.\n\n");
    }

    if !results.is_empty() {
        report.push_str(&format!(
            "{}:\n{}\n",
            if dry_run { "Would modify" } else { "Modified" },
            results.join("\n")
        ));
    }

    if !errors.is_empty() {
        report.push_str(&format!("\nErrors:\n{}\n", errors.join("\n")));
    }

    report.push_str(&format!(
        "\nSummary: {} files searched, {} modified, {} errors.",
        matching_files.len(),
        results.len(),
        errors.len()
    ));

    Ok(report)
}

fn search_for_files(pattern: &str, glob: &Option<String>) -> Result<Vec<String>, String> {
    // Implementation: walk the directory, filter by glob, search content
    todo!("Implement file search")
}

fn apply_replacement(
    path: &str,
    old_text: &str,
    new_text: &str,
    dry_run: bool,
) -> Result<usize, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read: {}", e))?;

    let count = content.matches(old_text).count();

    if count > 0 && !dry_run {
        let new_content = content.replace(old_text, new_text);
        std::fs::write(path, &new_content)
            .map_err(|e| format!("Cannot write: {}", e))?;
    }

    Ok(count)
}
```

**Advantages:**
- Fast -- no LLM round-trips for intermediate steps
- Consistent -- every file is processed the same way
- Token-efficient -- the model makes one call and gets a summary
- Reliable -- no risk of the model getting confused mid-workflow

**Disadvantages:**
- Less flexible -- cannot adapt to unexpected file structures
- More code to maintain -- you are writing orchestration logic
- Harder to debug -- the model does not see intermediate states

::: python Coming from Python
This is similar to the design choice in Python between writing a flexible script and writing a specific CLI command. A flexible script (`for f in files: process(f)`) handles edge cases. A specific command (`sed -i 's/old/new/g' *.rs`) is fast and consistent. In agent terms, model-driven composition is the flexible script, and system-driven composition is the specific command.
:::

## When to Use Each Approach

Here is a decision framework:

| Scenario | Recommended Approach |
|---|---|
| Simple, repetitive operation across many files | System-driven |
| Complex operation requiring judgment at each step | Model-driven |
| Operation the model performs frequently | System-driven (reduce token cost) |
| Operation that only happens occasionally | Model-driven (avoid building rarely-used tools) |
| Operation where consistency is critical | System-driven |
| Operation where adaptation is critical | Model-driven |

In practice, most coding agents start with model-driven composition for everything and add system-driven tools when they observe the model spending too many tokens on repetitive multi-step workflows.

## Transactional Tool Groups

Some operations should succeed or fail atomically. Consider a "rename symbol" operation that must update the definition, all usages, and all imports. If it updates the definition but fails halfway through the usages, the codebase is in a broken state.

Transactional tool groups handle this by collecting all changes and applying them at once:

```rust
pub struct Transaction {
    operations: Vec<FileOperation>,
}

enum FileOperation {
    Write { path: String, content: String },
    Edit { path: String, old: String, new: String },
}

impl Transaction {
    pub fn new() -> Self {
        Self { operations: Vec::new() }
    }

    pub fn add_edit(&mut self, path: String, old: String, new: String) {
        self.operations.push(FileOperation::Edit { path, old, new });
    }

    pub fn add_write(&mut self, path: String, content: String) {
        self.operations.push(FileOperation::Write { path, content });
    }

    /// Validate all operations without applying them.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        for (i, op) in self.operations.iter().enumerate() {
            match op {
                FileOperation::Edit { path, old, .. } => {
                    match std::fs::read_to_string(path) {
                        Ok(content) => {
                            let count = content.matches(old.as_str()).count();
                            if count == 0 {
                                errors.push(format!(
                                    "Operation {}: old_string not found in '{}'", i, path
                                ));
                            } else if count > 1 {
                                errors.push(format!(
                                    "Operation {}: old_string found {} times in '{}'",
                                    i, count, path
                                ));
                            }
                        }
                        Err(e) => {
                            errors.push(format!(
                                "Operation {}: cannot read '{}': {}", i, path, e
                            ));
                        }
                    }
                }
                FileOperation::Write { path, .. } => {
                    // Check parent directory exists
                    if let Some(parent) = std::path::Path::new(path).parent() {
                        if !parent.exists() {
                            errors.push(format!(
                                "Operation {}: parent directory does not exist for '{}'",
                                i, path
                            ));
                        }
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Apply all operations. Creates backups first.
    pub fn commit(&self) -> Result<String, String> {
        // First validate everything
        self.validate().map_err(|errors| {
            format!("Transaction validation failed:\n{}", errors.join("\n"))
        })?;

        // Create backups
        let mut backups: Vec<(String, Option<String>)> = Vec::new();
        for op in &self.operations {
            let path = match op {
                FileOperation::Write { path, .. } => path,
                FileOperation::Edit { path, .. } => path,
            };
            let backup = std::fs::read_to_string(path).ok();
            backups.push((path.clone(), backup));
        }

        // Apply all operations
        for op in &self.operations {
            let result = match op {
                FileOperation::Write { path, content } => {
                    std::fs::write(path, content).map_err(|e| e.to_string())
                }
                FileOperation::Edit { path, old, new } => {
                    let content = std::fs::read_to_string(path)
                        .map_err(|e| e.to_string())?;
                    let updated = content.replacen(old, new, 1);
                    std::fs::write(path, updated).map_err(|e| e.to_string())
                }
            };

            if let Err(e) = result {
                // Rollback all changes
                for (backup_path, backup_content) in &backups {
                    if let Some(content) = backup_content {
                        let _ = std::fs::write(backup_path, content);
                    }
                }
                return Err(format!("Transaction failed, rolled back: {}", e));
            }
        }

        Ok(format!(
            "Transaction committed: {} operations applied.",
            self.operations.len()
        ))
    }
}
```

The transaction validates all operations before applying any, creates backups, applies changes, and rolls back if any step fails.

::: wild In the Wild
Claude Code's edit tool operates on individual files and does not support transactions natively. Instead, it relies on the model to sequence edits correctly and uses git as a rollback mechanism (the user can `git checkout .` to undo agent changes). OpenCode takes a similar approach. True transactional editing is more common in IDE-integrated agents where the editor provides undo/redo infrastructure. For CLI agents, git integration is the pragmatic alternative to formal transactions.
:::

## Pipeline Composition

Another composition pattern is the pipeline, where the output of one tool becomes the input of the next. The model does this naturally across turns, but you can also build it into system-driven tools:

```rust
pub fn read_and_validate(path: &str) -> Result<String, String> {
    // Step 1: Read the file
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read '{}': {}", path, e))?;

    // Step 2: Check if it parses as valid Rust (if .rs file)
    if path.ends_with(".rs") {
        let syntax_check = std::process::Command::new("rustfmt")
            .arg("--check")
            .arg(path)
            .output()
            .map_err(|e| format!("Cannot run rustfmt: {}", e))?;

        if !syntax_check.status.success() {
            let stderr = String::from_utf8_lossy(&syntax_check.stderr);
            return Ok(format!(
                "{}\n\n[Warning: this file has formatting issues:\n{}]",
                content, stderr
            ));
        }
    }

    Ok(content)
}
```

This is a simple pipeline: read a file, then validate its formatting. More complex pipelines might search for files, read each one, extract specific sections, and aggregate results.

## Key Takeaways

- Model-driven composition lets the LLM orchestrate multi-step workflows -- it is flexible but slow and token-expensive
- System-driven composition handles multi-step operations inside a single tool call -- it is fast and consistent but less adaptive
- Start with model-driven composition for everything and add system-driven tools when you observe the model spending excessive tokens on repetitive workflows
- Transactional tool groups validate all operations upfront, create backups, and roll back on failure -- essential for operations that must be atomic
- Git integration serves as a pragmatic rollback mechanism for CLI agents, making formal transactions less critical than in IDE-integrated agents
