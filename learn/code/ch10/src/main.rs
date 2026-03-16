// Chapter 10: Search and Code Intelligence — Code snapshot

use std::path::Path;

/// Search file contents using a regex pattern (like ripgrep).
fn grep_search(pattern: &str, path: &str) -> Result<Vec<String>, String> {
    // TODO: Compile the regex pattern
    // TODO: Walk the directory tree
    // TODO: Search file contents for matches
    // TODO: Return matching lines with file paths and line numbers
    let _ = (pattern, path);
    Ok(vec!["TODO: implement grep_search".to_string()])
}

/// Find files matching a glob pattern.
fn glob_search(pattern: &str, path: &str) -> Result<Vec<String>, String> {
    // TODO: Parse the glob pattern
    // TODO: Walk the directory tree
    // TODO: Return matching file paths
    let _ = (pattern, Path::new(path));
    Ok(vec!["TODO: implement glob_search".to_string()])
}

fn main() {
    println!("Chapter 10: Search and Code Intelligence");

    // TODO: Register GrepTool and GlobTool
    // TODO: Add support for file type filters
    // TODO: Add context lines (-A, -B, -C)

    let results = grep_search(r"fn main", ".");
    println!("Grep results: {results:?}");

    let files = glob_search("**/*.rs", ".");
    println!("Glob results: {files:?}");
}
