---
title: Config Files As Context
description: Load project-specific configuration files and inject them into the conversation context to give the agent project awareness.
---

# Config Files As Context

> **What you'll learn:**
> - How to discover and load project configuration files like .agent.toml or CLAUDE.md
> - How to inject file contents into the system prompt or early conversation messages
> - How to handle configuration hierarchies with global, project, and directory-level overrides

A coding agent that knows nothing about your project is not very useful. When you open your agent in a Rust project, it should know to suggest `cargo test`, not `pytest`. When the project has specific conventions -- commit message formats, forbidden patterns, required code review steps -- the agent should respect them. Configuration files give the agent this awareness.

## Configuration File Discovery

The first step is finding configuration files. Most tools follow a hierarchical pattern: global settings, project-level overrides, and directory-level specialization. Let's build a discovery system:

```rust
use std::fs;
use std::path::{Path, PathBuf};

/// A discovered configuration file with its scope.
#[derive(Debug)]
pub struct ConfigFile {
    /// Where the file was found
    pub path: PathBuf,
    /// The scope determines override priority
    pub scope: ConfigScope,
    /// The raw content of the file
    pub content: String,
    /// Token count of the content
    pub token_count: usize,
}

/// Configuration scope, from broadest to most specific.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfigScope {
    /// ~/.config/agent/config.toml -- applies to all projects
    Global,
    /// <project_root>/.agent.toml -- applies to this project
    Project,
    /// <subdirectory>/.agent.toml -- applies to this subdirectory
    Directory,
}

/// Discovers configuration files by walking up from the current directory.
pub struct ConfigDiscovery {
    /// File names to look for, in order of preference
    config_names: Vec<String>,
}

impl ConfigDiscovery {
    pub fn new() -> Self {
        Self {
            config_names: vec![
                ".agent.toml".to_string(),
                ".agent.md".to_string(),
                "AGENT.md".to_string(),
                "CLAUDE.md".to_string(),
            ],
        }
    }

    /// Discover all configuration files from the working directory up to root,
    /// plus the global config.
    pub fn discover(&self, working_dir: &Path) -> Vec<ConfigFile> {
        let mut configs = Vec::new();

        // 1. Check for global config
        if let Some(global) = self.find_global_config() {
            configs.push(global);
        }

        // 2. Walk up from working_dir to find project and directory configs
        let mut current = working_dir.to_path_buf();
        let mut found_project_root = false;

        loop {
            for name in &self.config_names {
                let config_path = current.join(name);
                if config_path.exists() {
                    if let Ok(content) = fs::read_to_string(&config_path) {
                        let scope = if !found_project_root
                            && self.is_project_root(&current)
                        {
                            found_project_root = true;
                            ConfigScope::Project
                        } else if found_project_root {
                            // Above project root -- treat as global-ish
                            continue;
                        } else {
                            ConfigScope::Directory
                        };

                        let token_count = (content.len() as f64 * 0.3) as usize;
                        configs.push(ConfigFile {
                            path: config_path,
                            scope,
                            content,
                            token_count,
                        });
                    }
                }
            }

            if self.is_project_root(&current) && !found_project_root {
                found_project_root = true;
            }

            if !current.pop() {
                break;
            }
        }

        // Sort by scope (Global < Project < Directory) so more specific overrides come last
        configs.sort_by_key(|c| c.scope);
        configs
    }

    /// Check if a directory looks like a project root.
    fn is_project_root(&self, dir: &Path) -> bool {
        let markers = [
            "Cargo.toml",
            "package.json",
            "pyproject.toml",
            ".git",
            "go.mod",
        ];
        markers.iter().any(|m| dir.join(m).exists())
    }

    /// Find the global configuration file.
    fn find_global_config(&self) -> Option<ConfigFile> {
        let home = std::env::var("HOME").ok()?;
        let config_dir = PathBuf::from(home).join(".config").join("agent");

        for name in &self.config_names {
            let path = config_dir.join(name);
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    let token_count = (content.len() as f64 * 0.3) as usize;
                    return Some(ConfigFile {
                        path,
                        scope: ConfigScope::Global,
                        content,
                        token_count,
                    });
                }
            }
        }
        None
    }
}

fn main() {
    let discovery = ConfigDiscovery::new();
    let configs = discovery.discover(Path::new("."));

    if configs.is_empty() {
        println!("No configuration files found.");
    } else {
        for config in &configs {
            println!("{:?} config at {:?} ({} tokens)",
                config.scope, config.path, config.token_count);
            println!("  First line: {}",
                config.content.lines().next().unwrap_or("(empty)"));
        }
    }
}
```

