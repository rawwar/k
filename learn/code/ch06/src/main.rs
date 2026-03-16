// Chapter 6: Shell Execution — Code snapshot

use std::process::Command;

/// Execute a shell command and capture its output.
fn run_command(command: &str) -> Result<String, String> {
    // TODO: Add timeout support
    // TODO: Add working directory configuration
    // TODO: Capture both stdout and stderr

    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .map_err(|e| format!("Failed to execute command: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(stdout)
    } else {
        Err(format!("Command failed.\nstdout: {stdout}\nstderr: {stderr}"))
    }
}

fn main() {
    println!("Chapter 6: Shell Execution");

    // TODO: Register BashTool as a tool in the agent
    // TODO: Implement safety checks before execution

    match run_command("echo 'Hello from shell!'") {
        Ok(output) => println!("Output: {output}"),
        Err(e) => eprintln!("Error: {e}"),
    }
}
