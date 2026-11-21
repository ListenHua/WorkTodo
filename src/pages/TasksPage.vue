<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import {
  NButton,
  NCheckbox,
  NCheckboxGroup,
  NPopover,
  NSelect,
  NSkeleton,
  NTab,
  NTabs,
} from 'naive-ui'
import { useWorkspaceStore } from '../stores/workspace'
import { useUiStore } from '../stores/ui'
import { useTaskFiltersStore } from '../stores/task-filters'
import { priorityLabels } from '../shared/types'
import { defaultTaskFilters, taskDateFieldLabels, taskFilterEntry } from '../shared/task-filters'
import AppIcon from '../components/AppIcon.vue'
import EmptyState from '../components/EmptyState.vue'
import TaskDateFilter from '../components/TaskDateFilter.vue'
import TaskList from '../components/TaskList.vue'

const workspace = useWorkspaceStore()
const ui = useUiStore()
const route = useRoute()
const sessions = useTaskFiltersStore()
const workspaceKey = computed(() => sessions.workspaceKey)
const filters = computed({
  get: () => sessions.current,
  set: (value) => Object.assign(sessions.current, value),
})
const priorityMenu = ref(false)
const statusOptions = [
  { label: '未完成', value: 'pending' },
  { label: '进行中', value: 'doing' },
  { label: '已完成', value: 'done' },
  { label: '已取消', value: 'canceled' },
  { label: '已逾期', value: 'overdue' },
  { label: '全部', value: 'all' },
]
const priorities = ['high', 'medium', 'low'] as const
const priorityLabel = computed(() => {
  const selected = priorities.filter((priority) => filters.value.priorities.includes(priority))
  if (selected.length === priorities.length) return '全部优先级'
  return selected.length
    ? `${selected.map((priority) => priorityLabels[priority]).join('、')}优先级`
    : '未选择优先级'
})
const sortOptions = [
  { label: '优先级：高到低', value: 'priority' },
  { label: '最新创建', value: 'created' },
  { label: '计划日期：近到远', value: 'plan' },
  { label: '截止时间：近到远', value: 'due' },
  { label: '标题', value: 'title' },
]
const activeProjects = computed(() =>
  workspace.projects.filter((project) => project.status === 'active'),
)
const projectOptions = computed(() => [
  { label: '全部项目', value: '' },
  { label: '无所属项目', value: 'none' },
  ...activeProjects.value.map((project) => ({ label: project.name, value: project.id })),
])
watch(
  () => [route.fullPath, workspaceKey.value] as const,
  ([fullPath], previous) => {
    if (route.path !== '/tasks') return
    if (!previous || fullPath !== previous[0] || previous[1] === 'unbound') {
      const entry = taskFilterEntry(route.query, workspace.currentDay)
      if (entry) Object.assign(filters.value, entry)
    }
    ui.search = filters.value.search
    priorityMenu.value = false
  },
  { immediate: true },
)
watch(
  () => ui.search,
  (search) => {
    if (route.path === '/tasks') filters.value.search = search
  },
  { flush: 'post' },
)
watch(
  [activeProjects, () => workspace.loading, () => filters.value.project],
  ([projects, loading]) => {
    if (loading) return
    const project = filters.value.project
    if (project && project !== 'none' && !projects.some((item) => item.id === project))
      filters.value.project = ''
  },
  { immediate: true },
)
const filtered = computed(() => sessions.filtered)
const initialLoading = computed(() => workspace.loading && !workspace.tasks.length)
function clearDate() {
  filters.value.date = 'all'
  filters.value.range = null
}
const selectedFilters = computed(() => {
  const selected: { key: string; label: string; remove: () => void }[] = []
  const current = filters.value
  if (current.status !== 'pending')
    selected.push({
      key: 'status',
      label: `状态：${statusOptions.find((option) => option.value === current.status)?.label || '待处理'}`,
      remove: () => (filters.value.status = 'pending'),
    })
  if (current.project)
    selected.push({
      key: 'project',
      label: `项目：${current.project === 'none' ? '无所属项目' : activeProjects.value.find((project) => project.id === current.project)?.name || '所选项目'}`,
      remove: () => (filters.value.project = ''),
    })
  if (current.date !== 'all') {
    const range = current.range
    const label = range
      ? `${taskDateFieldLabels[current.dateField]}：${range[0] === range[1] ? range[0] : `${range[0]} — ${range[1]}`}`
      : ''
    if (label) selected.push({ key: 'date', label, remove: clearDate })
  }
  if (!current.priorities.length)
    selected.push({
      key: 'priorities',
      label: '优先级：未选择',
      remove: () => (filters.value.priorities = [...priorities]),
    })
  else if (current.priorities.length < priorities.length)
    for (const priority of priorities)
      if (current.priorities.includes(priority))
        selected.push({
          key: priority,
          label: `${priorityLabels[priority]}优先级`,
          remove: () => {
            filters.value.priorities = filters.value.priorities.filter(
              (value) => value !== priority,
            )
          },
        })
  if (current.search.trim())
    selected.push({
      key: 'search',
      label: `搜索：${current.search.trim()}`,
      remove: () => {
        filters.value.search = ''
        ui.search = ''
      },
    })
  return selected
})
const hasFilters = computed(() => selectedFilters.value.length > 0)
const canReset = computed(
  () => hasFilters.value || filters.value.sort !== 'priority' || filters.value.dateField !== 'plan',
)
function reset() {
  Object.assign(filters.value, defaultTaskFilters())
  ui.search = ''
  priorityMenu.value = false
}
</script>

