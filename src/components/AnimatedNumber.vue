<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'

const props = defineProps<{ value: number }>()
const root = ref<HTMLElement | null>(null)
const digits = computed(() => String(Math.max(0, Math.floor(props.value))).split(''))
const reel = Array.from({ length: 30 }, (_, i) => i % 10)
let animations: Animation[] = []
let version = 0
watch(
  () => props.value,
  async (value, previous = 0) => {
    const current = ++version
    animations.forEach((animation) => animation.cancel())
    animations = []
    await nextTick()
    if (current !== version || window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
    const before = String(Math.max(0, Math.floor(previous)))
      .padStart(digits.value.length, '0')
      .slice(-digits.value.length)
    root.value?.querySelectorAll<HTMLElement>('.rolling-strip').forEach((strip, i) => {
      const from = Number(before[i] ?? 0) + 10
      let to = Number(digits.value[i]) + 10
      if (value >= previous && to < from) to += 10
      if (value < previous && to > from) to -= 10
      if (to === from) return
      animations.push(
        strip.animate(
          [{ transform: `translateY(-${from}em)` }, { transform: `translateY(-${to}em)` }],
          { duration: 720, easing: 'cubic-bezier(.2,.75,.25,1)' },
        ),
      )
    })
  },
  { immediate: true, flush: 'post' },
)
onBeforeUnmount(() => {
  version++
  animations.forEach((animation) => animation.cancel())
})
</script>

<template>
  <strong
    ref="root"
    class="animated-number"
    :aria-label="String(value)"
    aria-live="polite"
    aria-atomic="true"
  >
    <span
      v-for="(digit, index) in digits"
      :key="digits.length - index"
      class="rolling-digit"
      aria-hidden="true"
    >
      <span class="rolling-strip" :style="{ transform: `translateY(-${Number(digit) + 10}em)` }">
        <span v-for="(item, position) in reel" :key="position">{{ item }}</span>
      </span>
    </span>
  </strong>
</template>
