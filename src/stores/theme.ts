import { computed, ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { appDarkTheme } from '../theme/naive'
import { preferences } from '../services/preferences'
import { desktop } from '../services/api'
import { listen } from '@tauri-apps/api/event'
import {
  applyThemeVariables,
  darkThemeOverrides,
  readThemePreference,
  themeOverrides,
  type ThemeMode,
} from '../theme'

export const useThemeStore = defineStore('theme', () => {
  const preference = ref<ThemeMode | 'system'>(readThemePreference())
  const systemDark = ref(window.matchMedia('(prefers-color-scheme: dark)').matches)
  const error = ref('')
  const mode = computed<ThemeMode>(() =>
    preference.value === 'system' ? (systemDark.value ? 'dark' : 'light') : preference.value,
  )
  const isDark = computed(() => mode.value === 'dark')
  const naiveTheme = computed(() => (isDark.value ? appDarkTheme : null))
  const overrides = computed(() => (isDark.value ? darkThemeOverrides : themeOverrides))
  watch(
    mode,
    (value) => {
      applyThemeVariables(value)
      document
        .querySelector<HTMLLinkElement>('link[rel="icon"]')
        ?.setAttribute('href', value === 'dark' ? '/app-icon.svg' : '/app-icon-light.svg')
      document
        .querySelector('meta[name="theme-color"]')
        ?.setAttribute('content', value === 'dark' ? '#111111' : '#f8f8f8')
    },
    { immediate: true },
  )
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (event) => {
    systemDark.value = event.matches
  })
  async function hydrate() {
    try {
      const saved = await preferences.get('appearance')
      preference.value = saved === 'light' || saved === 'dark' ? saved : 'system'
      error.value = ''
    } catch {
      error.value = '无法读取本地外观设置，请重试'
    }
  }
  let saveQueue = Promise.resolve()
  async function setTheme(nextMode: ThemeMode | 'system') {
    preference.value = nextMode
    const save = saveQueue.catch(() => {}).then(() => preferences.set('appearance', nextMode))
    saveQueue = save
    try {
      await save
      error.value = ''
    } catch {
      error.value = '外观已切换，但未能保存到本地，请重试'
    }
  }
  if (desktop)
    void listen<string>('preference-changed', ({ payload }) => {
      if (payload === 'appearance') void hydrate()
    }).catch(() => {})
  return { preference, mode, isDark, naiveTheme, overrides, error, hydrate, setTheme }
})
