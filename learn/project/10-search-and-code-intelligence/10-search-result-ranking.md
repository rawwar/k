---
title: Search Result Ranking
description: Rank and prioritize search results by relevance, recency, and structural importance to present the most useful matches first.
---

# Search Result Ranking

> **What you'll learn:**
> - How to score search results based on match quality, file path relevance, and structural context
> - How to apply recency and proximity boosts for results near the user's current focus area
> - How to truncate result sets intelligently to fit within context window token budgets

A search that returns 500 matches is not helpful -- it is overwhelming. The agent's context window has a finite token budget, and every search result that gets included displaces space for the LLM's reasoning, the conversation history, and the system prompt. Ranking ensures the *most relevant* results appear first, so when you truncate the list to fit the context window, you keep the good matches and discard the noise.

## Why Ranking Matters for Agents

Consider a grep search for `config` in a medium-sized Rust project. You might get:

- 3 matches in `src/config.rs` (the actual config module)
- 8 matches in `src/main.rs` (using the config)
- 15 matches in test files (test fixtures)
- 40 matches in `target/` build artifacts (if not filtered)
- 12 matches in comments and documentation

Without ranking, these arrive in filesystem walk order -- typically alphabetical by path. The most important result (the struct definition in `config.rs`) might appear third in a list of 78. With ranking, it appears first.

::: tip Coming from Python
Python developers working with search might use a simple sort:
```python
results.sort(key=lambda r: r.relevance_score, reverse=True)
```
The Rust equivalent uses `sort_by` with a comparison function or `sort_by_cached_key` for expensive score computations:
```rust
results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
```
The challenge in both languages is the same: defining what "relevance" means. The scoring model we build here combines multiple signals into a single score.
:::

## The Scoring Model

A search result's relevance is not a single factor -- it is a weighted combination of signals. Here is the scoring model:

```rust
#[derive(Debug, Clone)]
pub struct ScoredResult {
    pub path: std::path::PathBuf,
    pub line: usize,
    pub content: String,
    pub context: Vec<String>,
    pub score: f64,
    pub score_breakdown: ScoreBreakdown,
}

#[derive(Debug, Clone, Default)]
pub struct ScoreBreakdown {
    pub match_quality: f64,
    pub path_relevance: f64,
    pub structural_importance: f64,
    pub recency_boost: f64,
    pub proximity_boost: f64,
}

impl ScoreBreakdown {
    pub fn total(&self) -> f64 {
        self.match_quality
            + self.path_relevance
            + self.structural_importance
            + self.recency_boost
            + self.proximity_boost
    }
}
```

### Signal 1: Match Quality

How well does the match fit the search intent? An exact word match scores higher than a substring match. A match at the start of a line scores higher than one buried in the middle:

```rust
pub fn score_match_quality(line: &str, pattern: &str, match_start: usize) -> f64 {
    let mut score = 1.0; // Base score for any match

    // Bonus for matching at word boundary
    if match_start == 0 || !line.as_bytes()[match_start - 1].is_ascii_alphanumeric() {
        score += 0.5;
    }

    // Bonus for matching at the start of the line (after whitespace)
    let trimmed_start = line.len() - line.trim_start().len();
    if match_start == trimmed_start {
        score += 0.3;
    }

    // Bonus for matching the entire identifier (not a substring)
    let match_end = match_start + pattern.len();
    let at_end = match_end >= line.len()
        || !line.as_bytes()[match_end].is_ascii_alphanumeric();
    if at_end {
        score += 0.5;
    }

    // Penalty for very long lines (likely minified or generated code)
    if line.len() > 200 {
        score *= 0.5;
    }

    score
}
```

### Signal 2: Path Relevance

Files in `src/` are usually more important than files in `tests/` or `examples/`. The file path gives strong hints about relevance:

