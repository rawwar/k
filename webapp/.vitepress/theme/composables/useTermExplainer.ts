import { ref, onMounted, onUnmounted } from 'vue'
import { glossary } from '../glossary'

export interface TooltipState {
  visible: boolean
  term: string
  definition: string
  category: string
  x: number
  y: number
}

export function useTermExplainer() {
  const state = ref<TooltipState>({
    visible: false,
    term: '',
    definition: '',
    category: '',
    x: 0,
    y: 0,
  })

  function lookupTerm(raw: string): { term: string; definition: string; category: string } | null {
    const normalized = raw.trim().toLowerCase().replace(/[.,;:!?'"()[\]]/g, '')
    if (!normalized || normalized.length < 2) return null

    // Exact match first (handles multi-word terms naturally since selection can be anything)
    if (glossary[normalized]) {
      return { term: raw.trim(), ...glossary[normalized] }
    }

    // Try without trailing 's' (simple singular)
    const singular = normalized.endsWith('s') ? normalized.slice(0, -1) : null
    if (singular && glossary[singular]) {
      return { term: raw.trim(), ...glossary[singular] }
    }

    // Try without trailing 'ing' → base form (e.g. "streaming" → "stream")
    if (normalized.endsWith('ing') && normalized.length > 5) {
      const base = normalized.slice(0, -3)
      if (glossary[base]) return { term: raw.trim(), ...glossary[base] }
    }

    return null
  }

  function getSelectionText(): string {
    const sel = window.getSelection()
    if (!sel || sel.isCollapsed) return ''
    return sel.toString()
  }

  function getSelectionRect(): DOMRect | null {
    const sel = window.getSelection()
    if (!sel || sel.rangeCount === 0) return null
    return sel.getRangeAt(0).getBoundingClientRect()
  }

  function handleDblClick() {
    const text = getSelectionText()
    const result = lookupTerm(text)
    if (!result) {
      state.value.visible = false
      return
    }

    const rect = getSelectionRect()
    if (!rect) return

    // Position above the selection, centered horizontally
    const tooltipWidth = 340
    const margin = 12

    let x = rect.left + rect.width / 2 - tooltipWidth / 2
    // Clamp to viewport
    x = Math.max(margin, Math.min(x, window.innerWidth - tooltipWidth - margin))

    // y = top of selection; component decides above/below based on available space
    const y = rect.top + window.scrollY

    state.value = {
      visible: true,
      term: result.term,
      definition: result.definition,
      category: result.category,
      x,
      y,
    }
  }

  function hide() {
    state.value.visible = false
  }

  onMounted(() => {
    document.addEventListener('dblclick', handleDblClick)
  })

  onUnmounted(() => {
    document.removeEventListener('dblclick', handleDblClick)
  })

  return { tooltipState: state, hide }
}
