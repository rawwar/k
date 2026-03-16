---
title: Handling Large Files
description: Manage files that are too large to fit in the model's context window by reading ranges and summarizing content.
---

# Handling Large Files

> **What you'll learn:**
> - How to detect when a file exceeds a configurable size threshold and switch to range-based reading
> - How to implement line-range and byte-range reading to give the model focused slices of large files
> - How to provide file metadata (size, line count, type) when the full content cannot be returned

Most source files are small enough to fit comfortably in the model's context window. A typical Rust source file is 100-300 lines, and the model can handle thousands of files' worth of content. But some files are enormous: generated code, data files, minified JavaScript, lock files. When the model tries to read a 50,000-line file, you have a problem -- the output consumes most of the context window and pushes out the conversation history the model needs to stay on track.

## The Size Threshold

The first step is defining "too large." This depends on the model's context window size and how much of it you want file content to consume. A reasonable starting point:

```rust
/// Configuration for large file handling
pub struct FileSizeConfig {
    /// Maximum number of lines to return in a single read (default: 2000)
    pub max_lines: usize,
    /// Maximum file size in bytes to attempt reading at all (default: 10 MB)
    pub max_bytes: usize,
    /// Number of lines to return in the metadata preview (default: 20)
    pub preview_lines: usize,
}

impl Default for FileSizeConfig {
    fn default() -> Self {
        Self {
            max_lines: 2000,
            max_bytes: 10 * 1024 * 1024, // 10 MB
            preview_lines: 20,
        }
    }
}
```

These defaults mean: if a file has more than 2,000 lines, switch to range-based reading. If a file is larger than 10 MB, refuse to read it entirely (it is probably generated or binary). Return the first 20 lines as a preview when the file is too large.

## Checking File Size Before Reading

Update the read tool to check size before reading the entire file:

```rust
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Get file metadata without reading the full content.
pub fn file_info(path: &Path) -> Result<FileMetadata, String> {
    let metadata = fs::metadata(path)
        .map_err(|e| format!("Cannot read metadata for '{}': {}", path.display(), e))?;

    let size_bytes = metadata.len() as usize;

    // Count lines efficiently using a buffered reader
    let line_count = count_lines(path)?;

    // Detect file type from extension
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(FileMetadata {
        size_bytes,
        line_count,
        extension,
    })
}

pub struct FileMetadata {
    pub size_bytes: usize,
    pub line_count: usize,
    pub extension: String,
}

impl FileMetadata {
    pub fn summary(&self, path: &Path) -> String {
        format!(
            "File: {}\nSize: {} bytes ({:.1} KB)\nLines: {}\nType: .{}",
            path.display(),
            self.size_bytes,
            self.size_bytes as f64 / 1024.0,
            self.line_count,
            self.extension
        )
    }
}

/// Count lines in a file without loading it all into memory.
fn count_lines(path: &Path) -> Result<usize, String> {
    let file = fs::File::open(path)
        .map_err(|e| format!("Cannot open '{}': {}", path.display(), e))?;

    let reader = BufReader::new(file);
    let count = reader.lines().count();
    Ok(count)
}
```

The `count_lines` function uses a `BufReader` to count lines without loading the entire file into memory. For a 100 MB file, `fs::read_to_string` would allocate 100 MB of memory; `BufReader::lines()` processes line by line with a small buffer.

::: python Coming from Python
In Python, you would count lines efficiently like this:
```python
def count_lines(path: str) -> int:
    with open(path) as f:
        return sum(1 for _ in f)
```
This iterates line by line without loading the whole file, just like Rust's `BufReader::lines()`. The Rust version is more verbose but gives you the same streaming behavior. The key insight is the same in both languages: never call `read()` or `read_to_string()` on a file until you know its size.
:::

## Range-Based Reading

When a file is too large, the read tool should switch to range-based reading automatically. Here is the updated logic:

