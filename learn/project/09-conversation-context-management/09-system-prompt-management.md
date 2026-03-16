---
title: System Prompt Management
description: Design a system prompt architecture that composes static instructions, dynamic context, and tool documentation efficiently.
---

# System Prompt Management

> **What you'll learn:**
> - How to structure system prompts with layered sections for identity, capabilities, and constraints
> - How to dynamically inject context-dependent instructions based on the current project and tools
> - How to minimize system prompt token usage through templating and conditional inclusion

The system prompt is sent with every API request. It is the single most expensive piece of context in your agent because it never gets compacted, summarized, or dropped. Every token in the system prompt is a token you pay for on every turn of every session. This makes system prompt optimization one of the highest-leverage improvements you can make.

## Anatomy of an Agent System Prompt

A coding agent's system prompt has several distinct sections, each with a different purpose:

```rust
/// Sections that compose a system prompt.
pub struct SystemPromptBuilder {
    sections: Vec<PromptSection>,
}

#[derive(Debug, Clone)]
pub struct PromptSection {
    /// Section identifier for debugging
    pub name: String,
    /// The actual content
    pub content: String,
    /// Whether this section is required or can be omitted under pressure
    pub required: bool,
    /// Pre-computed token count
    pub token_count: usize,
}

impl SystemPromptBuilder {
    pub fn new() -> Self {
        Self {
            sections: Vec::new(),
        }
    }

    /// Add a required section that is always included.
    pub fn add_required(&mut self, name: &str, content: String, token_count: usize) {
        self.sections.push(PromptSection {
            name: name.to_string(),
            content,
            required: true,
            token_count,
        });
    }

    /// Add an optional section that can be dropped if the budget is tight.
    pub fn add_optional(&mut self, name: &str, content: String, token_count: usize) {
        self.sections.push(PromptSection {
            name: name.to_string(),
            content,
            required: false,
            token_count,
        });
    }

    /// Build the final system prompt, respecting a token budget.
    /// Required sections are always included. Optional sections are
    /// included in order until the budget runs out.
    pub fn build(&self, max_tokens: usize) -> (String, usize) {
        let mut result = String::new();
        let mut total_tokens = 0;

        // Always include required sections
        for section in &self.sections {
            if section.required {
                if !result.is_empty() {
                    result.push_str("\n\n");
                }
                result.push_str(&section.content);
                total_tokens += section.token_count;
            }
        }

        // Include optional sections if budget allows
        for section in &self.sections {
            if !section.required && total_tokens + section.token_count <= max_tokens {
                result.push_str("\n\n");
                result.push_str(&section.content);
                total_tokens += section.token_count;
            }
        }

        (result, total_tokens)
    }

    /// Report which sections are included and their token costs.
    pub fn breakdown(&self, max_tokens: usize) -> Vec<(String, usize, bool)> {
        let mut total = 0;
        self.sections.iter().map(|s| {
            let included = if s.required {
                total += s.token_count;
                true
            } else if total + s.token_count <= max_tokens {
                total += s.token_count;
                true
            } else {
                false
            };
            (s.name.clone(), s.token_count, included)
        }).collect()
    }
}

fn main() {
    let mut builder = SystemPromptBuilder::new();

    // Required: Identity and core behavior
    builder.add_required("identity",
        "You are an AI coding assistant. You help users write, debug, and understand code. \
         You have access to tools for reading files, writing files, and executing shell commands."
            .to_string(),
        30,
    );

    // Required: Safety constraints
    builder.add_required("safety",
        "IMPORTANT: Never execute destructive commands without user confirmation. \
         Never modify files outside the current project directory. \
         Always explain what you are about to do before using a tool."
            .to_string(),
        35,
    );

    // Optional: Coding style preferences
    builder.add_optional("style",
        "When writing Rust code, prefer: explicit error handling with Result, \
         descriptive variable names, documentation comments on public items, \
         and small focused functions."
            .to_string(),
        30,
    );

    // Optional: Project context
    builder.add_optional("project_context",
        "Current project: a CLI coding agent written in Rust. \
         Uses tokio for async, serde for serialization, reqwest for HTTP. \
         The main entry point is src/main.rs."
            .to_string(),
        35,
    );

    // Optional: Detailed examples
    builder.add_optional("examples",
        "When the user asks you to fix a bug, follow this process: \
         1. Read the relevant file. 2. Identify the issue. 3. Explain the fix. \
         4. Write the corrected code. 5. Suggest a test to verify."
            .to_string(),
        40,
    );

    // Build with different budgets
    for budget in [200, 100, 60] {
        let (prompt, tokens) = builder.build(budget);
        println!("Budget {}: {} tokens used, {} chars", budget, tokens, prompt.len());
        println!("Sections included:");
        for (name, count, included) in builder.breakdown(budget) {
            println!("  {} {} ({} tokens) {}",
                if included { "+" } else { "-" },
                name, count,
                if included { "" } else { "[SKIPPED]" });
        }
        println!();
    }
}
```

