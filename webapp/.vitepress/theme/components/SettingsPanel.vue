<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { useSettings, PROSE_FONTS, CODE_FONTS } from '../composables/useSettings'
import { darkThemes, lightThemes, type ThemeColors } from '../themes'

const { settings, setFontFamily, setCodeFontFamily, setFontSize, setDarkTheme, setLightTheme, resetSettings } = useSettings()

const isOpen = ref(false)
const panelRef = ref<HTMLElement | null>(null)
const triggerRef = ref<HTMLElement | null>(null)
const activeTab = ref<'themes' | 'fonts'>('themes')

const isDark = ref(false)

function checkDarkMode() {
  if (typeof document !== 'undefined') {
    isDark.value = document.documentElement.classList.contains('dark')
  }
}

function toggle() {
  isOpen.value = !isOpen.value
}

function onClickOutside(e: Event) {
  if (!isOpen.value) return
  const target = e.target as Node
  if (panelRef.value?.contains(target) || triggerRef.value?.contains(target)) return
  isOpen.value = false
}

function selectTheme(theme: ThemeColors) {
  if (theme.mode === 'dark') {
    setDarkTheme(theme.id)
  } else {
    setLightTheme(theme.id)
  }
}

const currentThemeId = computed(() =>
  isDark.value ? settings.value.darkTheme : settings.value.lightTheme
)

onMounted(() => {
  document.addEventListener('pointerdown', onClickOutside)
  checkDarkMode()
  const observer = new MutationObserver(checkDarkMode)
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
  onUnmounted(() => observer.disconnect())
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

      <!-- Tabs -->
      <div class="settings-tabs">
        <button
          class="tab-btn"
          :class="{ active: activeTab === 'themes' }"
          @click="activeTab = 'themes'"
        >Themes</button>
        <button
          class="tab-btn"
          :class="{ active: activeTab === 'fonts' }"
          @click="activeTab = 'fonts'"
        >Fonts</button>
      </div>

      <!-- Themes Tab -->
      <div v-if="activeTab === 'themes'" class="tab-content">
        <div class="theme-section">
          <label class="settings-label">{{ isDark ? 'Dark' : 'Light' }} Themes</label>
          <div class="theme-grid">
            <button
              v-for="theme in (isDark ? darkThemes : lightThemes)"
              :key="theme.id"
              class="theme-swatch"
              :class="{ selected: currentThemeId === theme.id }"
              :title="theme.name"
              @click="selectTheme(theme)"
            >
              <div class="swatch-colors">
                <div class="swatch-bg" :style="{ background: theme.colors.bg }" />
                <div class="swatch-accent" :style="{ background: theme.colors.brand1 }" />
                <div class="swatch-text" :style="{ color: theme.colors.text1, background: theme.colors.bg }">A</div>
              </div>
              <span class="swatch-name">{{ theme.name }}</span>
            </button>
          </div>
        </div>
        <p class="theme-hint">Toggle dark/light mode with the switch in the navbar to see the other set of themes.</p>
      </div>

      <!-- Fonts Tab -->
      <div v-if="activeTab === 'fonts'" class="tab-content">
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
  width: 340px;
  max-height: 80vh;
  overflow-y: auto;
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
  margin-bottom: 12px;
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

/* Tabs */
.settings-tabs {
  display: flex;
  gap: 4px;
  margin-bottom: 14px;
  padding: 3px;
  background: var(--vp-c-bg);
  border-radius: 8px;
}

.tab-btn {
  flex: 1;
  padding: 6px 12px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--vp-c-text-3);
  font-size: 0.8rem;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.tab-btn.active {
  background: var(--vp-c-bg-mute);
  color: var(--vp-c-text-1);
}

.tab-btn:hover:not(.active) {
  color: var(--vp-c-text-2);
}

.tab-content {
  animation: fadeIn 0.15s ease;
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

/* Theme grid */
.theme-section {
  margin-bottom: 12px;
}

.theme-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 8px;
}

.theme-swatch {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding: 8px 6px;
  border: 2px solid transparent;
  border-radius: 8px;
  background: var(--vp-c-bg);
  cursor: pointer;
  transition: all 0.2s;
}

.theme-swatch:hover {
  border-color: var(--vp-c-text-3);
}

.theme-swatch.selected {
  border-color: var(--vp-c-brand-1);
}

.swatch-colors {
  display: flex;
  width: 100%;
  height: 28px;
  border-radius: 4px;
  overflow: hidden;
  position: relative;
}

.swatch-bg {
  flex: 3;
}

.swatch-accent {
  flex: 1;
}

.swatch-text {
  position: absolute;
  left: 6px;
  top: 50%;
  transform: translateY(-50%);
  font-size: 0.75rem;
  font-weight: 700;
  line-height: 1;
}

.swatch-name {
  font-size: 0.7rem;
  color: var(--vp-c-text-2);
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
}

.theme-hint {
  font-size: 0.7rem;
  color: var(--vp-c-text-3);
  font-style: italic;
  margin-top: 8px;
  text-align: center;
}

/* Font controls */
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
