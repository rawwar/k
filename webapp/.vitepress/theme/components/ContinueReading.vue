<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useProgress } from '../composables/useProgress'

const ready = ref(false)

onMounted(() => {
  ready.value = true
})

const continueTarget = computed(() => {
  if (!ready.value) return null
  const { getMostRecentPage } = useProgress()
  return getMostRecentPage()
})

const displayTitle = computed(() => {
  if (!continueTarget.value) return ''
  const title = continueTarget.value.title
  return title.length > 24 ? title.slice(0, 22) + '...' : title
})
</script>

<template>
  <a
    v-if="ready && continueTarget"
    :href="continueTarget.path"
    class="continue-reading"
    :title="'Continue: ' + continueTarget.title"
  >
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
      <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
    </svg>
    <span class="continue-text">{{ displayTitle }}</span>
    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="9 18 15 12 9 6" />
    </svg>
  </a>
</template>

<style scoped>
.continue-reading {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: 8px;
  color: var(--vp-c-brand-1);
  font-size: 0.8rem;
  font-weight: 500;
  text-decoration: none;
  white-space: nowrap;
  transition: background-color 0.25s, color 0.25s;
}

.continue-reading:hover {
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-2);
}

.continue-text {
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
}

@media (max-width: 768px) {
  .continue-text {
    display: none;
  }
}
</style>
