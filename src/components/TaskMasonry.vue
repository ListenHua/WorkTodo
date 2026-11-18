<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch, type ComponentPublicInstance } from 'vue'
import type { Task } from '../shared/types'

const props = defineProps<{ tasks: Task[] }>()
defineSlots<{ default(props: { task: Task }): unknown }>()
const container = ref<HTMLElement | null>(null)
const cardWidth = ref(0)
const height = ref(0)
const positions = ref<Record<string, { left: number; top: number }>>({})
const readyIds = ref(new Set<string>())
const elements = new Map<string, HTMLElement>()
let observer: ResizeObserver | undefined
let frame = 0
let disposed = false

function scheduleLayout() {
  if (disposed) return
  cancelAnimationFrame(frame)
  frame = requestAnimationFrame(() => void layout())
}
async function layout() {
  const available = container.value?.clientWidth ?? 0
  if (!available || disposed) return
  const gap = 16
  const targetWidth = 320
  const columns = Math.max(1, Math.floor((available + gap) / (targetWidth + gap)))
  cardWidth.value = Math.min(targetWidth, available)
  // Text must wrap at the final width before measuring each card's natural height.
  await nextTick()
  if (disposed) return
  const bottoms = Array<number>(columns).fill(0)
  const next: typeof positions.value = {}
  for (const task of props.tasks) {
    const element = elements.get(task.id)
    if (!element) continue
    const column = bottoms.indexOf(Math.min(...bottoms))
    next[task.id] = { left: column * (cardWidth.value + gap), top: bottoms[column]! }
    bottoms[column]! += element.offsetHeight + gap
  }
  const hasNewCards = Object.keys(next).some((id) => !readyIds.value.has(id))
  positions.value = next
  height.value = Math.max(0, ...bottoms.map((bottom) => Math.max(0, bottom - gap)))
  if (hasNewCards) {
    await nextTick()
    if (disposed) return
    // Commit initial coordinates before enabling transitions between later layouts.
    container.value?.getBoundingClientRect()
  }
  readyIds.value = new Set(Object.keys(next))
}
function setElement(id: string, value: Element | ComponentPublicInstance | null) {
  const previous = elements.get(id)
  if (previous === value) return
  if (previous) observer?.unobserve(previous)
  if (value instanceof HTMLElement) {
    elements.set(id, value)
    observer?.observe(value)
  } else elements.delete(id)
  scheduleLayout()
}
watch(() => props.tasks.map((task) => task.id), scheduleLayout)
onMounted(() => {
  observer = new ResizeObserver(scheduleLayout)
  if (container.value) observer.observe(container.value)
  elements.forEach((element) => observer!.observe(element))
  scheduleLayout()
})
onBeforeUnmount(() => {
  disposed = true
  observer?.disconnect()
  cancelAnimationFrame(frame)
})
</script>

<template>
  <div class="task-masonry-wrapper">
    <ul ref="container" class="task-list task-masonry" :style="{ height: `${height}px` }">
      <li
        v-for="task in tasks"
        :key="task.id"
        :ref="(element) => setElement(task.id, element)"
        class="task-masonry-item"
        :class="{ 'is-positioned': !!positions[task.id], 'is-ready': readyIds.has(task.id) }"
        :data-task-id="task.id"
        :data-layout-x="positions[task.id]?.left ?? 0"
        :data-layout-y="positions[task.id]?.top ?? 0"
        :style="{
          width: cardWidth ? `${cardWidth}px` : '100%',
          transform: `translate3d(${positions[task.id]?.left ?? 0}px, ${positions[task.id]?.top ?? 0}px, 0)`,
          visibility: positions[task.id] ? 'visible' : 'hidden',
        }"
      >
        <slot :task="task" />
      </li>
    </ul>
  </div>
</template>