<template>
  <section class="panel task-page" aria-label="待办事项">
    <div class="task-filter-toolbar">
      <div class="task-filter-heading">
        <NTabs
          v-model:value="filters.status"
          type="segment"
          class="task-filter-tabs"
          aria-label="待办状态筛选"
          :theme-overrides="{
            tabColorSegment: 'var(--accent)',
            tabTextColorActiveSegment: 'var(--accent-contrast)',
          }"
        >
          <NTab v-for="option in statusOptions" :key="option.value" :name="option.value">
            {{ option.label }}
          </NTab>
        </NTabs>
        <div class="task-filter-results">
          <NSelect
            v-model:value="filters.sort"
            :options="sortOptions"
            class="task-sort-select"
            :input-props="{ 'aria-label': '待办排序' }"
          >
            <template #arrow><AppIcon name="down" :size="16" /></template>
          </NSelect>
        </div>
      </div>
      <div class="task-filter-controls" role="group" aria-label="待办筛选条件">
        <NSelect
          v-model:value="filters.project"
          :options="projectOptions"
          filterable
          class="task-project-select"
          :input-props="{ 'aria-label': '项目筛选，可搜索项目名称' }"
          :consistent-menu-width="false"
          placeholder="搜索并选择项目"
        >
          <template #arrow><AppIcon name="down" :size="16" /></template>
          <template #empty>没有匹配的项目</template>
        </NSelect>
        <NPopover
          v-model:show="priorityMenu"
          trigger="click"
          placement="bottom-start"
          :show-arrow="false"
          class="task-priority-menu"
        >
          <template #trigger>
            <NButton
              class="task-priority-trigger"
              aria-label="优先级筛选"
              :aria-expanded="priorityMenu"
            >
              <span>{{ priorityLabel }}</span><AppIcon name="down" :size="16" />
            </NButton>
          </template>
          <NCheckboxGroup v-model:value="filters.priorities" aria-label="选择待办优先级">
            <div class="task-priority-options">
              <NCheckbox v-for="priority in priorities" :key="priority" :value="priority">
                {{ priorityLabels[priority] }}优先级
              </NCheckbox>
            </div>
          </NCheckboxGroup>
        </NPopover>
        <TaskDateFilter v-model="filters" :today="workspace.currentDay" />
        <NButton v-if="canReset" text class="task-filter-reset" @click="reset">重置</NButton>
      </div>
      <div v-if="hasFilters" class="task-selected-filters" role="group" aria-label="已选筛选条件">
        <button
          v-for="item in selectedFilters"
          :key="item.key"
          type="button"
          class="task-filter-chip"
          :aria-label="`移除筛选：${item.label}`"
          @click="item.remove"
        >
          <span>{{ item.label }}</span><AppIcon name="close" :size="14" />
        </button>
      </div>
    </div>
    <div v-if="initialLoading" class="list-skeleton" aria-label="正在加载待办">
      <NSkeleton v-for="n in 4" :key="n" text :height="44" />
    </div>
    <TaskList v-else-if="filtered.length" :tasks="filtered" />
    <EmptyState
      v-else
      :kind="hasFilters ? 'search' : 'tasks'"
      :title="hasFilters ? '没有匹配的待办' : '暂无待办事项'"
      :action-label="hasFilters ? '点击重置' : '点击新增'"
      :disabled="!hasFilters && !!workspace.status.readOnly"
      @action="hasFilters ? reset() : ui.newTask()"
    />
  </section>
</template>

<style scoped>
.task-filter-toolbar {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding-bottom: 24px;
}
.task-filter-heading,
.task-filter-results,
.task-filter-controls,
.task-selected-filters {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
}
.task-filter-heading {
  justify-content: space-between;
  gap: 16px;
}
.task-filter-tabs {
  flex: 0 1 510px;
  min-width: 0;
}
.task-filter-tabs :deep(.n-tabs-tab) {
  padding: 7px 12px;
}
.task-filter-tabs :deep(.n-tabs-rail .n-tabs-tab-wrapper .n-tabs-tab--active:hover),
.task-filter-tabs :deep(.n-tabs-rail .n-tabs-tab-wrapper .n-tabs-tab--active:focus-visible) {
  color: var(--accent-contrast);
}
.task-filter-results {
  margin-left: auto;
  gap: 12px;
}
.task-sort-select {
  width: 180px;
}
.task-filter-controls {
  gap: 12px;
}
.task-project-select {
  width: 180px;
}
.task-priority-trigger {
  min-width: 140px;
}
.task-priority-trigger :deep(.n-button__content) {
  width: 100%;
  justify-content: space-between;
  gap: 12px;
}
.task-selected-filters {
  gap: 8px;
}
.task-filter-chip {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  max-width: 100%;
  padding: 5px 9px;
  border: 0;
  border-radius: 6px;
  background: var(--surface);
  color: var(--muted);
  font: inherit;
  font-size: 12px;
  line-height: 20px;
  cursor: pointer;
}
.task-filter-chip span {
  min-width: 0;
  overflow-wrap: anywhere;
}
.task-filter-chip .app-icon {
  flex-shrink: 0;
}
.task-filter-chip:hover {
  color: var(--text);
  background: var(--border);
}
.task-filter-chip:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
@media (max-width: 1100px) {
  .task-filter-tabs {
    flex-basis: 100%;
  }
  .task-filter-results {
    width: 100%;
    justify-content: flex-end;
  }
}
</style>

<style>
.task-priority-menu {
  min-width: 150px;
}
.task-priority-options {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 5px 0;
}
</style>
