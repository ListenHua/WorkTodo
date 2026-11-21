<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { NButton, NCheckbox, NDropdown, NModal, NSelect, NSkeleton } from 'naive-ui'
import { useWorkspaceStore } from '../stores/workspace'
import { useUiStore } from '../stores/ui'
import { useAppMessage } from '../composables/useAppMessage'
import { useModalState } from '../composables/useModalState'
import { errorMessage, localDate } from '../shared/domain'
import type { Task, TrashRequest } from '../shared/types'
import AppIcon from '../components/AppIcon.vue'
import EmptyState from '../components/EmptyState.vue'
import TaskMasonry from '../components/TaskMasonry.vue'

const workspace = useWorkspaceStore()
const ui = useUiStore()
const message = useAppMessage()
const filters = reactive({ project: '', date: 'all', sort: 'newest' })
const trash = computed(() => workspace.tasks.filter((task) => !!task.deletedAt))
const hasFilters = computed(() => !!ui.search.trim() || !!filters.project || filters.date !== 'all')
const filtered = computed(() => {
  const today = new Date(`${workspace.currentDay}T00:00:00`)
  const cutoff = new Date(today)
  cutoff.setDate(
    cutoff.getDate() - (filters.date === 'week' ? 6 : filters.date === 'month' ? 29 : 0),
  )
  const tomorrow = new Date(today)
  tomorrow.setDate(tomorrow.getDate() + 1)
  const query = ui.search.trim().toLocaleLowerCase()
  return trash.value
    .filter((task) => {
      if (
        filters.project === 'none'
          ? !!task.projectId
          : filters.project && task.projectId !== filters.project
      )
        return false
      const deleted = new Date(task.deletedAt!).getTime()
      if (filters.date !== 'all' && !(deleted >= cutoff.getTime() && deleted < tomorrow.getTime()))
        return false
      return `${task.title} ${task.description || ''} ${projectName(task)}`
        .toLocaleLowerCase()
        .includes(query)
    })
    .sort((a, b) => {
      if (filters.sort === 'title')
        return a.title.localeCompare(b.title, 'zh-CN') || a.id.localeCompare(b.id)
      const time = new Date(b.deletedAt!).getTime() - new Date(a.deletedAt!).getTime()
      return (filters.sort === 'oldest' ? -time : time) || a.id.localeCompare(b.id)
    })
})
const initialLoading = computed(() => workspace.loading && !workspace.tasks.length)
const readonly = computed(() => !!workspace.status.readOnly || !workspace.status.available)
const selecting = ref(false)
const selected = ref(new Set<string>())
const allSelected = computed(
  () => !!filtered.value.length && filtered.value.every((task) => selected.value.has(task.id)),
)
const selectedTasks = computed(() => filtered.value.filter((task) => selected.value.has(task.id)))
const busy = ref(false)
const {
  show: confirmationVisible,
  data: confirmation,
  open: openConfirmation,
  close: closeConfirmation,
  afterLeave: clearConfirmation,
} = useModalState<(TrashRequest & { title: string; description: string; button: string }) | null>(
  () => null,
)
const operationError = ref('')
const failures = ref<{ title: string; message: string }[]>([])
const failureSummary = ref('')
const context = ref<{ show: boolean; x: number; y: number; task: Task | null }>({
  show: false,
  x: 0,
  y: 0,
  task: null,
})
const contextOptions = computed(() => [
  { label: '恢复', key: 'restore', disabled: readonly.value || busy.value },
  { label: '彻底删除', key: 'purge', disabled: readonly.value || busy.value },
])

