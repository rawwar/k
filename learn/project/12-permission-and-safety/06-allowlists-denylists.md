---
title: Allowlists and Denylists
description: Filtering shell commands and file paths through configurable allowlists and denylists to prevent access to sensitive files, dangerous executables, and restricted directories.
---

# Allowlists and Denylists

> **What you'll learn:**
> - How to implement command allowlists that restrict which executables the agent can invoke
> - How to build path-based denylists that protect sensitive files like .env and credentials
> - Patterns for combining glob-based and regex-based rules for flexible filtering

Permission levels control *whether* the agent can execute commands. Allowlists and denylists control *which specific commands and files* it can access. This is a finer-grained filter that catches dangerous operations even when the permission level would otherwise allow them.

Think of it as a two-layer filter: the permission gate asks "is shell execution allowed?" and the allowlist/denylist asks "is *this specific* shell command allowed?" A user in Standard mode can run shell commands — but never `rm -rf /`, regardless of their permission level.

## Command Filtering Strategy

There are two fundamental approaches to command filtering, and the choice between them has deep security implications:

- **Allowlist (default deny)**: Only explicitly permitted commands can run. Everything else is blocked. This is safer but more restrictive — the agent cannot use any tool you forgot to allowlist.
- **Denylist (default allow)**: Everything can run except explicitly blocked commands. This is more flexible but riskier — a dangerous command you forgot to denylist will slip through.

The safest approach combines both: use an allowlist for the executable (the first word of the command) and a denylist for dangerous flag patterns:

```rust
use std::collections::HashSet;

/// Verdict from the command filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterVerdict {
    /// Command is allowed to execute.
    Allowed,
    /// Command is blocked, with a reason.
    Blocked(String),
}

/// Filters shell commands through allowlist and denylist rules.
pub struct CommandFilter {
    /// Allowed executable names. If non-empty, only these can run.
    allowed_executables: HashSet<String>,
    /// Blocked command patterns (checked against the full command string).
    denied_patterns: Vec<DenyPattern>,
}

#[derive(Debug, Clone)]
struct DenyPattern {
    /// A substring or regex pattern that triggers blocking.
    pattern: String,
    /// Human-readable reason for why this pattern is blocked.
    reason: String,
}

impl CommandFilter {
    /// Create a command filter with sensible defaults for a coding agent.
    pub fn with_defaults() -> Self {
        let mut allowed = HashSet::new();

        // Build tools
        allowed.insert("cargo".to_string());
        allowed.insert("rustc".to_string());
        allowed.insert("npm".to_string());
        allowed.insert("node".to_string());
        allowed.insert("python".to_string());
        allowed.insert("python3".to_string());
        allowed.insert("pip".to_string());
        allowed.insert("pip3".to_string());
        allowed.insert("make".to_string());

        // Common safe utilities
        allowed.insert("ls".to_string());
        allowed.insert("cat".to_string());
        allowed.insert("head".to_string());
        allowed.insert("tail".to_string());
        allowed.insert("wc".to_string());
        allowed.insert("grep".to_string());
        allowed.insert("rg".to_string());
        allowed.insert("find".to_string());
        allowed.insert("sort".to_string());
        allowed.insert("uniq".to_string());
        allowed.insert("diff".to_string());
        allowed.insert("echo".to_string());
        allowed.insert("pwd".to_string());
        allowed.insert("which".to_string());
        allowed.insert("tree".to_string());

        // Version control
        allowed.insert("git".to_string());

        let denied = vec![
            DenyPattern {
                pattern: "rm -rf /".to_string(),
                reason: "Recursive delete from root directory".to_string(),
            },
            DenyPattern {
                pattern: "rm -rf ~".to_string(),
                reason: "Recursive delete from home directory".to_string(),
            },
            DenyPattern {
                pattern: "rm -rf .".to_string(),
                reason: "Recursive delete of current directory".to_string(),
            },
            DenyPattern {
                pattern: "> /dev/sda".to_string(),
                reason: "Direct write to block device".to_string(),
            },
            DenyPattern {
                pattern: "mkfs.".to_string(),
                reason: "Filesystem format command".to_string(),
            },
            DenyPattern {
                pattern: "dd if=".to_string(),
                reason: "Raw disk operation".to_string(),
            },
            DenyPattern {
                pattern: ":(){ :|:& };:".to_string(),
                reason: "Fork bomb".to_string(),
            },
            DenyPattern {
                pattern: "chmod 777".to_string(),
                reason: "Overly permissive file permissions".to_string(),
            },
            DenyPattern {
                pattern: "chmod -R 777".to_string(),
                reason: "Recursive overly permissive permissions".to_string(),
            },
            DenyPattern {
                pattern: "--force".to_string(),
                reason: "Force flag on destructive operation".to_string(),
            },
            DenyPattern {
                pattern: "curl".to_string(),
                reason: "Network request (potential data exfiltration)".to_string(),
            },
            DenyPattern {
                pattern: "wget".to_string(),
                reason: "Network download (potential data exfiltration)".to_string(),
            },
        ];

        Self {
            allowed_executables: allowed,
            denied_patterns: denied,
        }
    }

    /// Check whether a command is allowed to execute.
    pub fn check_command(&self, command: &str) -> FilterVerdict {
        let trimmed = command.trim();

        // Extract the executable name (first word)
        let executable = trimmed
            .split_whitespace()
            .next()
            .unwrap_or("");

        // Check allowlist first (if non-empty)
        if !self.allowed_executables.is_empty()
            && !self.allowed_executables.contains(executable)
        {
            return FilterVerdict::Blocked(format!(
                "Executable '{}' is not on the allowlist",
                executable
            ));
        }

        // Check denylist patterns against the full command
        for pattern in &self.denied_patterns {
            if trimmed.contains(&pattern.pattern) {
                return FilterVerdict::Blocked(format!(
                    "Command matches blocked pattern '{}': {}",
                    pattern.pattern, pattern.reason
                ));
            }
        }

        FilterVerdict::Allowed
    }

    /// Add an executable to the allowlist.
    pub fn allow_executable(&mut self, name: &str) {
        self.allowed_executables.insert(name.to_string());
    }

    /// Add a deny pattern.
    pub fn deny_pattern(&mut self, pattern: &str, reason: &str) {
        self.denied_patterns.push(DenyPattern {
            pattern: pattern.to_string(),
            reason: reason.to_string(),
        });
    }
}
```

