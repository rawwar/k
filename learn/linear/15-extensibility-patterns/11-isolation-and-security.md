---
title: Isolation and Security
description: Address the security challenges of running third-party plugin code, including sandboxing, capability restriction, and trust models.
---

# Isolation and Security

> **What you'll learn:**
> - How to apply the principle of least privilege to plugins, granting only the specific capabilities each plugin declares it needs
> - Techniques for sandboxing plugin execution using WASM runtimes, subprocess isolation, or OS-level containment
> - How to design a trust model for extensions that distinguishes between built-in, verified, and community plugins with different privilege levels

Running third-party code is the most dangerous thing your agent can do -- and extensibility requires exactly that. Every MCP server you spawn, every dynamic plugin you load, and every hook command you execute is potential attack surface. A malicious plugin could steal API keys, delete files, or exfiltrate data through network requests. Even a well-intentioned but buggy plugin could crash the agent or corrupt its state.

Security for an extensible agent is not optional. It is the difference between a platform users trust and one they avoid. This subchapter covers the practical techniques for isolating plugin code, restricting capabilities, and building a trust model that balances safety with usability.

## The Principle of Least Privilege

Every plugin should have access to exactly the capabilities it needs and nothing more. A spell-checking plugin needs read access to messages -- it does not need network access, file system access, or the ability to execute shell commands. The challenge is enforcing this in practice.

Start by defining a capability system:

```rust
use std::collections::HashSet;
use serde::{Deserialize, Serialize};

/// Capabilities that a plugin can request.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum Capability {
    /// Read files from the filesystem
    FileRead { paths: Vec<String> },
    /// Write files to the filesystem
    FileWrite { paths: Vec<String> },
    /// Make network requests
    Network { allowed_hosts: Vec<String> },
    /// Execute shell commands
    ShellExec,
    /// Access environment variables
    EnvAccess { variables: Vec<String> },
    /// Subscribe to agent events
    EventSubscribe { event_types: Vec<String> },
    /// Register hooks (intercept agent behavior)
    HookRegister { hook_points: Vec<String> },
    /// Register new tools
    ToolRegister,
}

/// A plugin's capability declaration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub plugin_name: String,
    pub required: Vec<Capability>,
    pub optional: Vec<Capability>,
    pub justification: String,
}
```

The plugin declares what it needs in its manifest. At load time, the agent checks these declarations against its security policy:

```rust
/// Security policy that determines which capabilities to grant.
pub struct SecurityPolicy {
    pub trust_level: TrustLevel,
    pub capability_overrides: HashMap<String, CapabilityDecision>,
    pub blocked_capabilities: HashSet<Capability>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TrustLevel {
    /// Built-in plugins: full access, no restrictions
    BuiltIn,
    /// Verified plugins: broad access, some restrictions
    Verified,
    /// Community plugins: restricted access, user approval required
    Community,
    /// Untrusted plugins: minimal access, sandboxed execution
    Untrusted,
}

#[derive(Debug, Clone)]
pub enum CapabilityDecision {
    Grant,
    Deny,
    Prompt, // Ask the user at runtime
}

pub fn evaluate_capabilities(
    request: &CapabilityRequest,
    policy: &SecurityPolicy,
) -> Vec<(Capability, CapabilityDecision)> {
    let mut decisions = Vec::new();

    for cap in &request.required {
        let decision = if policy.blocked_capabilities.contains(cap) {
            CapabilityDecision::Deny
        } else {
            match policy.trust_level {
                TrustLevel::BuiltIn => CapabilityDecision::Grant,
                TrustLevel::Verified => match cap {
                    Capability::ShellExec => CapabilityDecision::Prompt,
                    _ => CapabilityDecision::Grant,
                },
                TrustLevel::Community => match cap {
                    Capability::FileRead { .. }
                    | Capability::EventSubscribe { .. } => {
                        CapabilityDecision::Grant
                    }
                    _ => CapabilityDecision::Prompt,
                },
                TrustLevel::Untrusted => CapabilityDecision::Deny,
            }
        };

        // Check per-plugin overrides
        let final_decision = policy
            .capability_overrides
            .get(&format!("{}:{:?}", request.plugin_name, cap))
            .cloned()
            .unwrap_or(decision);

        decisions.push((cap.clone(), final_decision));
    }

    decisions
}
```

::: python Coming from Python
Python plugin systems often run plugins in the same process with no isolation:
```python
# Dangerous: plugin has full access to everything
plugin = load_plugin("community_plugin")
plugin.init(agent_context)  # Can access files, network, os.system(), etc.
```
Some Python frameworks use `RestrictedPython` or `ast` analysis to limit what plugin code can do, but these are easily bypassed. Rust's type system provides a stronger foundation: if the `PluginContext` does not include a method for network access, the plugin genuinely cannot make network requests (assuming you are not giving it raw `std::net` access through dynamic linking).
:::

