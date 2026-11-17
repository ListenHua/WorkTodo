<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { NButton, NDropdown, NModal } from 'naive-ui'
import { useWorkspaceStore } from '../stores/workspace'
import { useAppMessage } from '../composables/useAppMessage'
import { errorMessage } from '../shared/domain'
import { projectSortOptions, sortProjects, type ProjectSortMode } from '../shared/project-sort'
import AppIcon from './AppIcon.vue'
import IconHint from './IconHint.vue'

const show = defineModel<boolean>('show', { required: true })
const workspace = useWorkspaceStore()
const message = useAppMessage()
const mode = ref<ProjectSortMode>('manual')
const modeOptions = projectSortOptions.map((option) => ({ label: option.label, key: option.value }))
const modeLabel = computed(
  () => projectSortOptions.find((option) => option.value === mode.value)?.label,
)
const order = ref<string[]>([])
const projects = computed(() =>
  sortProjects(workspace.projects, { mode: mode.value, order: order.value }),
)
const list = ref<HTMLUListElement | null>(null)
const error = ref('')
const draggingId = ref<string | null>(null)
const dropBefore = ref<string | null>()
let drag: {
  id: string
  pointerId: number
  handle: HTMLButtonElement
  startY: number
  x: number
  y: number
} | null = null
let frame = 0
watch(show, (visible) => {
  cancelDrag()
  if (!visible) return
  mode.value = workspace.projectSort.mode
  order.value = workspace.orderedProjects.map((project) => project.id)
  error.value = ''
})
function changeMode(value: ProjectSortMode) {
  cancelDrag()
  // Keep the displayed order when switching to manual sorting.
  order.value = projects.value.map((project) => project.id)
  mode.value = value
  error.value = ''
}
function cancelDrag() {
  const previous = drag
  drag = null
  draggingId.value = null
  dropBefore.value = undefined
  cancelAnimationFrame(frame)
  document.removeEventListener('keydown', escapeDrag, true)
  window.removeEventListener('blur', cancelDrag)
  if (previous?.handle.hasPointerCapture(previous.pointerId))
    previous.handle.releasePointerCapture(previous.pointerId)
}
function escapeDrag(event: KeyboardEvent) {
  if (event.key !== 'Escape') return
  event.preventDefault()
  event.stopImmediatePropagation()
  cancelDrag()
}
function updateTarget() {
  if (!drag || !list.value) return
  const bounds = list.value.getBoundingClientRect()
  if (
    drag.x < bounds.left ||
    drag.x > bounds.right ||
    drag.y < bounds.top - 12 ||
    drag.y > bounds.bottom + 12
  ) {
    dropBefore.value = undefined
    return
  }
  dropBefore.value =
    [...list.value.querySelectorAll<HTMLElement>('.project-sort-row')].find((row) => {
      if (row.dataset.projectId === drag!.id) return false
      const box = row.getBoundingClientRect()
      return drag!.y < box.top + box.height / 2
    })?.dataset.projectId ?? null
}
function scrollDrag() {
  if (!drag || !draggingId.value || !list.value) return
  const bounds = list.value.getBoundingClientRect()
  if (drag.x >= bounds.left && drag.x <= bounds.right) {
    const delta = drag.y < bounds.top + 32 ? -10 : drag.y > bounds.bottom - 32 ? 10 : 0
    list.value.scrollTop += delta
  }
  updateTarget()
  frame = requestAnimationFrame(scrollDrag)
}
function startDrag(event: PointerEvent, id: string) {
  if (
    !event.isPrimary ||
    event.button !== 0 ||
    workspace.sortingProjects ||
    projects.value.length < 2
  )
    return
  event.preventDefault()
  cancelDrag()
  const handle = event.currentTarget as HTMLButtonElement
  handle.focus({ preventScroll: true })
  handle.setPointerCapture(event.pointerId)
  drag = {
    id,
    pointerId: event.pointerId,
    handle,
    startY: event.clientY,
    x: event.clientX,
    y: event.clientY,
  }
  document.addEventListener('keydown', escapeDrag, true)
  window.addEventListener('blur', cancelDrag)
}
function moveDrag(event: PointerEvent) {
  if (!drag || drag.pointerId !== event.pointerId) return
  drag.x = event.clientX
  drag.y = event.clientY
  if (!draggingId.value) {
    if (Math.abs(drag.y - drag.startY) < 5) return
    draggingId.value = drag.id
    frame = requestAnimationFrame(scrollDrag)
  }
  event.preventDefault()
  updateTarget()
}
function finishDrag(event: PointerEvent) {
  if (!drag || drag.pointerId !== event.pointerId) return
  updateTarget()
  const id = draggingId.value
  const before = dropBefore.value
  cancelDrag()
  if (!id || before === undefined) return
  const next = projects.value.map((project) => project.id).filter((projectId) => projectId !== id)
  if (!workspace.projects.some((project) => project.id === id)) return
  const index = before === null ? next.length : next.indexOf(before)
  if (index < 0) return
  next.splice(index, 0, id)
  order.value = next
  mode.value = 'manual'
  error.value = ''
}
async function moveWithKeyboard(id: string, offset: number) {
  if (workspace.sortingProjects || drag) return
  const next = projects.value.map((project) => project.id)
  const index = next.indexOf(id)
  if (index < 0 || index + offset < 0 || index + offset >= next.length) return
  next.splice(index, 1)
  next.splice(index + offset, 0, id)
  order.value = next
  mode.value = 'manual'
  await nextTick()
  const row = [...(list.value?.querySelectorAll<HTMLElement>('.project-sort-row') ?? [])].find(
    (row) => row.dataset.projectId === id,
  )
  row?.querySelector<HTMLButtonElement>('button')?.focus()
}
async function save() {
  if (workspace.sortingProjects || draggingId.value) return
  error.value = ''
  try {
    await workspace.saveProjectSort({
      mode: mode.value,
      order: projects.value.map((project) => project.id),
    })
    show.value = false
    message.success('项目顺序已保存')
  } catch (cause) {
    error.value = errorMessage(cause)
  }
}
onBeforeUnmount(cancelDrag)
</script>