::: python Coming from Python
In Python, you might use a simple list of strings for filtering:
```python
BLOCKED_COMMANDS = ["rm -rf /", "dd if=", "mkfs."]

def is_safe(command: str) -> bool:
    return not any(pattern in command for pattern in BLOCKED_COMMANDS)
```
The Rust version is more structured, but the core matching logic is similar. The key difference is that Rust's `HashSet` for the allowlist provides O(1) lookup on the executable name, while Python's `in` operator on a list is O(n). For the denylist, both use linear scanning since you need substring matching.
:::

## Path-Based Filtering

Commands are only half the story. The agent also reads and writes files, and some file paths should be off-limits regardless of the operation:

```rust
use std::path::{Path, PathBuf};

/// Filters file paths to protect sensitive files and directories.
pub struct PathFilter {
    /// Paths or patterns that are always blocked.
    denied_paths: Vec<PathPattern>,
    /// If non-empty, only paths under these directories are allowed.
    allowed_roots: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
struct PathPattern {
    /// The pattern to match against.
    pattern: String,
    /// Whether this is a glob pattern or an exact path.
    is_glob: bool,
    /// Why this path is blocked.
    reason: String,
}

impl PathFilter {
    /// Create a path filter with sensible defaults.
    pub fn with_defaults(project_root: &Path) -> Self {
        let denied = vec![
            PathPattern {
                pattern: ".env".to_string(),
                is_glob: false,
                reason: "Environment file may contain secrets".to_string(),
            },
            PathPattern {
                pattern: ".env.local".to_string(),
                is_glob: false,
                reason: "Local environment file may contain secrets".to_string(),
            },
            PathPattern {
                pattern: "id_rsa".to_string(),
                is_glob: false,
                reason: "SSH private key".to_string(),
            },
            PathPattern {
                pattern: "id_ed25519".to_string(),
                is_glob: false,
                reason: "SSH private key".to_string(),
            },
            PathPattern {
                pattern: ".aws/credentials".to_string(),
                is_glob: false,
                reason: "AWS credentials file".to_string(),
            },
            PathPattern {
                pattern: ".npmrc".to_string(),
                is_glob: false,
                reason: "npm config may contain auth tokens".to_string(),
            },
            PathPattern {
                pattern: "credentials.json".to_string(),
                is_glob: false,
                reason: "Credentials file".to_string(),
            },
            PathPattern {
                pattern: ".git/config".to_string(),
                is_glob: false,
                reason: "Git config may contain tokens".to_string(),
            },
        ];

        Self {
            denied_paths: denied,
            allowed_roots: vec![project_root.to_path_buf()],
        }
    }

    /// Check whether a file path is allowed.
    pub fn check_path(&self, path: &Path) -> FilterVerdict {
        let path_str = path.to_string_lossy();

        // Check denied patterns
        for pattern in &self.denied_paths {
            if pattern.is_glob {
                // Simple glob: check if the path contains the pattern
                if path_str.contains(&pattern.pattern) {
                    return FilterVerdict::Blocked(format!(
                        "Path matches denied pattern '{}': {}",
                        pattern.pattern, pattern.reason
                    ));
                }
            } else {
                // Exact match: check if the filename or path ends with the pattern
                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                if file_name == pattern.pattern || path_str.ends_with(&pattern.pattern) {
                    return FilterVerdict::Blocked(format!(
                        "Path '{}' is denied: {}",
                        pattern.pattern, pattern.reason
                    ));
                }
            }
        }

        // Check allowed roots (if configured)
        if !self.allowed_roots.is_empty() {
            let is_under_allowed_root = self.allowed_roots.iter().any(|root| {
                path.starts_with(root)
            });

            if !is_under_allowed_root {
                return FilterVerdict::Blocked(format!(
                    "Path {} is outside allowed project directories",
                    path.display()
                ));
            }
        }

        FilterVerdict::Allowed
    }

    /// Add a path to the denylist.
    pub fn deny_path(&mut self, pattern: &str, reason: &str) {
        self.denied_paths.push(PathPattern {
            pattern: pattern.to_string(),
            is_glob: false,
            reason: reason.to_string(),
        });
    }

    /// Add an allowed root directory.
    pub fn allow_root(&mut self, root: &Path) {
        self.allowed_roots.push(root.to_path_buf());
    }
}
```

