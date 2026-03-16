---
title: Versioning Plugins
description: Design versioning strategies for the plugin API that allow the agent to evolve without breaking existing extensions.
---

# Versioning Plugins

> **What you'll learn:**
> - How to version your plugin API using semantic versioning and capability-based negotiation so plugins declare which API version they target
> - Techniques for maintaining backward compatibility through API evolution strategies like additive-only changes and deprecation cycles
> - How to implement version checking at plugin load time that gives clear diagnostics when a plugin is incompatible with the current agent version

The moment you publish a plugin API, you make a promise: plugins written against this API will continue to work. Breaking that promise -- even accidentally -- destroys trust in your extension ecosystem. Users who spent days building a custom plugin do not want it to break every time the agent updates. This subchapter covers how to version your plugin API so the agent can evolve without leaving plugins behind.

## Semantic Versioning for Plugin APIs

Semantic versioning (semver) provides a shared vocabulary for communicating compatibility. For a plugin API:

- **Major version** (1.x.x to 2.x.x): Breaking changes. Plugins written for v1 will not work with v2 without modification.
- **Minor version** (1.1.x to 1.2.x): New features added. Plugins written for 1.1 will work with 1.2, but plugins using 1.2 features will not work with 1.1.
- **Patch version** (1.1.1 to 1.1.2): Bug fixes only. No API changes.

```rust
use serde::{Deserialize, Serialize};

/// Semantic version with comparison support.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemVer {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(format!("Invalid semver: '{s}'. Expected format: X.Y.Z"));
        }
        Ok(Self {
            major: parts[0].parse().map_err(|_| format!("Invalid major version in '{s}'"))?,
            minor: parts[1].parse().map_err(|_| format!("Invalid minor version in '{s}'"))?,
            patch: parts[2].parse().map_err(|_| format!("Invalid patch version in '{s}'"))?,
        })
    }
}

impl std::fmt::Display for SemVer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// A version requirement that a plugin declares.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionReq {
    /// Minimum version (inclusive)
    pub min: SemVer,
    /// Maximum major version (exclusive) -- typically min.major + 1
    pub max_major: u32,
}

impl VersionReq {
    /// Create a requirement like ">=1.2.0, <2.0.0"
    pub fn compatible_with(version: &SemVer) -> Self {
        Self {
            min: version.clone(),
            max_major: version.major + 1,
        }
    }

    /// Check if a given version satisfies this requirement.
    pub fn matches(&self, version: &SemVer) -> bool {
        version.major >= self.min.major
            && version.major < self.max_major
            && (version.major > self.min.major
                || version.minor > self.min.minor
                || (version.minor == self.min.minor
                    && version.patch >= self.min.patch))
    }
}
```

## Plugin API Version Declaration

Every plugin declares which version of the plugin API it targets. The agent checks this at load time:

```rust
/// The current version of the plugin API.
pub const PLUGIN_API_VERSION: SemVer = SemVer {
    major: 1,
    minor: 3,
    patch: 0,
};

/// Extended plugin manifest with version information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedPluginManifest {
    pub name: String,
    pub version: SemVer,
    /// Which plugin API version this plugin targets
    pub api_version_req: VersionReq,
    pub capabilities: Vec<Capability>,
}

/// Check whether a plugin is compatible with the current API version.
pub fn check_plugin_compatibility(
    manifest: &VersionedPluginManifest,
) -> CompatibilityResult {
    let current = &PLUGIN_API_VERSION;

    if !manifest.api_version_req.matches(current) {
        return CompatibilityResult::Incompatible {
            plugin_name: manifest.name.clone(),
            plugin_requires: format!("{}", manifest.api_version_req.min),
            agent_provides: format!("{}", current),
            suggestion: if manifest.api_version_req.min.major > current.major {
                "Update the agent to a newer version".to_string()
            } else if manifest.api_version_req.min.major < current.major {
                "Update the plugin to target the new API version".to_string()
            } else {
                format!(
                    "Update the agent to at least version {}",
                    manifest.api_version_req.min
                )
            },
        };
    }

    // Check if the plugin uses features from a newer minor version
    if manifest.api_version_req.min.minor > current.minor {
        return CompatibilityResult::NeedsNewerAgent {
            plugin_name: manifest.name.clone(),
            minimum_version: manifest.api_version_req.min.clone(),
        };
    }

    CompatibilityResult::Compatible
}

#[derive(Debug)]
pub enum CompatibilityResult {
    Compatible,
    NeedsNewerAgent {
        plugin_name: String,
        minimum_version: SemVer,
    },
    Incompatible {
        plugin_name: String,
        plugin_requires: String,
        agent_provides: String,
        suggestion: String,
    },
}

impl std::fmt::Display for CompatibilityResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Compatible => write!(f, "Compatible"),
            Self::NeedsNewerAgent { plugin_name, minimum_version } => {
                write!(
                    f,
                    "Plugin '{plugin_name}' requires agent API {minimum_version} \
                     or newer. Please update the agent."
                )
            }
            Self::Incompatible {
                plugin_name, plugin_requires, agent_provides, suggestion,
            } => {
                write!(
                    f,
                    "Plugin '{plugin_name}' is incompatible: \
                     requires API {plugin_requires}, agent provides {agent_provides}. \
                     {suggestion}"
                )
            }
        }
    }
}
```

::: python Coming from Python
Python packages declare version dependencies in `pyproject.toml`:
```python
[project]
dependencies = [
    "agent-sdk>=1.2,<2.0",
]
```
pip resolves these at install time and raises an error if versions conflict. Rust's approach checks at plugin load time rather than install time, because plugins are loaded dynamically. The version check happens in your code rather than in a package manager, giving you full control over the error messages and fallback behavior.
:::

## Evolving the API Without Breaking Plugins

The hardest part of versioning is not the version numbers -- it is designing your API so that it can evolve without breaking existing plugins. Here are the key strategies:

### 1. Additive-Only Changes (Minor Versions)

Add new methods with default implementations so existing plugins do not need to change:

```rust
/// Version 1.0: Original trait
#[async_trait::async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn init(&self, ctx: &mut PluginContext) -> Result<()>;
    fn shutdown(&self) -> Result<()>;

    // Added in v1.1: health checking (has a default, so v1.0 plugins still compile)
    fn health_check(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    // Added in v1.2: configuration schema (has a default, so v1.0/v1.1 plugins still compile)
    fn config_schema(&self) -> Option<serde_json::Value> {
        None
    }

    // Added in v1.3: priority for load ordering (has a default)
    fn load_priority(&self) -> i32 {
        0
    }
}

#[derive(Debug, Clone)]
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}
```

Default trait implementations are your best friend for backward compatibility. Every new method on the `Plugin` trait gets a sensible default, so plugins compiled against v1.0 of the trait work unchanged with v1.3 of the agent.

### 2. Non-Breaking Data Changes

Use `#[serde(default)]` on struct fields so that adding new fields to serialized types does not break existing data:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCallContext {
    pub tool_name: String,
    pub arguments: serde_json::Value,

    // Added in v1.1: these fields have defaults so old data
    // (without these fields) still deserializes correctly
    #[serde(default)]
    pub invocation_id: String,

    #[serde(default)]
    pub timeout_ms: Option<u64>,

    #[serde(default)]
    pub metadata: serde_json::Value,
}
```

### 3. Deprecation Cycles

When you need to make a breaking change, deprecate the old API first and give plugin authors time to migrate:

```rust
#[async_trait::async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;

    /// New initialization method (v1.2+).
    /// Receives an owned context with richer capabilities.
    fn init_v2(&self, ctx: PluginContextV2) -> Result<()> {
        // Default: fall back to the old init method for backward compat
        #[allow(deprecated)]
        self.init(&mut ctx.into_legacy())
    }

    /// Original initialization method.
    /// Deprecated in v1.2 -- use init_v2 instead.
    #[deprecated(since = "1.2.0", note = "Use init_v2 instead")]
    fn init(&self, _ctx: &mut PluginContext) -> Result<()> {
        Ok(())
    }

    fn shutdown(&self) -> Result<()>;
}
```

The `#[deprecated]` attribute causes compiler warnings (not errors) when plugin authors use the old method, guiding them to migrate without breaking their code.

