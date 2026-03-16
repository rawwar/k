---
title: Auto Updates
description: Implement an automatic update system that checks for new versions and applies updates without disrupting active agent sessions.
---

# Auto Updates

> **What you'll learn:**
> - How to implement background version checking that queries GitHub releases or a custom update server without blocking agent startup
> - Techniques for downloading and atomically replacing the running binary, handling platform-specific challenges (file locking on Windows, code signing on macOS)
> - How to design update UX that respects user preferences -- supporting opt-in, opt-out, notify-only, and fully automatic update modes

Once your coding agent is installed on a user's machine, it needs to stay current. Security patches, new features, and bug fixes are only useful if they reach the user. But auto-updates are a sensitive topic: users want control over what runs on their machine, and a bad update can break their workflow. This subchapter covers how to implement an update system that is reliable, respectful, and transparent.

## Update Strategy Options

Before writing code, decide on your update model. Each has different tradeoffs for user trust and adoption:

| Strategy | Description | User Control | Adoption Speed |
|----------|-------------|-------------|----------------|
| **Notify only** | Check for updates, show a message | Highest | Slowest |
| **Prompt** | Ask the user if they want to update now | High | Moderate |
| **Auto-update** | Download and apply automatically | Low | Fastest |
| **Configurable** | Let the user choose their preference | Highest | Varies |

The configurable approach is the right default. Add an `update_mode` setting to your config:

```toml
[updates]
# "notify", "prompt", "auto", "disabled"
mode = "notify"

# How often to check (in hours)
check_interval_hours = 24
```

And the corresponding Rust types:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UpdateConfig {
    pub mode: UpdateMode,
    pub check_interval_hours: u64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum UpdateMode {
    #[default]
    Notify,
    Prompt,
    Auto,
    Disabled,
}
```

::: python Coming from Python
Python tools generally do not auto-update. `pip` has no built-in self-update mechanism (you run `pip install --upgrade pip`), and most CLI tools rely on the user manually running `pip install --upgrade toolname`. The `pipx` tool adds an `upgrade-all` command. Rust binaries have a significant advantage here: since the binary is a single self-contained file, updating means replacing one file. There is no virtual environment, no dependency resolution, and no risk of breaking other installed packages.
:::

## Checking for New Versions

The simplest version check queries the GitHub Releases API for the latest release tag. Perform this check in a background task that does not block agent startup:

```rust
use std::time::{Duration, SystemTime};
use std::path::PathBuf;
use std::fs;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Serialize, Deserialize)]
struct UpdateState {
    last_check: u64,  // Unix timestamp
    latest_version: Option<String>,
    download_url: Option<String>,
}

fn state_file() -> PathBuf {
    dirs::cache_dir()
        .expect("cache dir")
        .join("my-agent")
        .join("update-state.json")
}

async fn should_check_for_updates(config: &UpdateConfig) -> bool {
    if matches!(config.mode, UpdateMode::Disabled) {
        return false;
    }

    let state_path = state_file();
    if let Ok(contents) = fs::read_to_string(&state_path) {
        if let Ok(state) = serde_json::from_str::<UpdateState>(&contents) {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let interval = config.check_interval_hours * 3600;
            return now - state.last_check > interval;
        }
    }

    true  // No state file means we have never checked
}

async fn check_for_updates() -> Result<Option<UpdateInfo>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.github.com/repos/yourname/my-agent/releases/latest")
        .header("User-Agent", format!("my-agent/{}", CURRENT_VERSION))
        .timeout(Duration::from_secs(5))
        .send()
        .await?;

    let release: GitHubRelease = response.json().await?;
    let latest = release.tag_name.trim_start_matches('v');

    if version_is_newer(latest, CURRENT_VERSION) {
        Ok(Some(UpdateInfo {
            version: latest.to_string(),
            release_url: release.html_url,
            assets: release.assets,
        }))
    } else {
        Ok(None)
    }
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Deserialize, Clone)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