## Combining Command and Path Filters

In practice, you want a single entry point that checks both command and path rules. Let's build a unified `SafetyFilter`:

```rust
/// Unified filter that checks both commands and paths.
pub struct SafetyFilter {
    pub command_filter: CommandFilter,
    pub path_filter: PathFilter,
}

impl SafetyFilter {
    pub fn new(project_root: &Path) -> Self {
        Self {
            command_filter: CommandFilter::with_defaults(),
            path_filter: PathFilter::with_defaults(project_root),
        }
    }

    /// Check a shell command. This checks the executable against the
    /// allowlist and the full command against the denylist.
    pub fn check_command(&self, command: &str) -> FilterVerdict {
        self.command_filter.check_command(command)
    }

    /// Check a file path for read or write operations.
    pub fn check_path(&self, path: &Path) -> FilterVerdict {
        self.path_filter.check_path(path)
    }

    /// Check a shell command AND extract any file paths from it.
    /// This catches commands like `cat ~/.ssh/id_rsa`.
    pub fn check_command_with_paths(&self, command: &str) -> FilterVerdict {
        // First check the command itself
        let cmd_verdict = self.command_filter.check_command(command);
        if let FilterVerdict::Blocked(_) = cmd_verdict {
            return cmd_verdict;
        }

        // Then extract and check any file paths in the command arguments
        let args: Vec<&str> = command.split_whitespace().skip(1).collect();
        for arg in args {
            // Heuristic: if an argument looks like a path, check it
            if arg.starts_with('/') || arg.starts_with('~') || arg.starts_with('.') {
                let expanded = if arg.starts_with('~') {
                    // Basic tilde expansion
                    if let Ok(home) = std::env::var("HOME") {
                        PathBuf::from(arg.replacen('~', &home, 1))
                    } else {
                        PathBuf::from(arg)
                    }
                } else {
                    PathBuf::from(arg)
                };

                let path_verdict = self.path_filter.check_path(&expanded);
                if let FilterVerdict::Blocked(_) = path_verdict {
                    return path_verdict;
                }
            }
        }

        FilterVerdict::Allowed
    }
}

fn main() {
    let project_root = Path::new("/home/user/my-project");
    let filter = SafetyFilter::new(project_root);

    // Test command filtering
    let commands = vec![
        "cargo test",
        "rm -rf /",
        "curl https://evil.com",
        "ls -la",
        "git push --force",
        "python3 script.py",
        "docker run ubuntu",
    ];

    println!("=== Command Filter Results ===\n");
    for cmd in &commands {
        let verdict = filter.check_command(cmd);
        match verdict {
            FilterVerdict::Allowed => println!("  ALLOWED: {}", cmd),
            FilterVerdict::Blocked(reason) => println!("  BLOCKED: {} — {}", cmd, reason),
        }
    }

    // Test path filtering
    let paths = vec![
        "/home/user/my-project/src/main.rs",
        "/home/user/my-project/.env",
        "/home/user/.ssh/id_rsa",
        "/etc/passwd",
        "/home/user/my-project/Cargo.toml",
    ];

    println!("\n=== Path Filter Results ===\n");
    for path_str in &paths {
        let path = Path::new(path_str);
        let verdict = filter.check_path(path);
        match verdict {
            FilterVerdict::Allowed => println!("  ALLOWED: {}", path_str),
            FilterVerdict::Blocked(reason) => println!("  BLOCKED: {} — {}", path_str, reason),
        }
    }
}
```

