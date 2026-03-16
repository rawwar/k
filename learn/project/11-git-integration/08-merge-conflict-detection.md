---
title: Merge Conflict Detection
description: Detecting merge conflicts before they happen, parsing conflict markers when they occur, and strategies for agent-assisted or automatic conflict resolution.
---

# Merge Conflict Detection

> **What you'll learn:**
> - How to detect potential merge conflicts proactively using git merge-tree dry runs
> - Techniques for parsing conflict markers and extracting ours/theirs/base versions
> - Strategies for presenting conflicts to the LLM for intelligent resolution suggestions

Merge conflicts are one of the most disruptive events in a developer's workflow. When the agent's branch diverges from the target branch and both touch the same lines, git cannot automatically reconcile the changes. A well-built agent anticipates conflicts before they happen, detects them immediately when they occur, and can even suggest resolutions by understanding the intent behind each side of the conflict.

## Proactive Conflict Detection

The best time to detect a conflict is before it happens. Git's `merge-tree` command lets you simulate a merge without modifying the working tree. This is a true dry run -- it computes the merge result in memory and reports any conflicts:

```rust
use std::path::Path;
use std::process::Command;

fn run_git(repo_path: &Path, args: &[&str]) -> Result<(String, String, bool), String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    Ok((
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.success(),
    ))
}

#[derive(Debug)]
pub struct MergePreview {
    pub has_conflicts: bool,
    pub conflicting_files: Vec<String>,
    pub clean_merge_tree: Option<String>, // tree hash if merge is clean
}

/// Preview a merge without performing it
pub fn preview_merge(
    repo_path: &Path,
    target_branch: &str,
    source_branch: &str,
) -> Result<MergePreview, String> {
    let (stdout, stderr, success) = run_git(
        repo_path,
        &["merge-tree", "--write-tree", "--name-only", target_branch, source_branch],
    )?;

    if success {
        // Clean merge -- stdout contains the resulting tree hash
        let tree_hash = stdout.lines().next().unwrap_or("").trim().to_string();
        Ok(MergePreview {
            has_conflicts: false,
            conflicting_files: Vec::new(),
            clean_merge_tree: Some(tree_hash),
        })
    } else {
        // Conflicts detected -- parse the output to find conflicting files
        // merge-tree outputs conflict information to stdout
        let mut conflicting_files = Vec::new();

        // Look for lines that indicate conflict file paths
        let mut in_conflicts = false;
        for line in stdout.lines() {
            if line.starts_with("CONFLICT") {
                in_conflicts = true;
                // Extract file path from "CONFLICT (content): Merge conflict in <path>"
                if let Some(path) = line.split("Merge conflict in ").nth(1) {
                    conflicting_files.push(path.trim().to_string());
                }
            }
        }

        // Also check stderr for conflict markers
        if !in_conflicts {
            for line in stderr.lines() {
                if line.contains("CONFLICT") {
                    if let Some(path) = line.split("Merge conflict in ").nth(1) {
                        conflicting_files.push(path.trim().to_string());
                    }
                }
            }
        }

        Ok(MergePreview {
            has_conflicts: true,
            conflicting_files,
            clean_merge_tree: None,
        })
    }
}

fn main() {
    let repo = Path::new(".");

    match preview_merge(repo, "main", "agent/feature-branch") {
        Ok(preview) => {
            if preview.has_conflicts {
                println!("Merge would produce conflicts in {} files:",
                    preview.conflicting_files.len());
                for file in &preview.conflicting_files {
                    println!("  CONFLICT: {}", file);
                }
            } else {
                println!("Merge would be clean (tree: {:?})", preview.clean_merge_tree);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

::: python Coming from Python
In Python you might use `subprocess.run(["git", "merge-tree", ...])` and check `returncode`. The pattern is the same. The key insight here is language-agnostic: `git merge-tree --write-tree` is a pure dry run that never modifies the working tree or index. Most developers (and many agents) do not know about this command -- they attempt the merge and then try to abort if there are conflicts. The dry-run approach is far safer.
:::

## Parsing Conflict Markers

When a merge does produce conflicts (because the agent went ahead with `git merge` and it could not auto-resolve), the conflicted files contain standard markers that your agent needs to parse:

```
<<<<<<< HEAD (ours)
    our version of the code
=======
    their version of the code
>>>>>>> feature-branch (theirs)
```

With `diff3` style conflicts (which you should enable for better resolution), there is also a base section:

```
<<<<<<< HEAD
    our version
||||||| base
    common ancestor version
=======
    their version
>>>>>>> feature-branch
```

Let's build a parser:

```rust
#[derive(Debug, Clone)]
pub struct ConflictHunk {
    pub file_path: String,
    pub line_number: usize,
    pub ours: String,
    pub base: Option<String>,  // Only present with diff3 style
    pub theirs: String,
    pub ours_label: String,
    pub theirs_label: String,
}