struct UpdateInfo {
    version: String,
    release_url: String,
    assets: Vec<GitHubAsset>,
}
```

### Version Comparison

Use the `semver` crate for proper version comparison:

```rust
fn version_is_newer(latest: &str, current: &str) -> bool {
    match (semver::Version::parse(latest), semver::Version::parse(current)) {
        (Ok(latest_ver), Ok(current_ver)) => latest_ver > current_ver,
        _ => false, // If parsing fails, assume no update
    }
}
```

### Non-Blocking Check at Startup

Run the version check in a background task that does not delay the agent's startup:

```rust
pub fn spawn_update_check(config: UpdateConfig) {
    tokio::spawn(async move {
        if !should_check_for_updates(&config).await {
            return;
        }

        match check_for_updates().await {
            Ok(Some(update_info)) => {
                // Save state
                save_update_state(&update_info);

                match config.mode {
                    UpdateMode::Notify => {
                        eprintln!(
                            "\x1b[33mA new version is available: v{}\x1b[0m",
                            update_info.version
                        );
                        eprintln!(
                            "Update with: my-agent update  |  {}",
                            update_info.release_url
                        );
                    }
                    UpdateMode::Prompt => {
                        // The main loop will check for pending updates
                        // and prompt the user at a natural break point
                    }
                    UpdateMode::Auto => {
                        if let Err(e) = perform_update(&update_info).await {
                            eprintln!("Auto-update failed: {e}");
                        }
                    }
                    UpdateMode::Disabled => unreachable!(),
                }
            }
            Ok(None) => { /* Already on latest version */ }
            Err(_) => { /* Network error, silently ignore */ }
        }
    });
}
```

Notice that network errors are silently ignored. The update check must never interfere with the agent's primary functionality. If the user is offline, behind a firewall, or the GitHub API is down, the agent should work exactly as before.

## Downloading and Applying Updates

The update process involves downloading the new binary and replacing the currently installed one. The critical requirement is atomicity: if the download fails or the machine loses power mid-update, the user should still have a working binary.

### Platform Detection for Downloads

Select the right asset from the GitHub Release:

```rust
fn detect_target() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    { "aarch64-apple-darwin" }

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    { "x86_64-apple-darwin" }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    { "x86_64-unknown-linux-musl" }

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    { "aarch64-unknown-linux-musl" }

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    { "x86_64-pc-windows-msvc" }
}

fn find_asset_for_platform(assets: &[GitHubAsset]) -> Option<&GitHubAsset> {
    let target = detect_target();
    assets.iter().find(|a| a.name.contains(target))
}
```

### Atomic Binary Replacement

The safe way to replace a running binary is to write the new version to a temporary file, then rename (move) it over the old one. On Unix, `rename()` is atomic -- there is no moment when neither the old nor the new file exists:

```rust
use std::os::unix::fs::PermissionsExt;