The builder pattern lets you compose the system prompt from modular sections. Required sections (identity, safety) are always included. Optional sections (style, project context, examples) are included only when the budget allows. This means your system prompt automatically adapts to tight contexts without you needing to manually trim it.

::: python Coming from Python
In Python, you might build system prompts with string concatenation or templates:
```python
sections = [
    ("identity", "You are a coding assistant.", True),
    ("safety", "Never run dangerous commands.", True),
    ("style", "Use type hints in Python code.", False),
]
prompt = "\n\n".join(s[1] for s in sections
                      if s[2] or token_count < budget)
```
The Rust version uses a dedicated `PromptSection` struct instead of tuples,
which makes the code self-documenting. The `required` field is a `bool` rather
than a position in a tuple, so you cannot mix up the arguments.
:::

## Dynamic Context Injection

A static system prompt misses crucial context. Your agent should inject information that changes per session:

```rust
use std::path::Path;

/// Generates dynamic system prompt sections based on the current environment.
pub struct DynamicContext;

impl DynamicContext {
    /// Detect the project type and return relevant instructions.
    pub fn project_instructions(working_dir: &Path) -> Option<PromptSection> {
        // Check for Cargo.toml -> Rust project
        if working_dir.join("Cargo.toml").exists() {
            return Some(PromptSection {
                name: "project_type".to_string(),
                content: "This is a Rust project using Cargo. Use `cargo check` to verify \
                          compilation, `cargo test` to run tests, and `cargo clippy` for linting. \
                          Prefer idiomatic Rust patterns."
                    .to_string(),
                required: false,
                token_count: 30,
            });
        }

        // Check for package.json -> Node.js project
        if working_dir.join("package.json").exists() {
            return Some(PromptSection {
                name: "project_type".to_string(),
                content: "This is a Node.js/TypeScript project. Use `npm test` to run tests \
                          and `npm run build` to compile. Check tsconfig.json for TypeScript settings."
                    .to_string(),
                required: false,
                token_count: 30,
            });
        }

        // Check for pyproject.toml or requirements.txt -> Python project
        if working_dir.join("pyproject.toml").exists()
            || working_dir.join("requirements.txt").exists()
        {
            return Some(PromptSection {
                name: "project_type".to_string(),
                content: "This is a Python project. Use `pytest` to run tests and check \
                          pyproject.toml for project configuration and dependencies."
                    .to_string(),
                required: false,
                token_count: 25,
            });
        }

        None
    }

    /// Generate a section listing available tools.
    pub fn tool_summary(tool_names: &[&str]) -> PromptSection {
        let tools = tool_names.join(", ");
        PromptSection {
            name: "available_tools".to_string(),
            content: format!(
                "You have access to the following tools: {}. \
                 Use them when you need to interact with the filesystem or run commands. \
                 Always prefer reading files before making assumptions about their content.",
                tools
            ),
            required: true,
            token_count: 20 + tool_names.len() * 3,
        }
    }

    /// Generate a section with the current git state.
    pub fn git_context(branch: &str, has_uncommitted: bool) -> PromptSection {
        let mut content = format!("Current git branch: {}", branch);
        if has_uncommitted {
            content.push_str(". There are uncommitted changes in the working directory.");
        }
        let token_count = (content.len() as f64 * 0.3) as usize;
        PromptSection {
            name: "git_context".to_string(),
            content,
            required: false,
            token_count,
        }
    }
}

fn main() {
    let mut builder = SystemPromptBuilder::new();

    // Static sections
    builder.add_required("identity",
        "You are an AI coding assistant with access to file and shell tools.".to_string(),
        15,
    );

    // Dynamic sections
    let tools = DynamicContext::tool_summary(&["read_file", "write_file", "shell"]);
    builder.sections.push(tools);

    let git = DynamicContext::git_context("feature/auth-refactor", true);
    builder.sections.push(git);

    // Project type (would use actual working directory in production)
    let project = PromptSection {
        name: "project_type".to_string(),
        content: "This is a Rust project using Cargo.".to_string(),
        required: false,
        token_count: 8,
    };
    builder.sections.push(project);

    let (prompt, tokens) = builder.build(200);
    println!("System prompt ({} tokens):\n{}", tokens, prompt);
}
```

