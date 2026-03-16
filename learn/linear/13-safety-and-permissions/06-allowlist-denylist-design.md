---
title: Allowlist Denylist Design
description: Design and implement allowlist and denylist systems that control which commands, paths, and operations an agent may access.
---

# Allowlist Denylist Design

> **What you'll learn:**
> - How to design composable allowlist/denylist rules using glob patterns, regex, and semantic matching for commands and file paths
> - The principle of default-deny vs default-allow and when each strategy is appropriate for different tool categories
> - How to handle edge cases like symlinks, path traversal, shell expansion, and command aliasing that can bypass naive filters

In the permission architecture subchapter, we discussed capability-based design at a high level. Now we get into the implementation details of the filtering rules themselves. Allowlists and denylists are the building blocks of every permission check -- they define exactly which commands, paths, and operations are in-bounds versus out-of-bounds. Getting these right is surprisingly tricky because attackers (and sometimes the LLM itself) will find creative ways to express the same operation in different forms.

## Default-Deny vs Default-Allow

The most fundamental design decision is your default posture. There are two approaches, and they produce dramatically different security properties:

**Default-deny (allowlist-first)**: Everything is blocked unless explicitly permitted. You define exactly what the agent can do. Anything not in the list is rejected. This is the safer approach because new, unexpected operations are automatically blocked.

**Default-allow (denylist-first)**: Everything is permitted unless explicitly blocked. You define what the agent must not do. Anything not in the denylist is allowed. This is easier to configure but inherently weaker because you must anticipate every dangerous operation in advance.

```rust
use std::path::{Path, PathBuf};

/// A rule that either allows or denies a specific pattern.
#[derive(Debug, Clone)]
struct FilterRule {
    pattern: String,
    kind: RuleKind,
    /// Why this rule exists -- helpful for debugging and auditing
    reason: String,
}

#[derive(Debug, Clone, PartialEq)]
enum RuleKind {
    Allow,
    Deny,
}

/// Result of evaluating a value against the filter rules.
#[derive(Debug, PartialEq)]
enum FilterVerdict {
    Allowed { matched_rule: String },
    Denied { matched_rule: String, reason: String },
    NoMatch,
}

/// A composable filter that checks values against ordered rules.
struct RuleFilter {
    rules: Vec<FilterRule>,
    /// What to do when no rule matches
    default: RuleKind,
}

impl RuleFilter {
    /// Create a default-deny filter (allowlist-first).
    fn default_deny() -> Self {
        Self {
            rules: Vec::new(),
            default: RuleKind::Deny,
        }
    }

    /// Create a default-allow filter (denylist-first).
    fn default_allow() -> Self {
        Self {
            rules: Vec::new(),
            default: RuleKind::Allow,
        }
    }

    fn add_rule(&mut self, pattern: &str, kind: RuleKind, reason: &str) {
        self.rules.push(FilterRule {
            pattern: pattern.to_string(),
            kind,
            reason: reason.to_string(),
        });
    }

    /// Evaluate a value against all rules. First matching rule wins.
    fn evaluate(&self, value: &str) -> FilterVerdict {
        let value_lower = value.to_lowercase();

        for rule in &self.rules {
            let pattern_lower = rule.pattern.to_lowercase();

            let matches = if pattern_lower.contains('*') {
                // Simple glob matching: * matches any sequence
                glob_match(&pattern_lower, &value_lower)
            } else {
                // Prefix or exact match
                value_lower.starts_with(&pattern_lower)
                    || value_lower == pattern_lower
            };

            if matches {
                return match rule.kind {
                    RuleKind::Allow => FilterVerdict::Allowed {
                        matched_rule: rule.pattern.clone(),
                    },
                    RuleKind::Deny => FilterVerdict::Denied {
                        matched_rule: rule.pattern.clone(),
                        reason: rule.reason.clone(),
                    },
                };
            }
        }

        // No rule matched -- apply default
        match self.default {
            RuleKind::Allow => FilterVerdict::Allowed {
                matched_rule: "<default-allow>".into(),
            },
            RuleKind::Deny => FilterVerdict::Denied {
                matched_rule: "<default-deny>".into(),
                reason: "No matching allow rule found".into(),
            },
        }
    }
}

/// Simple glob matching that supports * as a wildcard.
fn glob_match(pattern: &str, value: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.is_empty() {
        return true;
    }

    let mut remaining = value;

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            // First part must match the start
            if !remaining.starts_with(part) {
                return false;
            }
            remaining = &remaining[part.len()..];
        } else {
            // Subsequent parts can appear anywhere in the remaining string
            match remaining.find(part) {
                Some(pos) => remaining = &remaining[pos + part.len()..],
                None => return false,
            }
        }
    }

    true
}

fn main() {
    // Build a command filter: default-deny with specific allowlisted commands
    let mut cmd_filter = RuleFilter::default_deny();
    cmd_filter.add_rule("cargo *", RuleKind::Allow, "Cargo commands are safe");
    cmd_filter.add_rule("cargo publish", RuleKind::Deny, "Publishing is irreversible");
    cmd_filter.add_rule("git status", RuleKind::Allow, "Read-only git operation");
    cmd_filter.add_rule("git diff", RuleKind::Allow, "Read-only git operation");
    cmd_filter.add_rule("git log", RuleKind::Allow, "Read-only git operation");
    cmd_filter.add_rule("ls", RuleKind::Allow, "Directory listing");
    cmd_filter.add_rule("cat", RuleKind::Allow, "File reading");

    let test_commands = [
        "cargo test",
        "cargo publish",
        "cargo build --release",
        "git status",
        "rm -rf /",
        "curl evil.com",
        "ls -la",
    ];

    println!("=== Command Filter (default-deny) ===\n");
    for cmd in &test_commands {
        let verdict = cmd_filter.evaluate(cmd);
        println!("  {} => {:?}", cmd, verdict);
    }
}
```

