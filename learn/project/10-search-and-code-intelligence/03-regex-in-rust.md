---
title: Regex In Rust
description: Master the Rust regex crate for building correct, safe, and performant regular expressions used in search tools.
---

# Regex In Rust

> **What you'll learn:**
> - How the `regex` crate guarantees linear-time matching and why this matters for untrusted patterns
> - How to use capture groups, alternation, and Unicode-aware character classes
> - How to compile and cache regex patterns for reuse across multiple search invocations

Regular expressions are the backbone of the grep tool you built in the previous subchapter. When the LLM sends a pattern like `fn\s+\w+\s*\(` to find function definitions, that pattern is compiled by the `regex` crate and executed against potentially thousands of files. Getting regex right means the difference between a search that completes in milliseconds and one that hangs for minutes -- or worse, one that returns wrong results silently.

This subchapter dives deep into Rust's `regex` crate: its safety guarantees, its syntax, and practical patterns you will use throughout the agent's search tools.

## The `regex` Crate's Safety Guarantee

The single most important thing to know about Rust's `regex` crate is that it guarantees **linear-time matching**. No matter what pattern the user (or LLM) provides, the time to match is proportional to the length of the input text -- never exponential.

This is not an academic concern. In Python, crafted "evil" regex patterns can cause catastrophic backtracking:

::: python Coming from Python
Python's `re` module uses a backtracking NFA engine. This means certain patterns can take exponential time:
```python
import re
# This gets exponentially slower as input grows
pattern = re.compile(r"(a+)+b")
# Try matching against "aaaaaaaaaaaaaaaaaaaaa!" -- it will hang
re.match(pattern, "a" * 25 + "!")  # Takes minutes or never finishes
```
Rust's `regex` crate uses a Thompson NFA / DFA hybrid engine that is immune to this attack. The same pattern in Rust completes instantly regardless of input size:
```rust
use regex::Regex;

let re = Regex::new(r"(a+)+b").unwrap();
// Completes in microseconds, even with a long input
let input = "a".repeat(1_000_000) + "!";
assert!(!re.is_match(&input));
```
This guarantee matters enormously for a coding agent, where the regex pattern comes from an LLM that might generate pathological patterns unintentionally.
:::

The trade-off is that Rust's `regex` crate does not support some features that require backtracking, notably **lookahead** and **lookbehind** assertions. If you need those, the `fancy-regex` crate provides them at the cost of losing the linear-time guarantee.

## Core Regex Syntax

Here is a quick reference of the patterns you will use most often in search tools:

```rust
use regex::Regex;

fn main() {
    // Literal matching
    let re = Regex::new(r"println!").unwrap();
    assert!(re.is_match(r#"    println!("hello");"#));

    // Character classes
    let re = Regex::new(r"[A-Z][a-z]+Error").unwrap();
    assert!(re.is_match("ParseError"));
    assert!(re.is_match("TimeoutError"));
    assert!(!re.is_match("parseerror"));

    // Word boundary
    let re = Regex::new(r"\bfn\b").unwrap();
    assert!(re.is_match("fn main() {}"));
    assert!(!re.is_match("define")); // "fn" inside "define" does not match

    // Alternation
    let re = Regex::new(r"use (std|tokio|serde)::").unwrap();
    assert!(re.is_match("use std::io;"));
    assert!(re.is_match("use tokio::fs;"));

    // Quantifiers
    let re = Regex::new(r"fn\s+\w+\s*\(").unwrap();
    assert!(re.is_match("fn main() {"));
    assert!(re.is_match("fn   parse_config ("));

    // Anchors
    let re = Regex::new(r"^use ").unwrap();
    assert!(re.is_match("use std::io;"));
    assert!(!re.is_match("  use std::io;")); // does not start with "use"
}
```

### Unicode Support

The `regex` crate is Unicode-aware by default. Character classes like `\w` match Unicode letters, not just ASCII. This is usually what you want for a coding agent working with international codebases:

```rust
use regex::Regex;

fn main() {
    let re = Regex::new(r"\w+").unwrap();

    // Matches Unicode identifiers
    let captures: Vec<&str> = re.find_iter("let nombre = 42;")
        .map(|m| m.as_str())
        .collect();
    assert_eq!(captures, vec!["let", "nombre", "42"]);

    // If you need ASCII-only matching, use (?-u:\w)
    let re_ascii = Regex::new(r"(?-u:\w)+").unwrap();
    // This restricts \w to [a-zA-Z0-9_]
}
```

## Capture Groups

Capture groups let you extract specific parts of a match. They are essential for structured search where you want not just "did it match?" but "what specifically matched?":

```rust
use regex::Regex;

fn main() {
    // Named capture groups for function signatures
    let re = Regex::new(
        r"fn\s+(?P<name>\w+)\s*\((?P<params>[^)]*)\)\s*(?:->(?P<ret>[^{]+))?"
    ).unwrap();

    let code = "fn parse_config(path: &str) -> Result<Config, Error> {";

    if let Some(caps) = re.captures(code) {
        let name = caps.name("name").unwrap().as_str();
        let params = caps.name("params").unwrap().as_str();
        let ret = caps.name("ret").map(|m| m.as_str().trim());

        println!("Function: {name}");
        println!("Parameters: {params}");
        println!("Returns: {ret:?}");
    }
}
```

Output:
```
Function: parse_config
Parameters: path: &str
Returns: Some("Result<Config, Error>")
```

Named capture groups (`(?P<name>...)`) make the code self-documenting. When you come back to this regex a month later, you can immediately see what each group extracts.

## Compiling and Caching Patterns

Regex compilation is not free -- the crate builds a finite automaton from the pattern. For the grep tool, you compile once and then match against many files. But what about when the same pattern comes up in multiple tool invocations?