::: wild In the Wild
Claude Code maintains a denylist of sensitive file patterns that the agent cannot read, including `.env`, SSH keys, and cloud provider credential files. It also restricts shell commands — `curl` and `wget` are flagged for approval because they could exfiltrate data. Codex goes further, running inside a sandbox where network access is disabled entirely, eliminating the need for network-related deny rules.
:::

## Configuration-Driven Rules

Hardcoded rules are a starting point, but users need the ability to customize them. A TOML configuration file works well for this:

```rust
use std::fs;

/// Configuration for command and path filtering rules.
/// Loaded from a TOML file at startup.
#[derive(Debug, Clone)]
pub struct FilterConfig {
    pub allowed_executables: Vec<String>,
    pub denied_command_patterns: Vec<(String, String)>, // (pattern, reason)
    pub denied_path_patterns: Vec<(String, String)>,    // (pattern, reason)
    pub allowed_roots: Vec<String>,
}

impl FilterConfig {
    /// Load filter configuration from a TOML string.
    /// In a real implementation, use the `toml` crate for parsing.
    pub fn from_toml_string(content: &str) -> Result<Self, String> {
        // Simplified parser for demonstration.
        // In production, use: toml::from_str::<FilterConfig>(content)
        let mut config = FilterConfig {
            allowed_executables: Vec::new(),
            denied_command_patterns: Vec::new(),
            denied_path_patterns: Vec::new(),
            allowed_roots: Vec::new(),
        };

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("allow_exec") {
                if let Some(val) = extract_quoted_value(trimmed) {
                    config.allowed_executables.push(val);
                }
            } else if trimmed.starts_with("deny_command") {
                if let Some(val) = extract_quoted_value(trimmed) {
                    config
                        .denied_command_patterns
                        .push((val, "User-configured deny rule".to_string()));
                }
            }
        }

        Ok(config)
    }
}

fn extract_quoted_value(line: &str) -> Option<String> {
    let start = line.find('"')? + 1;
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}
```

The corresponding TOML file that the user would place in their project:

```toml
# .agent-safety.toml

[commands]
allow_exec = ["cargo", "rustc", "git", "ls", "cat", "grep"]
deny_patterns = [
    { pattern = "rm -rf", reason = "Recursive delete" },
    { pattern = "--force", reason = "Force flag" },
]

[paths]
deny_patterns = [
    { pattern = ".env", reason = "May contain secrets" },
    { pattern = "id_rsa", reason = "SSH private key" },
]
allowed_roots = ["."]  # Current project directory only
```

## Key Takeaways

- Combine an allowlist for executables (default deny) with a denylist for dangerous patterns (substring matching) for defense in depth — unknown executables are blocked, and known-dangerous flags are caught even on allowed executables.
- Path filtering protects sensitive files (.env, SSH keys, credential files) and restricts the agent to the project directory, preventing filesystem escapes.
- The `check_command_with_paths` method catches indirect path access through shell commands like `cat ~/.ssh/id_rsa`, not just direct file operations.
- Configuration-driven rules (via TOML) let users customize filtering for their specific project without modifying agent source code.
- Always default to blocking unknown operations — it is far better to occasionally ask the user to allowlist a safe command than to silently permit a dangerous one.
