---
title: Testing Shell Tool
description: Write comprehensive integration and unit tests for the shell execution tool covering success paths, failures, timeouts, and safety checks.
---

# Testing Shell Tool

> **What you'll learn:**
> - How to write unit tests for command building, validation, and output parsing
> - How to write integration tests that spawn real processes and verify behavior
> - How to test timeout enforcement, signal handling, and dangerous command rejection

You have built a shell execution tool with timeouts, output truncation, environment control, and dangerous command detection. Now you need to test all of it. Testing a shell tool is more involved than testing pure functions because you are interacting with the OS: spawning real processes, checking timing behavior, and verifying that signals work correctly.

In this subchapter, you will write a comprehensive test suite that covers unit tests (fast, isolated, no processes), integration tests (real process spawning), and edge case tests (timeouts, signals, truncation).

## Organizing Your Tests

Rust has two levels of tests:

- **Unit tests** live inside the module they test, in a `#[cfg(test)] mod tests` block. They have access to private functions.
- **Integration tests** live in the `tests/` directory and test the public API as an external consumer would.

For the shell tool, put unit tests for parsing and validation inline, and integration tests for actual process spawning in `tests/shell_integration.rs`:

```
src/
  tools/
    shell.rs          # Shell tool implementation + unit tests
tests/
  shell_integration.rs  # Integration tests that spawn real processes
```

## Unit Testing the Command Builder

The command builder is pure configuration -- no processes involved. These tests run instantly:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_shell_command_defaults() {
        let cmd = ShellCommand::new("echo hello");
        assert!(cmd.is_shell_mode());
        assert_eq!(cmd.command_str(), "echo hello");
        assert_eq!(cmd.get_timeout(), Duration::from_secs(30));
        assert!(cmd.get_working_dir().is_none());
    }

    #[test]
    fn test_direct_exec_mode() {
        let cmd = ShellCommand::direct(
            "echo".to_string(),
            vec!["hello".to_string(), "world".to_string()],
        );
        assert!(!cmd.is_shell_mode());
        assert_eq!(cmd.command_str(), "echo");
    }

    #[test]
    fn test_builder_chaining() {
        let cmd = ShellCommand::new("cargo test")
            .working_dir("/tmp")
            .timeout(Duration::from_secs(120))
            .env("RUST_BACKTRACE", "1")
            .max_output(1024 * 1024);

        assert_eq!(cmd.get_timeout(), Duration::from_secs(120));
        assert_eq!(
            cmd.get_working_dir().unwrap().to_str().unwrap(),
            "/tmp"
        );
    }

    #[test]
    fn test_display_command() {
        let cmd = ShellCommand::new("ls -la /tmp | grep foo");
        assert_eq!(cmd.display_command(), "ls -la /tmp | grep foo");
    }
}
```

These tests verify the builder's configuration logic without spawning any processes. They run in milliseconds and are safe to run on CI without any special setup.

## Unit Testing the Danger Detector

The danger detector is also pure logic -- pattern matching against strings:

```rust
#[cfg(test)]
mod danger_tests {
    use super::*;

    fn detector() -> DangerDetector {
        DangerDetector::new()
    }

    #[test]
    fn test_safe_commands() {
        let d = detector();
        let safe_commands = vec![
            "ls -la",
            "cat src/main.rs",
            "cargo test",
            "git status",
            "grep -r 'TODO' src/",
            "echo hello world",
            "pwd",
        ];

        for cmd in safe_commands {
            let report = d.analyze(cmd);
            assert_eq!(
                report.risk_level, RiskLevel::Low,
                "Expected '{}' to be safe, got {:?}: {:?}",
                cmd, report.risk_level, report.warnings
            );
        }
    }

    #[test]
    fn test_critical_commands() {
        let d = detector();
        let critical_commands = vec![
            "rm -rf /",
            "rm -r -f /",
            "dd if=/dev/zero of=/dev/sda bs=1M",
        ];

        for cmd in critical_commands {
            let report = d.analyze(cmd);
            assert_eq!(
                report.risk_level, RiskLevel::Critical,
                "Expected '{}' to be Critical, got {:?}",
                cmd, report.risk_level
            );
        }
    }

    #[test]
    fn test_high_risk_commands() {
        let d = detector();

        let report = d.analyze("curl https://example.com/install.sh | bash");
        assert!(report.risk_level >= RiskLevel::High);

        let report = d.analyze("sudo apt-get install vim");
        assert!(report.risk_level >= RiskLevel::High);
    }