## Subprocess Isolation

The simplest and most effective isolation strategy is running plugins as separate processes. This leverages the operating system's process isolation -- a plugin in a separate process cannot access the agent's memory, file descriptors, or resources without going through the defined IPC protocol.

```rust
use std::process::Stdio;
use tokio::process::Command;

/// Configuration for sandboxed plugin execution.
pub struct SandboxConfig {
    /// Maximum memory the plugin process can use (bytes)
    pub memory_limit: Option<u64>,
    /// Maximum CPU time (seconds)
    pub cpu_time_limit: Option<u64>,
    /// Allowed filesystem paths (everything else is blocked)
    pub allowed_paths: Vec<String>,
    /// Whether network access is permitted
    pub allow_network: bool,
    /// Working directory for the plugin
    pub working_directory: Option<String>,
}

/// Spawn a sandboxed plugin process.
/// On macOS, uses sandbox-exec; on Linux, uses seccomp/namespaces.
pub async fn spawn_sandboxed(
    command: &str,
    args: &[String],
    sandbox: &SandboxConfig,
) -> Result<tokio::process::Child> {
    #[cfg(target_os = "macos")]
    {
        // macOS: use sandbox-exec with a profile
        let profile = build_sandbox_profile(sandbox);
        Command::new("sandbox-exec")
            .arg("-p")
            .arg(&profile)
            .arg(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn sandboxed process: {e}"))
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: use bwrap (bubblewrap) for namespace isolation
        let mut bwrap_args = vec![
            "--ro-bind".to_string(), "/usr".to_string(), "/usr".to_string(),
            "--ro-bind".to_string(), "/lib".to_string(), "/lib".to_string(),
            "--ro-bind".to_string(), "/lib64".to_string(), "/lib64".to_string(),
            "--proc".to_string(), "/proc".to_string(),
            "--dev".to_string(), "/dev".to_string(),
        ];

        // Add allowed paths as bind mounts
        for path in &sandbox.allowed_paths {
            bwrap_args.extend_from_slice(&[
                "--bind".to_string(), path.clone(), path.clone(),
            ]);
        }

        if !sandbox.allow_network {
            bwrap_args.push("--unshare-net".to_string());
        }

        bwrap_args.push("--".to_string());
        bwrap_args.push(command.to_string());
        bwrap_args.extend(args.iter().cloned());

        Command::new("bwrap")
            .args(&bwrap_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn sandboxed process: {e}"))
    }
}

#[cfg(target_os = "macos")]
fn build_sandbox_profile(sandbox: &SandboxConfig) -> String {
    let mut profile = String::from("(version 1)\n(deny default)\n");
    profile.push_str("(allow process-exec)\n");
    profile.push_str("(allow mach-lookup)\n");

    for path in &sandbox.allowed_paths {
        profile.push_str(&format!(
            "(allow file-read* (subpath \"{path}\"))\n\
             (allow file-write* (subpath \"{path}\"))\n"
        ));
    }

    if sandbox.allow_network {
        profile.push_str("(allow network*)\n");
    }

    profile
}
```

## WebAssembly Sandboxing

For the strongest isolation without process overhead, WebAssembly (WASM) runtimes like `wasmtime` let you run plugin code in a sandboxed environment within the same process. WASM plugins have no inherent access to the filesystem, network, or system calls -- they can only use capabilities explicitly granted by the host:

```rust
use wasmtime::*;

pub struct WasmPluginHost {
    engine: Engine,
    store: Store<PluginState>,
}

struct PluginState {
    /// Data the plugin is allowed to read
    allowed_data: HashMap<String, String>,
    /// Accumulated output from the plugin
    output: Vec<String>,
}

impl WasmPluginHost {
    pub fn new() -> Result<Self> {
        let engine = Engine::default();
        let store = Store::new(&engine, PluginState {
            allowed_data: HashMap::new(),
            output: Vec::new(),
        });

        Ok(Self { engine, store })
    }

    /// Load and execute a WASM plugin with controlled capabilities.
    pub fn execute_plugin(
        &mut self,
        wasm_bytes: &[u8],
        function_name: &str,
        input: &str,
    ) -> Result<String> {
        let module = Module::new(&self.engine, wasm_bytes)?;

        // Create a linker with only the functions we want to expose
        let mut linker = Linker::new(&self.engine);

        // Expose a "log" function so the plugin can produce output
        linker.func_wrap(
            "env",
            "log",
            |mut caller: Caller<'_, PluginState>, ptr: i32, len: i32| {
                let memory = caller.get_export("memory")
                    .and_then(|e| e.into_memory());
                if let Some(memory) = memory {
                    let data = memory.data(&caller);
                    let slice = &data[ptr as usize..(ptr + len) as usize];
                    if let Ok(msg) = std::str::from_utf8(slice) {
                        caller.data_mut().output.push(msg.to_string());
                    }
                }
            },
        )?;

        // Note: we deliberately do NOT expose file I/O,
        // network, or other dangerous functions

        let instance = linker.instantiate(&mut self.store, &module)?;

        let func = instance
            .get_typed_func::<(i32, i32), i32>(&mut self.store, function_name)?;

        // Write input to WASM memory and call the function
        // (simplified -- real implementation needs memory management)
        let _result = func.call(&mut self.store, (0, input.len() as i32))?;

        Ok(self.store.data().output.join("\n"))
    }
}
```