function projectName(task: Task) {
  return workspace.projects.find((project) => project.id === task.projectId)?.name || '无所属项目'
}
function deletedLabel(task: Task) {
  const date = new Date(task.deletedAt!)
  if (!Number.isFinite(date.getTime())) return '删除时间未知'
  return `${localDate(date)} ${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`
}
function needsInbox(task: Task) {
  return (
    !!task.projectId &&
    ['todo', 'doing'].includes(task.status) &&
    workspace.projects.some(
      (project) => project.id === task.projectId && project.status !== 'active',
    )
  )
}
function resetFilters() {
  ui.search = ''
  Object.assign(filters, { project: '', date: 'all', sort: 'newest' })
}
function toggleSelection(id: string) {
  if (busy.value || readonly.value) return
  const next = new Set(selected.value)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  selected.value = next
}
function selectAll(checked: boolean) {
  selected.value = checked ? new Set(filtered.value.map((task) => task.id)) : new Set()
}
function exitSelection() {
  if (busy.value) return
  selecting.value = false
  selected.value = new Set()
}
function closeContext() {
  context.value.show = false
}
async function showContext(event: MouseEvent | KeyboardEvent, task: Task) {
  event.preventDefault()
  event.stopPropagation()
  if (selecting.value || busy.value) return
  closeContext()
  const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect()
  const x = event instanceof MouseEvent && event.button === 2 ? event.clientX : bounds.left + 24
  const y = event instanceof MouseEvent && event.button === 2 ? event.clientY : bounds.top + 24
  await nextTick()
  context.value = { show: true, x, y, task }
}
function contextAction(key: string) {
  const task = trash.value.find((task) => task.id === context.value.task?.id)
  closeContext()
  if (task && (key === 'restore' || key === 'purge')) requestAction(key, [task])
}
function requestAction(action: TrashRequest['action'], tasks: Task[], clear = false) {
  if (!tasks.length || busy.value || readonly.value) return
  closeContext()
  operationError.value = ''
  const rehome = action === 'restore' ? tasks.filter(needsInbox).length : 0
  const input: TrashRequest = {
    ids: tasks.map((task) => task.id),
    action,
    bindingId: workspace.status.bindingId || 'unbound',
    restoreToInbox: rehome > 0,
  }
  if (action === 'restore' && !rehome) {
    void execute(input)
    return
  }
  openConfirmation({
    ...input,
    title: action === 'restore' ? '恢复待办' : clear ? '清空回收站' : '彻底删除待办',
    button: action === 'restore' ? '恢复' : clear ? '清空回收站' : '彻底删除',
    description:
      action === 'restore'
        ? `将恢复 ${tasks.length} 项待办，其中 ${rehome} 项的原项目已归档或结束，将恢复为无所属项目。附件将复制，原文件保留。`
        : clear
          ? `清空回收站中的全部 ${tasks.length} 项待办？${hasFilters.value ? '此操作包含当前筛选之外的待办。' : ''}删除后无法恢复，本地附件文件将保留。`
          : tasks.length === 1
            ? `彻底删除“${tasks[0]!.title}”？删除后无法恢复，本地附件文件将保留。`
            : `彻底删除选中的 ${tasks.length} 项待办？删除后无法恢复，本地附件文件将保留。`,
  })
}
async function execute(input: TrashRequest) {
  if (busy.value || readonly.value) return
  busy.value = true
  operationError.value = ''
  failures.value = []
  failureSummary.value = ''
  const names = new Map(trash.value.map((task) => [task.id, task.title]))
  try {
    const result = await workspace.manageTrash(input)
    if ((workspace.status.bindingId || 'unbound') !== input.bindingId) return
    closeConfirmation()
    const verb = input.action === 'restore' ? '恢复' : '彻底删除'
    if (result.failed.length) {
      failures.value = result.failed.map((failure) => ({
        title: names.get(failure.id) || '待办',
        message: failure.message,
      }))
      failureSummary.value = `已${verb} ${result.succeededIds.length} 项，${result.failed.length} 项未处理。`
    } else message.success(`已${verb} ${result.succeededIds.length} 项待办`)
    if (!trash.value.length) {
      selecting.value = false
      selected.value = new Set()
    }
  } catch (cause) {
    operationError.value = errorMessage(cause)
  } finally {
    busy.value = false
  }
}
watch(filtered, (tasks) => {
  const visible = new Set(tasks.map((task) => task.id))
  selected.value = new Set([...selected.value].filter((id) => visible.has(id)))
  closeContext()
})
watch(
  () => workspace.status.bindingId,
  () => {
    closeConfirmation()
    selecting.value = false
    selected.value = new Set()
    operationError.value = ''
    failures.value = []
    failureSummary.value = ''
    closeContext()
  },
)
watch(readonly, (value) => {
  if (value && !busy.value) {
    closeConfirmation()
    exitSelection()
  }
})
onMounted(() => {
  document.addEventListener('scroll', closeContext, true)
  window.addEventListener('resize', closeContext)
  window.addEventListener('blur', closeContext)
})
onBeforeUnmount(() => {
  document.removeEventListener('scroll', closeContext, true)
  window.removeEventListener('resize', closeContext)
  window.removeEventListener('blur', closeContext)
})
</script>

