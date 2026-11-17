<script setup lang="ts">
import IconHint from '../components/IconHint.vue'
import { useAppMessage } from '../composables/useAppMessage'
import { computed, ref } from 'vue'
import type { Project } from '../shared/types'
import { useRoute, useRouter } from 'vue-router'
import { NButton } from 'naive-ui'
import { useWorkspaceStore } from '../stores/workspace'
import { useUiStore } from '../stores/ui'
import { api, desktop } from '../services/api'
import { invoke } from '@tauri-apps/api/core'
import { errorMessage, progress } from '../shared/domain'
import AppIcon from '../components/AppIcon.vue'
import EmptyState from '../components/EmptyState.vue'
import TaskList from '../components/TaskList.vue'
import ProjectFiles from '../components/ProjectFiles.vue'
import ProjectLinks from '../components/ProjectLinks.vue'
import ProjectLinkIcons from '../components/ProjectLinkIcons.vue'
const workspace = useWorkspaceStore()
const message = useAppMessage()
const ui = useUiStore()
const route = useRoute()
const router = useRouter()
const opening = ref(false)
const marking = ref(new Set<string>())
async function toggleBookmark(project: Project) {
  if (marking.value.has(project.id)) return
  marking.value.add(project.id)
  try {
    const pinned = !project.pinned
    await workspace.setProjectPinned(project.id, pinned)
    message.success(pinned ? '已加入快捷访问' : '已取消快捷访问')
  } catch (cause) {
    message.error(errorMessage(cause))
  } finally {
    marking.value.delete(project.id)
  }
}
const visibleProjects = computed(() => {
  const query = ui.search.trim().toLocaleLowerCase()
  return workspace.projects.filter((p) =>
    `${p.name} ${p.description} ${(p.tags ?? []).map((tag) => tag.name).join(' ')}`
      .toLocaleLowerCase()
      .includes(query),
  )
})
const selected = computed(() => workspace.projects.find((p) => p.id === route.query.project))
const projectPath = computed(() => {
  const root = workspace.status.rootPath || ''
  return `${root.replace(/[\\/]+$/, '')}${root.includes('\\') ? '\\' : '/'}${selected.value?.directoryRelpath || ''}`
})
async function copyPath() {
  try {
    if (desktop) await invoke('projects_copy_path', { id: selected.value?.id })
    else await navigator.clipboard.writeText(projectPath.value)
    message.success('路径已复制')
  } catch {
    message.error('无法复制路径，请手动选择复制')
  }
}
const selectedTab = computed(() =>
  ['files', 'links'].includes(String(route.query.view)) ? String(route.query.view) : 'tasks',
)
function selectTab(view: string) {
  void router.replace({ query: { ...route.query, view } })
}
function focusTab(view: 'tasks' | 'files' | 'links') {
  selectTab(view)
  document.getElementById(`project-${view}-tab`)?.focus()
}
const projectTasks = computed(() =>
  workspace.tasks.filter((t) => !t.deletedAt && t.projectId === selected.value?.id),
)
async function openFolder(id: string) {
  opening.value = true
  try {
    await api.openProjectFolder(id)
  } catch (cause) {
    message.error(errorMessage(cause))
  } finally {
    opening.value = false
  }
}
</script>
<template>
  <template v-if="selected">
    <div class="project-directory">
      <AppIcon name="folder" :size="18" /><code :title="projectPath">{{ projectPath }}</code>
      <button class="text-link" @click="copyPath">复制路径</button>
      <NButton text :loading="opening" @click="openFolder(selected.id)"
        >打开文件夹<AppIcon name="external" :size="15"
      /></NButton>
    </div>
    <div class="project-tabs" role="tablist" aria-label="项目视图">
      <button
        v-for="(tab, index) in [
          { key: 'tasks', label: '待办' },
          { key: 'files', label: '文件' },
          { key: 'links', label: '链接' },
        ]"
        :key="tab.key"
        :id="`project-${tab.key}-tab`"
        role="tab"
        :tabindex="selectedTab === tab.key ? 0 : -1"
        @keydown.right.prevent="focusTab((['tasks', 'files', 'links'] as const)[(index + 1) % 3]!)"
        @keydown.left.prevent="focusTab((['tasks', 'files', 'links'] as const)[(index + 2) % 3]!)"
        @keydown.home.prevent="focusTab('tasks')"
        @keydown.end.prevent="focusTab('links')"
        :aria-selected="selectedTab === tab.key"
        :aria-controls="`project-${tab.key}-panel`"
        :class="{ active: selectedTab === tab.key }"
        @click="selectTab(tab.key)"
      >
        {{ tab.label }}
      </button>
    </div>
    <div
      v-if="selectedTab === 'files'"
      id="project-files-panel"
      role="tabpanel"
      aria-labelledby="project-files-tab"
    >
      <ProjectFiles :key="selected.id" :project-id="selected.id" />
    </div>
    <div
      v-else-if="selectedTab === 'links'"
      id="project-links-panel"
      role="tabpanel"
      aria-labelledby="project-links-tab"
    >
      <ProjectLinks :project="selected" />
    </div>
    <section
      v-else
      id="project-tasks-panel"
      role="tabpanel"
      aria-labelledby="project-tasks-tab"
      class="panel"
    >
      <div class="panel-heading">
        <div>
          <h2>项目待办</h2>
          <p>{{ selected.completedCount }} / {{ selected.taskCount }} 项已完成</p>
        </div>
        <span class="project-progress-label">{{
          progress(selected) === null ? '暂无任务' : `${progress(selected)}%`
        }}</span>
      </div>
      <TaskList v-if="projectTasks.length" :tasks="projectTasks" />
      <EmptyState v-else title="暂无待办" /></section
  ></template>
  <template v-else>
    <div v-if="visibleProjects.length" class="project-grid">
      <article v-for="project in visibleProjects" :key="project.id" class="panel project-card">
        <RouterLink
          :to="{ path: '/projects', query: { project: project.id } }"
          class="project-card-main"
        >
          <h2>{{ project.name }}</h2>
          <p v-if="project.description" class="project-description">{{ project.description }}</p>
        </RouterLink>
        <IconHint :text="project.pinned ? '取消快捷访问' : '加入快捷访问'"
          ><button
            type="button"
            :class="['icon-button', 'project-bookmark', { marked: project.pinned }]"
            :aria-label="`${project.pinned ? '取消快捷访问' : '加入快捷访问'} ${project.name}`"
            :aria-pressed="!!project.pinned"
            :aria-busy="marking.has(project.id)"
            :disabled="marking.has(project.id)"
            @click.stop.prevent="toggleBookmark(project)"
          >
            <AppIcon :name="project.pinned ? 'bookmarked' : 'bookmark'" :size="20" /></button
        ></IconHint>
        <div v-if="project.tags?.length" class="project-tags" aria-label="项目标签">
          <span
            v-for="tag in project.tags"
            :key="tag.name"
            class="project-tag"
            :style="{ '--tag-color': tag.color, background: `${tag.color}18` }"
          >
            <span class="project-tag-dot" aria-hidden="true" />{{ tag.name }}
          </span>
        </div>
        <ProjectLinkIcons v-if="project.links?.length" :links="project.links" />
      </article>
    </div>
    <section v-else class="panel">
      <EmptyState icon="folder" :title="ui.search.trim() ? '没有匹配的项目' : '暂无项目'"
        ><NButton v-if="ui.search.trim()" @click="ui.search = ''">清除搜索</NButton
        ><NButton v-else type="primary" @click="ui.newProject()">创建项目</NButton></EmptyState
      >
    </section>
  </template>
</template>
