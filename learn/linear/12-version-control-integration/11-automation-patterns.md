---
title: Automation Patterns
description: Git automation workflows for agents — auto-commit on tool success, PR creation, CI integration, and hook-based validation of agent-generated changes.
---

# Automation Patterns

> **What you'll learn:**
> - How to implement auto-commit workflows that create well-structured commits after each successful agent tool execution
> - Automating pull request creation with descriptive titles, summaries of changes, and links to relevant context
> - Integrating with CI/CD pipelines: triggering checks on agent branches, interpreting results, and iterating on failures

With all the Git primitives covered in the previous subchapters -- status, diff, commit, branching, safety -- you can now compose them into higher-level automation workflows. These patterns are where Git integration transforms from a collection of commands into a coherent agent capability. The automation layer decides when to commit, what to put in the commit message, when to create a PR, and how to respond to CI feedback.

## Auto-Commit on Tool Success

The most common automation pattern is committing after each successful tool execution. This creates a clean, granular history where each commit corresponds to one agent action:

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct AutoCommitter {
    repo_dir: PathBuf,
    agent_name: String,
    agent_email: String,
}

impl AutoCommitter {
    pub fn new(
        repo_dir: impl Into<PathBuf>,
        agent_name: impl Into<String>,
        agent_email: impl Into<String>,
    ) -> Self {
        Self {
            repo_dir: repo_dir.into(),
            agent_name: agent_name.into(),
            agent_email: agent_email.into(),
        }
    }

    /// Commit changes from a tool execution with context
    pub fn commit_tool_result(
        &self,
        tool_name: &str,
        files_modified: &[&str],
        description: &str,
    ) -> Result<Option<String>, String> {
        // Check if there are any changes to commit
        if !self.has_changes()? {
            return Ok(None);
        }

        // Stage only the files the tool modified
        for file in files_modified {
            self.run_git(&["add", file])?;
        }

        // Verify something was actually staged
        if !self.has_staged_changes()? {
            return Ok(None);
        }

        // Build the commit message
        let message = format!(
            "{}: {}\n\nTool: {}\nFiles: {}\n\nCo-Authored-By: {} <{}>",
            tool_name,
            description,
            tool_name,
            files_modified.join(", "),
            self.agent_name,
            self.agent_email,
        );

        let output = self.run_git(&["commit", "-m", &message])?;

        // Extract the commit hash
        let hash = self.run_git(&["rev-parse", "--short", "HEAD"])?;
        Ok(Some(hash.trim().to_string()))
    }

    /// Squash all agent commits on the current branch into a single commit
    pub fn squash_agent_commits(&self, base_branch: &str) -> Result<String, String> {
        // Count commits since diverging from base
        let count_output = self.run_git(&[
            "rev-list", "--count",
            &format!("{}..HEAD", base_branch),
        ])?;
        let count: usize = count_output.trim().parse().unwrap_or(0);

        if count <= 1 {
            return Ok("Nothing to squash (0 or 1 commits)".to_string());
        }

        // Soft reset to the merge base, keeping all changes staged
        let merge_base = self.run_git(&["merge-base", base_branch, "HEAD"])?;
        self.run_git(&["reset", "--soft", merge_base.trim()])?;

        // Create a single combined commit
        let message = format!(
            "Agent changes: {} commits squashed\n\nCo-Authored-By: {} <{}>",
            count, self.agent_name, self.agent_email,
        );
        self.run_git(&["commit", "-m", &message])?;

        Ok(format!("Squashed {} commits into one", count))
    }

    fn has_changes(&self) -> Result<bool, String> {
        let output = self.run_git(&["status", "--porcelain"])?;
        Ok(!output.trim().is_empty())
    }

    fn has_staged_changes(&self) -> Result<bool, String> {
        let output = Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .current_dir(&self.repo_dir)
            .output()
            .map_err(|e| format!("Failed to check staged: {}", e))?;
        Ok(!output.status.success())
    }

