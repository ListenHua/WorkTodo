<script setup lang="ts">
import IconHint from './IconHint.vue'
import { listen } from '@tauri-apps/api/event'
import { useAppMessage } from '../composables/useAppMessage'
import { useTrayActions } from '../composables/useTrayActions'
import { api, desktop } from '../services/api'
import { errorMessage } from '../shared/domain'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { NButton } from 'naive-ui'
import { useWorkspaceStore } from '../stores/workspace'
import { useUiStore } from '../stores/ui'
import { useReleaseStore } from '../stores/release'
import { useThemeStore } from '../stores/theme'
import { useTaskFiltersStore } from '../stores/task-filters'
import AppIcon from './AppIcon.vue'
import TopbarSearch from './TopbarSearch.vue'
import AnimatedThemeToggler from './magicui/AnimatedThemeToggler.vue'

const emit = defineEmits<{ toolbarResize: [height: number] }>()
const toolbarHost = ref<HTMLElement | null>(null)
const pageToolbarHeight = ref(0)
let toolbarObserver: ResizeObserver | undefined
const topbarActions = ref<HTMLElement | null>(null)
const topbarSideWidth = ref(192)
let topbarActionsObserver: ResizeObserver | undefined
const workspace = useWorkspaceStore()
const message = useAppMessage()
const ui = useUiStore()
const release = useReleaseStore()
const theme = useThemeStore()
const taskFilters = useTaskFiltersStore()
const route = useRoute()
const router = useRouter()
const showSearch = computed(() => route.path !== '/settings')
watch(
  () => workspace.status.bindingId,
  (next, previous) => {
    if (!previous || next === previous) return
    ui.projectDialog = false
    ui.folderProjectsDialog = false
    ui.taskDialog = false
    ui.editingProject = null
    ui.editingTask = null
    ui.defaultProjectId = null
    ui.search = ''
    workspace.sortingProjects = false
    if (route.path !== '/settings') void router.replace('/')
  },
)

