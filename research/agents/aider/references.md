# Aider — References

## Primary Sources

### Repository & Documentation
- **GitHub Repository**: [github.com/Aider-AI/aider](https://github.com/Aider-AI/aider)
- **Documentation Site**: [aider.chat](https://aider.chat/)
- **Installation Guide**: [aider.chat/docs/install.html](https://aider.chat/docs/install.html)
- **Usage Guide**: [aider.chat/docs/usage.html](https://aider.chat/docs/usage.html)
- **PyPI Package**: [pypi.org/project/aider-chat/](https://pypi.org/project/aider-chat/)

### Leaderboards & Benchmarks
- **LLM Leaderboard**: [aider.chat/docs/leaderboards/](https://aider.chat/docs/leaderboards/)
- **Benchmark Methodology**: [aider.chat/docs/benchmarks.html](https://aider.chat/docs/benchmarks.html)

### Key Blog Posts
- **Repo-Map Deep Dive** (Oct 2023): [aider.chat/2023/10/22/repomap.html](https://aider.chat/2023/10/22/repomap.html) — How tree-sitter powers the codebase map
- **Unified Diffs for LLMs** (Dec 2023): [aider.chat/2023/12/21/unified-diffs.html](https://aider.chat/2023/12/21/unified-diffs.html) — The udiff format to combat lazy coding
- **Architect/Editor Mode** (Sep 2024): [aider.chat/2024/09/26/architect.html](https://aider.chat/2024/09/26/architect.html) — Two-model approach for SOTA results

### Technical Documentation
- **Edit Formats**: [aider.chat/docs/more/edit-formats.html](https://aider.chat/docs/more/edit-formats.html)
- **Chat Modes**: [aider.chat/docs/usage/modes.html](https://aider.chat/docs/usage/modes.html)
- **Git Integration**: [aider.chat/docs/git.html](https://aider.chat/docs/git.html)
- **Repo-Map**: [aider.chat/docs/repomap.html](https://aider.chat/docs/repomap.html)
- **Voice Coding**: [aider.chat/docs/usage/voice.html](https://aider.chat/docs/usage/voice.html)
- **Watch Mode (IDE Integration)**: [aider.chat/docs/usage/watch.html](https://aider.chat/docs/usage/watch.html)
- **Lint & Test**: [aider.chat/docs/usage/lint-test.html](https://aider.chat/docs/usage/lint-test.html)
- **LLM Connections**: [aider.chat/docs/llms.html](https://aider.chat/docs/llms.html)
- **Configuration**: [aider.chat/docs/config.html](https://aider.chat/docs/config.html)
- **Advanced Model Settings**: [aider.chat/docs/config/adv-model-settings.html](https://aider.chat/docs/config/adv-model-settings.html)
- **Supported Languages**: [aider.chat/docs/languages.html](https://aider.chat/docs/languages.html)
- **Release History**: [aider.chat/HISTORY.html](https://aider.chat/HISTORY.html)

## Key Source Files

### Core Architecture
- **Base Coder**: [aider/coders/base_coder.py](https://github.com/Aider-AI/aider/blob/main/aider/coders/base_coder.py) — The core Coder class
- **Repo Map**: [aider/repomap.py](https://github.com/Aider-AI/aider/blob/main/aider/repomap.py) — Tree-sitter repo map generation
- **Models**: [aider/models.py](https://github.com/Aider-AI/aider/blob/main/aider/models.py) — Model configuration and routing
- **Commands**: [aider/commands.py](https://github.com/Aider-AI/aider/blob/main/aider/commands.py) — Slash command handling
- **Git Integration**: [aider/repo.py](https://github.com/Aider-AI/aider/blob/main/aider/repo.py) — Git operations

### Edit Format Implementations
- **Diff (Search/Replace)**: [aider/coders/editblock_coder.py](https://github.com/Aider-AI/aider/blob/main/aider/coders/editblock_coder.py)
- **Whole File**: [aider/coders/wholefile_coder.py](https://github.com/Aider-AI/aider/blob/main/aider/coders/wholefile_coder.py)
- **Unified Diff**: [aider/coders/udiff_coder.py](https://github.com/Aider-AI/aider/blob/main/aider/coders/udiff_coder.py)
- **Architect Mode**: [aider/coders/architect_coder.py](https://github.com/Aider-AI/aider/blob/main/aider/coders/architect_coder.py)

### Prompts (Edit Format Instructions)
- **Diff Prompts**: [aider/coders/editblock_prompts.py](https://github.com/Aider-AI/aider/blob/main/aider/coders/editblock_prompts.py)
- **Whole Prompts**: [aider/coders/wholefile_prompts.py](https://github.com/Aider-AI/aider/blob/main/aider/coders/wholefile_prompts.py)
- **Commit Prompt**: [aider/prompts.py](https://github.com/Aider-AI/aider/blob/main/aider/prompts.py)

### Tree-Sitter Queries
- **Query Files**: [aider/queries/](https://github.com/Aider-AI/aider/tree/main/aider/queries) — Language-specific `.scm` files for symbol extraction

## Dependencies

| Package | Role |
|---------|------|
| [litellm](https://github.com/BerriAI/litellm) | Universal LLM API abstraction |
| [tree-sitter](https://tree-sitter.github.io/tree-sitter/) | Language-aware code parsing |
| [grep-ast](https://github.com/Aider-AI/grep-ast) | AST-aware code searching and formatting |
| [prompt_toolkit](https://python-prompt-toolkit.readthedocs.io/) | Rich terminal UI |
| [pygments](https://pygments.org/) | Syntax highlighting |
| [diskcache](https://github.com/grantjenks/python-diskcache) | Persistent caching (tags cache) |
| [tqdm](https://tqdm.github.io/) | Progress bars |

## Community

- **Discord**: [discord.gg/Y7X7bhMQFV](https://discord.gg/Y7X7bhMQFV)
- **Blog**: [aider.chat/blog/](https://aider.chat/blog/)
- **OpenRouter Ranking**: Top 20 application on OpenRouter
- **Creator**: Paul Gauthier ([@paul-gauthier](https://github.com/paul-gauthier))

## Notable Mentions

- Eric S. Raymond: _"My life has changed... Aider... It's going to rock your world."_
- Nick Dobos: _"Best agent for actual dev work in existing codebases."_
- BeetleB (Hacker News): _"Aider ... is the tool to benchmark against."_
- Reilly Sweetland: _"Aider is the precision tool of LLM code gen... Minimal, thoughtful and capable of surgical changes."_