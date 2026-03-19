# Junie CLI — Context Management

## Overview

Junie's context management strategy is shaped by two key factors: its JetBrains
heritage (which provides deep project understanding capabilities) and its multi-model
architecture (which requires intelligent context routing between different LLMs).

Unlike agents that rely primarily on file content and conversation history, Junie
builds context from project metadata, build system analysis, IDE inspection data
(when available), and project-level configuration files — creating a richer
understanding of the codebase before the LLM ever sees the code.

## Context Sources

### 1. Project Structure Analysis

Junie builds a project model from the directory structure and configuration files:

```
Project Context {
  root: "/home/user/my-project"
  type: JAVA_MAVEN
  
  modules: [
    Module {
      name: "core"
      path: "core/"
      sources: ["src/main/java/"]
      tests: ["src/test/java/"]
      resources: ["src/main/resources/"]
    },
    Module {
      name: "web"
      path: "web/"
      sources: ["src/main/java/"]
      tests: ["src/test/java/"]
      dependencies: ["core"]
    }
  ]
  
  build_system: MAVEN
  java_version: 17
  frameworks: [SPRING_BOOT, SPRING_DATA_JPA]
  test_framework: JUNIT_5
  
  significant_files: [
    "pom.xml",
    "web/pom.xml", 
    "core/pom.xml",
    "application.yml",
    ".editorconfig"
  ]
}
```

This structural context informs:
- **Where to look** for relevant code
- **How to build and test** the project
- **What conventions** the project follows
- **How modules relate** to each other

### 2. Build File Parsing

Build files are the richest source of project metadata:

#### Maven (pom.xml) Context Extraction

```xml
<!-- Junie extracts: -->
<project>
  <groupId>com.example</groupId>         <!-- Organization context -->
  <artifactId>user-service</artifactId>  <!-- Project identity -->
  <version>2.1.0</version>               <!-- Maturity indicator -->
  
  <parent>
    <artifactId>spring-boot-starter-parent</artifactId>
    <version>3.2.0</version>              <!-- Framework version -->
  </parent>
  
  <dependencies>
    <dependency>
      <groupId>org.springframework.boot</groupId>
      <artifactId>spring-boot-starter-web</artifactId>
      <!-- → This is a Spring Boot web project -->
    </dependency>
    <dependency>
      <groupId>org.springframework.boot</groupId>
      <artifactId>spring-boot-starter-data-jpa</artifactId>
      <!-- → Uses JPA for persistence -->
    </dependency>
    <dependency>
      <groupId>org.junit.jupiter</groupId>
      <artifactId>junit-jupiter</artifactId>
      <scope>test</scope>
      <!-- → Uses JUnit 5 for testing -->
    </dependency>
  </dependencies>
</project>
```

#### package.json Context Extraction

```json
{
  "name": "my-react-app",
  "dependencies": {
    "react": "^18.2.0",
    "react-router-dom": "^6.0.0",
    "axios": "^1.6.0"
  },
  "devDependencies": {
    "jest": "^29.0.0",
    "@testing-library/react": "^14.0.0",
    "typescript": "^5.3.0",
    "eslint": "^8.0.0"
  },
  "scripts": {
    "build": "react-scripts build",
    "test": "react-scripts test",
    "lint": "eslint src/"
  }
}
```

From this, Junie infers:
- React 18 with TypeScript (modern React patterns)
- React Router for navigation (SPA architecture)
- Jest + Testing Library for testing (component testing approach)
- ESLint for linting (code quality standards)
- CRA-based build system (react-scripts)

#### pyproject.toml Context Extraction

```toml
[project]
name = "data-pipeline"
requires-python = ">=3.11"
dependencies = [
    "pandas>=2.0",
    "sqlalchemy>=2.0",
    "fastapi>=0.100",
]

[project.optional-dependencies]
test = [
    "pytest>=7.0",
    "pytest-asyncio",
    "httpx",
]

[tool.pytest.ini_options]
testpaths = ["tests"]
asyncio_mode = "auto"
```

From this, Junie infers:
- Python 3.11+ data pipeline project
- Uses pandas for data processing, SQLAlchemy for DB, FastAPI for API
- Async-first testing with pytest-asyncio
- Tests in `tests/` directory

### 3. IDE Inspection Data (IDE Mode)

When running in the IDE, Junie has access to real-time analysis data:

```
IDE Context {
  // Type information for all symbols
  type_index: {
    "UserService.createUser": "(CreateUserRequest) → User",
    "UserRepository.save": "(User) → User",
    ...
  }
  
  // Current inspections/warnings
  inspections: [
    Warning {
      file: "UserService.java"
      line: 42
      message: "Method 'validateEmail' is never used"
      severity: WARNING
      quickfix_available: true
    },
    ...
  ]
  
  // Compilation status
  compilation: {
    errors: 0
    warnings: 3
    last_successful_build: "2025-01-15T10:30:00Z"
  }
  
  // Test status
  test_status: {
    last_run: "2025-01-15T10:25:00Z"
    passed: 142
    failed: 0
    skipped: 3
  }
}
```

