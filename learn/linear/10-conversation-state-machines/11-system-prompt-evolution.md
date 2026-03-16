---
title: System Prompt Evolution
description: Techniques for evolving the system prompt during a conversation based on learned context, activated tools, and phase transitions in the agent workflow.
---

# System Prompt Evolution

> **What you'll learn:**
> - How dynamic system prompts that incorporate learned project context outperform static prompts for long-running sessions
> - Techniques for injecting relevant file contents, project conventions, and recent decisions into the system prompt
> - Managing system prompt token budget as the conversation evolves and more context becomes available

Most tutorials treat the system prompt as a static string you set once and forget. For a coding agent, this leaves enormous value on the table. A static prompt says "You are a helpful coding assistant." A dynamic prompt says "You are working on a Rust project using Actix Web 4.x. The project follows a hexagonal architecture with ports in `src/ports/` and adapters in `src/adapters/`. The user prefers explicit error handling with `thiserror` over `anyhow`. Tests use `mockall` for mocking." The second prompt produces dramatically better responses because the model doesn't waste turns discovering this context through conversation.

## The Layered System Prompt

Organize your system prompt as layers that can be independently updated:

```rust
use std::collections::BTreeMap;

struct SystemPromptBuilder {
    /// Fixed identity and behavioral instructions (never changes)
    identity_layer: String,
    /// Project-specific context (changes when project is detected)
    project_layer: Option<String>,
    /// Tool-specific instructions (changes when tools are activated/deactivated)
    tool_layer: Option<String>,
    /// Session-specific context (changes as conversation progresses)
    session_layer: Option<String>,
    /// User preferences (loaded from config file)
    preferences_layer: Option<String>,
    /// Dynamic context injections, ordered by priority
    injections: BTreeMap<u32, ContextInjection>,
}

#[derive(Debug, Clone)]
struct ContextInjection {
    label: String,
    content: String,
    token_count: u32,
    /// When this injection was added
    added_at: std::time::Instant,
    /// Whether this injection expires after a certain duration
    ttl: Option<std::time::Duration>,
}

impl SystemPromptBuilder {
    fn new(identity: String) -> Self {
        Self {
            identity_layer: identity,
            project_layer: None,
            tool_layer: None,
            session_layer: None,
            preferences_layer: None,
            injections: BTreeMap::new(),
        }
    }

    fn set_project_context(&mut self, context: String) {
        self.project_layer = Some(context);
    }

    fn set_tool_instructions(&mut self, instructions: String) {
        self.tool_layer = Some(instructions);
    }

    fn set_session_context(&mut self, context: String) {
        self.session_layer = Some(context);
    }

    fn add_injection(&mut self, priority: u32, label: String, content: String, tokenizer: &dyn Tokenizer) {
        let token_count = tokenizer.count_tokens(&content);
        self.injections.insert(priority, ContextInjection {
            label,
            content,
            token_count,
            added_at: std::time::Instant::now(),
            ttl: None,
        });
    }

    fn build(&self, token_budget: u32, tokenizer: &dyn Tokenizer) -> String {
        let mut parts = Vec::new();
        let mut remaining_budget = token_budget;

        // Identity layer always included (it's the core prompt)
        let identity_tokens = tokenizer.count_tokens(&self.identity_layer);
        parts.push(self.identity_layer.clone());
        remaining_budget = remaining_budget.saturating_sub(identity_tokens);

        // Add layers in priority order, respecting budget
        let optional_layers = [
            ("Project Context", &self.project_layer),
            ("User Preferences", &self.preferences_layer),
            ("Tool Instructions", &self.tool_layer),
            ("Session Context", &self.session_layer),
        ];

        for (label, layer) in &optional_layers {
            if let Some(content) = layer {
                let tokens = tokenizer.count_tokens(content);
                if tokens <= remaining_budget {
                    parts.push(format!("\n## {}\n{}", label, content));
                    remaining_budget -= tokens;
                }
            }
        }

        // Add dynamic injections, highest priority first
        for (_, injection) in self.injections.iter().rev() {
            // Skip expired injections
            if let Some(ttl) = injection.ttl {
                if injection.added_at.elapsed() > ttl {
                    continue;
                }
            }

            if injection.token_count <= remaining_budget {
                parts.push(format!("\n## {}\n{}", injection.label, injection.content));
                remaining_budget -= injection.token_count;
            }
        }

        parts.join("\n\n")
    }

    fn total_tokens(&self, tokenizer: &dyn Tokenizer) -> u32 {
        let full_prompt = self.build(u32::MAX, tokenizer);
        tokenizer.count_tokens(&full_prompt)
    }
}
```