/// Parse conflict markers from a file's content
pub fn parse_conflicts(file_path: &str, content: &str) -> Vec<ConflictHunk> {
    let mut conflicts = Vec::new();
    let mut current_section = None; // "ours", "base", "theirs"
    let mut ours = String::new();
    let mut base: Option<String> = None;
    let mut theirs = String::new();
    let mut ours_label = String::new();
    let mut theirs_label = String::new();
    let mut conflict_start_line = 0;

    for (line_num, line) in content.lines().enumerate() {
        if line.starts_with("<<<<<<<") {
            current_section = Some("ours");
            ours.clear();
            base = None;
            theirs.clear();
            conflict_start_line = line_num + 1;
            ours_label = line.trim_start_matches('<').trim().to_string();
        } else if line.starts_with("|||||||") && current_section == Some("ours") {
            current_section = Some("base");
            base = Some(String::new());
        } else if line.starts_with("=======") {
            current_section = Some("theirs");
        } else if line.starts_with(">>>>>>>") {
            theirs_label = line.trim_start_matches('>').trim().to_string();

            conflicts.push(ConflictHunk {
                file_path: file_path.to_string(),
                line_number: conflict_start_line,
                ours: ours.clone(),
                base: base.clone(),
                theirs: theirs.clone(),
                ours_label: ours_label.clone(),
                theirs_label: theirs_label.clone(),
            });

            current_section = None;
        } else {
            match current_section {
                Some("ours") => {
                    if !ours.is_empty() {
                        ours.push('\n');
                    }
                    ours.push_str(line);
                }
                Some("base") => {
                    if let Some(ref mut b) = base {
                        if !b.is_empty() {
                            b.push('\n');
                        }
                        b.push_str(line);
                    }
                }
                Some("theirs") => {
                    if !theirs.is_empty() {
                        theirs.push('\n');
                    }
                    theirs.push_str(line);
                }
                _ => {} // Outside a conflict marker -- normal line
            }
        }
    }

    conflicts
}

fn main() {
    let conflicted_content = r#"fn authenticate(user: &str, password: &str) -> bool {
<<<<<<< HEAD
    // Check against the local database
    let hash = hash_password(password);
    db.verify_user(user, &hash)
||||||| merged common ancestors
    // Simple authentication
    password == "secret"
=======
    // Use the new OAuth provider
    oauth_provider.authenticate(user, password).await
>>>>>>> feature/oauth-login
}
"#;

    let conflicts = parse_conflicts("src/auth.rs", conflicted_content);

    for conflict in &conflicts {
        println!("Conflict in {} at line {}:", conflict.file_path, conflict.line_number);
        println!("  OURS ({}):\n    {}", conflict.ours_label, conflict.ours.replace('\n', "\n    "));
        if let Some(ref base) = conflict.base {
            println!("  BASE:\n    {}", base.replace('\n', "\n    "));
        }
        println!("  THEIRS ({}):\n    {}", conflict.theirs_label, conflict.theirs.replace('\n', "\n    "));
    }
}
```

## Formatting Conflicts for LLM Resolution

The LLM can often suggest intelligent conflict resolutions if you present the conflict with enough context. Here is how to format a conflict for the LLM prompt:

```rust
#[derive(Debug, Clone)]
struct ConflictHunk {
    file_path: String,
    line_number: usize,
    ours: String,
    base: Option<String>,
    theirs: String,
    ours_label: String,
    theirs_label: String,
}

/// Format a conflict for the LLM to analyze and suggest a resolution
fn format_conflict_for_llm(
    conflict: &ConflictHunk,
    surrounding_context: Option<&str>,
) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!(
        "## Merge Conflict in `{}` (line {})\n\n",
        conflict.file_path, conflict.line_number
    ));

    if let Some(context) = surrounding_context {
        prompt.push_str("### Surrounding code:\n```\n");
        prompt.push_str(context);
        prompt.push_str("\n```\n\n");
    }

    prompt.push_str(&format!(
        "### Current branch ({}):\n```\n{}\n```\n\n",
        conflict.ours_label, conflict.ours
    ));

    if let Some(ref base) = conflict.base {
        prompt.push_str(&format!(
            "### Common ancestor (original code):\n```\n{}\n```\n\n",
            base
        ));
    }

    prompt.push_str(&format!(
        "### Incoming branch ({}):\n```\n{}\n```\n\n",
        conflict.theirs_label, conflict.theirs
    ));

    prompt.push_str(
        "Please analyze both changes and suggest a resolution that preserves \
         the intent of both modifications. If the changes are incompatible, \
         explain why and recommend which version to keep.\n"
    );

    prompt
}

