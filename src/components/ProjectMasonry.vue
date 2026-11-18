<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch, type ComponentPublicInstance } from 'vue'
import type { Project } from '../shared/types'

const props = defineProps<{ projects: Project[] }>()
defineSlots<{ default(props: { project: Project; index: number }): unknown }>()
const container = ref<HTMLElement | null>(null)
const width = ref(0)
const height = ref(0)
const positions = ref<Record<string, { left: number; top: number }>>({})
const elements = new Map<string, HTMLElement>()
let observer: ResizeObserver | undefined
let frame = 0
let disposed = false

function scheduleLayout() {
  cancelAnimationFrame(frame)
  frame = requestAnimationFrame(() => void layout())
}
async function layout() {
  const available = container.value?.clientWidth ?? 0
  if (!available || disposed) return
  const gap = 20
  const columns = Math.min(3, Math.max(1, Math.floor((available + gap) / 280)))
  width.value = (available - gap * (columns - 1)) / columns
  // Measure after the new card width has been applied, including wrapped text.
  await nextTick()
  if (disposed) return
  const bottoms = Array<number>(columns).fill(0)
  const next: typeof positions.value = {}
  for (const project of props.projects) {
    const element = elements.get(project.id)
    if (!element) continue
    const column = bottoms.indexOf(Math.min(...bottoms))
    next[project.id] = { left: column * (width.value + gap), top: bottoms[column]! }
    bottoms[column]! += element.getBoundingClientRect().height + gap
  }
  positions.value = next
  height.value = Math.max(0, ...bottoms) - (props.projects.length ? gap : 0)
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
watch(() => props.projects.map((project) => project.id), scheduleLayout)
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
  <div ref="container" class="project-grid project-masonry" :style="{ height: `${height}px` }">
    <div
      v-for="(project, index) in projects"
      :key="project.id"
      :ref="(element) => setElement(project.id, element)"
      class="project-masonry-item"
      :data-layout-x="positions[project.id]?.left ?? 0"
      :data-layout-y="positions[project.id]?.top ?? 0"
      :style="{
        width: width ? `${width}px` : '100%',
        transform: `translate3d(${positions[project.id]?.left ?? 0}px, ${positions[project.id]?.top ?? 0}px, 0)`,
        visibility: positions[project.id] ? 'visible' : 'hidden',
      }"
    >
      <slot :project="project" :index="index" />
    </div>
  </div>
</template>
