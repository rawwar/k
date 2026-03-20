---
title: Project Type and Structure Detection
status: complete
---

# Project Detection

> How coding agents detect project type, language, framework, build system, and repository structure — and how this meta-understanding guides all subsequent code understanding.

## Overview

Project detection is the first step in code understanding. Before an agent can search, index, or analyze code, it needs to know the basics: What language is this? What framework? How is it built? How is it tested? What are the conventions? This meta-understanding shapes every subsequent decision the agent makes.

### The Detection Hierarchy

```
┌──────────────────────────────────────────┐
│ 1. Language Detection                     │  What languages are used?
│    └── File extensions, shebangs          │
├──────────────────────────────────────────┤
│ 2. Framework Detection                    │  React? Django? Express?
│    └── Package manifests, imports         │
├──────────────────────────────────────────┤
│ 3. Build System Detection                 │  npm? cargo? make?
│    └── Build files, scripts              │
├──────────────────────────────────────────┤
│ 4. Project Structure Detection            │  Monorepo? Workspace?
│    └── Directory layout, workspace files │
├──────────────────────────────────────────┤
│ 5. Convention Detection                   │  Coding style, patterns
│    └── Config files, CLAUDE.md, linters  │
└──────────────────────────────────────────┘
```

### Agent Approaches to Project Detection

| Agent | Detection Approach | Key Feature |
|---|---|---|
| **Claude Code** | CLAUDE.md + auto-detection | Project instruction files |
| **Codex** | AGENTS.md + CODEX.md | Hierarchical instruction files |
| **Aider** | File extension analysis | Language-based tree-sitter grammar selection |
| **ForgeCode** | Multi-signal detection | Comprehensive project analysis |
| **Gemini CLI** | Project config detection | Gemini context file |
| **Droid** | Full project analysis | Deep structural understanding |
| **Junie CLI** | JetBrains project model | Full IDE project detection |
| **OpenHands** | Runtime detection | Detects during exploration |
| **mini-SWE-agent** | Minimal | Basic language detection |

---

## Language Detection

### File Extension Mapping

The most basic and reliable language detection method:

```python
EXTENSION_MAP = {
    # JavaScript / TypeScript
    ".js": "javascript", ".jsx": "javascript",
    ".ts": "typescript", ".tsx": "typescript",
    ".mjs": "javascript", ".cjs": "javascript",
    ".mts": "typescript", ".cts": "typescript",

    # Python
    ".py": "python", ".pyi": "python",
    ".pyw": "python", ".pyx": "cython",

    # Go
    ".go": "go",

    # Rust
    ".rs": "rust",

    # Java / Kotlin
    ".java": "java", ".kt": "kotlin", ".kts": "kotlin",

    # C / C++
    ".c": "c", ".h": "c",
    ".cpp": "cpp", ".cc": "cpp", ".cxx": "cpp",
    ".hpp": "hpp", ".hh": "cpp",

    # Ruby
    ".rb": "ruby", ".erb": "erb",

    # PHP
    ".php": "php",

    # Swift
    ".swift": "swift",

    # Shell
    ".sh": "bash", ".bash": "bash", ".zsh": "zsh",

    # Web
    ".html": "html", ".htm": "html",
    ".css": "css", ".scss": "scss", ".less": "less",

    # Data / Config
    ".json": "json", ".yaml": "yaml", ".yml": "yaml",
    ".toml": "toml", ".xml": "xml",
    ".md": "markdown", ".mdx": "mdx",
}

def detect_language(filepath):
    """Detect language from file extension."""
    ext = Path(filepath).suffix.lower()
    return EXTENSION_MAP.get(ext, None)
```

### Shebang Detection

For files without clear extensions (scripts, executables):

```python
def detect_from_shebang(filepath):
    """Detect language from shebang line."""
    try:
        with open(filepath, 'r') as f:
            first_line = f.readline().strip()
    except (UnicodeDecodeError, IOError):
        return None

    if not first_line.startswith("#!"):
        return None

    shebang_map = {
        "python": "python",
        "python3": "python",
        "node": "javascript",
        "bash": "bash",
        "sh": "bash",
        "ruby": "ruby",
        "perl": "perl",
        "php": "php",
    }

    for key, language in shebang_map.items():
        if key in first_line:
            return language
    return None
```

### Language Distribution Analysis

Understanding the primary language(s) helps agents prioritize their understanding efforts:

```python
def analyze_language_distribution(repo_root):
    """Analyze the distribution of languages in a repository."""
    counts = Counter()
    line_counts = Counter()

    for filepath in walk_source_files(repo_root):
        language = detect_language(str(filepath))
        if language:
            counts[language] += 1
            try:
                lines = filepath.read_text().count('\n')
                line_counts[language] += lines
            except (UnicodeDecodeError, IOError):
                pass

    total_files = sum(counts.values())
    total_lines = sum(line_counts.values())

    return {
        "primary_language": counts.most_common(1)[0][0] if counts else None,
        "languages": {
            lang: {
                "files": count,
                "pct_files": count / total_files * 100,
                "lines": line_counts[lang],
                "pct_lines": line_counts[lang] / total_lines * 100,
            }
            for lang, count in counts.most_common(10)
        }
    }
```

---

## Framework Detection

### Detection Strategies

Frameworks are detected through a combination of signals:

```python
class FrameworkDetector:
    """Detect frameworks used in a project."""

    FRAMEWORK_SIGNALS = {
        # JavaScript/TypeScript Frameworks
        "react": {
            "packages": ["react", "react-dom"],
            "files": ["src/App.tsx", "src/App.jsx", "src/App.js"],
            "imports": ["from 'react'", "from \"react\""],
        },
        "next.js": {
            "packages": ["next"],
            "files": ["next.config.js", "next.config.mjs", "next.config.ts"],
            "directories": ["pages/", "app/"],
        },
        "express": {
            "packages": ["express"],
            "imports": ["require('express')", "from 'express'"],
        },
        "fastify": {
            "packages": ["fastify"],
            "imports": ["require('fastify')", "from 'fastify'"],
        },
        "vue": {
            "packages": ["vue"],
            "files": ["vue.config.js", "nuxt.config.ts"],
        },
        "angular": {
            "packages": ["@angular/core"],
            "files": ["angular.json"],
        },

        # Python Frameworks
        "django": {
            "packages": ["django"],
            "files": ["manage.py", "settings.py"],
            "imports": ["from django", "import django"],
        },
        "flask": {
            "packages": ["flask"],
            "imports": ["from flask", "import flask"],
        },
        "fastapi": {
            "packages": ["fastapi"],
            "imports": ["from fastapi", "import fastapi"],
        },

        # Go Frameworks
        "gin": {
            "imports": ["github.com/gin-gonic/gin"],
        },
        "echo": {
            "imports": ["github.com/labstack/echo"],
        },
        "fiber": {
            "imports": ["github.com/gofiber/fiber"],
        },

        # Rust Frameworks
        "actix-web": {
            "packages": ["actix-web"],
            "imports": ["use actix_web"],
        },
        "axum": {
            "packages": ["axum"],
            "imports": ["use axum"],
        },
        "tokio": {
            "packages": ["tokio"],
            "imports": ["use tokio"],
        },
    }

    def detect(self, repo_root):
        """Detect all frameworks used in the project."""
        detected = []

        # Check package manifests
        manifests = self.read_manifests(repo_root)

        for framework, signals in self.FRAMEWORK_SIGNALS.items():
            confidence = 0.0

            # Check package dependencies
            if "packages" in signals:
                for pkg in signals["packages"]:
                    if pkg in manifests.get("dependencies", {}):
                        confidence += 0.8
                    elif pkg in manifests.get("devDependencies", {}):
                        confidence += 0.4

            # Check for framework-specific files
            if "files" in signals:
                for filepath in signals["files"]:
                    if (Path(repo_root) / filepath).exists():
                        confidence += 0.5

            # Check for framework-specific directories
            if "directories" in signals:
                for dirpath in signals["directories"]:
                    if (Path(repo_root) / dirpath).is_dir():
                        confidence += 0.3

            if confidence >= 0.5:
                detected.append({
                    "framework": framework,
                    "confidence": min(confidence, 1.0),
                })

        return sorted(detected, key=lambda x: -x["confidence"])
```

---

## Build System Detection

### Common Build Systems and Their Markers

| Build System | Marker Files | Language | Commands |
|---|---|---|---|
| **npm/yarn/pnpm** | package.json, yarn.lock, pnpm-lock.yaml | JS/TS | npm run build, yarn build |
| **Cargo** | Cargo.toml, Cargo.lock | Rust | cargo build, cargo test |
| **Go modules** | go.mod, go.sum | Go | go build, go test |
| **pip/poetry** | requirements.txt, pyproject.toml, setup.py | Python | pip install, pytest |
| **Maven** | pom.xml | Java | mvn compile, mvn test |
| **Gradle** | build.gradle, build.gradle.kts | Java/Kotlin | gradle build, gradle test |
| **Make** | Makefile | Any | make, make test |
| **CMake** | CMakeLists.txt | C/C++ | cmake, make |
| **Bazel** | BUILD, WORKSPACE, .bazelrc | Any | bazel build, bazel test |

