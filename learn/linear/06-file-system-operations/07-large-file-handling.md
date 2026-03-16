---
title: Large File Handling
description: Strategies for reading and editing large files without loading them entirely into memory or exceeding context window limits.
---

# Large File Handling

> **What you'll learn:**
> - How to detect when a file is too large for full reading and apply streaming or chunked approaches
> - Techniques for editing large files in place without loading the entire content into memory
> - How to set sensible file size limits in your agent and communicate them to the model via tool descriptions

Most source code files are small -- a few hundred lines, a few kilobytes. But codebases also contain large files: generated code, vendored dependencies, SQL migration dumps, CSV data files, log files, and lock files like `package-lock.json` or `Cargo.lock`. Your agent needs to handle these without crashing, running out of memory, or flooding the context window with tens of thousands of lines.

## Defining "Large"

There are two dimensions of "large" that matter for a coding agent:

1. **Memory-large**: Files that would consume excessive memory if loaded entirely. A 100 MB log file would allocate 100 MB of heap. This is the traditional large file problem.

2. **Context-large**: Files that would consume too many tokens if sent to the LLM. A 10,000-line file might be only 300 KB on disk (fine for memory) but would use 100,000+ tokens in the context window (very much not fine).

For a coding agent, context-large is usually the binding constraint. You'll hit token limits long before you hit memory limits.

```rust
use std::fs;
use std::path::Path;

const MAX_FILE_SIZE_BYTES: u64 = 10 * 1024 * 1024; // 10 MB
const MAX_LINES_FOR_FULL_READ: usize = 2000;
const LARGE_FILE_PREVIEW_LINES: usize = 100;

#[derive(Debug)]
pub enum FileSize {
    Normal,
    ContextLarge { lines: usize },
    MemoryLarge { bytes: u64 },
    Binary,
}

pub fn classify_file(path: &Path) -> Result<FileSize, std::io::Error> {
    let metadata = fs::metadata(path)?;
    let size = metadata.len();

    if size > MAX_FILE_SIZE_BYTES {
        return Ok(FileSize::MemoryLarge { bytes: size });
    }

    // Check for binary content
    let mut file = fs::File::open(path)?;
    let mut header = [0u8; 8192];
    use std::io::Read;
    let n = file.read(&mut header)?;
    if header[..n].contains(&0) {
        return Ok(FileSize::Binary);
    }

    // Count lines for context size classification
    let content = fs::read_to_string(path)?;
    let line_count = content.lines().count();
    if line_count > MAX_LINES_FOR_FULL_READ {
        return Ok(FileSize::ContextLarge { lines: line_count });
    }

    Ok(FileSize::Normal)
}
```

## Handling Context-Large Files

When a file is too large for a full read, provide a preview with metadata:

```rust
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub fn read_large_file_preview(
    path: &Path,
    preview_lines: usize,
) -> Result<String, std::io::Error> {
    let metadata = fs::metadata(path)?;
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);

    let mut output = String::new();
    let mut total_lines = 0;
    let mut preview = String::new();

    for (i, line_result) in reader.lines().enumerate() {
        let line = line_result?;
        total_lines = i + 1;
        if i < preview_lines {
            preview.push_str(&format!("{:>6}\t{}\n", i + 1, line));
        }
    }

    // Header with file metadata
    output.push_str(&format!(
        "File: {} ({} lines, {} bytes)\n",
        path.display(),
        total_lines,
        metadata.len()
    ));
    output.push_str(&format!(
        "Showing first {} lines of {}:\n\n",
        preview_lines.min(total_lines),
        total_lines
    ));
    output.push_str(&preview);

    if total_lines > preview_lines {
        output.push_str(&format!(
            "\n... {} more lines. Use offset parameter to read specific sections.\n",
            total_lines - preview_lines
        ));
    }

    Ok(output)
}
```

This tells the LLM how large the file is and shows the beginning. The model can then use the offset parameter to navigate to specific sections.

::: tip Coming from Python
In Python you might use `itertools.islice(open("file.txt"), 100)` to read the first 100 lines without loading the whole file. Rust's `BufReader` + `lines()` gives you the same lazy iteration. The difference is that Rust makes the buffering explicit -- you choose the buffer strategy rather than relying on Python's implicit buffering.
:::

## Memory-Mapped File Access

For truly large files (hundreds of megabytes), memory mapping lets the OS handle paging:

```rust
use std::fs::File;
use std::path::Path;
use memmap2::Mmap;

fn search_large_file(
    path: &Path,
    pattern: &str,
) -> Result<Vec<(usize, String)>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };

    // mmap gives us a &[u8] that is backed by the OS page cache
    // The OS loads pages on demand as we access them
    let content = std::str::from_utf8(&mmap)
        .map_err(|e| format!("File is not valid UTF-8: {e}"))?;

    let mut matches = Vec::new();
    for (line_num, line) in content.lines().enumerate() {
        if line.contains(pattern) {
            matches.push((line_num + 1, line.to_string()));
        }
    }

    Ok(matches)
}
```