::: tip In the Wild
MCP handles versioning through protocol version negotiation during the initialization handshake. The client declares which protocol version it supports, and the server responds with its version. If they are incompatible, the connection fails with a clear error message. This is similar to HTTP content negotiation -- both sides declare their capabilities and find common ground. For your plugin API, the same principle applies: version checking at connection/load time with clear diagnostics is far better than mysterious runtime failures.
:::

## Version Checking in the Plugin Manager

Integrate version checking into the plugin loading pipeline so incompatible plugins are caught early:

```rust
impl PluginManager {
    pub async fn load_plugin_with_version_check(
        &mut self,
        manifest: VersionedPluginManifest,
        plugin: Box<dyn Plugin>,
    ) -> Result<()> {
        // Step 1: Check API version compatibility
        match check_plugin_compatibility(&manifest) {
            CompatibilityResult::Compatible => {
                println!(
                    "Plugin '{}' v{} is compatible (API {})",
                    manifest.name, manifest.version, PLUGIN_API_VERSION
                );
            }
            CompatibilityResult::NeedsNewerAgent { plugin_name, minimum_version } => {
                return Err(anyhow::anyhow!(
                    "Plugin '{plugin_name}' requires agent API {minimum_version} \
                     or newer. Current agent API is {PLUGIN_API_VERSION}. \
                     Please update the agent.",
                ));
            }
            CompatibilityResult::Incompatible {
                plugin_name, suggestion, ..
            } => {
                return Err(anyhow::anyhow!(
                    "Plugin '{plugin_name}' is incompatible with this agent version. \
                     {suggestion}",
                ));
            }
        }

        // Step 2: Check capability compatibility
        // (does the agent support all capabilities the plugin requires?)

        // Step 3: Load and initialize
        let mut ctx = PluginContext::from(self.context.clone());
        plugin.init(&mut ctx)?;

        self.plugins.push(LoadedPlugin {
            manifest: PluginManifest {
                name: manifest.name,
                version: manifest.version.to_string(),
                provides: vec![],
                requires: vec![],
            },
            instance: PluginInstance::Embedded(plugin),
            state: PluginState::Initialized,
        });

        Ok(())
    }
}
```

## Best Practices for Plugin API Versioning

1. **Start at 1.0.0**, not 0.x. Plugins authors need confidence that you have committed to stability.

2. **Bump the major version sparingly.** Every major version fragments the ecosystem -- some plugins update immediately, others never do.

3. **Use feature flags instead of version bumps** for optional capabilities. A plugin declares `requires: ["tool_registry_v2"]` rather than `api_version >= 1.5`.

4. **Document every API change** in a changelog. Plugin authors need to know what changed and how to adapt.

5. **Provide a migration guide** for major versions. Show before/after code for every breaking change.

## Key Takeaways

- **Semantic versioning** communicates compatibility: major for breaking changes, minor for additions, patch for fixes -- and plugins declare which version range they support.
- **Default trait implementations** are the primary tool for backward-compatible API evolution in Rust -- every new method on the `Plugin` trait should have a default.
- **`#[serde(default)]`** on struct fields lets you add new data fields without breaking deserialization of existing plugin data.
- **Deprecation cycles** (warn, then remove in the next major version) give plugin authors time to migrate rather than breaking them immediately.
- **Version checking at load time** with clear, actionable error messages ("update the agent" or "update the plugin") is essential for a healthy extension ecosystem.