```rust
use std::io::{BufRead, BufReader};

/// Read a specific range of lines from a file efficiently.
/// Uses a buffered reader to skip lines without loading them into memory.
pub fn read_line_range(
    path: &Path,
    start_line: usize,  // 1-based
    max_lines: usize,
) -> Result<(Vec<String>, usize), String> {
    let file = fs::File::open(path)
        .map_err(|e| format!("Cannot open '{}': {}", path.display(), e))?;

    let reader = BufReader::new(file);
    let mut lines_collected = Vec::new();
    let mut total_lines = 0;

    for (idx, line_result) in reader.lines().enumerate() {
        total_lines = idx + 1;
        let line_num = idx + 1; // 1-based

        if line_num < start_line {
            // Skip lines before the requested range
            continue;
        }

        if lines_collected.len() >= max_lines {
            // We have enough lines, but keep counting total
            continue;
        }

        let line = line_result
            .map_err(|e| format!("Error reading line {}: {}", line_num, e))?;

        lines_collected.push(format!("{:>4}\t{}", line_num, line));
    }

    Ok((lines_collected, total_lines))
}
```

This function streams through the file, collecting only the lines in the requested range. Lines before the range are skipped (though they still must be iterated over), and lines after are counted but not stored. The memory usage is proportional to `max_lines`, not the file size.

## Updating the Read Tool

Now integrate size checking into the read tool's execute method:

```rust
fn execute(&self, input: &Value) -> Result<String, String> {
    let path_str = input.get("path").and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: path".to_string())?;

    let path = resolve_path(&self.base_dir, path_str)?;
    let config = FileSizeConfig::default();

    // Check file size first
    let metadata = fs::metadata(&path)
        .map_err(|e| format!("Cannot access '{}': {}", path.display(), e))?;

    let size = metadata.len() as usize;

    if size > config.max_bytes {
        // File is too large to read at all
        let info = file_info(&path)?;
        return Ok(format!(
            "[File too large to read]\n{}\n\nUse offset and limit parameters \
             to read a specific range, or consider if you need this file.",
            info.summary(&path)
        ));
    }

    // Parse offset and limit
    let offset = input.get("offset")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(1);

    let limit = input.get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    // For large files without explicit range, auto-limit
    let effective_limit = match limit {
        Some(lim) => lim,
        None => {
            // Count lines to check if file exceeds threshold
            let line_count = count_lines(&path)?;
            if line_count > config.max_lines {
                // Return metadata + preview instead of full content
                let (preview, total) = read_line_range(
                    &path, 1, config.preview_lines,
                )?;

                let mut output = format!(
                    "[File has {} lines, showing first {}. \
                     Use offset and limit to read specific ranges.]\n\n",
                    total, config.preview_lines
                );
                output.push_str(&preview.join("\n"));
                output.push_str(&format!(
                    "\n\n[... {} more lines. Use offset={} limit=N to continue.]",
                    total - config.preview_lines,
                    config.preview_lines + 1
                ));

                return Ok(output);
            }
            line_count
        }
    };

    // Read the requested range
    let (lines, total) = read_line_range(&path, offset, effective_limit)?;

    let mut result = String::new();

    if offset > 1 || limit.is_some() {
        result.push_str(&format!(
            "[Showing lines {}-{} of {} total]\n",
            offset,
            offset + lines.len() - 1,
            total
        ));
    }

    result.push_str(&lines.join("\n"));

    Ok(result)
}
```

The updated flow is:

1. Check if the file exceeds the byte limit -- if so, return metadata only
2. If no explicit range was requested, count lines to check the line limit
3. If the line count exceeds the threshold, return a preview with instructions
4. Otherwise, read normally with the range-based reader

The metadata messages are critical. They tell the model "this file has 5,000 lines, here are the first 20, use offset and limit to read more." The model can then make informed decisions about which range to read.

## Handling Binary Files in the Size Check

Combine the binary detection from the permissions subchapter with the size check for a complete pre-read validation:

```rust
/// Validate a file before reading, returning an early response for
/// binary or oversized files.
pub fn validate_for_read(
    path: &Path,
    config: &FileSizeConfig,
) -> Result<Option<String>, String> {
    // Check if file exists
    if !path.exists() {
        return Err(format!("File not found: '{}'", path.display()));
    }

    // Check if it's a directory
    if path.is_dir() {
        return Err(format!(
            "'{}' is a directory. Use glob_search to list contents.",
            path.display()
        ));
    }

    // Check for binary content
    if is_binary_file(path)? {
        let size = fs::metadata(path)
            .map(|m| m.len())
            .unwrap_or(0);
        return Ok(Some(format!(
            "[Binary file: {} ({} bytes). Cannot display as text.]",
            path.display(),
            size
        )));
    }

    // Check file size
    let size = fs::metadata(path)
        .map_err(|e| format!("Cannot read metadata: {}", e))?
        .len() as usize;

    if size > config.max_bytes {
        return Ok(Some(format!(
            "[File too large: {} ({} bytes, limit is {} bytes).\n\
             Cannot read files larger than {:.1} MB.]",
            path.display(),
            size,
            config.max_bytes,
            config.max_bytes as f64 / (1024.0 * 1024.0)
        )));
    }

    // File is valid for reading
    Ok(None)
}
```

