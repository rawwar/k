// Chapter 12: Permission and Safety — Code snapshot
//
// Builds on ch11 (git helpers) and adds a layered safety system:
//   - Permission levels (ReadOnly, Standard, FullAuto)
//   - Operation classification (Read, Write, SafeExec, UnsafeExec, Destructive)
//   - Path-based allow/deny rules
//   - Command-based allow/deny rules with dangerous-pattern detection
//   - An interactive approval flow (y/n/a/q)
//   - Integrated permission checks before tool execution

use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// 1. Git helper (carried forward from ch11)
// ---------------------------------------------------------------------------

/// Run a git command and return its stdout on success.
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

fn git_status() -> Result<String, String> {
    git(&["status", "--short"])
}

// ---------------------------------------------------------------------------
// 2. Permission levels
// ---------------------------------------------------------------------------

/// Three-tier permission level.  Ordering is `ReadOnly < Standard < FullAuto`,
/// so `current_level >= required_level` is all you need for a simple gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PermissionLevel {
    /// Can only observe — file reads, searches, git status.
    ReadOnly,
    /// Can read and write; destructive actions still need approval.
    Standard,
    /// Everything is allowed without prompts.
    FullAuto,
}

impl std::fmt::Display for PermissionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadOnly => write!(f, "read-only"),
            Self::Standard => write!(f, "standard"),
            Self::FullAuto => write!(f, "full-auto"),
        }
    }
}

// ---------------------------------------------------------------------------
// 3. Operation classification
// ---------------------------------------------------------------------------

/// Risk tier for a single tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationClass {
    /// Pure reads — never modifies state.
    Read,
    /// Creates or modifies files inside the project.
    Write,
    /// Shell command classified as safe (e.g. `cargo check`, `ls`).
    SafeExec,
    /// Shell command with potential side effects.
    UnsafeExec,
    /// Irreversible operation (force-push, rm -rf, etc.).
    Destructive,
}

impl OperationClass {
    /// Minimum permission level that can even attempt this class.
    pub fn required_permission(&self) -> PermissionLevel {
        match self {
            Self::Read => PermissionLevel::ReadOnly,
            Self::Write | Self::SafeExec => PermissionLevel::Standard,
            Self::UnsafeExec => PermissionLevel::Standard,
            Self::Destructive => PermissionLevel::FullAuto,
        }
    }

    /// Whether the user must confirm interactively, even when the level is
    /// high enough.
    pub fn requires_approval(&self, current: PermissionLevel) -> bool {
        match self {
            Self::Read | Self::SafeExec => false,
            Self::Write => current == PermissionLevel::Standard,
            Self::UnsafeExec | Self::Destructive => current != PermissionLevel::FullAuto,
        }
    }
}

// ---------------------------------------------------------------------------
// 4. Permission registry — maps tool names to OperationClass
// ---------------------------------------------------------------------------

pub struct PermissionRegistry {
    rules: HashMap<String, OperationClass>,
    default_class: OperationClass,
}

impl PermissionRegistry {
    pub fn new() -> Self {
        let mut rules = HashMap::new();

        // File tools
        rules.insert("read_file".into(), OperationClass::Read);
        rules.insert("list_directory".into(), OperationClass::Read);
        rules.insert("search_files".into(), OperationClass::Read);
        rules.insert("grep".into(), OperationClass::Read);
        rules.insert("glob".into(), OperationClass::Read);
        rules.insert("write_file".into(), OperationClass::Write);
        rules.insert("edit_file".into(), OperationClass::Write);

        // Git — read operations
        rules.insert("git:status".into(), OperationClass::Read);
        rules.insert("git:diff".into(), OperationClass::Read);
        rules.insert("git:log".into(), OperationClass::Read);

        // Git — write operations
        rules.insert("git:add".into(), OperationClass::Write);
        rules.insert("git:commit".into(), OperationClass::Write);

        // Git — dangerous operations
        rules.insert("git:push".into(), OperationClass::UnsafeExec);
        rules.insert("git:push --force".into(), OperationClass::Destructive);
        rules.insert("git:reset --hard".into(), OperationClass::Destructive);

        // Shell
        rules.insert("shell:safe".into(), OperationClass::SafeExec);
        rules.insert("shell:unsafe".into(), OperationClass::UnsafeExec);

        Self {
            rules,
            default_class: OperationClass::UnsafeExec,
        }
    }

