---
title: Testing Safety Systems
description: Verify that permission checks, sandboxes, and approval flows actually work under adversarial conditions through systematic testing.
---

# Testing Safety Systems

> **What you'll learn:**
> - How to write adversarial test cases that attempt to bypass permission checks, escape sandboxes, and circumvent approval flows
> - Techniques for fuzzing safety boundaries with randomized inputs to discover edge cases in allowlist/denylist logic
> - How to build regression test suites that ensure safety invariants are never accidentally weakened by future code changes

Building safety systems is only half the job. The other half -- and arguably the more important half -- is verifying that they actually work. Safety code that has never been tested against adversarial inputs is safety code that only works by coincidence. In this subchapter, we build a comprehensive testing strategy that gives you confidence your permission checks, denylists, sandboxes, and approval flows hold up under pressure.

## The Safety Testing Mindset

Testing safety systems requires a fundamentally different mindset from testing regular application code. When testing a web handler, you verify that valid inputs produce correct outputs. When testing a safety system, you verify that invalid, malicious, and unexpected inputs are correctly rejected. Your test suite should read like a catalog of attack attempts:

```rust
#[cfg(test)]
mod permission_tests {
    use super::*;

    /// Test that basic path traversal is blocked.
    #[test]
    fn test_path_traversal_blocked() {
        let checker = PathChecker::new("/home/user/project");

        // Direct traversal
        assert!(checker.check("../../../etc/passwd").is_err());
        // Traversal embedded in a legitimate-looking path
        assert!(checker.check("/home/user/project/../../etc/passwd").is_err());
        // Double-encoded traversal
        assert!(checker.check("/home/user/project/%2e%2e/etc/passwd").is_err());
    }

    /// Test that null bytes in paths are rejected.
    #[test]
    fn test_null_byte_injection() {
        let checker = PathChecker::new("/home/user/project");

        // Null byte can truncate path checks in some implementations
        assert!(checker.check("/home/user/project/safe.txt\0../../etc/passwd").is_err());
    }

    /// Test that symlinks outside the project are blocked.
    #[test]
    fn test_symlink_escape() {
        let checker = PathChecker::new("/home/user/project");

        // Even if the symlink is inside the project, it points outside
        // The checker must resolve symlinks before comparing paths
        // (This test would need actual symlinks in the test environment)
        assert!(checker.check("/home/user/project/link_to_etc").is_err());
    }

    /// Test that denylisted commands cannot be executed.
    #[test]
    fn test_command_denylist() {
        let checker = CommandChecker::new();

        // Basic dangerous commands
        assert!(checker.check("rm -rf /").is_err());
        assert!(checker.check("sudo anything").is_err());

        // Evasion attempts
        assert!(checker.check("r\\m -rf /").is_err());  // Backslash evasion
        assert!(checker.check("'rm' '-rf' '/'").is_err()); // Quote evasion
        assert!(checker.check("rm    -rf   /").is_err()); // Extra spaces
    }

    /// Test that allowed commands work correctly.
    #[test]
    fn test_command_allowlist() {
        let checker = CommandChecker::new();

        assert!(checker.check("cargo test").is_ok());
        assert!(checker.check("cargo build --release").is_ok());
        assert!(checker.check("git status").is_ok());
        assert!(checker.check("git diff HEAD").is_ok());
    }
}

// The types these tests reference:

struct PathChecker {
    project_root: String,
}

impl PathChecker {
    fn new(root: &str) -> Self {
        Self { project_root: root.to_string() }
    }

    fn check(&self, path: &str) -> Result<(), String> {
        // Check for null bytes
        if path.contains('\0') {
            return Err("Path contains null byte".into());
        }

        // Check for URL-encoded traversal
        if path.contains("%2e") || path.contains("%2E") {
            return Err("Path contains encoded traversal".into());
        }

        // Normalize and check containment
        let normalized = normalize_path(path);
        if !normalized.starts_with(&self.project_root) {
            return Err(format!("Path {} escapes project root", path));
        }

        Ok(())
    }
}

fn normalize_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for component in path.split('/') {
        match component {
            ".." => { parts.pop(); },
            "." | "" => {},
            other => parts.push(other),
        }
    }
    format!("/{}", parts.join("/"))
}

struct CommandChecker {
    denylist: Vec<String>,
    allowlist: Vec<String>,
}

impl CommandChecker {
    fn new() -> Self {
        Self {
            denylist: vec![
                "rm -rf".into(), "sudo".into(), "chmod 777".into(),
            ],
            allowlist: vec![
                "cargo".into(), "git".into(), "ls".into(), "cat".into(),
            ],
        }
    }

    fn check(&self, command: &str) -> Result<(), String> {
        // Normalize: remove extra spaces, backslashes, quotes
        let normalized = command
            .replace('\\', "")
            .replace('\'', "")
            .replace('"', "")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        // Check denylist first
        let lower = normalized.to_lowercase();
        for denied in &self.denylist {
            if lower.contains(&denied.to_lowercase()) {
                return Err(format!("Command matches denylist: {}", denied));
            }
        }

        // Check allowlist
        let first_word = normalized.split_whitespace().next().unwrap_or("");
        if self.allowlist.iter().any(|a| first_word == a) {
            return Ok(());
        }

        Err("Command not in allowlist".into())
    }
}

fn main() {
    // Run the tests conceptually
    let path_checker = PathChecker::new("/home/user/project");
    let cmd_checker = CommandChecker::new();

    println!("=== Path Safety Tests ===\n");
    let path_tests = [
        ("/home/user/project/src/main.rs", true),
        ("../../../etc/passwd", false),
        ("/home/user/project/../../etc/passwd", false),
        ("/home/user/project/safe.txt\0../../etc/passwd", false),
    ];

    for (path, should_pass) in &path_tests {
        let result = path_checker.check(path);
        let passed = result.is_ok() == *should_pass;
        println!("  {} => {} ({})",
            path,
            if result.is_ok() { "ALLOWED" } else { "BLOCKED" },
            if passed { "CORRECT" } else { "WRONG!" },
        );
    }

    println!("\n=== Command Safety Tests ===\n");
    let cmd_tests = [
        ("cargo test", true),
        ("rm -rf /", false),
        ("r\\m -rf /", false),
        ("sudo cargo test", false),
        ("git status", true),
    ];

    for (cmd, should_pass) in &cmd_tests {
        let result = cmd_checker.check(cmd);
        let passed = result.is_ok() == *should_pass;
        println!("  {:30} => {} ({})",
            cmd,
            if result.is_ok() { "ALLOWED" } else { "BLOCKED" },
            if passed { "CORRECT" } else { "WRONG!" },
        );
    }
}
```