fn main() {
    let conflict = ConflictHunk {
        file_path: "src/auth.rs".to_string(),
        line_number: 5,
        ours: "    let hash = hash_password(password);\n    db.verify_user(user, &hash)".to_string(),
        base: Some("    password == \"secret\"".to_string()),
        theirs: "    oauth_provider.authenticate(user, password).await".to_string(),
        ours_label: "HEAD".to_string(),
        theirs_label: "feature/oauth-login".to_string(),
    };

    let prompt = format_conflict_for_llm(&conflict, None);
    println!("{}", prompt);
}
```

## Detecting Conflicting Files in the Working Tree

After a failed merge, you need to enumerate which files have conflicts so the agent (or user) can resolve them one by one:

```rust
use std::path::Path;
use std::process::Command;

fn run_git_checked(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// List files that currently have unresolved merge conflicts
pub fn list_conflicted_files(repo_path: &Path) -> Result<Vec<String>, String> {
    let output = run_git_checked(repo_path, &["diff", "--name-only", "--diff-filter=U"])?;
    Ok(output.lines().map(String::from).filter(|s| !s.is_empty()).collect())
}

/// Check if the repository is in a merge state
pub fn is_merging(repo_path: &Path) -> Result<bool, String> {
    let git_dir = run_git_checked(repo_path, &["rev-parse", "--git-dir"])?;
    let merge_head = Path::new(&git_dir).join("MERGE_HEAD");
    Ok(merge_head.exists())
}

/// Abort a merge in progress (returns to pre-merge state)
pub fn abort_merge(repo_path: &Path) -> Result<String, String> {
    if !is_merging(repo_path)? {
        return Err("No merge in progress".to_string());
    }
    run_git_checked(repo_path, &["merge", "--abort"])
}

/// Mark a file as resolved after editing it to remove conflict markers
pub fn mark_resolved(repo_path: &Path, file_path: &str) -> Result<(), String> {
    // Verify the file no longer contains conflict markers
    let content = std::fs::read_to_string(Path::new(repo_path).join(file_path))
        .map_err(|e| format!("Cannot read {}: {}", file_path, e))?;

    if content.contains("<<<<<<<") || content.contains(">>>>>>>") {
        return Err(format!(
            "{} still contains conflict markers. Resolve all conflicts before marking as resolved.",
            file_path
        ));
    }

    run_git_checked(repo_path, &["add", "--", file_path])?;
    Ok(())
}

fn main() {
    let repo = Path::new(".");

    if let Ok(true) = is_merging(repo) {
        println!("Merge in progress!");

        match list_conflicted_files(repo) {
            Ok(files) => {
                println!("Conflicted files:");
                for f in &files {
                    println!("  {}", f);
                }
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    } else {
        println!("No merge in progress");
    }
}
```

::: wild In the Wild
Claude Code detects merge conflicts and presents them to the user with clear formatting showing both sides. Rather than attempting automatic resolution (which could introduce subtle bugs), it explains the conflict and suggests a resolution for the user to review and approve. This human-in-the-loop approach is appropriate for merge conflicts because the "correct" resolution often depends on product intent that the agent cannot determine from code alone.
:::

## Enabling diff3 for Better Conflict Context

The default conflict style only shows "ours" and "theirs." The `diff3` style also includes the common ancestor (base), which makes resolution much easier because you can see what the original code looked like before either side changed it. Your agent should recommend or enable this:

```rust
use std::path::Path;
use std::process::Command;

fn run_git_checked(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Check if diff3 conflict style is configured
pub fn has_diff3_style(repo_path: &Path) -> Result<bool, String> {
    let output = run_git_checked(
        repo_path,
        &["config", "--get", "merge.conflictStyle"],
    );

    match output {
        Ok(style) => Ok(style.trim() == "diff3" || style.trim() == "zdiff3"),
        Err(_) => Ok(false), // Not configured, defaults to "merge" style
    }
}

/// Suggest diff3 configuration if not already set
pub fn suggest_diff3(repo_path: &Path) -> String {
    match has_diff3_style(repo_path) {
        Ok(true) => "diff3 conflict style is already configured.".to_string(),
        _ => {
            "Recommendation: Enable diff3 conflict style for better merge conflict resolution.\n\
             Run: git config merge.conflictStyle diff3\n\
             This adds the common ancestor to conflict markers, making resolution easier."
                .to_string()
        }
    }
}

fn main() {
    let repo = Path::new(".");
    println!("{}", suggest_diff3(repo));
}
```

## Key Takeaways

- Use `git merge-tree --write-tree` to preview merges without modifying the working tree -- always check for conflicts before attempting a merge.
- Parse conflict markers into structured `ConflictHunk` types that separate ours, base, and theirs for programmatic handling.
- Format conflicts with surrounding code context for the LLM, which can suggest intelligent resolutions that preserve the intent of both changes.
- Always verify that conflict markers are fully removed before marking a file as resolved with `git add`.
- Enable `diff3` conflict style so the common ancestor is included in conflict markers -- this dramatically improves both human and LLM resolution quality.
