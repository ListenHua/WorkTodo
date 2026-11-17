import { createApp } from 'vue'
import { createPinia } from 'pinia'
import './styles/main.css'
import App from './App.vue'
import { router } from './router'
import { useThemeStore } from './stores/theme'

const pinia = createPinia()
// Apply the saved theme before mounting the first frame.
async function bootstrap() {
  await useThemeStore(pinia).hydrate()
  createApp(App).use(pinia).use(router).mount('#app')
}
void bootstrap()