    /// Look up the class for `tool` (optionally with `:subcommand`).
    pub fn classify(&self, tool: &str, subcommand: Option<&str>) -> &OperationClass {
        if let Some(sub) = subcommand {
            let key = format!("{tool}:{sub}");
            if let Some(cls) = self.rules.get(&key) {
                return cls;
            }
        }
        self.rules.get(tool).unwrap_or(&self.default_class)
    }
}

// ---------------------------------------------------------------------------
// 5. Permission gate — three-way decision
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecision {
    Allowed,
    NeedsApproval { reason: String },
    Denied { reason: String },
}

pub struct PermissionGate {
    level: PermissionLevel,
    registry: PermissionRegistry,
}

impl PermissionGate {
    pub fn new(level: PermissionLevel) -> Self {
        Self {
            level,
            registry: PermissionRegistry::new(),
        }
    }

    pub fn check(&self, tool: &str, subcommand: Option<&str>) -> PermissionDecision {
        let cls = self.registry.classify(tool, subcommand);
        let required = cls.required_permission();

        if self.level < required {
            return PermissionDecision::Denied {
                reason: format!(
                    "Requires {required} permission, current level is {}",
                    self.level
                ),
            };
        }

        if cls.requires_approval(self.level) {
            let label = subcommand
                .map(|s| format!("{tool}:{s}"))
                .unwrap_or_else(|| tool.to_string());
            return PermissionDecision::NeedsApproval {
                reason: format!("'{label}' classified as {cls:?} — requires approval"),
            };
        }

        PermissionDecision::Allowed
    }

    pub fn set_level(&mut self, level: PermissionLevel) {
        self.level = level;
    }

    pub fn level(&self) -> PermissionLevel {
        self.level
    }
}

// ---------------------------------------------------------------------------
// 6. Command filter — allowlist + denylist for shell commands
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterVerdict {
    Allowed,
    Blocked(String),
}

struct DenyPattern {
    pattern: String,
    reason: String,
}

pub struct CommandFilter {
    allowed_executables: HashSet<String>,
    denied_patterns: Vec<DenyPattern>,
}

