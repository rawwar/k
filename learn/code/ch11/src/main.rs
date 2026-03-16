// Chapter 11: Git Integration — Code snapshot
//
// Builds on ch10 (Search and Code Intelligence) by adding version control
// awareness. The agent can now inspect repository state, create commits,
// and manage branches — all through safe, non-destructive git operations
// driven by shelling out to the git CLI via std::process::Command.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// Tool trait (from ch4) — the interface every tool implements
// ---------------------------------------------------------------------------

trait Tool {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    fn execute(&self, input: &Value) -> Result<String, String>;
}

// ---------------------------------------------------------------------------
// Git helper — every git operation goes through this wrapper
// ---------------------------------------------------------------------------

/// Run a git command in a given repo directory and return trimmed stdout.
fn run_git(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(classify_git_error(&stderr))
    }
}

/// Check whether a path is inside a git repository.
fn is_git_repo(path: &Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Turn raw git stderr into an LLM-friendly error message.
fn classify_git_error(stderr: &str) -> String {
    if stderr.contains("not a git repository") {
        "Error: Not inside a git repository. Git operations are not available in this directory."
            .to_string()
    } else if stderr.contains("index.lock") {
        "Error: The git index is locked by another process. Wait a moment and retry.".to_string()
    } else if stderr.contains("Please commit your changes or stash them") {
        "Error: Working tree has uncommitted changes that would be overwritten. \
         Commit or stash current changes first."
            .to_string()
    } else if stderr.contains("pathspec") && stderr.contains("did not match") {
        format!(
            "Error: A branch or path was not found. Use git status to check available refs. \
             Detail: {}",
            stderr
        )
    } else if stderr.contains("CONFLICT") || stderr.contains("merge conflict") {
        "Error: Merge conflict detected. Resolve conflicts before proceeding.".to_string()
    } else {
        format!("Git error: {}", stderr)
    }
}

// ---------------------------------------------------------------------------
// Safety layer — block destructive commands, gate risky ones
// ---------------------------------------------------------------------------

/// Commands the agent must never run.
const BLOCKED_PATTERNS: &[&str] = &[
    "push --force",
    "push -f",
    "reset --hard",
    "clean -f",
    "clean -fd",
    "rebase",
    "filter-branch",
    "reflog expire",
];

/// Commands that require explicit user approval.
const GATED_PATTERNS: &[&str] = &[
    "push",
    "merge",
    "branch -D",
    "branch -d",
    "stash drop",
    "tag -d",
];

#[derive(Debug)]
enum CommandSafety {
    Safe,
    NeedsApproval(String),
    Blocked(String),
}

fn check_safety(args: &[&str]) -> CommandSafety {
    let joined = args.join(" ");

    // Check "checkout ." which discards all working tree changes.
    if args.first() == Some(&"checkout")
        && (args.contains(&".") || (args.contains(&"--") && args.last() == Some(&".")))
    {
        return CommandSafety::Blocked(
            "'git checkout .' discards all working tree changes. Use stash instead.".to_string(),
        );
    }

    for pattern in BLOCKED_PATTERNS {
        if joined.contains(pattern) {
            return CommandSafety::Blocked(format!(
                "'git {}' is destructive and blocked by the safety layer.",
                joined
            ));
        }
    }

    for pattern in GATED_PATTERNS {
        if joined.starts_with(pattern) {
            return CommandSafety::NeedsApproval(format!(
                "'git {}' modifies shared state. User approval required.",
                joined
            ));
        }
    }

    CommandSafety::Safe
}

/// Execute a git command only if it passes the safety check.
fn safe_git(repo_path: &Path, args: &[&str], user_approved: bool) -> Result<String, String> {
    match check_safety(args) {
        CommandSafety::Blocked(reason) => Err(reason),
        CommandSafety::NeedsApproval(reason) => {
            if user_approved {
                run_git(repo_path, args)
            } else {
                Err(format!("Approval required: {}", reason))
            }
        }
        CommandSafety::Safe => run_git(repo_path, args),
    }
}

// ---------------------------------------------------------------------------
// Pre-flight checks — verify repo state before mutating operations
// ---------------------------------------------------------------------------

struct PreFlightResult {
    passed: bool,
    checks: Vec<(String, bool, String)>,
}

impl PreFlightResult {
    fn summary(&self) -> String {
        self.checks
            .iter()
            .map(|(name, passed, msg)| {
                let icon = if *passed { "OK" } else { "FAIL" };
                format!("[{}] {}: {}", icon, name, msg)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn pre_flight_checks(repo_path: &Path) -> PreFlightResult {
    let mut checks: Vec<(String, bool, String)> = Vec::new();

    // 1. Inside a git repository?
    let in_repo = is_git_repo(repo_path);
    checks.push((
        "Git repository".into(),
        in_repo,
        if in_repo {
            "Inside a git repository".into()
        } else {
            "Not inside a git repository".into()
        },
    ));
    if !in_repo {
        return PreFlightResult {
            passed: false,
            checks,
        };
    }

    // 2. On a branch (not detached HEAD)?
    let branch = run_git(repo_path, &["symbolic-ref", "--short", "HEAD"]);
    checks.push((
        "Branch status".into(),
        branch.is_ok(),
        match &branch {
            Ok(name) => format!("On branch '{}'", name),
            Err(_) => "HEAD is detached -- commits may be lost".into(),
        },
    ));

    // 3. No merge conflicts?
    let conflict_files = run_git(repo_path, &["diff", "--name-only", "--diff-filter=U"])
        .unwrap_or_default();
    let has_conflicts = !conflict_files.is_empty();
    checks.push((
        "No merge conflicts".into(),
        !has_conflicts,
        if has_conflicts {
            "Unresolved merge conflicts detected".into()
        } else {
            "No merge conflicts".into()
        },
    ));

    // 4. Index not locked?
    let git_dir = run_git(repo_path, &["rev-parse", "--git-dir"]).unwrap_or_default();
    let index_locked = Path::new(&git_dir).join("index.lock").exists();
    checks.push((
        "Index not locked".into(),
        !index_locked,
        if index_locked {
            "index.lock exists -- another git process may be running".into()
        } else {
            "Git index is available".into()
        },
    ));

    let passed = checks.iter().all(|(_, ok, _)| *ok);
    PreFlightResult { passed, checks }
}

// ---------------------------------------------------------------------------
// Structured status types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed { from: String },
    Untracked,
    Unmerged,
}

#[derive(Debug, Clone)]
struct StatusEntry {
    path: PathBuf,
    staged: Option<FileStatus>,
    unstaged: Option<FileStatus>,
}

struct RepoStatus {
    branch: Option<String>,
    entries: Vec<StatusEntry>,
}

impl RepoStatus {
    fn is_clean(&self) -> bool {
        self.entries.is_empty()
    }
}

fn parse_status_char(c: char) -> Option<FileStatus> {
    match c {
        'M' => Some(FileStatus::Modified),
        'A' => Some(FileStatus::Added),
        'D' => Some(FileStatus::Deleted),
        '.' => None,
        _ => None,
    }
}

/// Parse `git status --porcelain=v2 --branch` into structured data.
fn parse_porcelain_status(repo_path: &Path) -> Result<RepoStatus, String> {
    let raw = run_git(repo_path, &["status", "--porcelain=v2", "--branch"])?;
    let mut branch = None;
    let mut entries = Vec::new();

    for line in raw.lines() {
        if line.starts_with("# branch.head ") {
            branch = Some(line["# branch.head ".len()..].to_string());
            continue;
        }

        if line.starts_with("1 ") {
            // Ordinary entry: 1 XY sub mH mI mW hH hI path
            let parts: Vec<&str> = line.splitn(9, ' ').collect();
            if parts.len() >= 9 {
                let xy: Vec<char> = parts[1].chars().collect();
                if xy.len() == 2 {
                    entries.push(StatusEntry {
                        path: PathBuf::from(parts[8]),
                        staged: parse_status_char(xy[0]),
                        unstaged: parse_status_char(xy[1]),
                    });
                }
            }
        } else if line.starts_with("2 ") {
            // Renamed entry: 2 XY sub mH mI mW hH hI X-score path\torigPath
            let parts: Vec<&str> = line.splitn(10, ' ').collect();
            if parts.len() >= 10 {
                let path_parts: Vec<&str> = parts[9].splitn(2, '\t').collect();
                entries.push(StatusEntry {
                    path: PathBuf::from(path_parts[0]),
                    staged: Some(FileStatus::Renamed {
                        from: path_parts.get(1).unwrap_or(&"").to_string(),
                    }),
                    unstaged: None,
                });
            }
        } else if line.starts_with("u ") {
            // Unmerged
            let parts: Vec<&str> = line.splitn(11, ' ').collect();
            if parts.len() >= 11 {
                entries.push(StatusEntry {
                    path: PathBuf::from(parts[10]),
                    staged: Some(FileStatus::Unmerged),
                    unstaged: Some(FileStatus::Unmerged),
                });
            }
        } else if line.starts_with("? ") {
            // Untracked
            entries.push(StatusEntry {
                path: PathBuf::from(&line[2..]),
                staged: None,
                unstaged: Some(FileStatus::Untracked),
            });
        }
    }

    Ok(RepoStatus { branch, entries })
}

/// Format RepoStatus into concise text the LLM can reason about.
fn format_status_for_llm(status: &RepoStatus) -> String {
    let mut out = String::new();

    if let Some(ref branch) = status.branch {
        out.push_str(&format!("On branch: {}\n", branch));
    }

    if status.is_clean() {
        out.push_str("Working tree is clean -- no pending changes.\n");
        return out;
    }

    let staged: Vec<_> = status.entries.iter().filter(|e| e.staged.is_some()).collect();
    let unstaged: Vec<_> = status
        .entries
        .iter()
        .filter(|e| {
            matches!(
                &e.unstaged,
                Some(s) if !matches!(s, FileStatus::Untracked)
            )
        })
        .collect();
    let untracked: Vec<_> = status
        .entries
        .iter()
        .filter(|e| matches!(e.unstaged, Some(FileStatus::Untracked)))
        .collect();

    if !staged.is_empty() {
        out.push_str(&format!("Staged for commit ({} files):\n", staged.len()));
        for entry in &staged {
            let label = match &entry.staged {
                Some(FileStatus::Modified) => "modified",
                Some(FileStatus::Added) => "new file",
                Some(FileStatus::Deleted) => "deleted",
                Some(FileStatus::Renamed { from }) => {
                    out.push_str(&format!(
                        "  renamed: {} -> {}\n",
                        from,
                        entry.path.display()
                    ));
                    continue;
                }
                _ => "changed",
            };
            out.push_str(&format!("  {}: {}\n", label, entry.path.display()));
        }
    }

    if !unstaged.is_empty() {
        out.push_str(&format!(
            "Unstaged changes ({} files):\n",
            unstaged.len()
        ));
        for entry in &unstaged {
            out.push_str(&format!("  modified: {}\n", entry.path.display()));
        }
    }

    if !untracked.is_empty() {
        out.push_str(&format!("Untracked files ({}):\n", untracked.len()));
        for entry in &untracked {
            out.push_str(&format!("  {}\n", entry.path.display()));
        }
    }

    out
}

// ---------------------------------------------------------------------------
// GitStatus tool
// ---------------------------------------------------------------------------

struct GitStatusTool {
    repo_path: PathBuf,
}

impl Tool for GitStatusTool {
    fn name(&self) -> &str {
        "git_status"
    }

    fn description(&self) -> &str {
        "Show the current git repository status: branch, staged changes, \
         unstaged modifications, and untracked files."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    fn execute(&self, _input: &Value) -> Result<String, String> {
        if !is_git_repo(&self.repo_path) {
            return Err(
                "Not inside a git repository. Git tools are unavailable.".to_string()
            );
        }

        let status = parse_porcelain_status(&self.repo_path)?;
        Ok(format_status_for_llm(&status))
    }
}

// ---------------------------------------------------------------------------
// GitDiff tool
// ---------------------------------------------------------------------------

struct GitDiffTool {
    repo_path: PathBuf,
    max_lines: usize,
}

impl GitDiffTool {
    /// Truncate large diffs so they fit within the LLM context window.
    fn truncate(&self, text: &str) -> String {
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() <= self.max_lines {
            text.to_string()
        } else {
            let truncated: String = lines[..self.max_lines].join("\n");
            format!(
                "{}\n\n[Output truncated: showing {} of {} lines]",
                truncated,
                self.max_lines,
                lines.len()
            )
        }
    }
}

impl Tool for GitDiffTool {
    fn name(&self) -> &str {
        "git_diff"
    }

    fn description(&self) -> &str {
        "Show the diff of changes in the repository. Set 'staged' to true \
         to see changes staged for commit, or false (default) for unstaged changes."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "staged": {
                    "type": "boolean",
                    "description": "If true, show staged (cached) changes. Default: false."
                },
                "paths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Limit the diff to these file paths."
                }
            },
            "additionalProperties": false
        })
    }

    fn execute(&self, input: &Value) -> Result<String, String> {
        if !is_git_repo(&self.repo_path) {
            return Err(
                "Not inside a git repository. Git tools are unavailable.".to_string()
            );
        }

        let staged = input
            .get("staged")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut args: Vec<&str> = vec!["diff", "-U3"];
        if staged {
            args.push("--cached");
        }

        // If specific paths are requested, add them after "--".
        let path_strings: Vec<String> = input
            .get("paths")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|p| p.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        if !path_strings.is_empty() {
            args.push("--");
            // Build the full command manually so owned Strings stay alive.
            let mut cmd = Command::new("git");
            cmd.args(&args).current_dir(&self.repo_path);
            for p in &path_strings {
                cmd.arg(p);
            }
            let output = cmd
                .output()
                .map_err(|e| format!("Failed to run git diff: {}", e))?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.trim().is_empty() {
                return Ok("No differences found.".to_string());
            }
            return Ok(self.truncate(&stdout));
        }

        // Also get a --numstat summary to show file-level stats.
        let mut stat_args: Vec<&str> = vec!["diff", "--numstat"];
        if staged {
            stat_args.push("--cached");
        }
        let numstat = run_git(&self.repo_path, &stat_args).unwrap_or_default();
        let diff_output = run_git(&self.repo_path, &args)?;

        if diff_output.is_empty() {
            return Ok("No differences found.".to_string());
        }

        let mut result = String::new();

        // Prepend a compact summary.
        if !numstat.is_empty() {
            let mut total_ins: usize = 0;
            let mut total_del: usize = 0;
            let mut file_count: usize = 0;
            for line in numstat.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() == 3 {
                    total_ins += parts[0].parse::<usize>().unwrap_or(0);
                    total_del += parts[1].parse::<usize>().unwrap_or(0);
                    file_count += 1;
                }
            }
            result.push_str(&format!(
                "Summary: {} file(s) changed, +{} insertions, -{} deletions\n\n",
                file_count, total_ins, total_del
            ));
        }

        result.push_str(&self.truncate(&diff_output));
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// GitCommit tool
// ---------------------------------------------------------------------------

struct GitCommitTool {
    repo_path: PathBuf,
}

impl Tool for GitCommitTool {
    fn name(&self) -> &str {
        "git_commit"
    }

    fn description(&self) -> &str {
        "Stage specific files and create a git commit. Provide the files to stage \
         and a commit message. Files are staged selectively (never 'git add -A'). \
         A Co-Authored-By trailer is added automatically."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "required": ["message", "files"],
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The commit message."
                },
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "File paths to stage before committing."
                }
            },
            "additionalProperties": false
        })
    }

    fn execute(&self, input: &Value) -> Result<String, String> {
        if !is_git_repo(&self.repo_path) {
            return Err(
                "Not inside a git repository. Git tools are unavailable.".to_string()
            );
        }

        // Run pre-flight checks before any mutating operation.
        let preflight = pre_flight_checks(&self.repo_path);
        if !preflight.passed {
            return Err(format!(
                "Pre-flight checks failed — cannot commit:\n{}",
                preflight.summary()
            ));
        }

        let message = input
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or("commit requires a 'message' field")?;

        let files = input
            .get("files")
            .and_then(|v| v.as_array())
            .ok_or("commit requires a 'files' array")?;

        if files.is_empty() {
            return Err(
                "No files specified. Provide at least one file path to stage.".to_string()
            );
        }

        // Stage each file individually.
        let mut staged: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();
        for file_val in files {
            if let Some(path) = file_val.as_str() {
                match run_git(&self.repo_path, &["add", "--", path]) {
                    Ok(_) => staged.push(path.to_string()),
                    Err(e) => errors.push(format!("{}: {}", path, e)),
                }
            }
        }

        if staged.is_empty() {
            return Err(format!(
                "Failed to stage any files:\n{}",
                errors.join("\n")
            ));
        }

        // Verify that something is actually staged.
        let cached = run_git(&self.repo_path, &["diff", "--cached", "--name-only"])?;
        if cached.is_empty() {
            return Err(
                "Nothing staged for commit after adding files. \
                 The files may already match HEAD."
                    .to_string(),
            );
        }

        // Build full commit message with co-authorship trailer.
        let full_message = format!(
            "{}\n\nCo-Authored-By: AI Agent <agent@example.com>",
            message
        );

        run_git(&self.repo_path, &["commit", "-m", &full_message])?;

        // Report result.
        let hash = run_git(&self.repo_path, &["rev-parse", "--short", "HEAD"])?;
        let file_count = cached.lines().count();

        let mut result = format!(
            "Created commit {} ({} file(s)).\nMessage: {}",
            hash, file_count, message
        );

        if !errors.is_empty() {
            result.push_str(&format!(
                "\nWarning — could not stage some files:\n{}",
                errors.join("\n")
            ));
        }

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// GitLog tool — recent commit history for context
// ---------------------------------------------------------------------------

struct GitLogTool {
    repo_path: PathBuf,
}

impl Tool for GitLogTool {
    fn name(&self) -> &str {
        "git_log"
    }

    fn description(&self) -> &str {
        "Show recent git commit history. Useful for understanding what has changed \
         and following the project's commit message conventions."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "count": {
                    "type": "integer",
                    "description": "Number of commits to show (default: 10)."
                },
                "path": {
                    "type": "string",
                    "description": "Limit log to commits affecting this file."
                }
            },
            "additionalProperties": false
        })
    }

    fn execute(&self, input: &Value) -> Result<String, String> {
        if !is_git_repo(&self.repo_path) {
            return Err(
                "Not inside a git repository. Git tools are unavailable.".to_string()
            );
        }

        let count = input
            .get("count")
            .and_then(|v| v.as_u64())
            .unwrap_or(10);
        let count_str = format!("-{}", count);

        let mut args = vec!["log", &count_str, "--format=%h %ci %s", "--no-merges"];

        let path_str: String;
        if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
            path_str = path.to_string();
            args.push("--");
            args.push(&path_str);
        }

        let output = run_git(&self.repo_path, &args)?;
        if output.is_empty() {
            Ok("No commits found.".to_string())
        } else {
            Ok(output)
        }
    }
}

