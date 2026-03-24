import{_ as i,C as r,o as a,c as d,a2 as e,b as p,w as n,a as o,E as l,a3 as c}from"./chunks/framework.B72_uj9U.js";const C=JSON.parse('{"title":"Junie CLI — Context Management","description":"","frontmatter":{},"headers":[],"relativePath":"research/agents/junie-cli/context-management.md","filePath":"research/agents/junie-cli/context-management.md"}'),u={name:"research/agents/junie-cli/context-management.md"};function y(m,s,h,g,f,q){const t=r("Mermaid");return a(),d("div",null,[s[2]||(s[2]=e(`<h1 id="junie-cli-—-context-management" tabindex="-1">Junie CLI — Context Management <a class="header-anchor" href="#junie-cli-—-context-management" aria-label="Permalink to &quot;Junie CLI — Context Management&quot;">​</a></h1><h2 id="overview" tabindex="-1">Overview <a class="header-anchor" href="#overview" aria-label="Permalink to &quot;Overview&quot;">​</a></h2><p>Junie&#39;s context management strategy is shaped by two key factors: its JetBrains heritage (which provides deep project understanding capabilities) and its multi-model architecture (which requires intelligent context routing between different LLMs).</p><p>Unlike agents that rely primarily on file content and conversation history, Junie builds context from project metadata, build system analysis, IDE inspection data (when available), and project-level configuration files — creating a richer understanding of the codebase before the LLM ever sees the code.</p><h2 id="context-sources" tabindex="-1">Context Sources <a class="header-anchor" href="#context-sources" aria-label="Permalink to &quot;Context Sources&quot;">​</a></h2><h3 id="_1-project-structure-analysis" tabindex="-1">1. Project Structure Analysis <a class="header-anchor" href="#_1-project-structure-analysis" aria-label="Permalink to &quot;1. Project Structure Analysis&quot;">​</a></h3><p>Junie builds a project model from the directory structure and configuration files:</p><div class="language-"><button title="Copy Code" class="copy"></button><span class="lang"></span><pre class="shiki catppuccin-mocha vp-code" tabindex="0"><code><span class="line"><span>Project Context {</span></span>
<span class="line"><span>  root: &quot;/home/user/my-project&quot;</span></span>
<span class="line"><span>  type: JAVA_MAVEN</span></span>
<span class="line"><span>  </span></span>
<span class="line"><span>  modules: [</span></span>
<span class="line"><span>    Module {</span></span>
<span class="line"><span>      name: &quot;core&quot;</span></span>
<span class="line"><span>      path: &quot;core/&quot;</span></span>
<span class="line"><span>      sources: [&quot;src/main/java/&quot;]</span></span>
<span class="line"><span>      tests: [&quot;src/test/java/&quot;]</span></span>
<span class="line"><span>      resources: [&quot;src/main/resources/&quot;]</span></span>
<span class="line"><span>    },</span></span>
<span class="line"><span>    Module {</span></span>
<span class="line"><span>      name: &quot;web&quot;</span></span>
<span class="line"><span>      path: &quot;web/&quot;</span></span>
<span class="line"><span>      sources: [&quot;src/main/java/&quot;]</span></span>
<span class="line"><span>      tests: [&quot;src/test/java/&quot;]</span></span>
<span class="line"><span>      dependencies: [&quot;core&quot;]</span></span>
<span class="line"><span>    }</span></span>
<span class="line"><span>  ]</span></span>
<span class="line"><span>  </span></span>
<span class="line"><span>  build_system: MAVEN</span></span>
<span class="line"><span>  java_version: 17</span></span>
<span class="line"><span>  frameworks: [SPRING_BOOT, SPRING_DATA_JPA]</span></span>
<span class="line"><span>  test_framework: JUNIT_5</span></span>
<span class="line"><span>  </span></span>
<span class="line"><span>  significant_files: [</span></span>
<span class="line"><span>    &quot;pom.xml&quot;,</span></span>
<span class="line"><span>    &quot;web/pom.xml&quot;, </span></span>
<span class="line"><span>    &quot;core/pom.xml&quot;,</span></span>
<span class="line"><span>    &quot;application.yml&quot;,</span></span>
<span class="line"><span>    &quot;.editorconfig&quot;</span></span>
<span class="line"><span>  ]</span></span>
<span class="line"><span>}</span></span></code></pre></div><p>This structural context informs:</p><ul><li><strong>Where to look</strong> for relevant code</li><li><strong>How to build and test</strong> the project</li><li><strong>What conventions</strong> the project follows</li><li><strong>How modules relate</strong> to each other</li></ul><h3 id="_2-build-file-parsing" tabindex="-1">2. Build File Parsing <a class="header-anchor" href="#_2-build-file-parsing" aria-label="Permalink to &quot;2. Build File Parsing&quot;">​</a></h3><p>Build files are the richest source of project metadata:</p><h4 id="maven-pom-xml-context-extraction" tabindex="-1">Maven (pom.xml) Context Extraction <a class="header-anchor" href="#maven-pom-xml-context-extraction" aria-label="Permalink to &quot;Maven (pom.xml) Context Extraction&quot;">​</a></h4><div class="language-xml"><button title="Copy Code" class="copy"></button><span class="lang">xml</span><pre class="shiki catppuccin-mocha vp-code" tabindex="0"><code><span class="line"><span style="color:#6C7086;font-style:italic;">&lt;!-- Junie extracts: --&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">&lt;</span><span style="color:#89B4FA;">project</span><span style="color:#94E2D5;">&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">  &lt;</span><span style="color:#89B4FA;">groupId</span><span style="color:#94E2D5;">&gt;</span><span style="color:#CDD6F4;">com.example</span><span style="color:#94E2D5;">&lt;/</span><span style="color:#89B4FA;">groupId</span><span style="color:#94E2D5;">&gt;</span><span style="color:#6C7086;font-style:italic;">         &lt;!-- Organization context --&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">  &lt;</span><span style="color:#89B4FA;">artifactId</span><span style="color:#94E2D5;">&gt;</span><span style="color:#CDD6F4;">user-service</span><span style="color:#94E2D5;">&lt;/</span><span style="color:#89B4FA;">artifactId</span><span style="color:#94E2D5;">&gt;</span><span style="color:#6C7086;font-style:italic;">  &lt;!-- Project identity --&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">  &lt;</span><span style="color:#89B4FA;">version</span><span style="color:#94E2D5;">&gt;</span><span style="color:#CDD6F4;">2.1.0</span><span style="color:#94E2D5;">&lt;/</span><span style="color:#89B4FA;">version</span><span style="color:#94E2D5;">&gt;</span><span style="color:#6C7086;font-style:italic;">               &lt;!-- Maturity indicator --&gt;</span></span>
<span class="line"><span style="color:#CDD6F4;">  </span></span>
<span class="line"><span style="color:#94E2D5;">  &lt;</span><span style="color:#89B4FA;">parent</span><span style="color:#94E2D5;">&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">    &lt;</span><span style="color:#89B4FA;">artifactId</span><span style="color:#94E2D5;">&gt;</span><span style="color:#CDD6F4;">spring-boot-starter-parent</span><span style="color:#94E2D5;">&lt;/</span><span style="color:#89B4FA;">artifactId</span><span style="color:#94E2D5;">&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">    &lt;</span><span style="color:#89B4FA;">version</span><span style="color:#94E2D5;">&gt;</span><span style="color:#CDD6F4;">3.2.0</span><span style="color:#94E2D5;">&lt;/</span><span style="color:#89B4FA;">version</span><span style="color:#94E2D5;">&gt;</span><span style="color:#6C7086;font-style:italic;">              &lt;!-- Framework version --&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">  &lt;/</span><span style="color:#89B4FA;">parent</span><span style="color:#94E2D5;">&gt;</span></span>
<span class="line"><span style="color:#CDD6F4;">  </span></span>
<span class="line"><span style="color:#94E2D5;">  &lt;</span><span style="color:#89B4FA;">dependencies</span><span style="color:#94E2D5;">&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">    &lt;</span><span style="color:#89B4FA;">dependency</span><span style="color:#94E2D5;">&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">      &lt;</span><span style="color:#89B4FA;">groupId</span><span style="color:#94E2D5;">&gt;</span><span style="color:#CDD6F4;">org.springframework.boot</span><span style="color:#94E2D5;">&lt;/</span><span style="color:#89B4FA;">groupId</span><span style="color:#94E2D5;">&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">      &lt;</span><span style="color:#89B4FA;">artifactId</span><span style="color:#94E2D5;">&gt;</span><span style="color:#CDD6F4;">spring-boot-starter-web</span><span style="color:#94E2D5;">&lt;/</span><span style="color:#89B4FA;">artifactId</span><span style="color:#94E2D5;">&gt;</span></span>
<span class="line"><span style="color:#6C7086;font-style:italic;">      &lt;!-- → This is a Spring Boot web project --&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">    &lt;/</span><span style="color:#89B4FA;">dependency</span><span style="color:#94E2D5;">&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">    &lt;</span><span style="color:#89B4FA;">dependency</span><span style="color:#94E2D5;">&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">      &lt;</span><span style="color:#89B4FA;">groupId</span><span style="color:#94E2D5;">&gt;</span><span style="color:#CDD6F4;">org.springframework.boot</span><span style="color:#94E2D5;">&lt;/</span><span style="color:#89B4FA;">groupId</span><span style="color:#94E2D5;">&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">      &lt;</span><span style="color:#89B4FA;">artifactId</span><span style="color:#94E2D5;">&gt;</span><span style="color:#CDD6F4;">spring-boot-starter-data-jpa</span><span style="color:#94E2D5;">&lt;/</span><span style="color:#89B4FA;">artifactId</span><span style="color:#94E2D5;">&gt;</span></span>
<span class="line"><span style="color:#6C7086;font-style:italic;">      &lt;!-- → Uses JPA for persistence --&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">    &lt;/</span><span style="color:#89B4FA;">dependency</span><span style="color:#94E2D5;">&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">    &lt;</span><span style="color:#89B4FA;">dependency</span><span style="color:#94E2D5;">&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">      &lt;</span><span style="color:#89B4FA;">groupId</span><span style="color:#94E2D5;">&gt;</span><span style="color:#CDD6F4;">org.junit.jupiter</span><span style="color:#94E2D5;">&lt;/</span><span style="color:#89B4FA;">groupId</span><span style="color:#94E2D5;">&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">      &lt;</span><span style="color:#89B4FA;">artifactId</span><span style="color:#94E2D5;">&gt;</span><span style="color:#CDD6F4;">junit-jupiter</span><span style="color:#94E2D5;">&lt;/</span><span style="color:#89B4FA;">artifactId</span><span style="color:#94E2D5;">&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">      &lt;</span><span style="color:#89B4FA;">scope</span><span style="color:#94E2D5;">&gt;</span><span style="color:#CDD6F4;">test</span><span style="color:#94E2D5;">&lt;/</span><span style="color:#89B4FA;">scope</span><span style="color:#94E2D5;">&gt;</span></span>
<span class="line"><span style="color:#6C7086;font-style:italic;">      &lt;!-- → Uses JUnit 5 for testing --&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">    &lt;/</span><span style="color:#89B4FA;">dependency</span><span style="color:#94E2D5;">&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">  &lt;/</span><span style="color:#89B4FA;">dependencies</span><span style="color:#94E2D5;">&gt;</span></span>
<span class="line"><span style="color:#94E2D5;">&lt;/</span><span style="color:#89B4FA;">project</span><span style="color:#94E2D5;">&gt;</span></span></code></pre></div><h4 id="package-json-context-extraction" tabindex="-1">package.json Context Extraction <a class="header-anchor" href="#package-json-context-extraction" aria-label="Permalink to &quot;package.json Context Extraction&quot;">​</a></h4><div class="language-json"><button title="Copy Code" class="copy"></button><span class="lang">json</span><pre class="shiki catppuccin-mocha vp-code" tabindex="0"><code><span class="line"><span style="color:#9399B2;">{</span></span>
<span class="line"><span style="color:#9399B2;">  &quot;</span><span style="color:#89B4FA;">name</span><span style="color:#9399B2;">&quot;</span><span style="color:#9399B2;">:</span><span style="color:#A6E3A1;"> &quot;my-react-app&quot;</span><span style="color:#9399B2;">,</span></span>
<span class="line"><span style="color:#9399B2;">  &quot;</span><span style="color:#89B4FA;">dependencies</span><span style="color:#9399B2;">&quot;</span><span style="color:#9399B2;">:</span><span style="color:#9399B2;"> {</span></span>
<span class="line"><span style="color:#9399B2;">    &quot;</span><span style="color:#89B4FA;">react</span><span style="color:#9399B2;">&quot;</span><span style="color:#9399B2;">:</span><span style="color:#A6E3A1;"> &quot;^18.2.0&quot;</span><span style="color:#9399B2;">,</span></span>
<span class="line"><span style="color:#9399B2;">    &quot;</span><span style="color:#89B4FA;">react-router-dom</span><span style="color:#9399B2;">&quot;</span><span style="color:#9399B2;">:</span><span style="color:#A6E3A1;"> &quot;^6.0.0&quot;</span><span style="color:#9399B2;">,</span></span>
<span class="line"><span style="color:#9399B2;">    &quot;</span><span style="color:#89B4FA;">axios</span><span style="color:#9399B2;">&quot;</span><span style="color:#9399B2;">:</span><span style="color:#A6E3A1;"> &quot;^1.6.0&quot;</span></span>
<span class="line"><span style="color:#9399B2;">  },</span></span>
<span class="line"><span style="color:#9399B2;">  &quot;</span><span style="color:#89B4FA;">devDependencies</span><span style="color:#9399B2;">&quot;</span><span style="color:#9399B2;">:</span><span style="color:#9399B2;"> {</span></span>
<span class="line"><span style="color:#9399B2;">    &quot;</span><span style="color:#89B4FA;">jest</span><span style="color:#9399B2;">&quot;</span><span style="color:#9399B2;">:</span><span style="color:#A6E3A1;"> &quot;^29.0.0&quot;</span><span style="color:#9399B2;">,</span></span>
<span class="line"><span style="color:#9399B2;">    &quot;</span><span style="color:#89B4FA;">@testing-library/react</span><span style="color:#9399B2;">&quot;</span><span style="color:#9399B2;">:</span><span style="color:#A6E3A1;"> &quot;^14.0.0&quot;</span><span style="color:#9399B2;">,</span></span>
<span class="line"><span style="color:#9399B2;">    &quot;</span><span style="color:#89B4FA;">typescript</span><span style="color:#9399B2;">&quot;</span><span style="color:#9399B2;">:</span><span style="color:#A6E3A1;"> &quot;^5.3.0&quot;</span><span style="color:#9399B2;">,</span></span>
<span class="line"><span style="color:#9399B2;">    &quot;</span><span style="color:#89B4FA;">eslint</span><span style="color:#9399B2;">&quot;</span><span style="color:#9399B2;">:</span><span style="color:#A6E3A1;"> &quot;^8.0.0&quot;</span></span>
<span class="line"><span style="color:#9399B2;">  },</span></span>
<span class="line"><span style="color:#9399B2;">  &quot;</span><span style="color:#89B4FA;">scripts</span><span style="color:#9399B2;">&quot;</span><span style="color:#9399B2;">:</span><span style="color:#9399B2;"> {</span></span>
<span class="line"><span style="color:#9399B2;">    &quot;</span><span style="color:#89B4FA;">build</span><span style="color:#9399B2;">&quot;</span><span style="color:#9399B2;">:</span><span style="color:#A6E3A1;"> &quot;react-scripts build&quot;</span><span style="color:#9399B2;">,</span></span>
<span class="line"><span style="color:#9399B2;">    &quot;</span><span style="color:#89B4FA;">test</span><span style="color:#9399B2;">&quot;</span><span style="color:#9399B2;">:</span><span style="color:#A6E3A1;"> &quot;react-scripts test&quot;</span><span style="color:#9399B2;">,</span></span>
<span class="line"><span style="color:#9399B2;">    &quot;</span><span style="color:#89B4FA;">lint</span><span style="color:#9399B2;">&quot;</span><span style="color:#9399B2;">:</span><span style="color:#A6E3A1;"> &quot;eslint src/&quot;</span></span>
<span class="line"><span style="color:#9399B2;">  }</span></span>
<span class="line"><span style="color:#9399B2;">}</span></span></code></pre></div><p>From this, Junie infers:</p><ul><li>React 18 with TypeScript (modern React patterns)</li><li>React Router for navigation (SPA architecture)</li><li>Jest + Testing Library for testing (component testing approach)</li><li>ESLint for linting (code quality standards)</li><li>CRA-based build system (react-scripts)</li></ul><h4 id="pyproject-toml-context-extraction" tabindex="-1">pyproject.toml Context Extraction <a class="header-anchor" href="#pyproject-toml-context-extraction" aria-label="Permalink to &quot;pyproject.toml Context Extraction&quot;">​</a></h4><div class="language-toml"><button title="Copy Code" class="copy"></button><span class="lang">toml</span><pre class="shiki catppuccin-mocha vp-code" tabindex="0"><code><span class="line"><span style="color:#9399B2;">[</span><span style="color:#CDD6F4;">project</span><span style="color:#9399B2;">]</span></span>
<span class="line"><span style="color:#CDD6F4;">name </span><span style="color:#94E2D5;">=</span><span style="color:#A6E3A1;"> &quot;data-pipeline&quot;</span></span>
<span class="line"><span style="color:#CDD6F4;">requires-python </span><span style="color:#94E2D5;">=</span><span style="color:#A6E3A1;"> &quot;&gt;=3.11&quot;</span></span>
<span class="line"><span style="color:#CDD6F4;">dependencies </span><span style="color:#94E2D5;">=</span><span style="color:#9399B2;"> [</span></span>
<span class="line"><span style="color:#A6E3A1;">    &quot;pandas&gt;=2.0&quot;</span><span style="color:#9399B2;">,</span></span>
<span class="line"><span style="color:#A6E3A1;">    &quot;sqlalchemy&gt;=2.0&quot;</span><span style="color:#9399B2;">,</span></span>
<span class="line"><span style="color:#A6E3A1;">    &quot;fastapi&gt;=0.100&quot;</span><span style="color:#9399B2;">,</span></span>
<span class="line"><span style="color:#9399B2;">]</span></span>
<span class="line"></span>
<span class="line"><span style="color:#9399B2;">[</span><span style="color:#CDD6F4;">project.optional-dependencies</span><span style="color:#9399B2;">]</span></span>
<span class="line"><span style="color:#CDD6F4;">test </span><span style="color:#94E2D5;">=</span><span style="color:#9399B2;"> [</span></span>
<span class="line"><span style="color:#A6E3A1;">    &quot;pytest&gt;=7.0&quot;</span><span style="color:#9399B2;">,</span></span>
<span class="line"><span style="color:#A6E3A1;">    &quot;pytest-asyncio&quot;</span><span style="color:#9399B2;">,</span></span>
<span class="line"><span style="color:#A6E3A1;">    &quot;httpx&quot;</span><span style="color:#9399B2;">,</span></span>
<span class="line"><span style="color:#9399B2;">]</span></span>
<span class="line"></span>
<span class="line"><span style="color:#9399B2;">[</span><span style="color:#CDD6F4;">tool.pytest.ini_options</span><span style="color:#9399B2;">]</span></span>
<span class="line"><span style="color:#CDD6F4;">testpaths </span><span style="color:#94E2D5;">=</span><span style="color:#9399B2;"> [</span><span style="color:#A6E3A1;">&quot;tests&quot;</span><span style="color:#9399B2;">]</span></span>
<span class="line"><span style="color:#CDD6F4;">asyncio_mode </span><span style="color:#94E2D5;">=</span><span style="color:#A6E3A1;"> &quot;auto&quot;</span></span></code></pre></div><p>From this, Junie infers:</p><ul><li>Python 3.11+ data pipeline project</li><li>Uses pandas for data processing, SQLAlchemy for DB, FastAPI for API</li><li>Async-first testing with pytest-asyncio</li><li>Tests in <code>tests/</code> directory</li></ul><h3 id="_3-ide-inspection-data-ide-mode" tabindex="-1">3. IDE Inspection Data (IDE Mode) <a class="header-anchor" href="#_3-ide-inspection-data-ide-mode" aria-label="Permalink to &quot;3. IDE Inspection Data (IDE Mode)&quot;">​</a></h3><p>When running in the IDE, Junie has access to real-time analysis data:</p><div class="language-"><button title="Copy Code" class="copy"></button><span class="lang"></span><pre class="shiki catppuccin-mocha vp-code" tabindex="0"><code><span class="line"><span>IDE Context {</span></span>
<span class="line"><span>  // Type information for all symbols</span></span>
<span class="line"><span>  type_index: {</span></span>
<span class="line"><span>    &quot;UserService.createUser&quot;: &quot;(CreateUserRequest) → User&quot;,</span></span>
<span class="line"><span>    &quot;UserRepository.save&quot;: &quot;(User) → User&quot;,</span></span>
<span class="line"><span>    ...</span></span>
<span class="line"><span>  }</span></span>
<span class="line"><span>  </span></span>
<span class="line"><span>  // Current inspections/warnings</span></span>
<span class="line"><span>  inspections: [</span></span>
<span class="line"><span>    Warning {</span></span>
<span class="line"><span>      file: &quot;UserService.java&quot;</span></span>
<span class="line"><span>      line: 42</span></span>
<span class="line"><span>      message: &quot;Method &#39;validateEmail&#39; is never used&quot;</span></span>
<span class="line"><span>      severity: WARNING</span></span>
<span class="line"><span>      quickfix_available: true</span></span>
<span class="line"><span>    },</span></span>
<span class="line"><span>    ...</span></span>
<span class="line"><span>  ]</span></span>
<span class="line"><span>  </span></span>
<span class="line"><span>  // Compilation status</span></span>
<span class="line"><span>  compilation: {</span></span>
<span class="line"><span>    errors: 0</span></span>
<span class="line"><span>    warnings: 3</span></span>
<span class="line"><span>    last_successful_build: &quot;2025-01-15T10:30:00Z&quot;</span></span>
<span class="line"><span>  }</span></span>
<span class="line"><span>  </span></span>
<span class="line"><span>  // Test status</span></span>
<span class="line"><span>  test_status: {</span></span>
<span class="line"><span>    last_run: &quot;2025-01-15T10:25:00Z&quot;</span></span>
<span class="line"><span>    passed: 142</span></span>
<span class="line"><span>    failed: 0</span></span>
<span class="line"><span>    skipped: 3</span></span>
<span class="line"><span>  }</span></span>
<span class="line"><span>}</span></span></code></pre></div><p>This inspection context is <strong>not available in CLI mode</strong>, which is one of the key differences between the two operational modes.</p><h3 id="_4-agents-md-project-rules" tabindex="-1">4. AGENTS.md / Project Rules <a class="header-anchor" href="#_4-agents-md-project-rules" aria-label="Permalink to &quot;4. AGENTS.md / Project Rules&quot;">​</a></h3><p>Junie supports project-level configuration through AGENTS.md files (and potentially other configuration formats):</p><div class="language-markdown"><button title="Copy Code" class="copy"></button><span class="lang">markdown</span><pre class="shiki catppuccin-mocha vp-code" tabindex="0"><code><span class="line"><span style="color:#F38BA8;"># AGENTS.md</span></span>
<span class="line"></span>
<span class="line"><span style="color:#FAB387;">## Project Overview</span></span>
<span class="line"><span style="color:#CDD6F4;">This is a Spring Boot microservice for user management.</span></span>
<span class="line"></span>
<span class="line"><span style="color:#FAB387;">## Coding Standards</span></span>
<span class="line"><span style="color:#94E2D5;">-</span><span style="color:#CDD6F4;"> Use Java 17 features (records, sealed classes, pattern matching)</span></span>
<span class="line"><span style="color:#94E2D5;">-</span><span style="color:#CDD6F4;"> Follow Google Java Style Guide</span></span>
<span class="line"><span style="color:#94E2D5;">-</span><span style="color:#CDD6F4;"> All public methods must have Javadoc</span></span>
<span class="line"><span style="color:#94E2D5;">-</span><span style="color:#CDD6F4;"> Use constructor injection, not field injection</span></span>
<span class="line"></span>
<span class="line"><span style="color:#FAB387;">## Testing Requirements</span></span>
<span class="line"><span style="color:#94E2D5;">-</span><span style="color:#CDD6F4;"> Unit tests for all service methods</span></span>
<span class="line"><span style="color:#94E2D5;">-</span><span style="color:#CDD6F4;"> Integration tests for repository methods</span></span>
<span class="line"><span style="color:#94E2D5;">-</span><span style="color:#CDD6F4;"> Use @SpringBootTest for integration tests</span></span>
<span class="line"><span style="color:#94E2D5;">-</span><span style="color:#CDD6F4;"> Minimum 80% code coverage</span></span>
<span class="line"></span>
<span class="line"><span style="color:#FAB387;">## Architecture</span></span>
<span class="line"><span style="color:#94E2D5;">-</span><span style="color:#CDD6F4;"> Controller → Service → Repository pattern</span></span>
<span class="line"><span style="color:#94E2D5;">-</span><span style="color:#CDD6F4;"> DTOs for API communication, entities for persistence</span></span>
<span class="line"><span style="color:#94E2D5;">-</span><span style="color:#CDD6F4;"> Use MapStruct for DTO-entity mapping</span></span>
<span class="line"></span>
<span class="line"><span style="color:#FAB387;">## Do Not</span></span>
<span class="line"><span style="color:#94E2D5;">-</span><span style="color:#CDD6F4;"> Do not use Lombok (project convention)</span></span>
<span class="line"><span style="color:#94E2D5;">-</span><span style="color:#CDD6F4;"> Do not modify the database migration files</span></span>
<span class="line"><span style="color:#94E2D5;">-</span><span style="color:#CDD6F4;"> Do not change the API versioning scheme</span></span></code></pre></div><p>The AGENTS.md content is included in the context for every LLM interaction, ensuring that the agent follows project-specific conventions.</p><h4 id="agents-md-placement-and-hierarchy" tabindex="-1">AGENTS.md Placement and Hierarchy <a class="header-anchor" href="#agents-md-placement-and-hierarchy" aria-label="Permalink to &quot;AGENTS.md Placement and Hierarchy&quot;">​</a></h4><div class="language-"><button title="Copy Code" class="copy"></button><span class="lang"></span><pre class="shiki catppuccin-mocha vp-code" tabindex="0"><code><span class="line"><span>project-root/</span></span>
<span class="line"><span>├── AGENTS.md                    # Project-level rules (always loaded)</span></span>
<span class="line"><span>├── src/</span></span>
<span class="line"><span>│   ├── AGENTS.md                # Source-specific rules (loaded for src/ files)</span></span>
<span class="line"><span>│   ├── main/</span></span>
<span class="line"><span>│   │   └── java/</span></span>
<span class="line"><span>│   │       └── com/</span></span>
<span class="line"><span>│   │           └── example/</span></span>
<span class="line"><span>│   │               ├── AGENTS.md  # Package-specific rules (if supported)</span></span>
<span class="line"><span>│   │               └── UserService.java</span></span>
<span class="line"><span>│   └── test/</span></span>
<span class="line"><span>│       └── AGENTS.md            # Test-specific rules</span></span>
<span class="line"><span>└── docs/</span></span>
<span class="line"><span>    └── AGENTS.md                # Documentation-specific rules</span></span></code></pre></div><p>Rules from more specific AGENTS.md files likely override or supplement rules from parent directories.</p><h3 id="_5-conversation-history" tabindex="-1">5. Conversation History <a class="header-anchor" href="#_5-conversation-history" aria-label="Permalink to &quot;5. Conversation History&quot;">​</a></h3><p>Junie maintains conversation context across the task lifecycle:</p><div class="language-"><button title="Copy Code" class="copy"></button><span class="lang"></span><pre class="shiki catppuccin-mocha vp-code" tabindex="0"><code><span class="line"><span>Conversation Context {</span></span>
<span class="line"><span>  messages: [</span></span>
<span class="line"><span>    { role: &quot;user&quot;, content: &quot;Add email validation to UserService&quot; },</span></span>
<span class="line"><span>    { role: &quot;assistant&quot;, content: &quot;I&#39;ll analyze the codebase...&quot; },</span></span>
<span class="line"><span>    { role: &quot;tool_result&quot;, content: &quot;UserService.java contents...&quot; },</span></span>
<span class="line"><span>    { role: &quot;assistant&quot;, content: &quot;Plan: 1. Add validation, 2. ...&quot; },</span></span>
<span class="line"><span>    { role: &quot;tool_result&quot;, content: &quot;Test results: 3 passed, 1 failed&quot; },</span></span>
<span class="line"><span>    { role: &quot;assistant&quot;, content: &quot;Test failed, fixing...&quot; },</span></span>
<span class="line"><span>    ...</span></span>
<span class="line"><span>  ]</span></span>
<span class="line"><span>  </span></span>
<span class="line"><span>  // Summarized context for long conversations</span></span>
<span class="line"><span>  summary: &quot;Working on adding email validation to UserService.</span></span>
<span class="line"><span>            Created Validator utility. Running tests after fix.&quot;</span></span>
<span class="line"><span>  </span></span>
<span class="line"><span>  // Active file context</span></span>
<span class="line"><span>  open_files: {</span></span>
<span class="line"><span>    &quot;UserService.java&quot;: { content: &quot;...&quot;, modified: true },</span></span>
<span class="line"><span>    &quot;Validator.java&quot;: { content: &quot;...&quot;, created: true },</span></span>
<span class="line"><span>    &quot;UserServiceTest.java&quot;: { content: &quot;...&quot;, modified: true }</span></span>
<span class="line"><span>  }</span></span>
<span class="line"><span>}</span></span></code></pre></div><h3 id="_6-git-context" tabindex="-1">6. Git Context <a class="header-anchor" href="#_6-git-context" aria-label="Permalink to &quot;6. Git Context&quot;">​</a></h3><p>Git history provides additional project context:</p><div class="language-"><button title="Copy Code" class="copy"></button><span class="lang"></span><pre class="shiki catppuccin-mocha vp-code" tabindex="0"><code><span class="line"><span>Git Context {</span></span>
<span class="line"><span>  current_branch: &quot;feature/email-validation&quot;</span></span>
<span class="line"><span>  base_branch: &quot;main&quot;</span></span>
<span class="line"><span>  </span></span>
<span class="line"><span>  recent_commits: [</span></span>
<span class="line"><span>    { sha: &quot;abc123&quot;, message: &quot;Add user creation endpoint&quot;, author: &quot;dev1&quot; },</span></span>
<span class="line"><span>    { sha: &quot;def456&quot;, message: &quot;Setup Spring Boot project&quot;, author: &quot;dev2&quot; },</span></span>
<span class="line"><span>    ...</span></span>
<span class="line"><span>  ]</span></span>
<span class="line"><span>  </span></span>
<span class="line"><span>  uncommitted_changes: [</span></span>
<span class="line"><span>    { file: &quot;UserService.java&quot;, status: &quot;modified&quot; },</span></span>
<span class="line"><span>    { file: &quot;Validator.java&quot;, status: &quot;added&quot; },</span></span>
<span class="line"><span>  ]</span></span>
<span class="line"><span>  </span></span>
<span class="line"><span>  // Relevant file history</span></span>
<span class="line"><span>  file_history: {</span></span>
<span class="line"><span>    &quot;UserService.java&quot;: [</span></span>
<span class="line"><span>      { sha: &quot;abc123&quot;, message: &quot;Add user creation endpoint&quot; },</span></span>
<span class="line"><span>      { sha: &quot;ghi789&quot;, message: &quot;Initial service scaffolding&quot; },</span></span>
<span class="line"><span>    ]</span></span>
<span class="line"><span>  }</span></span>
<span class="line"><span>}</span></span></code></pre></div><h2 id="multi-model-context-routing" tabindex="-1">Multi-Model Context Routing <a class="header-anchor" href="#multi-model-context-routing" aria-label="Permalink to &quot;Multi-Model Context Routing&quot;">​</a></h2><h3 id="the-context-routing-problem" tabindex="-1">The Context Routing Problem <a class="header-anchor" href="#the-context-routing-problem" aria-label="Permalink to &quot;The Context Routing Problem&quot;">​</a></h3><p>Different LLMs have different strengths, and different sub-tasks need different amounts and types of context. Junie&#39;s multi-model router must decide:</p><ol><li><strong>Which model</strong> to send each sub-task to</li><li><strong>How much context</strong> to include</li><li><strong>What type of context</strong> is most relevant</li><li><strong>How to format</strong> the context for each model</li></ol><h3 id="context-routing-strategy" tabindex="-1">Context Routing Strategy <a class="header-anchor" href="#context-routing-strategy" aria-label="Permalink to &quot;Context Routing Strategy&quot;">​</a></h3>`,44)),(a(),p(c,null,{default:n(()=>[l(t,{id:"mermaid-195",class:"mermaid",graph:"flowchart%20TD%0A%20%20%20%20IN%5B%22Input%3A%20Sub-task%20%2B%20available%20context%22%5D%20--%3E%20CL%7BStep%201%3A%20Classify%20sub-task%7D%0A%20%20%20%20CL%20--%3E%7Cplanning%7C%20M1%5B%22Full%20context%20%E2%86%92%20reasoning%20model%22%5D%0A%20%20%20%20CL%20--%3E%7Csimple%20edit%7C%20M2%5B%22Minimal%20context%20%E2%86%92%20fast%20model%22%5D%0A%20%20%20%20CL%20--%3E%7Cdebugging%7C%20M3%5B%22Error%20%2B%20code%20context%20%E2%86%92%20strong%20model%22%5D%0A%20%20%20%20CL%20--%3E%7Canalysis%7C%20M4%5B%22Broad%20context%20%E2%86%92%20reasoning%20model%22%5D%0A%20%20%20%20M1%20%26%20M2%20%26%20M3%20%26%20M4%20--%3E%20CTX%5B%22Step%202%3A%20Select%20relevant%20context%5Cn(always%3A%20AGENTS.md%20%2B%20task%20description%3B%5Cnfor%20edits%3A%20target%20%2B%20nearby%20files%3B%5Cnfor%20debugging%3A%20error%20output%20%2B%20code%3B%5Cnfor%20planning%3A%20project%20structure)%22%5D%0A%20%20%20%20CTX%20--%3E%20FMT%5B%22Step%203%3A%20Format%20for%20target%20model%5Cn(token%20budget%2C%20system%20prompt%2C%5Cnmodel-specific%20format)%22%5D%0A%20%20%20%20FMT%20--%3E%20EX%5B%22Step%204%3A%20Execute%20and%20collect%20response%22%5D%0A"})]),fallback:n(()=>[...s[0]||(s[0]=[o(" Loading... ",-1)])]),_:1})),s[3]||(s[3]=e(`<h3 id="context-budget-management" tabindex="-1">Context Budget Management <a class="header-anchor" href="#context-budget-management" aria-label="Permalink to &quot;Context Budget Management&quot;">​</a></h3><p>Each model has a context window limit, and Junie must manage the budget:</p><div class="language-"><button title="Copy Code" class="copy"></button><span class="lang"></span><pre class="shiki catppuccin-mocha vp-code" tabindex="0"><code><span class="line"><span>Context Budget Allocation (example for 128K context window):</span></span>
<span class="line"><span></span></span>
<span class="line"><span>  System prompt + AGENTS.md:        ~2K tokens</span></span>
<span class="line"><span>  Task description + plan:          ~1K tokens</span></span>
<span class="line"><span>  Conversation summary:             ~2K tokens</span></span>
<span class="line"><span>  Active file contents:            ~20K tokens</span></span>
<span class="line"><span>  Related file contents:           ~10K tokens</span></span>
<span class="line"><span>  Build/test context:               ~2K tokens</span></span>
<span class="line"><span>  Tool definitions:                 ~3K tokens</span></span>
<span class="line"><span>  ─────────────────────────────────────────</span></span>
<span class="line"><span>  Total used:                      ~40K tokens</span></span>
<span class="line"><span>  Remaining for generation:        ~88K tokens</span></span></code></pre></div><p>For models with smaller context windows, Junie must be more aggressive about context pruning — summarizing conversations, truncating file contents, and selecting only the most relevant files.</p><h3 id="cross-model-context-continuity" tabindex="-1">Cross-Model Context Continuity <a class="header-anchor" href="#cross-model-context-continuity" aria-label="Permalink to &quot;Cross-Model Context Continuity&quot;">​</a></h3><p>When switching between models during a task, Junie must maintain continuity:</p><div class="language-"><button title="Copy Code" class="copy"></button><span class="lang"></span><pre class="shiki catppuccin-mocha vp-code" tabindex="0"><code><span class="line"><span>Step 1: Planning (Claude Opus)</span></span>
<span class="line"><span>  Context: Full project structure, task description, AGENTS.md</span></span>
<span class="line"><span>  Output: Detailed plan with file modifications</span></span>
<span class="line"><span></span></span>
<span class="line"><span>Step 2: Implementation (Gemini Flash)</span></span>
<span class="line"><span>  Context: Plan from step 1, target file content, AGENTS.md</span></span>
<span class="line"><span>  Output: Code changes</span></span>
<span class="line"><span>  </span></span>
<span class="line"><span>  Challenge: Gemini Flash didn&#39;t see the full project context</span></span>
<span class="line"><span>  Solution: Include relevant excerpts from the plan + necessary files</span></span>
<span class="line"><span></span></span>
<span class="line"><span>Step 3: Debugging (Claude Sonnet)</span></span>
<span class="line"><span>  Context: Test failure output, modified files, original plan</span></span>
<span class="line"><span>  Output: Diagnosis and fix</span></span>
<span class="line"><span>  </span></span>
<span class="line"><span>  Challenge: Claude Sonnet didn&#39;t see the original planning context</span></span>
<span class="line"><span>  Solution: Include plan summary + specific failure details</span></span></code></pre></div><p>This cross-model continuity is one of the hardest challenges in multi-model architectures. Information loss at model boundaries can lead to inconsistent or suboptimal results.</p><h2 id="context-construction-pipeline" tabindex="-1">Context Construction Pipeline <a class="header-anchor" href="#context-construction-pipeline" aria-label="Permalink to &quot;Context Construction Pipeline&quot;">​</a></h2>`,9)),(a(),p(c,null,{default:n(()=>[l(t,{id:"mermaid-219",class:"mermaid",graph:"flowchart%20TD%0A%20%20%20%20UR%5BUser%20Request%5D%20--%3E%20PR%5B%22Parse%20Request%5Cn(extract%20intent%2C%20scope%2C%20constraints)%22%5D%0A%20%20%20%20PR%20--%3E%20LPM%5B%22Load%20Project%20Metadata%5Cn(build%20files%2C%20AGENTS.md%2C%20directory%20structure)%22%5D%0A%20%20%20%20LPM%20--%3E%20IRF%5B%22Identify%20Relevant%20Files%5Cn(use%20project%20structure%20%2B%20request)%22%5D%0A%20%20%20%20IRF%20--%3E%20LFC%5B%22Load%20File%20Contents%5Cn(read%20relevant%20files%2C%20summarize%20large%20ones)%22%5D%0A%20%20%20%20LFC%20--%3E%20AIC%5B%22Add%20IDE%20Context%20if%20available%5Cn(inspections%2C%20type%20info%2C%20test%20results)%22%5D%0A%20%20%20%20AIC%20--%3E%20LGC%5B%22Load%20Git%20Context%5Cn(recent%20commits%2C%20current%20branch%2C%5Cnuncommitted%20changes)%22%5D%0A%20%20%20%20LGC%20--%3E%20FMT%5B%22Format%20for%20Target%20Model%5Cn(assemble%20context%2C%20respect%20token%20budget)%22%5D%0A%20%20%20%20FMT%20--%3E%20LLM%5B%22Submit%20to%20LLM%22%5D%0A"})]),fallback:n(()=>[...s[1]||(s[1]=[o(" Loading... ",-1)])]),_:1})),s[4]||(s[4]=e(`<h2 id="ide-mode-vs-cli-mode-context-comparison" tabindex="-1">IDE Mode vs CLI Mode Context Comparison <a class="header-anchor" href="#ide-mode-vs-cli-mode-context-comparison" aria-label="Permalink to &quot;IDE Mode vs CLI Mode Context Comparison&quot;">​</a></h2><table tabindex="0"><thead><tr><th>Context Source</th><th>IDE Mode</th><th>CLI Mode</th></tr></thead><tbody><tr><td>Project structure</td><td>Full (from project model)</td><td>Partial (from directory scan)</td></tr><tr><td>Build file analysis</td><td>Deep (IDE parser)</td><td>Moderate (text analysis)</td></tr><tr><td>Type information</td><td>Complete (PSI)</td><td>None (LLM infers)</td></tr><tr><td>Inspections</td><td>Real-time</td><td>Not available</td></tr><tr><td>Test results</td><td>Structured (TestResult)</td><td>Parsed from terminal output</td></tr><tr><td>Import graph</td><td>Complete (reference resolution)</td><td>Not available</td></tr><tr><td>Compilation status</td><td>Real-time</td><td>Must run build command</td></tr><tr><td>AGENTS.md</td><td>Loaded at project open</td><td>Loaded at session start</td></tr><tr><td>Git context</td><td>Via IDE Git integration</td><td>Via git CLI commands</td></tr><tr><td>Conversation history</td><td>Same</td><td>Same</td></tr><tr><td>Framework knowledge</td><td>Plugin-enhanced</td><td>Heuristic</td></tr></tbody></table><h2 id="context-caching-and-refresh" tabindex="-1">Context Caching and Refresh <a class="header-anchor" href="#context-caching-and-refresh" aria-label="Permalink to &quot;Context Caching and Refresh&quot;">​</a></h2><h3 id="what-gets-cached" tabindex="-1">What Gets Cached <a class="header-anchor" href="#what-gets-cached" aria-label="Permalink to &quot;What Gets Cached&quot;">​</a></h3><div class="language-"><button title="Copy Code" class="copy"></button><span class="lang"></span><pre class="shiki catppuccin-mocha vp-code" tabindex="0"><code><span class="line"><span>Stable context (cached for session duration):</span></span>
<span class="line"><span>  - Project structure (refreshed on file system changes)</span></span>
<span class="line"><span>  - Build file contents (refreshed on modification)</span></span>
<span class="line"><span>  - AGENTS.md rules (loaded once, refreshed on change)</span></span>
<span class="line"><span>  - Framework detection results</span></span>
<span class="line"><span></span></span>
<span class="line"><span>Semi-stable context (cached with TTL):</span></span>
<span class="line"><span>  - File contents (invalidated on modification)</span></span>
<span class="line"><span>  - Git status (refreshed periodically)</span></span>
<span class="line"><span>  - IDE inspections (updated continuously in IDE mode)</span></span>
<span class="line"><span></span></span>
<span class="line"><span>Volatile context (never cached):</span></span>
<span class="line"><span>  - Test results (always re-run)</span></span>
<span class="line"><span>  - Build results (always re-build)</span></span>
<span class="line"><span>  - Shell command output</span></span></code></pre></div><h3 id="context-refresh-triggers" tabindex="-1">Context Refresh Triggers <a class="header-anchor" href="#context-refresh-triggers" aria-label="Permalink to &quot;Context Refresh Triggers&quot;">​</a></h3><div class="language-"><button title="Copy Code" class="copy"></button><span class="lang"></span><pre class="shiki catppuccin-mocha vp-code" tabindex="0"><code><span class="line"><span>File save → Refresh file content cache</span></span>
<span class="line"><span>             → Re-run inspections (IDE mode)</span></span>
<span class="line"><span>             → Update git status</span></span>
<span class="line"><span></span></span>
<span class="line"><span>Build completion → Update compilation status</span></span>
<span class="line"><span>                    → Refresh error/warning context</span></span>
<span class="line"><span></span></span>
<span class="line"><span>Test completion → Update test results context</span></span>
<span class="line"><span>                   → Mark failed tests for attention</span></span>
<span class="line"><span></span></span>
<span class="line"><span>Git operation → Refresh git context</span></span>
<span class="line"><span>                 → Update branch information</span></span>
<span class="line"><span></span></span>
<span class="line"><span>User message → Refresh conversation context</span></span>
<span class="line"><span>                → Re-evaluate relevant files</span></span></code></pre></div><h2 id="key-insights" tabindex="-1">Key Insights <a class="header-anchor" href="#key-insights" aria-label="Permalink to &quot;Key Insights&quot;">​</a></h2><ol><li><p><strong>Build files are the rosetta stone</strong>: The richest context comes from build system files. They tell the agent what language, framework, test system, and conventions the project uses — all without reading a single line of source code.</p></li><li><p><strong>IDE context is a massive advantage</strong>: The type information, inspections, and structured test results available in IDE mode represent context that CLI agents simply cannot replicate. This gap is fundamental, not just incremental.</p></li><li><p><strong>Multi-model context routing is hard</strong>: Maintaining coherent context across model boundaries is one of the toughest challenges in multi-model architectures. Each model switch is an opportunity for context loss.</p></li><li><p><strong>AGENTS.md standardization helps</strong>: By supporting project-level configuration files, Junie allows teams to encode their conventions and requirements in a format the agent can always access, regardless of IDE or CLI mode.</p></li><li><p><strong>Context budget management is critical</strong>: With multi-model routing, different models have different context windows. The agent must dynamically adjust what context to include based on both the task requirements and the target model&#39;s capabilities.</p></li></ol>`,9))])}const x=i(u,[["render",y]]);export{C as __pageData,x as default};