<template>
  <section class="trash-page" aria-label="回收站" :aria-busy="busy">
    <div class="trash-heading">
      <span class="muted" role="status">{{
        hasFilters ? `当前 ${filtered.length} 项，共 ${trash.length} 项` : `共 ${trash.length} 项`
      }}</span>
      <div class="trash-actions">
        <NButton
          :disabled="readonly || busy || !trash.length || selecting"
          @click="selecting = true"
          >批量管理</NButton
        >
        <NButton
          type="error"
          :disabled="readonly || busy || !trash.length"
          @click="requestAction('purge', trash, true)"
          >清空回收站</NButton
        >
      </div>
    </div>
    <Teleport v-if="selecting" to="#page-toolbar">
      <div class="trash-selection-toolbar" role="region" aria-label="回收站批量管理">
        <NCheckbox
          :checked="allSelected"
          :indeterminate="!!selected.size && !allSelected"
          :disabled="busy || readonly || !filtered.length"
          @update:checked="selectAll"
          >全选当前结果</NCheckbox
        >
        <span class="muted" role="status">已选 {{ selected.size }} 项</span>
        <div class="trash-actions">
          <NButton
            type="success"
            :disabled="busy || readonly || !selected.size"
            @click="requestAction('restore', selectedTasks)"
            >恢复</NButton
          >
          <NButton
            type="error"
            :disabled="busy || readonly || !selected.size"
            @click="requestAction('purge', selectedTasks)"
            >彻底删除</NButton
          >
          <NButton :disabled="busy" @click="exitSelection">退出</NButton>
        </div>
      </div>
    </Teleport>
    <div class="list-toolbar trash-filters">
      <NSelect
        v-model:value="filters.project"
        aria-label="项目筛选"
        :disabled="busy"
        :options="[
          { label: '全部项目', value: '' },
          { label: '无所属项目', value: 'none' },
          ...workspace.projects.map((project) => ({ label: project.name, value: project.id })),
        ]"
      >
        <template #arrow><AppIcon name="down" :size="16" /></template>
      </NSelect>
      <NSelect
        v-model:value="filters.date"
        aria-label="删除时间筛选"
        :disabled="busy"
        :options="[
          { label: '全部删除时间', value: 'all' },
          { label: '今天删除', value: 'today' },
          { label: '最近 7 天', value: 'week' },
          { label: '最近 30 天', value: 'month' },
        ]"
      >
        <template #arrow><AppIcon name="down" :size="16" /></template>
      </NSelect>
      <NSelect
        v-model:value="filters.sort"
        aria-label="回收站排序"
        :disabled="busy"
        :options="[
          { label: '最近删除', value: 'newest' },
          { label: '最早删除', value: 'oldest' },
          { label: '标题', value: 'title' },
        ]"
      >
        <template #arrow><AppIcon name="down" :size="16" /></template>
      </NSelect>
      <NButton v-if="hasFilters" text :disabled="busy" @click="resetFilters">重置</NButton>
    </div>
    <p v-if="operationError && !confirmation" class="form-error" role="alert">
      {{ operationError }}
    </p>
    <div v-if="failures.length" class="trash-failures" role="alert">
      <p>{{ failureSummary }}</p>
      <ul>
        <li v-for="(failure, index) in failures" :key="index">
          {{ failure.title }}：{{ failure.message }}
        </li>
      </ul>
    </div>
    <div v-if="initialLoading" class="list-skeleton" aria-label="正在加载回收站">
      <NSkeleton v-for="n in 4" :key="n" text :height="44" />
    </div>
    <TaskMasonry v-else-if="filtered.length" :tasks="filtered" v-slot="{ task }">
      <article
        class="task-card task-row trash-card"
        :class="{ 'is-selected': selected.has(task.id), 'is-selecting': selecting }"
        :aria-label="task.title"
        tabindex="0"
        @click="selecting && toggleSelection(task.id)"
        @mousedown.right.prevent
        @contextmenu="showContext($event, task)"
        @keydown.shift.f10.prevent.stop="showContext($event, task)"
        @keydown="$event.code === 'ContextMenu' && showContext($event, task)"
      >
        <div class="task-card-header">
          <NCheckbox
            v-if="selecting"
            :checked="selected.has(task.id)"
            :disabled="busy || readonly"
            :aria-label="`选择 ${task.title}`"
            @click.stop
            @update:checked="toggleSelection(task.id)"
          />
          <h3 class="trash-title">{{ task.title }}</h3>
          <NDropdown
            v-if="!selecting"
            trigger="click"
            :options="contextOptions"
            @select="(key) => requestAction(key as 'restore' | 'purge', [task])"
          >
            <button
              type="button"
              class="icon-button task-more"
              :aria-label="`更多操作 ${task.title}`"
              :disabled="busy || readonly"
            >
              <AppIcon name="more" :size="20" />
            </button>
          </NDropdown>
        </div>
        <p v-if="task.description?.trim()" class="task-card-description trash-description">
          {{ task.description }}
        </p>
        <div class="task-metadata">
          <span><AppIcon name="folder" :size="13" />{{ projectName(task) }}</span>
          <span><AppIcon name="clock" :size="13" />删除于 {{ deletedLabel(task) }}</span>
        </div>
        <div v-if="!selecting" class="trash-card-actions">
          <NButton
            size="small"
            type="success"
            :disabled="busy || readonly"
            @click="requestAction('restore', [task])"
            >恢复</NButton
          >
          <NButton
            size="small"
            type="error"
            :disabled="busy || readonly"
            @click="requestAction('purge', [task])"
            >彻底删除</NButton
          >
        </div>
      </article>
    </TaskMasonry>
    <EmptyState
      v-else
      :kind="hasFilters ? 'search' : 'tasks'"
      :title="hasFilters ? '没有匹配的回收站内容' : '回收站为空'"
      :action-label="hasFilters ? '点击清除筛选' : '点击查看待办'"
      :action-to="hasFilters ? undefined : '/tasks'"
      @action="resetFilters"
    />
    <NDropdown
      trigger="manual"
      :show="context.show"
      :x="context.x"
      :y="context.y"
      :options="contextOptions"
      :menu-props="() => ({ role: 'menu', 'aria-label': '回收站待办操作' })"
      :node-props="() => ({ role: 'menuitem' })"
      placement="bottom-start"
      @select="contextAction"
      @clickoutside="closeContext"
      @update:show="context.show = $event"
    />
    <NModal
      :show="confirmationVisible"
      @after-leave="clearConfirmation"
      preset="card"
      :title="confirmation?.title"
      style="width: min(440px, calc(100vw - 32px))"
      :closable="!busy"
      :mask-closable="!busy"
      :close-on-esc="!busy"
      @update:show="
        (show) => {
          if (!show && !busy) closeConfirmation()
        }
      "
    >
      <p class="trash-confirmation">{{ confirmation?.description }}</p>
      <p v-if="operationError" class="form-error" role="alert">{{ operationError }}</p>
      <template #action>
        <NButton :disabled="busy" @click="closeConfirmation">取消</NButton>
        <NButton
          :type="confirmation?.action === 'restore' ? 'success' : 'error'"
          :loading="busy"
          :disabled="readonly"
          @click="confirmationVisible && confirmation && execute(confirmation)"
          >{{ confirmation?.button }}</NButton
        >
      </template>
    </NModal>
  </section>