```rust
use std::path::Path;

pub fn score_path_relevance(path: &Path) -> f64 {
    let path_str = path.to_string_lossy().to_lowercase();
    let mut score = 1.0;

    // Source directories get a boost
    if path_str.contains("/src/") || path_str.starts_with("src/") {
        score += 0.5;
    }

    // Test files get a slight penalty (usually less relevant)
    if path_str.contains("/test")
        || path_str.contains("_test.")
        || path_str.contains(".test.")
    {
        score -= 0.3;
    }

    // Example and benchmark files get a penalty
    if path_str.contains("/examples/") || path_str.contains("/benches/") {
        score -= 0.2;
    }

    // Vendor/third-party code gets a heavy penalty
    if path_str.contains("/vendor/")
        || path_str.contains("node_modules/")
        || path_str.contains("/third_party/")
    {
        score -= 1.0;
    }

    // Files closer to the project root are often more important
    let depth = path.components().count();
    if depth <= 3 {
        score += 0.2;
    } else if depth > 6 {
        score -= 0.1;
    }

    // Boost for files with "main", "lib", or "mod" in the name
    let filename = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if filename == "main" || filename == "lib" || filename == "mod" {
        score += 0.3;
    }

    score
}
```

### Signal 3: Structural Importance

If you have AST information, a definition scores higher than a usage, and a public definition scores higher than a private one:

```rust
pub fn score_structural_importance(
    node_kind: Option<&str>,
    is_definition: bool,
    is_public: bool,
) -> f64 {
    let mut score = 0.0;

    // Definitions are more important than usages
    if is_definition {
        score += 1.0;
    }

    // Public items are more important than private ones
    if is_public {
        score += 0.5;
    }

    // Certain node kinds are more important
    if let Some(kind) = node_kind {
        match kind {
            "function_item" | "function_definition" => score += 0.8,
            "struct_item" | "class_definition" => score += 0.9,
            "enum_item" => score += 0.7,
            "trait_item" | "interface_declaration" => score += 0.9,
            "impl_item" => score += 0.6,
            "type_item" | "type_alias" => score += 0.5,
            _ => {}
        }
    }

    score
}
```

### Signal 4: Recency Boost

Recently modified files are more likely to be relevant to the current task:

```rust
use std::time::SystemTime;

pub fn score_recency(modified: Option<SystemTime>) -> f64 {
    let Some(modified) = modified else {
        return 0.0;
    };

    let age = SystemTime::now()
        .duration_since(modified)
        .unwrap_or_default();

    let hours = age.as_secs() as f64 / 3600.0;

    if hours < 1.0 {
        1.0 // Modified in the last hour
    } else if hours < 24.0 {
        0.5 // Modified today
    } else if hours < 168.0 {
        0.2 // Modified this week
    } else {
        0.0 // Older than a week
    }
}
```

### Signal 5: Proximity Boost

Results near the user's "focus area" (files they have recently read or edited in the conversation) get a boost:

```rust
use std::collections::HashSet;
use std::path::Path;

pub fn score_proximity(
    result_path: &Path,
    focus_files: &HashSet<std::path::PathBuf>,
) -> f64 {
    // Exact file match -- highest boost
    if focus_files.contains(result_path) {
        return 1.5;
    }

    // Same directory as a focus file
    let result_dir = result_path.parent();
    for focus in focus_files {
        if result_dir == focus.parent() {
            return 0.8;
        }
    }

    // Same parent directory (sibling directories)
    for focus in focus_files {
        if let (Some(r_parent), Some(f_parent)) = (
            result_dir.and_then(|d| d.parent()),
            focus.parent().and_then(|d| d.parent()),
        ) {
            if r_parent == f_parent {
                return 0.3;
            }
        }
    }

    0.0
}
```

## Combining Scores and Ranking

Now bring all the signals together:

```rust
use std::collections::HashSet;
use std::path::PathBuf;

pub fn rank_results(
    results: &mut Vec<ScoredResult>,
    focus_files: &HashSet<PathBuf>,
) {
    for result in results.iter_mut() {
        let breakdown = ScoreBreakdown {
            match_quality: score_match_quality(
                &result.content,
                "", // pattern would be passed in
                0,
            ),
            path_relevance: score_path_relevance(&result.path),
            structural_importance: 0.0, // populated by code-aware search
            recency_boost: score_recency(
                std::fs::metadata(&result.path)
                    .ok()
                    .and_then(|m| m.modified().ok()),
            ),
            proximity_boost: score_proximity(&result.path, focus_files),
        };

        result.score = breakdown.total();
        result.score_breakdown = breakdown;
    }

    // Sort by score descending
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}
```

## Context-Window-Aware Truncation

The final step is truncating the ranked results to fit the available token budget. A simple character-based approximation works well enough:

```rust
pub fn truncate_to_token_budget(
    results: &[ScoredResult],
    max_tokens: usize,
) -> (Vec<&ScoredResult>, usize) {
    // Approximate: 1 token ~= 4 characters for English/code
    let chars_per_token = 4;
    let max_chars = max_tokens * chars_per_token;

    let mut total_chars = 0;
    let mut included = Vec::new();

    for result in results {
        // Estimate the character cost of including this result
        let result_chars = result.path.to_string_lossy().len()
            + 10 // line number, separators
            + result.content.len()
            + result.context.iter().map(|c| c.len() + 5).sum::<usize>();

        if total_chars + result_chars > max_chars && !included.is_empty() {
            break;
        }

        total_chars += result_chars;
        included.push(result);
    }

    let omitted = results.len() - included.len();
    (included, omitted)
}

pub fn format_ranked_results(
    results: &[ScoredResult],
    max_tokens: usize,
) -> String {
    let (included, omitted) = truncate_to_token_budget(results, max_tokens);

    if included.is_empty() {
        return "No matches found.".to_string();
    }

    let mut output = String::new();

    for result in &included {
        output.push_str(&format!(
            "{}:{}: {}\n",
            result.path.display(),
            result.line,
            result.content.trim(),
        ));

        // Include context lines
        for ctx in &result.context {
            output.push_str(&format!("  {ctx}\n"));
        }

        output.push_str("---\n");
    }

    if omitted > 0 {
        output.push_str(&format!(
            "\n[{omitted} additional results omitted. Refine your search for more specific results.]\n"
        ));
    }

    output
}
```

The truncation message is important: it tells the LLM that more results exist and suggests refining the search. This is a cue the LLM can act on -- it might narrow the search path, add a file type filter, or use a more specific pattern.

::: info In the Wild
Claude Code's search tools include a result limit and report how many results were truncated. This pattern appears across coding agents: showing a bounded number of results with a "N more matches" indicator lets the LLM decide whether the current results are sufficient or whether it needs to refine the search. The key insight is that truncation should be *informative* -- the agent needs to know that important results might have been cut off.
:::

## Deduplication

When multiple search tools produce overlapping results (grep finds a line, semantic search finds the same function definition), you should deduplicate before ranking:

```rust
pub fn deduplicate_results(results: &mut Vec<ScoredResult>) {
    let mut seen = HashSet::new();

    results.retain(|result| {
        let key = (result.path.clone(), result.line);
        seen.insert(key)
    });
}
```

## Key Takeaways

- Ranking transforms raw search results into an ordered list where the most useful matches appear first -- this is critical because context window truncation discards results from the bottom.
- The scoring model combines five signals: match quality, path relevance, structural importance, recency, and proximity to the user's focus area. Each signal contributes independently.
- Path-based scoring (boosting `src/`, penalizing `tests/` and `vendor/`) is simple to implement and provides a surprisingly strong relevance signal.
- Context-window-aware truncation uses character-based token estimation to fit results within the available budget, always including a count of omitted results so the LLM can refine its search.
- Deduplication across search tools prevents the same match from appearing multiple times when grep, glob, and semantic search results are combined.