Memory mapping creates a virtual memory region that maps directly to the file on disk. When you access a byte, the OS loads the corresponding page from disk transparently. This means:

- You can work with files larger than available RAM (the OS pages data in and out).
- No explicit read calls -- you access the file through a byte slice.
- The OS kernel optimizes read-ahead and caching for you.

The `unsafe` block is required because memory-mapped files can change underneath you if another process modifies the file. For a coding agent, this risk is minimal (you're the one modifying files), but it's good to be aware of.

::: details When to use mmap vs BufReader
Use `BufReader` for sequential reads (reading line by line from start to end). Use `mmap` for random access (searching, jumping to specific byte offsets) or when you need to search a very large file without loading it all into a `String`. For most coding agent operations, `BufReader` is the right choice. Reserve `mmap` for specialized use cases like searching generated files or log analysis.
:::

## Editing Large Files

Editing a large file with string replacement still requires loading the file into memory for the search. But you can minimize memory usage:

```rust
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use tempfile::NamedTempFile;

pub fn edit_large_file_by_line_range(
    path: &Path,
    start_line: usize,
    end_line: usize,
    replacement: &str,
) -> Result<String, String> {
    let file = fs::File::open(path)
        .map_err(|e| format!("Cannot open: {e}"))?;
    let reader = BufReader::new(file);
    let parent = path.parent().unwrap_or(Path::new("."));

    let mut temp = NamedTempFile::new_in(parent)
        .map_err(|e| format!("Cannot create temp: {e}"))?;

    let mut line_num = 0;
    let mut replaced = false;

    for line_result in reader.lines() {
        let line = line_result.map_err(|e| format!("Read error: {e}"))?;
        line_num += 1;

        if line_num == start_line && !replaced {
            // Write the replacement content
            temp.write_all(replacement.as_bytes())
                .map_err(|e| format!("Write error: {e}"))?;
            temp.write_all(b"\n")
                .map_err(|e| format!("Write error: {e}"))?;
            replaced = true;
        } else if line_num > start_line && line_num <= end_line {
            // Skip lines in the replaced range
            continue;
        } else {
            // Copy unchanged lines
            temp.write_all(line.as_bytes())
                .map_err(|e| format!("Write error: {e}"))?;
            temp.write_all(b"\n")
                .map_err(|e| format!("Write error: {e}"))?;
        }
    }

    temp.flush().map_err(|e| format!("Flush error: {e}"))?;
    temp.persist(path).map_err(|e| format!("Persist error: {e}"))?;

    Ok(format!(
        "Replaced lines {}-{} ({} lines removed, replacement written)",
        start_line, end_line, end_line - start_line + 1
    ))
}
```

This approach streams through the file line by line, writing unchanged lines directly to the temp file and inserting the replacement at the target location. Peak memory usage is proportional to the longest single line, not the file size.

## Setting and Communicating Limits

Your agent's tool descriptions should clearly communicate file size limits to the LLM:

```rust
pub fn read_tool_description() -> &'static str {
    r#"Read a file from disk. Returns the file contents with line numbers.

Parameters:
- path (string, required): Absolute path to the file
- offset (integer, optional): Line number to start reading from (default: 0)
- limit (integer, optional): Maximum number of lines to return (default: 2000)

Notes:
- Files larger than 10MB will be rejected
- Binary files cannot be read with this tool
- Files longer than 2000 lines are truncated by default; use offset to paginate
- The total line count is always shown so you can request specific ranges"#
}
```

By encoding limits in the tool description, the LLM can make informed decisions about how to approach large files. It knows to use pagination rather than trying to read everything at once.

::: wild In the Wild
Claude Code caps file reads at a configurable line limit (defaulting to around 2000 lines) and tells the model how many total lines the file has. This lets the model paginate through large files by requesting specific offsets. The model learns to read only the relevant sections rather than consuming entire large files, which preserves context window budget for the conversation.
:::

## Key Takeaways

- Classify files by both memory size and context size -- a 10,000-line file is small for memory but very large for LLM context
- Provide file metadata (total lines, byte size) in the read output so the LLM can make informed decisions about which sections to request
- Use `BufReader` for streaming reads and `mmap` for random access on very large files
- Stream edits through a temp file for large file modifications, writing unchanged lines and replacement content without holding the whole file in memory
- Communicate file size limits in tool descriptions so the LLM knows the constraints before it attempts to read or edit