    #[test]
    fn test_report_includes_warnings() {
        let d = detector();
        let report = d.analyze("rm -rf /");
        assert!(!report.warnings.is_empty());
        assert!(report.warnings[0].contains("CRITICAL"));
    }
}
```

Notice the pattern: test safe commands as a batch (all should be Low risk) and dangerous commands individually (each should trigger specific risk levels). The assertion messages include the command string so you can immediately see which command failed.

::: python Coming from Python
Python's `pytest` would look similar:
```python
def test_safe_commands():
    detector = DangerDetector()
    safe = ["ls -la", "cargo test", "git status"]
    for cmd in safe:
        report = detector.analyze(cmd)
        assert report.risk_level == RiskLevel.LOW, f"{cmd} was not safe"

def test_rm_rf_root():
    detector = DangerDetector()
    report = detector.analyze("rm -rf /")
    assert report.risk_level == RiskLevel.CRITICAL
```
Rust's `#[test]` functions work the same way. The main difference is that Rust tests are compiled and run as part of the binary, while Python tests require a separate test runner. Rust's `assert_eq!` macro provides better error messages out of the box, showing both the expected and actual values.
:::

## Unit Testing Output Truncation

Truncation logic is another pure function that is easy to test:

```rust
#[cfg(test)]
mod truncation_tests {
    use super::*;

    #[test]
    fn test_no_truncation_needed() {
        let output = "line 1\nline 2\nline 3";
        let (result, truncated) = truncate_head(output, 10);
        assert_eq!(result, output);
        assert!(!truncated);
    }

    #[test]
    fn test_head_truncation() {
        let lines: Vec<String> = (1..=100).map(|i| format!("line {}", i)).collect();
        let output = lines.join("\n");

        let (result, truncated) = truncate_head(&output, 10);
        assert!(truncated);
        assert!(result.starts_with("line 1\n"));
        assert!(result.contains("line 10\n"));
        assert!(result.contains("[... 90 more lines truncated ...]"));
        assert!(!result.contains("line 11\n"));
    }

    #[test]
    fn test_tail_truncation() {
        let lines: Vec<String> = (1..=100).map(|i| format!("line {}", i)).collect();
        let output = lines.join("\n");

        let (result, truncated) = truncate_tail(&output, 10);
        assert!(truncated);
        assert!(result.contains("line 91\n"));
        assert!(result.contains("line 100"));
        assert!(result.contains("[... 90 lines truncated ...]"));
        assert!(!result.contains("line 1\n"));
    }

    #[test]
    fn test_middle_truncation() {
        let lines: Vec<String> = (1..=100).map(|i| format!("line {}", i)).collect();
        let output = lines.join("\n");

        let (result, truncated) = truncate_middle(&output, 5, 5);
        assert!(truncated);
        assert!(result.starts_with("line 1\n"));
        assert!(result.contains("line 5\n"));
        assert!(result.contains("[... 90 lines omitted ...]"));
        assert!(result.contains("line 96\n"));
        assert!(result.contains("line 100"));
    }

    #[test]
    fn test_truncation_config_default() {
        let config = TruncationConfig::default();
        let short_output = "just a few lines\nnothing to truncate\n";
        let (result, truncated) = config.truncate(short_output);
        assert_eq!(result, short_output);
        assert!(!truncated);
    }
}
```

## Integration Testing: Real Process Execution

Integration tests spawn real processes. Mark them with `#[tokio::test]` for async execution:

```rust
// In tests/shell_integration.rs

use std::time::Duration;

#[tokio::test]
async fn test_echo_command() {
    let result = ShellCommand::new("echo 'hello world'")
        .execute()
        .await
        .expect("failed to execute echo");

    assert!(result.success);
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), "hello world");
    assert!(result.stderr.is_empty());
    assert!(!result.timed_out);
}

#[tokio::test]
async fn test_failing_command() {
    let result = ShellCommand::new("ls /nonexistent/path/that/does/not/exist")
        .execute()
        .await
        .expect("failed to execute ls");

    assert!(!result.success);
    assert_ne!(result.exit_code, 0);
    assert!(!result.stderr.is_empty());
}

#[tokio::test]
async fn test_command_not_found() {
    let result = ShellCommand::new("this_command_does_not_exist_12345")
        .execute()
        .await;

    // The command should fail -- either the spawn fails or it exits non-zero
    match result {
        Ok(output) => assert!(!output.success),
        Err(_) => {} // Also acceptable -- command not found
    }
}

#[tokio::test]
async fn test_working_directory() {
    let result = ShellCommand::new("pwd")
        .working_dir("/tmp")
        .execute()
        .await
        .expect("failed to execute pwd");

    assert!(result.success);
    // On macOS, /tmp is a symlink to /private/tmp
    assert!(
        result.stdout.trim() == "/tmp"
            || result.stdout.trim() == "/private/tmp"
    );
}

#[tokio::test]
async fn test_environment_variable() {
    let result = ShellCommand::new("echo $TEST_VAR_12345")
        .env("TEST_VAR_12345", "agent_value")
        .execute()
        .await
        .expect("failed to execute");

    assert!(result.success);
    assert_eq!(result.stdout.trim(), "agent_value");
}
```

