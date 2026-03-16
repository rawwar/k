// Chapter 11: Git Integration — Code snapshot

use std::process::Command;

/// Run a git command and return its output.
fn git(args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run git: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

/// Check the current git status.
fn git_status() -> Result<String, String> {
    // TODO: Parse and format git status output
    git(&["status", "--short"])
}

/// Get the diff of staged or unstaged changes.
fn git_diff(staged: bool) -> Result<String, String> {
    // TODO: Support diffing specific files
    if staged {
        git(&["diff", "--cached"])
    } else {
        git(&["diff"])
    }
}

fn main() {
    println!("Chapter 11: Git Integration");

    // TODO: Implement git-aware context (current branch, recent commits)
    // TODO: Add safe commit creation with proper messages
    // TODO: Add PR creation via gh CLI

    match git_status() {
        Ok(status) => println!("Git status:\n{status}"),
        Err(e) => eprintln!("Git error: {e}"),
    }

    let _ = git_diff(false);
    println!("TODO: Full git integration");
}