watch(
  () => [route.path, route.query.project, route.query.view],
  () => {
    if (route.path !== '/tasks') ui.search = ''
  },
  { immediate: true },
)
const compactSidebar = ref(false)
let sidebarMedia: MediaQueryList | undefined
function updateSidebar() {
  compactSidebar.value = sidebarMedia?.matches ?? false
}
const selectedProject = computed(() =>
  route.path === '/projects'
    ? workspace.projects.find((p) => p.id === route.query.project)
    : undefined,
)
useTrayActions(() => {
  const project = selectedProject.value
  const projectTasks =
    project?.status === 'active' && !['files', 'links', 'code'].includes(String(route.query.view))
  if (route.path !== '/tasks' && !projectTasks) return false
  if (!ui.taskDialog) ui.newTask(projectTasks ? project!.id : null)
  return true
})
const flatPage = computed(() => !!selectedProject.value || !['/', '/projects'].includes(route.path))
const cardPage = computed(() =>
  selectedProject.value
    ? !['files', 'links', 'code'].includes(String(route.query.view))
    : ['/', '/projects', '/tasks', '/trash'].includes(route.path),
)
const openingProjectFolder = ref(false)
async function openProjectFolder() {
  const project = selectedProject.value
  if (!project || openingProjectFolder.value) return
  openingProjectFolder.value = true
  try {
    await api.openProjectFolder(project.id)
  } catch (cause) {
    message.error(errorMessage(cause))
  } finally {
    openingProjectFolder.value = false
  }
}
const pinnedProjects = computed(() => workspace.projects.filter((project) => project.pinned))
const navigation = [
  { path: '/', title: '概览', icon: 'grid' },
  { path: '/tasks', title: '待办', icon: 'list' },
  { path: '/projects', title: '项目空间', icon: 'folder' },
  { path: '/reminders', title: '提醒中心', icon: 'clock' },
  { path: '/trash', title: '回收站', icon: 'inbox' },
]
const dateLabel = computed(() =>
  new Date(`${workspace.currentDay}T12:00:00`).toLocaleDateString('zh-CN', {
    month: 'long',
    day: 'numeric',
    weekday: 'long',
  }),
)
function keyboard(event: KeyboardEvent) {
  if (event.isComposing || !event.altKey || event.key.toLowerCase() !== 'n') return
  event.preventDefault()
  ui.newTask(selectedProject.value?.status === 'active' ? selectedProject.value.id : null)
}
function wake() {
  if (!document.hidden) void workspace.refresh()
}
let clockTimer: ReturnType<typeof setInterval> | undefined
let closed = false
const listeners: (() => void)[] = []
onMounted(async () => {
  if (topbarActions.value) {
    const updateActionsWidth = () => {
      topbarSideWidth.value = Math.ceil(topbarActions.value?.getBoundingClientRect().width || 0)
    }
    updateActionsWidth()
    topbarActionsObserver = new ResizeObserver(updateActionsWidth)
    topbarActionsObserver.observe(topbarActions.value)
  }
  if (toolbarHost.value) {
    toolbarObserver = new ResizeObserver(() => {
      pageToolbarHeight.value = toolbarHost.value?.offsetHeight ?? 0
      emit('toolbarResize', pageToolbarHeight.value)
    })
    toolbarObserver.observe(toolbarHost.value)
  }
  sidebarMedia = window.matchMedia('(max-width: 760px)')
  updateSidebar()
  sidebarMedia.addEventListener('change', updateSidebar)
  clockTimer = setInterval(workspace.syncClock, 30_000)
  window.addEventListener('keydown', keyboard)
  document.addEventListener('visibilitychange', wake)
  window.addEventListener('focus', wake)
  if (desktop) {
    const changes = await listen('workspace-changed', () => workspace.refresh())
    const open = await listen<string>('open-task', async (event) => {
      await workspace.refresh()
      const task = workspace.tasks.find((t) => t.id === event.payload && !t.deletedAt)
      if (task) ui.editTask(task)
    })
    const captureSaved = await listen('capture-saved', () => message.success('已添加待办'))
    if (closed) {
      captureSaved()
      changes()
      open()
    } else listeners.push(changes, open, captureSaved)
  }
})
onUnmounted(() => {
  toolbarObserver?.disconnect()
  topbarActionsObserver?.disconnect()
  emit('toolbarResize', 0)
  sidebarMedia?.removeEventListener('change', updateSidebar)
  closed = true
  if (clockTimer) clearInterval(clockTimer)
  listeners.forEach((off) => off())
  window.removeEventListener('keydown', keyboard)
  document.removeEventListener('visibilitychange', wake)
  window.removeEventListener('focus', wake)
})
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <IconHint text="概览" :disabled="!compactSidebar"
        ><RouterLink to="/" class="brand" aria-label="WorkTodo"
          ><img
            class="brand-symbol"
            :src="theme.isDark ? '/app-icon.svg' : '/app-icon-light.svg'"
            width="39"
            height="39"
            alt=""
          /><span>WorkTodo</span></RouterLink
        ></IconHint
      >
      <nav aria-label="主导航">
        <IconHint
          v-for="item in navigation"
          :key="item.path"
          :text="item.title"
          :disabled="!compactSidebar"
          ><RouterLink
            :aria-label="item.title"
            :to="item.path"
            :class="['nav-item', { active: route.path === item.path }]"
          >
            <AppIcon :name="item.icon" /><span>{{ item.title }}</span>
          </RouterLink></IconHint
        >
      </nav>
      <section v-if="pinnedProjects.length" class="sidebar-projects" aria-label="快捷访问">
        <div class="sidebar-caption">快捷访问</div>
        <div class="sidebar-project-list">
          <RouterLink
            v-for="project in pinnedProjects"
            :key="project.id"
            :to="{ path: '/projects', query: { project: project.id } }"
            :class="['project-nav', { active: selectedProject?.id === project.id }]"
            :title="project.name"
            ><span class="color-dot" :style="{ background: project.color }" /><span>{{
              project.name
            }}</span></RouterLink
          >
        </div>
      </section>
      <div class="sidebar-bottom">
        <RouterLink
          to="/settings"
          aria-label="设置"
          :class="['nav-item', { active: route.path === '/settings' }]"
          ><AppIcon name="settings" /><span>设置</span
          ><span class="sidebar-version">v{{ release.info.version }}</span></RouterLink
        >
      </div>
    </aside>
    <div
      class="main-column"
      :style="{ '--page-toolbar-height': `${pageToolbarHeight}px` }"
      :class="{
        'flat-layout': flatPage,
        'card-layout': cardPage,
        'without-topbar-search': !showSearch,
        'overview-layout': route.path === '/',
      }"
    >
      <header class="topbar" :style="{ '--topbar-side-width': `${topbarSideWidth}px` }">
        <div class="topbar-title">
          <IconHint text="返回全部项目" v-if="selectedProject"
            ><button
              class="icon-button project-back"
              aria-label="返回全部项目"
              @click="router.push('/projects')"
            >
              <AppIcon name="back" :size="21" /></button
          ></IconHint>
          <h1 :title="selectedProject?.name || String(route.meta.title)">
            {{ selectedProject?.name || route.meta.title }}
          </h1>
          <span
            v-if="route.path === '/tasks'"
            class="topbar-task-count muted"
            role="status"
            aria-label="待办数量"
            aria-live="polite"
            aria-atomic="true"
            >{{
              workspace.loading && !workspace.tasks.length ? '…' : taskFilters.filtered.length
            }}</span
          >
        </div>
        <TopbarSearch v-if="showSearch" />
        <div ref="topbarActions" class="topbar-actions">
          <span class="date-label">{{ dateLabel }}</span>
          <AnimatedThemeToggler />
          <IconHint text="刷新数据"
            ><button
              class="icon-button refresh-button"
              aria-label="刷新数据"
              :disabled="workspace.loading"
              @click="workspace.refresh()"
            >
              <AppIcon name="refresh" :size="18" /></button
          ></IconHint>
          <IconHint text="新建待办" v-if="route.path === '/tasks'"
            ><button
              class="icon-button topbar-primary-action"
              aria-label="新建待办"
              @click="ui.newTask()"
            >
              <AppIcon name="plus" :size="21" /></button
          ></IconHint>
          <IconHint text="打开项目文件夹" v-if="selectedProject"
            ><button
              class="icon-button"
              aria-label="打开项目文件夹"
              :disabled="openingProjectFolder"
              :aria-busy="openingProjectFolder"
              @click="openProjectFolder"
            >
              <AppIcon name="folder" :size="20" /></button
          ></IconHint>
          <IconHint
            text="添加待办"
            v-if="selectedProject?.status === 'active' && route.query.view !== 'code'"
            ><button
              class="icon-button topbar-primary-action"
              aria-label="添加待办"
              @click="ui.newTask(selectedProject.id)"
            >
              <AppIcon name="plus" :size="21" /></button
          ></IconHint>
          <IconHint text="编辑项目" v-if="selectedProject"
            ><button
              class="icon-button"
              aria-label="编辑项目"
              @click="ui.editProject(selectedProject)"
            >
              <AppIcon name="edit" :size="20" /></button
          ></IconHint>
          <button
            v-if="route.path === '/projects' && !selectedProject"
            type="button"
            class="icon-button"
            aria-label="从文件夹创建项目"
            :disabled="
              !workspace.status.available || workspace.status.readOnly || workspace.sortingProjects
            "
            @click="ui.folderProjectsDialog = true"
          >
            <AppIcon name="folder" :size="20" />
          </button>
          <IconHint text="新建项目" v-if="route.path === '/projects' && !selectedProject"
            ><button
              class="icon-button topbar-primary-action"
              aria-label="新建项目"
              @click="ui.newProject()"
            >
              <AppIcon name="plus" :size="21" /></button
          ></IconHint>
        </div>
      </header>
      <div id="page-toolbar" ref="toolbarHost" class="page-toolbar" />
      <div v-if="!workspace.desktop" class="preview-banner">
        <AppIcon name="info" :size="15" />浏览器预览：刷新后数据清空，不写入本地文件。
      </div>
      <main
        class="page-container"
        :class="{
          'flat-page': flatPage,
          'overview-page': route.path === '/',
          'project-detail-page': !!selectedProject,
        }"
        :data-module="route.path"
      >
        <div v-if="workspace.status.readOnly" class="error-banner" role="alert">
          {{ workspace.status.storageError || '工作目录暂不可写，当前显示只读缓存' }}
          <NButton size="small" @click="workspace.refresh()">重新检查</NButton>
        </div>
        <div v-if="workspace.error" class="error-banner" role="alert">
          {{ workspace.error }}<NButton size="small" @click="workspace.refresh()">重试</NButton>
        </div>
        <slot />
      </main>
    </div>
  </div>
</template>
