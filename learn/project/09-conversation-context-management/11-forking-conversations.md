---
title: Forking Conversations
description: Support conversation branching so users can explore alternative approaches without losing their original conversation thread.
---

# Forking Conversations

> **What you'll learn:**
> - How to implement a tree-structured conversation history that supports branching points
> - How to let users fork from any point in the conversation to explore an alternative direction
> - How to switch between conversation branches and optionally merge insights back

Sometimes a user wants to try two different approaches to the same problem. "Let's try refactoring this with traits... actually, wait, let me also try the enum-based approach." Without forking, the user has to pick one direction and lose the other. With forking, they can branch the conversation at the decision point, explore both paths, and then continue with whichever one worked better.

## The Branching Model

A forked conversation is a tree rather than a linear list. Each fork point creates a new branch that shares all the history before the fork but has independent history after it:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Identifier for a branch within a conversation tree.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BranchId(pub String);

impl BranchId {
    pub fn main() -> Self {
        Self("main".to_string())
    }

    pub fn new(name: &str) -> Self {
        Self(name.to_string())
    }
}

/// A single branch in the conversation tree.
#[derive(Debug, Serialize, Deserialize)]
pub struct Branch {
    pub id: BranchId,
    /// Human-readable description of what this branch explores
    pub description: String,
    /// The branch this was forked from
    pub parent: Option<BranchId>,
    /// Index in the parent branch where this fork occurred
    pub fork_point: usize,
    /// Messages that are unique to this branch (after the fork point)
    pub messages: Vec<BranchMessage>,
    /// When this branch was created
    pub created_at: std::time::SystemTime,
}

/// A message within a branch (simplified for the branching example).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMessage {
    pub id: u64,
    pub role: String,
    pub content: String,
    pub token_count: usize,
}

/// A tree-structured conversation that supports branching.
#[derive(Debug, Serialize, Deserialize)]
pub struct ConversationTree {
    /// All branches indexed by their ID
    branches: HashMap<BranchId, Branch>,
    /// Which branch is currently active
    active_branch: BranchId,
    /// Next message ID counter
    next_id: u64,
}

impl ConversationTree {
    /// Create a new conversation tree with a single main branch.
    pub fn new() -> Self {
        let main_branch = Branch {
            id: BranchId::main(),
            description: "Main conversation".to_string(),
            parent: None,
            fork_point: 0,
            messages: Vec::new(),
            created_at: std::time::SystemTime::now(),
        };

        let mut branches = HashMap::new();
        branches.insert(BranchId::main(), main_branch);

        Self {
            branches,
            active_branch: BranchId::main(),
            next_id: 1,
        }
    }

    /// Add a message to the currently active branch.
    pub fn push(&mut self, role: String, content: String, token_count: usize) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let msg = BranchMessage { id, role, content, token_count };
        self.branches.get_mut(&self.active_branch)
            .expect("active branch must exist")
            .messages.push(msg);

