---
title: Extension Marketplace Concepts
description: Exploring the design of an extension marketplace for discovering, installing, and updating community-contributed plugins, including trust, versioning, and distribution concerns.
---

# Extension Marketplace Concepts

> **What you'll learn:**
> - How to design a plugin discovery and installation workflow for community extensions
> - The trust and security challenges of running third-party code in a coding agent
> - Patterns for plugin versioning, compatibility checking, and update distribution

You have built the infrastructure for plugins, skills, MCP servers, and config-driven extensions. The next question is: how do users find and install extensions built by others? An extension marketplace is the discovery and distribution layer that connects plugin developers with users. While building a full marketplace is a product-level effort, understanding the design patterns prepares you to build one when the time comes.

## The Extension Package Format

Before distribution, you need a standard package format. An extension package is a zip archive with a well-known structure:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The manifest file (extension.toml) that every extension package must contain.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExtensionPackage {
    pub metadata: PackageMetadata,
    #[serde(default)]
    pub capabilities: PackageCapabilities,
    #[serde(default)]
    pub dependencies: Vec<PackageDependency>,
    #[serde(default)]
    pub compatibility: Compatibility,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub name: String,
    pub display_name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub license: String,
    pub repository: Option<String>,
    pub homepage: Option<String>,
    pub keywords: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PackageCapabilities {
    /// Tool definitions to register.
    #[serde(default)]
    pub tools: Vec<ToolConfig>,
    /// MCP server to launch.
    #[serde(default)]
    pub mcp_server: Option<McpServerConfig>,
    /// Skill definitions.
    #[serde(default)]
    pub skills: Vec<SkillDefinition>,
    /// Custom commands.
    #[serde(default)]
    pub commands: Vec<CommandConfig>,
    /// Hooks to register.
    #[serde(default)]
    pub hooks: HookConfigs,
    /// Prompt additions.
    #[serde(default)]
    pub prompts: Vec<PromptConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageDependency {
    pub name: String,
    pub version_req: String, // semver like ">=1.0.0, <2.0.0"
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Compatibility {
    /// Minimum agent version required.
    pub min_agent_version: Option<String>,
    /// Maximum agent version supported.
    pub max_agent_version: Option<String>,
    /// Supported platforms.
    #[serde(default)]
    pub platforms: Vec<String>, // "macos", "linux", "windows"
}
```

A real package on disk looks like this:

```
my-extension-1.0.0.zip
├── extension.toml          # Package manifest
├── tools/
│   └── custom_tool.toml    # Tool definitions
├── skills/
│   └── domain_skill.toml   # Skill definitions
├── prompts/
│   └── system_additions.md # Prompt text
├── scripts/
│   └── hook_check.py       # Hook scripts
└── README.md               # Documentation
```

## The Registry API

A marketplace needs a registry -- a server that stores package metadata and allows searching:

```rust
use serde::{Deserialize, Serialize};

/// A client for the extension registry API.
pub struct RegistryClient {
    base_url: String,
    http_client: reqwest::Client,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub packages: Vec<PackageListing>,
    pub total_count: usize,
    pub page: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageListing {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub latest_version: String,
    pub author: String,
    pub downloads: u64,
    pub rating: f32,
    pub verified: bool,
    pub keywords: Vec<String>,
}

impl RegistryClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            http_client: reqwest::Client::new(),
        }
    }

    /// Search for extensions by keyword.
    pub async fn search(
        &self,
        query: &str,
        page: usize,
    ) -> Result<SearchResult, anyhow::Error> {
        let url = format!(
            "{}/api/v1/extensions/search?q={}&page={}",
            self.base_url,
            urlencoding::encode(query),
            page
        );

        let response = self
            .http_client
            .get(&url)
            .send()
            .await?
            .json::<SearchResult>()
            .await?;

        Ok(response)
    }

    /// Get the details and download URL for a specific package version.
    pub async fn get_package(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<PackageDetails, anyhow::Error> {
        let version_part = version.unwrap_or("latest");
        let url = format!(
            "{}/api/v1/extensions/{}/{}",
            self.base_url, name, version_part
        );

        let response = self
            .http_client
            .get(&url)
            .send()
            .await?
            .json::<PackageDetails>()
            .await?;

        Ok(response)
    }

    /// Download a package archive.
    pub async fn download(
        &self,
        name: &str,
        version: &str,
        dest: &std::path::Path,
    ) -> Result<std::path::PathBuf, anyhow::Error> {
        let url = format!(
            "{}/api/v1/extensions/{}/{}/download",
            self.base_url, name, version
        );

        let response = self.http_client.get(&url).send().await?;
        let bytes = response.bytes().await?;

        let file_path = dest.join(format!("{}-{}.zip", name, version));
        tokio::fs::write(&file_path, bytes).await?;

        Ok(file_path)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageDetails {
    pub metadata: PackageMetadata,
    pub versions: Vec<VersionInfo>,
    pub download_url: String,
    pub checksum_sha256: String,
    pub verified: bool,
    pub trust_level: TrustLevel,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionInfo {
    pub version: String,
    pub published_at: String,
    pub downloads: u64,
    pub compatible_agent_versions: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TrustLevel {
    /// Published by the agent team.
    Official,
    /// Published by a verified organization.
    Verified,
    /// Community-contributed, not verified.
    Community,
    /// Unknown or new publisher.
    Unverified,
}
```

::: tip Coming from Python
Python's PyPI and `pip install` is the canonical example of a package registry:
```python
# Install a package
pip install some-extension

# Search (deprecated in PyPI, but the pattern exists)
pip search "coding agent extension"
```
The extension marketplace follows the same model: a central registry, a CLI install command, and semver versioning. The key difference is that agent extensions can execute arbitrary code with access to your filesystem and network, so the trust model is far more important than with most Python packages. A malicious pytest plugin is bad; a malicious coding agent plugin with shell access is catastrophic.
:::

## The Installation Manager

The installation manager handles downloading, verifying, extracting, and activating extensions:

```rust
use std::path::{Path, PathBuf};

pub struct InstallationManager {
    extensions_dir: PathBuf,
    registry: RegistryClient,
    installed: HashMap<String, InstalledExtension>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledExtension {
    pub name: String,
    pub version: String,
    pub install_path: PathBuf,
    pub trust_level: TrustLevel,
    pub enabled: bool,
}

impl InstallationManager {
    pub fn new(extensions_dir: PathBuf, registry_url: &str) -> Self {
        Self {
            extensions_dir,
            registry: RegistryClient::new(registry_url),
            installed: HashMap::new(),
        }
    }

    /// Install an extension from the registry.
    pub async fn install(
        &mut self,
        name: &str,
        version: Option<&str>,
    ) -> Result<InstalledExtension, anyhow::Error> {
        println!("[marketplace] Installing extension '{}'...", name);

        // Fetch package details
        let details = self.registry.get_package(name, version).await?;

        // Check compatibility
        self.check_compatibility(&details)?;

        // Display trust warning for unverified packages
        if matches!(details.trust_level, TrustLevel::Unverified) {
            println!(
                "WARNING: Extension '{}' is from an unverified publisher. \
                 It may execute arbitrary code on your system.",
                name
            );
        }

        // Download the package
        let archive_path = self
            .registry
            .download(name, &details.metadata.version, &self.extensions_dir)
            .await?;

        // Verify checksum
        self.verify_checksum(&archive_path, &details.checksum_sha256)?;

        // Extract to extensions directory
        let install_path = self
            .extensions_dir
            .join(name)
            .join(&details.metadata.version);

        self.extract_archive(&archive_path, &install_path)?;

        // Clean up the archive
        tokio::fs::remove_file(&archive_path).await?;

        // Register as installed
        let installed = InstalledExtension {
            name: name.to_string(),
            version: details.metadata.version.clone(),
            install_path: install_path.clone(),
            trust_level: details.trust_level,
            enabled: true,
        };

        self.installed.insert(name.to_string(), installed.clone());
        self.save_installed_manifest()?;

        println!(
            "[marketplace] Extension '{}' v{} installed at {:?}",
            name, details.metadata.version, install_path
        );

        Ok(installed)
    }

    /// Uninstall an extension.
    pub async fn uninstall(&mut self, name: &str) -> Result<(), anyhow::Error> {
        let extension = self
            .installed
            .remove(name)
            .ok_or_else(|| anyhow::anyhow!("Extension '{}' is not installed", name))?;

        // Remove the files
        if extension.install_path.exists() {
            tokio::fs::remove_dir_all(&extension.install_path).await?;
        }

        self.save_installed_manifest()?;
        println!("[marketplace] Extension '{}' uninstalled", name);
        Ok(())
    }

    /// Check for available updates.
    pub async fn check_updates(&self) -> Result<Vec<UpdateAvailable>, anyhow::Error> {
        let mut updates = Vec::new();

        for (name, installed) in &self.installed {
            if let Ok(details) = self.registry.get_package(name, None).await {
                if details.metadata.version != installed.version {
                    updates.push(UpdateAvailable {
                        name: name.clone(),
                        current_version: installed.version.clone(),
                        latest_version: details.metadata.version,
                    });
                }
            }
        }

        Ok(updates)
    }

    fn check_compatibility(
        &self,
        _details: &PackageDetails,
    ) -> Result<(), anyhow::Error> {
        // Check agent version compatibility
        // Check platform compatibility
        // In practice, compare semver ranges
        Ok(())
    }

    fn verify_checksum(
        &self,
        _path: &Path,
        _expected: &str,
    ) -> Result<(), anyhow::Error> {
        // In practice: compute SHA256 of the file and compare
        // use sha2::{Sha256, Digest};
        // let mut hasher = Sha256::new();
        // let bytes = std::fs::read(path)?;
        // hasher.update(&bytes);
        // let result = format!("{:x}", hasher.finalize());
        // assert_eq!(result, expected);
        Ok(())
    }

    fn extract_archive(
        &self,
        _archive: &Path,
        _dest: &Path,
    ) -> Result<(), anyhow::Error> {
        // In practice: use the `zip` crate to extract
        // let file = std::fs::File::open(archive)?;
        // let mut archive = zip::ZipArchive::new(file)?;
        // archive.extract(dest)?;
        Ok(())
    }

    fn save_installed_manifest(&self) -> Result<(), anyhow::Error> {
        let manifest_path = self.extensions_dir.join("installed.json");
        let json = serde_json::to_string_pretty(&self.installed)?;
        std::fs::write(manifest_path, json)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct UpdateAvailable {
    pub name: String,
    pub current_version: String,
    pub latest_version: String,
}
```

## Trust and Security

Running third-party code in a coding agent is inherently risky. The agent typically has access to the filesystem, network, and shell. A malicious extension could exfiltrate code, delete files, or install backdoors. Your marketplace needs multiple layers of trust:

```rust
/// Security review status of an extension.
#[derive(Debug, Serialize, Deserialize)]
pub struct SecurityReview {
    pub reviewed_version: String,
    pub reviewer: String,
    pub review_date: String,
    pub findings: Vec<SecurityFinding>,
    pub approval_status: ApprovalStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SecurityFinding {
    pub severity: Severity,
    pub description: String,
    pub file: String,
    pub line: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ApprovalStatus {
    Approved,
    ConditionallyApproved(String), // with conditions
    Rejected(String),              // with reason
    Pending,
}
```

The trust model has several layers:

| Trust Level | Who | Verification | Isolation |
|-------------|-----|-------------|-----------|
| **Official** | Agent team | Full code review | In-process |
| **Verified** | Known organizations | Identity verified, automated scanning | In-process with monitoring |
| **Community** | Anyone | Automated scanning only | Process isolation (MCP/subprocess) |
| **Unverified** | Unknown | None | Process isolation + resource limits |

::: info In the Wild
The VS Code marketplace provides a useful reference. Extensions are published by anyone but verified publishers get a blue checkmark. VS Code extensions run in a shared process (the extension host) with access to the workspace filesystem. This has led to real security incidents where malicious extensions harvested credentials. Coding agents face even higher stakes because they have shell access. The MCP model partially addresses this -- MCP servers run as separate processes with only the capabilities they explicitly declare, providing natural sandboxing.
:::

## User-Facing Commands

Expose marketplace functionality through slash commands:

```rust
pub struct MarketplaceCommand {
    metadata: CommandMetadata,
}

impl MarketplaceCommand {
    pub fn new() -> Self {
        Self {
            metadata: CommandMetadata {
                name: "extensions".to_string(),
                summary: "Search, install, and manage extensions".to_string(),
                usage: "/extensions [search|install|uninstall|list|update] [args]".to_string(),
                arguments: vec![
                    CommandArgument {
                        name: "action".to_string(),
                        description: "Action to perform".to_string(),
                        required: true,
                        default_value: None,
                    },
                ],
                owner: "core".to_string(),
                hidden: false,
            },
        }
    }
}

#[async_trait]
impl SlashCommand for MarketplaceCommand {
    fn metadata(&self) -> &CommandMetadata {
        &self.metadata
    }

    async fn execute(
        &self,
        args: &[String],
        _context: &CommandContext,
    ) -> CommandResult {
        match args.first().map(|s| s.as_str()) {
            Some("search") => {
                let query = args.get(1).map(|s| s.as_str()).unwrap_or("");
                CommandResult::Output(format!(
                    "Searching for extensions matching '{}'...\n\
                     (In production, this queries the registry API)",
                    query
                ))
            }
            Some("install") => {
                if let Some(name) = args.get(1) {
                    CommandResult::Output(format!(
                        "Installing extension '{}'...\n\
                         (In production, this downloads and installs the package)",
                        name
                    ))
                } else {
                    CommandResult::Error(
                        "Usage: /extensions install <name>".to_string()
                    )
                }
            }
            Some("list") => {
                CommandResult::Output(
                    "Installed extensions:\n  (none)\n\n\
                     Use /extensions install <name> to install extensions."
                        .to_string(),
                )
            }
            Some("update") => {
                CommandResult::Output(
                    "Checking for updates...\n\
                     All extensions are up to date."
                        .to_string(),
                )
            }
            Some("uninstall") => {
                if let Some(name) = args.get(1) {
                    CommandResult::Output(format!(
                        "Uninstalling extension '{}'...",
                        name
                    ))
                } else {
                    CommandResult::Error(
                        "Usage: /extensions uninstall <name>".to_string()
                    )
                }
            }
            _ => CommandResult::Error(
                "Usage: /extensions [search|install|uninstall|list|update]"
                    .to_string(),
            ),
        }
    }

    fn completions(&self, partial: &str, arg_index: usize) -> Vec<String> {
        if arg_index == 0 {
            vec!["search", "install", "uninstall", "list", "update"]
                .into_iter()
                .filter(|a| a.starts_with(partial))
                .map(String::from)
                .collect()
        } else {
            Vec::new()
        }
    }
}
```

## Key Takeaways

- An extension marketplace has three core components: a package format (standardized archive with a manifest), a registry API (search, download, version management), and an installation manager (download, verify, extract, activate)
- Trust is the central challenge: coding agent extensions have filesystem and shell access, making security review and tiered trust levels (official, verified, community, unverified) essential
- Checksum verification ensures packages are not tampered with during download, and semver compatibility checking prevents extensions from breaking with agent updates
- Process isolation (MCP/subprocess) provides natural sandboxing for untrusted community extensions -- the strongest defense against malicious code
- Start with configuration-based distribution (sharing TOML files and MCP server references) before investing in a full marketplace infrastructure