This returns `Ok(None)` for normal files and `Ok(Some(message))` for files that need special handling. The caller can return the message directly as the tool result.

## Testing Large File Handling

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_count_lines() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("lines.txt");
        let mut f = fs::File::create(&path).unwrap();
        for i in 1..=100 {
            writeln!(f, "Line {}", i).unwrap();
        }

        assert_eq!(count_lines(&path).unwrap(), 100);
    }

    #[test]
    fn test_read_line_range() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("data.txt");
        let mut f = fs::File::create(&path).unwrap();
        for i in 1..=50 {
            writeln!(f, "Line {}", i).unwrap();
        }

        let (lines, total) = read_line_range(&path, 10, 5).unwrap();
        assert_eq!(total, 50);
        assert_eq!(lines.len(), 5);
        assert!(lines[0].contains("Line 10"));
        assert!(lines[4].contains("Line 14"));
    }

    #[test]
    fn test_read_line_range_beyond_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("short.txt");
        let mut f = fs::File::create(&path).unwrap();
        for i in 1..=5 {
            writeln!(f, "Line {}", i).unwrap();
        }

        let (lines, total) = read_line_range(&path, 3, 100).unwrap();
        assert_eq!(total, 5);
        assert_eq!(lines.len(), 3); // Lines 3, 4, 5
    }

    #[test]
    fn test_file_info() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.rs");
        fs::write(&path, "fn main() {\n    println!(\"hi\");\n}\n").unwrap();

        let info = file_info(&path).unwrap();
        assert_eq!(info.line_count, 3);
        assert_eq!(info.extension, "rs");
        assert!(info.size_bytes > 0);
    }

    #[test]
    fn test_validate_binary_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("binary.bin");
        fs::write(&path, b"\x00\x01\x02\x03binary\x00data").unwrap();

        let config = FileSizeConfig::default();
        let result = validate_for_read(&path, &config).unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().contains("Binary file"));
    }

    #[test]
    fn test_validate_normal_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("normal.rs");
        fs::write(&path, "fn main() {}\n").unwrap();

        let config = FileSizeConfig::default();
        let result = validate_for_read(&path, &config).unwrap();
        assert!(result.is_none()); // No issue, proceed with read
    }

    #[test]
    fn test_validate_oversized_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("huge.txt");
        // Create a file larger than the limit
        let content = "x".repeat(200);
        fs::write(&path, &content).unwrap();

        let config = FileSizeConfig {
            max_bytes: 100, // Very small limit for testing
            ..Default::default()
        };
        let result = validate_for_read(&path, &config).unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().contains("too large"));
    }
}
```

::: wild In the Wild
Claude Code's Read tool automatically truncates output at 2,000 lines and includes a message telling the model to use offset/limit parameters for the rest. This threshold is configurable. The tool also returns file metadata (size, line count) when truncation occurs, so the model can make informed decisions about what to read next. This pagination pattern is essential for working with real codebases where files like `Cargo.lock` or generated bindings can be thousands of lines long.
:::

## Key Takeaways

- Always check file size before reading: use `fs::metadata` for byte size and `BufReader::lines().count()` for line count, both of which avoid loading the file into memory.
- When a file exceeds the line threshold, return a preview (first N lines) plus metadata, with instructions telling the model how to request specific ranges.
- Range-based reading with `BufReader` processes files line-by-line, keeping memory usage proportional to the output size rather than the file size.
- Combine binary detection, size limits, and directory checks into a single validation step at the top of the read tool's execute method for clean, early-return logic.
- The metadata messages ("File has 5000 lines, showing first 20") are not just informational -- they teach the model how to use the tool's offset/limit parameters to navigate large files.