::: wild In the Wild
The Extism project provides a production-ready WASM plugin framework for Rust that handles the complexity of host-guest communication, memory management, and capability restriction. It lets you write plugins in any language that compiles to WASM (Rust, Go, C, AssemblyScript, and others) and run them with fine-grained capability control. For a coding agent, WASM sandboxing is ideal for user-defined content transformers and validators that should not have access to the filesystem or network.
:::

## Validating Plugin Inputs and Outputs

Even with process or WASM isolation, you need to validate what crosses the boundary. A malicious plugin might return crafted output designed to inject commands or manipulate the LLM:

```rust
/// Validate a tool result before passing it back to the LLM.
pub fn validate_tool_output(output: &str, max_length: usize) -> Result<String> {
    // Enforce size limits
    if output.len() > max_length {
        return Ok(format!(
            "{}\n... [output truncated at {} bytes]",
            &output[..max_length],
            max_length
        ));
    }

    // Strip control characters that could manipulate the terminal
    let sanitized: String = output
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect();

    Ok(sanitized)
}

/// Validate that a plugin's tool registration is not trying
/// to shadow built-in tools.
pub fn validate_tool_registration(
    plugin_name: &str,
    tool_name: &str,
    builtin_tools: &HashSet<String>,
    trust_level: &TrustLevel,
) -> Result<()> {
    if builtin_tools.contains(tool_name) && *trust_level != TrustLevel::BuiltIn {
        return Err(anyhow::anyhow!(
            "Plugin '{plugin_name}' attempted to register tool '{tool_name}' \
             which shadows a built-in tool. This requires BuiltIn trust level."
        ));
    }

    // Validate tool name format
    if !tool_name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return Err(anyhow::anyhow!(
            "Plugin '{plugin_name}' tool name '{tool_name}' contains \
             invalid characters"
        ));
    }

    Ok(())
}
```

## Resource Limits

Plugins can be dangerous not just through malice but through resource consumption -- a runaway plugin can eat all available memory or CPU:

```rust
/// Enforce resource limits on a plugin subprocess.
pub struct ResourceLimits {
    /// Maximum wall-clock time for any single tool call
    pub max_execution_time: Duration,
    /// Maximum number of concurrent operations
    pub max_concurrent_ops: usize,
    /// Maximum output size in bytes
    pub max_output_bytes: usize,
}

/// A rate limiter for plugin operations.
pub struct PluginRateLimiter {
    operations: tokio::sync::Semaphore,
    limits: ResourceLimits,
}

impl PluginRateLimiter {
    pub fn new(limits: ResourceLimits) -> Self {
        Self {
            operations: tokio::sync::Semaphore::new(limits.max_concurrent_ops),
            limits,
        }
    }

    /// Execute a plugin operation with resource limits applied.
    pub async fn execute<F, T>(&self, operation: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        // Acquire a concurrency permit
        let _permit = self.operations.acquire().await
            .map_err(|_| anyhow::anyhow!("Rate limiter closed"))?;

        // Apply timeout
        tokio::time::timeout(self.limits.max_execution_time, operation)
            .await
            .map_err(|_| anyhow::anyhow!(
                "Plugin operation timed out after {:?}",
                self.limits.max_execution_time
            ))?
    }
}
```

## Key Takeaways

- The **principle of least privilege** means plugins declare needed capabilities in their manifest, and the agent grants only what is required based on the plugin's trust level.
- **Subprocess isolation** is the simplest and most effective sandboxing strategy -- a plugin in a separate process cannot access the agent's memory or resources without going through the IPC protocol.
- **WebAssembly sandboxing** provides stronger isolation within a single process, with fine-grained capability control and support for plugins written in multiple languages.
- **Input and output validation** at the plugin boundary prevents injection attacks, output manipulation, and resource exhaustion regardless of the isolation mechanism.
- A **trust model** (built-in, verified, community, untrusted) lets users make informed decisions about which plugins to trust, with different capability defaults for each level.
