---
title: Encoding and Unicode
description: Handling character encoding detection, UTF-8 validation, BOM handling, and line ending normalization in file operations.
---

# Encoding and Unicode

> **What you'll learn:**
> - How to detect file encoding and handle the common cases of UTF-8, UTF-16, Latin-1, and mixed encoding files
> - Why UTF-8 validation matters in Rust (where strings must be valid UTF-8) and how to handle invalid sequences
> - How to normalize line endings across platforms and handle BOM (byte order mark) in file reading and writing

Character encoding is one of those problems that seems simple until it isn't. Most modern codebases use UTF-8, and Rust's `String` type requires valid UTF-8. But real-world codebases contain legacy files in Latin-1, configuration files with Windows line endings, XML files with UTF-16 encoding, and occasional files with byte-order marks. Your agent needs to handle all of these gracefully rather than crashing on the first non-UTF-8 byte.

## Rust's String Model

Rust enforces that `String` and `&str` always contain valid UTF-8. This is fundamentally different from Python 3's `str` (which represents Unicode code points internally as UCS-2 or UCS-4) and from C's `char*` (which is just bytes with no encoding guarantee).

```rust
fn string_basics() {
    // This works -- valid UTF-8
    let greeting = String::from("Hello, ");

    // This also works -- emoji are valid UTF-8
    let emoji = String::from("Hello ");

    // String::from_utf8 validates the bytes
    let valid_bytes = vec![72, 101, 108, 108, 111]; // "Hello"
    let s = String::from_utf8(valid_bytes).unwrap();
    assert_eq!(s, "Hello");

    // Invalid UTF-8 bytes cause an error, not silent corruption
    let invalid_bytes = vec![0xFF, 0xFE, 0x48, 0x69];
    let result = String::from_utf8(invalid_bytes);
    assert!(result.is_err());
}
```

This strictness is a feature, not a bug. When your agent reads a file into a `String`, you know the content is valid UTF-8. When it passes that string to the LLM API, there will be no encoding errors downstream.

::: python Coming from Python
Python 3 made the `str`/`bytes` split, which was a huge improvement over Python 2's ambiguous `str` type. Rust takes a similar approach: `String` is always Unicode text, and `Vec<u8>` is arbitrary bytes. The difference is that Python's `str` can represent any Unicode code point, while Rust's `String` is specifically UTF-8 encoded. This means Rust strings have the nice property that indexing by byte position is O(1), but you can't index by character position without iterating because UTF-8 characters can be 1-4 bytes long.
:::

## Reading Non-UTF-8 Files

When `fs::read_to_string` encounters invalid UTF-8, it returns an error. You have several options:

### Option 1: Lossy Conversion

Replace invalid bytes with the Unicode replacement character:

```rust
use std::fs;
use std::path::Path;

fn read_file_lossy(path: &Path) -> Result<String, std::io::Error> {
    let bytes = fs::read(path)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
```

`from_utf8_lossy` replaces each invalid byte sequence with the replacement character. This is safe for displaying content to the LLM but lossy -- if you write the content back, the original bytes are lost.

### Option 2: Encoding Detection and Conversion

Use the `encoding_rs` crate to detect and convert encodings:

```rust
use encoding_rs::Encoding;
use std::fs;
use std::path::Path;

fn read_file_with_encoding(
    path: &Path,
    encoding_name: Option<&str>,
) -> Result<String, String> {
    let bytes = fs::read(path)
        .map_err(|e| format!("Cannot read file: {e}"))?;

    // Try to detect encoding from BOM or use specified encoding
    let encoding = if let Some(name) = encoding_name {
        Encoding::for_label(name.as_bytes())
            .ok_or_else(|| format!("Unknown encoding: {name}"))?
    } else {
        detect_encoding(&bytes)
    };

    let (text, _encoding_used, had_errors) = encoding.decode(&bytes);

    if had_errors {
        // Some bytes couldn't be decoded -- warn but continue
        Ok(format!(
            "Warning: some bytes could not be decoded from {}\n{}",
            encoding.name(),
            text
        ))
    } else {
        Ok(text.into_owned())
    }
}

fn detect_encoding(bytes: &[u8]) -> &'static Encoding {
    // Check for BOM (Byte Order Mark)
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return encoding_rs::UTF_8;
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return encoding_rs::UTF_16LE;
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return encoding_rs::UTF_16BE;
    }

    // Try UTF-8 first (most common for source code)
    if std::str::from_utf8(bytes).is_ok() {
        return encoding_rs::UTF_8;
    }

    // Fall back to Windows-1252 (Latin-1 superset) for Western text
    encoding_rs::WINDOWS_1252
}
```

The `encoding_rs` crate is the Rust equivalent of Python's `codecs` module. It supports all major encodings and is extremely fast (it's the encoding library used by Firefox).

## Handling the BOM (Byte Order Mark)

The BOM is a special Unicode character (U+FEFF) placed at the beginning of a file to indicate encoding. It's commonly found in files created by Windows tools like Notepad:

```rust
/// Strip the UTF-8 BOM if present at the start of a string
fn strip_bom(content: &str) -> &str {
    content.strip_prefix('\u{FEFF}').unwrap_or(content)
}

/// Strip BOM from raw bytes and return encoding hint
fn detect_and_strip_bom(bytes: &[u8]) -> (&[u8], Option<&str>) {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        (&bytes[3..], Some("utf-8"))
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        (&bytes[2..], Some("utf-16le"))
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        (&bytes[2..], Some("utf-16be"))
    } else {
        (bytes, None)
    }
}
```

When reading a file for the LLM, strip the BOM so it doesn't appear as a mysterious character at the beginning of the content. When writing back, preserve the BOM if the original file had one:

```rust
use std::fs;
use std::path::Path;

pub fn read_preserving_bom(path: &Path) -> Result<(String, bool), String> {
    let bytes = fs::read(path)
        .map_err(|e| format!("Cannot read: {e}"))?;

    let has_bom = bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
    let text_bytes = if has_bom { &bytes[3..] } else { &bytes };

    let content = String::from_utf8(text_bytes.to_vec())
        .map_err(|e| format!("Invalid UTF-8: {e}"))?;

    Ok((content, has_bom))
}

pub fn write_preserving_bom(
    path: &Path,
    content: &str,
    had_bom: bool,
) -> Result<(), String> {
    let bytes = if had_bom {
        let mut v = vec![0xEF, 0xBB, 0xBF];
        v.extend_from_slice(content.as_bytes());
        v
    } else {
        content.as_bytes().to_vec()
    };

    fs::write(path, bytes)
        .map_err(|e| format!("Cannot write: {e}"))
}
```

## Line Ending Normalization

Different platforms use different line endings:
- Unix/macOS: `\n` (LF, line feed)
- Windows: `\r\n` (CRLF, carriage return + line feed)
- Classic Mac (pre-OS X): `\r` (CR, carriage return only -- very rare now)

When your agent reads a file, you should normalize line endings for consistent processing and preserve the original style when writing back:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineEnding {
    Lf,   // Unix: \n
    CrLf, // Windows: \r\n
    Mixed, // Both styles present
}

pub fn detect_line_ending(content: &str) -> LineEnding {
    let crlf_count = content.matches("\r\n").count();
    let lf_only_count = content.matches('\n').count() - crlf_count;

    if crlf_count > 0 && lf_only_count > 0 {
        LineEnding::Mixed
    } else if crlf_count > 0 {
        LineEnding::CrLf
    } else {
        LineEnding::Lf
    }
}

pub fn normalize_to_lf(content: &str) -> String {
    content.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn apply_line_ending(content: &str, ending: LineEnding) -> String {
    let normalized = normalize_to_lf(content);
    match ending {
        LineEnding::Lf | LineEnding::Mixed => normalized,
        LineEnding::CrLf => normalized.replace('\n', "\r\n"),
    }
}
```

For mixed line endings, it's generally best to normalize to the dominant style or to LF. The LLM doesn't care about line endings (it sees `\n` as a token boundary), so normalization during reading is safe. Just remember to restore the original style when writing back.

::: wild In the Wild
Claude Code normalizes line endings internally and preserves the original line ending style when writing back. This prevents the common problem where an agent edit inadvertently converts an entire file from CRLF to LF (or vice versa), creating a noisy diff that touches every line. Git's `core.autocrlf` setting handles some of this, but the agent shouldn't rely on Git configuration being set correctly.
:::

## Unicode Normalization

Unicode has multiple ways to represent the same visual character. For example, "e" with an accent can be:
- U+00E9 (precomposed: a single code point)
- U+0065 U+0301 (decomposed: 'e' followed by combining accent)

This can affect string matching in your edit tool:

```rust
fn demonstrate_normalization() {
    let precomposed = "\u{00E9}"; // single code point
    let decomposed = "\u{0065}\u{0301}"; // e + combining accent

    // They look the same when rendered but are different bytes
    assert_ne!(precomposed, decomposed);

    // For file editing, this means string replacement might fail
    // if the model generates one form and the file contains the other
}
```

In practice, this is rarely a problem for source code (which uses ASCII identifiers) but can be an issue with comments, string literals, or documentation files. If you encounter matching failures that look like they should succeed, Unicode normalization might be the cause.

The `unicode-normalization` crate can normalize both strings to the same form before comparison:

```rust
use unicode_normalization::UnicodeNormalization;

fn normalize_for_matching(s: &str) -> String {
    s.nfc().collect::<String>() // NFC = canonical decomposition + composition
}
```

## Key Takeaways

- Rust's `String` is always valid UTF-8 -- use `from_utf8_lossy` for best-effort reading and `encoding_rs` for proper encoding conversion
- Detect and strip BOMs when reading, then restore them when writing to avoid introducing or removing BOMs inadvertently
- Normalize line endings to LF internally and restore the original style (LF or CRLF) when writing back to prevent noisy diffs
- Encoding detection heuristics (BOM check, UTF-8 validation, fallback to Windows-1252) handle the vast majority of real-world files
- Unicode normalization differences (NFC vs. NFD) can cause string matching failures with non-ASCII text -- normalize before comparing when needed