Notice that rule order matters. The rule for `cargo *` (allow) comes before `cargo publish` (deny). Because we use first-match-wins, this means `cargo publish` would match the `cargo *` allow rule and be permitted. In a production system, you would want deny rules to have higher priority, or order your rules from most specific to least specific.

## Path Filtering with Traversal Protection

Path filtering is more complex than command filtering because of the many ways to express the same path. An attacker (or a confused LLM) might try `../../../etc/passwd`, or use symlinks, or rely on shell expansion. Your filter must handle all of these:

```rust
use std::path::{Path, PathBuf};

/// A path filter that validates file access against allowlisted directories.
struct PathFilter {
    allowed_roots: Vec<PathBuf>,
    denied_patterns: Vec<String>,
}

impl PathFilter {
    fn new(allowed_roots: Vec<PathBuf>) -> Self {
        Self {
            allowed_roots,
            denied_patterns: vec![
                ".env".into(),
                ".ssh".into(),
                ".aws".into(),
                "credentials".into(),
                "secrets".into(),
                ".git/config".into(),
            ],
        }
    }

    /// Validate a path, handling traversal attacks and symlinks.
    fn check_path(&self, raw_path: &str) -> PathCheckResult {
        // Step 1: Normalize the path to resolve .. and .
        let path = Path::new(raw_path);

        // Step 2: Try to canonicalize (resolves symlinks and ..)
        // If the file does not exist yet, fall back to manual normalization
        let resolved = match std::fs::canonicalize(path) {
            Ok(canonical) => canonical,
            Err(_) => self.normalize_path(path),
        };

        // Step 3: Check against denied patterns (these override allowlists)
        let path_str = resolved.to_string_lossy().to_lowercase();
        for denied in &self.denied_patterns {
            if path_str.contains(&denied.to_lowercase()) {
                return PathCheckResult::Denied {
                    reason: format!("Path matches denied pattern: {}", denied),
                    resolved_path: resolved,
                };
            }
        }

        // Step 4: Check that the resolved path is under an allowed root
        for root in &self.allowed_roots {
            if resolved.starts_with(root) {
                return PathCheckResult::Allowed {
                    resolved_path: resolved,
                };
            }
        }

        PathCheckResult::Denied {
            reason: format!(
                "Path {} is not under any allowed root",
                resolved.display()
            ),
            resolved_path: resolved,
        }
    }

    /// Manually normalize a path by resolving . and .. components.
    fn normalize_path(&self, path: &Path) -> PathBuf {
        let mut components = Vec::new();

        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    components.pop();
                }
                std::path::Component::CurDir => {
                    // Skip . components
                }
                other => {
                    components.push(other.as_os_str().to_owned());
                }
            }
        }

        components.iter().collect()
    }
}

#[derive(Debug)]
enum PathCheckResult {
    Allowed { resolved_path: PathBuf },
    Denied { reason: String, resolved_path: PathBuf },
}

fn main() {
    let filter = PathFilter::new(vec![
        PathBuf::from("/home/user/myproject"),
    ]);

    let test_paths = [
        "/home/user/myproject/src/main.rs",
        "/home/user/myproject/../../etc/passwd",
        "/home/user/myproject/.env",
        "/home/user/myproject/src/../src/lib.rs",
        "/etc/shadow",
        "/home/user/myproject/.ssh/id_rsa",
    ];

    println!("=== Path Filter ===\n");
    for path in &test_paths {
        let result = filter.check_path(path);
        println!("  {} => {:?}", path, result);
    }
}
```