impl CommandFilter {
    /// Sensible defaults for a coding-agent context.
    pub fn with_defaults() -> Self {
        let allowed: HashSet<String> = [
            "cargo", "rustc", "npm", "node", "python", "python3", "pip", "pip3",
            "make", "ls", "cat", "head", "tail", "wc", "grep", "rg", "find",
            "sort", "uniq", "diff", "echo", "pwd", "which", "tree", "git",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let denied = vec![
            DenyPattern {
                pattern: "rm -rf /".into(),
                reason: "Recursive delete from root".into(),
            },
            DenyPattern {
                pattern: "rm -rf ~".into(),
                reason: "Recursive delete from home".into(),
            },
            DenyPattern {
                pattern: "rm -rf .".into(),
                reason: "Recursive delete of current directory".into(),
            },
            DenyPattern {
                pattern: "> /dev/sda".into(),
                reason: "Direct write to block device".into(),
            },
            DenyPattern {
                pattern: "mkfs.".into(),
                reason: "Filesystem format command".into(),
            },
            DenyPattern {
                pattern: "dd if=".into(),
                reason: "Raw disk operation".into(),
            },
            DenyPattern {
                pattern: ":(){ :|:& };:".into(),
                reason: "Fork bomb".into(),
            },
            DenyPattern {
                pattern: "chmod 777".into(),
                reason: "Overly permissive file permissions".into(),
            },
            DenyPattern {
                pattern: "chmod -R 777".into(),
                reason: "Recursive overly permissive permissions".into(),
            },
        ];

        Self {
            allowed_executables: allowed,
            denied_patterns: denied,
        }
    }

    pub fn check_command(&self, command: &str) -> FilterVerdict {
        let trimmed = command.trim();
        let executable = trimmed.split_whitespace().next().unwrap_or("");

        // Allowlist check
        if !self.allowed_executables.is_empty()
            && !self.allowed_executables.contains(executable)
        {
            return FilterVerdict::Blocked(format!(
                "Executable '{executable}' is not on the allowlist"
            ));
        }

        // Denylist check
        for dp in &self.denied_patterns {
            if trimmed.contains(&dp.pattern) {
                return FilterVerdict::Blocked(format!(
                    "Matches blocked pattern '{}': {}",
                    dp.pattern, dp.reason
                ));
            }
        }

        FilterVerdict::Allowed
    }
}

// ---------------------------------------------------------------------------
// 7. Path filter — workspace boundary + sensitive-file denylist
// ---------------------------------------------------------------------------

struct PathPattern {
    pattern: String,
    reason: String,
}

pub struct PathFilter {
    denied_paths: Vec<PathPattern>,
    allowed_roots: Vec<PathBuf>,
}

impl PathFilter {
    pub fn with_defaults(project_root: &Path) -> Self {
        let denied = vec![
            PathPattern { pattern: ".env".into(), reason: "May contain secrets".into() },
            PathPattern { pattern: ".env.local".into(), reason: "May contain secrets".into() },
            PathPattern { pattern: "id_rsa".into(), reason: "SSH private key".into() },
            PathPattern { pattern: "id_ed25519".into(), reason: "SSH private key".into() },
            PathPattern { pattern: ".aws/credentials".into(), reason: "AWS credentials".into() },
            PathPattern { pattern: "credentials.json".into(), reason: "Credentials file".into() },
            PathPattern { pattern: ".npmrc".into(), reason: "npm auth tokens".into() },
        ];

        Self {
            denied_paths: denied,
            allowed_roots: vec![project_root.to_path_buf()],
        }
    }

    pub fn check_path(&self, path: &Path) -> FilterVerdict {
        let path_str = path.to_string_lossy();

        // Check denied patterns
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        for pp in &self.denied_paths {
            if file_name == pp.pattern || path_str.ends_with(&pp.pattern) {
                return FilterVerdict::Blocked(format!(
                    "Path '{}' is denied: {}",
                    pp.pattern, pp.reason
                ));
            }
        }

        // Workspace boundary
        if !self.allowed_roots.is_empty() {
            let inside = self.allowed_roots.iter().any(|root| path.starts_with(root));
            if !inside {
                return FilterVerdict::Blocked(format!(
                    "Path {} is outside allowed project directories",
                    path.display()
                ));
            }
        }

        FilterVerdict::Allowed
    }
}

// ---------------------------------------------------------------------------
// 8. PermissionManager — unified facade that owns all safety layers
// ---------------------------------------------------------------------------

pub struct PermissionManager {
    pub gate: PermissionGate,
    pub command_filter: CommandFilter,
    pub path_filter: PathFilter,
    always_approved_tools: HashSet<String>,
}

impl PermissionManager {
    pub fn new(level: PermissionLevel, project_root: &Path) -> Self {
        Self {
            gate: PermissionGate::new(level),
            command_filter: CommandFilter::with_defaults(),
            path_filter: PathFilter::with_defaults(project_root),
            always_approved_tools: HashSet::new(),
        }
    }

    // ---- high-level checks ------------------------------------------------

    /// Check a tool call (not a shell command).  Returns the three-way
    /// permission decision taking "always-approved" into account.
    pub fn check_tool(
        &self,
        tool: &str,
        subcommand: Option<&str>,
    ) -> PermissionDecision {
        // If the user already said "always" for this tool, skip approval.
        if self.always_approved_tools.contains(tool) {
            return PermissionDecision::Allowed;
        }
        self.gate.check(tool, subcommand)
    }