<template>
  <NModal
    v-model:show="show"
    preset="card"
    title="排序项目"
    aria-label="排序项目"
    class="form-modal project-sort-modal"
    :bordered="false"
    :closable="false"
    :mask-closable="false"
    :close-on-esc="!workspace.sortingProjects && !draggingId"
  >
    <template #header-extra>
      <button
        type="button"
        class="icon-button"
        aria-label="关闭排序弹窗"
        :disabled="workspace.sortingProjects"
        @click="show = false"
      >
        <AppIcon name="close" />
      </button>
    </template>
    <form class="dialog-form" @submit.prevent="save">
      <NDropdown
        trigger="click"
        :value="mode"
        :options="modeOptions"
        :disabled="workspace.sortingProjects || !!draggingId"
        :menu-props="() => ({ role: 'menu', 'aria-label': '排序方式选项' })"
        :node-props="() => ({ role: 'menuitem' })"
        @select="changeMode"
      >
        <NButton
          block
          quaternary
          class="project-sort-mode"
          aria-label="排序方式"
          :disabled="workspace.sortingProjects || !!draggingId"
        >
          <span>{{ modeLabel }}</span><AppIcon name="down" :size="16" />
        </NButton>
      </NDropdown>
      <ul
        ref="list"
        class="project-sort-list"
        aria-label="项目排序列表"
        :class="{ 'is-sorting': draggingId }"
        @dragstart.prevent
      >
        <li
          v-for="project in projects"
          :key="project.id"
          class="project-sort-row"
          :data-project-id="project.id"
          :class="{
            'is-dragging': draggingId === project.id,
            'drop-before': draggingId && dropBefore === project.id,
            'drop-after': draggingId && dropBefore === null && project.id === projects.at(-1)?.id,
          }"
        >
          <IconHint text="拖拽排序" :disabled="!!draggingId">
            <button
              type="button"
              class="icon-button project-sort-handle"
              :aria-label="`拖拽排序 ${project.name}`"
              aria-keyshortcuts="ArrowUp ArrowDown"
              :disabled="workspace.sortingProjects || projects.length < 2"
              @pointerdown="startDrag($event, project.id)"
              @pointermove="moveDrag"
              @pointerup="finishDrag"
              @pointercancel="cancelDrag"
              @lostpointercapture="cancelDrag"
              @keydown.up.prevent="moveWithKeyboard(project.id, -1)"
              @keydown.down.prevent="moveWithKeyboard(project.id, 1)"
            >
              <AppIcon name="drag" :size="18" />
            </button>
          </IconHint>
          <span
            class="project-sort-color"
            :style="{ backgroundColor: project.color }"
            aria-hidden="true"
          />
          <span class="project-sort-name">{{ project.name }}</span>
        </li>
      </ul>
      <p v-if="error" class="form-error" role="alert">{{ error }}</p>
      <div class="dialog-footer">
        <NButton :disabled="workspace.sortingProjects" @click="show = false">取消</NButton>
        <NButton
          type="primary"
          attr-type="submit"
          :loading="workspace.sortingProjects"
          :disabled="!!draggingId"
          >保存</NButton
        >
      </div>
    </form>
  </NModal>
</template>
