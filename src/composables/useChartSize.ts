import { onMounted, onUnmounted, ref } from 'vue'

export function useChartSize() {
  const element = ref<HTMLElement | null>(null)
  const width = ref(520)
  const height = ref(240)
  let observer: ResizeObserver | undefined
  onMounted(() => {
    if (!element.value) return
    observer = new ResizeObserver(([entry]) => {
      if (!entry) return
      width.value = Math.max(160, entry.contentRect.width)
      height.value = Math.max(160, entry.contentRect.height)
    })
    observer.observe(element.value)
  })
  onUnmounted(() => observer?.disconnect())
  return { element, width, height }
}
