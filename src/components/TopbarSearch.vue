<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { NInput } from 'naive-ui'
import { useUiStore } from '../stores/ui'
import { useWorkspaceStore } from '../stores/workspace'
import type { Task } from '../shared/types'
import AppIcon from './AppIcon.vue'
import EmptyState from './EmptyState.vue'
const ui = useUiStore()
const workspace = useWorkspaceStore()
const route = useRoute()
const router = useRouter()
const root = ref<HTMLElement>()
const focused = ref(false)
const active = ref(0)
const localSearch = computed(() => ['/tasks', '/trash', '/reminders'].includes(route.path))
const placeholder = computed(() => {
  if (route.path === '/tasks') return '搜索待办…'
  if (route.path === '/trash') return '搜索回收站…'
  if (route.path === '/reminders') return '搜索提醒…'
  return projectList.value ? '搜索项目…' : '搜索项目或待办…'
})
const projectList = computed(() => route.path === '/projects' && !route.query.project)
const query = computed(() => ui.search.trim().toLocaleLowerCase())
const results = computed(() => {
  if (localSearch.value || !query.value) return []
  const projects = workspace.projects
    .filter((p) =>
      `${p.name} ${p.description} ${(p.tags ?? []).map((tag) => tag.name).join(' ')}`
        .toLocaleLowerCase()
        .includes(query.value),
    )
    .slice(0, 5)
    .map((p) => ({
      id: `project-${p.id}`,
      title: p.name,
      kind: '项目',
      icon: 'folder',
      projectId: p.id,
      task: undefined as Task | undefined,
    }))
  const tasks = workspace.tasks
    .filter(
      (t) =>
        !t.deletedAt &&
        `${t.title} ${t.description || ''}`.toLocaleLowerCase().includes(query.value),
    )
    .slice(0, 5)
    .map((t) => ({
      id: `task-${t.id}`,
      title: t.title,
      kind: '待办',
      icon: 'list',
      projectId: '',
      task: t,
    }))
  return projectList.value ? projects : [...projects, ...tasks]
})
const expanded = computed(
  () => focused.value && !!query.value && !projectList.value && !localSearch.value,
)
function choose(index: number) {
  const item = results.value[index]
  if (!item) return
  focused.value = false
  ui.search = ''
  if (item.task) ui.editTask(item.task)
  else void router.push({ path: '/projects', query: { project: item.projectId } })
}
function move(step: number) {
  focused.value = true
  if (results.value.length)
    active.value = (active.value + step + results.value.length) % results.value.length
}
function keyboard(event: KeyboardEvent) {
  if (event.isComposing || localSearch.value) return
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    move(event.key === 'ArrowDown' ? 1 : -1)
  } else if (event.key === 'Enter' && query.value) {
    event.preventDefault()
    choose(active.value)
  } else if (event.key === 'Escape') {
    focused.value = false
    event.stopPropagation()
  }
}
function outside(event: PointerEvent) {
  if (!root.value?.contains(event.target as Node)) focused.value = false
}
watch(
  () => [route.path, route.query.project, route.query.view],
  () => {
    focused.value = false
  },
)
watch(query, () => {
  active.value = 0
})
onMounted(() => document.addEventListener('pointerdown', outside))
onBeforeUnmount(() => document.removeEventListener('pointerdown', outside))
</script>
<template>
  <div
    ref="root"
    class="topbar-search"
    @focusin="focused = true"
    @focusout="
      (event) => {
        if (!root?.contains(event.relatedTarget as Node)) focused = false
      }
    "
  >
    <NInput
      v-model:value="ui.search"
      round
      :bordered="false"
      clearable
      :placeholder="placeholder"
      :input-props="{
        role: localSearch ? 'searchbox' : 'combobox',
        'aria-label': '顶部搜索',
        'aria-expanded': localSearch ? undefined : expanded,
        'aria-controls': expanded ? 'topbar-search-results' : undefined,
        'aria-activedescendant':
          expanded && results[active] ? `search-${results[active]!.id}` : undefined,
        autocomplete: 'off',
      }"
      @keydown="keyboard"
    >
      <template #prefix><AppIcon name="search" :size="17" /></template>
      <template #clear-icon><AppIcon name="clear" :size="16" /></template>
    </NInput>
    <div
      v-if="expanded"
      id="topbar-search-results"
      class="topbar-search-results"
      :role="results.length ? 'listbox' : undefined"
      aria-label="搜索结果"
    >
      <button
        v-for="(item, index) in results"
        :id="`search-${item.id}`"
        :key="item.id"
        type="button"
        role="option"
        :aria-selected="active === index"
        :class="{ active: active === index }"
        @pointerdown.prevent
        @click="choose(index)"
        @pointermove="active = index"
      >
        <AppIcon :name="item.icon" :size="18" /><span>{{ item.title }}</span
        ><small>{{ item.kind }}</small>
      </button>
      <EmptyState
        v-if="!results.length"
        compact
        kind="search"
        title="没有搜索结果"
        action-label="点击清除搜索"
        role="status"
        @action="ui.search = ''"
      />
    </div>
  </div>
</template>