The `build` method respects a token budget by including layers in priority order. The identity layer always fits (it's the non-negotiable core). Project context comes next because it has the highest impact on response quality. Tool instructions and session context fill remaining space.

::: python Coming from Python
In Python, you might build a dynamic prompt with f-strings and string concatenation. The layered builder pattern here is functionally equivalent but more structured. The `BTreeMap` for injections is like Python's `SortedDict` from the `sortedcontainers` library -- it keeps injections ordered by priority so the builder can iterate from highest to lowest.
:::

## Project Context Detection

When a user starts a session, the agent should automatically detect project characteristics and inject them into the system prompt:

```rust
use std::path::Path;

struct ProjectDetector;

impl ProjectDetector {
    fn detect(working_dir: &Path) -> Option<ProjectContext> {
        let mut context = ProjectContext::default();

        // Detect language/framework from config files
        if working_dir.join("Cargo.toml").exists() {
            context.language = Some("Rust".into());
            if let Ok(toml) = std::fs::read_to_string(working_dir.join("Cargo.toml")) {
                context.dependencies = Self::parse_cargo_deps(&toml);
            }
        } else if working_dir.join("package.json").exists() {
            context.language = Some("JavaScript/TypeScript".into());
        } else if working_dir.join("pyproject.toml").exists()
            || working_dir.join("setup.py").exists() {
            context.language = Some("Python".into());
        }

        // Detect project structure
        if working_dir.join("src/lib.rs").exists() {
            context.project_type = Some("Rust library".into());
        } else if working_dir.join("src/main.rs").exists() {
            context.project_type = Some("Rust binary".into());
        }

        // Load AGENTS.md or CLAUDE.md for custom instructions
        for instructions_file in &["AGENTS.md", "CLAUDE.md", ".agent-instructions"] {
            let path = working_dir.join(instructions_file);
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    context.custom_instructions = Some(content);
                    break;
                }
            }
        }

        // Detect conventions from existing code
        if let Some(conventions) = Self::detect_conventions(working_dir) {
            context.conventions = conventions;
        }

        Some(context)
    }

    fn parse_cargo_deps(toml_content: &str) -> Vec<String> {
        // Simplified dependency extraction
        let mut deps = Vec::new();
        let mut in_deps = false;
        for line in toml_content.lines() {
            if line.starts_with("[dependencies]") {
                in_deps = true;
                continue;
            }
            if line.starts_with('[') {
                in_deps = false;
                continue;
            }
            if in_deps {
                if let Some(name) = line.split('=').next() {
                    let name = name.trim();
                    if !name.is_empty() {
                        deps.push(name.to_string());
                    }
                }
            }
        }
        deps
    }

    fn detect_conventions(working_dir: &Path) -> Option<Vec<String>> {
        let mut conventions = Vec::new();

        // Check for common Rust patterns
        if working_dir.join(".rustfmt.toml").exists() {
            conventions.push("Uses rustfmt with custom config".into());
        }
        if working_dir.join("clippy.toml").exists() {
            conventions.push("Uses clippy with custom lints".into());
        }
        if working_dir.join("tests").is_dir() {
            conventions.push("Integration tests in tests/ directory".into());
        }

        if conventions.is_empty() {
            None
        } else {
            Some(conventions)
        }
    }
}

#[derive(Debug, Default)]
struct ProjectContext {
    language: Option<String>,
    project_type: Option<String>,
    dependencies: Vec<String>,
    conventions: Vec<String>,
    custom_instructions: Option<String>,
}

impl ProjectContext {
    fn to_prompt_section(&self) -> String {
        let mut parts = Vec::new();

        if let Some(ref lang) = self.language {
            parts.push(format!("Language: {}", lang));
        }
        if let Some(ref project_type) = self.project_type {
            parts.push(format!("Project type: {}", project_type));
        }
        if !self.dependencies.is_empty() {
            parts.push(format!("Key dependencies: {}", self.dependencies.join(", ")));
        }
        if !self.conventions.is_empty() {
            parts.push(format!("Conventions:\n- {}", self.conventions.join("\n- ")));
        }
        if let Some(ref instructions) = self.custom_instructions {
            parts.push(format!("Custom instructions:\n{}", instructions));
        }

        parts.join("\n")
    }
}
```

::: tip In the Wild
Claude Code reads a `CLAUDE.md` file from the project root (and parent directories) to inject custom instructions into its system prompt. This file can contain project-specific coding conventions, preferred patterns, and even instructions about which files to avoid modifying. The system prompt is rebuilt for each API call, incorporating the latest project context. This means if a user creates a `CLAUDE.md` file mid-session, the very next LLM call will include those instructions. OpenCode supports a similar mechanism with its `.opencode` configuration directory.
:::

## Session-Adaptive Prompt Updates

As the conversation progresses, the system prompt should evolve to reflect what the agent has learned:

```rust
struct PromptEvolver {
    builder: SystemPromptBuilder,
    /// Track which files have been discussed
    discussed_files: Vec<String>,
    /// Track decisions made during the session
    session_decisions: Vec<String>,
    /// The current phase of work
    current_phase: WorkPhase,
}

#[derive(Debug, Clone)]
enum WorkPhase {
    Understanding,  // Reading code, asking questions
    Planning,       // Designing the solution
    Implementing,   // Writing code
    Debugging,      // Fixing issues
    Reviewing,      // Checking work
}

impl PromptEvolver {
    fn on_tool_result(&mut self, tool_name: &str, result: &str, tokenizer: &dyn Tokenizer) {
        // Track file reads to build project awareness
        if tool_name == "read_file" || tool_name == "search_files" {
            if let Some(path) = self.extract_file_path(result) {
                if !self.discussed_files.contains(&path) {
                    self.discussed_files.push(path);
                    self.update_session_context(tokenizer);
                }
            }
        }
    }

    fn on_assistant_message(&mut self, message: &str, tokenizer: &dyn Tokenizer) {
        // Detect phase transitions from the assistant's language
        let new_phase = if message.contains("let me understand")
            || message.contains("looking at") {
            Some(WorkPhase::Understanding)
        } else if message.contains("here's my plan")
            || message.contains("I'll approach this") {
            Some(WorkPhase::Planning)
        } else if message.contains("let me implement")
            || message.contains("I'll write") {
            Some(WorkPhase::Implementing)
        } else if message.contains("error") || message.contains("debug") {
            Some(WorkPhase::Debugging)
        } else {
            None
        };

        if let Some(phase) = new_phase {
            self.current_phase = phase;
            self.update_session_context(tokenizer);
        }
    }

    fn update_session_context(&mut self, tokenizer: &dyn Tokenizer) {
        let context = format!(
            "Current work phase: {:?}\n\
             Files involved in this session: {}\n\
             Key decisions made:\n{}",
            self.current_phase,
            if self.discussed_files.is_empty() {
                "none yet".to_string()
            } else {
                self.discussed_files.join(", ")
            },
            if self.session_decisions.is_empty() {
                "- none yet".to_string()
            } else {
                self.session_decisions.iter()
                    .map(|d| format!("- {}", d))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        );

        self.builder.set_session_context(context);
    }

    fn extract_file_path(&self, content: &str) -> Option<String> {
        // Simple heuristic: look for paths with extensions
        content.lines()
            .find(|line| line.contains('/') && line.contains('.'))
            .map(|line| line.trim().to_string())
    }
}
```

The prompt evolves through three mechanisms: **project detection** (run once at session start), **tool result tracking** (update as the agent reads files), and **phase detection** (infer the current work phase from the agent's language). Each update rebuilds the session context layer without touching the identity or project layers.

## Prompt Token Budget Management

Dynamic system prompts can grow unboundedly if you inject every piece of discovered context. Set explicit limits:

```rust
impl SystemPromptBuilder {
    fn build_with_budget_report(
        &self,
        token_budget: u32,
        tokenizer: &dyn Tokenizer,
    ) -> (String, PromptBudgetReport) {
        let prompt = self.build(token_budget, tokenizer);
        let actual_tokens = tokenizer.count_tokens(&prompt);

        let report = PromptBudgetReport {
            budget: token_budget,
            used: actual_tokens,
            identity_tokens: tokenizer.count_tokens(&self.identity_layer),
            project_tokens: self.project_layer.as_ref()
                .map(|p| tokenizer.count_tokens(p))
                .unwrap_or(0),
            tool_tokens: self.tool_layer.as_ref()
                .map(|t| tokenizer.count_tokens(t))
                .unwrap_or(0),
            session_tokens: self.session_layer.as_ref()
                .map(|s| tokenizer.count_tokens(s))
                .unwrap_or(0),
            injection_count: self.injections.len(),
            layers_dropped: self.count_dropped_layers(token_budget, tokenizer),
        };

        (prompt, report)
    }

    fn count_dropped_layers(&self, budget: u32, tokenizer: &dyn Tokenizer) -> usize {
        let full_tokens = self.total_tokens(tokenizer);
        if full_tokens <= budget {
            0
        } else {
            // Estimate how many layers were dropped
            let over_budget = full_tokens - budget;
            let avg_layer_size = full_tokens / (4 + self.injections.len() as u32).max(1);
            (over_budget / avg_layer_size.max(1)) as usize
        }
    }
}

#[derive(Debug)]
struct PromptBudgetReport {
    budget: u32,
    used: u32,
    identity_tokens: u32,
    project_tokens: u32,
    tool_tokens: u32,
    session_tokens: u32,
    injection_count: usize,
    layers_dropped: usize,
}
```

If the system prompt exceeds its budget, the builder drops the lowest-priority layers. This is better than truncating mid-sentence -- you lose entire sections cleanly, and the identity layer (the most important part) is always preserved.

## Key Takeaways

- Organize system prompts in layers (identity, project, tools, session, preferences) that can be independently updated and have clear priority ordering.
- Detect project context automatically from config files (`Cargo.toml`, `CLAUDE.md`, `.rustfmt.toml`) and inject it into the system prompt at session start.
- Evolve the session context layer as the conversation progresses, tracking discussed files, decisions made, and the current work phase.
- Set explicit token budgets for the system prompt and drop low-priority layers when the budget is exceeded -- never truncate the identity layer.
- Rebuild the system prompt before each API call so it always reflects the latest context, and log a budget report to monitor prompt growth over time.
