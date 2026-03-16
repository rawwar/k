// Chapter 12: Permission and Safety — Code snapshot

use std::path::Path;

/// Permission level for tool operations.
#[derive(Debug, Clone, PartialEq)]
enum Permission {
    /// Always allowed without asking.
    Allow,
    /// Requires user confirmation.
    AskUser,
    /// Always denied.
    Deny,
}

/// Check if a tool invocation is permitted.
fn check_permission(tool_name: &str, args: &serde_json::Value) -> Permission {
    // TODO: Implement permission rules based on tool and arguments
    // TODO: Read-only tools (grep, glob, read) are always allowed
    // TODO: Write tools (edit, write) need confirmation
    // TODO: Shell execution needs confirmation with command preview
    // TODO: Destructive git operations should be denied by default
    let _ = (tool_name, args);
    Permission::AskUser
}

/// Check if a file path is within the allowed workspace.
fn is_path_allowed(path: &str) -> bool {
    // TODO: Implement workspace boundary checks
    // TODO: Prevent access to sensitive files (.env, credentials)
    let _ = Path::new(path);
    true
}

fn main() {
    println!("Chapter 12: Permission and Safety");

    let perm = check_permission("bash", &serde_json::json!({"command": "rm -rf /"}));
    println!("Permission for dangerous command: {perm:?}");

    let allowed = is_path_allowed("/etc/passwd");
    println!("Path allowed: {allowed}");

    println!("TODO: Implement full permission and safety system");
}
