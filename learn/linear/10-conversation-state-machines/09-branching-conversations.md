---
title: Branching Conversations
description: Implementing conversation branching that lets users explore alternative approaches, undo sequences of messages, and compare different solution paths.
---

# Branching Conversations

> **What you'll learn:**
> - How to implement a tree-structured conversation history that supports branching at any message point
> - Building undo/redo functionality that navigates the conversation tree by switching between branches
> - UI and data model considerations for displaying and selecting between alternative conversation branches

Linear conversation history -- one message after another in a single sequence -- is how most chat applications work. But coding agents face situations where linearity becomes limiting. The user asks the agent to try approach A, it doesn't work out, and they want to go back and try approach B instead. Without branching, the only option is to manually undo by sending corrective messages, polluting the context with failed attempts. With branching, you fork the conversation at the decision point and explore both paths independently.

## From Linear to Tree Structure

The shift from a linear message list to a conversation tree requires rethinking your data model. Instead of each message having a single predecessor, it has a parent, and each parent can have multiple children:

```rust
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
struct ConversationNode {
    message: Message,
    parent: Option<Uuid>,
    children: Vec<Uuid>,
}

struct ConversationTree {
    /// All nodes in the conversation, indexed by message ID
    nodes: HashMap<Uuid, ConversationNode>,
    /// The root node (system prompt)
    root: Uuid,
    /// The currently active leaf (where new messages will be appended)
    active_leaf: Uuid,
    /// Named branches for easy reference
    branch_names: HashMap<String, Uuid>,
}

impl ConversationTree {
    fn new(system_message: Message) -> Self {
        let root_id = system_message.id;
        let root_node = ConversationNode {
            message: system_message,
            parent: None,
            children: Vec::new(),
        };

        let mut nodes = HashMap::new();
        nodes.insert(root_id, root_node);

        Self {
            nodes,
            root: root_id,
            active_leaf: root_id,
            branch_names: HashMap::new(),
        }
    }

    /// Append a message to the active branch
    fn append(&mut self, message: Message) {
        let msg_id = message.id;
        let node = ConversationNode {
            message,
            parent: Some(self.active_leaf),
            children: Vec::new(),
        };

        // Add as child of current leaf
        if let Some(parent) = self.nodes.get_mut(&self.active_leaf) {
            parent.children.push(msg_id);
        }

        self.nodes.insert(msg_id, node);
        self.active_leaf = msg_id;
    }

    /// Create a branch at a specific message and switch to it
    fn branch_at(
        &mut self,
        branch_point: Uuid,
        branch_name: String,
    ) -> Result<(), BranchError> {
        if !self.nodes.contains_key(&branch_point) {
            return Err(BranchError::NodeNotFound(branch_point));
        }

        self.active_leaf = branch_point;
        self.branch_names.insert(branch_name, branch_point);
        Ok(())
    }

    /// Get the linear path from root to active leaf (for API calls)
    fn active_path(&self) -> Vec<&Message> {
        let mut path = Vec::new();
        let mut current = Some(self.active_leaf);

        while let Some(id) = current {
            if let Some(node) = self.nodes.get(&id) {
                path.push(&node.message);
                current = node.parent;
            } else {
                break;
            }
        }

        path.reverse();
        path
    }

    /// Get all branch points (nodes with more than one child)
    fn branch_points(&self) -> Vec<BranchPoint> {
        self.nodes.iter()
            .filter(|(_, node)| node.children.len() > 1)
            .map(|(id, node)| {
                BranchPoint {
                    node_id: *id,
                    message_preview: self.preview_message(&node.message),
                    child_count: node.children.len(),
                    branch_names: self.branch_names.iter()
                        .filter(|(_, &bid)| bid == *id)
                        .map(|(name, _)| name.clone())
                        .collect(),
                }
            })
            .collect()
    }

    fn preview_message(&self, msg: &Message) -> String {
        msg.content.iter()
            .filter_map(|block| match block {
                ContentBlock::Text(t) => Some(t.chars().take(80).collect::<String>()),
                _ => None,
            })
            .next()
            .unwrap_or_else(|| "[non-text message]".to_string())
    }
}

#[derive(Debug)]
struct BranchPoint {
    node_id: Uuid,
    message_preview: String,
    child_count: usize,
    branch_names: Vec<String>,
}

#[derive(Debug)]
enum BranchError {
    NodeNotFound(Uuid),
    InvalidBranchName(String),
}
```

