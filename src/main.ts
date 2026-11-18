import { createApp } from 'vue'
import { createPinia } from 'pinia'
import './styles/main.css'
import './styles/dashboard.css'
import './styles/task-cards.css'
import App from './App.vue'
import { router } from './router'
import { useThemeStore } from './stores/theme'

// Custom application menus handle context actions; suppress WebView/browser defaults.
// Do not stop propagation: card and file menus still receive the event.
document.addEventListener('contextmenu', (event) => event.preventDefault())

const pinia = createPinia()
// Apply the saved theme before mounting the first frame.
async function bootstrap() {
  await useThemeStore(pinia).hydrate()
  createApp(App).use(pinia).use(router).mount('#app')
}
void bootstrap()
