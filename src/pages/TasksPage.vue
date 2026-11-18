<script setup lang="ts">
import { computed, reactive, watch } from 'vue'
import { useRoute } from 'vue-router'
import { NButton, NSelect, NSkeleton } from 'naive-ui'
import { useWorkspaceStore } from '../stores/workspace'
import { useUiStore } from '../stores/ui'
import { priorityLabels, statusLabels } from '../shared/types'
import { filterTasks, type TaskFilters } from '../shared/task-filters'
import AppIcon from '../components/AppIcon.vue'
import EmptyState from '../components/EmptyState.vue'
import TaskList from '../components/TaskList.vue'
const workspace = useWorkspaceStore()
const ui = useUiStore()
const route = useRoute()
const filters = reactive<Omit<TaskFilters, 'search'>>({
  status: 'pending',
  project: '',
  priority: '',
  date: 'all',
  sort: 'priority',
})
watch(
  () => [route.path, route.query.view, route.query.date, route.query.project],
  () => {
    filters.status = route.query.view === 'done' ? 'done' : 'pending'
    const date = route.query.date || route.query.view
    filters.date = ['overdue', 'week', 'today', 'unplanned'].includes(String(date))
      ? (date as TaskFilters['date'])
      : 'all'
    filters.project =
      typeof route.query.project === 'string'
        ? route.query.project === 'inbox'
          ? 'none'
          : route.query.project
        : ''
  },
  { immediate: true },
)
const filtered = computed(() =>
  filterTasks(
    workspace.tasks,
    workspace.projects,
    { ...filters, search: ui.search },
    String(route.meta.filter),
    workspace.currentDay,
    workspace.now,
  ),
)
const initialLoading = computed(() => workspace.loading && !workspace.tasks.length)
const hasFilters = computed(
  () =>
    !!ui.search.trim() ||
    !!filters.project ||
    !!filters.priority ||
    filters.date !== 'all' ||
    filters.status !== 'pending',
)
function reset() {
  ui.search = ''
  Object.assign(filters, {
    status: 'pending',
    project: '',
    priority: '',
    date: 'all',
    sort: 'priority',
  })
}
</script>
<template>
  <section class="panel">
    <div class="list-toolbar task-toolbar">
      <NSelect
        v-if="route.meta.filter !== 'trash'"
        v-model:value="filters.status"
        aria-label="状态筛选"
        :options="[
          { label: '未完成', value: 'pending' },
          { label: '全部状态', value: 'all' },
          ...Object.entries(statusLabels).map(([value, label]) => ({ value, label })),
        ]"
      >
        <template #arrow><AppIcon name="down" :size="16" /></template>
      </NSelect>
      <NSelect
        v-model:value="filters.project"
        aria-label="项目筛选"
        :options="[
          { label: '全部项目', value: '' },
          { label: '无', value: 'none' },
          ...workspace.projects.map((p) => ({ label: p.name, value: p.id })),
        ]"
      >
        <template #arrow><AppIcon name="down" :size="16" /></template>
      </NSelect>
      <NSelect
        v-model:value="filters.priority"
        aria-label="优先级筛选"
        :options="[
          { label: '全部优先级', value: '' },
          ...Object.entries(priorityLabels).map(([value, label]) => ({
            value,
            label: `${label}优先级`,
          })),
        ]"
      >
        <template #arrow><AppIcon name="down" :size="16" /></template>
      </NSelect>
      <NSelect
        v-model:value="filters.date"
        aria-label="日期筛选"
        :options="[
          { label: '全部日期', value: 'all' },
          { label: '已逾期', value: 'overdue' },
          { label: '今天', value: 'today' },
          { label: '未来七天', value: 'week' },
          { label: '未设计划日期', value: 'unplanned' },
        ]"
      >
        <template #arrow><AppIcon name="down" :size="16" /></template>
      </NSelect>
      <NSelect
        v-model:value="filters.sort"
        aria-label="待办排序"
        :options="[
          { label: '最新创建', value: 'created' },
          { label: '截止时间', value: 'due' },
          { label: '优先级', value: 'priority' },
          { label: '标题', value: 'title' },
        ]"
      >
        <template #arrow><AppIcon name="down" :size="16" /></template>
      </NSelect>
      <span class="muted task-count">{{ filtered.length }} 项</span>
      <NButton v-if="hasFilters" text @click="reset">重置</NButton>
    </div>
    <div v-if="initialLoading" class="list-skeleton" aria-label="正在加载待办">
      <NSkeleton v-for="n in 4" :key="n" text :height="44" />
    </div>
    <TaskList v-else-if="filtered.length" :tasks="filtered" />
    <EmptyState
      v-else
      :title="
        hasFilters
          ? route.meta.filter === 'trash'
            ? '没有匹配的回收站内容'
            : '没有匹配的待办'
          : route.meta.filter === 'trash'
            ? '回收站为空'
            : '暂无待办'
      "
    >
      <NButton v-if="hasFilters" @click="reset">清除筛选</NButton>
      <NButton v-else-if="route.meta.filter !== 'trash'" type="primary" @click="ui.newTask()"
        >新建待办</NButton
      >
    </EmptyState>
  </section>
</template>
