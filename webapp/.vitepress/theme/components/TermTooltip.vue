<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'

const props = defineProps<{
  visible: boolean
  term: string
  definition: string
  category: string
  x: number
  y: number
}>()

const emit = defineEmits<{
  (e: 'hide'): void
}>()

// Position the tooltip above the selection, flipping below if near top of viewport
const style = computed(() => {
  const TOOLTIP_HEIGHT_APPROX = 160
  const ARROW_OFFSET = 12
  const scrollY = typeof window !== 'undefined' ? window.scrollY : 0
  const viewportY = props.y - scrollY

  const showBelow = viewportY < TOOLTIP_HEIGHT_APPROX + 40

  const top = showBelow
    ? props.y + 28 // below
    : props.y - TOOLTIP_HEIGHT_APPROX - ARROW_OFFSET // above

  return {
    left: `${props.x}px`,
    top: `${top}px`,
  }
})

const arrowStyle = computed(() => {
  const scrollY = typeof window !== 'undefined' ? window.scrollY : 0
  const viewportY = props.y - scrollY
  const TOOLTIP_HEIGHT_APPROX = 160
  const showBelow = viewportY < TOOLTIP_HEIGHT_APPROX + 40
  return showBelow ? 'arrow-below' : 'arrow-above'
})

function onKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape') emit('hide')
}

function onClickOutside(e: MouseEvent) {
  const el = document.getElementById('term-tooltip')
  if (el && !el.contains(e.target as Node)) {
    emit('hide')
  }
}

onMounted(() => {
  document.addEventListener('keydown', onKeyDown)
  // Use setTimeout to avoid the dblclick that opened us from immediately closing us
  setTimeout(() => {
    document.addEventListener('mousedown', onClickOutside)
  }, 50)
})

onUnmounted(() => {
  document.removeEventListener('keydown', onKeyDown)
  document.removeEventListener('mousedown', onClickOutside)
})
</script>

<template>
  <Transition name="tooltip-fade">
    <div
      v-if="visible"
      id="term-tooltip"
      class="term-tooltip"
      :style="style"
      :class="arrowStyle"
    >
      <div class="term-tooltip-header">
        <span class="term-tooltip-term">{{ term }}</span>
        <span class="term-tooltip-badge">{{ category }}</span>
        <button class="term-tooltip-close" @click="emit('hide')" aria-label="Close">✕</button>
      </div>
      <p class="term-tooltip-definition">{{ definition }}</p>
    </div>
  </Transition>
</template>

<style scoped>
.term-tooltip {
  position: absolute;
  z-index: 9999;
  width: 340px;
  background: var(--tt-bg, #1e1e2e);
  border: 1px solid var(--tt-border, #313244);
  border-radius: 10px;
  padding: 14px 16px 14px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.45), 0 2px 8px rgba(0, 0, 0, 0.3);
  font-family: var(--vp-font-family-base);
  pointer-events: all;
}

/* Arrow above (tooltip is above selection) */
.term-tooltip.arrow-above::after {
  content: '';
  position: absolute;
  bottom: -7px;
  left: 50%;
  transform: translateX(-50%);
  border-width: 7px 7px 0;
  border-style: solid;
  border-color: var(--tt-border, #313244) transparent transparent;
}
.term-tooltip.arrow-above::before {
  content: '';
  position: absolute;
  bottom: -6px;
  left: 50%;
  transform: translateX(-50%);
  border-width: 6px 6px 0;
  border-style: solid;
  border-color: var(--tt-bg, #1e1e2e) transparent transparent;
  z-index: 1;
}

/* Arrow below (tooltip is below selection) */
.term-tooltip.arrow-below::after {
  content: '';
  position: absolute;
  top: -7px;
  left: 50%;
  transform: translateX(-50%);
  border-width: 0 7px 7px;
  border-style: solid;
  border-color: transparent transparent var(--tt-border, #313244);
}
.term-tooltip.arrow-below::before {
  content: '';
  position: absolute;
  top: -6px;
  left: 50%;
  transform: translateX(-50%);
  border-width: 0 6px 6px;
  border-style: solid;
  border-color: transparent transparent var(--tt-bg, #1e1e2e);
  z-index: 1;
}

.term-tooltip-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.term-tooltip-term {
  font-weight: 700;
  font-size: 0.95rem;
  color: var(--tt-term, #89b4fa);
  flex: 1;
  text-transform: capitalize;
}

.term-tooltip-badge {
  font-size: 0.68rem;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--tt-badge-text, #a6e3a1);
  background: var(--tt-badge-bg, rgba(166, 227, 161, 0.12));
  border-radius: 4px;
  padding: 2px 6px;
  white-space: nowrap;
}

.term-tooltip-close {
  background: none;
  border: none;
  color: var(--tt-muted, #6c7086);
  cursor: pointer;
  font-size: 0.8rem;
  padding: 0;
  line-height: 1;
  transition: color 0.15s;
  flex-shrink: 0;
}
.term-tooltip-close:hover {
  color: var(--tt-term, #89b4fa);
}

.term-tooltip-definition {
  font-size: 0.84rem;
  line-height: 1.6;
  color: var(--tt-text, #cdd6f4);
  margin: 0;
}

/* Fade + scale transition */
.tooltip-fade-enter-active {
  transition: opacity 0.15s ease, transform 0.15s ease;
}
.tooltip-fade-leave-active {
  transition: opacity 0.1s ease, transform 0.1s ease;
}
.tooltip-fade-enter-from,
.tooltip-fade-leave-to {
  opacity: 0;
  transform: scale(0.95) translateY(4px);
}

/* Light mode overrides */
:root:not(.dark) .term-tooltip {
  --tt-bg: #ffffff;
  --tt-border: #e2e8f0;
  --tt-term: #2563eb;
  --tt-badge-text: #059669;
  --tt-badge-bg: rgba(5, 150, 105, 0.1);
  --tt-text: #374151;
  --tt-muted: #9ca3af;
}
</style>
