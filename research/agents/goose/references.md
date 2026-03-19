# Goose — References

## Primary Sources

### GitHub Repository
- **Main repo**: https://github.com/block/goose
- **License**: Apache 2.0
- **Language**: Rust (core), TypeScript (desktop UI)
- **Stars**: ~15k+
- **Organization**: Block, Inc.

### Official Documentation
- **Docs home**: https://block.github.io/goose/
- **Quickstart**: https://block.github.io/goose/docs/quickstart
- **Installation**: https://block.github.io/goose/docs/getting-started/installation
- **Providers**: https://block.github.io/goose/docs/getting-started/providers
- **Extensions**: https://block.github.io/goose/docs/getting-started/using-extensions
- **Extension Directory**: https://block.github.io/goose/extensions
- **Tutorials**: https://block.github.io/goose/docs/category/tutorials
- **Troubleshooting**: https://block.github.io/goose/docs/troubleshooting/diagnostics-and-reporting
- **Known Issues**: https://block.github.io/goose/docs/troubleshooting/known-issues

### Built-in Extension Docs
- **Developer**: https://block.github.io/goose/docs/mcp/developer-mcp
- **Computer Controller**: https://block.github.io/goose/docs/mcp/computer-controller-mcp
- **Memory**: https://block.github.io/goose/docs/mcp/memory-mcp
- **Tutorial**: https://block.github.io/goose/docs/mcp/tutorial-mcp
- **Auto Visualiser**: https://block.github.io/goose/docs/mcp/autovisualiser-mcp
- **Apps**: https://block.github.io/goose/docs/mcp/apps-mcp
- **Chat Recall**: https://block.github.io/goose/docs/mcp/chatrecall-mcp
- **Code Mode**: https://block.github.io/goose/docs/mcp/code-mode-mcp
- **Extension Manager**: https://block.github.io/goose/docs/mcp/extension-manager-mcp
- **Summon**: https://block.github.io/goose/docs/mcp/summon-mcp
- **Todo**: https://block.github.io/goose/docs/mcp/todo-mcp
- **Top of Mind**: https://block.github.io/goose/docs/mcp/tom-mcp

### Guides
- **GooseHints**: https://block.github.io/goose/docs/guides/context-engineering/using-goosehints
- **Permissions**: https://block.github.io/goose/docs/guides/goose-permissions
- **Tool Permissions**: https://block.github.io/goose/docs/guides/managing-tools/tool-permissions
- **GooseIgnore**: https://block.github.io/goose/docs/guides/using-gooseignore
- **Config Files**: https://block.github.io/goose/docs/guides/config-files
- **Enhanced Code Editing**: https://block.github.io/goose/docs/guides/enhanced-code-editing
- **Codebase Analysis**: https://block.github.io/goose/docs/guides/codebase-analysis
- **Rate Limits**: https://block.github.io/goose/docs/guides/handling-llm-rate-limits-with-goose
- **Security**: https://block.github.io/goose/docs/guides/security/
- **Environment Variables**: https://block.github.io/goose/docs/guides/environment-variables
- **CI/CD**: https://block.github.io/goose/docs/tutorials/cicd
- **Custom Extensions**: https://block.github.io/goose/docs/tutorials/custom-extensions
- **Docker**: https://block.github.io/goose/docs/tutorials/goose-in-docker
- **Custom Distributions**: https://github.com/block/goose/blob/main/CUSTOM_DISTROS.md
- **Governance**: https://github.com/block/goose/blob/main/GOVERNANCE.md

## Source Code References

### Core Agent
- **Agent struct & reply loop**: `crates/goose/src/agents/agent.rs` (~97KB)
- **Extension manager**: `crates/goose/src/agents/extension_manager.rs` (~81KB)
- **MCP client**: `crates/goose/src/agents/mcp_client.rs`
- **Extension config types**: `crates/goose/src/agents/extension.rs`
- **Tool execution**: `crates/goose/src/agents/tool_execution.rs`
- **LLM streaming & toolshim**: `crates/goose/src/agents/reply_parts.rs`
- **Retry manager**: `crates/goose/src/agents/retry.rs`
- **Agent types**: `crates/goose/src/agents/types.rs`