async fn perform_update(
    update_info: &UpdateInfo,
) -> Result<(), Box<dyn std::error::Error>> {
    let asset = find_asset_for_platform(&update_info.assets)
        .ok_or("No binary available for this platform")?;

    let current_exe = std::env::current_exe()?;

    // Download to a temporary file in the same directory
    // (same filesystem guarantees atomic rename)
    let tmp_path = current_exe.with_extension("tmp-update");

    eprintln!("Downloading v{}...", update_info.version);

    let client = reqwest::Client::new();
    let response = client
        .get(&asset.browser_download_url)
        .header("User-Agent", format!("my-agent/{}", CURRENT_VERSION))
        .send()
        .await?;

    let bytes = response.bytes().await?;

    // If the download is a tar.gz, extract the binary
    // For simplicity, assume the asset is the raw binary or handle extraction
    extract_binary_from_archive(&bytes, &tmp_path)?;

    // Set executable permission
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&tmp_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&tmp_path, perms)?;
    }

    // Atomic rename: old binary is replaced in one operation
    #[cfg(unix)]
    {
        fs::rename(&tmp_path, &current_exe)?;
    }

    // Windows cannot rename over a running executable.
    // Instead, rename the old binary and put the new one in place.
    #[cfg(windows)]
    {
        let backup = current_exe.with_extension("old");
        let _ = fs::remove_file(&backup);  // Remove previous backup
        fs::rename(&current_exe, &backup)?;
        fs::rename(&tmp_path, &current_exe)?;
    }

    eprintln!(
        "Updated to v{}. Restart the agent to use the new version.",
        update_info.version
    );

    Ok(())
}
```

::: details Platform challenges
**Windows**: You cannot delete or overwrite a running executable on Windows. The workaround is to rename the running binary to a `.old` extension, put the new binary in the original location, and clean up the `.old` file on next launch.

**macOS**: Apple's Gatekeeper may quarantine downloaded binaries. If your binary is code-signed with an Apple Developer ID, this is not an issue. For unsigned binaries, users may need to run `xattr -d com.apple.quarantine /path/to/binary` after an update.

**Linux**: The simplest case. `rename()` atomically replaces the old binary, and Linux allows deleting a file that is still being executed (the running process keeps a file descriptor to the old inode).
:::

## The `self_update` Crate

For a production-quality update system, the [`self_update`](https://crates.io/crates/self_update) crate provides a tested, maintained implementation:

```toml
[dependencies]
self_update = { version = "0.42", optional = true, features = ["archive-tar", "compression-flate2"] }
```

```rust
#[cfg(feature = "self-update")]
fn update_command() -> Result<(), Box<dyn std::error::Error>> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner("yourname")
        .repo_name("my-agent")
        .bin_name("my-agent")
        .current_version(env!("CARGO_PKG_VERSION"))
        .show_download_progress(true)
        .no_confirm(false)  // Ask before applying
        .build()?
        .update()?;

    println!("Updated to version: {}", status.version());
    Ok(())
}
```

The `self_update` crate handles platform detection, archive extraction, atomic replacement, and progress display. It is the practical choice for most projects.

## Update UX Patterns

The user experience around updates matters as much as the technical implementation. Here are patterns that respect users:

### Notify on Startup (Recommended Default)

```
$ my-agent
A new version is available: v0.6.0 (current: v0.5.2)
Run `my-agent update` to upgrade, or visit:
https://github.com/yourname/my-agent/releases/tag/v0.6.0

>
```

The notification appears once, does not block, and the user can choose to act on it or ignore it.

### Explicit Update Command

Provide a dedicated `update` subcommand:

```bash
$ my-agent update
Checking for updates... found v0.6.0 (current: v0.5.2)
Download and install? [y/N] y
Downloading v0.6.0... done
Updated successfully. Restart to use the new version.
```

### Respect Disabling

Some users work in environments where updates must go through IT approval or change management. Always respect the `disabled` setting and never check the network:

```rust
if matches!(config.updates.mode, UpdateMode::Disabled) {
    // Do nothing. Don't even make an HTTP request.
    return;
}
```

::: wild In the Wild
Claude Code checks for updates and notifies users when a new version is available. The pattern of non-blocking version checks at startup is common across production CLI tools -- it keeps the tool responsive while ensuring users learn about important updates. The key principle is that the update check must never fail loudly or block the user's primary workflow.
:::

## Key Takeaways

- Offer configurable update modes (notify, prompt, auto, disabled) and default to the least intrusive option -- notification only.
- Run version checks in a background task with a short timeout so they never block agent startup or interfere with offline usage.
- Use atomic file operations (rename/move) to replace binaries -- never delete the old binary before the new one is ready.
- Handle platform-specific challenges: Windows cannot overwrite a running executable, macOS may quarantine unsigned downloads.
- The `self_update` crate provides a production-tested implementation covering platform detection, archive extraction, and atomic replacement.
