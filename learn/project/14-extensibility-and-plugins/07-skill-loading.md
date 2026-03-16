---
title: Skill Loading
description: Implementing skill packages that bundle related tools, prompts, and configuration into loadable units the agent can activate based on context or user request.
---

# Skill Loading

> **What you'll learn:**
> - How to define skill packages that bundle tools, system prompts, and default configurations
> - How to implement a skill loader that activates and deactivates skills at runtime
> - Patterns for context-aware skill activation based on project type or user commands

Individual tools are useful, but real workflows require coordinated sets of tools, prompts, and configurations. A "Rust development" skill might bundle a Cargo runner tool, a Rust-specific system prompt, and knowledge about Rust project conventions. A "database" skill might bundle SQL tools, schema inspection prompts, and connection configuration. Skills are the packaging layer that turns loose tools into coherent capabilities.

## What Is a Skill?

A skill is a bundle of related extension components that work together:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// A skill package that bundles tools, prompts, and configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDefinition {
    /// Unique identifier for this skill.
    pub name: String,
    /// Human-readable display name.
    pub display_name: String,
    /// What this skill enables.
    pub description: String,
    /// Version of the skill package.
    pub version: String,
    /// File patterns that trigger auto-activation (e.g., "Cargo.toml", "*.py").
    pub activation_triggers: Vec<String>,
    /// Tool definitions this skill provides.
    pub tools: Vec<ToolDefinition>,
    /// Additional system prompt text injected when the skill is active.
    pub system_prompt_additions: Vec<String>,
    /// Default configuration values.
    pub default_config: HashMap<String, Value>,
    /// Other skills this skill depends on.
    pub dependencies: Vec<String>,
}

/// Runtime state for a loaded skill.
#[derive(Debug)]
pub struct LoadedSkill {
    pub definition: SkillDefinition,
    pub active: bool,
    pub registered_tool_names: Vec<String>,
}
```

::: tip Coming from Python
In Python, you might think of skills as something like namespace packages or Django apps -- self-contained bundles of related functionality:
```python
# A Python "skill" might be a package with a manifest
# skills/rust_dev/__init__.py
SKILL_MANIFEST = {
    "name": "rust-dev",
    "tools": [cargo_run, cargo_test, cargo_check],
    "system_prompt": "You are working on a Rust project...",
    "triggers": ["Cargo.toml"],
}
```
Rust does not have Python's dynamic package loading, but the concept is identical: a manifest declares what the skill provides, and the loader activates it based on context. The key difference is that Rust skills are either compiled in (as data structures) or loaded from configuration files.
:::

## The Skill Loader

The skill loader manages a catalog of available skills and handles activation and deactivation:

```rust
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct SkillLoader {
    available: HashMap<String, SkillDefinition>,
    loaded: HashMap<String, LoadedSkill>,
    tool_registry: Arc<RwLock<ToolRegistry>>,
}

#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("Skill '{0}' not found")]
    NotFound(String),
    #[error("Skill '{0}' is already active")]
    AlreadyActive(String),
    #[error("Skill '{0}' is not active")]
    NotActive(String),
    #[error("Dependency '{dep}' required by skill '{skill}' is not available")]
    MissingDependency { skill: String, dep: String },
    #[error("Failed to load skill: {0}")]
    LoadError(String),
}

impl SkillLoader {
    pub fn new(tool_registry: Arc<RwLock<ToolRegistry>>) -> Self {
        Self {
            available: HashMap::new(),
            loaded: HashMap::new(),
            tool_registry,
        }
    }

    /// Register a skill definition in the catalog.
    pub fn register_skill(&mut self, skill: SkillDefinition) {
        println!("[skills] Registered skill: {}", skill.name);
        self.available.insert(skill.name.clone(), skill);
    }

