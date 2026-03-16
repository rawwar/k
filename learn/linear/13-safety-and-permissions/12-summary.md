---
title: Summary
description: Consolidate the safety and permissions concepts into a unified defense-in-depth strategy for production coding agents.
---

# Summary

> **What you'll learn:**
> - How all the safety layers -- permissions, approvals, checkpoints, sandboxing, and auditing -- compose into a defense-in-depth architecture
> - A checklist of safety properties every production coding agent should satisfy before deployment
> - How to evaluate and iterate on safety systems as agent capabilities and threat landscapes evolve

Throughout this chapter, you have built eleven distinct safety mechanisms. Each one addresses specific threats, but none of them is sufficient alone. The real power comes from how they compose into a unified defense-in-depth strategy. Let's pull everything together and see how these layers interact to protect users, codebases, and systems.

## The Defense-in-Depth Stack

Every tool invocation in your coding agent passes through multiple safety layers before it reaches execution. Here is the complete pipeline, ordered from outermost to innermost:

```rust
/// The complete safety pipeline that every tool invocation passes through.
/// Each layer can block the operation, and the operation must pass ALL layers.
fn safety_pipeline(
    tool_name: &str,
    command: Option<&str>,
    path: Option<&str>,
) -> Result<String, String> {

    // Layer 1: Plan Mode Check
    // If in plan mode, record the action and stop here
    if is_plan_mode() {
        return Ok(format!("[PLANNED] {} - recorded, not executed", tool_name));
    }

    // Layer 2: Permission Architecture
    // Does the agent have the capability to use this tool at all?
    check_permissions(tool_name, path)?;

    // Layer 3: Allowlist/Denylist
    // Is this specific command/path allowed by the filter rules?
    if let Some(cmd) = command {
        check_allowlist_denylist(cmd)?;
    }
    if let Some(p) = path {
        check_path_boundaries(p)?;
    }

    // Layer 4: Rate Limiting
    // Has the agent exceeded its action budget?
    check_rate_limits(tool_name)?;

    // Layer 5: Approval Flow
    // Does this action require human approval?
    check_approval(tool_name, command, path)?;

    // Layer 6: Checkpoint
    // Create a checkpoint before executing (for mutating operations)
    if is_mutating(tool_name) {
        create_checkpoint(tool_name)?;
    }

    // Layer 7: Sandboxed Execution
    // Execute the action within the sandbox
    let result = execute_in_sandbox(tool_name, command, path)?;

    // Layer 8: Audit Trail
    // Record what happened (always, regardless of outcome)
    record_audit_event(tool_name, command, path, &result);

    Ok(result)
}

// Simplified implementations for illustration
fn is_plan_mode() -> bool { false }
fn check_permissions(_tool: &str, _path: Option<&str>) -> Result<(), String> { Ok(()) }
fn check_allowlist_denylist(cmd: &str) -> Result<(), String> {
    if cmd.contains("rm -rf") { Err("Blocked by denylist".into()) } else { Ok(()) }
}
fn check_path_boundaries(path: &str) -> Result<(), String> {
    if path.contains("..") { Err("Path traversal detected".into()) } else { Ok(()) }
}
fn check_rate_limits(_tool: &str) -> Result<(), String> { Ok(()) }
fn check_approval(_tool: &str, _cmd: Option<&str>, _path: Option<&str>) -> Result<(), String> { Ok(()) }
fn is_mutating(tool: &str) -> bool {
    matches!(tool, "file_write" | "shell" | "file_delete")
}
fn create_checkpoint(_tool: &str) -> Result<(), String> { Ok(()) }
fn execute_in_sandbox(_tool: &str, _cmd: Option<&str>, _path: Option<&str>) -> Result<String, String> {
    Ok("executed successfully".into())
}
fn record_audit_event(_tool: &str, _cmd: Option<&str>, _path: Option<&str>, _result: &str) {}

fn main() {
    // Safe operation -- passes all layers
    match safety_pipeline("shell", Some("cargo test"), None) {
        Ok(result) => println!("SUCCESS: {}", result),
        Err(reason) => println!("BLOCKED: {}", reason),
    }

    // Dangerous operation -- blocked at denylist layer
    match safety_pipeline("shell", Some("rm -rf /"), None) {
        Ok(result) => println!("SUCCESS: {}", result),
        Err(reason) => println!("BLOCKED: {}", reason),
    }

    // Path traversal -- blocked at path boundary layer
    match safety_pipeline("file_read", None, Some("../../etc/passwd")) {
        Ok(result) => println!("SUCCESS: {}", result),
        Err(reason) => println!("BLOCKED: {}", reason),
    }
}
```