    fn run_git(&self, args: &[&str]) -> Result<String, String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_dir)
            .output()
            .map_err(|e| format!("Failed to run git: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    }
}
```

::: python Coming from Python
Python automation scripts often use `subprocess.run(["git", "commit", ...])` in a similar fashion. The key difference in Rust is the explicit error handling on every call. In Python, you might let a failed `git commit` raise a `CalledProcessError` and catch it at a high level. In Rust, each `Result` must be handled, which forces you to think about what should happen when a commit fails -- which is exactly the kind of careful handling an agent needs.
:::

## Automating Pull Request Creation

After the agent finishes its work on a branch, the natural next step is creating a pull request. You can use the GitHub CLI (`gh`) or call the GitHub API directly:

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct PullRequestAutomation {
    repo_dir: PathBuf,
}

#[derive(Debug)]
pub struct PullRequestInfo {
    pub url: String,
    pub number: u64,
    pub title: String,
}

impl PullRequestAutomation {
    pub fn new(repo_dir: impl Into<PathBuf>) -> Self {
        Self { repo_dir: repo_dir.into() }
    }

    /// Push the current branch and create a pull request
    pub fn create_pr(
        &self,
        title: &str,
        body: &str,
        base_branch: &str,
    ) -> Result<PullRequestInfo, String> {
        // Get the current branch name
        let branch = self.run_git(&["branch", "--show-current"])?;
        let branch = branch.trim();

        // Push the branch to the remote
        self.run_git(&["push", "-u", "origin", branch])?;

        // Create the PR using the GitHub CLI
        let output = Command::new("gh")
            .args([
                "pr", "create",
                "--title", title,
                "--body", body,
                "--base", base_branch,
                "--head", branch,
            ])
            .current_dir(&self.repo_dir)
            .output()
            .map_err(|e| format!("Failed to run gh: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("PR creation failed: {}", stderr));
        }

        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Extract PR number from URL
        let number = url.rsplit('/')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        Ok(PullRequestInfo {
            url,
            number,
            title: title.to_string(),
        })
    }

    /// Generate a PR description from the diff between base and HEAD
    pub fn generate_pr_body(
        &self,
        base_branch: &str,
    ) -> Result<String, String> {
        // Get the commit log since diverging from base
        let log = self.run_git(&[
            "log", "--oneline",
            &format!("{}..HEAD", base_branch),
        ])?;

        // Get the diff stats
        let stats = self.run_git(&[
            "diff", "--stat",
            &format!("{}..HEAD", base_branch),
        ])?;

        // Get the list of changed files
        let files = self.run_git(&[
            "diff", "--name-only",
            &format!("{}..HEAD", base_branch),
        ])?;

        let mut body = String::new();
        body.push_str("## Summary\n\n");
        body.push_str("This PR contains changes made by the coding agent.\n\n");

        body.push_str("## Commits\n\n");
        body.push_str("```\n");
        body.push_str(&log);
        body.push_str("```\n\n");

        body.push_str("## Files Changed\n\n");
        for file in files.lines() {
            let file = file.trim();
            if !file.is_empty() {
                body.push_str(&format!("- `{}`\n", file));
            }
        }

        body.push_str("\n## Stats\n\n");
        body.push_str("```\n");
        body.push_str(&stats);
        body.push_str("```\n");

        Ok(body)
    }

    fn run_git(&self, args: &[&str]) -> Result<String, String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_dir)
            .output()
            .map_err(|e| format!("Failed to run git: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    }
}
```

## CI Integration: Checking Agent Work

After pushing a branch or creating a PR, the agent should monitor CI results and react to failures:

```rust
use std::process::Command;
use std::path::Path;
use std::thread;
use std::time::Duration;

#[derive(Debug, PartialEq)]
pub enum CiStatus {
    Pending,
    Success,
    Failure(Vec<String>), // List of failed check names
    Unknown,
}

pub fn check_ci_status(repo_dir: &Path, branch: &str) -> Result<CiStatus, String> {
    // Use the GitHub CLI to check PR status
    let output = Command::new("gh")
        .args([
            "pr", "checks", "--json",
            "name,status,conclusion",
            "--jq", ".[] | [.name, .status, .conclusion] | @tsv",
        ])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to check CI status: {}", e))?;

    if !output.status.success() {
        return Ok(CiStatus::Unknown);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut all_passed = true;
    let mut any_pending = false;
    let mut failures = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 3 {
            let name = parts[0];
            let status = parts[1];
            let conclusion = parts[2];

            match status {
                "IN_PROGRESS" | "QUEUED" | "PENDING" => {
                    any_pending = true;
                }
                "COMPLETED" => {
                    if conclusion != "SUCCESS" && conclusion != "SKIPPED" {
                        all_passed = false;
                        failures.push(name.to_string());
                    }
                }
                _ => {}
            }
        }
    }

    if any_pending {
        Ok(CiStatus::Pending)
    } else if all_passed {
        Ok(CiStatus::Success)
    } else {
        Ok(CiStatus::Failure(failures))
    }
}

/// Wait for CI to complete and return the final status
pub fn wait_for_ci(
    repo_dir: &Path,
    branch: &str,
    timeout_secs: u64,
    poll_interval_secs: u64,
) -> Result<CiStatus, String> {
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    let interval = Duration::from_secs(poll_interval_secs);

    loop {
        let status = check_ci_status(repo_dir, branch)?;

        match status {
            CiStatus::Pending => {
                if start.elapsed() > timeout {
                    return Err("CI check timed out".to_string());
                }
                thread::sleep(interval);
            }
            other => return Ok(other),
        }
    }
}
```

::: wild In the Wild
Claude Code integrates with the GitHub workflow by creating commits and PRs that follow the repository's conventions. When Claude Code creates a PR, it generates a descriptive title and body that summarize the changes. The agent uses the diff and commit history to build a meaningful PR description rather than a generic one. This automation pattern -- pushing a branch, creating a PR, and monitoring CI -- is the complete end-to-end workflow that turns agent modifications into reviewable, mergeable changes.
:::

## Git Hooks for Agent Validation

Git hooks can validate agent-generated changes before they are committed. Your agent should respect existing hooks and can install its own:

```rust
use std::path::{Path, PathBuf};
use std::fs;
use std::os::unix::fs::PermissionsExt;

pub struct HookManager {
    hooks_dir: PathBuf,
}

impl HookManager {
    pub fn new(repo_dir: &Path) -> Self {
        Self {
            hooks_dir: repo_dir.join(".git/hooks"),
        }
    }

    /// Check if a specific hook exists and is executable
    pub fn has_hook(&self, hook_name: &str) -> bool {
        let hook_path = self.hooks_dir.join(hook_name);
        hook_path.exists() && hook_path.is_file()
    }

    /// Install a pre-commit hook that validates agent changes
    pub fn install_agent_pre_commit(&self) -> Result<(), String> {
        let hook_path = self.hooks_dir.join("pre-commit");

        // Don't overwrite existing hooks
        if hook_path.exists() {
            return Err("pre-commit hook already exists. Modify it manually to add agent validation.".to_string());
        }

        let hook_content = r#"#!/bin/sh
# Agent validation pre-commit hook
# Checks that agent-generated files meet basic quality standards

# Check for debug prints left in code
if git diff --cached --name-only | xargs grep -l 'dbg!' 2>/dev/null; then
    echo "Error: dbg! macro found in staged files. Remove before committing."
    exit 1
fi

# Check for TODO markers that should have been resolved
if git diff --cached --name-only | xargs grep -l 'TODO: AGENT' 2>/dev/null; then
    echo "Error: Unresolved AGENT TODO markers found."
    exit 1
fi

# Check that Rust files compile
if git diff --cached --name-only | grep -q '\.rs$'; then
    if command -v cargo > /dev/null 2>&1; then
        cargo check --quiet 2>/dev/null
        if [ $? -ne 0 ]; then
            echo "Error: Rust compilation failed. Fix before committing."
            exit 1
        fi
    fi
fi

exit 0
"#;

        fs::write(&hook_path, hook_content)
            .map_err(|e| format!("Failed to write hook: {}", e))?;

        // Make the hook executable
        let mut perms = fs::metadata(&hook_path)
            .map_err(|e| format!("Failed to read permissions: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&hook_path, perms)
            .map_err(|e| format!("Failed to set permissions: {}", e))?;

        Ok(())
    }

    /// List all installed hooks
    pub fn list_hooks(&self) -> Result<Vec<String>, String> {
        let entries = fs::read_dir(&self.hooks_dir)
            .map_err(|e| format!("Failed to read hooks dir: {}", e))?;

        let mut hooks = Vec::new();
        for entry in entries {
            if let Ok(entry) = entry {
                let name = entry.file_name().to_string_lossy().to_string();
                // Skip sample hooks
                if !name.ends_with(".sample") {
                    hooks.push(name);
                }
            }
        }

        Ok(hooks)
    }
}
```

## Complete Automation Workflow

Here is how all the automation pieces compose into a full workflow:

```rust
use std::path::Path;

pub fn automated_agent_workflow(
    repo_dir: &Path,
    task_description: &str,
) -> Result<String, String> {
    let auto_committer = AutoCommitter::new(
        repo_dir, "Agent", "agent@example.com"
    );
    let pr_automation = PullRequestAutomation::new(repo_dir);

    // 1. Create a feature branch
    let branch_name = format!("agent/{}", task_description
        .to_lowercase().replace(' ', "-"));
    Command::new("git")
        .args(["checkout", "-b", &branch_name])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("Failed to create branch: {}", e))?;

    // 2. Agent does work, auto-committing after each tool execution
    // (This is called by the agent loop after each tool)
    // auto_committer.commit_tool_result("edit", &["src/main.rs"], "Fix error handling")?;

    // 3. Optionally squash commits for a cleaner history
    // auto_committer.squash_agent_commits("main")?;

    // 4. Generate PR description and create the PR
    let body = pr_automation.generate_pr_body("main")?;
    let pr = pr_automation.create_pr(
        &format!("Agent: {}", task_description),
        &body,
        "main",
    )?;

    // 5. Monitor CI and report results
    let status = wait_for_ci(repo_dir, &branch_name, 600, 30)?;
    match status {
        CiStatus::Success => {
            Ok(format!("PR created and CI passed: {}", pr.url))
        }
        CiStatus::Failure(checks) => {
            Ok(format!("PR created but CI failed ({}): {}",
                checks.join(", "), pr.url))
        }
        _ => {
            Ok(format!("PR created, CI status unknown: {}", pr.url))
        }
    }
}
```

## Key Takeaways

- Auto-commit after each tool execution creates granular, traceable history -- each commit corresponds to one agent action, making it easy to review and selectively undo.
- Offer commit squashing as an option for users who prefer a clean, single-commit history before creating a PR.
- Automate PR creation by generating descriptive titles and bodies from the commit log and diff stats -- this saves the user manual effort and produces consistent, informative PR descriptions.
- Monitor CI status after pushing agent branches, and surface failures clearly so the agent or user can iterate on fixes.
- Respect existing Git hooks and consider installing agent-specific validation hooks that catch common issues (debug statements, compilation errors, unresolved markers) before they reach the commit history.