    /// Check a shell command against the command filter first, then the
    /// permission gate.
    pub fn check_shell_command(&self, command: &str) -> PermissionDecision {
        match self.command_filter.check_command(command) {
            FilterVerdict::Blocked(reason) => {
                return PermissionDecision::Denied { reason };
            }
            FilterVerdict::Allowed => {}
        }
        // Shell commands that survive the filter still go through the gate.
        self.gate.check("shell", Some("unsafe"))
    }

    /// Check a file path for read/write access.
    pub fn check_path(&self, path: &Path) -> FilterVerdict {
        self.path_filter.check_path(path)
    }

    /// Mark a tool as always-approved for this session.
    pub fn approve_always(&mut self, tool: &str) {
        self.always_approved_tools.insert(tool.to_string());
    }
}

// ---------------------------------------------------------------------------
// 9. Approval flow — interactive y/n/a/q prompt
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalResponse {
    ApproveOnce,
    ApproveAlways,
    Deny,
    AbortSession,
}

/// Show the user what the agent wants to do and wait for a decision.
///
/// `description` should be a human-readable summary, e.g.
///   "Write file: src/main.rs (42 lines)"
///   "Execute: cargo test --release"
pub fn request_approval(description: &str) -> io::Result<ApprovalResponse> {
    println!();
    println!("  [APPROVAL REQUIRED]");
    println!("  {description}");
    println!();
    println!("  [y] approve   [n] deny   [a] always approve this tool   [q] abort session");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        write!(stdout, "  > ")?;
        stdout.flush()?;

        let mut input = String::new();
        stdin.lock().read_line(&mut input)?;

        match input.trim().to_lowercase().as_str() {
            "y" | "yes" => return Ok(ApprovalResponse::ApproveOnce),
            "a" | "always" => return Ok(ApprovalResponse::ApproveAlways),
            "n" | "no" => return Ok(ApprovalResponse::Deny),
            "q" | "quit" | "abort" => return Ok(ApprovalResponse::AbortSession),
            _ => println!("  Please enter: y (approve), n (deny), a (always), q (abort)"),
        }
    }
}

// ---------------------------------------------------------------------------
// 10. Tool execution with integrated permission checks
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ToolResult {
    Success(String),
    Denied(String),
    SessionAborted,
}

/// Execute a generic tool call with the full safety pipeline:
///   permission gate -> approval (if needed) -> execute
pub fn execute_tool(
    mgr: &mut PermissionManager,
    tool: &str,
    subcommand: Option<&str>,
    description: &str,
    execute: impl FnOnce() -> Result<String, String>,
) -> ToolResult {
    let decision = mgr.check_tool(tool, subcommand);

    match decision {
        PermissionDecision::Allowed => match execute() {
            Ok(out) => ToolResult::Success(out),
            Err(e) => ToolResult::Denied(format!("Execution error: {e}")),
        },
        PermissionDecision::NeedsApproval { reason } => {
            let prompt = format!("{description}\n  Reason: {reason}");
            match request_approval(&prompt) {
                Ok(ApprovalResponse::ApproveOnce) => match execute() {
                    Ok(out) => ToolResult::Success(out),
                    Err(e) => ToolResult::Denied(format!("Execution error: {e}")),
                },
                Ok(ApprovalResponse::ApproveAlways) => {
                    mgr.approve_always(tool);
                    match execute() {
                        Ok(out) => ToolResult::Success(out),
                        Err(e) => ToolResult::Denied(format!("Execution error: {e}")),
                    }
                }
                Ok(ApprovalResponse::Deny) => {
                    ToolResult::Denied("User denied the operation".into())
                }
                Ok(ApprovalResponse::AbortSession) => ToolResult::SessionAborted,
                Err(e) => ToolResult::Denied(format!("Approval I/O error: {e}")),
            }
        }
        PermissionDecision::Denied { reason } => ToolResult::Denied(reason),
    }
}