This inspection context is **not available in CLI mode**, which is one of the key
differences between the two operational modes.

### 4. AGENTS.md / Project Rules

Junie supports project-level configuration through AGENTS.md files (and potentially
other configuration formats):

```markdown
# AGENTS.md

## Project Overview
This is a Spring Boot microservice for user management.

## Coding Standards
- Use Java 17 features (records, sealed classes, pattern matching)
- Follow Google Java Style Guide
- All public methods must have Javadoc
- Use constructor injection, not field injection

## Testing Requirements
- Unit tests for all service methods
- Integration tests for repository methods
- Use @SpringBootTest for integration tests
- Minimum 80% code coverage

## Architecture
- Controller → Service → Repository pattern
- DTOs for API communication, entities for persistence
- Use MapStruct for DTO-entity mapping

## Do Not
- Do not use Lombok (project convention)
- Do not modify the database migration files
- Do not change the API versioning scheme
```

The AGENTS.md content is included in the context for every LLM interaction, ensuring
that the agent follows project-specific conventions.

#### AGENTS.md Placement and Hierarchy

```
project-root/
├── AGENTS.md                    # Project-level rules (always loaded)
├── src/
│   ├── AGENTS.md                # Source-specific rules (loaded for src/ files)
│   ├── main/
│   │   └── java/
│   │       └── com/
│   │           └── example/
│   │               ├── AGENTS.md  # Package-specific rules (if supported)
│   │               └── UserService.java
│   └── test/
│       └── AGENTS.md            # Test-specific rules
└── docs/
    └── AGENTS.md                # Documentation-specific rules
```

Rules from more specific AGENTS.md files likely override or supplement
rules from parent directories.

### 5. Conversation History

Junie maintains conversation context across the task lifecycle:

```
Conversation Context {
  messages: [
    { role: "user", content: "Add email validation to UserService" },
    { role: "assistant", content: "I'll analyze the codebase..." },
    { role: "tool_result", content: "UserService.java contents..." },
    { role: "assistant", content: "Plan: 1. Add validation, 2. ..." },
    { role: "tool_result", content: "Test results: 3 passed, 1 failed" },
    { role: "assistant", content: "Test failed, fixing..." },
    ...
  ]
  
  // Summarized context for long conversations
  summary: "Working on adding email validation to UserService.
            Created Validator utility. Running tests after fix."
  
  // Active file context
  open_files: {
    "UserService.java": { content: "...", modified: true },
    "Validator.java": { content: "...", created: true },
    "UserServiceTest.java": { content: "...", modified: true }
  }
}
```

### 6. Git Context

Git history provides additional project context:

```
Git Context {
  current_branch: "feature/email-validation"
  base_branch: "main"
  
  recent_commits: [
    { sha: "abc123", message: "Add user creation endpoint", author: "dev1" },
    { sha: "def456", message: "Setup Spring Boot project", author: "dev2" },
    ...
  ]
  
  uncommitted_changes: [
    { file: "UserService.java", status: "modified" },
    { file: "Validator.java", status: "added" },
  ]
  
  // Relevant file history
  file_history: {
    "UserService.java": [
      { sha: "abc123", message: "Add user creation endpoint" },
      { sha: "ghi789", message: "Initial service scaffolding" },
    ]
  }
}
```

## Multi-Model Context Routing

### The Context Routing Problem

Different LLMs have different strengths, and different sub-tasks need different
amounts and types of context. Junie's multi-model router must decide:

1. **Which model** to send each sub-task to
2. **How much context** to include
3. **What type of context** is most relevant
4. **How to format** the context for each model

### Context Routing Strategy

```
┌──────────────────────────────────────────────────┐
│              Context Router                       │
│                                                  │
│  Input: Sub-task + available context             │
│                                                  │
│  Step 1: Classify the sub-task                   │
│    - Planning? → Full context, reasoning model   │
│    - Simple edit? → Minimal context, fast model  │
│    - Debugging? → Error + code context, strong   │
│    - Analysis? → Broad context, reasoning model  │
│                                                  │
│  Step 2: Select relevant context                 │
│    - Always include: AGENTS.md, task description │
│    - For edits: Target file + nearby files       │
│    - For debugging: Error output + relevant code │
│    - For planning: Project structure + task       │
│                                                  │
│  Step 3: Format for target model                 │
│    - Each model may prefer different formats     │
│    - Token budget varies by model                │
│    - System prompt tailored to model strengths   │
│                                                  │
│  Step 4: Execute and collect response            │
└──────────────────────────────────────────────────┘
```

### Context Budget Management

Each model has a context window limit, and Junie must manage the budget:

```
Context Budget Allocation (example for 128K context window):

  System prompt + AGENTS.md:        ~2K tokens
  Task description + plan:          ~1K tokens
  Conversation summary:             ~2K tokens
  Active file contents:            ~20K tokens
  Related file contents:           ~10K tokens
  Build/test context:               ~2K tokens
  Tool definitions:                 ~3K tokens
  ─────────────────────────────────────────
  Total used:                      ~40K tokens
  Remaining for generation:        ~88K tokens
```

For models with smaller context windows, Junie must be more aggressive
about context pruning — summarizing conversations, truncating file contents,
and selecting only the most relevant files.

### Cross-Model Context Continuity

When switching between models during a task, Junie must maintain continuity:

```
Step 1: Planning (Claude Opus)
  Context: Full project structure, task description, AGENTS.md
  Output: Detailed plan with file modifications

Step 2: Implementation (Gemini Flash)
  Context: Plan from step 1, target file content, AGENTS.md
  Output: Code changes
  
  Challenge: Gemini Flash didn't see the full project context
  Solution: Include relevant excerpts from the plan + necessary files

Step 3: Debugging (Claude Sonnet)
  Context: Test failure output, modified files, original plan
  Output: Diagnosis and fix
  
  Challenge: Claude Sonnet didn't see the original planning context
  Solution: Include plan summary + specific failure details
```

This cross-model continuity is one of the hardest challenges in multi-model
architectures. Information loss at model boundaries can lead to inconsistent
or suboptimal results.

## Context Construction Pipeline

```
User Request
     │
     ▼
┌─────────────────┐
│  Parse Request    │ Extract intent, scope, constraints
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Load Project    │ Build files, AGENTS.md, directory structure
│  Metadata        │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Identify        │ Use project structure + request to find
│  Relevant Files  │ files that need to be in context
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Load File       │ Read relevant files, potentially
│  Contents        │ summarizing large files
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Add IDE Context │ Inspections, type info, test results
│  (if available)  │ (IDE mode only)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Load Git        │ Recent commits, current branch,
│  Context         │ uncommitted changes
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Format for      │ Assemble context, respect token budget,
│  Target Model    │ format for selected model
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Submit to LLM   │ Send formatted context + task to model
└─────────────────┘
```

## IDE Mode vs CLI Mode Context Comparison

| Context Source | IDE Mode | CLI Mode |
|---|---|---|
| Project structure | Full (from project model) | Partial (from directory scan) |
| Build file analysis | Deep (IDE parser) | Moderate (text analysis) |
| Type information | Complete (PSI) | None (LLM infers) |
| Inspections | Real-time | Not available |
| Test results | Structured (TestResult) | Parsed from terminal output |
| Import graph | Complete (reference resolution) | Not available |
| Compilation status | Real-time | Must run build command |
| AGENTS.md | Loaded at project open | Loaded at session start |
| Git context | Via IDE Git integration | Via git CLI commands |
| Conversation history | Same | Same |
| Framework knowledge | Plugin-enhanced | Heuristic |

## Context Caching and Refresh

### What Gets Cached

```
Stable context (cached for session duration):
  - Project structure (refreshed on file system changes)
  - Build file contents (refreshed on modification)
  - AGENTS.md rules (loaded once, refreshed on change)
  - Framework detection results

Semi-stable context (cached with TTL):
  - File contents (invalidated on modification)
  - Git status (refreshed periodically)
  - IDE inspections (updated continuously in IDE mode)

Volatile context (never cached):
  - Test results (always re-run)
  - Build results (always re-build)
  - Shell command output
```

### Context Refresh Triggers

```
File save → Refresh file content cache
             → Re-run inspections (IDE mode)
             → Update git status

Build completion → Update compilation status
                    → Refresh error/warning context

Test completion → Update test results context
                   → Mark failed tests for attention

Git operation → Refresh git context
                 → Update branch information

User message → Refresh conversation context
                → Re-evaluate relevant files
```

## Key Insights

1. **Build files are the rosetta stone**: The richest context comes from build system
   files. They tell the agent what language, framework, test system, and conventions
   the project uses — all without reading a single line of source code.

2. **IDE context is a massive advantage**: The type information, inspections, and
   structured test results available in IDE mode represent context that CLI agents
   simply cannot replicate. This gap is fundamental, not just incremental.

3. **Multi-model context routing is hard**: Maintaining coherent context across model
   boundaries is one of the toughest challenges in multi-model architectures. Each
   model switch is an opportunity for context loss.

4. **AGENTS.md standardization helps**: By supporting project-level configuration
   files, Junie allows teams to encode their conventions and requirements in a format
   the agent can always access, regardless of IDE or CLI mode.

5. **Context budget management is critical**: With multi-model routing, different
   models have different context windows. The agent must dynamically adjust what
   context to include based on both the task requirements and the target model's
   capabilities.
