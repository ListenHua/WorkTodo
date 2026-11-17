<script setup lang="ts">
import IconHint from '../IconHint.vue'
// Vue adaptation of Magic UI's AnimatedThemeToggler (MIT). See public/licenses/magic-ui.txt.
// https://github.com/magicuidesign/magicui/blob/main/apps/www/registry/magicui/animated-theme-toggler.tsx
import { computed, nextTick, ref } from 'vue'
import { useThemeStore } from '../../stores/theme'
import AppIcon from '../AppIcon.vue'

const props = withDefaults(defineProps<{ duration?: number }>(), { duration: 400 })
const theme = useThemeStore()
const button = ref<HTMLButtonElement | null>(null)
const isAnimating = ref(false)
const label = computed(() => (theme.isDark ? '切换到浅色模式' : '切换到深色模式'))

async function toggleTheme() {
  if (!button.value || isAnimating.value) return
  const nextMode = theme.isDark ? 'light' : 'dark'
  const updateTheme = async () => {
    await theme.setTheme(nextMode)
    await nextTick()
  }

  if (
    typeof document.startViewTransition !== 'function' ||
    window.matchMedia('(prefers-reduced-motion: reduce)').matches
  ) {
    await updateTheme()
    return
  }

  isAnimating.value = true
  const { top, left, width, height } = button.value.getBoundingClientRect()
  const x = left + width / 2
  const y = top + height / 2
  const radius = Math.hypot(Math.max(x, window.innerWidth - x), Math.max(y, window.innerHeight - y))
  let finished: Promise<unknown> = Promise.resolve()

  try {
    const transition = document.startViewTransition(updateTheme)
    finished = transition.finished.catch(() => undefined)
    await transition.ready
    await document.documentElement.animate(
      {
        clipPath: [`circle(0px at ${x}px ${y}px)`, `circle(${radius}px at ${x}px ${y}px)`],
      },
      {
        duration: props.duration,
        easing: 'ease-in-out',
        pseudoElement: '::view-transition-new(root)',
      },
    ).finished
  } catch {
    // Hidden windows and interrupted transitions can skip animation; keep the selected theme.
    if (theme.mode !== nextMode) await updateTheme()
  } finally {
    await finished
    isAnimating.value = false
  }
}
</script>

<template>
  <IconHint :text="label"
    ><button
      ref="button"
      type="button"
      class="icon-button theme-toggler"
      :aria-label="label"
      :aria-pressed="theme.isDark"
      :aria-busy="isAnimating"
      :disabled="isAnimating"
      @click="toggleTheme"
    >
      <AppIcon :name="theme.isDark ? 'sun' : 'moon'" :size="19" /></button
  ></IconHint>
</template>