The critical insight is that each layer is independent. If the denylist fails to catch a dangerous command (maybe through an evasion technique you did not anticipate), the sandbox still prevents the damage. If the sandbox has a misconfiguration, the audit trail records what happened so you can investigate and fix it. No single layer needs to be perfect because the others compensate for its weaknesses.

## Chapter Concepts in Review

Let's walk through each safety mechanism and how it fits into the larger picture:

**Threat Modeling** (Subchapter 1) gave you a systematic way to identify what can go wrong. The three threat categories -- malicious prompts, accidental damage, and data exfiltration -- informed every design decision that followed.

**Permission Architectures** (Subchapter 2) established capability-based access control. By defining exactly what the agent can do (default-deny), you ensured that unknown operations are automatically blocked.

**Approval Flows** (Subchapter 3) added human judgment to the pipeline. Risk classification routes each operation to the appropriate level of scrutiny: auto-approve for safe operations, session-approve for moderate risk, and always-approve for high-risk actions.

**Checkpoint Systems** (Subchapter 4) captured codebase state at strategic points using git commits and stashes, creating recovery points that the rollback mechanism can target.

**Rollback Mechanisms** (Subchapter 5) built on checkpoints to provide undo at multiple granularities -- individual tool calls, entire turns, or complete sessions. They also honestly communicate what cannot be undone (irreversible side effects).

**Allowlist/Denylist Design** (Subchapter 6) implemented the filtering rules that decide which specific commands, paths, and operations are in-bounds. You learned to handle evasion techniques like shell escaping, command substitution, and path traversal.

**Sandboxing** (Subchapter 7) added OS-level enforcement that cannot be bypassed by creative command construction. Whether using macOS sandbox-exec, Linux namespaces, or containers, the operating system itself enforces the boundaries.

**Plan Mode** (Subchapter 8) gave users complete visibility by letting the agent propose actions without executing them. The dual-mode architecture ensures the agent reasons the same way in both modes.

**Audit Trails** (Subchapter 9) recorded every significant event with structured metadata and correlation contexts, enabling post-incident analysis and compliance verification.

**Rate Limiting** (Subchapter 10) prevented runaway agents with token bucket limiters and circuit breakers that detect repetitive failure loops.

**Testing Safety Systems** (Subchapter 11) verified that all of these mechanisms actually work under adversarial conditions, using property-based testing, regression suites, and integration tests.

## Production Safety Checklist

Before deploying a coding agent (even for your own use), verify each of these properties:

```rust
/// Safety properties every production coding agent should satisfy.
struct SafetyChecklist {
    items: Vec<ChecklistItem>,
}

struct ChecklistItem {
    category: String,
    requirement: String,
    verified: bool,
}

impl SafetyChecklist {
    fn production_checklist() -> Self {
        Self {
            items: vec![
                ChecklistItem {
                    category: "Permissions".into(),
                    requirement: "Default-deny policy: agent cannot access resources not explicitly granted".into(),
                    verified: false,
                },
                ChecklistItem {
                    category: "Permissions".into(),
                    requirement: "File access is scoped to the project directory with path traversal protection".into(),
                    verified: false,
                },
                ChecklistItem {
                    category: "Approval".into(),
                    requirement: "Destructive operations require explicit user approval every time".into(),
                    verified: false,
                },
                ChecklistItem {
                    category: "Approval".into(),
                    requirement: "Irreversible operations (publish, push) are clearly flagged".into(),
                    verified: false,
                },
                ChecklistItem {
                    category: "Containment".into(),
                    requirement: "Shell commands are validated against denylist before execution".into(),
                    verified: false,
                },
                ChecklistItem {
                    category: "Containment".into(),
                    requirement: "Command evasion techniques (escaping, quoting, substitution) are detected".into(),
                    verified: false,
                },
                ChecklistItem {
                    category: "Recovery".into(),
                    requirement: "Checkpoints are created before every mutating operation".into(),
                    verified: false,
                },
                ChecklistItem {
                    category: "Recovery".into(),
                    requirement: "Undo command can revert the last turn's changes".into(),
                    verified: false,
                },
                ChecklistItem {
                    category: "Monitoring".into(),
                    requirement: "All tool invocations are logged with structured audit events".into(),
                    verified: false,
                },
                ChecklistItem {
                    category: "Monitoring".into(),
                    requirement: "Rate limits prevent runaway loops (circuit breaker active)".into(),
                    verified: false,
                },
                ChecklistItem {
                    category: "Data Protection".into(),
                    requirement: "Sensitive files (.env, credentials) cannot be read or sent to the LLM".into(),
                    verified: false,
                },
                ChecklistItem {
                    category: "Testing".into(),
                    requirement: "Adversarial test suite covers known bypass techniques".into(),
                    verified: false,
                },
            ],
        }
    }

    fn display(&self) {
        println!("=== Production Safety Checklist ===\n");
        let mut current_category = String::new();

        for item in &self.items {
            if item.category != current_category {
                println!("\n  [{}]", item.category);
                current_category = item.category.clone();
            }

            let marker = if item.verified { "x" } else { " " };
            println!("    [{}] {}", marker, item.requirement);
        }

        let verified = self.items.iter().filter(|i| i.verified).count();
        println!("\n  Progress: {}/{} verified", verified, self.items.len());
    }
}

fn main() {
    let checklist = SafetyChecklist::production_checklist();
    checklist.display();
}
```