## Property-Based Testing for Safety

Individual test cases are valuable, but they only cover the specific inputs you thought to test. Property-based testing generates random inputs and checks that safety invariants always hold:

```rust
/// A simple property-based test framework for safety invariants.
struct PropertyTest {
    name: String,
    passes: u32,
    failures: Vec<String>,
}

impl PropertyTest {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            passes: 0,
            failures: Vec::new(),
        }
    }

    /// Run the property test with generated inputs.
    fn run<F>(&mut self, iterations: u32, test_fn: F)
    where
        F: Fn(&str) -> Result<(), String>,
    {
        let inputs = self.generate_adversarial_paths(iterations);

        for input in &inputs {
            match test_fn(input) {
                Ok(()) => {
                    // The path was allowed -- verify it is actually safe
                    if self.looks_dangerous(input) {
                        self.failures.push(format!(
                            "SAFETY VIOLATION: '{}' was allowed but looks dangerous",
                            input
                        ));
                    } else {
                        self.passes += 1;
                    }
                }
                Err(_) => {
                    // The path was blocked -- this is correct for dangerous paths
                    self.passes += 1;
                }
            }
        }
    }

    /// Generate adversarial path inputs for testing.
    fn generate_adversarial_paths(&self, count: u32) -> Vec<String> {
        let mut paths = Vec::new();

        // Traversal patterns
        let traversals = ["../", "..\\", "%2e%2e/", "..%2f", "%2e%2e%2f"];
        for t in &traversals {
            paths.push(format!("/home/user/project/{}{}{}/etc/passwd", t, t, t));
        }

        // Null byte injections
        paths.push("/home/user/project/safe.rs\0/etc/passwd".into());

        // Very long paths
        paths.push(format!("/home/user/project/{}", "a".repeat(10000)));

        // Unicode tricks
        paths.push("/home/user/project/\u{2025}/etc/passwd".into()); // Two-dot leader
        paths.push("/home/user/project/\u{FF0E}\u{FF0E}/etc/passwd".into()); // Fullwidth dots

        // Normal safe paths (should be allowed)
        paths.push("/home/user/project/src/main.rs".into());
        paths.push("/home/user/project/Cargo.toml".into());

        // Trim to requested count
        paths.truncate(count as usize);
        paths
    }

    /// Heuristic check for obviously dangerous paths.
    fn looks_dangerous(&self, path: &str) -> bool {
        let sensitive = ["/etc/", "/root/", ".ssh/", ".env", "/dev/"];
        let lower = path.to_lowercase();
        sensitive.iter().any(|s| lower.contains(s))
    }

    fn report(&self) {
        println!("Property test '{}': {} passes, {} failures",
            self.name, self.passes, self.failures.len());
        for failure in &self.failures {
            println!("  FAILURE: {}", failure);
        }
    }
}

fn main() {
    let mut test = PropertyTest::new("path_safety_invariant");

    let checker = PathChecker::new("/home/user/project");

    test.run(20, |path| checker.check(path));
    test.report();
}

// PathChecker from previous example
struct PathChecker {
    project_root: String,
}

impl PathChecker {
    fn new(root: &str) -> Self {
        Self { project_root: root.to_string() }
    }

    fn check(&self, path: &str) -> Result<(), String> {
        if path.contains('\0') {
            return Err("Null byte in path".into());
        }
        if path.contains("%2e") || path.contains("%2E") || path.contains("%2f") || path.contains("%2F") {
            return Err("Encoded traversal in path".into());
        }

        let normalized = normalize_path(path);
        if !normalized.starts_with(&self.project_root) {
            return Err(format!("Path escapes project root: {}", normalized));
        }
        Ok(())
    }
}

fn normalize_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for component in path.split('/') {
        match component {
            ".." => { parts.pop(); }
            "." | "" => {}
            other => parts.push(other),
        }
    }
    format!("/{}", parts.join("/"))
}
```

