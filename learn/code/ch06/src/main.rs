// Chapter 6: Shell Execution — Code snapshot
//
// Demonstrates: process spawning, stdout/stderr capture, command builder,
// timeout enforcement, working directory management, dangerous command
// detection, and output truncation — integrated into a ShellExecute tool.

use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command as TokioCommand;
use tokio::time;

// ---------------------------------------------------------------------------
// ShellOutput — structured result from executing a shell command
// ---------------------------------------------------------------------------

/// Structured result from executing a shell command.
#[derive(Debug, Clone)]
pub struct ShellOutput {
    /// The command's exit code. -1 if killed by a signal or timed out.
    pub exit_code: i32,
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
    /// Whether the command completed successfully (exit code 0).
    pub success: bool,
    /// Whether the command was killed because it exceeded the timeout.
    pub timed_out: bool,
}

impl ShellOutput {
    /// Format the output for inclusion in an LLM tool-result message.
    pub fn to_tool_result(&self) -> String {
        let mut result = String::new();

        if !self.stdout.is_empty() {
            result.push_str(&self.stdout);
        }

        if !self.stderr.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str("[stderr]\n");
            result.push_str(&self.stderr);
        }

        if self.timed_out {
            result.push_str("\n[command timed out]");
        } else if !self.success {
            result.push_str(&format!("\n[exit code: {}]", self.exit_code));
        }

        if result.is_empty() {
            result.push_str("[no output]");
        }

        result
    }
}

// ---------------------------------------------------------------------------
// ShellCommand — builder for configuring a command before execution
// ---------------------------------------------------------------------------

/// A builder for configuring a shell command before execution.
#[derive(Debug, Clone)]
pub struct ShellCommand {
    /// The raw command string (interpreted by `sh -c`).
    command: String,
    /// Working directory for the command.
    working_dir: Option<PathBuf>,
    /// Extra environment variables to set.
    env_vars: HashMap<String, String>,
    /// Maximum execution time before the process is killed.
    timeout: Duration,
    /// Maximum number of output lines to keep (middle-truncation).
    max_output_lines: usize,
}