::: tip In the Wild
Claude Code satisfies these safety properties through a combination of its permission system, approval prompts, file scoping, and command filtering. It represents the current state of the art for local coding agent safety. Codex achieves many of these properties through environmental isolation (sandboxed containers with no network by default), trading some agent capability for stronger guarantees. Both approaches are valid -- the right choice depends on your use case and risk tolerance. The industry consensus is converging on defense-in-depth: multiple independent layers that each catch a subset of threats.
:::

## Evolving Your Safety Posture

Safety is not a one-time implementation. As your agent gains new capabilities, the threat model changes. Here is how to keep your safety systems current:

1. **Review the threat model quarterly.** New tools, new data sources, and new deployment contexts introduce new attack surfaces. Re-run the STRIDE analysis from Subchapter 1.

2. **Monitor audit trails for anomalies.** Look for patterns that suggest the agent is being misused or misbehaving -- unusual file access patterns, repeated denials, or high error rates.

3. **Expand the regression test suite.** Every time a user reports unexpected behavior, add a test case. Every time you read about a new prompt injection technique, add a test case.

4. **Measure false positive rates.** If your safety system blocks too many legitimate operations, users will be tempted to disable it. Track how often users override denials, and adjust rules to reduce friction while maintaining safety.

5. **Stay current with agent safety research.** The field of LLM safety is evolving rapidly. New attack techniques (and new defenses) emerge frequently. Follow the security research community and update your defenses accordingly.

::: python Coming from Python
Python developers transitioning to Rust for agent development gain a significant advantage: Rust's type system and ownership model make it structurally harder to introduce safety bugs. A `PermissionResult` enum forces exhaustive handling. A `&mut self` method prevents concurrent access without synchronization. A `PathBuf` prevents certain classes of injection. These compile-time guarantees do not replace runtime safety checks, but they provide a foundation that Python simply cannot match. The tradeoff is development speed -- Rust's compiler catches more bugs at build time, but you spend more time satisfying it.
:::

## What's Next

With your safety architecture in place, you have a coding agent that is safe to use in real development workflows. The next chapters build on this foundation:

- **Chapter 14** covers context management -- how to handle conversation history, token limits, and compaction strategies that keep the agent effective over long sessions.
- **Chapter 15** brings everything together into a production-ready agent with logging, configuration, and deployment considerations.

The safety mechanisms from this chapter will be referenced throughout the remaining chapters. Every new feature you add to the agent should be evaluated against the threat model, and every new tool should pass through the safety pipeline.

## Key Takeaways

- Defense in depth means no single safety layer needs to be perfect -- each layer compensates for weaknesses in the others, and an attacker must bypass ALL layers to cause harm
- The safety pipeline processes every tool invocation through plan mode, permissions, allowlist/denylist, rate limiting, approval, checkpointing, sandboxing, and audit logging in that order
- A production safety checklist covering permissions, approval, containment, recovery, monitoring, data protection, and testing provides a concrete verification framework
- Safety systems must evolve with the agent -- quarterly threat model reviews, growing regression test suites, and monitoring of false positive rates keep defenses current
- The best safety architecture balances security with usability -- overly restrictive systems get disabled by frustrated users, which is worse than a moderately permissive system that stays enabled