## Regression Tests for Safety Invariants

Every time you discover a bypass -- through testing, fuzzing, or a real incident -- add a regression test that specifically covers that case. Over time, this builds a comprehensive catalog of known attack patterns:

```rust
/// A catalog of known bypass attempts that serve as regression tests.
/// Each entry documents a real or hypothetical bypass and verifies the fix.
struct SafetyRegressionSuite {
    tests: Vec<RegressionTest>,
}

struct RegressionTest {
    /// Identifier for this regression test
    id: String,
    /// When this bypass was discovered
    discovered: String,
    /// Description of the bypass
    description: String,
    /// The input that triggered the bypass
    input: String,
    /// Expected result after the fix
    expected_blocked: bool,
    /// Test function
    test_fn: Box<dyn Fn(&str) -> bool>,
}

impl SafetyRegressionSuite {
    fn new() -> Self {
        Self { tests: Vec::new() }
    }

    fn add(&mut self, test: RegressionTest) {
        self.tests.push(test);
    }

    fn run_all(&self) -> (u32, u32) {
        let mut passed = 0u32;
        let mut failed = 0u32;

        println!("=== Safety Regression Suite ===\n");
        for test in &self.tests {
            let was_blocked = (test.test_fn)(&test.input);
            let correct = was_blocked == test.expected_blocked;

            if correct {
                passed += 1;
                println!("  PASS [{}]: {}", test.id, test.description);
            } else {
                failed += 1;
                println!("  FAIL [{}]: {}", test.id, test.description);
                println!("    Input: {}", test.input);
                println!("    Expected blocked: {}, Got blocked: {}",
                    test.expected_blocked, was_blocked);
            }
        }

        println!("\nResults: {} passed, {} failed", passed, failed);
        (passed, failed)
    }
}

fn main() {
    let mut suite = SafetyRegressionSuite::new();

    let checker = CommandChecker::new();

    suite.add(RegressionTest {
        id: "REG-001".into(),
        discovered: "2025-01-15".into(),
        description: "Backslash evasion in rm command".into(),
        input: r"r\m -rf /".into(),
        expected_blocked: true,
        test_fn: Box::new(move |input| {
            let c = CommandChecker::new();
            c.check(input).is_err()
        }),
    });

    suite.add(RegressionTest {
        id: "REG-002".into(),
        discovered: "2025-02-03".into(),
        description: "Quote wrapping to bypass denylist".into(),
        input: "'sudo' 'bash'".into(),
        expected_blocked: true,
        test_fn: Box::new(move |input| {
            let c = CommandChecker::new();
            c.check(input).is_err()
        }),
    });

    suite.add(RegressionTest {
        id: "REG-003".into(),
        discovered: "2025-03-10".into(),
        description: "Legitimate cargo command should pass".into(),
        input: "cargo test --release".into(),
        expected_blocked: false,
        test_fn: Box::new(move |input| {
            let c = CommandChecker::new();
            c.check(input).is_err()
        }),
    });

    suite.add(RegressionTest {
        id: "REG-004".into(),
        discovered: "2025-03-12".into(),
        description: "Command substitution should be blocked".into(),
        input: "echo $(rm -rf /)".into(),
        expected_blocked: true,
        test_fn: Box::new(move |input| {
            let c = CommandChecker::new();
            c.check(input).is_err()
        }),
    });

    suite.run_all();
}

// Reuse the CommandChecker from earlier
struct CommandChecker {
    denylist: Vec<String>,
    allowlist: Vec<String>,
}

impl CommandChecker {
    fn new() -> Self {
        Self {
            denylist: vec!["rm -rf".into(), "sudo".into(), "chmod 777".into()],
            allowlist: vec!["cargo".into(), "git".into(), "ls".into(), "cat".into(), "echo".into()],
        }
    }

    fn check(&self, command: &str) -> Result<(), String> {
        let normalized = command
            .replace('\\', "")
            .replace('\'', "")
            .replace('"', "")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        let lower = normalized.to_lowercase();

        // Check for command substitution
        if lower.contains("$(") || lower.contains('`') {
            return Err("Command substitution detected".into());
        }

        for denied in &self.denylist {
            if lower.contains(&denied.to_lowercase()) {
                return Err(format!("Denylist match: {}", denied));
            }
        }

        let first_word = normalized.split_whitespace().next().unwrap_or("");
        if self.allowlist.iter().any(|a| first_word == a) {
            return Ok(());
        }

        Err("Not in allowlist".into())
    }
}
```

::: wild In the Wild
Claude Code's safety mechanisms are tested against known prompt injection patterns and command evasion techniques. The development team maintains a growing set of adversarial test cases based on real-world bypass attempts reported by users and security researchers. Codex similarly tests its sandboxing by attempting escapes from inside the sandbox -- verifying that network requests fail when network is disabled, that file access outside the mount point is blocked, and that privilege escalation attempts are rejected.
:::

::: python Coming from Python
Python developers often use `pytest` with parameterized tests for exhaustive input coverage. In Rust, the `proptest` crate provides property-based testing similar to Python's `hypothesis`. The key difference is that Rust's test failures are caught at compile time when they violate type constraints -- for example, a `PathBuf` cannot contain a null byte, so the type system eliminates an entire class of injection attacks that Python's `str` type happily accepts.
:::

## Testing the Integration

Individual component tests are necessary but not sufficient. You must also test how safety components interact -- can an operation that is allowed by the permission system be blocked by the rate limiter? Does the approval flow correctly integrate with the audit trail?

```rust
/// An integration test that verifies the full safety pipeline.
fn test_full_safety_pipeline() {
    println!("=== Integration Test: Full Safety Pipeline ===\n");

    // Step 1: Permission check
    let perm_result = check_permission("shell", "cargo test");
    println!("1. Permission check: {:?}", perm_result);
    assert!(perm_result);

    // Step 2: Denylist check (should pass for cargo test)
    let deny_result = check_denylist("cargo test");
    println!("2. Denylist check: {:?}", deny_result);
    assert!(deny_result);

    // Step 3: Rate limit check
    let rate_result = check_rate_limit("shell");
    println!("3. Rate limit check: {:?}", rate_result);
    assert!(rate_result);

    // Step 4: Approval check (cargo test is auto-approved)
    let approval_result = check_approval("shell", "cargo test");
    println!("4. Approval check: {:?}", approval_result);
    assert!(approval_result);

    // Step 5: Audit logging
    let audit_result = log_audit_event("shell", "cargo test", "approved");
    println!("5. Audit logged: {:?}", audit_result);
    assert!(audit_result);

    println!("\nAll pipeline checks passed!");

    // Now test that a dangerous command is caught at step 2
    println!("\n--- Testing dangerous command ---");
    let perm_ok = check_permission("shell", "rm -rf /");
    let deny_ok = check_denylist("rm -rf /");
    println!("Permission: {}, Denylist: {}", perm_ok, deny_ok);
    assert!(!deny_ok, "Dangerous command should be denied");
    println!("Correctly blocked at denylist stage!");
}