::: python Coming from Python
Python tools like `ruff` and `black` walk up the directory tree looking for
`pyproject.toml` or `setup.cfg`. The pattern is the same:
```python
def find_config(start_dir):
    current = Path(start_dir)
    while current != current.parent:
        for name in [".agent.toml", "CLAUDE.md"]:
            config = current / name
            if config.exists():
                return config
        current = current.parent
    return None
```
Rust's `Path::pop()` method modifies the path in place and returns `false` when
it reaches the root, which serves the same purpose as comparing `current` with
`current.parent` in Python's pathlib.
:::

## Parsing TOML Configuration

For structured configuration, TOML is a good format -- it is human-friendly and Rust has excellent support for it:

```toml
# Example .agent.toml
[agent]
model = "claude-sonnet-4-20250514"
max_tokens = 8192

[context]
# Additional instructions injected into the system prompt
instructions = """
This project uses a specific error handling pattern:
- All public functions return Result<T, AppError>
- Use the ? operator for propagation
- Log errors at the boundary (main, API handlers)
"""

[tools]
# Restrict which tools the agent can use
allowed = ["read_file", "write_file", "shell"]

[safety]
# Patterns that should never be executed
blocked_commands = ["rm -rf", "DROP TABLE", "git push --force"]
# Files that should never be modified
read_only_patterns = ["*.lock", "Cargo.lock", "package-lock.json"]
```

Parse it with the `toml` crate:

```rust
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct AgentConfig {
    #[serde(default)]
    pub agent: AgentSection,
    #[serde(default)]
    pub context: ContextSection,
    #[serde(default)]
    pub tools: ToolsSection,
    #[serde(default)]
    pub safety: SafetySection,
}

#[derive(Debug, Default, Deserialize)]
pub struct AgentSection {
    pub model: Option<String>,
    pub max_tokens: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ContextSection {
    pub instructions: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ToolsSection {
    #[serde(default)]
    pub allowed: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct SafetySection {
    #[serde(default)]
    pub blocked_commands: Vec<String>,
    #[serde(default)]
    pub read_only_patterns: Vec<String>,
}

impl AgentConfig {
    /// Parse a TOML configuration string.
    pub fn parse(content: &str) -> Result<Self, String> {
        toml::from_str(content)
            .map_err(|e| format!("Failed to parse config: {}", e))
    }

    /// Merge another config into this one. The other config's values
    /// take precedence (used for directory overriding project config).
    pub fn merge(&mut self, other: &AgentConfig) {
        if other.agent.model.is_some() {
            self.agent.model = other.agent.model.clone();
        }
        if other.agent.max_tokens.is_some() {
            self.agent.max_tokens = other.agent.max_tokens;
        }
        if other.context.instructions.is_some() {
            // Append directory-level instructions to project-level ones
            if let Some(ref existing) = self.context.instructions {
                self.context.instructions = Some(format!(
                    "{}\n\n{}",
                    existing,
                    other.context.instructions.as_ref().unwrap()
                ));
            } else {
                self.context.instructions = other.context.instructions.clone();
            }
        }
        if !other.tools.allowed.is_empty() {
            self.tools.allowed = other.tools.allowed.clone();
        }
        // Safety: union of all blocked patterns
        self.safety.blocked_commands.extend(other.safety.blocked_commands.clone());
        self.safety.read_only_patterns.extend(other.safety.read_only_patterns.clone());
    }
}

fn main() {
    let toml_content = r#"
[agent]
model = "claude-sonnet-4-20250514"
max_tokens = 8192

[context]
instructions = """
Use Result<T, AppError> for all public functions.
Prefer the ? operator for error propagation.
"""

[tools]
allowed = ["read_file", "write_file", "shell"]

[safety]
blocked_commands = ["rm -rf /", "DROP TABLE"]
read_only_patterns = ["*.lock"]
"#;

    match AgentConfig::parse(toml_content) {
        Ok(config) => {
            println!("Model: {:?}", config.agent.model);
            println!("Instructions: {:?}", config.context.instructions);
            println!("Allowed tools: {:?}", config.tools.allowed);
            println!("Blocked commands: {:?}", config.safety.blocked_commands);
        }
        Err(e) => println!("Error: {}", e),
    }
}
```