## Caching the System Prompt

Since the system prompt is sent with every request, you should cache the built prompt and only rebuild it when something changes:

```rust
use std::time::{Duration, Instant};

/// A cached system prompt that rebuilds only when invalidated.
pub struct CachedSystemPrompt {
    builder: SystemPromptBuilder,
    cached_prompt: Option<(String, usize)>,
    cached_at: Option<Instant>,
    /// Rebuild the cache after this duration
    max_age: Duration,
    /// Current token budget for the system prompt
    budget: usize,
}

impl CachedSystemPrompt {
    pub fn new(builder: SystemPromptBuilder, budget: usize) -> Self {
        Self {
            builder,
            cached_prompt: None,
            cached_at: None,
            max_age: Duration::from_secs(300), // Rebuild every 5 minutes
            budget,
        }
    }

    /// Get the current system prompt, rebuilding if stale.
    pub fn get(&mut self) -> (&str, usize) {
        let needs_rebuild = match self.cached_at {
            None => true,
            Some(t) => t.elapsed() > self.max_age,
        };

        if needs_rebuild {
            let (prompt, tokens) = self.builder.build(self.budget);
            self.cached_prompt = Some((prompt, tokens));
            self.cached_at = Some(Instant::now());
        }

        let (ref prompt, tokens) = self.cached_prompt.as_ref().unwrap();
        (prompt, *tokens)
    }

    /// Force a rebuild (call when tools change, project changes, etc.).
    pub fn invalidate(&mut self) {
        self.cached_at = None;
    }

    /// Update the token budget (e.g., when switching models).
    pub fn set_budget(&mut self, budget: usize) {
        if budget != self.budget {
            self.budget = budget;
            self.invalidate();
        }
    }
}

fn main() {
    let mut builder = SystemPromptBuilder::new();
    builder.add_required("identity", "You are a coding assistant.".to_string(), 8);
    builder.add_optional("style", "Write clean, idiomatic code.".to_string(), 7);

    let mut cached = CachedSystemPrompt::new(builder, 50);

    // First call builds the cache
    let (prompt, tokens) = cached.get();
    println!("First call: {} tokens", tokens);

    // Second call hits the cache
    let (prompt2, tokens2) = cached.get();
    println!("Second call: {} tokens (cached)", tokens2);

    // Invalidate and rebuild
    cached.invalidate();
    let (prompt3, tokens3) = cached.get();
    println!("After invalidation: {} tokens (rebuilt)", tokens3);
}
```

::: wild In the Wild
Claude Code composes its system prompt from multiple sources: a base identity prompt, tool documentation (generated from the tool registry), project-specific instructions from CLAUDE.md files, and dynamic context like the current git branch. The total system prompt can be 2,000--5,000 tokens depending on the number of registered tools and the project configuration. OpenCode follows a similar pattern with layered prompt composition and caching.
:::

## Measuring System Prompt Efficiency

Track how much of your context budget the system prompt consumes:

```rust
fn analyze_system_prompt_efficiency(
    system_prompt_tokens: usize,
    model_limit: usize,
    response_reserve: usize,
) {
    let usable = model_limit - response_reserve;
    let percentage = system_prompt_tokens as f64 / usable as f64 * 100.0;

    println!("System prompt: {} tokens ({:.1}% of usable context)",
        system_prompt_tokens, percentage);

    if percentage > 5.0 {
        println!("WARNING: System prompt is consuming more than 5% of context.");
        println!("Consider condensing optional sections or moving detailed");
        println!("instructions into the conversation as needed.");
    } else {
        println!("System prompt size is within healthy limits.");
    }
}

fn main() {
    // Test with different prompt sizes
    analyze_system_prompt_efficiency(1_000, 200_000, 8_000);
    println!();
    analyze_system_prompt_efficiency(10_000, 200_000, 8_000);
}
```

A well-optimized system prompt for a coding agent should be under 3% of the usable context. If it creeps above 5%, you are paying a significant tax on every request.

## Key Takeaways

- The system prompt is sent with every request and never gets compacted -- every token in it is a per-turn cost multiplied by the length of the session
- Use a builder pattern with required and optional sections so the prompt adapts automatically to the available budget
- Inject dynamic context (project type, git branch, available tools) into the system prompt so the model has up-to-date awareness
- Cache the built system prompt and rebuild it only when inputs change (tool registration, project switch, budget change)
- Keep the system prompt under 3% of usable context -- move detailed instructions to optional sections or inject them as conversation messages when needed
