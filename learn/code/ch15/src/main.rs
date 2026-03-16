// Chapter 15: Production Polish — Code snapshot

use clap::Parser;
use tracing::{info, warn};

/// A CLI coding agent.
#[derive(Parser, Debug)]
#[command(name = "cli-agent", version, about = "An AI-powered CLI coding agent")]
struct Args {
    /// The initial prompt to send to the agent.
    #[arg(short, long)]
    prompt: Option<String>,

    /// The LLM provider to use.
    #[arg(short = 'P', long, default_value = "anthropic")]
    provider: String,

    /// The model to use.
    #[arg(short, long, default_value = "claude-sonnet-4-20250514")]
    model: String,

    /// Enable verbose logging.
    #[arg(short, long)]
    verbose: bool,

    // TODO: Add --max-turns flag
    // TODO: Add --config path flag
    // TODO: Add --non-interactive flag
}

/// Initialize structured logging.
fn init_logging(verbose: bool) {
    let filter = if verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();
}

/// The main agent entrypoint.
async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    info!(provider = %args.provider, model = %args.model, "Starting CLI agent");

    // TODO: Initialize the selected provider
    // TODO: Register all tools
    // TODO: Set up the permission system
    // TODO: Load project-specific config (CLAUDE.md)
    // TODO: Start the REPL or run a single prompt
    // TODO: Run the agentic loop
    // TODO: Handle graceful shutdown

    if let Some(prompt) = &args.prompt {
        info!(prompt = %prompt, "Running single prompt");
        println!("TODO: Execute prompt: {prompt}");
    } else {
        info!("Starting interactive REPL");
        println!("TODO: Start interactive mode");
    }

    warn!("Agent not yet fully implemented");
    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    init_logging(args.verbose);

    if let Err(e) = run(args).await {
        eprintln!("Fatal error: {e}");
        std::process::exit(1);
    }
}
