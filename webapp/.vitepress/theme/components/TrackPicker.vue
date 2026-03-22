<template>
  <div class="track-picker">
    <div class="track-card">
      <a href="/project/" class="track-link">
        <div class="track-icon">&#x1F3D7;&#xFE0F;</div>
        <h2>Project-Based Track</h2>
        <p class="track-desc">Build a working CLI coding agent step by step. Each chapter adds a new capability to your agent.</p>
        <div class="track-meta">
          <span>15 chapters</span>
          <span>&middot;</span>
          <span>Hands-on</span>
        </div>
        <div class="track-cta">Start Building &rarr;</div>
      </a>
    </div>
    <div class="track-card">
      <a href="/linear/" class="track-link">
        <div class="track-icon">&#x1F4DA;</div>
        <h2>Linear Tutorial Track</h2>
        <p class="track-desc">Understand the theory and concepts first, then implement. Deep dives into each component of a coding agent.</p>
        <div class="track-meta">
          <span>18 chapters</span>
          <span>&middot;</span>
          <span>Concept-first</span>
        </div>
        <div class="track-cta">Start Learning &rarr;</div>
      </a>
    </div>
  </div>
</template>

<style scoped>
.track-picker {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
  gap: 1.5rem;
  margin-top: 2rem;
  padding: 0 1rem;
}

.track-card {
  border: 1px solid var(--vp-c-divider);
  border-radius: 12px;
  background: var(--vp-c-bg-soft);
  transition: all 0.3s ease;
  overflow: hidden;
}

.track-card:hover {
  border-color: var(--vp-c-brand-1);
  transform: translateY(-2px);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
}

.track-link {
  display: block;
  padding: 2rem;
  text-decoration: none;
  color: inherit;
}

.track-icon {
  font-size: 2.5rem;
  margin-bottom: 1rem;
}

.track-card h2 {
  margin: 0 0 0.75rem;
  font-size: 1.4rem;
  color: var(--vp-c-text-1);
  border: none;
  padding: 0;
}

.track-desc {
  color: var(--vp-c-text-2);
  line-height: 1.6;
  margin: 0 0 1rem;
}

.track-meta {
  display: flex;
  gap: 0.5rem;
  color: var(--vp-c-text-3);
  font-size: 0.875rem;
  margin-bottom: 1.25rem;
}

.track-cta {
  color: var(--vp-c-brand-1);
  font-weight: 600;
  font-size: 0.95rem;
}
</style>