    /// Load skills from a directory of TOML definition files.
    pub fn load_skills_from_dir(&mut self, dir: &Path) -> Result<(), SkillError> {
        if !dir.exists() {
            return Ok(()); // No skills directory is fine
        }

        let entries = std::fs::read_dir(dir)
            .map_err(|e| SkillError::LoadError(format!("Cannot read dir: {}", e)))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "toml") {
                let content = std::fs::read_to_string(&path)
                    .map_err(|e| SkillError::LoadError(
                        format!("Cannot read {:?}: {}", path, e),
                    ))?;

                let skill: SkillDefinition = toml::from_str(&content)
                    .map_err(|e| SkillError::LoadError(
                        format!("Invalid TOML in {:?}: {}", path, e),
                    ))?;

                self.register_skill(skill);
            }
        }

        Ok(())
    }

    /// Activate a skill by name, registering its tools and prompt additions.
    pub async fn activate(&mut self, skill_name: &str) -> Result<(), SkillError> {
        // Check if already active
        if self.loaded.get(skill_name).map_or(false, |s| s.active) {
            return Err(SkillError::AlreadyActive(skill_name.to_string()));
        }

        // Look up the skill definition
        let definition = self
            .available
            .get(skill_name)
            .cloned()
            .ok_or_else(|| SkillError::NotFound(skill_name.to_string()))?;

        // Activate dependencies first
        for dep in &definition.dependencies {
            if !self.loaded.get(dep).map_or(false, |s| s.active) {
                if self.available.contains_key(dep) {
                    // Recursive activation -- use Box::pin for async recursion
                    self.activate(dep).await?;
                } else {
                    return Err(SkillError::MissingDependency {
                        skill: skill_name.to_string(),
                        dep: dep.clone(),
                    });
                }
            }
        }

        // Register the skill's tools
        let mut registered_names = Vec::new();
        {
            let mut registry = self.tool_registry.write().await;
            for tool_def in &definition.tools {
                let tool_name = tool_def.name.clone();
                let description = tool_def.description.clone();
                let params = tool_def.parameters.clone();

                // Create a simple handler that delegates to the skill's tool logic
                let handler_def = ToolDefinition {
                    name: tool_name.clone(),
                    description,
                    parameters: params,
                };

                // For config-defined skills, the handler is a shell command wrapper
                let result = registry.register(
                    skill_name,
                    handler_def,
                    Arc::new(move |params| {
                        Box::pin(async move {
                            // Default handler returns a placeholder
                            // Real implementations override this during skill setup
                            Ok(serde_json::json!({
                                "status": "ok",
                                "params": params
                            }))
                        })
                    }),
                );

                match result {
                    Ok(()) => registered_names.push(tool_name),
                    Err(e) => {
                        eprintln!(
                            "[skills] Warning: could not register tool '{}': {}",
                            tool_name, e
                        );
                    }
                }
            }
        }

        println!(
            "[skills] Activated '{}' with {} tools",
            skill_name,
            registered_names.len()
        );

        self.loaded.insert(
            skill_name.to_string(),
            LoadedSkill {
                definition,
                active: true,
                registered_tool_names: registered_names,
            },
        );

        Ok(())
    }

    /// Deactivate a skill, removing its tools from the registry.
    pub async fn deactivate(&mut self, skill_name: &str) -> Result<(), SkillError> {
        let skill = self
            .loaded
            .get(skill_name)
            .ok_or_else(|| SkillError::NotActive(skill_name.to_string()))?;

        if !skill.active {
            return Err(SkillError::NotActive(skill_name.to_string()));
        }

        // Remove all tools registered by this skill
        {
            let mut registry = self.tool_registry.write().await;
            registry.deregister_all_by_owner(skill_name);
        }

        // Mark as inactive
        if let Some(loaded) = self.loaded.get_mut(skill_name) {
            loaded.active = false;
            loaded.registered_tool_names.clear();
        }

        println!("[skills] Deactivated '{}'", skill_name);
        Ok(())
    }

    /// Get system prompt additions from all active skills.
    pub fn active_prompt_additions(&self) -> Vec<String> {
        self.loaded
            .values()
            .filter(|s| s.active)
            .flat_map(|s| s.definition.system_prompt_additions.clone())
            .collect()
    }

    /// List all available skills and their activation status.
    pub fn list_skills(&self) -> Vec<SkillStatus> {
        self.available
            .values()
            .map(|def| SkillStatus {
                name: def.name.clone(),
                display_name: def.display_name.clone(),
                description: def.description.clone(),
                active: self
                    .loaded
                    .get(&def.name)
                    .map_or(false, |s| s.active),
            })
            .collect()
    }
}

