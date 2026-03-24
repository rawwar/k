import { ref, watch } from 'vue'
import { getThemeById, DEFAULT_DARK_THEME, DEFAULT_LIGHT_THEME, type ThemeColors } from '../themes'

export interface Settings {
  fontFamily: string
  codeFontFamily: string
  fontSize: number
  darkTheme: string
  lightTheme: string
}

export const PROSE_FONTS = [
  { label: 'System Default', value: 'system-ui, -apple-system, BlinkMacSystemFont, sans-serif' },
  { label: 'Inter', value: "'Inter', sans-serif" },
  { label: 'Source Sans 3', value: "'Source Sans 3', sans-serif" },
  { label: 'Nunito Sans', value: "'Nunito Sans', sans-serif" },
  { label: 'Literata', value: "'Literata', serif" },
  { label: 'Merriweather', value: "'Merriweather', serif" },
]

export const CODE_FONTS = [
  { label: 'JetBrains Mono', value: "'JetBrains Mono', monospace" },
  { label: 'Fira Code', value: "'Fira Code', monospace" },
  { label: 'Source Code Pro', value: "'Source Code Pro', monospace" },
  { label: 'IBM Plex Mono', value: "'IBM Plex Mono', monospace" },
  { label: 'Cascadia Code', value: "'Cascadia Code', monospace" },
  { label: 'Victor Mono', value: "'Victor Mono', monospace" },
  { label: 'Inconsolata', value: "'Inconsolata', monospace" },
  { label: 'Space Mono', value: "'Space Mono', monospace" },
  { label: 'Roboto Mono', value: "'Roboto Mono', monospace" },
  { label: 'Ubuntu Mono', value: "'Ubuntu Mono', monospace" },
]

const STORAGE_KEY = 'k-learn:settings'

const DEFAULT_SETTINGS: Settings = {
  fontFamily: 'system-ui, -apple-system, BlinkMacSystemFont, sans-serif',
  codeFontFamily: "'JetBrains Mono', monospace",
  fontSize: 16,
  darkTheme: DEFAULT_DARK_THEME,
  lightTheme: DEFAULT_LIGHT_THEME,
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

function applyThemeColors(theme: ThemeColors) {
  if (typeof document === 'undefined') return
  const root = document.documentElement
  const c = theme.colors

  root.style.setProperty('--vp-c-bg', c.bg)
  root.style.setProperty('--vp-c-bg-soft', c.bgSoft)
  root.style.setProperty('--vp-c-bg-mute', c.bgMute)
  root.style.setProperty('--vp-c-bg-alt', c.bgAlt)
  root.style.setProperty('--vp-c-text-1', c.text1)
  root.style.setProperty('--vp-c-text-2', c.text2)
  root.style.setProperty('--vp-c-text-3', c.text3)
  root.style.setProperty('--vp-c-brand-1', c.brand1)
  root.style.setProperty('--vp-c-brand-2', c.brand2)
  root.style.setProperty('--vp-c-brand-3', c.brand3)
  root.style.setProperty('--vp-c-brand-soft', c.brandSoft)
  root.style.setProperty('--vp-c-default-1', c.bgMute)
  root.style.setProperty('--vp-c-default-2', c.bgSoft)
  root.style.setProperty('--vp-c-default-3', c.bg)
  root.style.setProperty('--vp-c-default-soft', c.brandSoft)
  root.style.setProperty('--vp-c-divider', c.divider)
  root.style.setProperty('--vp-c-gutter', c.gutter)
  root.style.setProperty('--vp-sidebar-bg-color', c.sidebarBg)
  root.style.setProperty('--vp-code-block-bg', c.codeBlockBg)
  root.style.setProperty('--vp-c-tip-1', c.tip)
  root.style.setProperty('--vp-c-tip-soft', c.tipSoft)
  root.style.setProperty('--vp-c-warning-1', c.warning)
  root.style.setProperty('--vp-c-warning-soft', c.warningSoft)
  root.style.setProperty('--vp-c-danger-1', c.danger)
  root.style.setProperty('--vp-c-danger-soft', c.dangerSoft)

  // Brand button colors
  root.style.setProperty('--vp-button-brand-bg', c.brand1)
  root.style.setProperty('--vp-button-brand-hover-bg', c.brand2)
  root.style.setProperty('--vp-button-brand-active-bg', c.brand3)

  root.setAttribute('data-theme', theme.id)
}

function applyToDOM(s: Settings) {
  if (typeof document === 'undefined') return
  const root = document.documentElement
  root.style.setProperty('--vp-font-family-base', s.fontFamily)
  root.style.setProperty('--vp-font-family-mono', s.codeFontFamily)
  root.style.setProperty('--k-font-size', `${s.fontSize}px`)

  // Apply the correct theme based on current dark/light mode
  const isDark = root.classList.contains('dark')
  const themeId = isDark ? s.darkTheme : s.lightTheme
  const theme = getThemeById(themeId)
  if (theme) applyThemeColors(theme)
}

export function useSettings() {
  if (!initialized && typeof window !== 'undefined') {
    settings.value = loadFromStorage()
    applyToDOM(settings.value)
    initialized = true

    watch(settings, (val) => {
      saveToStorage(val)
      applyToDOM(val)
    }, { deep: true })

    // Re-apply theme when VitePress toggles dark/light mode
    const observer = new MutationObserver(() => {
      applyToDOM(settings.value)
    })
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['class'],
    })
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

  function setDarkTheme(id: string) {
    settings.value = { ...settings.value, darkTheme: id }
  }

  function setLightTheme(id: string) {
    settings.value = { ...settings.value, lightTheme: id }
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
    setDarkTheme,
    setLightTheme,
    resetSettings,
    applySettings,
    PROSE_FONTS,
    CODE_FONTS,
  }
}