/// Execute a shell command with the full safety pipeline:
///   command filter -> permission gate -> approval -> execute
pub fn execute_shell(
    mgr: &mut PermissionManager,
    command: &str,
) -> ToolResult {
    // Step 1: command filter (allowlist + denylist)
    let filter_decision = mgr.check_shell_command(command);

    match filter_decision {
        PermissionDecision::Denied { reason } => {
            return ToolResult::Denied(reason);
        }
        PermissionDecision::Allowed => {
            // Safe enough that no further approval is needed.
            let output = Command::new("sh")
                .arg("-c")
                .arg(command)
                .output();
            match output {
                Ok(o) if o.status.success() => {
                    ToolResult::Success(String::from_utf8_lossy(&o.stdout).into())
                }
                Ok(o) => ToolResult::Denied(
                    String::from_utf8_lossy(&o.stderr).into(),
                ),
                Err(e) => ToolResult::Denied(format!("Failed to spawn shell: {e}")),
            }
        }
        PermissionDecision::NeedsApproval { reason } => {
            let prompt = format!("Execute: {command}\n  Reason: {reason}");
            match request_approval(&prompt) {
                Ok(ApprovalResponse::ApproveOnce | ApprovalResponse::ApproveAlways) => {
                    let output = Command::new("sh")
                        .arg("-c")
                        .arg(command)
                        .output();
                    match output {
                        Ok(o) if o.status.success() => {
                            ToolResult::Success(
                                String::from_utf8_lossy(&o.stdout).into(),
                            )
                        }
                        Ok(o) => ToolResult::Denied(
                            String::from_utf8_lossy(&o.stderr).into(),
                        ),
                        Err(e) => {
                            ToolResult::Denied(format!("Failed to spawn shell: {e}"))
                        }
                    }
                }
                Ok(ApprovalResponse::Deny) => {
                    ToolResult::Denied("User denied the command".into())
                }
                Ok(ApprovalResponse::AbortSession) => ToolResult::SessionAborted,
                Err(e) => ToolResult::Denied(format!("Approval I/O error: {e}")),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 11. main — demonstrate every safety layer
// ---------------------------------------------------------------------------

fn main() {
    println!("Chapter 12: Permission and Safety\n");

    // -- Carry forward ch11: show git status --------------------------------
    println!("=== Git Status (from ch11) ===");
    match git_status() {
        Ok(s) if s.is_empty() => println!("  (clean working tree)"),
        Ok(s) => print!("{s}"),
        Err(e) => eprintln!("  git error: {e}"),
    }
    println!();

    // -- Permission gate demo -----------------------------------------------
    println!("=== Permission Gate (Standard mode) ===");
    let gate = PermissionGate::new(PermissionLevel::Standard);

    let checks: Vec<(&str, Option<&str>)> = vec![
        ("read_file", None),
        ("write_file", None),
        ("shell", Some("safe")),
        ("shell", Some("unsafe")),
        ("git", Some("push --force")),
    ];

    for (tool, sub) in &checks {
        let label = sub
            .map(|s| format!("{tool}:{s}"))
            .unwrap_or_else(|| tool.to_string());
        let decision = gate.check(tool, *sub);
        println!("  {label:25} => {decision:?}");
    }

    // ReadOnly mode — writes are denied outright
    let ro_gate = PermissionGate::new(PermissionLevel::ReadOnly);
    let d = ro_gate.check("write_file", None);
    println!("  {:<25} => {:?}", "write_file (read-only)", d);
    println!();

    // -- Command filter demo ------------------------------------------------
    println!("=== Command Filter ===");
    let cmd_filter = CommandFilter::with_defaults();

    let commands = [
        "cargo test",
        "ls -la",
        "rm -rf /",
        "rm -rf ~",
        "dd if=/dev/zero of=/dev/sda",
        "chmod 777 /var/www",
        "docker run ubuntu",         // not on allowlist
        "git push --force origin main", // 'git' is allowed but check for deny
    ];

    for cmd in &commands {
        let v = cmd_filter.check_command(cmd);
        match &v {
            FilterVerdict::Allowed => println!("  ALLOWED  {cmd}"),
            FilterVerdict::Blocked(r) => println!("  BLOCKED  {cmd}\n           {r}"),
        }
    }
    println!();

    // -- Path filter demo ---------------------------------------------------
    println!("=== Path Filter ===");
    let project = Path::new("/home/user/my-project");
    let path_filter = PathFilter::with_defaults(project);

    let paths = [
        "/home/user/my-project/src/main.rs",
        "/home/user/my-project/.env",
        "/home/user/.ssh/id_rsa",
        "/etc/passwd",
        "/home/user/my-project/Cargo.toml",
    ];

    for p in &paths {
        let v = path_filter.check_path(Path::new(p));
        match &v {
            FilterVerdict::Allowed => println!("  ALLOWED  {p}"),
            FilterVerdict::Blocked(r) => println!("  BLOCKED  {p}\n           {r}"),
        }
    }
    println!();

    // -- PermissionManager: unified facade ----------------------------------
    println!("=== PermissionManager (unified checks) ===");
    let mgr = PermissionManager::new(PermissionLevel::Standard, project);

    // Shell command that is on the denylist
    let d = mgr.check_shell_command("rm -rf /");
    println!("  rm -rf /     => {d:?}");

    // Shell command that passes the filter but needs approval
    let d = mgr.check_shell_command("cargo test");
    println!("  cargo test   => {d:?}");

    // Path outside workspace
    let v = mgr.check_path(Path::new("/etc/passwd"));
    println!("  /etc/passwd  => {v:?}");

    // Sensitive file inside workspace
    let v = mgr.check_path(Path::new("/home/user/my-project/.env"));
    println!("  .env         => {v:?}");

    // Tool call
    let d = mgr.check_tool("read_file", None);
    println!("  read_file    => {d:?}");

    let d = mgr.check_tool("write_file", None);
    println!("  write_file   => {d:?}");
    println!();

    // -- Approval flow note -------------------------------------------------
    // The interactive approval (request_approval) and the execute_tool /
    // execute_shell helpers are designed for the real agent loop.  Running them
    // here would block on stdin, so we just summarise the API.
    println!("=== Approval + Execution API (not interactive in this demo) ===");
    println!("  request_approval(description) -> ApprovalResponse");
    println!("  execute_tool(mgr, tool, sub, desc, closure) -> ToolResult");
    println!("  execute_shell(mgr, command) -> ToolResult");
    println!();
    println!("All safety layers operational.");
}

// ---------------------------------------------------------------------------
// 12. Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- PermissionLevel ordering ------------------------------------------

    #[test]
    fn permission_level_ordering() {
        assert!(PermissionLevel::ReadOnly < PermissionLevel::Standard);
        assert!(PermissionLevel::Standard < PermissionLevel::FullAuto);
    }

    // -- PermissionGate decisions ------------------------------------------

    #[test]
    fn read_always_allowed() {
        let gate = PermissionGate::new(PermissionLevel::ReadOnly);
        assert_eq!(gate.check("read_file", None), PermissionDecision::Allowed);
        assert_eq!(gate.check("grep", None), PermissionDecision::Allowed);
    }

    #[test]
    fn write_denied_in_readonly() {
        let gate = PermissionGate::new(PermissionLevel::ReadOnly);
        match gate.check("write_file", None) {
            PermissionDecision::Denied { .. } => {}
            other => panic!("expected Denied, got {other:?}"),
        }
    }

    #[test]
    fn write_needs_approval_in_standard() {
        let gate = PermissionGate::new(PermissionLevel::Standard);
        match gate.check("write_file", None) {
            PermissionDecision::NeedsApproval { .. } => {}
            other => panic!("expected NeedsApproval, got {other:?}"),
        }
    }

    #[test]
    fn destructive_denied_below_fullauto() {
        let gate = PermissionGate::new(PermissionLevel::Standard);
        match gate.check("git", Some("push --force")) {
            PermissionDecision::Denied { .. } => {}
            other => panic!("expected Denied, got {other:?}"),
        }
    }

    #[test]
    fn everything_allowed_in_fullauto() {
        let gate = PermissionGate::new(PermissionLevel::FullAuto);
        assert_eq!(gate.check("write_file", None), PermissionDecision::Allowed);
        assert_eq!(
            gate.check("git", Some("push --force")),
            PermissionDecision::Allowed
        );
    }

    // -- CommandFilter ------------------------------------------------------

    #[test]
    fn safe_commands_allowed() {
        let f = CommandFilter::with_defaults();
        assert_eq!(f.check_command("cargo test"), FilterVerdict::Allowed);
        assert_eq!(f.check_command("ls -la"), FilterVerdict::Allowed);
        assert_eq!(f.check_command("git status"), FilterVerdict::Allowed);
    }

    #[test]
    fn dangerous_commands_blocked() {
        let f = CommandFilter::with_defaults();
        match f.check_command("rm -rf /") {
            FilterVerdict::Blocked(_) => {}
            other => panic!("expected Blocked, got {other:?}"),
        }
        match f.check_command("dd if=/dev/zero of=/dev/sda") {
            FilterVerdict::Blocked(_) => {}
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    #[test]
    fn unknown_executable_blocked() {
        let f = CommandFilter::with_defaults();
        match f.check_command("docker run ubuntu") {
            FilterVerdict::Blocked(reason) => {
                assert!(reason.contains("not on the allowlist"));
            }
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    // -- PathFilter ---------------------------------------------------------

    #[test]
    fn project_paths_allowed() {
        let root = Path::new("/home/user/project");
        let f = PathFilter::with_defaults(root);
        assert_eq!(
            f.check_path(Path::new("/home/user/project/src/lib.rs")),
            FilterVerdict::Allowed
        );
    }

    #[test]
    fn sensitive_files_blocked() {
        let root = Path::new("/home/user/project");
        let f = PathFilter::with_defaults(root);
        match f.check_path(Path::new("/home/user/project/.env")) {
            FilterVerdict::Blocked(_) => {}
            other => panic!("expected Blocked, got {other:?}"),
        }
        match f.check_path(Path::new("/home/user/.ssh/id_rsa")) {
            FilterVerdict::Blocked(_) => {}
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    #[test]
    fn paths_outside_project_blocked() {
        let root = Path::new("/home/user/project");
        let f = PathFilter::with_defaults(root);
        match f.check_path(Path::new("/etc/passwd")) {
            FilterVerdict::Blocked(reason) => {
                assert!(reason.contains("outside"));
            }
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    // -- PermissionManager (integration) ------------------------------------

    #[test]
    fn manager_blocks_denylist_commands() {
        let mgr = PermissionManager::new(
            PermissionLevel::FullAuto,
            Path::new("/tmp/project"),
        );
        match mgr.check_shell_command("rm -rf /") {
            PermissionDecision::Denied { .. } => {}
            other => panic!("expected Denied, got {other:?}"),
        }
    }

    #[test]
    fn manager_always_approved_skips_prompt() {
        let mut mgr = PermissionManager::new(
            PermissionLevel::Standard,
            Path::new("/tmp/project"),
        );
        // Before approval, write_file needs approval
        match mgr.check_tool("write_file", None) {
            PermissionDecision::NeedsApproval { .. } => {}
            other => panic!("expected NeedsApproval, got {other:?}"),
        }
        // After always-approve, it should be Allowed
        mgr.approve_always("write_file");
        assert_eq!(
            mgr.check_tool("write_file", None),
            PermissionDecision::Allowed
        );
    }
}