```python
class BuildSystemDetector:
    BUILD_SYSTEMS = [
        {
            "name": "npm",
            "markers": ["package.json"],
            "lock_files": ["package-lock.json"],
            "commands": {"build": "npm run build", "test": "npm test", "install": "npm install"},
        },
        {
            "name": "yarn",
            "markers": ["package.json"],
            "lock_files": ["yarn.lock"],
            "commands": {"build": "yarn build", "test": "yarn test", "install": "yarn install"},
        },
        {
            "name": "pnpm",
            "markers": ["package.json"],
            "lock_files": ["pnpm-lock.yaml"],
            "commands": {"build": "pnpm build", "test": "pnpm test", "install": "pnpm install"},
        },
        {
            "name": "cargo",
            "markers": ["Cargo.toml"],
            "lock_files": ["Cargo.lock"],
            "commands": {"build": "cargo build", "test": "cargo test", "install": "cargo build"},
        },
        {
            "name": "go",
            "markers": ["go.mod"],
            "lock_files": ["go.sum"],
            "commands": {"build": "go build ./...", "test": "go test ./...", "install": "go mod download"},
        },
        {
            "name": "poetry",
            "markers": ["pyproject.toml"],
            "lock_files": ["poetry.lock"],
            "commands": {"build": "poetry build", "test": "poetry run pytest", "install": "poetry install"},
        },
        {
            "name": "pip",
            "markers": ["requirements.txt", "setup.py", "setup.cfg"],
            "lock_files": [],
            "commands": {"test": "pytest", "install": "pip install -r requirements.txt"},
        },
        {
            "name": "maven",
            "markers": ["pom.xml"],
            "lock_files": [],
            "commands": {"build": "mvn compile", "test": "mvn test", "install": "mvn install"},
        },
        {
            "name": "gradle",
            "markers": ["build.gradle", "build.gradle.kts"],
            "lock_files": ["gradle.lockfile"],
            "commands": {"build": "gradle build", "test": "gradle test"},
        },
        {
            "name": "make",
            "markers": ["Makefile"],
            "lock_files": [],
            "commands": {"build": "make", "test": "make test"},
        },
    ]

    def detect(self, repo_root):
        detected = []
        for system in self.BUILD_SYSTEMS:
            for marker in system["markers"]:
                if (Path(repo_root) / marker).exists():
                    # Check lock files to disambiguate (npm vs yarn vs pnpm)
                    has_lock = any(
                        (Path(repo_root) / lock).exists()
                        for lock in system["lock_files"]
                    )
                    detected.append({
                        "name": system["name"],
                        "confirmed": has_lock or not system["lock_files"],
                        "commands": system["commands"],
                    })
                    break
        return detected
```

---

## Monorepo and Workspace Detection

### Common Monorepo Patterns

| Tool | Configuration | Detection |
|---|---|---|
| **npm workspaces** | `package.json: { "workspaces": [...] }` | Parse package.json workspaces field |
| **yarn workspaces** | `package.json: { "workspaces": [...] }` | Same as npm |
| **pnpm workspaces** | `pnpm-workspace.yaml` | Parse workspace file |
| **Lerna** | `lerna.json` | Parse lerna config |
| **Nx** | `nx.json`, `workspace.json` | Parse Nx config |
| **Turborepo** | `turbo.json` | Parse turbo config |
| **Cargo workspaces** | `Cargo.toml: [workspace]` | Parse Cargo.toml |
| **Go workspaces** | `go.work` | Parse go.work file |
| **Bazel** | `WORKSPACE`, `MODULE.bazel` | Detect workspace file |

