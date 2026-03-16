// Chapter 15: Production Polish — Code snapshot
//
// Builds on ch14 (Extensibility) by adding the production hardening that turns
// a prototype into a shippable tool: layered configuration, structured logging,
// graceful error reporting, session management, and a startup banner.

use std::fmt;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use clap::Parser;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// Error types — classify errors by recoverability
// ---------------------------------------------------------------------------

/// User-facing agent errors with friendly messages instead of panics.
#[derive(Debug)]
enum AgentError {
    Config { message: String },
    Session { message: String },
    Io { path: PathBuf, source: std::io::Error },
    Provider { message: String },
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::Config { message } => write!(f, "Configuration error: {message}"),
            AgentError::Session { message } => write!(f, "Session error: {message}"),
            AgentError::Io { path, source } => {
                write!(f, "I/O error at {}: {source}", path.display())
            }
            AgentError::Provider { message } => write!(f, "Provider error: {message}"),
        }
    }
}

impl std::error::Error for AgentError {}

// ---------------------------------------------------------------------------
// Configuration — layered: defaults → global file → env → CLI flags
// ---------------------------------------------------------------------------

/// Application configuration loaded from config.toml files and environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct AgentConfig {
    provider: ProviderConfig,
    logging: LoggingConfig,
    ui: UiConfig,
    session: SessionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct ProviderConfig {
    /// LLM provider name: "anthropic", "openai", or "ollama"
    name: String,
    /// Model identifier
    model: String,
    /// Maximum tokens for the response
    max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct LoggingConfig {
    /// Log level: "error", "warn", "info", "debug", "trace"
    level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct UiConfig {
    /// Show a startup banner
    banner: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct SessionConfig {
    /// Directory to store session files
    directory: Option<PathBuf>,
    /// Automatically save on exit
    auto_save: bool,
}

// Sensible defaults compiled into the binary (layer 1).
impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            provider: ProviderConfig::default(),
            logging: LoggingConfig::default(),
            ui: UiConfig::default(),
            session: SessionConfig::default(),
        }
    }
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            name: "anthropic".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 4096,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self { banner: true }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            directory: None,
            auto_save: true,
        }
    }
}

/// Return the path to the global config file, following platform conventions.
fn global_config_path() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("cli-agent").join("config.toml");
    }
    if let Some(config_dir) = dirs::config_dir() {
        return config_dir.join("cli-agent").join("config.toml");
    }
    // Fallback
    PathBuf::from(".cli-agent.toml")
}

