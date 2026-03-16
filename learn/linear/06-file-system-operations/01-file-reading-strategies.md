---
title: File Reading Strategies
description: Techniques for reading files efficiently in an agent context, including full reads, line-range selection, and memory-conscious approaches.
---

# File Reading Strategies

> **What you'll learn:**
> - How to implement a file read tool that supports full file, line-range, and offset-based reading modes
> - When to read entire files versus partial reads and how this decision affects context window usage
> - How to detect binary files, handle encoding, and add line numbers for LLM consumption

A coding agent's read tool is the first thing it reaches for. Before it can edit a file, generate tests, or reason about a bug, it needs to see the source code. But reading files for an LLM is not the same as reading files for a human -- you need to think about context window budgets, line numbers for precise edits later, binary file detection, and the choice between loading everything at once versus streaming pieces on demand.

In this subchapter, you'll build a file reading function that covers the common patterns you'll find in production agents. Let's start with the simplest case and work our way up.

## Reading an Entire File

The most straightforward approach is reading the whole file into a `String`. Rust's standard library makes this a one-liner:

```rust
use std::fs;
use std::path::Path;

fn read_file(path: &Path) -> Result<String, std::io::Error> {
    fs::read_to_string(path)
}
```

This reads the entire file into memory as a UTF-8 string. If the file contains invalid UTF-8 bytes, it returns an error. For most source code files -- Rust, Python, JavaScript, TOML, JSON -- this works perfectly.

::: tip Coming from Python
In Python you would write `Path("file.txt").read_text()` or use `open("file.txt").read()`. Rust's `fs::read_to_string` is the direct equivalent. The key difference: Python's `open()` defaults to the platform encoding (often UTF-8 on modern systems but historically locale-dependent), while Rust's `read_to_string` strictly requires valid UTF-8 and returns an error otherwise. There is no silent encoding fallback.
:::

## Adding Line Numbers

When an LLM reads a file, it needs line numbers to reference specific locations. If you later ask it to edit line 42, it needs to have seen "42" next to that line. Here is a function that reads a file and prepends line numbers:

```rust
use std::fs;
use std::path::Path;

fn read_file_with_line_numbers(path: &Path) -> Result<String, std::io::Error> {
    let content = fs::read_to_string(path)?;
    let numbered: String = content
        .lines()
        .enumerate()
        .map(|(i, line)| format!("{:>6}\t{}", i + 1, line))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(numbered)
}
```

The `{:>6}` format right-aligns the line number in a 6-character column, which keeps the code aligned up to 999,999 lines. The tab character separates the line number from the content, making it easy for the LLM to distinguish metadata from actual code.

## Line-Range Reading

Full file reads waste context window tokens when the LLM only needs a specific section. A line-range read lets you specify a start and end line:

```rust
use std::fs;
use std::path::Path;

fn read_file_range(
    path: &Path,
    start_line: usize,
    end_line: usize,
) -> Result<String, std::io::Error> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    // Clamp to valid range (1-indexed input)
    let start = start_line.saturating_sub(1).min(total_lines);
    let end = end_line.min(total_lines);

    let numbered: String = lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{:>6}\t{}", start + i + 1, line))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(numbered)
}
```

This still reads the entire file into memory, then slices it. For files under a few megabytes, this is perfectly acceptable. The alternative -- seeking to a byte offset -- is faster but requires you to know the byte position of each line, which means you would need an index. We'll address truly large files in [Large File Handling](/linear/06-file-system-operations/07-large-file-handling).

## Detecting Binary Files

You do not want your agent trying to read a compiled binary, an image, or a `.wasm` file as text. A simple heuristic is to check the first few kilobytes for null bytes:

```rust
use std::fs::File;
use std::io::Read;
use std::path::Path;

fn is_binary_file(path: &Path) -> Result<bool, std::io::Error> {
    let mut file = File::open(path)?;
    let mut buffer = [0u8; 8192];
    let bytes_read = file.read(&mut buffer)?;

    // If we find a null byte in the first 8KB, it's likely binary
    Ok(buffer[..bytes_read].contains(&0))
}
```

This is the same heuristic that Git uses to detect binary files. It is not perfect -- some exotic text encodings may contain null bytes -- but it covers the vast majority of cases you'll encounter in a codebase.

## Streaming with BufReader