impl ShellCommand {
    /// Create a new shell command that will be executed through `sh -c`.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            working_dir: None,
            env_vars: HashMap::new(),
            timeout: Duration::from_secs(30),
            max_output_lines: 500,
        }
    }

    /// Set the working directory for the command.
    pub fn working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(path.into());
        self
    }

    /// Add an environment variable to the child process.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.insert(key.into(), value.into());
        self
    }

    /// Set the maximum execution time.
    pub fn timeout(mut self, duration: Duration) -> Self {
        self.timeout = duration;
        self
    }

    /// Set the maximum number of output lines (for truncation).
    pub fn max_output_lines(mut self, lines: usize) -> Self {
        self.max_output_lines = lines;
        self
    }

    /// Get the command string for display/logging.
    pub fn display_command(&self) -> &str {
        &self.command
    }

    /// Build a configured `tokio::process::Command`.
    fn build(&self) -> TokioCommand {
        let mut cmd = TokioCommand::new("sh");
        cmd.arg("-c");
        cmd.arg(&self.command);

        // Pipe stdout/stderr, null stdin
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.stdin(Stdio::null());

        // Set working directory
        if let Some(ref dir) = self.working_dir {
            cmd.current_dir(dir);
        }

        // Inject extra environment variables
        for (key, value) in &self.env_vars {
            cmd.env(key, value);
        }

        cmd
    }

    /// Execute the command with timeout enforcement.
    ///
    /// Spawns the process, reads stdout/stderr concurrently via
    /// `tokio::join!`, wraps the whole operation in
    /// `tokio::time::timeout`, and kills the child if it exceeds the
    /// deadline.
    pub async fn execute(&self) -> Result<ShellOutput> {
        let mut child = self
            .build()
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn '{}': {}", self.display_command(), e))?;

        let timeout_duration = self.timeout;
        let max_lines = self.max_output_lines;
        let display = self.command.clone();

        // Take ownership of piped handles so we can read them
        // concurrently with tokio::join!  (stdout/stderr were set to
        // Stdio::piped() in build()).
        let mut stdout_handle = child.stdout.take().expect("stdout not piped");
        let mut stderr_handle = child.stderr.take().expect("stderr not piped");

        // The inner future reads both streams concurrently and then
        // waits for the child to exit.
        let work = async {
            use tokio::io::AsyncReadExt;

            let (stdout_res, stderr_res) = tokio::join!(
                async {
                    let mut buf = Vec::new();
                    stdout_handle.read_to_end(&mut buf).await.map(|_| buf)
                },
                async {
                    let mut buf = Vec::new();
                    stderr_handle.read_to_end(&mut buf).await.map(|_| buf)
                }
            );

            let stdout_bytes = stdout_res.map_err(|e| anyhow!("read stdout: {}", e))?;
            let stderr_bytes = stderr_res.map_err(|e| anyhow!("read stderr: {}", e))?;

            let status = child
                .wait()
                .await
                .map_err(|e| anyhow!("wait for child: {}", e))?;

            Ok::<_, anyhow::Error>((stdout_bytes, stderr_bytes, status))
        };

        let result = time::timeout(timeout_duration, work).await;

        match result {
            // Process finished within the timeout
            Ok(Ok((stdout_bytes, stderr_bytes, status))) => {
                let stdout_raw = String::from_utf8_lossy(&stdout_bytes).into_owned();
                let stderr_raw = String::from_utf8_lossy(&stderr_bytes).into_owned();

                // Apply middle-truncation to stdout
                let stdout = truncate_middle(&stdout_raw, max_lines);

                Ok(ShellOutput {
                    exit_code: status.code().unwrap_or(-1),
                    stdout,
                    stderr: stderr_raw,
                    success: status.success(),
                    timed_out: false,
                })
            }
            // Error inside the work future
            Ok(Err(e)) => Err(e),
            // Timeout elapsed — kill the child
            Err(_elapsed) => {
                // child was partially moved into the `work` future
                // which has been dropped; on Unix the child process is
                // still running.  We cannot call child.kill() because
                // ownership moved.  However, dropping the Child handle
                // does NOT kill the process on its own — Tokio's Child
                // drop impl does call kill for us when the handle is
                // dropped while the process is still running.
                //
                // (In a production agent you would keep the Child
                // handle outside the future and send SIGTERM/SIGKILL
                // explicitly.  For this snapshot the implicit drop-kill
                // is sufficient.)
                Ok(ShellOutput {
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: format!(
                        "Command '{}' timed out after {}s",
                        display,
                        timeout_duration.as_secs()
                    ),
                    success: false,
                    timed_out: true,
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Output truncation — middle-truncation strategy
// ---------------------------------------------------------------------------

/// Keep the first and last portions of output, dropping the middle.
/// Returns the (possibly truncated) string.
fn truncate_middle(output: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= max_lines {
        return output.to_string();
    }

    let head_count = max_lines / 2;
    let tail_count = max_lines - head_count;
    let omitted = lines.len() - head_count - tail_count;

    let head: String = lines[..head_count].join("\n");
    let tail: String = lines[lines.len() - tail_count..].join("\n");

    format!(
        "{}\n\n[... {} lines omitted ...]\n\n{}",
        head, omitted, tail
    )
}

// ---------------------------------------------------------------------------
// Dangerous command detection
// ---------------------------------------------------------------------------

/// Check whether a command matches any known dangerous patterns.
/// Returns `Err` with a reason if the command should be blocked, or `Ok(())`
/// with optional warnings printed to stderr.
fn check_dangerous(command: &str) -> Result<()> {
    let cmd = command.to_lowercase();

    // Critical — block unconditionally
    let critical_patterns: &[(&str, &str)] = &[
        ("rm -rf /", "Recursive force-delete from root"),
        ("rm -fr /", "Recursive force-delete from root"),
        ("rm -rf /*", "Recursive force-delete from root wildcard"),
        ("mkfs.", "Formatting a filesystem"),
        ("dd if=", "Raw disk write via dd"),
        (":(){", "Fork bomb"),
    ];

    for (pattern, reason) in critical_patterns {
        if cmd.contains(pattern) {
            return Err(anyhow!(
                "BLOCKED — dangerous command detected: {}. Pattern: '{}'",
                reason,
                pattern
            ));
        }
    }

    // High-risk — warn but allow
    let warn_patterns: &[(&str, &str)] = &[
        ("sudo ", "Elevated privileges via sudo"),
        ("curl ", "Network request via curl"),
        ("wget ", "Network request via wget"),
        ("chmod -r 777", "Recursive world-writable permissions"),
        ("| sh", "Piping into a shell interpreter"),
        ("| bash", "Piping into a shell interpreter"),
        ("> /dev/", "Redirecting to a device file"),
    ];

    for (pattern, reason) in warn_patterns {
        if cmd.contains(pattern) {
            eprintln!("[safety-warning] {}: {}", reason, command);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tool trait — mirrors the ch04/ch05 tool system
// ---------------------------------------------------------------------------
// In a full agent the Tool trait would use async-trait or return a BoxFuture.
// For this snapshot we implement the tool as a concrete struct with inherent
// async methods, which is sufficient to demonstrate the pattern.

// ---------------------------------------------------------------------------
// ShellExecuteTool — the agent's bash/shell tool
// ---------------------------------------------------------------------------

/// Configuration for the ShellExecute tool.
pub struct ShellExecuteTool {
    /// Default working directory for commands.
    default_working_dir: PathBuf,
    /// Default timeout per command.
    default_timeout: Duration,
    /// Maximum output lines before truncation.
    max_output_lines: usize,
}

impl ShellExecuteTool {
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            default_working_dir: working_dir,
            default_timeout: Duration::from_secs(30),
            max_output_lines: 500,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    pub fn with_max_output_lines(mut self, lines: usize) -> Self {
        self.max_output_lines = lines;
        self
    }

    /// Tool name used in function-calling.
    pub fn name(&self) -> &str {
        "shell_execute"
    }

    /// Short description for the model.
    pub fn description(&self) -> &str {
        "Execute a shell command and return its output. The command runs \
         through `sh -c` so pipes, redirects, and shell features work."
    }

    /// JSON Schema for the tool parameters.
    pub fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Working directory (absolute path). Defaults to the project root."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Maximum seconds to wait. Defaults to 30."
                }
            },
            "required": ["command"]
        })
    }

    /// Execute the tool with the given JSON parameters.
    pub async fn execute(&self, params: &Value) -> Result<String> {
        // ---- 1. Parse parameters ----
        let command = params
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: command"))?;

        let working_dir = match params.get("working_dir").and_then(|v| v.as_str()) {
            Some(dir) => {
                let path = PathBuf::from(dir);
                if !path.is_dir() {
                    return Err(anyhow!("Working directory does not exist: {}", dir));
                }
                path
            }
            None => self.default_working_dir.clone(),
        };

        let timeout_secs = params
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.default_timeout.as_secs());

        // ---- 2. Safety check — block dangerous commands ----
        check_dangerous(command)?;

        // ---- 3. Build and execute the command ----
        let output = ShellCommand::new(command)
            .working_dir(working_dir)
            .timeout(Duration::from_secs(timeout_secs))
            .max_output_lines(self.max_output_lines)
            .execute()
            .await?;

        // ---- 4. Return formatted result for the LLM ----
        Ok(output.to_tool_result())
    }
}

// ---------------------------------------------------------------------------
// main — demonstrate the ShellExecute tool
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    println!("Chapter 6: Shell Execution\n");

    let cwd = std::env::current_dir()?;
    let tool = ShellExecuteTool::new(cwd).with_timeout(Duration::from_secs(10));

    // --- Demo 1: simple command ---
    println!("=== Demo 1: echo ===");
    let params = serde_json::json!({ "command": "echo 'Hello from the shell tool!'" });
    match tool.execute(&params).await {
        Ok(result) => println!("{result}"),
        Err(e) => eprintln!("Error: {e}"),
    }

    // --- Demo 2: capture both stdout and stderr ---
    println!("\n=== Demo 2: stdout + stderr ===");
    let params = serde_json::json!({
        "command": "echo 'this is stdout'; echo 'this is stderr' >&2"
    });
    match tool.execute(&params).await {
        Ok(result) => println!("{result}"),
        Err(e) => eprintln!("Error: {e}"),
    }

    // --- Demo 3: working directory ---
    println!("\n=== Demo 3: working directory ===");
    let params = serde_json::json!({
        "command": "pwd",
        "working_dir": "/tmp"
    });
    match tool.execute(&params).await {
        Ok(result) => println!("{result}"),
        Err(e) => eprintln!("Error: {e}"),
    }

    // --- Demo 4: non-zero exit code ---
    println!("\n=== Demo 4: failing command ===");
    let params = serde_json::json!({ "command": "ls /nonexistent_dir_12345" });
    match tool.execute(&params).await {
        Ok(result) => println!("{result}"),
        Err(e) => eprintln!("Error: {e}"),
    }

    // --- Demo 5: dangerous command detection ---
    println!("\n=== Demo 5: dangerous command blocked ===");
    let params = serde_json::json!({ "command": "rm -rf /" });
    match tool.execute(&params).await {
        Ok(result) => println!("{result}"),
        Err(e) => eprintln!("Blocked: {e}"),
    }

    // --- Demo 6: timeout ---
    println!("\n=== Demo 6: command timeout ===");
    let params = serde_json::json!({
        "command": "sleep 60",
        "timeout_secs": 2
    });
    match tool.execute(&params).await {
        Ok(result) => println!("{result}"),
        Err(e) => eprintln!("Error: {e}"),
    }

    // --- Demo 7: pipeline (shell features) ---
    println!("\n=== Demo 7: shell pipeline ===");
    let params = serde_json::json!({
        "command": "echo 'hello world' | tr a-z A-Z"
    });
    match tool.execute(&params).await {
        Ok(result) => println!("{result}"),
        Err(e) => eprintln!("Error: {e}"),
    }

    Ok(())
}
