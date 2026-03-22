import { ref, watch } from 'vue'

export interface Settings {
  fontFamily: string
  codeFontFamily: string
  fontSize: number
}

export const PROSE_FONTS = [
  { label: 'System Default', value: 'system-ui, -apple-system, BlinkMacSystemFont, sans-serif' },
  { label: 'Inter', value: "'Inter', sans-serif" },
  { label: 'JetBrains Mono', value: "'JetBrains Mono', monospace" },
  { label: 'Fira Code', value: "'Fira Code', monospace" },
  { label: 'Source Code Pro', value: "'Source Code Pro', monospace" },
  { label: 'IBM Plex Mono', value: "'IBM Plex Mono', monospace" },
]

export const CODE_FONTS = [
  { label: 'JetBrains Mono', value: "'JetBrains Mono', monospace" },
  { label: 'Fira Code', value: "'Fira Code', monospace" },
  { label: 'Source Code Pro', value: "'Source Code Pro', monospace" },
  { label: 'IBM Plex Mono', value: "'IBM Plex Mono', monospace" },
]

const STORAGE_KEY = 'k-learn:settings'

const DEFAULT_SETTINGS: Settings = {
  fontFamily: 'system-ui, -apple-system, BlinkMacSystemFont, sans-serif',
  codeFontFamily: "'JetBrains Mono', monospace",
  fontSize: 16,
}

// Module-level singleton state
const settings = ref<Settings>({ ...DEFAULT_SETTINGS })
let initialized = false

function loadFromStorage(): Settings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) {
      const parsed = JSON.parse(raw)
      return { ...DEFAULT_SETTINGS, ...parsed }
    }
  } catch {}
  return { ...DEFAULT_SETTINGS }
}

function saveToStorage(s: Settings) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(s))
  } catch {}
}

function applyToDOM(s: Settings) {
  if (typeof document === 'undefined') return
  const root = document.documentElement
  root.style.setProperty('--vp-font-family-base', s.fontFamily)
  root.style.setProperty('--vp-font-family-mono', s.codeFontFamily)
  root.style.setProperty('--k-font-size', `${s.fontSize}px`)
}

export function useSettings() {
  // Hydrate from localStorage once on client
  if (!initialized && typeof window !== 'undefined') {
    settings.value = loadFromStorage()
    applyToDOM(settings.value)
    initialized = true

    watch(settings, (val) => {
      saveToStorage(val)
      applyToDOM(val)
    }, { deep: true })
  }

  function setFontFamily(family: string) {
    settings.value = { ...settings.value, fontFamily: family }
  }

  function setCodeFontFamily(family: string) {
    settings.value = { ...settings.value, codeFontFamily: family }
  }

  function setFontSize(size: number) {
    settings.value = { ...settings.value, fontSize: Math.min(22, Math.max(14, size)) }
  }

  function resetSettings() {
    settings.value = { ...DEFAULT_SETTINGS }
  }

  function applySettings() {
    applyToDOM(settings.value)
  }

  return {
    settings,
    setFontFamily,
    setCodeFontFamily,
    setFontSize,
    resetSettings,
    applySettings,
    PROSE_FONTS,
    CODE_FONTS,
  }
}
