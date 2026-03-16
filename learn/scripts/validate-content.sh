#!/usr/bin/env bash
# Validates learn/ content against authoring standards.
# Run from the learn/ directory: bash scripts/validate-content.sh

set -euo pipefail
cd "$(dirname "$0")/.."

errors=0
warnings=0

err()  { echo "ERROR: $1"; errors=$((errors + 1)); }
warn() { echo "WARN:  $1"; warnings=$((warnings + 1)); }

echo "=== Content Validation ==="
echo

# 1. Frontmatter: every .md (except index.md, README.md) needs title + description
echo "-- Checking frontmatter ---"
for f in project/*/*.md linear/*/*.md; do
    [[ "$(basename "$f")" == "index.md" ]] && continue
    head -1 "$f" | grep -q '^---' || { err "$f: missing frontmatter"; continue; }
    head -10 "$f" | grep -q '^title:' || err "$f: missing 'title' in frontmatter"
    head -10 "$f" | grep -q '^description:' || err "$f: missing 'description' in frontmatter"
done

# 2. "What you'll learn" block in non-index, non-summary subchapters
echo "-- Checking 'What you'll learn' blocks ---"
for f in project/*/*.md linear/*/*.md; do
    base="$(basename "$f")"
    [[ "$base" == "index.md" ]] && continue
    [[ "$base" == *summary* ]] && continue
    grep -q "What you'll learn" "$f" || warn "$f: missing 'What you'll learn' block"
done

# 3. Key Takeaways in non-index subchapters
echo "-- Checking 'Key Takeaways' sections ---"
for f in project/*/*.md linear/*/*.md; do
    [[ "$(basename "$f")" == "index.md" ]] && continue
    grep -q "Key Takeaways" "$f" || warn "$f: missing 'Key Takeaways' section"
done

# 4. Exercises in summary files
echo "-- Checking exercises in summary files ---"
for f in project/*/1[24]-summary*.md linear/*/1[24]-summary*.md; do
    [ -f "$f" ] || continue
    grep -q "## Exercises" "$f" || err "$f: missing '## Exercises' section"
done

# 5. No incorrect callout syntax
echo "-- Checking callout syntax ---"
bad_callouts=$(grep -rl '::: tip Coming from Python\|::: tip In the Wild\|::: info In the Wild' project/ linear/ 2>/dev/null || true)
if [ -n "$bad_callouts" ]; then
    while IFS= read -r f; do
        err "$f: incorrect callout syntax (use ::: python or ::: wild)"
    done <<< "$bad_callouts"
fi

# 6. No TODO markers in content
echo "-- Checking for TODO markers ---"
todo_files=$(grep -rl '<!-- TODO\|// TODO\|# TODO' project/ linear/ 2>/dev/null || true)
if [ -n "$todo_files" ]; then
    while IFS= read -r f; do
        warn "$f: contains TODO marker"
    done <<< "$todo_files"
fi

# 7. Code snapshots compile
echo "-- Checking code snapshots ---"
for ch in code/ch*/; do
    [ -f "$ch/Cargo.toml" ] || continue
    chname="$(basename "$ch")"
    if ! cargo check --manifest-path "$ch/Cargo.toml" --quiet 2>/dev/null; then
        err "$chname: cargo check failed"
    fi
done

echo
echo "=== Results ==="
echo "Errors:   $errors"
echo "Warnings: $warnings"
[ "$errors" -eq 0 ] && echo "All checks passed." || echo "Fix errors above."
exit "$errors"