The critical method is `active_path()`. The LLM API always receives a linear sequence of messages, so you need to extract the path from root to the current leaf. The tree structure is your internal bookkeeping; the API sees a simple array.

::: python Coming from Python
Python's `dict` and reference-based objects make tree structures natural: each node is a dict with a `children` list. Rust requires explicit ownership decisions. Here, the `HashMap<Uuid, ConversationNode>` acts as an arena allocator -- all nodes are owned by the map, and references between them use `Uuid` keys instead of pointers. This is a common Rust pattern for graph-like structures where nodes reference each other.
:::

## Undo and Redo via Branch Navigation

With a tree structure, undo becomes "move the active leaf to its parent" and redo becomes "move to a child." If there are multiple children (multiple branches from the same point), redo needs to know which branch to follow:

```rust
impl ConversationTree {
    /// Undo: move active leaf to the parent of the current active message
    fn undo(&mut self) -> Result<UndoResult, BranchError> {
        let current = self.nodes.get(&self.active_leaf)
            .ok_or(BranchError::NodeNotFound(self.active_leaf))?;

        match current.parent {
            Some(parent_id) => {
                let old_leaf = self.active_leaf;
                self.active_leaf = parent_id;
                Ok(UndoResult::MovedTo {
                    from: old_leaf,
                    to: parent_id,
                    can_redo: true,
                })
            }
            None => Ok(UndoResult::AtRoot),
        }
    }

    /// Redo: move active leaf to a specific child, or the first child by default
    fn redo(&mut self, child_index: Option<usize>) -> Result<RedoResult, BranchError> {
        let current = self.nodes.get(&self.active_leaf)
            .ok_or(BranchError::NodeNotFound(self.active_leaf))?;

        if current.children.is_empty() {
            return Ok(RedoResult::AtLeaf);
        }

        let idx = child_index.unwrap_or(0);
        if idx >= current.children.len() {
            return Err(BranchError::InvalidBranchName(
                format!("Child index {} out of range ({})", idx, current.children.len())
            ));
        }

        let child_id = current.children[idx];

        // Walk to the deepest descendant along the first-child path
        let mut target = child_id;
        loop {
            let node = self.nodes.get(&target).unwrap();
            if node.children.is_empty() {
                break;
            }
            target = node.children[0]; // Follow first child
        }

        self.active_leaf = target;
        Ok(RedoResult::MovedTo {
            to: target,
            alternatives: current.children.len() - 1,
        })
    }
}

#[derive(Debug)]
enum UndoResult {
    MovedTo { from: Uuid, to: Uuid, can_redo: bool },
    AtRoot,
}

#[derive(Debug)]
enum RedoResult {
    MovedTo { to: Uuid, alternatives: usize },
    AtLeaf,
}
```

The redo behavior is interesting: when you redo, you don't just move one step forward. You move all the way to the leaf of the selected branch, restoring the full conversation up to where you left off. This matches the mental model of "go back to where I was on that branch."

## Comparing Branches

One powerful use case for branching is comparing different approaches. The user asks "try implementing this with an iterator" on one branch and "try implementing this with a for loop" on another, then compares the results:

```rust
impl ConversationTree {
    /// Extract the messages unique to each branch from a common ancestor
    fn compare_branches(
        &self,
        branch_a: Uuid,
        branch_b: Uuid,
    ) -> Result<BranchComparison, BranchError> {
        // Find common ancestor
        let path_a = self.path_to_root(branch_a);
        let path_b = self.path_to_root(branch_b);

        let path_a_set: std::collections::HashSet<Uuid> = path_a.iter().copied().collect();
        let common_ancestor = path_b.iter()
            .find(|id| path_a_set.contains(id))
            .copied()
            .ok_or(BranchError::NodeNotFound(branch_b))?;

        // Extract messages unique to each branch (after the common ancestor)
        let unique_a = self.path_between(common_ancestor, branch_a);
        let unique_b = self.path_between(common_ancestor, branch_b);

        Ok(BranchComparison {
            common_ancestor,
            branch_a_messages: unique_a.iter()
                .filter_map(|id| self.nodes.get(id).map(|n| &n.message))
                .collect(),
            branch_b_messages: unique_b.iter()
                .filter_map(|id| self.nodes.get(id).map(|n| &n.message))
                .collect(),
        })
    }

    fn path_to_root(&self, from: Uuid) -> Vec<Uuid> {
        let mut path = Vec::new();
        let mut current = Some(from);
        while let Some(id) = current {
            path.push(id);
            current = self.nodes.get(&id).and_then(|n| n.parent);
        }
        path
    }

    fn path_between(&self, ancestor: Uuid, descendant: Uuid) -> Vec<Uuid> {
        let mut path = Vec::new();
        let mut current = Some(descendant);
        while let Some(id) = current {
            if id == ancestor {
                break;
            }
            path.push(id);
            current = self.nodes.get(&id).and_then(|n| n.parent);
        }
        path.reverse();
        path
    }
}

struct BranchComparison<'a> {
    common_ancestor: Uuid,
    branch_a_messages: Vec<&'a Message>,
    branch_b_messages: Vec<&'a Message>,
}
```

::: wild In the Wild
Claude Code supports conversation continuation with `--continue` and session resumption with `--resume`, which implicitly creates a form of branching. When a user resumes a session, the original conversation path is preserved and new messages extend from the resume point. If the user resumes the same session multiple times, each resumption effectively creates a branch, though Claude Code presents these as separate continuations rather than an explicit tree. Codex uses a different model entirely -- its sandbox architecture means each task runs in isolation, so there's no need for branching within a session; instead, users just start new tasks.
:::

## Persisting Tree-Structured Conversations

The tree structure complicates persistence. You need to store parent-child relationships alongside messages:

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct PersistedTreeNode {
    message: PersistedMessage,
    parent_id: Option<String>,
    children_ids: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct PersistedTree {
    nodes: Vec<PersistedTreeNode>,
    root_id: String,
    active_leaf_id: String,
    branch_names: Vec<(String, String)>, // (name, node_id)
}

impl ConversationTree {
    fn to_persisted(&self) -> PersistedTree {
        let nodes = self.nodes.iter().map(|(_, node)| {
            PersistedTreeNode {
                message: serialize_message(&node.message),
                parent_id: node.parent.map(|id| id.to_string()),
                children_ids: node.children.iter()
                    .map(|id| id.to_string())
                    .collect(),
            }
        }).collect();

        PersistedTree {
            nodes,
            root_id: self.root.to_string(),
            active_leaf_id: self.active_leaf.to_string(),
            branch_names: self.branch_names.iter()
                .map(|(name, id)| (name.clone(), id.to_string()))
                .collect(),
        }
    }
}
```

For SQLite storage, tree relationships map naturally to a relational model with a self-referencing `parent_id` column. For JSON Lines, you write each node as a line that includes its parent and children IDs -- the tree is reconstructed on load by building the `HashMap` from these references.

## When to Use Branching

Branching adds complexity. Don't implement it unless your agent needs it. Good use cases include:

- **Exploratory coding**: "Try this approach" -> doesn't work -> branch back and try another
- **A/B testing prompts**: Send the same question to two different models on separate branches and compare
- **Safe experimentation**: Branch before a risky operation so you can return to the safe state
- **Code review**: Branch to show alternative implementations without losing the original

For most agents, starting with linear history and adding branching later (behind a feature flag) is the right approach. The tree structure is a superset of linear history -- every linear conversation is a tree with no branches.

## Key Takeaways

- Model branching conversations as a tree where each node has a parent and multiple possible children, stored in a `HashMap<Uuid, ConversationNode>` arena.
- The `active_path()` method extracts the linear root-to-leaf path that the LLM API requires -- the tree is internal bookkeeping, not something the API sees.
- Undo moves the active leaf to its parent; redo follows a child path to its leaf, restoring the full conversation state on that branch.
- Branch comparison finds the common ancestor and extracts messages unique to each branch, enabling side-by-side review of alternative approaches.
- Start with linear history and add branching behind a feature flag -- the tree structure is a superset of linear, so the migration path is clean.