/// Search upward from `start` for a project-level `.agent.toml`.
fn find_project_config(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        let candidate = current.join(".agent.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Load and merge configuration from all layers.
fn load_config(project_dir: &Path) -> Result<AgentConfig, AgentError> {
    // Layer 1: built-in defaults
    let mut config = AgentConfig::default();

    // Layer 2: global config file
    let global_path = global_config_path();
    if global_path.exists() {
        let global = load_toml_file(&global_path)?;
        merge_into_config(&mut config, &global)?;
        debug!(path = %global_path.display(), "Loaded global config");
    }

    // Layer 3: project config file
    if let Some(project_path) = find_project_config(project_dir) {
        let project = load_toml_file(&project_path)?;
        merge_into_config(&mut config, &project)?;
        debug!(path = %project_path.display(), "Loaded project config");
    }

    // Layer 4: environment variable overrides
    if let Ok(val) = std::env::var("AGENT_PROVIDER") {
        config.provider.name = val;
    }
    if let Ok(val) = std::env::var("AGENT_MODEL") {
        config.provider.model = val;
    }
    if let Ok(val) = std::env::var("AGENT_LOG_LEVEL") {
        config.logging.level = val;
    }

    Ok(config)
}

fn load_toml_file(path: &Path) -> Result<String, AgentError> {
    std::fs::read_to_string(path).map_err(|e| AgentError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

/// Deserialize a TOML string on top of an existing config (partial override).
fn merge_into_config(config: &mut AgentConfig, toml_str: &str) -> Result<(), AgentError> {
    // Re-serialize current config to a TOML Value, merge the override, deserialize back.
    let base_str =
        toml::to_string(config).map_err(|e| AgentError::Config {
            message: format!("failed to serialize base config: {e}"),
        })?;
    let mut base: toml::Value =
        toml::from_str(&base_str).map_err(|e| AgentError::Config {
            message: format!("internal config error: {e}"),
        })?;
    let overrides: toml::Value =
        toml::from_str(toml_str).map_err(|e| AgentError::Config {
            message: format!("invalid TOML: {e}"),
        })?;

    if let (toml::Value::Table(base_t), toml::Value::Table(over_t)) =
        (&mut base, overrides)
    {
        merge_tables(base_t, over_t);
    }

    *config = base.try_into().map_err(|e: toml::de::Error| AgentError::Config {
        message: format!("config merge failed: {e}"),
    })?;
    Ok(())
}

fn merge_tables(
    base: &mut toml::map::Map<String, toml::Value>,
    overrides: toml::map::Map<String, toml::Value>,
) {
    for (key, value) in overrides {
        match (base.get_mut(&key), &value) {
            (Some(toml::Value::Table(existing)), toml::Value::Table(incoming)) => {
                merge_tables(existing, incoming.clone());
            }
            _ => {
                base.insert(key, value);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CLI — clap with --version, config override flags, and session management
// ---------------------------------------------------------------------------

/// A CLI coding agent — production-polished.
#[derive(Parser, Debug)]
#[command(
    name = "cli-agent",
    version,
    about = "An AI-powered CLI coding agent",
    after_help = "Examples:\n  \
        cli-agent                             Start interactive session\n  \
        cli-agent -p \"fix the failing test\"   Run with initial prompt\n  \
        cli-agent --provider openai           Override provider"
)]
struct Args {
    /// The initial prompt to send to the agent.
    #[arg(short, long)]
    prompt: Option<String>,

    /// The LLM provider to use.
    #[arg(short = 'P', long, env = "AGENT_PROVIDER")]
    provider: Option<String>,

    /// The model to use.
    #[arg(short, long, env = "AGENT_MODEL")]
    model: Option<String>,

    /// Enable verbose logging (debug level).
    #[arg(short, long)]
    verbose: bool,

    /// Maximum number of agentic turns before stopping.
    #[arg(long, default_value_t = 20)]
    max_turns: u32,

    /// Path to a config file (overrides default discovery).
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Run non-interactively (no confirmation prompts).
    #[arg(long)]
    non_interactive: bool,

    /// Resume a previous session from a JSON file.
    #[arg(long)]
    resume: Option<PathBuf>,

    /// Path to save the session when finished.
    #[arg(long)]
    save_session: Option<PathBuf>,
}

impl Args {
    /// Validate flag combinations that clap cannot express declaratively.
    fn validate(&self) -> Result<(), String> {
        if self.non_interactive && self.prompt.is_none() {
            return Err(
                "Non-interactive mode requires a prompt. \
                 Use: cli-agent --non-interactive -p \"your prompt\""
                    .to_string(),
            );
        }
        Ok(())
    }
}

/// Apply CLI flags on top of the loaded config (layer 5 — highest priority).
fn apply_cli_overrides(config: &mut AgentConfig, args: &Args) {
    if let Some(ref provider) = args.provider {
        config.provider.name = provider.clone();
    }
    if let Some(ref model) = args.model {
        config.provider.model = model.clone();
    }
    if args.verbose {
        config.logging.level = "debug".to_string();
    }
    if let Some(ref path) = args.save_session {
        config.session.directory = path.parent().map(|p| p.to_path_buf());
    }
}

// ---------------------------------------------------------------------------
// Structured logging — tracing crate
// ---------------------------------------------------------------------------

/// Initialize the tracing subscriber with an env-filter.
fn init_logging(level: &str) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();
}

// ---------------------------------------------------------------------------
// Session management — save / load conversation to JSON
// ---------------------------------------------------------------------------

/// A single message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
    #[serde(with = "chrono::serde::ts_seconds")]
    timestamp: DateTime<Utc>,
}

/// Persisted session state.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Session {
    /// Unique session identifier.
    id: String,
    /// When the session was created.
    #[serde(with = "chrono::serde::ts_seconds")]
    created_at: DateTime<Utc>,
    /// Provider used for this session.
    provider: String,
    /// Model used for this session.
    model: String,
    /// Conversation history.
    messages: Vec<Message>,
}

impl Session {
    /// Create a fresh session.
    fn new(provider: &str, model: &str) -> Self {
        let id = format!("{}", Utc::now().format("%Y%m%d-%H%M%S"));
        Self {
            id,
            created_at: Utc::now(),
            provider: provider.to_string(),
            model: model.to_string(),
            messages: Vec::new(),
        }
    }

    /// Append a message to the conversation.
    fn add_message(&mut self, role: &str, content: &str) {
        self.messages.push(Message {
            role: role.to_string(),
            content: content.to_string(),
            timestamp: Utc::now(),
        });
    }

    /// Save session to a JSON file.
    fn save(&self, path: &Path) -> Result<(), AgentError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AgentError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| AgentError::Session {
            message: format!("failed to serialize session: {e}"),
        })?;
        std::fs::write(path, json).map_err(|e| AgentError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        info!(path = %path.display(), "Session saved");
        Ok(())
    }

    /// Load a session from a JSON file.
    fn load(path: &Path) -> Result<Self, AgentError> {
        let data = std::fs::read_to_string(path).map_err(|e| AgentError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let session: Session =
            serde_json::from_str(&data).map_err(|e| AgentError::Session {
                message: format!("failed to parse session file: {e}"),
            })?;
        info!(
            path = %path.display(),
            messages = session.messages.len(),
            "Session loaded"
        );
        Ok(session)
    }

    /// Determine the default save path inside the sessions directory.
    fn default_path(sessions_dir: &Path) -> PathBuf {
        let name = format!("session-{}.json", Utc::now().format("%Y%m%d-%H%M%S"));
        sessions_dir.join(name)
    }
}

// ---------------------------------------------------------------------------
// Startup banner
// ---------------------------------------------------------------------------

fn print_banner(config: &AgentConfig) {
    let version = env!("CARGO_PKG_VERSION");
    println!("┌─────────────────────────────────────────┐");
    println!("│  cli-agent v{:<27}│", version);
    println!("│  Provider: {:<29}│", config.provider.name);
    println!("│  Model:    {:<29}│", config.provider.model);
    println!("└─────────────────────────────────────────┘");
    println!();
}

// ---------------------------------------------------------------------------
// Agent loop (simplified representative implementation)
// ---------------------------------------------------------------------------

async fn run(args: Args, config: AgentConfig) -> Result<(), AgentError> {
    // Optionally resume a prior session, or start fresh.
    let mut session = if let Some(ref resume_path) = args.resume {
        Session::load(resume_path)?
    } else {
        Session::new(&config.provider.name, &config.provider.model)
    };

    info!(
        provider = %config.provider.name,
        model = %config.provider.model,
        session_id = %session.id,
        "Agent started"
    );

    if let Some(ref prompt) = args.prompt {
        // Single-prompt (non-interactive) mode.
        info!(prompt = %prompt, "Running single prompt");
        session.add_message("user", prompt);

        // In a real implementation this would call the LLM via reqwest.
        let reply = format!(
            "[agent would call {} / {} with: {}]",
            config.provider.name, config.provider.model, prompt
        );
        session.add_message("assistant", &reply);
        println!("{reply}");
    } else {
        // Interactive REPL mode.
        info!("Starting interactive REPL");
        println!("Type a message (or \"exit\" to quit):\n");

        let stdin = std::io::stdin();
        let mut turn = 0u32;
        loop {
            if turn >= args.max_turns {
                warn!(max_turns = args.max_turns, "Max turns reached, stopping");
                println!("Reached maximum of {} turns.", args.max_turns);
                break;
            }

            // Read user input.
            let mut input = String::new();
            print!("> ");
            // Flush stdout so the prompt appears before blocking on read.
            use std::io::Write;
            std::io::stdout().flush().ok();
            match stdin.read_line(&mut input) {
                Ok(0) => break, // EOF
                Ok(_) => {}
                Err(e) => {
                    warn!(error = %e, "Failed to read input");
                    break;
                }
            }
            let input = input.trim();
            if input.is_empty() {
                continue;
            }
            if input == "exit" || input == "quit" {
                break;
            }

            session.add_message("user", input);

            // Placeholder for the real LLM call (ch05-ch08 cover this).
            let reply = format!(
                "[turn {}: agent would respond via {} / {}]",
                turn + 1,
                config.provider.name,
                config.provider.model,
            );
            session.add_message("assistant", &reply);
            println!("{reply}\n");

            turn += 1;
        }
    }

    // Save session on exit.
    let save_path = if let Some(ref explicit) = args.save_session {
        explicit.clone()
    } else if config.session.auto_save {
        let sessions_dir = config
            .session
            .directory
            .clone()
            .unwrap_or_else(|| {
                dirs::data_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("cli-agent")
                    .join("sessions")
            });
        Session::default_path(&sessions_dir)
    } else {
        // Auto-save disabled and no explicit path — skip.
        info!("Session not saved (auto_save disabled)");
        return Ok(());
    };

    session.save(&save_path)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Entrypoint — ties together config, logging, validation, and the agent loop
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Validate flag combinations early, before any heavy work.
    if let Err(msg) = args.validate() {
        eprintln!("Error: {msg}");
        std::process::exit(1);
    }

    // Load layered configuration.
    let project_dir =
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut config = match load_config(&project_dir) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("Hint: run with default settings or create a config file.");
            // Fall back to defaults so the agent is still usable.
            AgentConfig::default()
        }
    };
    apply_cli_overrides(&mut config, &args);

    // Initialize structured logging.
    init_logging(&config.logging.level);

    debug!(config = ?config, "Final merged configuration");

    // Show startup banner.
    if config.ui.banner {
        print_banner(&config);
    }

    // Run the agent and handle errors gracefully.
    if let Err(e) = run(args, config).await {
        // User-friendly error reporting — no panics, no raw backtraces.
        eprintln!("\n{e}");
        tracing::error!(error = %e, "Agent exited with error");
        std::process::exit(1);
    }
}