#[derive(Debug, Serialize)]
pub struct SkillStatus {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub active: bool,
}
```

## Context-Aware Skill Activation

A powerful pattern is automatically activating skills based on the project context. When the user opens a Rust project, the Rust development skill should activate without being asked:

```rust
impl SkillLoader {
    /// Auto-activate skills based on files present in the working directory.
    pub async fn auto_activate_for_directory(
        &mut self,
        working_dir: &Path,
    ) -> Result<Vec<String>, SkillError> {
        let mut activated = Vec::new();

        for (name, definition) in &self.available.clone() {
            // Skip if already active
            if self.loaded.get(name).map_or(false, |s| s.active) {
                continue;
            }

            let should_activate = definition.activation_triggers.iter().any(|trigger| {
                if trigger.contains('*') {
                    // Glob pattern matching
                    glob_match(trigger, working_dir)
                } else {
                    // Exact file check
                    working_dir.join(trigger).exists()
                }
            });

            if should_activate {
                match self.activate(name).await {
                    Ok(()) => {
                        activated.push(name.clone());
                    }
                    Err(e) => {
                        eprintln!(
                            "[skills] Auto-activation of '{}' failed: {}",
                            name, e
                        );
                    }
                }
            }
        }

        if !activated.is_empty() {
            println!(
                "[skills] Auto-activated skills: {}",
                activated.join(", ")
            );
        }

        Ok(activated)
    }
}

/// Simple glob matching for activation triggers.
fn glob_match(pattern: &str, dir: &Path) -> bool {
    // Check if any file in the directory matches the pattern
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();

            if pattern.starts_with("*.") {
                let ext = &pattern[1..]; // ".rs", ".py", etc.
                if name.ends_with(ext) {
                    return true;
                }
            }
        }
    }
    false
}
```

## Example Skill Definitions

A Rust development skill defined in TOML:

```toml
name = "rust-dev"
display_name = "Rust Development"
description = "Tools and prompts for Rust project development"
version = "1.0.0"
activation_triggers = ["Cargo.toml"]
dependencies = []

[[system_prompt_additions]]
text = """
You are working on a Rust project. Use `cargo check` to verify code compiles,
`cargo test` to run tests, and `cargo clippy` for lint checks. Prefer idiomatic
Rust patterns: use Result for error handling, prefer iterators over manual loops,
and leverage the type system for safety.
"""

[[tools]]
name = "cargo_run"
description = "Run a cargo command in the project"

[tools.parameters]
type = "object"

[tools.parameters.properties.subcommand]
type = "string"
description = "The cargo subcommand to run (build, test, check, clippy, run)"

[tools.parameters.properties.args]
type = "string"
description = "Additional arguments to pass to cargo"

[tools.parameters.required]
values = ["subcommand"]
```

A Python development skill:

```toml
name = "python-dev"
display_name = "Python Development"
description = "Tools and prompts for Python project development"
version = "1.0.0"
activation_triggers = ["pyproject.toml", "setup.py", "requirements.txt"]
dependencies = []

[[system_prompt_additions]]
text = """
You are working on a Python project. Use pytest for testing, ruff or flake8 for
linting, and mypy for type checking. Follow PEP 8 style guidelines and use
type hints for function signatures.
"""

[[tools]]
name = "pytest_run"
description = "Run pytest with optional arguments"

[tools.parameters]
type = "object"

[tools.parameters.properties.args]
type = "string"
description = "Arguments to pass to pytest (e.g., '-v', 'tests/test_foo.py')"
```

::: info In the Wild
Claude Code implements a concept similar to skills through its slash commands and CLAUDE.md project files. When you open a project that contains a `CLAUDE.md` file, that file's contents are injected into the system prompt -- effectively a project-specific skill. The `/init` command generates this file based on project analysis. This lightweight approach achieves context-aware behavior without a formal skill system: the project file is the skill.
:::

## Integrating Skills with the System Prompt

Active skills inject their prompts into the LLM's system message:

```rust
impl Agent {
    fn build_system_prompt(&self, skill_loader: &SkillLoader) -> String {
        let mut prompt = String::from(
            "You are a coding assistant. You have access to the following tools...\n\n"
        );

        // Append skill-specific guidance
        let additions = skill_loader.active_prompt_additions();
        if !additions.is_empty() {
            prompt.push_str("\n## Active Skills\n\n");
            for addition in additions {
                prompt.push_str(&addition);
                prompt.push_str("\n\n");
            }
        }

        prompt
    }
}
```

This is where skills become more than just tool bundles. The system prompt additions give the LLM domain-specific knowledge about how to use the tools effectively, what conventions to follow, and what patterns to apply.

## Key Takeaways

- Skills bundle related tools, system prompt additions, and configuration into coherent capability packages that work together for a specific domain
- The skill loader manages a catalog of available skills, handles dependency resolution, and coordinates activation/deactivation of tool registrations
- Context-aware activation uses file-based triggers (like detecting `Cargo.toml` for a Rust skill) to automatically enable relevant skills without user intervention
- System prompt injection from active skills gives the LLM domain-specific guidance on how to use skill tools effectively, beyond what tool descriptions alone provide
- TOML-based skill definitions let users create and share skill packages without writing Rust code, making the skill system accessible to the broader community