```python
class MonorepoDetector:
    def detect(self, repo_root):
        root = Path(repo_root)
        result = {"is_monorepo": False, "type": None, "packages": []}

        # Check npm/yarn/pnpm workspaces
        pkg_json = root / "package.json"
        if pkg_json.exists():
            pkg = json.loads(pkg_json.read_text())
            workspaces = pkg.get("workspaces", [])
            if isinstance(workspaces, dict):
                workspaces = workspaces.get("packages", [])
            if workspaces:
                result["is_monorepo"] = True
                result["type"] = "npm-workspaces"
                result["packages"] = self.resolve_globs(root, workspaces)

        # Check pnpm workspace
        pnpm_ws = root / "pnpm-workspace.yaml"
        if pnpm_ws.exists():
            import yaml
            ws_config = yaml.safe_load(pnpm_ws.read_text())
            packages = ws_config.get("packages", [])
            result["is_monorepo"] = True
            result["type"] = "pnpm-workspaces"
            result["packages"] = self.resolve_globs(root, packages)

        # Check Cargo workspace
        cargo_toml = root / "Cargo.toml"
        if cargo_toml.exists():
            import tomllib
            cargo = tomllib.loads(cargo_toml.read_text())
            if "workspace" in cargo:
                members = cargo["workspace"].get("members", [])
                result["is_monorepo"] = True
                result["type"] = "cargo-workspace"
                result["packages"] = self.resolve_globs(root, members)

        # Check Turborepo
        if (root / "turbo.json").exists():
            result["is_monorepo"] = True
            result["type"] = "turborepo"

        # Check Nx
        if (root / "nx.json").exists():
            result["is_monorepo"] = True
            result["type"] = "nx"

        return result
```

---

## Project Instruction Files

### The CLAUDE.md / CODEX.md / AGENTS.md Pattern

The most impactful recent development in project detection is the **project instruction file** — a markdown file at the project root that tells agents about the project:

**CLAUDE.md (Claude Code):**
```markdown
# Project: E-Commerce API

## Architecture
- Express.js REST API with TypeScript
- PostgreSQL database with Prisma ORM
- Redis for caching and session management

## Build & Test
- Build: `npm run build`
- Test: `npm test`
- Lint: `npm run lint`

## Conventions
- Use functional components with hooks (no class components)
- All database queries go through Prisma client
- Error responses use the AppError class from src/utils/errors.ts
- Tests use vitest, not jest

## Important Notes
- Never modify migration files directly
- The auth middleware is in src/middleware/auth.ts
- Environment variables are validated at startup in src/config.ts
```

**CODEX.md / AGENTS.md (Codex):**
Similar format, with Codex supporting hierarchical instruction files — a root AGENTS.md plus subdirectory AGENTS.md files that apply only within that subtree:

```
project/
├── AGENTS.md          # Root-level instructions
├── src/
│   ├── AGENTS.md      # Source-specific instructions
│   ├── api/
│   │   └── AGENTS.md  # API-specific instructions
│   └── models/
└── tests/
    └── AGENTS.md      # Test-specific instructions
```

### How Agents Process Instruction Files

```python
class InstructionFileLoader:
    INSTRUCTION_FILES = [
        "CLAUDE.md", "CODEX.md", "AGENTS.md",
        ".claude", ".codex",
        "GEMINI.md", ".gemini",
        "COPILOT.md",
    ]

    def load(self, repo_root, current_file=None):
        instructions = []

        # Load root-level instructions
        for filename in self.INSTRUCTION_FILES:
            filepath = Path(repo_root) / filename
            if filepath.exists():
                instructions.append({
                    "scope": "project",
                    "source": str(filepath),
                    "content": filepath.read_text(),
                })

        # Load directory-level instructions (if we know the current file)
        if current_file:
            directory = Path(current_file).parent
            while directory != Path(repo_root).parent:
                for filename in self.INSTRUCTION_FILES:
                    filepath = directory / filename
                    if filepath.exists():
                        instructions.append({
                            "scope": str(directory.relative_to(repo_root)),
                            "source": str(filepath),
                            "content": filepath.read_text(),
                        })
                directory = directory.parent

        return instructions
```

---

## Test Framework Detection

Knowing how to run tests is critical for agents that verify their changes:

```python
TEST_FRAMEWORKS = {
    "jest": {
        "configs": ["jest.config.js", "jest.config.ts", "jest.config.cjs"],
        "package_key": "jest",
        "test_patterns": ["**/*.test.ts", "**/*.spec.ts", "**/__tests__/**"],
        "command": "npx jest",
    },
    "vitest": {
        "configs": ["vitest.config.ts", "vitest.config.js"],
        "package_key": "vitest",
        "test_patterns": ["**/*.test.ts", "**/*.spec.ts"],
        "command": "npx vitest run",
    },
    "pytest": {
        "configs": ["pytest.ini", "pyproject.toml", "setup.cfg", "conftest.py"],
        "package_key": "pytest",
        "test_patterns": ["**/test_*.py", "**/*_test.py"],
        "command": "pytest",
    },
    "go-test": {
        "configs": ["go.mod"],
        "test_patterns": ["**/*_test.go"],
        "command": "go test ./...",
    },
    "cargo-test": {
        "configs": ["Cargo.toml"],
        "test_patterns": ["**/tests/**/*.rs", "src/**/*.rs"],
        "command": "cargo test",
    },
    "rspec": {
        "configs": [".rspec", "spec/spec_helper.rb"],
        "test_patterns": ["spec/**/*_spec.rb"],
        "command": "bundle exec rspec",
    },
}

def detect_test_framework(repo_root):
    """Detect the test framework used in the project."""
    root = Path(repo_root)
    detected = []

    for name, config in TEST_FRAMEWORKS.items():
        for config_file in config["configs"]:
            if (root / config_file).exists():
                detected.append({
                    "name": name,
                    "command": config["command"],
                    "patterns": config["test_patterns"],
                })
                break

    return detected
```

---

## Comprehensive Project Analysis

### Putting It All Together

```python
class ProjectAnalyzer:
    """Comprehensive project analysis for coding agents."""

    def analyze(self, repo_root):
        return {
            "languages": self.detect_languages(repo_root),
            "frameworks": self.detect_frameworks(repo_root),
            "build_system": self.detect_build_system(repo_root),
            "test_framework": self.detect_test_framework(repo_root),
            "monorepo": self.detect_monorepo(repo_root),
            "instructions": self.load_instructions(repo_root),
            "structure": self.analyze_structure(repo_root),
            "conventions": self.detect_conventions(repo_root),
        }

    def detect_conventions(self, repo_root):
        """Detect coding conventions from config files."""
        root = Path(repo_root)
        conventions = {}

        # Linting config
        linter_configs = {
            ".eslintrc.js": "eslint",
            ".eslintrc.json": "eslint",
            "eslint.config.js": "eslint (flat)",
            ".prettierrc": "prettier",
            "biome.json": "biome",
            "ruff.toml": "ruff",
            ".flake8": "flake8",
            ".golangci.yml": "golangci-lint",
            "rustfmt.toml": "rustfmt",
            ".editorconfig": "editorconfig",
        }

        for config_file, tool in linter_configs.items():
            if (root / config_file).exists():
                conventions[tool] = str(root / config_file)

        # TypeScript config
        if (root / "tsconfig.json").exists():
            conventions["typescript"] = "tsconfig.json"

        return conventions

    def analyze_structure(self, repo_root):
        """Analyze the project directory structure."""
        root = Path(repo_root)
        structure = {
            "source_dirs": [],
            "test_dirs": [],
            "config_dirs": [],
            "doc_dirs": [],
        }

        common_source = ["src", "lib", "app", "pkg", "internal", "cmd"]
        common_test = ["test", "tests", "spec", "__tests__", "e2e"]
        common_config = ["config", "conf", ".config"]
        common_doc = ["docs", "doc", "documentation"]

        for dirname in common_source:
            if (root / dirname).is_dir():
                structure["source_dirs"].append(dirname)

        for dirname in common_test:
            if (root / dirname).is_dir():
                structure["test_dirs"].append(dirname)

        for dirname in common_config:
            if (root / dirname).is_dir():
                structure["config_dirs"].append(dirname)

        for dirname in common_doc:
            if (root / dirname).is_dir():
                structure["doc_dirs"].append(dirname)

        return structure
```

---

## Key Takeaways

1. **Project instruction files (CLAUDE.md, AGENTS.md) are the most impactful detection mechanism.** They provide explicit, human-curated information that no automated detection can match.

2. **Language detection is trivial; framework detection is where the value is.** Knowing a project uses TypeScript tells the agent little; knowing it uses Next.js with Prisma tells it everything about the architecture.

3. **Build and test detection enables verification.** Agents that can run `npm test` or `cargo test` after editing can verify their changes, dramatically improving reliability.

4. **Monorepo detection prevents scope creep.** In a monorepo, the agent should focus on the relevant package, not the entire repository.

5. **Convention detection prevents style violations.** Reading linter configs tells the agent what style rules to follow, avoiding issues that would be caught in code review.

6. **The trend is toward explicit project files.** Rather than trying to infer everything, the ecosystem is converging on explicit instruction files that tell agents what they need to know.