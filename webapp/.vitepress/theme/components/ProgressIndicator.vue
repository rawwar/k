<script setup lang="ts">
import { computed, ref, onMounted } from 'vue'
import { useRoute } from 'vitepress'
import { useProgress } from '../composables/useProgress'

const props = defineProps<{
  mode: 'sidebar' | 'chapter'
}>()

const route = useRoute()
const ready = ref(false)

onMounted(() => {
  ready.value = true
})

const currentTrack = computed(() => {
  const path = route.path
  if (path.startsWith('/learn/project/')) return 'project'
  if (path.startsWith('/learn/linear/')) return 'linear'
  return null
})

const currentChapterSlug = computed(() => {
  if (!currentTrack.value) return null
  const match = route.path.match(/^\/learn\/(project|linear)\/([^/]+)/)
  return match ? match[2] : null
})

const isChapterIndex = computed(() => {
  if (!currentTrack.value || !currentChapterSlug.value) return false
  // Chapter index pages end with the chapter slug (no subchapter)
  const path = route.path.replace(/\/$/, '')
  const parts = path.split('/').filter(Boolean)
  return parts.length === 3 // e.g. ['learn', 'project', '01-hello-rust-cli']
})

// Sidebar mode: track-level progress
const trackProgress = computed(() => {
  if (!ready.value || !currentTrack.value) return null
  const { getTrackProgress } = useProgress()
  return getTrackProgress(currentTrack.value)
})

// Chapter mode: chapter-level progress (only on chapter index pages)
const chapterProgress = computed(() => {
  if (!ready.value || !currentTrack.value || !currentChapterSlug.value || !isChapterIndex.value) return null
  const { getChapterProgress } = useProgress()
  return getChapterProgress(currentTrack.value, currentChapterSlug.value)
})

const trackLabel = computed(() => {
  if (!currentTrack.value) return ''
  return currentTrack.value === 'project' ? 'Project Track' : 'Linear Track'
})

const progressPercent = computed(() => {
  if (!trackProgress.value || trackProgress.value.totalSections === 0) return 0
  return Math.round((trackProgress.value.readSections / trackProgress.value.totalSections) * 100)
})

const chapterPercent = computed(() => {
  if (!chapterProgress.value || chapterProgress.value.total === 0) return 0
  return Math.round((chapterProgress.value.read / chapterProgress.value.total) * 100)
})
</script>

<template>
  <!-- Sidebar mode: compact track progress -->
  <div v-if="mode === 'sidebar' && ready && trackProgress && trackProgress.totalSections > 0" class="sidebar-progress">
    <div class="sidebar-progress-header">
      <span class="sidebar-progress-label">{{ trackLabel }}</span>
      <span class="sidebar-progress-stats">{{ trackProgress.readSections }}/{{ trackProgress.totalSections }} sections</span>
    </div>
    <div class="progress-bar">
      <div class="progress-bar-fill" :style="{ width: progressPercent + '%' }" />
    </div>
    <div class="sidebar-progress-chapters">
      {{ trackProgress.completedChapters }}/{{ trackProgress.totalChapters }} chapters complete
    </div>
  </div>

  <!-- Chapter mode: segmented progress on chapter index pages -->
  <div v-if="mode === 'chapter' && ready && chapterProgress && chapterProgress.total > 0" class="chapter-progress">
    <div class="chapter-progress-header">
      <span class="chapter-progress-label">Progress</span>
      <span class="chapter-progress-stats">{{ chapterProgress.read }}/{{ chapterProgress.total }} sections</span>
    </div>
    <div class="progress-segments">
      <a
        v-for="sub in chapterProgress.subchapters"
        :key="sub.link"
        :href="sub.link"
        class="progress-segment"
        :class="{ visited: sub.visited }"
        :title="sub.title + (sub.visited ? ' (read)' : '')"
      />
    </div>
  </div>
</template>

<style scoped>
.sidebar-progress {
  padding: 12px 16px;
  margin: 0 0 8px;
  border-bottom: 1px solid var(--vp-c-divider);
}

.sidebar-progress-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}

.sidebar-progress-label {
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--vp-c-text-2);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.sidebar-progress-stats {
  font-size: 0.7rem;
  color: var(--vp-c-text-3);
}

.progress-bar {
  height: 4px;
  background: var(--vp-c-bg-mute);
  border-radius: 2px;
  overflow: hidden;
}

.progress-bar-fill {
  height: 100%;
  background: var(--vp-c-brand-1);
  border-radius: 2px;
  transition: width 0.3s ease;
}

.sidebar-progress-chapters {
  font-size: 0.7rem;
  color: var(--vp-c-text-3);
  margin-top: 6px;
}

/* Chapter mode */
.chapter-progress {
  padding: 16px 0;
  margin-bottom: 16px;
  border-bottom: 1px solid var(--vp-c-divider);
}

.chapter-progress-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 10px;
}

.chapter-progress-label {
  font-size: 0.8rem;
  font-weight: 600;
  color: var(--vp-c-text-2);
}

.chapter-progress-stats {
  font-size: 0.8rem;
  color: var(--vp-c-text-3);
}

.progress-segments {
  display: flex;
  gap: 3px;
}

.progress-segment {
  flex: 1;
  height: 8px;
  border-radius: 4px;
  background: var(--vp-c-bg-mute);
  transition: background-color 0.25s;
  cursor: pointer;
  text-decoration: none;
}

.progress-segment.visited {
  background: var(--vp-c-brand-1);
}

.progress-segment:hover {
  background: var(--vp-c-brand-2);
}
</style>
