import{_ as a,o as n,c as l,a2 as p}from"./chunks/framework.B72_uj9U.js";const u=JSON.parse('{"title":"Release Automation","description":"Building a CI/CD release pipeline with GitHub Actions that builds, tests, signs, and publishes binaries and creates GitHub Releases on tag push.","frontmatter":{"title":"Release Automation","description":"Building a CI/CD release pipeline with GitHub Actions that builds, tests, signs, and publishes binaries and creates GitHub Releases on tag push."},"headers":[],"relativePath":"learn/project/15-production-polish/08-release-automation.md","filePath":"learn/project/15-production-polish/08-release-automation.md"}'),e={name:"learn/project/15-production-polish/08-release-automation.md"};function o(t,s,c,r,i,y){return n(),l("div",null,[...s[0]||(s[0]=[p(`<h1 id="release-automation" tabindex="-1">Release Automation <a class="header-anchor" href="#release-automation" aria-label="Permalink to &quot;Release Automation&quot;">​</a></h1><blockquote><p><strong>What you&#39;ll learn:</strong></p><ul><li>How to design a GitHub Actions workflow that triggers on version tags and produces release artifacts</li><li>How to build and upload platform-specific binaries and generate checksums for verification</li><li>Techniques for creating GitHub Releases with auto-generated release notes from commit history</li></ul></blockquote><p>Manual releases are error-prone. You forget to build for one platform, or you upload the wrong binary, or you forget to update the Homebrew formula. A good release pipeline handles all of this automatically: push a Git tag, and everything else happens without human intervention. GitHub Actions is the natural choice for projects hosted on GitHub, and in this subchapter you will build a complete release workflow.</p><h2 id="the-release-workflow-overview" tabindex="-1">The Release Workflow Overview <a class="header-anchor" href="#the-release-workflow-overview" aria-label="Permalink to &quot;The Release Workflow Overview&quot;">​</a></h2><p>Here is the flow triggered by pushing a version tag:</p><ol><li><strong>Tag push</strong> (<code>v0.1.0</code>) triggers the workflow.</li><li><strong>Test</strong> -- run the full test suite to catch last-minute issues.</li><li><strong>Build</strong> -- cross-compile for all target platforms.</li><li><strong>Package</strong> -- create tarballs and generate SHA256 checksums.</li><li><strong>Release</strong> -- create a GitHub Release with binaries, checksums, and release notes.</li><li><strong>Notify</strong> -- update the Homebrew formula and optionally publish to crates.io.</li></ol><h2 id="the-complete-github-actions-workflow" tabindex="-1">The Complete GitHub Actions Workflow <a class="header-anchor" href="#the-complete-github-actions-workflow" aria-label="Permalink to &quot;The Complete GitHub Actions Workflow&quot;">​</a></h2><p>Create <code>.github/workflows/release.yml</code>:</p><div class="language-yaml"><button title="Copy Code" class="copy"></button><span class="lang">yaml</span><pre class="shiki catppuccin-mocha vp-code" tabindex="0"><code><span class="line"><span style="color:#89B4FA;">name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Release</span></span>
<span class="line"></span>
<span class="line"><span style="color:#FAB387;">on</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">  push</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">    tags</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#A6E3A1;"> &#39;v[0-9]+.*&#39;</span></span>
<span class="line"></span>
<span class="line"><span style="color:#89B4FA;">permissions</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">  contents</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> write</span></span>
<span class="line"></span>
<span class="line"><span style="color:#89B4FA;">env</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">  CARGO_TERM_COLOR</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> always</span></span>
<span class="line"><span style="color:#89B4FA;">  BINARY_NAME</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> agent</span></span>
<span class="line"></span>
<span class="line"><span style="color:#89B4FA;">jobs</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#6C7086;font-style:italic;">  # First, run tests to make sure everything passes</span></span>
<span class="line"><span style="color:#89B4FA;">  test</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">    name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Test</span></span>
<span class="line"><span style="color:#89B4FA;">    runs-on</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> ubuntu-latest</span></span>
<span class="line"><span style="color:#89B4FA;">    steps</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> actions/checkout@v4</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> dtolnay/rust-toolchain@stable</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Swatinem/rust-cache@v2</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> run</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> cargo test --all-features</span></span>
<span class="line"></span>
<span class="line"><span style="color:#6C7086;font-style:italic;">  # Build for each target platform</span></span>
<span class="line"><span style="color:#89B4FA;">  build</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">    name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Build \${{ matrix.target }}</span></span>
<span class="line"><span style="color:#89B4FA;">    needs</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> test</span></span>
<span class="line"><span style="color:#89B4FA;">    runs-on</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> \${{ matrix.os }}</span></span>
<span class="line"><span style="color:#89B4FA;">    strategy</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">      fail-fast</span><span style="color:#94E2D5;">:</span><span style="color:#FAB387;"> false</span></span>
<span class="line"><span style="color:#89B4FA;">      matrix</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">        include</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#9399B2;">          -</span><span style="color:#89B4FA;"> target</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> x86_64-unknown-linux-musl</span></span>
<span class="line"><span style="color:#89B4FA;">            os</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> ubuntu-latest</span></span>
<span class="line"><span style="color:#89B4FA;">            use_cross</span><span style="color:#94E2D5;">:</span><span style="color:#FAB387;"> true</span></span>
<span class="line"><span style="color:#9399B2;">          -</span><span style="color:#89B4FA;"> target</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> aarch64-unknown-linux-musl</span></span>
<span class="line"><span style="color:#89B4FA;">            os</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> ubuntu-latest</span></span>
<span class="line"><span style="color:#89B4FA;">            use_cross</span><span style="color:#94E2D5;">:</span><span style="color:#FAB387;"> true</span></span>
<span class="line"><span style="color:#9399B2;">          -</span><span style="color:#89B4FA;"> target</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> x86_64-apple-darwin</span></span>
<span class="line"><span style="color:#89B4FA;">            os</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> macos-latest</span></span>
<span class="line"><span style="color:#89B4FA;">            use_cross</span><span style="color:#94E2D5;">:</span><span style="color:#FAB387;"> false</span></span>
<span class="line"><span style="color:#9399B2;">          -</span><span style="color:#89B4FA;"> target</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> aarch64-apple-darwin</span></span>
<span class="line"><span style="color:#89B4FA;">            os</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> macos-latest</span></span>
<span class="line"><span style="color:#89B4FA;">            use_cross</span><span style="color:#94E2D5;">:</span><span style="color:#FAB387;"> false</span></span>
<span class="line"><span style="color:#9399B2;">          -</span><span style="color:#89B4FA;"> target</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> x86_64-pc-windows-msvc</span></span>
<span class="line"><span style="color:#89B4FA;">            os</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> windows-latest</span></span>
<span class="line"><span style="color:#89B4FA;">            use_cross</span><span style="color:#94E2D5;">:</span><span style="color:#FAB387;"> false</span></span>
<span class="line"></span>
<span class="line"><span style="color:#89B4FA;">    steps</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> actions/checkout@v4</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> dtolnay/rust-toolchain@stable</span></span>
<span class="line"><span style="color:#89B4FA;">        with</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">          targets</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> \${{ matrix.target }}</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Swatinem/rust-cache@v2</span></span>
<span class="line"><span style="color:#89B4FA;">        with</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">          key</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> \${{ matrix.target }}</span></span>
<span class="line"></span>
<span class="line"><span style="color:#6C7086;font-style:italic;">      # Install cross for Linux builds</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Install cross</span></span>
<span class="line"><span style="color:#89B4FA;">        if</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> matrix.use_cross</span></span>
<span class="line"><span style="color:#89B4FA;">        run</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> cargo install cross --git https://github.com/cross-rs/cross</span></span>
<span class="line"></span>
<span class="line"><span style="color:#6C7086;font-style:italic;">      # Build the binary</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Build</span></span>
<span class="line"><span style="color:#89B4FA;">        run</span><span style="color:#94E2D5;">:</span><span style="color:#CBA6F7;"> |</span></span>
<span class="line"><span style="color:#A6E3A1;">          if [ &quot;\${{ matrix.use_cross }}&quot; = &quot;true&quot; ]; then</span></span>
<span class="line"><span style="color:#A6E3A1;">            cross build --release --target \${{ matrix.target }}</span></span>
<span class="line"><span style="color:#A6E3A1;">          else</span></span>
<span class="line"><span style="color:#A6E3A1;">            cargo build --release --target \${{ matrix.target }}</span></span>
<span class="line"><span style="color:#A6E3A1;">          fi</span></span>
<span class="line"><span style="color:#89B4FA;">        shell</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> bash</span></span>
<span class="line"></span>
<span class="line"><span style="color:#6C7086;font-style:italic;">      # Package the binary</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Package (Unix)</span></span>
<span class="line"><span style="color:#89B4FA;">        if</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> runner.os != &#39;Windows&#39;</span></span>
<span class="line"><span style="color:#89B4FA;">        run</span><span style="color:#94E2D5;">:</span><span style="color:#CBA6F7;"> |</span></span>
<span class="line"><span style="color:#A6E3A1;">          cd target/\${{ matrix.target }}/release</span></span>
<span class="line"><span style="color:#A6E3A1;">          tar czf ../../../\${{ env.BINARY_NAME }}-\${{ matrix.target }}.tar.gz \${{ env.BINARY_NAME }}</span></span>
<span class="line"><span style="color:#89B4FA;">        shell</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> bash</span></span>
<span class="line"></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Package (Windows)</span></span>
<span class="line"><span style="color:#89B4FA;">        if</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> runner.os == &#39;Windows&#39;</span></span>
<span class="line"><span style="color:#89B4FA;">        run</span><span style="color:#94E2D5;">:</span><span style="color:#CBA6F7;"> |</span></span>
<span class="line"><span style="color:#A6E3A1;">          cd target/\${{ matrix.target }}/release</span></span>
<span class="line"><span style="color:#A6E3A1;">          7z a ../../../\${{ env.BINARY_NAME }}-\${{ matrix.target }}.zip \${{ env.BINARY_NAME }}.exe</span></span>
<span class="line"><span style="color:#89B4FA;">        shell</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> bash</span></span>
<span class="line"></span>
<span class="line"><span style="color:#6C7086;font-style:italic;">      # Upload the artifact for the release job</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Upload artifact</span></span>
<span class="line"><span style="color:#89B4FA;">        uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> actions/upload-artifact@v4</span></span>
<span class="line"><span style="color:#89B4FA;">        with</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">          name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> \${{ env.BINARY_NAME }}-\${{ matrix.target }}</span></span>
<span class="line"><span style="color:#89B4FA;">          path</span><span style="color:#94E2D5;">:</span><span style="color:#CBA6F7;"> |</span></span>
<span class="line"><span style="color:#A6E3A1;">            \${{ env.BINARY_NAME }}-\${{ matrix.target }}.tar.gz</span></span>
<span class="line"><span style="color:#A6E3A1;">            \${{ env.BINARY_NAME }}-\${{ matrix.target }}.zip</span></span>
<span class="line"><span style="color:#89B4FA;">          if-no-files-found</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> error</span></span>
<span class="line"></span>
<span class="line"><span style="color:#6C7086;font-style:italic;">  # Create the GitHub Release</span></span>
<span class="line"><span style="color:#89B4FA;">  release</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">    name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Create Release</span></span>
<span class="line"><span style="color:#89B4FA;">    needs</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> build</span></span>
<span class="line"><span style="color:#89B4FA;">    runs-on</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> ubuntu-latest</span></span>
<span class="line"><span style="color:#89B4FA;">    steps</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> actions/checkout@v4</span></span>
<span class="line"><span style="color:#89B4FA;">        with</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">          fetch-depth</span><span style="color:#94E2D5;">:</span><span style="color:#FAB387;"> 0</span><span style="color:#6C7086;font-style:italic;">  # Full history for release notes</span></span>
<span class="line"></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Download all artifacts</span></span>
<span class="line"><span style="color:#89B4FA;">        uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> actions/download-artifact@v4</span></span>
<span class="line"><span style="color:#89B4FA;">        with</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">          path</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> artifacts</span></span>
<span class="line"></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Collect release files</span></span>
<span class="line"><span style="color:#89B4FA;">        run</span><span style="color:#94E2D5;">:</span><span style="color:#CBA6F7;"> |</span></span>
<span class="line"><span style="color:#A6E3A1;">          mkdir release-files</span></span>
<span class="line"><span style="color:#A6E3A1;">          find artifacts -type f \\( -name &quot;*.tar.gz&quot; -o -name &quot;*.zip&quot; \\) \\</span></span>
<span class="line"><span style="color:#A6E3A1;">            -exec mv {} release-files/ \\;</span></span>
<span class="line"><span style="color:#A6E3A1;">          ls -la release-files/</span></span>
<span class="line"></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Generate checksums</span></span>
<span class="line"><span style="color:#89B4FA;">        run</span><span style="color:#94E2D5;">:</span><span style="color:#CBA6F7;"> |</span></span>
<span class="line"><span style="color:#A6E3A1;">          cd release-files</span></span>
<span class="line"><span style="color:#A6E3A1;">          shasum -a 256 * &gt; checksums-sha256.txt</span></span>
<span class="line"><span style="color:#A6E3A1;">          cat checksums-sha256.txt</span></span>
<span class="line"></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Get version from tag</span></span>
<span class="line"><span style="color:#89B4FA;">        id</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> version</span></span>
<span class="line"><span style="color:#89B4FA;">        run</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> echo &quot;VERSION=\${GITHUB_REF#refs/tags/v}&quot; &gt;&gt; $GITHUB_OUTPUT</span></span>
<span class="line"></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Generate release notes</span></span>
<span class="line"><span style="color:#89B4FA;">        id</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> notes</span></span>
<span class="line"><span style="color:#89B4FA;">        run</span><span style="color:#94E2D5;">:</span><span style="color:#CBA6F7;"> |</span></span>
<span class="line"><span style="color:#A6E3A1;">          # Get commits since the last tag</span></span>
<span class="line"><span style="color:#A6E3A1;">          PREV_TAG=$(git describe --tags --abbrev=0 HEAD^ 2&gt;/dev/null || echo &quot;&quot;)</span></span>
<span class="line"><span style="color:#A6E3A1;">          if [ -n &quot;$PREV_TAG&quot; ]; then</span></span>
<span class="line"><span style="color:#A6E3A1;">            NOTES=$(git log --pretty=format:&quot;- %s (%h)&quot; &quot;$PREV_TAG&quot;..HEAD)</span></span>
<span class="line"><span style="color:#A6E3A1;">          else</span></span>
<span class="line"><span style="color:#A6E3A1;">            NOTES=$(git log --pretty=format:&quot;- %s (%h)&quot; HEAD~10..HEAD)</span></span>
<span class="line"><span style="color:#A6E3A1;">          fi</span></span>
<span class="line"><span style="color:#A6E3A1;">          # Write to a file to handle multiline content</span></span>
<span class="line"><span style="color:#A6E3A1;">          echo &quot;$NOTES&quot; &gt; release-notes.md</span></span>
<span class="line"></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Create GitHub Release</span></span>
<span class="line"><span style="color:#89B4FA;">        uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> softprops/action-gh-release@v2</span></span>
<span class="line"><span style="color:#89B4FA;">        with</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">          name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> v\${{ steps.version.outputs.VERSION }}</span></span>
<span class="line"><span style="color:#89B4FA;">          body_path</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> release-notes.md</span></span>
<span class="line"><span style="color:#89B4FA;">          files</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> release-files/*</span></span>
<span class="line"><span style="color:#89B4FA;">          draft</span><span style="color:#94E2D5;">:</span><span style="color:#FAB387;"> false</span></span>
<span class="line"><span style="color:#89B4FA;">          prerelease</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> \${{ contains(github.ref, &#39;-rc&#39;) || contains(github.ref, &#39;-beta&#39;) }}</span></span></code></pre></div><div class="custom-block python"><p class="custom-block-title">Coming from Python</p><p>Python release pipelines typically involve <code>twine upload</code> to PyPI and building wheels for different platforms. Rust&#39;s release pipeline is more involved because you are distributing native binaries -- there is no runtime to handle platform differences for you. The upside is that your users get a zero-dependency binary that starts instantly, unlike Python tools that may require installing Python itself and managing virtual environments.</p></div><h2 id="adding-homebrew-formula-updates" tabindex="-1">Adding Homebrew Formula Updates <a class="header-anchor" href="#adding-homebrew-formula-updates" aria-label="Permalink to &quot;Adding Homebrew Formula Updates&quot;">​</a></h2><p>Extend the release workflow with a job that updates your Homebrew tap:</p><div class="language-yaml"><button title="Copy Code" class="copy"></button><span class="lang">yaml</span><pre class="shiki catppuccin-mocha vp-code" tabindex="0"><code><span class="line"><span style="color:#6C7086;font-style:italic;">  # Update Homebrew formula</span></span>
<span class="line"><span style="color:#89B4FA;">  homebrew</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">    name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Update Homebrew</span></span>
<span class="line"><span style="color:#89B4FA;">    needs</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> release</span></span>
<span class="line"><span style="color:#89B4FA;">    runs-on</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> ubuntu-latest</span></span>
<span class="line"><span style="color:#89B4FA;">    steps</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Download checksums</span></span>
<span class="line"><span style="color:#89B4FA;">        uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> actions/download-artifact@v4</span></span>
<span class="line"><span style="color:#89B4FA;">        with</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">          path</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> artifacts</span></span>
<span class="line"></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Compute checksums</span></span>
<span class="line"><span style="color:#89B4FA;">        id</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> checksums</span></span>
<span class="line"><span style="color:#89B4FA;">        run</span><span style="color:#94E2D5;">:</span><span style="color:#CBA6F7;"> |</span></span>
<span class="line"><span style="color:#A6E3A1;">          cd artifacts</span></span>
<span class="line"><span style="color:#A6E3A1;">          # Find the checksum file from the release</span></span>
<span class="line"><span style="color:#A6E3A1;">          for target in \\</span></span>
<span class="line"><span style="color:#A6E3A1;">            &quot;aarch64-apple-darwin&quot; \\</span></span>
<span class="line"><span style="color:#A6E3A1;">            &quot;x86_64-apple-darwin&quot; \\</span></span>
<span class="line"><span style="color:#A6E3A1;">            &quot;aarch64-unknown-linux-musl&quot; \\</span></span>
<span class="line"><span style="color:#A6E3A1;">            &quot;x86_64-unknown-linux-musl&quot;; do</span></span>
<span class="line"><span style="color:#A6E3A1;">            file=&quot;agent-\${target}/agent-\${target}.tar.gz&quot;</span></span>
<span class="line"><span style="color:#A6E3A1;">            if [ -f &quot;$file&quot; ]; then</span></span>
<span class="line"><span style="color:#A6E3A1;">              sha=$(shasum -a 256 &quot;$file&quot; | awk &#39;{print $1}&#39;)</span></span>
<span class="line"><span style="color:#A6E3A1;">              echo &quot;\${target}_SHA256=\${sha}&quot; &gt;&gt; $GITHUB_OUTPUT</span></span>
<span class="line"><span style="color:#A6E3A1;">            fi</span></span>
<span class="line"><span style="color:#A6E3A1;">          done</span></span>
<span class="line"></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Get version from tag</span></span>
<span class="line"><span style="color:#89B4FA;">        id</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> version</span></span>
<span class="line"><span style="color:#89B4FA;">        run</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> echo &quot;VERSION=\${GITHUB_REF#refs/tags/v}&quot; &gt;&gt; $GITHUB_OUTPUT</span></span>
<span class="line"></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Update Homebrew formula</span></span>
<span class="line"><span style="color:#89B4FA;">        uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> mislav/bump-homebrew-formula-action@v3</span></span>
<span class="line"><span style="color:#89B4FA;">        with</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">          formula-name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> agent</span></span>
<span class="line"><span style="color:#89B4FA;">          homebrew-tap</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> yourusername/homebrew-agent</span></span>
<span class="line"><span style="color:#89B4FA;">          tag-name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> \${{ github.ref_name }}</span></span>
<span class="line"><span style="color:#89B4FA;">          download-url</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> https://github.com/\${{ github.repository }}/releases/download/\${{ github.ref_name }}/agent-x86_64-apple-darwin.tar.gz</span></span>
<span class="line"><span style="color:#89B4FA;">        env</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">          COMMITTER_TOKEN</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> \${{ secrets.HOMEBREW_TAP_TOKEN }}</span></span></code></pre></div><p>The <code>HOMEBREW_TAP_TOKEN</code> secret needs to be a GitHub personal access token with <code>repo</code> scope that can push to your <code>homebrew-agent</code> repository.</p><h2 id="continuous-integration-for-every-push" tabindex="-1">Continuous Integration for Every Push <a class="header-anchor" href="#continuous-integration-for-every-push" aria-label="Permalink to &quot;Continuous Integration for Every Push&quot;">​</a></h2><p>Beyond release automation, you want a CI workflow that runs on every push and pull request:</p><div class="language-yaml"><button title="Copy Code" class="copy"></button><span class="lang">yaml</span><pre class="shiki catppuccin-mocha vp-code" tabindex="0"><code><span class="line"><span style="color:#6C7086;font-style:italic;"># .github/workflows/ci.yml</span></span>
<span class="line"><span style="color:#89B4FA;">name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> CI</span></span>
<span class="line"></span>
<span class="line"><span style="color:#FAB387;">on</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">  push</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">    branches</span><span style="color:#94E2D5;">:</span><span style="color:#9399B2;"> [</span><span style="color:#A6E3A1;">main</span><span style="color:#9399B2;">]</span></span>
<span class="line"><span style="color:#89B4FA;">  pull_request</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">    branches</span><span style="color:#94E2D5;">:</span><span style="color:#9399B2;"> [</span><span style="color:#A6E3A1;">main</span><span style="color:#9399B2;">]</span></span>
<span class="line"></span>
<span class="line"><span style="color:#89B4FA;">env</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">  CARGO_TERM_COLOR</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> always</span></span>
<span class="line"></span>
<span class="line"><span style="color:#89B4FA;">jobs</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">  check</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">    name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Check</span></span>
<span class="line"><span style="color:#89B4FA;">    runs-on</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> ubuntu-latest</span></span>
<span class="line"><span style="color:#89B4FA;">    steps</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> actions/checkout@v4</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> dtolnay/rust-toolchain@stable</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Swatinem/rust-cache@v2</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> run</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> cargo check --all-features</span></span>
<span class="line"></span>
<span class="line"><span style="color:#89B4FA;">  test</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">    name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Test</span></span>
<span class="line"><span style="color:#89B4FA;">    runs-on</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> \${{ matrix.os }}</span></span>
<span class="line"><span style="color:#89B4FA;">    strategy</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">      matrix</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">        os</span><span style="color:#94E2D5;">:</span><span style="color:#9399B2;"> [</span><span style="color:#A6E3A1;">ubuntu-latest</span><span style="color:#9399B2;">,</span><span style="color:#A6E3A1;"> macos-latest</span><span style="color:#9399B2;">,</span><span style="color:#A6E3A1;"> windows-latest</span><span style="color:#9399B2;">]</span></span>
<span class="line"><span style="color:#89B4FA;">    steps</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> actions/checkout@v4</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> dtolnay/rust-toolchain@stable</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Swatinem/rust-cache@v2</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> run</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> cargo test --all-features</span></span>
<span class="line"></span>
<span class="line"><span style="color:#89B4FA;">  fmt</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">    name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Format</span></span>
<span class="line"><span style="color:#89B4FA;">    runs-on</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> ubuntu-latest</span></span>
<span class="line"><span style="color:#89B4FA;">    steps</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> actions/checkout@v4</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> dtolnay/rust-toolchain@stable</span></span>
<span class="line"><span style="color:#89B4FA;">        with</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">          components</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> rustfmt</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> run</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> cargo fmt --all -- --check</span></span>
<span class="line"></span>
<span class="line"><span style="color:#89B4FA;">  clippy</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">    name</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Clippy</span></span>
<span class="line"><span style="color:#89B4FA;">    runs-on</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> ubuntu-latest</span></span>
<span class="line"><span style="color:#89B4FA;">    steps</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> actions/checkout@v4</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> dtolnay/rust-toolchain@stable</span></span>
<span class="line"><span style="color:#89B4FA;">        with</span><span style="color:#94E2D5;">:</span></span>
<span class="line"><span style="color:#89B4FA;">          components</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> clippy</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> uses</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> Swatinem/rust-cache@v2</span></span>
<span class="line"><span style="color:#9399B2;">      -</span><span style="color:#89B4FA;"> run</span><span style="color:#94E2D5;">:</span><span style="color:#A6E3A1;"> cargo clippy --all-features -- -D warnings</span></span></code></pre></div><p>This workflow catches formatting issues, linting violations, test failures, and compilation errors before code reaches the main branch.</p><h2 id="triggering-a-release" tabindex="-1">Triggering a Release <a class="header-anchor" href="#triggering-a-release" aria-label="Permalink to &quot;Triggering a Release&quot;">​</a></h2><p>With the pipeline in place, releasing a new version is a single command:</p><div class="language-bash"><button title="Copy Code" class="copy"></button><span class="lang">bash</span><pre class="shiki catppuccin-mocha vp-code" tabindex="0"><code><span class="line"><span style="color:#6C7086;font-style:italic;"># Tag the release</span></span>
<span class="line"><span style="color:#89B4FA;font-style:italic;">git</span><span style="color:#A6E3A1;"> tag</span><span style="color:#A6E3A1;"> v0.1.0</span></span>
<span class="line"><span style="color:#89B4FA;font-style:italic;">git</span><span style="color:#A6E3A1;"> push</span><span style="color:#A6E3A1;"> origin</span><span style="color:#A6E3A1;"> v0.1.0</span></span></code></pre></div><p>The workflow automatically:</p><ol><li>Runs the test suite.</li><li>Builds binaries for Linux (x86_64, ARM64), macOS (Intel, Apple Silicon), and Windows.</li><li>Creates tarballs with checksums.</li><li>Publishes a GitHub Release with auto-generated release notes.</li><li>Updates the Homebrew formula.</li></ol><p>If any step fails, the release is not published, and you get a notification to investigate.</p><div class="custom-block wild"><p class="custom-block-title">In the Wild</p><p>Claude Code&#39;s release process is automated through Anthropic&#39;s internal CI systems. OpenCode uses a similar GitHub Actions approach to build multi-platform binaries on every tagged release. The <code>cargo-dist</code> tool is an emerging option that automates much of this setup -- it generates GitHub Actions workflows and Homebrew formulas from your <code>Cargo.toml</code> metadata, reducing the boilerplate to a single <code>cargo dist init</code> command.</p></div><h2 id="key-takeaways" tabindex="-1">Key Takeaways <a class="header-anchor" href="#key-takeaways" aria-label="Permalink to &quot;Key Takeaways&quot;">​</a></h2><ul><li>Trigger release workflows on version tag pushes (<code>v*</code>) rather than manual triggers -- this makes the release process a single <code>git tag</code> + <code>git push</code> command.</li><li>Use a build matrix to compile for all target platforms in parallel, combining <code>cross</code> for Linux targets and native builds for macOS and Windows.</li><li>Generate SHA256 checksums for every release artifact and include them in the GitHub Release, giving users a way to verify download integrity.</li><li>Maintain a separate CI workflow for every push and pull request that checks formatting, linting, and runs tests on multiple operating systems.</li><li>Automate Homebrew formula updates as part of the release pipeline so that <code>brew upgrade</code> picks up new versions without manual intervention.</li></ul>`,27)])])}const E=a(e,[["render",o]]);export{u as __pageData,E as default};