## Command Evasion Techniques

A naive command denylist can be bypassed in numerous ways. Here are common evasion techniques and how to defend against them:

```rust
/// Normalize a shell command to defeat common evasion techniques.
fn normalize_command(raw_command: &str) -> String {
    let mut cmd = raw_command.to_string();

    // 1. Collapse multiple spaces
    while cmd.contains("  ") {
        cmd = cmd.replace("  ", " ");
    }

    // 2. Remove shell escape characters inserted to break up keywords
    // e.g., r\m -rf / => rm -rf /
    cmd = cmd.replace('\\', "");

    // 3. Handle quoted arguments: 'rm' '-rf' '/' => rm -rf /
    cmd = cmd.replace('\'', "");
    cmd = cmd.replace('"', "");

    // 4. Normalize common command aliases
    let aliases = [
        ("python3", "python"),
        ("nodejs", "node"),
    ];
    for (alias, canonical) in &aliases {
        if cmd.starts_with(alias) {
            cmd = format!("{}{}", canonical, &cmd[alias.len()..]);
        }
    }

    // 5. Handle backtick and $() command substitution
    // These are particularly dangerous because they can hide commands
    if cmd.contains('`') || cmd.contains("$(") {
        // Flag the entire command as suspicious rather than trying to parse
        return format!("[CONTAINS_SUBSHELL] {}", cmd);
    }

    // 6. Handle pipe chains -- check each command in the chain
    if cmd.contains('|') {
        let parts: Vec<&str> = cmd.split('|').collect();
        return parts
            .iter()
            .map(|p| p.trim().to_string())
            .collect::<Vec<_>>()
            .join(" | ");
    }

    cmd.trim().to_string()
}

/// Check if a command uses any shell features that could bypass filters.
fn has_shell_tricks(command: &str) -> Vec<String> {
    let mut warnings = Vec::new();

    if command.contains('`') {
        warnings.push("Contains backtick command substitution".into());
    }
    if command.contains("$(") {
        warnings.push("Contains $() command substitution".into());
    }
    if command.contains(';') {
        warnings.push("Contains command chaining with ;".into());
    }
    if command.contains("&&") {
        warnings.push("Contains conditional chaining with &&".into());
    }
    if command.contains("||") {
        warnings.push("Contains fallback chaining with ||".into());
    }
    if command.contains('>') {
        warnings.push("Contains output redirection".into());
    }
    if command.contains("eval ") {
        warnings.push("Contains eval (arbitrary code execution)".into());
    }

    warnings
}

