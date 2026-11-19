<script setup lang="ts">
import IconHint from './IconHint.vue'
import { computed, nextTick, onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import { NButton, NInput, NModal, NPopconfirm } from 'naive-ui'
import type { Project, ProjectLink } from '../shared/types'
import {
  defaultLinkName,
  linkIcons,
  normalizeLinkUrl,
  validateProjectLinks,
} from '../shared/project-links'
import { extractProjectLinkIcon, openProjectLink } from '../services/project-links'
import { useWorkspaceStore } from '../stores/workspace'
import { useAppMessage } from '../composables/useAppMessage'
import { errorMessage } from '../shared/domain'
import ProjectLinkIcon from './ProjectLinkIcon.vue'
import AppIcon from './AppIcon.vue'
import EmptyState from './EmptyState.vue'
const props = defineProps<{ project: Project }>()
const workspace = useWorkspaceStore()
const message = useAppMessage()
const pendingLinks = shallowRef<ProjectLink[] | null>(null)
const links = computed(() => pendingLinks.value ?? props.project.links ?? [])
const listElement = ref<HTMLUListElement | null>(null)
const draggingId = ref<string | null>(null)
// undefined means outside the list; null means insert at the end.
const dropBefore = ref<string | null>()
let drag: {
  id: string
  pointerId: number
  handle: HTMLButtonElement
  startX: number
  startY: number
  x: number
  y: number
} | null = null
let scrollFrame = 0
const show = ref(false)
const editingId = ref<string | null>(null)
const name = ref('')
const url = ref('')
const icon = ref<ProjectLink['icon']>('')
const iconData = ref<string>()
const extracting = ref(false)
const iconError = ref('')
let extraction = 0
const busy = ref(false)
const error = ref('')
watch(
  () => props.project.id,
  () => {
    show.value = false
    cancelSort()
    pendingLinks.value = null
  },
)
watch(
  url,
  (next, previous) => {
    let changed = next !== previous
    try {
      changed = normalizeLinkUrl(next) !== normalizeLinkUrl(previous)
    } catch {
      /* incomplete input */
    }
    if (changed) {
      ++extraction
      extracting.value = false
      iconData.value = undefined
      iconError.value = ''
      if (icon.value === 'website') icon.value = ''
    }
  },
  { flush: 'sync' },
)
watch(show, () => {
  ++extraction
  extracting.value = false
})
onBeforeUnmount(() => {
  ++extraction
  cancelSort()
})
async function extractIcon() {
  if (extracting.value || busy.value) return
  if (iconData.value && icon.value !== 'website') {
    icon.value = 'website'
    fillName()
    return
  }
  const request = ++extraction
  extracting.value = true
  iconError.value = ''
  try {
    const result = await extractProjectLinkIcon(url.value)
    if (request !== extraction || !show.value) return
    iconData.value = result
    icon.value = 'website'
    fillName()
  } catch (cause) {
    if (request === extraction && show.value) iconError.value = errorMessage(cause)
  } finally {
    if (request === extraction) extracting.value = false
  }
}
function chooseIcon(value: ProjectLink['icon']) {
  icon.value = value
  fillName()
}
function fillName() {
  if (!name.value.trim()) name.value = defaultLinkName(icon.value, url.value)
}
function normalizeAddress() {
  try {
    url.value = normalizeLinkUrl(url.value)
  } catch {
    /* validate on save */
  }
}
function edit(link?: ProjectLink) {
  editingId.value = link?.id ?? null
  name.value = link?.name ?? ''
  url.value = link?.url ?? ''
  icon.value = link?.icon ?? ''
  iconData.value = link?.iconData
  iconError.value = ''
  error.value = ''
  show.value = true
}
async function persist(next: ProjectLink[]) {
  const { name, description, color } = props.project
  await workspace.updateProject(props.project.id, {
    name,
    description,
    color,
    links: validateProjectLinks(next),
  })
}
async function save() {
  fillName()
  if (busy.value || extracting.value) return
  busy.value = true
  error.value = ''
  try {
    const link: ProjectLink = {
      id: editingId.value ?? crypto.randomUUID(),
      name: name.value,
      url: url.value,
      icon: icon.value,
      ...(icon.value === 'website' ? { iconData: iconData.value } : {}),
    }
    const next = editingId.value
      ? links.value.map((old) => (old.id === editingId.value ? link : old))
      : [...links.value, link]
    await persist(next)
    show.value = false
    message.success(editingId.value ? '链接已保存' : '链接已添加')
  } catch (cause) {
    error.value = errorMessage(cause)
  } finally {
    busy.value = false
  }
}
async function remove(link: ProjectLink) {
  if (busy.value) return false
  busy.value = true
  try {
    await persist(links.value.filter((item) => item.id !== link.id))
    message.success('链接已删除')
  } catch (cause) {
    message.error(errorMessage(cause))
    return false
  } finally {
    busy.value = false
  }
}
function updateDropTarget() {
  const list = listElement.value
  if (!drag || !list) return
  const bounds = list.getBoundingClientRect()
  if (
    drag.x < bounds.left ||
    drag.x > bounds.right ||
    drag.y < Math.max(0, bounds.top - 48) ||
    drag.y > Math.min(window.innerHeight, bounds.bottom + 48)
  ) {
    dropBefore.value = undefined
    return
  }
  const rows = [...list.querySelectorAll<HTMLElement>('.project-link-row')]
  dropBefore.value =
    rows.find((row) => {
      if (row.dataset.linkId === drag!.id) return false
      const box = row.getBoundingClientRect()
      return drag!.y < box.top + box.height / 2
    })?.dataset.linkId ?? null
}
function autoScroll() {
  if (!drag || !draggingId.value) return
  const list = listElement.value?.getBoundingClientRect()
  if (list && drag.x >= list.left && drag.x <= list.right) {
    const top = document.querySelector('.topbar')?.getBoundingClientRect().bottom ?? 0
    const edge = 48
    const delta =
      drag.y < top + edge
        ? -Math.min(16, (top + edge - drag.y) / 3)
        : drag.y > window.innerHeight - edge
          ? Math.min(16, (drag.y - window.innerHeight + edge) / 3)
          : 0
    if (delta && document.scrollingElement) document.scrollingElement.scrollTop += delta
  }
  updateDropTarget()
  scrollFrame = requestAnimationFrame(autoScroll)
}
function cancelSort() {
  const previous = drag
  drag = null
  draggingId.value = null
  dropBefore.value = undefined
  cancelAnimationFrame(scrollFrame)
  document.removeEventListener('keydown', escapeSort, true)
  window.removeEventListener('blur', cancelSort)
  if (previous?.handle.hasPointerCapture(previous.pointerId))
    previous.handle.releasePointerCapture(previous.pointerId)
}
function escapeSort(event: KeyboardEvent) {
  if (event.key !== 'Escape') return
  event.preventDefault()
  event.stopImmediatePropagation()
  cancelSort()
}
function startSort(event: PointerEvent, link: ProjectLink) {
  if (!event.isPrimary || event.button !== 0 || busy.value || show.value || links.value.length < 2)
    return
  event.preventDefault()
  cancelSort()
  const handle = event.currentTarget as HTMLButtonElement
  handle.focus({ preventScroll: true })
  handle.setPointerCapture(event.pointerId)
  drag = {
    id: link.id,
    pointerId: event.pointerId,
    handle,
    startX: event.clientX,
    startY: event.clientY,
    x: event.clientX,
    y: event.clientY,
  }
  document.addEventListener('keydown', escapeSort, true)
  window.addEventListener('blur', cancelSort)
}
function moveSort(event: PointerEvent) {
  if (!drag || drag.pointerId !== event.pointerId) return
  drag.x = event.clientX
  drag.y = event.clientY
  if (!draggingId.value) {
    if (Math.hypot(drag.x - drag.startX, drag.y - drag.startY) < 5) return
    draggingId.value = drag.id
    scrollFrame = requestAnimationFrame(autoScroll)
  }
  event.preventDefault()
  updateDropTarget()
}
function finishSort(event: PointerEvent) {
  if (!drag || drag.pointerId !== event.pointerId) return
  updateDropTarget()
  const id = draggingId.value
  const before = dropBefore.value
  cancelSort()
  if (!id || before === undefined) return
  const item = links.value.find((link) => link.id === id)
  if (!item) return
  const next = links.value.filter((link) => link.id !== id)
  const index = before === null ? next.length : next.findIndex((link) => link.id === before)
  if (index < 0) return
  next.splice(index, 0, item)
  void saveOrder(next)
}
async function saveOrder(next: ProjectLink[], focusId?: string) {
  if (busy.value || next.every((link, index) => link.id === links.value[index]?.id)) return
  const projectId = props.project.id
  pendingLinks.value = next
  busy.value = true
  try {
    await persist(next)
    if (props.project.id === projectId) message.success('链接顺序已保存')
  } catch (cause) {
    if (props.project.id === projectId) message.error(errorMessage(cause))
  } finally {
    pendingLinks.value = null
    busy.value = false
    if (focusId && props.project.id === projectId) {
      await nextTick()
      const row = [
        ...(listElement.value?.querySelectorAll<HTMLElement>('.project-link-row') ?? []),
      ].find((item) => item.dataset.linkId === focusId)
      row
        ?.querySelector<HTMLButtonElement>('.project-link-sort-handle')
        ?.focus({ preventScroll: true })
    }
  }
}
function moveWithKeyboard(id: string, offset: number) {
  if (drag || busy.value || show.value) return
  const next = [...links.value]
  const index = next.findIndex((link) => link.id === id)
  const target = index + offset
  if (index < 0 || target < 0 || target >= next.length) return
  const [item] = next.splice(index, 1)
  next.splice(target, 0, item!)
  void saveOrder(next, id)
}
async function open(link: ProjectLink) {
  try {
    await openProjectLink(link.url)
  } catch (cause) {
    message.error(errorMessage(cause))
  }
}
</script>
<template>
  <section class="panel project-links-panel" aria-label="项目链接列表">
    <div class="panel-heading">
      <h2>项目链接</h2>
      <NButton type="primary" :disabled="busy || !!draggingId || links.length >= 50" @click="edit()"
        ><AppIcon name="plus" :size="17" />添加链接</NButton
      >
    </div>
    <ul
      v-if="links.length"
      ref="listElement"
      class="project-links-list"
      :class="{ 'is-sorting': draggingId, 'drop-at-end': draggingId && dropBefore === null }"
      :aria-busy="busy"
      @dragstart.prevent
    >
      <li
        v-for="link in links"
        :key="link.id"
        class="project-link-row"
        :data-link-id="link.id"
        :class="{
          'is-dragging': draggingId === link.id,
          'drop-before': draggingId && dropBefore === link.id,
        }"
      >
        <button
          type="button"
          class="icon-button project-link-sort-handle"
          :aria-label="`拖拽排序 ${link.name}`"
          aria-keyshortcuts="ArrowUp ArrowDown"
          :disabled="busy || links.length < 2"
          @pointerdown="startSort($event, link)"
          @pointermove="moveSort"
          @pointerup="finishSort"
          @pointercancel="cancelSort"
          @lostpointercapture="cancelSort"
          @keydown.up.prevent="moveWithKeyboard(link.id, -1)"
          @keydown.down.prevent="moveWithKeyboard(link.id, 1)"
        >
          <AppIcon name="drag" :size="18" />
        </button>
        <button
          class="project-link-entry"
          :aria-label="`打开链接 ${link.name}`"
          :title="link.url"
          @click="open(link)"
        >
          <ProjectLinkIcon :link="link" /><span class="project-link-info"
            ><strong>{{ link.name }}</strong
            ><small>{{ link.url }}</small></span
          ><AppIcon name="external" :size="17" />
        </button>
        <IconHint text="编辑"
          ><button
            class="icon-button"
            :disabled="busy || !!draggingId"
            :aria-label="`编辑链接 ${link.name}`"
            @click="edit(link)"
          >
            <AppIcon name="edit" :size="18" /></button
        ></IconHint>
        <NPopconfirm
          :show-icon="false"
          :positive-button-props="{ type: 'error', disabled: busy }"
          @positive-click="remove(link)"
          ><template #trigger
            ><span class="icon-hint-anchor"
              ><IconHint text="删除"
                ><button
                  class="icon-button danger-text"
                  :disabled="busy || !!draggingId"
                  :aria-label="`删除链接 ${link.name}`"
                >
                  <AppIcon name="delete" :size="18" /></button></IconHint></span></template
          >删除“{{ link.name }}”？</NPopconfirm
        >
      </li>
    </ul>
    <EmptyState
      v-else
      kind="links"
      title="暂无链接"
      action-label="点击新增"
      :disabled="busy || !!workspace.status.readOnly"
      @action="edit()"
    />
  </section>
  <NModal
    v-model:show="show"
    preset="card"
    :title="editingId ? '编辑链接' : '添加链接'"
    :aria-label="editingId ? '编辑链接' : '添加链接'"
    class="form-modal"
    :bordered="false"
    :closable="false"
    :mask-closable="false"
    :close-on-esc="!busy"
  >
    <template #header-extra
      ><button class="icon-button" aria-label="关闭链接弹窗" :disabled="busy" @click="show = false">
        <AppIcon name="close" /></button
    ></template>
    <form class="dialog-form" @submit.prevent="save">
      <label id="project-link-icon-label">图标</label>
      <div class="link-icon-picker" role="radiogroup" aria-labelledby="project-link-icon-label">
        <IconHint :text="option.label" v-for="option in linkIcons" :key="option.value"
          ><button
            type="button"
            role="radio"
            class="link-icon-choice"
            :class="{ selected: icon === option.value }"
            :aria-checked="icon === option.value"
            :aria-label="option.label"
            :disabled="busy || extracting"
            @click="chooseIcon(option.value)"
          >
            <span v-if="!option.value" class="link-icon-default">默认</span>
            <ProjectLinkIcon v-else :link="{ name, icon: option.value }" /></button
        ></IconHint>
        <IconHint :text="extracting ? '正在提取网站图标' : '提取网站图标'"
          ><button
            type="button"
            role="radio"
            class="link-icon-choice"
            :class="{ selected: icon === 'website', 'is-extracting': extracting }"
            :aria-checked="icon === 'website'"
            :aria-label="extracting ? '正在提取网站图标' : '提取网站图标'"
            :disabled="busy || extracting || !url.trim()"
            @click="extractIcon"
          >
            <ProjectLinkIcon
              v-if="iconData && !extracting"
              :link="{ name, icon: 'website', iconData }"
            />
            <AppIcon v-else name="refresh" :size="21" /></button
        ></IconHint>
      </div>
      <p v-if="iconError" class="form-error" role="status">{{ iconError }}</p>
      <label for="project-link-name">链接名称</label
      ><NInput
        v-model:value="name"
        :input-props="{ id: 'project-link-name' }"
        maxlength="80"
        placeholder="例如：测试环境"
        :disabled="busy"
        autofocus
      />
      <label for="project-link-url">URL</label
      ><NInput
        v-model:value="url"
        :input-props="{ id: 'project-link-url' }"
        maxlength="2048"
        placeholder="例如：example.com"
        :disabled="busy"
        @blur="normalizeAddress"
      />
      <p v-if="error" class="form-error" role="alert">{{ error }}</p>
      <div class="dialog-footer">
        <NButton :disabled="busy" @click="show = false">取消</NButton
        ><NButton type="primary" attr-type="submit" :loading="busy" :disabled="extracting"
          >保存链接</NButton
        >
      </div>
    </form>
  </NModal>
</template>
