import { computed, nextTick, onBeforeUnmount, ref, watch, type Ref } from 'vue'
import { useWorkspaceStore } from '../stores/workspace'
import { useAppMessage } from './useAppMessage'
import { errorMessage } from '../shared/domain'
import { sortProjects, type ProjectSortMode } from '../shared/project-sort'

export function useProjectCardSort(
  space: Ref<HTMLElement | null>,
  toolbar: Ref<HTMLElement | null>,
) {
  const workspace = useWorkspaceStore()
  const message = useAppMessage()
  const sorting = ref(false)
  const mode = ref<ProjectSortMode>('manual')
  const order = ref<string[]>([])
  const error = ref('')
  const announcement = ref('')
  const draggingId = ref<string | null>(null)
  const previewOrder = ref<string[] | null>(null)
  const projects = computed(() =>
    sorting.value
      ? sortProjects(
          workspace.projects,
          previewOrder.value
            ? { mode: 'manual', order: previewOrder.value }
            : { mode: mode.value, order: order.value },
        )
      : workspace.orderedProjects,
  )
  let drag: {
    id: string
    pointerId: number
    card: HTMLElement
    capture: HTMLElement
    validDrop: boolean
    lockedArea: {
      id: string
      left: number
      top: number
      right: number
      bottom: number
      after: boolean
    } | null
    startX: number
    startY: number
    x: number
    y: number
    offsetX: number
    offsetY: number
  } | null = null
  let preview: HTMLElement | null = null
  let frame = 0

  function cancelDrag() {
    const previous = drag
    drag = null
    draggingId.value = null
    previewOrder.value = null
    preview?.remove()
    preview = null
    cancelAnimationFrame(frame)
    window.removeEventListener('blur', cancelDrag)
    window.removeEventListener('resize', cancelDrag)
    if (previous?.capture.hasPointerCapture(previous.pointerId))
      previous.capture.releasePointerCapture(previous.pointerId)
  }
  function beginSorting() {
    if (workspace.loading || workspace.sortingProjects || workspace.status.readOnly) return
    cancelDrag()
    order.value = workspace.orderedProjects.map((project) => project.id)
    mode.value = workspace.projectSort.mode
    error.value = ''
    announcement.value = ''
    sorting.value = true
  }
  function cancelSorting() {
    cancelDrag()
    sorting.value = false
    error.value = ''
  }
  function changeMode(value: ProjectSortMode) {
    cancelDrag()
    order.value = projects.value.map((project) => project.id)
    mode.value = value
    error.value = ''
  }
  function updateTarget() {
    if (!drag || !space.value) return
    const grid = space.value.querySelector<HTMLElement>('.project-grid')
    const bounds = grid?.getBoundingClientRect()
    const toolbarBounds = toolbar.value?.getBoundingClientRect()
    drag.validDrop = !!(
      draggingId.value &&
      bounds &&
      Math.hypot(drag.x - drag.startX, drag.y - drag.startY) >= 5 &&
      drag.x >= bounds.left - 20 &&
      drag.x <= bounds.right + 20 &&
      drag.y >= Math.max(bounds.top - 20, toolbarBounds?.bottom ?? 0) &&
      drag.y <= bounds.bottom + 20
    )
    if (!drag.validDrop || !bounds) return
    const x = drag.x - bounds.left
    const y = drag.y - bounds.top
    // Hold the insertion while the pointer is still over the area it entered.
    // Reflowing cards must not repeatedly swap places under a stationary pointer.
    const locked = drag.lockedArea
    if (
      locked &&
      x >= locked.left - 8 &&
      x <= locked.right + 8 &&
      y >= locked.top - 8 &&
      y <= locked.bottom + 8
    ) {
      const middle = (locked.top + locked.bottom) / 2
      const after = locked.after ? y >= middle - 6 : y > middle + 6
      if (after !== locked.after) {
        locked.after = after
        previewPlacement(locked.id, after)
      }
      return
    }
    drag.lockedArea = null
    let nearest: {
      id: string
      left: number
      top: number
      right: number
      bottom: number
    } | null = null
    let distance = Infinity
    for (const card of space.value.querySelectorAll<HTMLElement>('.project-card')) {
      const id = card.dataset.projectId
      const item = card.parentElement
      if (!id || !item) continue
      // Use the intended layout, independently of the in-flight CSS animation.
      const left = Number(item.dataset.layoutX)
      const top = Number(item.dataset.layoutY)
      const right = left + item.offsetWidth
      const bottom = top + item.offsetHeight
      const dx = Math.max(left - x, 0, x - right)
      const dy = Math.max(top - y, 0, y - bottom)
      const next = dx * dx + dy * dy
      if (next < distance) {
        distance = next
        nearest = { id, left, top, right, bottom }
      }
    }
    if (!nearest || nearest.id === drag.id) return
    const after = y >= (nearest.top + nearest.bottom) / 2
    drag.lockedArea = { ...nearest, after }
    previewPlacement(nearest.id, after)
  }
  function previewPlacement(targetId: string, after: boolean) {
    if (!drag) return
    const id = drag.id
    const current = projects.value.map((project) => project.id)
    const next = current.filter((value) => value !== id)
    const index = next.indexOf(targetId)
    if (index < 0) return
    next.splice(index + Number(after), 0, id)
    if (next.some((id, index) => id !== current[index])) previewOrder.value = next
  }
  function dragFrame() {
    if (!drag || !draggingId.value) return
    if (preview) {
      preview.style.left = `${drag.x - drag.offsetX}px`
      preview.style.top = `${drag.y - drag.offsetY}px`
    }
    const bounds = space.value?.getBoundingClientRect()
    if (bounds && drag.x >= bounds.left && drag.x <= bounds.right) {
      const toolbarBounds = toolbar.value?.getBoundingClientRect()
      const top = Math.max(80, toolbarBounds?.bottom ?? 0)
      const delta = drag.y < top + 36 ? -12 : drag.y > window.innerHeight - 48 ? 12 : 0
      if (delta) window.scrollBy(0, delta)
    }
    updateTarget()
    frame = requestAnimationFrame(dragFrame)
  }
  function startDrag(event: PointerEvent, id: string) {
    if (
      !sorting.value ||
      !event.isPrimary ||
      event.button !== 0 ||
      workspace.sortingProjects ||
      (space.value?.querySelectorAll('.project-card').length ?? 0) < 2
    )
      return
    event.preventDefault()
    cancelDrag()
    const card = event.currentTarget as HTMLElement
    const box = card.getBoundingClientRect()
    card.focus({ preventScroll: true })
    const capture = space.value!
    capture.setPointerCapture(event.pointerId)
    drag = {
      id,
      card,
      capture,
      validDrop: false,
      lockedArea: null,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      x: event.clientX,
      y: event.clientY,
      offsetX: event.clientX - box.left,
      offsetY: event.clientY - box.top,
    }
    window.addEventListener('blur', cancelDrag)
    window.addEventListener('resize', cancelDrag)
  }
  function activateDrag() {
    if (!drag) return
    const box = drag.card.getBoundingClientRect()
    preview = drag.card.cloneNode(true) as HTMLElement
    preview.classList.add('project-card-drag-preview')
    preview.setAttribute('aria-hidden', 'true')
    preview.inert = true
    preview.style.width = `${box.width}px`
    preview.style.left = `${drag.x - drag.offsetX}px`
    preview.style.top = `${drag.y - drag.offsetY}px`
    document.body.append(preview)
    draggingId.value = drag.id
    frame = requestAnimationFrame(dragFrame)
  }
  function moveDrag(event: PointerEvent) {
    if (!drag || event.pointerId !== drag.pointerId) return
    drag.x = event.clientX
    drag.y = event.clientY
    if (!draggingId.value) {
      if (Math.hypot(drag.x - drag.startX, drag.y - drag.startY) < 5) return
      activateDrag()
    }
    event.preventDefault()
  }
  function finishDrag(event: PointerEvent) {
    if (!drag || drag.pointerId !== event.pointerId) return
    drag.x = event.clientX
    drag.y = event.clientY
    updateTarget()
    const id = draggingId.value
    const next = previewOrder.value
    const valid = drag.validDrop
    cancelDrag()
    if (!id || !next || !valid) return
    order.value = next
    mode.value = 'manual'
    error.value = ''
    announcePosition(id)
  }
  function announcePosition(id: string) {
    const index = projects.value.findIndex((project) => project.id === id)
    announcement.value = `已将${projects.value[index]?.name ?? '项目'}移至第 ${index + 1} 位`
  }
  async function moveWithKeyboard(id: string, offset: number, event: KeyboardEvent) {
    if (!sorting.value) return
    event.preventDefault()
    if (workspace.sortingProjects || drag) return
    const next = projects.value.map((project) => project.id)
    const index = next.indexOf(id)
    if (index < 0 || index + offset < 0 || index + offset >= next.length) return
    next.splice(index, 1)
    next.splice(index + offset, 0, id)
    order.value = next
    mode.value = 'manual'
    error.value = ''
    announcePosition(id)
    await nextTick()
    const card = [...(space.value?.querySelectorAll<HTMLElement>('.project-card') ?? [])].find(
      (element) => element.dataset.projectId === id,
    )
    card?.focus({ preventScroll: true })
  }
  function escapeSorting(event: KeyboardEvent) {
    if (event.key !== 'Escape' || !sorting.value || workspace.sortingProjects) return
    event.preventDefault()
    event.stopImmediatePropagation()
    if (drag) cancelDrag()
    else cancelSorting()
  }
  async function saveSorting() {
    if (workspace.sortingProjects || drag) return
    error.value = ''
    try {
      await workspace.saveProjectSort({
        mode: mode.value,
        order: projects.value.map((project) => project.id),
      })
      cancelSorting()
      message.success('项目顺序已保存')
    } catch (cause) {
      error.value = errorMessage(cause)
    }
  }
  watch(sorting, (active) => {
    if (active) document.addEventListener('keydown', escapeSorting, true)
    else document.removeEventListener('keydown', escapeSorting, true)
  })
  watch(() => workspace.status.bindingId, cancelSorting)
  watch(
    () => workspace.status.readOnly,
    (readOnly) => {
      if (readOnly) cancelSorting()
    },
  )
  watch(() => workspace.projects.map((project) => project.id).join(','), cancelDrag)
  onBeforeUnmount(() => {
    cancelDrag()
    document.removeEventListener('keydown', escapeSorting, true)
  })
  return {
    sorting,
    mode,
    projects,
    error,
    announcement,
    draggingId,
    beginSorting,
    cancelSorting,
    changeMode,
    cancelDrag,
    startDrag,
    moveDrag,
    finishDrag,
    moveWithKeyboard,
    saveSorting,
  }
}
