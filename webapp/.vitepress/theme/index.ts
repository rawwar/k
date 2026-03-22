import DefaultTheme from 'vitepress/theme'
import '@fontsource/jetbrains-mono/400.css'
import '@fontsource/jetbrains-mono/700.css'
import '@fontsource/inter/400.css'
import '@fontsource/inter/700.css'
import '@fontsource/fira-code/400.css'
import '@fontsource/fira-code/700.css'
import '@fontsource/source-code-pro/400.css'
import '@fontsource/source-code-pro/700.css'
import '@fontsource/ibm-plex-mono/400.css'
import '@fontsource/ibm-plex-mono/700.css'
import './style.css'
import TrackPicker from './components/TrackPicker.vue'
import CustomLayout from './components/CustomLayout.vue'
import { useProgress } from './composables/useProgress'
import { useSettings } from './composables/useSettings'

export default {
  extends: DefaultTheme,
  Layout: CustomLayout,
  enhanceApp({ app, router }) {
    app.component('TrackPicker', TrackPicker)

    if (typeof window !== 'undefined') {
      // Apply saved font settings on load
      const { applySettings } = useSettings()
      applySettings()

      // Track page visits for progress
      router.onAfterRouteChanged = (to: string) => {
        const { markVisited } = useProgress()
        markVisited(to)
      }
    }
  }
}