// ---------------------------------------------------------------------------
// Tool registry (from ch4, simplified)
// ---------------------------------------------------------------------------

struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    fn new() -> Self {
        Self { tools: Vec::new() }
    }

    fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools
            .iter()
            .find(|t| t.name() == name)
            .map(|t| t.as_ref())
    }

    /// Build the tool definitions array the Claude API expects.
    fn api_definitions(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name(),
                    "description": t.description(),
                    "input_schema": t.input_schema(),
                })
            })
            .collect()
    }

    fn list_names(&self) -> Vec<&str> {
        self.tools.iter().map(|t| t.name()).collect()
    }
}

// ---------------------------------------------------------------------------
// Main — demonstrate the git tools
// ---------------------------------------------------------------------------

fn main() {
    println!("Chapter 11: Git Integration\n");

    let repo_path = PathBuf::from(".");

    // ------------------------------------------------------------------
    // Gate all git tools on being inside a repository (ch11 lesson 1).
    // ------------------------------------------------------------------
    if !is_git_repo(&repo_path) {
        println!("Not inside a git repository — git tools will be unavailable.");
        println!("Run this binary from within a git repo to see the tools in action.");
        return;
    }

    // ------------------------------------------------------------------
    // Register git tools (builds on the ch4 Tool trait and ch10 pattern).
    // ------------------------------------------------------------------
    let mut registry = ToolRegistry::new();

    registry.register(Box::new(GitStatusTool {
        repo_path: repo_path.clone(),
    }));
    registry.register(Box::new(GitDiffTool {
        repo_path: repo_path.clone(),
        max_lines: 500,
    }));
    registry.register(Box::new(GitCommitTool {
        repo_path: repo_path.clone(),
    }));
    registry.register(Box::new(GitLogTool {
        repo_path: repo_path.clone(),
    }));

    println!(
        "Registered tools: {}\n",
        registry.list_names().join(", ")
    );

    // ------------------------------------------------------------------
    // Show API tool definitions (what gets sent to Claude).
    // ------------------------------------------------------------------
    let defs = registry.api_definitions();
    println!(
        "Tool definitions for the API:\n{}\n",
        serde_json::to_string_pretty(&defs).unwrap()
    );

    // ------------------------------------------------------------------
    // Pre-flight checks — always run before mutating operations.
    // ------------------------------------------------------------------
    let preflight = pre_flight_checks(&repo_path);
    println!(
        "Pre-flight checks {}:\n{}\n",
        if preflight.passed { "PASSED" } else { "FAILED" },
        preflight.summary()
    );

    // ------------------------------------------------------------------
    // Demonstrate: git_status tool
    // ------------------------------------------------------------------
    println!("--- git_status ---");
    if let Some(tool) = registry.get("git_status") {
        match tool.execute(&serde_json::json!({})) {
            Ok(output) => println!("{}", output),
            Err(e) => eprintln!("{}", e),
        }
    }

    // ------------------------------------------------------------------
    // Demonstrate: git_diff tool (unstaged changes)
    // ------------------------------------------------------------------
    println!("\n--- git_diff (unstaged) ---");
    if let Some(tool) = registry.get("git_diff") {
        match tool.execute(&serde_json::json!({})) {
            Ok(output) => println!("{}", output),
            Err(e) => eprintln!("{}", e),
        }
    }

    // ------------------------------------------------------------------
    // Demonstrate: git_diff tool (staged changes)
    // ------------------------------------------------------------------
    println!("\n--- git_diff (staged) ---");
    if let Some(tool) = registry.get("git_diff") {
        match tool.execute(&serde_json::json!({ "staged": true })) {
            Ok(output) => println!("{}", output),
            Err(e) => eprintln!("{}", e),
        }
    }

    // ------------------------------------------------------------------
    // Demonstrate: git_log tool
    // ------------------------------------------------------------------
    println!("\n--- git_log ---");
    if let Some(tool) = registry.get("git_log") {
        match tool.execute(&serde_json::json!({ "count": 5 })) {
            Ok(output) => println!("{}", output),
            Err(e) => eprintln!("{}", e),
        }
    }

    // ------------------------------------------------------------------
    // Demonstrate: safety layer
    // ------------------------------------------------------------------
    println!("\n--- Safety layer demos ---");

    match safe_git(&repo_path, &["status", "--short"], false) {
        Ok(output) => println!("[SAFE] git status: {}", if output.is_empty() { "(clean)" } else { &output }),
        Err(e) => eprintln!("[SAFE] error: {}", e),
    }

    match safe_git(&repo_path, &["reset", "--hard", "HEAD~1"], false) {
        Ok(_) => println!("This should not happen"),
        Err(e) => println!("[BLOCKED] {}", e),
    }

    match safe_git(&repo_path, &["push", "origin", "main"], false) {
        Ok(_) => println!("This should not happen"),
        Err(e) => println!("[GATED] {}", e),
    }

    // ------------------------------------------------------------------
    // Note: git_commit is intentionally not demonstrated here because it
    // mutates the repository. In a real agent loop the LLM would invoke it
    // like this:
    //
    //   tool.execute(&serde_json::json!({
    //       "message": "feat: add git integration tools",
    //       "files": ["src/main.rs", "Cargo.toml"]
    //   }));
    //
    // The tool stages files selectively, runs pre-flight checks, and adds
    // a Co-Authored-By trailer before creating the commit.
    // ------------------------------------------------------------------

    println!("\nDone. All git tools operational.");
}