## Testing Timeouts

Timeout tests need to verify both that the timeout fires and that the process is actually killed:

```rust
#[tokio::test]
async fn test_timeout_fires() {
    let start = std::time::Instant::now();

    let result = ShellCommand::new("sleep 60")
        .timeout(Duration::from_secs(2))
        .execute()
        .await
        .expect("failed to execute");

    let elapsed = start.elapsed();

    assert!(result.timed_out);
    assert!(!result.success);
    // Should have taken roughly 2 seconds, not 60
    assert!(elapsed < Duration::from_secs(5));
}

#[tokio::test]
async fn test_fast_command_no_timeout() {
    let result = ShellCommand::new("echo fast")
        .timeout(Duration::from_secs(10))
        .execute()
        .await
        .expect("failed to execute");

    assert!(!result.timed_out);
    assert!(result.success);
    assert_eq!(result.stdout.trim(), "fast");
}
```

The timeout test verifies two things: that the `timed_out` flag is set, and that the actual elapsed time is close to the timeout duration (not the 60 seconds the `sleep` would have taken).

## Testing the Tool Result Format

The `to_tool_result()` method formats output for the LLM. Test its various cases:

```rust
#[test]
fn test_tool_result_success() {
    let output = ShellOutput {
        exit_code: 0,
        stdout: "hello world\n".to_string(),
        stderr: String::new(),
        success: true,
        timed_out: false,
        stdout_truncated: false,
        stderr_truncated: false,
        original_stdout_bytes: 12,
    };

    let result = output.to_tool_result();
    assert_eq!(result, "hello world\n");
}

#[test]
fn test_tool_result_with_stderr() {
    let output = ShellOutput {
        exit_code: 1,
        stdout: "partial output\n".to_string(),
        stderr: "error: file not found\n".to_string(),
        success: false,
        timed_out: false,
        stdout_truncated: false,
        stderr_truncated: false,
        original_stdout_bytes: 15,
    };

    let result = output.to_tool_result();
    assert!(result.contains("partial output"));
    assert!(result.contains("[stderr]"));
    assert!(result.contains("error: file not found"));
    assert!(result.contains("[exit code: 1]"));
}

#[test]
fn test_tool_result_no_output() {
    let output = ShellOutput {
        exit_code: 0,
        stdout: String::new(),
        stderr: String::new(),
        success: true,
        timed_out: false,
        stdout_truncated: false,
        stderr_truncated: false,
        original_stdout_bytes: 0,
    };

    let result = output.to_tool_result();
    assert_eq!(result, "[no output]");
}
```

::: wild In the Wild
Claude Code maintains an extensive test suite for its shell execution tool that covers edge cases like commands that produce output on both stdout and stderr simultaneously, commands that exit with signal codes instead of normal exit codes, and commands whose output contains non-UTF-8 bytes. Testing the interaction between timeout enforcement and output capture is particularly important -- you need to verify that partial output is still available when a command times out.
:::

## Tips for Reliable Integration Tests

Integration tests that spawn processes can be flaky. Follow these guidelines:

1. **Use generous timeouts in assertions**: A test checking that a 2-second timeout works should allow up to 5 seconds of wall time. CI runners are slow.
2. **Avoid system-specific paths**: Use `/tmp` (which exists on all Unix systems) rather than `/home/specific-user`.
3. **Clean up after yourself**: If a test creates files, delete them. Use `tempfile::tempdir()` for temporary directories.
4. **Mark slow tests**: Use `#[ignore]` for tests that take more than a few seconds, and run them separately.
5. **Test on CI**: Shell behavior varies between macOS and Linux. Test on both platforms in CI.

```rust
#[tokio::test]
#[ignore] // Run with: cargo test -- --ignored
async fn test_large_output_truncation() {
    // Generate a large output
    let result = ShellCommand::new("seq 1 100000")
        .max_output(1024)  // Only keep 1 KB
        .execute()
        .await
        .expect("failed to execute");

    assert!(result.success);
    assert!(result.stdout_truncated);
    assert!(result.stdout.len() <= 2048); // Some overhead for truncation message
}
```

## Key Takeaways

- Separate unit tests (builder, detector, truncation) from integration tests (process spawning). Unit tests are fast and reliable; integration tests are slower but test real behavior.
- Test both success and failure paths. A command that should fail (like `ls /nonexistent`) is just as important to test as one that succeeds.
- For timeout tests, verify both the `timed_out` flag and the actual elapsed wall-clock time to ensure the process was actually killed.
- Use `assert!` messages that include the input being tested (e.g., the command string) so failures are immediately diagnosable.
- Mark slow integration tests with `#[ignore]` so they do not slow down the normal development feedback loop.