## Markdown Configuration Files

Some projects prefer a markdown file like `CLAUDE.md` or `AGENT.md` that contains free-form instructions. This is simpler than TOML and more readable for non-technical team members:

```rust
/// Load a markdown configuration file and inject it as context.
/// Markdown configs are simpler -- the entire file content becomes
/// a system prompt section.
pub fn load_markdown_config(path: &Path) -> Option<PromptSection> {
    let content = fs::read_to_string(path).ok()?;

    // Skip empty files
    if content.trim().is_empty() {
        return None;
    }

    let token_count = (content.len() as f64 * 0.3) as usize;

    Some(PromptSection {
        name: format!("config:{}", path.file_name()?.to_string_lossy()),
        content: format!(
            "Project-specific instructions from {}:\n\n{}",
            path.file_name()?.to_string_lossy(),
            content.trim()
        ),
        required: false,
        token_count: token_count + 10, // Account for the prefix
    })
}

fn main() {
    // Simulate loading a CLAUDE.md file
    let sample_content = r#"# Project Instructions

## Code Style
- Use `snake_case` for all function and variable names
- Every public function must have a doc comment
- Maximum line length is 100 characters

## Testing
- Run `cargo test` before committing
- Every bug fix must include a regression test

## Architecture
- Keep the `src/tools/` directory for tool implementations
- The agentic loop lives in `src/agent.rs`
- Configuration types go in `src/config.rs`
"#;

    let token_count = (sample_content.len() as f64 * 0.3) as usize;
    println!("CLAUDE.md would consume ~{} tokens", token_count);
    println!("Content:\n{}", sample_content);
}
```

## Putting It All Together

Here is how config discovery feeds into the system prompt builder:

```rust
/// Load all configuration and inject it into the system prompt builder.
pub fn inject_config_context(
    builder: &mut SystemPromptBuilder,
    working_dir: &Path,
) -> usize {
    let discovery = ConfigDiscovery::new();
    let configs = discovery.discover(working_dir);
    let mut total_tokens = 0;

    for config in &configs {
        // Try to parse as TOML first
        if config.path.extension().map_or(false, |e| e == "toml") {
            if let Ok(parsed) = AgentConfig::parse(&config.content) {
                if let Some(instructions) = &parsed.context.instructions {
                    let token_count = (instructions.len() as f64 * 0.3) as usize;
                    builder.add_optional(
                        &format!("config:{:?}", config.scope),
                        instructions.clone(),
                        token_count,
                    );
                    total_tokens += token_count;
                }
            }
        } else {
            // Treat as markdown -- inject the full content
            let section_name = format!(
                "config:{}:{}",
                format!("{:?}", config.scope).to_lowercase(),
                config.path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            );
            builder.add_optional(&section_name, config.content.clone(), config.token_count);
            total_tokens += config.token_count;
        }
    }

    total_tokens
}

fn main() {
    let mut builder = SystemPromptBuilder::new();
    builder.add_required("identity",
        "You are a coding assistant.".to_string(), 8);

    let config_tokens = inject_config_context(&mut builder, Path::new("."));
    println!("Injected {} tokens of configuration context", config_tokens);

    let (prompt, total) = builder.build(500);
    println!("Final system prompt: {} tokens", total);
}
```

::: wild In the Wild
Claude Code reads `CLAUDE.md` files from the project root and all parent directories, merging them hierarchically. A `CLAUDE.md` at the repository root sets project-wide conventions, while one in a subdirectory can add directory-specific instructions. This is a powerful pattern for monorepos where different packages have different conventions. The content is injected into the system prompt as-is, which means the model sees it on every turn. OpenCode supports a similar `.opencode` configuration file that mixes structured settings with free-form instructions.
:::

## Key Takeaways

- Discover configuration files hierarchically: global settings, project root, and subdirectory overrides, each with increasing specificity
- Support both structured formats (TOML for machine-readable settings) and freeform markdown (for human-readable instructions)
- Merge configurations by scope -- more specific scopes override broader ones, except for safety settings which should union
- Inject configuration content into the system prompt as optional sections so they are automatically dropped under context pressure
- Track the token cost of configuration injection -- a large CLAUDE.md file that consumes 2,000 tokens is a per-request tax on every turn