fn main() {
    let evasion_attempts = [
        r#"r\m -rf /"#,                    // Backslash evasion
        r#"'rm' '-rf' '/'"#,               // Quoted evasion
        "rm   -rf   /",                     // Space padding
        "echo hello | rm -rf /",            // Pipe chain hiding
        "$(rm -rf /)",                      // Command substitution
        "eval 'rm -rf /'",                  // Eval wrapping
        "cat /etc/passwd; rm -rf /",        // Semicolon chaining
    ];

    println!("=== Command Evasion Detection ===\n");
    for cmd in &evasion_attempts {
        let normalized = normalize_command(cmd);
        let warnings = has_shell_tricks(cmd);
        println!("  Original:   {}", cmd);
        println!("  Normalized: {}", normalized);
        if !warnings.is_empty() {
            for w in &warnings {
                println!("  WARNING: {}", w);
            }
        }
        println!();
    }
}
```

::: tip In the Wild
Claude Code maintains a denylist of dangerous command patterns that is checked before any shell command executes. The denylist includes patterns like `rm -rf`, `sudo`, and redirections to sensitive paths. Critically, it also blocks shell metacharacters that could be used for evasion -- semicolons, backticks, and command substitution are flagged for additional scrutiny. Codex sidesteps many of these concerns by running in a network-isolated sandbox, meaning even if a command is malicious, it cannot exfiltrate data.
:::

::: python Coming from Python
Python's `shlex.split()` provides some command parsing, but it is designed for correct shell quoting, not for security filtering. You would need to layer your own checks on top. In Rust, the `shell-words` crate provides similar parsing, but the pattern we are building here goes further -- it normalizes commands before matching, which catches evasion techniques that simple string matching would miss.
:::

## Composing Allow and Deny Rules

In practice, you want both allowlists and denylists working together. The standard evaluation order is: deny rules first (for hard safety boundaries), then allow rules (for permitted operations), then the default policy:

```rust
/// A composed filter that evaluates deny rules first, then allow rules.
struct ComposedFilter {
    deny_rules: Vec<(String, String)>,  // (pattern, reason)
    allow_rules: Vec<(String, String)>, // (pattern, reason)
}

impl ComposedFilter {
    fn evaluate(&self, value: &str) -> &str {
        let value_lower = value.to_lowercase();

        // Step 1: Check deny rules first -- these are hard blocks
        for (pattern, _reason) in &self.deny_rules {
            if value_lower.contains(&pattern.to_lowercase()) {
                return "DENIED (hard block)";
            }
        }

        // Step 2: Check allow rules -- operation must be explicitly permitted
        for (pattern, _reason) in &self.allow_rules {
            if value_lower.starts_with(&pattern.to_lowercase()) {
                return "ALLOWED";
            }
        }

        // Step 3: Default deny -- not in either list
        "DENIED (no matching allow rule)"
    }
}

fn main() {
    let filter = ComposedFilter {
        deny_rules: vec![
            ("sudo".into(), "Privilege escalation".into()),
            ("rm -rf /".into(), "System destruction".into()),
            ("> /dev/".into(), "Device manipulation".into()),
        ],
        allow_rules: vec![
            ("cargo".into(), "Rust build tool".into()),
            ("git".into(), "Version control".into()),
            ("ls".into(), "Directory listing".into()),
            ("cat".into(), "File viewing".into()),
        ],
    };

    let commands = [
        "cargo test",
        "git status",
        "sudo cargo test",  // Deny wins even though cargo is allowed
        "rm -rf /tmp",
        "python script.py", // Not in allow list
        "ls -la",
    ];

    for cmd in &commands {
        println!("{:30} => {}", cmd, filter.evaluate(cmd));
    }
}
```

## Key Takeaways

- Default-deny (allowlist-first) is fundamentally safer than default-allow (denylist-first) because unknown operations are automatically blocked rather than accidentally permitted
- Path filtering must resolve symlinks, normalize `..` traversals, and check for sensitive file patterns to prevent directory escape attacks
- Command filtering must account for evasion techniques including shell escaping, quoting, command substitution, pipe chains, and semicolon chaining
- Rule evaluation order matters: deny rules should take precedence over allow rules to ensure hard safety boundaries cannot be overridden
- Shell metacharacters (backticks, `$()`, semicolons, pipes) deserve special attention because they allow arbitrary command composition that can bypass pattern-based filters