When you read with `fs::read_to_string`, Rust allocates a single `String` large enough to hold the whole file. For a 10 MB log file, that is 10 MB of heap allocation. Using `BufReader` lets you process lines one at a time:

```rust
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

fn read_file_streaming(
    path: &Path,
    max_lines: usize,
) -> Result<String, std::io::Error> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut output = String::new();

    for (i, line_result) in reader.lines().enumerate() {
        if i >= max_lines {
            output.push_str(&format!(
                "\n... truncated after {} lines\n",
                max_lines
            ));
            break;
        }
        let line = line_result?;
        output.push_str(&format!("{:>6}\t{}\n", i + 1, line));
    }

    Ok(output)
}
```

`BufReader` wraps the file handle with an internal 8 KB buffer. It reads 8 KB at a time from the OS, then yields individual lines from that buffer. This means you never hold more than 8 KB of file data plus the output string in memory at once. The `max_lines` cap prevents a runaway read from consuming your entire context window budget.

::: tip Coming from Python
Python's `open()` returns a file object that you can iterate line by line: `for line in open("file.txt"):`. This is buffered by default and conceptually identical to Rust's `BufReader`. The difference is that Rust forces you to be explicit about buffering -- `File::open` returns an unbuffered handle, and you wrap it in `BufReader` yourself.
:::

## Putting It Together: A Read Tool

A production agent read tool combines these strategies. Here is a complete function that handles the common cases:

```rust
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MB
const MAX_LINES_DEFAULT: usize = 2000;

pub fn read_tool(
    path: &Path,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<String, String> {
    // Check if file exists
    if !path.exists() {
        return Err(format!("File not found: {}", path.display()));
    }

    // Check file size
    let metadata = fs::metadata(path)
        .map_err(|e| format!("Cannot read metadata: {e}"))?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(format!(
            "File is {} bytes, exceeding the {} byte limit",
            metadata.len(),
            MAX_FILE_SIZE
        ));
    }

    // Detect binary
    let mut file = File::open(path)
        .map_err(|e| format!("Cannot open file: {e}"))?;
    let mut header = [0u8; 8192];
    let header_len = file.read(&mut header)
        .map_err(|e| format!("Cannot read file: {e}"))?;
    if header[..header_len].contains(&0) {
        return Err("Binary file detected, cannot display as text".into());
    }

    // Read the full content as a string
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Cannot read file as UTF-8: {e}"))?;
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    let start = offset.unwrap_or(0).min(total_lines);
    let max_lines = limit.unwrap_or(MAX_LINES_DEFAULT);
    let end = (start + max_lines).min(total_lines);

    let mut output = String::new();
    for (i, line) in lines[start..end].iter().enumerate() {
        output.push_str(&format!("{:>6}\t{}\n", start + i + 1, line));
    }

    if end < total_lines {
        output.push_str(&format!(
            "\n... {} more lines not shown (total: {})\n",
            total_lines - end,
            total_lines
        ));
    }

    Ok(output)
}
```

This function: checks for file existence, enforces a size limit, detects binary content, reads the file as UTF-8, applies offset and limit parameters, and adds line numbers. The LLM sees a clean, numbered view of the file with clear indicators when content is truncated.

::: details Why 2000 lines as the default limit?
Most LLM context windows can comfortably hold 2000 lines of source code (roughly 40,000-60,000 tokens depending on the code). Going beyond this starts to crowd out room for the conversation history and system prompt. Claude Code uses a similar default, showing around 2000 lines per read and telling the model how many additional lines exist.
:::

::: wild In the Wild
Claude Code's Read tool provides line numbers in `cat -n` format and supports an offset parameter for pagination. It also tells the model the total line count so the LLM can decide whether to request additional pages. OpenCode takes a similar approach, limiting output to a configurable maximum and appending a truncation notice.
:::

## Key Takeaways

- Use `fs::read_to_string` for simple, full-file reads of UTF-8 text -- it is the Rust equivalent of Python's `Path.read_text()`
- Always add line numbers when presenting file content to an LLM -- the model needs them to reference specific locations in later edit operations
- Implement binary detection (null-byte check in first 8 KB) and file size limits to prevent the agent from loading inappropriate content
- Use `BufReader` for streaming reads when you need to cap line counts or handle files that could be large
- A production read tool combines size checks, binary detection, line numbering, and offset/limit pagination into a single cohesive function