        id
    }

    /// Fork a new branch from the current position in the active branch.
    pub fn fork(&mut self, name: &str, description: &str) -> BranchId {
        let branch_id = BranchId::new(name);
        let fork_point = self.branches[&self.active_branch].messages.len();

        let new_branch = Branch {
            id: branch_id.clone(),
            description: description.to_string(),
            parent: Some(self.active_branch.clone()),
            fork_point,
            messages: Vec::new(),
            created_at: std::time::SystemTime::now(),
        };

        self.branches.insert(branch_id.clone(), new_branch);
        self.active_branch = branch_id.clone();
        branch_id
    }

    /// Fork from a specific message index in the active branch.
    pub fn fork_at(&mut self, name: &str, description: &str, at_index: usize) -> BranchId {
        let branch_id = BranchId::new(name);
        let current_branch = &self.branches[&self.active_branch];

        // Validate the fork point
        let fork_point = at_index.min(current_branch.messages.len());

        let new_branch = Branch {
            id: branch_id.clone(),
            description: description.to_string(),
            parent: Some(self.active_branch.clone()),
            fork_point,
            messages: Vec::new(),
            created_at: std::time::SystemTime::now(),
        };

        self.branches.insert(branch_id.clone(), new_branch);
        self.active_branch = branch_id.clone();
        branch_id
    }

    /// Switch to a different branch.
    pub fn switch_to(&mut self, branch_id: &BranchId) -> Result<(), String> {
        if self.branches.contains_key(branch_id) {
            self.active_branch = branch_id.clone();
            Ok(())
        } else {
            Err(format!("Branch {:?} does not exist", branch_id.0))
        }
    }

    /// Get the full message history for the active branch,
    /// including all ancestor messages up to the root.
    pub fn active_messages(&self) -> Vec<&BranchMessage> {
        self.branch_messages(&self.active_branch)
    }

    /// Get the full message history for a given branch.
    fn branch_messages(&self, branch_id: &BranchId) -> Vec<&BranchMessage> {
        let branch = &self.branches[branch_id];

        // Recursively collect ancestor messages
        let mut messages = if let Some(ref parent_id) = branch.parent {
            let parent_msgs = self.branch_messages(parent_id);
            // Only take messages up to the fork point
            parent_msgs.into_iter().take(branch.fork_point).collect()
        } else {
            Vec::new()
        };

        // Append this branch's own messages
        messages.extend(branch.messages.iter());
        messages
    }

    /// Get the total token count for the active branch.
    pub fn active_tokens(&self) -> usize {
        self.active_messages().iter().map(|m| m.token_count).sum()
    }

    /// List all branches with their metadata.
    pub fn list_branches(&self) -> Vec<BranchSummary> {
        self.branches.values().map(|b| {
            let total_messages = self.branch_messages(&b.id).len();
            BranchSummary {
                id: b.id.clone(),
                description: b.description.clone(),
                is_active: b.id == self.active_branch,
                own_messages: b.messages.len(),
                total_messages,
                parent: b.parent.clone(),
            }
        }).collect()
    }
}

/// Summary of a branch for display purposes.
#[derive(Debug)]
pub struct BranchSummary {
    pub id: BranchId,
    pub description: String,
    pub is_active: bool,
    pub own_messages: usize,
    pub total_messages: usize,
    pub parent: Option<BranchId>,
}

fn main() {
    let mut tree = ConversationTree::new();

    // Build up some conversation on main
    tree.push("user".into(), "I need to refactor the auth module".into(), 10);
    tree.push("assistant".into(), "I see two approaches: traits or enums".into(), 15);

    println!("=== Main branch: {} messages ===", tree.active_messages().len());

    // Fork to try the trait approach
    let trait_branch = tree.fork("trait-approach", "Refactor auth using traits");
    tree.push("user".into(), "Let's try the trait approach".into(), 8);
    tree.push("assistant".into(), "Here's a trait-based auth design...".into(), 50);
    tree.push("user".into(), "That looks complex, what about enums?".into(), 10);

    println!("=== Trait branch: {} messages ===", tree.active_messages().len());

    // Switch back to main and fork for the enum approach
    tree.switch_to(&BranchId::main()).unwrap();
    let enum_branch = tree.fork("enum-approach", "Refactor auth using enums");
    tree.push("user".into(), "Let's try the enum approach".into(), 8);
    tree.push("assistant".into(), "Here's an enum-based auth design...".into(), 40);

    println!("=== Enum branch: {} messages ===", tree.active_messages().len());

    // List all branches
    println!("\nAll branches:");
    for b in tree.list_branches() {
        println!("  {} {} - \"{}\" ({} own msgs, {} total)",
            if b.is_active { "*" } else { " " },
            b.id.0,
            b.description,
            b.own_messages,
            b.total_messages);
    }

    // Show messages on the active branch
    println!("\nActive branch messages:");
    for msg in tree.active_messages() {
        println!("  [{}] {}: {}", msg.id, msg.role,
            &msg.content[..50.min(msg.content.len())]);
    }
}
```

::: python Coming from Python
Python's list-based conversation history makes forking awkward:
```python
# Naive forking by copying the list
fork = messages[:fork_point].copy()
fork.append(new_message)
```
This copies all the shared messages, doubling memory usage. Our Rust tree
structure avoids this by storing only the unique messages per branch and
reconstructing the full history by walking parent pointers. Rust's borrow
checker ensures you cannot accidentally mutate a shared parent branch while
reading from a child.
:::

## User Interface for Forking

Your TUI from Chapter 8 needs commands for fork management. Here is how the command handling integrates:

```rust
/// Commands for managing conversation branches.
pub enum BranchCommand {
    /// Create a new fork from the current point
    Fork { name: String, description: String },
    /// Create a fork from a specific message
    ForkAt { name: String, description: String, message_index: usize },
    /// Switch to a different branch
    Switch { name: String },
    /// List all branches
    List,
    /// Show the current branch's history
    Show,
}

