---
title: CLI Flags and Options
description: Designing a comprehensive CLI interface with clap, supporting subcommands, flags, environment variable fallbacks, and shell completion generation.
---

# CLI Flags and Options

> **What you'll learn:**
> - How to design a CLI interface with clap that covers all agent configuration and operation modes
> - How to implement environment variable fallbacks for flags that users set persistently
> - Techniques for generating shell completion scripts for bash, zsh, and fish

Up to this point, your agent has probably accepted a few basic arguments -- maybe a prompt string or a `--verbose` flag. A production tool needs a comprehensive CLI interface: subcommands for different modes of operation, flags that override config file settings, environment variable fallbacks for CI environments, and shell completions that make the tool discoverable. The `clap` crate is the standard choice for this in Rust, and its derive API makes building complex CLIs feel almost declarative.

## Designing the CLI Structure

Before writing code, think about how users will interact with your agent. Here are the key commands:

- `agent` -- start an interactive session (default)
- `agent "fix the failing test"` -- run with an initial prompt
- `agent --model claude-sonnet-4-20250514 "explain this code"` -- override the model
- `agent config show` -- display current configuration
- `agent config init` -- generate a default config file
- `agent completions zsh` -- generate shell completions

Add `clap` to your `Cargo.toml`:

```toml
[dependencies]
clap = { version = "4", features = ["derive", "env"] }
clap_complete = "4"
```

Now define the CLI with clap's derive API:

```rust
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "agent",
    about = "A CLI coding agent powered by LLMs",
    version,
    after_help = "Examples:\n  \
        agent                          Start interactive session\n  \
        agent \"fix the tests\"          Run with initial prompt\n  \
        agent -m claude-sonnet-4-20250514 \"explain\"  Override model\n  \
        agent config show              Show current config"
)]
pub struct Cli {
    /// Initial prompt to send to the agent (starts interactive mode if omitted)
    pub prompt: Option<String>,

    /// LLM provider to use
    #[arg(short = 'p', long, env = "AGENT_PROVIDER")]
    pub provider: Option<String>,

    /// Model to use for generation
    #[arg(short = 'm', long, env = "AGENT_MODEL")]
    pub model: Option<String>,

    /// Maximum tokens in the response
    #[arg(long, env = "AGENT_MAX_TOKENS")]
    pub max_tokens: Option<u32>,

    /// API base URL (overrides config file)
    #[arg(long, env = "AGENT_API_URL")]
    pub api_url: Option<String>,

    /// Path to config file (overrides default discovery)
    #[arg(short = 'c', long)]
    pub config: Option<PathBuf>,

    /// Working directory for the agent (defaults to current directory)
    #[arg(short = 'w', long)]
    pub workdir: Option<PathBuf>,

    /// Enable verbose output (debug-level logging)
    #[arg(short = 'v', long, default_value_t = false)]
    pub verbose: bool,

    /// Output logs in JSON format
    #[arg(long, default_value_t = false)]
    pub json_logs: bool,

    /// Disable colored output
    #[arg(long, default_value_t = false)]
    pub no_color: bool,

    /// Run non-interactively (no prompts for confirmation)
    #[arg(long, default_value_t = false)]
    pub non_interactive: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Generate shell completion scripts
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: ShellType,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Display the current merged configuration
    Show,
    /// Generate a default config file
    Init {
        /// Write global config instead of project config
        #[arg(long, default_value_t = false)]
        global: bool,
    },
    /// Show config file paths and which ones exist
    Paths,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    PowerShell,
}
```

::: python Coming from Python
If you have used `argparse` or `click` in Python, clap's derive API will feel familiar but more powerful. Python's `click` uses decorators to build CLI interfaces. Clap uses derive macros on structs, which means your CLI definition is also a typed data structure -- you get autocompletion in your IDE, compile-time validation of flag combinations, and the guarantee that every parsed argument lands in a known type. The `env` attribute on an `#[arg]` is like `click.option(envvar="AGENT_MODEL")`, but the type system ensures the environment variable value is parsed correctly.
:::

## Merging CLI Flags with Configuration

CLI flags have the highest priority in the config hierarchy. After loading the config file, apply any CLI overrides:

```rust
pub fn apply_cli_overrides(config: &mut AgentConfig, cli: &Cli) {
    if let Some(ref provider) = cli.provider {
        config.provider.name = provider.clone();
    }
    if let Some(ref model) = cli.model {
        config.provider.model = model.clone();
    }
    if let Some(ref api_url) = cli.api_url {
        config.provider.api_url = Some(api_url.clone());
    }
    if let Some(max_tokens) = cli.max_tokens {
        config.provider.max_tokens = max_tokens;
    }
    if cli.verbose {
        config.logging.level = "debug".to_string();
    }
    if cli.json_logs {
        config.logging.json = true;
    }
    if cli.no_color {
        config.ui.color = false;
    }
}
```

The complete startup flow ties everything together:

```rust
use clap::Parser;
use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Handle subcommands that do not need the full agent
    if let Some(command) = &cli.command {
        return handle_command(command, &cli).await;
    }

    // Load config from files, then apply CLI overrides
    let workdir = cli.workdir.clone().unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    });

    let mut config = load_config(&workdir)?;
    apply_cli_overrides(&mut config, &cli);

    // Initialize logging with the final config
    init_logging(cli.verbose, config.logging.json);

    tracing::info!(
        provider = %config.provider.name,
        model = %config.provider.model,
        "Agent starting"
    );

    // Start the agent
    if let Some(ref prompt) = cli.prompt {
        run_single_prompt(&config, prompt).await
    } else {
        run_interactive(&config).await
    }
}
```

## Implementing Subcommands

Each subcommand handles a specific operational task:

```rust
use std::path::PathBuf;

async fn handle_command(command: &Commands, cli: &Cli) -> anyhow::Result<()> {
    match command {
        Commands::Config { action } => handle_config_command(action, cli),
        Commands::Completions { shell } => handle_completions(shell),
    }
}

fn handle_config_command(action: &ConfigAction, cli: &Cli) -> anyhow::Result<()> {
    match action {
        ConfigAction::Show => {
            let workdir = cli.workdir.clone().unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
            });
            let config = load_config(&workdir)?;
            let toml_str = toml::to_string_pretty(&config)?;
            println!("{toml_str}");
            Ok(())
        }
        ConfigAction::Init { global } => {
            let path = if *global {
                global_config_path()
            } else {
                PathBuf::from(".agent.toml")
            };

            if path.exists() {
                anyhow::bail!("Config file already exists at {}", path.display());
            }

            // Create parent directories
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let default_config = AgentConfig::default();
            let toml_str = toml::to_string_pretty(&default_config)?;
            std::fs::write(&path, toml_str)?;
            println!("Created config file at {}", path.display());
            Ok(())
        }
        ConfigAction::Paths => {
            let global = global_config_path();
            let exists = |p: &PathBuf| if p.exists() { "exists" } else { "not found" };
            println!("Global: {} ({})", global.display(), exists(&global));

            let workdir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            if let Some(project) = find_project_config(&workdir) {
                println!("Project: {} (exists)", project.display());
            } else {
                println!("Project: .agent.toml (not found)");
            }
            Ok(())
        }
    }
}
```

## Generating Shell Completions

Shell completions are what make a CLI feel polished. Users type `agent --` and hit Tab to see all available flags. The `clap_complete` crate generates these from your clap definition:

```rust
use clap::CommandFactory;
use clap_complete::{generate, Shell};

fn handle_completions(shell: &ShellType) -> anyhow::Result<()> {
    let mut cmd = Cli::command();
    let shell = match shell {
        ShellType::Bash => Shell::Bash,
        ShellType::Zsh => Shell::Zsh,
        ShellType::Fish => Shell::Fish,
        ShellType::PowerShell => Shell::PowerShell,
    };

    generate(shell, &mut cmd, "agent", &mut std::io::stdout());
    Ok(())
}
```

Users install completions by piping the output to the right location:

```bash
# Zsh (add to ~/.zshrc)
agent completions zsh > ~/.zfunc/_agent

# Bash (add to ~/.bashrc)
agent completions bash > /etc/bash_completion.d/agent

# Fish
agent completions fish > ~/.config/fish/completions/agent.fish
```

## Validating Flag Combinations

Some flag combinations do not make sense. Clap lets you express these constraints:

```rust
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "agent")]
pub struct Cli {
    /// Initial prompt to send to the agent
    pub prompt: Option<String>,

    /// Run non-interactively (no prompts for confirmation)
    #[arg(long, default_value_t = false)]
    pub non_interactive: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

impl Cli {
    /// Validate flag combinations that clap cannot express declaratively.
    pub fn validate(&self) -> Result<(), String> {
        if self.non_interactive && self.prompt.is_none() && self.command.is_none() {
            return Err(
                "Non-interactive mode requires a prompt. \
                 Use: agent --non-interactive \"your prompt here\""
                    .to_string(),
            );
        }
        Ok(())
    }
}
```

Call `cli.validate()` right after parsing:

```rust
let cli = Cli::parse();
if let Err(msg) = cli.validate() {
    eprintln!("Error: {msg}");
    std::process::exit(1);
}
```

::: wild In the Wild
Claude Code accepts commands like `claude "fix the bug"` for non-interactive use and supports flags like `--model`, `--verbose`, and `--output-format`. OpenCode uses a similar pattern with `opencode` launching the interactive TUI and supporting various configuration flags. Both tools rely on environment variables as the primary mechanism for API keys, keeping secrets out of config files and command history.
:::

## Key Takeaways

- Use clap's derive API to define your CLI as a typed struct -- you get type-safe parsing, automatic help text generation, and IDE autocompletion all from one definition.
- Add `env` attributes to flags that users might want to set persistently (provider, model, API URL), giving them the option of environment variables without extra code.
- Implement subcommands for non-agent operations like `config show`, `config init`, and `completions` to make your tool self-documenting and self-configuring.
- Generate shell completions for bash, zsh, and fish with `clap_complete` -- this small feature dramatically improves the discoverability of your CLI's options.
- Validate flag combinations that clap cannot express declaratively (like `--non-interactive` requiring a prompt) in a separate `validate()` method called after parsing.