The `lazy_static` or `std::sync::LazyLock` (stabilized in Rust 1.80) approaches let you compile a regex once and reuse it for the lifetime of the program:

```rust
use regex::Regex;
use std::sync::LazyLock;

// Compiled once, reused across all calls
static FUNCTION_DEF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*(?:pub\s+)?fn\s+(\w+)").unwrap()
});

static STRUCT_DEF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*(?:pub\s+)?struct\s+(\w+)").unwrap()
});

pub fn find_functions(source: &str) -> Vec<&str> {
    FUNCTION_DEF
        .captures_iter(source)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str()))
        .collect()
}

pub fn find_structs(source: &str) -> Vec<&str> {
    STRUCT_DEF
        .captures_iter(source)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str()))
        .collect()
}

fn main() {
    let source = r#"
pub struct Config {
    path: String,
}

pub fn parse_config(path: &str) -> Config {
    Config { path: path.to_string() }
}

fn helper() {}
"#;

    println!("Functions: {:?}", find_functions(source));
    println!("Structs: {:?}", find_structs(source));
}
```

Output:
```
Functions: ["parse_config", "helper"]
Structs: ["Config"]
```

For user-provided patterns (from the LLM), you cannot use `LazyLock` because the pattern is not known at compile time. Instead, use a simple cache:

```rust
use regex::Regex;
use std::collections::HashMap;
use std::sync::Mutex;

pub struct RegexCache {
    cache: Mutex<HashMap<String, Regex>>,
}

impl RegexCache {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn get_or_compile(&self, pattern: &str) -> Result<Regex, String> {
        let mut cache = self.cache.lock().unwrap();

        if let Some(re) = cache.get(pattern) {
            return Ok(re.clone());
        }

        let re = Regex::new(pattern)
            .map_err(|e| format!("Invalid regex: {e}"))?;

        cache.insert(pattern.to_string(), re.clone());
        Ok(re)
    }
}
```

The `Regex` type in Rust implements `Clone` cheaply -- it uses `Arc` internally, so cloning just increments a reference count.

## The `RegexSet` for Multi-Pattern Matching

When you need to check multiple patterns against the same text (for example, searching for functions, structs, and enums simultaneously), `RegexSet` is dramatically faster than running each pattern separately:

```rust
use regex::RegexSet;

fn main() {
    let set = RegexSet::new(&[
        r"fn\s+\w+",        // function definition
        r"struct\s+\w+",    // struct definition
        r"enum\s+\w+",      // enum definition
        r"impl\s+\w+",      // impl block
    ]).unwrap();

    let line = "pub fn parse_config(path: &str) -> Config {";

    let matches: Vec<usize> = set.matches(line).into_iter().collect();
    println!("Matched pattern indices: {matches:?}");
    // Output: Matched pattern indices: [0]
}
```

`RegexSet` compiles all patterns into a single automaton that scans the input once. This is perfect for search tools that need to categorize matches by type.

## Handling LLM-Generated Patterns

The LLM will sometimes generate patterns that are syntactically valid regex but semantically wrong, or patterns that are not regex at all. Here is a defensive wrapper:

```rust
use regex::Regex;

pub fn safe_compile_pattern(pattern: &str) -> Result<Regex, String> {
    // Reject patterns that are unreasonably long
    if pattern.len() > 1000 {
        return Err("Pattern too long (max 1000 characters)".to_string());
    }

    // Try compiling as regex first
    match Regex::new(pattern) {
        Ok(re) => Ok(re),
        Err(_) => {
            // If it fails, try escaping as a literal pattern
            let escaped = regex::escape(pattern);
            Regex::new(&escaped)
                .map_err(|e| format!("Cannot compile pattern: {e}"))
        }
    }
}
```

The fallback to `regex::escape` is important: if the LLM sends a literal string like `Config { path:` that contains regex metacharacters, we escape those characters and search for the literal text rather than returning an error. This makes the tool more forgiving.

## Multiline Mode and Flags

The `(?m)` flag changes `^` and `$` to match the start and end of each line rather than the start and end of the entire input. This is essential for line-by-line matching in source files:

```rust
use regex::Regex;

fn main() {
    // Without (?m), ^ only matches the very start of the string
    let re = Regex::new(r"^fn\s+\w+").unwrap();
    let code = "struct Foo {}\nfn main() {}";
    assert!(!re.is_match(code)); // Fails -- "fn" is not at position 0

    // With (?m), ^ matches the start of any line
    let re = Regex::new(r"(?m)^fn\s+\w+").unwrap();
    assert!(re.is_match(code)); // Succeeds -- "fn" is at the start of line 2

    // (?s) makes . match newlines (dotall mode)
    let re = Regex::new(r"(?s)fn main\(\).*\}").unwrap();
    let code = "fn main() {\n    println!(\"hello\");\n}";
    assert!(re.is_match(code)); // Matches across lines
}
```

For the grep tool, you typically search line-by-line so multiline mode is less relevant. But for the semantic search tools you will build later, matching across lines becomes important.

## Key Takeaways

- Rust's `regex` crate guarantees linear-time matching, making it safe to compile untrusted patterns from the LLM without risk of catastrophic backtracking.
- Use `LazyLock` for patterns known at compile time (like function definition matchers) and a `HashMap`-based cache for user-provided patterns.
- Named capture groups (`(?P<name>...)`) make complex regex patterns self-documenting and are essential for extracting structured data from matches.
- `RegexSet` matches multiple patterns in a single pass over the input, which is ideal for categorizing search results by type.
- Always provide a fallback for LLM-generated patterns: if the pattern fails to compile as regex, escape it and search as a literal string rather than returning an error.