/// Parse a branch command from user input.
pub fn parse_branch_command(input: &str) -> Option<BranchCommand> {
    let parts: Vec<&str> = input.splitn(3, ' ').collect();

    match parts.get(0).map(|s| *s) {
        Some("/fork") => {
            let name = parts.get(1).unwrap_or(&"branch").to_string();
            let description = parts.get(2).unwrap_or(&"New branch").to_string();
            Some(BranchCommand::Fork { name, description })
        }
        Some("/switch") => {
            let name = parts.get(1)?.to_string();
            Some(BranchCommand::Switch { name })
        }
        Some("/branches") => Some(BranchCommand::List),
        _ => None,
    }
}

/// Handle a branch command and return a user-facing message.
pub fn handle_branch_command(
    tree: &mut ConversationTree,
    cmd: BranchCommand,
) -> String {
    match cmd {
        BranchCommand::Fork { name, description } => {
            let id = tree.fork(&name, &description);
            format!("Created and switched to branch '{}': {}", id.0, description)
        }
        BranchCommand::ForkAt { name, description, message_index } => {
            let id = tree.fork_at(&name, &description, message_index);
            format!("Forked from message {} to branch '{}'", message_index, id.0)
        }
        BranchCommand::Switch { name } => {
            match tree.switch_to(&BranchId::new(&name)) {
                Ok(()) => format!("Switched to branch '{}' ({} messages)",
                    name, tree.active_messages().len()),
                Err(e) => format!("Error: {}", e),
            }
        }
        BranchCommand::List => {
            let branches = tree.list_branches();
            let mut output = String::from("Branches:\n");
            for b in &branches {
                output.push_str(&format!(
                    "  {} {} - {} ({} msgs)\n",
                    if b.is_active { "*" } else { " " },
                    b.id.0,
                    b.description,
                    b.total_messages,
                ));
            }
            output
        }
        BranchCommand::Show => {
            let msgs = tree.active_messages();
            let mut output = format!("Branch history ({} messages):\n", msgs.len());
            for msg in &msgs {
                let preview = if msg.content.len() > 60 {
                    format!("{}...", &msg.content[..57])
                } else {
                    msg.content.clone()
                };
                output.push_str(&format!("  [{}] {}: {}\n", msg.id, msg.role, preview));
            }
            output
        }
    }
}

fn main() {
    let mut tree = ConversationTree::new();
    tree.push("user".into(), "Help me refactor".into(), 5);
    tree.push("assistant".into(), "I see two approaches".into(), 10);

    // Simulate user commands
    let commands = [
        "/fork traits Try trait-based approach",
        "/branches",
        "/switch main",
        "/fork enums Try enum-based approach",
        "/branches",
    ];

    for input in &commands {
        println!("> {}", input);
        if let Some(cmd) = parse_branch_command(input) {
            let result = handle_branch_command(&mut tree, cmd);
            println!("{}\n", result);
        }
    }
}
```

::: wild In the Wild
Claude Code does not expose conversation forking as a user-facing feature, but internally it maintains the concept of "conversation continuations" where the agent can retry a failed approach from a specific point in the conversation. OpenCode supports a more explicit branching model where users can navigate between different conversation paths. The underlying data structure in both cases is tree-shaped rather than linear.
:::

## Key Takeaways

- Model conversations as a tree rather than a list -- each branch shares ancestor messages and stores only its own unique additions
- Walk parent pointers to reconstruct the full history for any branch, avoiding duplicate storage of shared messages
- Expose forking through simple commands (`/fork`, `/switch`, `/branches`) that integrate with your existing TUI
- Track fork points by index so branches always reference a specific moment in the parent conversation
- Keep the `ConversationTree` serializable (derive `Serialize`/`Deserialize`) so forks persist across sessions