fn check_permission(tool: &str, _command: &str) -> bool {
    // Simplified: shell tool is allowed
    tool == "shell" || tool == "file_read" || tool == "file_write"
}

fn check_denylist(command: &str) -> bool {
    let denied = ["rm -rf", "sudo", "chmod 777"];
    let lower = command.to_lowercase();
    !denied.iter().any(|d| lower.contains(d))
}

fn check_rate_limit(_tool: &str) -> bool {
    true // Simplified: always within limits
}

fn check_approval(_tool: &str, command: &str) -> bool {
    let auto_approved = ["cargo", "git status", "git diff", "ls"];
    auto_approved.iter().any(|a| command.starts_with(a))
}

fn log_audit_event(_tool: &str, _command: &str, _outcome: &str) -> bool {
    true // Simplified: always succeeds
}

fn main() {
    test_full_safety_pipeline();
}
```

## Key Takeaways

- Safety testing requires an adversarial mindset: write tests that actively try to bypass your security mechanisms, not just tests that verify normal operation works
- Property-based testing with randomized inputs discovers edge cases that hand-written tests miss -- especially important for path traversal and command injection filters
- Every discovered bypass should become a permanent regression test, building a growing catalog of attack patterns that can never regress
- Integration tests must verify that all safety layers work together correctly -- a permission check passing does not mean the operation should proceed if the rate limiter or denylist blocks it
- Command normalization (removing backslashes, quotes, extra spaces) must happen before denylist matching, or evasion through formatting is trivial