</template>

<style scoped>
.trash-heading,
.trash-actions,
.trash-selection-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
}
.trash-heading {
  justify-content: space-between;
  margin-bottom: 20px;
  font-size: 12px;
}
.trash-actions {
  flex-wrap: wrap;
}
.trash-selection-toolbar {
  flex-wrap: wrap;
  padding: 12px 24px;
  border-block: 1px solid var(--border);
  background: var(--surface);
  font-size: 12px;
}
.trash-selection-toolbar .trash-actions {
  margin-left: auto;
}
.trash-filters {
  flex-wrap: wrap;
  gap: 12px;
  padding: 0 0 24px;
}
.trash-filters > :deep(.n-select) {
  width: 150px;
}
.trash-card.is-selecting {
  cursor: pointer;
}
.trash-card.is-selected {
  box-shadow: inset 0 0 0 2px var(--accent);
}
.trash-title {
  flex: 1;
  min-width: 0;
  margin: 0;
  color: var(--text);
  font-size: 14px;
  font-weight: 500;
  line-height: 1.65;
  overflow-wrap: anywhere;
}
.trash-description {
  display: -webkit-box;
  overflow: hidden;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}
.trash-card .task-metadata > span {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--muted);
  overflow-wrap: anywhere;
}
.trash-card-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 14px;
}
.trash-confirmation {
  line-height: 1.8;
  overflow-wrap: anywhere;
}
.trash-failures {
  margin-bottom: 20px;
  padding: 14px 16px;
  border-radius: 8px;
  color: var(--error-text);
  background: var(--error-soft);
  line-height: 1.7;
}
.trash-failures ul {
  max-height: 180px;
  overflow: auto;
  margin: 8px 0 0;
  padding-left: 18px;
  font-size: 12px;
}
@media (max-width: 560px) {
  .trash-heading {
    flex-wrap: wrap;
  }
  .trash-filters > :deep(.n-select) {
    width: min(150px, 100%);
  }
}
</style>
