<script setup lang="ts">
import IconHint from './IconHint.vue'
import { computed, h, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { NButton, NDropdown, NPopconfirm } from 'naive-ui'
import type { Task } from '../shared/types'
import { statusLabels } from '../shared/types'
import { errorMessage } from '../shared/domain'
import { useWorkspaceStore } from '../stores/workspace'
import { useUiStore } from '../stores/ui'
import { useAppMessage } from '../composables/useAppMessage'
import AppIcon from './AppIcon.vue'
import TaskMasonry from './TaskMasonry.vue'
import TaskRow from './TaskRow.vue'

const props = defineProps<{ tasks: Task[]; layout?: 'masonry' | 'list' }>()
const ui = useUiStore()
const workspace = useWorkspaceStore()
const message = useAppMessage()
const pendingIds = ref(new Set<string>())
const route = useRoute()
const contextTask = ref<Task | null>(null)
const contextVisible = ref(false)
const contextVersion = ref(0)
const contextPosition = ref({ x: 0, y: 0 })
let contextTrigger: HTMLElement | null = null
let mounted = true
const contextOptions = computed(() => [
  { label: '编辑待办', key: 'edit', disabled: !!workspace.status.readOnly },
  { label: '删除待办', key: 'delete', disabled: !!workspace.status.readOnly },
])
function closeContextMenu() {
  contextVisible.value = false
  contextVersion.value++
}
function closeContextMenuOnScroll() {
  if (contextVisible.value) closeContextMenu()
}
async function showContextMenu(event: MouseEvent | KeyboardEvent, task: Task) {
  event.preventDefault()
  event.stopPropagation()
  closeContextMenu()
  if (task.deletedAt || pendingIds.value.has(task.id)) return
  contextTask.value = task
  const card = event.currentTarget as HTMLElement
  contextTrigger =
    (event.target as HTMLElement).closest<HTMLElement>('button,a') ??
    card.querySelector<HTMLElement>('.task-title')
  const trigger = contextTrigger ?? card
  const version = contextVersion.value
  await nextTick()
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
  if (!mounted || version !== contextVersion.value) return
  const bounds = trigger.getBoundingClientRect()
  contextPosition.value =
    event instanceof MouseEvent && event.button === 2
      ? { x: event.clientX, y: event.clientY }
      : {
          x: Math.max(12, Math.min(bounds.left + 24, window.innerWidth - 160)),
          y: Math.max(12, Math.min(bounds.top + 24, window.innerHeight - 100)),
        }
  contextVisible.value = true
}
function selectContextAction(key: string) {
  const task = props.tasks.find((item) => item.id === contextTask.value?.id)
  closeContextMenu()
  if (task) void action(task, key)
}
function contextKeydown(event: KeyboardEvent) {
  if (event.key !== 'Escape' || !contextVisible.value) return
  event.preventDefault()
  event.stopImmediatePropagation()
  closeContextMenu()
  contextTrigger?.focus({ preventScroll: true })
}
watch(
  [() => route.fullPath, () => ui.taskDialog, () => ui.search, () => props.tasks],
  closeContextMenu,
)
onMounted(() => {
  document.addEventListener('contextmenu', closeContextMenu, true)
  document.addEventListener('scroll', closeContextMenuOnScroll, true)
  document.addEventListener('keydown', contextKeydown, true)
  window.addEventListener('resize', closeContextMenu)
  window.addEventListener('blur', closeContextMenu)
})
onBeforeUnmount(() => {
  mounted = false
  closeContextMenu()
  document.removeEventListener('contextmenu', closeContextMenu, true)
  document.removeEventListener('scroll', closeContextMenuOnScroll, true)
  document.removeEventListener('keydown', contextKeydown, true)
  window.removeEventListener('resize', closeContextMenu)
  window.removeEventListener('blur', closeContextMenu)
})
async function toggle(task: Task) {
  if (pendingIds.value.has(task.id)) return
  pendingIds.value.add(task.id)
  try {
    await workspace.toggleTask(task)
  } catch (cause) {
    message.error(errorMessage(cause))
  } finally {
    pendingIds.value.delete(task.id)
  }
}
async function action(task: Task, key: string) {
  if (task.deletedAt || workspace.status.readOnly || pendingIds.value.has(task.id)) return
  if (key === 'edit') return ui.editTask(task)
  if (key !== 'delete') return
  pendingIds.value.add(task.id)
  try {
    await workspace.trashTask(task.id)
    let restoring = false
    const toast = message.success(
      () =>
        h('span', { class: 'undo-message' }, [
          '待办已删除',
          h(
            'button',
            {
              class: 'text-link',
              onClick: async () => {
                if (restoring) return
                restoring = true
                try {
                  const inactive =
                    !!task.projectId &&
                    workspace.projects.find((p) => p.id === task.projectId)?.status !== 'active'
                  await workspace.trashTask(task.id, true, inactive)
                  toast.destroy()
                } catch (e) {
                  message.error(errorMessage(e))
                  restoring = false
                }
              },
            },
            '撤销',
          ),
        ]),
      { duration: 6000 },
    )
  } catch (cause) {
    message.error(errorMessage(cause))
  } finally {
    pendingIds.value.delete(task.id)
  }
}
function projectName(task: Task) {
  return workspace.projects.find((p) => p.id === task.projectId)?.name || ''
}
function dueLabel(task: Task) {
  if (!task.dueAt) return ''
  return new Date(task.dueAt).toLocaleString('zh-CN', {
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
}
function overdue(task: Task) {
  return (
    !!task.dueAt &&
    new Date(task.dueAt).getTime() < workspace.now &&
    ['todo', 'doing'].includes(task.status)
  )
}
</script>
<template>
  <ul v-if="layout === 'list'" class="task-list">
    <TaskRow
      v-for="task in tasks"
      :key="task.id"
      :task="task"
      :pending="pendingIds.has(task.id)"
      :project-name="projectName(task) || '无'"
      :due-label="dueLabel(task)"
      :overdue="overdue(task)"
      :needs-restore-confirmation="
        !!task.projectId &&
        workspace.projects.find((p) => p.id === task.projectId)?.status !== 'active' &&
        ['todo', 'doing'].includes(task.status)
      "
      @mousedown.right.prevent
      @contextmenu="showContextMenu($event, task)"
      @keydown.shift.f10.prevent.stop="showContextMenu($event, task)"
      @keydown="$event.code === 'ContextMenu' && showContextMenu($event, task)"
      @toggle="toggle(task)"
      @action="(key) => action(task, key)"
      @restore="
        (toInbox) =>
          workspace.trashTask(task.id, true, toInbox).catch((e) => message.error(errorMessage(e)))
      "
    />
  </ul>
  <TaskMasonry v-else :tasks="tasks" v-slot="{ task }">
    <article
      class="task-row task-card"
      @mousedown.right.prevent
      @contextmenu="showContextMenu($event, task)"
      @keydown.shift.f10.prevent.stop="showContextMenu($event, task)"
      @keydown="$event.code === 'ContextMenu' && showContextMenu($event, task)"
      :class="[`task-priority-${task.priority}`, { 'is-pending': pendingIds.has(task.id) }]"
    >
      <div class="task-card-header">
        <IconHint v-if="!task.deletedAt" :text="task.status === 'done' ? '重新打开' : '完成'">
          <button
            class="task-checkbox"
            :class="{ checked: task.status === 'done' }"
            :disabled="pendingIds.has(task.id) || task.status === 'canceled'"
            :aria-label="task.status === 'done' ? `重新打开 ${task.title}` : `完成 ${task.title}`"
            @click="toggle(task)"
          >
            <AppIcon v-if="task.status === 'done'" name="check" :size="13" />
          </button>
        </IconHint>
        <button
          class="text-button task-title"
          :class="{ 'task-done': task.status === 'done' }"
          :disabled="!!task.deletedAt"
          :aria-label="`编辑 ${task.title}`"
          @click="action(task, 'edit')"
        >
          {{ task.title }}
        </button>
        <NDropdown
          v-if="!task.deletedAt"
          trigger="click"
          :options="[
            { label: '编辑待办', key: 'edit' },
            { label: '删除待办', key: 'delete' },
          ]"
          @select="(key) => action(task, key)"
        >
          <button
            class="icon-button task-more"
            :aria-label="`更多操作 ${task.title}`"
            :disabled="pendingIds.has(task.id)"
          >
            <AppIcon name="more" :size="20" />
          </button>
        </NDropdown>
      </div>
      <p v-if="task.description?.trim()" class="task-card-description">{{ task.description }}</p>
      <div v-if="projectName(task) || task.planDate || task.dueAt" class="task-metadata">
        <span v-if="projectName(task)" class="task-project-label" :title="projectName(task)">
          <AppIcon name="folder" :size="13" />{{ projectName(task) }}
        </span>
        <span v-if="task.planDate" class="task-date"
          ><AppIcon name="calendar" :size="13" />计划 {{ task.planDate }}</span
        >
        <span v-if="task.dueAt" class="task-date" :class="{ 'is-overdue': overdue(task) }">
          <AppIcon name="clock" :size="13" />{{ overdue(task) ? '已逾期' : '截止' }}
          {{ dueLabel(task) }}
        </span>
      </div>
      <div v-if="task.status !== 'todo' || task.deletedAt" class="task-card-footer">
        <div class="task-indicators">
          <span v-if="task.status !== 'todo'" class="tag">{{ statusLabels[task.status] }}</span>
        </div>
        <NPopconfirm
          v-if="
            task.deletedAt &&
            task.projectId &&
            workspace.projects.find((p) => p.id === task.projectId)?.status !== 'active' &&
            ['todo', 'doing'].includes(task.status)
          "
          :show-icon="false"
          @positive-click="
            workspace.trashTask(task.id, true, true).catch((e) => message.error(errorMessage(e)))
          "
        >
          <template #trigger><NButton size="small">恢复</NButton></template
          >原项目已归档，恢复为无项目？附件将复制，原文件保留。
        </NPopconfirm>
        <NButton
          v-else-if="task.deletedAt"
          size="small"
          @click="workspace.trashTask(task.id, true).catch((e) => message.error(errorMessage(e)))"
          >恢复</NButton
        >
      </div>
    </article>
  </TaskMasonry>
  <NDropdown
    :key="contextVersion"
    trigger="manual"
    :show="contextVisible"
    :x="contextPosition.x"
    :y="contextPosition.y"
    :options="contextOptions"
    :menu-props="() => ({ role: 'menu', 'aria-label': '待办操作' })"
    :node-props="
      (option) => ({
        role: 'menuitem',
        'aria-disabled': option.disabled ? ('true' as const) : ('false' as const),
      })
    "
    placement="bottom-start"
    @select="selectContextAction"
    @clickoutside="closeContextMenu"
    @update:show="contextVisible = $event"
  />
</template>