### Platform Extensions
- **Developer (shell, edit, write, tree)**: `crates/goose/src/agents/platform_extensions/developer/`
- **Analyze (tree-sitter)**: `crates/goose/src/agents/platform_extensions/analyze/`
- **Todo**: `crates/goose/src/agents/platform_extensions/todo/`
- **Apps**: `crates/goose/src/agents/platform_extensions/apps/`
- **Chat Recall**: `crates/goose/src/agents/platform_extensions/chatrecall/`
- **Extension Manager**: `crates/goose/src/agents/platform_extensions/extension_manager_ext/`
- **Summon (subagents)**: `crates/goose/src/agents/platform_extensions/summon/`
- **Top of Mind**: `crates/goose/src/agents/platform_extensions/tom/`
- **Platform registry**: `crates/goose/src/agents/platform_extensions/mod.rs`

### Built-in MCP Servers
- **Computer Controller**: `crates/goose-mcp/src/computercontroller/`
- **Memory**: `crates/goose-mcp/src/memory/`
- **Auto Visualiser**: `crates/goose-mcp/src/autovisualiser/`
- **Tutorial**: `crates/goose-mcp/src/tutorial/`
- **Server runner**: `crates/goose-mcp/src/mcp_server_runner.rs`
- **Builtin registry**: `crates/goose-mcp/src/lib.rs`

### Configuration
- **Config singleton**: `crates/goose/src/config/base.rs`
- **Extension persistence**: `crates/goose/src/config/extensions.rs`
- **Permission modes**: `crates/goose/src/config/goose_mode.rs`
- **Permission manager**: `crates/goose/src/config/permission.rs`
- **Model config**: `crates/goose/src/model.rs`

### Context Management
- **Compaction & summarization**: `crates/goose/src/context_mgmt/mod.rs`
- **Token counting**: `crates/goose/src/token_counter.rs`
- **Conversation types**: `crates/goose/src/conversation/mod.rs` (~44KB)
- **Message types**: `crates/goose/src/conversation/message.rs` (~53KB)

### Providers
- **Provider trait**: `crates/goose/src/providers/base.rs`
- **Provider implementations**: `crates/goose/src/providers/` (30+ files)
- **Anthropic**: `crates/goose/src/providers/anthropic.rs`
- **OpenAI**: `crates/goose/src/providers/openai.rs`
- **Google**: `crates/goose/src/providers/google.rs`
- **Ollama**: `crates/goose/src/providers/ollama.rs`
- **Toolshim**: `crates/goose/src/providers/toolshim.rs`

### Server & CLI
- **HTTP server**: `crates/goose-server/`
- **CLI**: `crates/goose-cli/`
- **ACP support**: `crates/goose-acp/`
- **Desktop UI**: `ui/desktop/`

## External References

### MCP (Model Context Protocol)
- **MCP specification**: https://modelcontextprotocol.io/
- **MCP quickstart**: https://modelcontextprotocol.io/quickstart/server
- **MCP servers directory**: https://github.com/modelcontextprotocol/servers
- **rmcp (Rust SDK)**: https://crates.io/crates/rmcp

### Block (Square)
- **Block website**: https://block.xyz/
- **Block open source**: https://opensource.block.xyz/

### Community
- **Discord**: https://discord.gg/goose-oss
- **YouTube**: https://www.youtube.com/@goose-oss
- **LinkedIn**: https://www.linkedin.com/company/goose-oss
- **Twitter/X**: https://x.com/goose_oss
- **Bluesky**: https://bsky.app/profile/opensource.block.xyz

### Benchmarks
- **Terminal-Bench**: https://terminal-bench.com/ (leaderboard)
- **Berkeley Function-Calling Leaderboard**: https://gorilla.cs.berkeley.edu/leaderboard.html (referenced by Goose docs for model selection)

### Related Projects
- **Agent Communication Protocol (ACP)**: https://agentclientprotocol.com/
- **Tetrate Agent Router**: https://tetrate.io/products/tetrate-agent-router-service
- **OpenRouter**: https://openrouter.ai/

## Research Notes

- Research conducted via GitHub API (source code analysis), official documentation, and web sources
- Source code analysis focused on commit `c97fbb9f3cef8593586633f91ea221250c5c38c9` (main branch)
- Version at time of research: 1.28.0
- The agent loop (`agent.rs`) is remarkably large at ~97KB, indicating significant complexity concentrated in a single file
- The extension manager (`extension_manager.rs`) at ~81KB is similarly large
- Conversation and message types total ~97KB across two files, reflecting the complexity of MCP message handling