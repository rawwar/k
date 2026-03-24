import DefaultTheme from 'vitepress/theme'
// Code fonts
import '@fontsource/jetbrains-mono/400.css'
import '@fontsource/jetbrains-mono/700.css'
import '@fontsource/fira-code/400.css'
import '@fontsource/fira-code/700.css'
import '@fontsource/source-code-pro/400.css'
import '@fontsource/source-code-pro/700.css'
import '@fontsource/ibm-plex-mono/400.css'
import '@fontsource/ibm-plex-mono/700.css'
import '@fontsource/cascadia-code/400.css'
import '@fontsource/cascadia-code/700.css'
import '@fontsource/victor-mono/400.css'
import '@fontsource/victor-mono/700.css'
import '@fontsource/inconsolata/400.css'
import '@fontsource/inconsolata/700.css'
import '@fontsource/space-mono/400.css'
import '@fontsource/space-mono/700.css'
import '@fontsource/roboto-mono/400.css'
import '@fontsource/roboto-mono/700.css'
import '@fontsource/ubuntu-mono/400.css'
import '@fontsource/ubuntu-mono/700.css'
// Prose fonts
import '@fontsource/inter/400.css'
import '@fontsource/inter/700.css'
import '@fontsource/source-sans-3/400.css'
import '@fontsource/source-sans-3/700.css'
import '@fontsource/nunito-sans/400.css'
import '@fontsource/nunito-sans/700.css'
import '@fontsource/literata/400.css'
import '@fontsource/literata/700.css'
import '@fontsource/merriweather/400.css'
import '@fontsource/merriweather/700.css'
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
