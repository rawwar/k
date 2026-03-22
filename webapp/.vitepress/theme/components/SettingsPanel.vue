<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useSettings, PROSE_FONTS, CODE_FONTS } from '../composables/useSettings'

const { settings, setFontFamily, setCodeFontFamily, setFontSize, resetSettings } = useSettings()

const isOpen = ref(false)
const panelRef = ref<HTMLElement | null>(null)
const triggerRef = ref<HTMLElement | null>(null)

function toggle() {
  isOpen.value = !isOpen.value
}

function onClickOutside(e: Event) {
  if (!isOpen.value) return
  const target = e.target as Node
  if (panelRef.value?.contains(target) || triggerRef.value?.contains(target)) return
  isOpen.value = false
}

onMounted(() => {
  document.addEventListener('pointerdown', onClickOutside)
})

onUnmounted(() => {
  document.removeEventListener('pointerdown', onClickOutside)
})
</script>

<template>
  <div class="settings-wrapper">
    <button
      ref="triggerRef"
      class="settings-trigger"
      :class="{ active: isOpen }"
      @click="toggle"
      aria-label="Display settings"
      title="Display settings"
    >
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="3" />
        <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
      </svg>
    </button>

    <div v-if="isOpen" ref="panelRef" class="settings-panel">
      <div class="settings-header">
        <span class="settings-title">Display Settings</span>
        <button class="settings-reset" @click="resetSettings">Reset</button>
      </div>

      <div class="settings-group">
        <label class="settings-label">Reading Font</label>
        <select
          class="settings-select"
          :value="settings.fontFamily"
          @change="setFontFamily(($event.target as HTMLSelectElement).value)"
        >
          <option v-for="font in PROSE_FONTS" :key="font.value" :value="font.value">
            {{ font.label }}
          </option>
        </select>
      </div>

      <div class="settings-group">
        <label class="settings-label">Code Font</label>
        <select
          class="settings-select"
          :value="settings.codeFontFamily"
          @change="setCodeFontFamily(($event.target as HTMLSelectElement).value)"
        >
          <option v-for="font in CODE_FONTS" :key="font.value" :value="font.value">
            {{ font.label }}
          </option>
        </select>
      </div>

      <div class="settings-group">
        <label class="settings-label">Font Size</label>
        <div class="font-size-controls">
          <button class="size-btn" @click="setFontSize(settings.fontSize - 1)" :disabled="settings.fontSize <= 14">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="5" y1="12" x2="19" y2="12" /></svg>
          </button>
          <span class="size-value">{{ settings.fontSize }}px</span>
          <button class="size-btn" @click="setFontSize(settings.fontSize + 1)" :disabled="settings.fontSize >= 22">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>
          </button>
        </div>
      </div>

      <div class="settings-preview">
        <span :style="{ fontFamily: settings.fontFamily, fontSize: settings.fontSize + 'px' }">The quick brown fox</span>
        <code :style="{ fontFamily: settings.codeFontFamily, fontSize: settings.fontSize + 'px' }">fn main() {}</code>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-wrapper {
  position: relative;
  display: flex;
  align-items: center;
}

.settings-trigger {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--vp-c-text-2);
  cursor: pointer;
  transition: color 0.25s, background-color 0.25s;
}

.settings-trigger:hover,
.settings-trigger.active {
  color: var(--vp-c-text-1);
  background: var(--vp-c-bg-mute);
}

.settings-panel {
  position: absolute;
  top: calc(100% + 8px);
  right: 0;
  width: 280px;
  padding: 16px;
  background: var(--vp-c-bg-soft);
  border: 1px solid var(--vp-c-divider);
  border-radius: 12px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.4);
  z-index: 100;
}

.settings-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.settings-title {
  font-weight: 600;
  font-size: 0.875rem;
  color: var(--vp-c-text-1);
}

.settings-reset {
  border: none;
  background: none;
  color: var(--vp-c-brand-1);
  font-size: 0.75rem;
  cursor: pointer;
  padding: 2px 6px;
  border-radius: 4px;
  transition: background-color 0.25s;
}

.settings-reset:hover {
  background: var(--vp-c-brand-soft);
}

.settings-group {
  margin-bottom: 14px;
}

.settings-label {
  display: block;
  font-size: 0.75rem;
  font-weight: 500;
  color: var(--vp-c-text-3);
  margin-bottom: 6px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.settings-select {
  width: 100%;
  padding: 8px 10px;
  background: var(--vp-c-bg);
  border: 1px solid var(--vp-c-divider);
  border-radius: 8px;
  color: var(--vp-c-text-1);
  font-size: 0.85rem;
  cursor: pointer;
  outline: none;
  transition: border-color 0.25s;
}

.settings-select:focus {
  border-color: var(--vp-c-brand-1);
}

.font-size-controls {
  display: flex;
  align-items: center;
  gap: 12px;
}

.size-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border: 1px solid var(--vp-c-divider);
  border-radius: 8px;
  background: var(--vp-c-bg);
  color: var(--vp-c-text-1);
  cursor: pointer;
  transition: border-color 0.25s, background-color 0.25s;
}

.size-btn:hover:not(:disabled) {
  border-color: var(--vp-c-brand-1);
  background: var(--vp-c-brand-soft);
}

.size-btn:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}

.size-value {
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--vp-c-text-1);
  min-width: 40px;
  text-align: center;
}

.settings-preview {
  margin-top: 14px;
  padding: 10px 12px;
  background: var(--vp-c-bg);
  border-radius: 8px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.settings-preview span {
  color: var(--vp-c-text-2);
}

.settings-preview code {
  color: var(--vp-c-brand-1);
  font-size: 0.85em;
}
</style>
