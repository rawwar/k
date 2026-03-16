import DefaultTheme from 'vitepress/theme'
import '@fontsource/jetbrains-mono/400.css'
import '@fontsource/jetbrains-mono/700.css'
import './style.css'
import TrackPicker from './components/TrackPicker.vue'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component('TrackPicker', TrackPicker)
  }
}
