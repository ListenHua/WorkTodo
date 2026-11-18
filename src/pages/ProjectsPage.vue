<script setup lang="ts">
import { compareTaskPriority } from '../shared/task-priority'
import IconHint from '../components/IconHint.vue'
import { useAppMessage } from '../composables/useAppMessage'
import { computed, h, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { Project } from '../shared/types'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { NButton, NDropdown, NModal, NTab, NTabs } from 'naive-ui'
import { useWorkspaceStore } from '../stores/workspace'
import { useUiStore } from '../stores/ui'
import { errorMessage, progress } from '../shared/domain'
import AppIcon from '../components/AppIcon.vue'
import EmptyState from '../components/EmptyState.vue'
import TaskList from '../components/TaskList.vue'
import ProjectFiles from '../components/ProjectFiles.vue'
import WorkspaceFiles from '../components/WorkspaceFiles.vue'
import ProjectLinks from '../components/ProjectLinks.vue'
import ProjectLinkIcons from '../components/ProjectLinkIcons.vue'
import ProjectMasonry from '../components/ProjectMasonry.vue'
import { useProjectCardSort } from '../composables/useProjectCardSort'
import { projectSortOptions } from '../shared/project-sort'
const workspace = useWorkspaceStore()
const message = useAppMessage()
const ui = useUiStore()
const route = useRoute()
const router = useRouter()
const projectSpace = ref<HTMLElement | null>(null)
const sortToolbar = ref<HTMLElement | null>(null)
const {
  sorting,
  mode: sortMode,
  projects: displayedProjects,
  error: sortError,
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
} = useProjectCardSort(projectSpace, sortToolbar)
const sortModeOptions = projectSortOptions.map((option) => ({
  label: option.label,
  key: option.value,
}))
const sortModeLabel = computed(
  () => projectSortOptions.find((option) => option.value === sortMode.value)?.label,
)
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
  return displayedProjects.value.filter((p) =>
    `${p.name} ${p.description} ${(p.tags ?? []).map((tag) => tag.name).join(' ')}`
      .toLocaleLowerCase()
      .includes(query),
  )
})
const selected = computed(() => workspace.projects.find((p) => p.id === route.query.project))
const contextProject = ref<Project | null>(null)
const deleteTarget = ref<Project | null>(null)
const deleting = ref(false)
const contextVisible = ref(false)
const contextVersion = ref(0)
const contextPosition = ref({ x: 0, y: 0 })
let contextTrigger: HTMLElement | null = null
let mounted = true
const contextOptions = computed(() =>
  contextProject.value
    ? [
        { label: '打开项目', key: 'open', disabled: sorting.value },
        { label: '编辑项目', key: 'edit', disabled: sorting.value || !!workspace.status.readOnly },
        {
          label: '删除项目',
          key: 'delete',
          disabled: sorting.value || !!workspace.status.readOnly,
        },
      ]
    : [
        {
          label: '排序项目',
          key: 'sort',
          icon: () => h(AppIcon, { name: 'drag', size: 17 }),
          disabled:
            sorting.value ||
            workspace.loading ||
            workspace.projects.length < 2 ||
            !!workspace.status.readOnly ||
            workspace.sortingProjects,
        },
        {
          label: '新建项目',
          key: 'create',
          icon: () => h(AppIcon, { name: 'plus', size: 17 }),
          disabled: !!workspace.status.readOnly,
        },
      ],
)
async function showContextMenu(event: MouseEvent | KeyboardEvent, project: Project | null = null) {
  event.preventDefault()
  event.stopPropagation()
  contextProject.value = project
  contextTrigger =
    (event.target as HTMLElement).closest<HTMLElement>('a,button') ??
    (event.currentTarget as HTMLElement)
  contextVisible.value = false
  const version = ++contextVersion.value
  await nextTick()
  // Keyboard focus may scroll a long masonry list before the menu is opened.
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
  if (!mounted || version !== contextVersion.value || selected.value) return
  const bounds = contextTrigger.getBoundingClientRect()
  contextPosition.value =
    event instanceof MouseEvent && event.button === 2
      ? { x: event.clientX, y: event.clientY }
      : {
          x: Math.max(12, Math.min(bounds.left + 24, window.innerWidth - 180)),
          y: Math.max(96, Math.min(bounds.top + 24, window.innerHeight - 120)),
        }
  contextVisible.value = true
}
function closeContextMenu() {
  contextVisible.value = false
  contextVersion.value++
}
function closeContextMenuOnScroll() {
  if (contextVisible.value) closeContextMenu()
}
function selectContextAction(key: string) {
  const project = workspace.projects.find((item) => item.id === contextProject.value?.id)
  closeContextMenu()
  if (project) {
    if (sorting.value) return
    if (key === 'open') void router.push({ path: '/projects', query: { project: project.id } })
    if (workspace.status.readOnly) return
    if (key === 'edit') ui.editProject(project)
    if (key === 'delete') deleteTarget.value = project
    return
  }
  if (key === 'sort') beginSorting()
  if (key === 'create') ui.newProject()
}
async function confirmDeleteProject() {
  if (!deleteTarget.value || deleting.value) return
  deleting.value = true
  try {
    await workspace.deleteProject(deleteTarget.value.id)
    deleteTarget.value = null
    message.success('项目已删除')
  } catch (cause) {
    message.error(errorMessage(cause))
  } finally {
    deleting.value = false
  }
}
function contextKeydown(event: KeyboardEvent) {
  if (event.key !== 'Escape' || !contextVisible.value) return
  event.preventDefault()
  event.stopImmediatePropagation()
  closeContextMenu()
  contextTrigger?.focus({ preventScroll: true })
}
watch([() => route.fullPath, () => ui.projectDialog, () => ui.search, sorting], closeContextMenu)
watch([() => route.fullPath, () => ui.projectDialog], () => {
  cancelSorting()
})
watch(() => ui.search, cancelDrag)
watch(
  () => workspace.projects,
  () => {
    if (
      contextProject.value &&
      !workspace.projects.some((project) => project.id === contextProject.value?.id)
    )
      closeContextMenu()
  },
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
  document.removeEventListener('contextmenu', closeContextMenu, true)
  document.removeEventListener('scroll', closeContextMenuOnScroll, true)
  document.removeEventListener('keydown', contextKeydown, true)
  window.removeEventListener('resize', closeContextMenu)
  window.removeEventListener('blur', closeContextMenu)
})
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
  workspace.tasks
    .filter((t) => !t.deletedAt && t.projectId === selected.value?.id)
    .sort(compareTaskPriority),
)
</script>
<template>
  <template v-if="selected">
    <NTabs
      :value="selectedTab"
      type="line"
      class="project-tabs"
      role="tablist"
      aria-label="项目视图"
      :theme-overrides="{ tabBorderColor: 'transparent', tabGapMediumLine: '20px' }"
      @update:value="selectTab"
    >
      <NTab
        v-for="(tab, index) in [
          { key: 'tasks', label: '待办' },
          { key: 'files', label: '文件' },
          { key: 'links', label: '链接' },
        ]"
        :key="tab.key"
        :name="tab.key"
        :id="`project-${tab.key}-tab`"
        role="tab"
        :tabindex="selectedTab === tab.key ? 0 : -1"
        @keydown.right.prevent="focusTab((['tasks', 'files', 'links'] as const)[(index + 1) % 3]!)"
        @keydown.left.prevent="focusTab((['tasks', 'files', 'links'] as const)[(index + 2) % 3]!)"
        @keydown.home.prevent="focusTab('tasks')"
        @keydown.end.prevent="focusTab('links')"
        :aria-selected="selectedTab === tab.key"
        :aria-controls="`project-${tab.key}-panel`"
        @keydown.enter.prevent="selectTab(tab.key)"
        @keydown.space.prevent="selectTab(tab.key)"
      >
        {{ tab.label }}
      </NTab>
    </NTabs>
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
  <section
    v-else
    ref="projectSpace"
    class="project-space"
    :class="{ 'is-sorting-projects': sorting, 'is-dragging-project': draggingId }"
    aria-label="项目空间"
    tabindex="0"
    @pointermove="moveDrag"
    @pointerup="finishDrag"
    @pointercancel="cancelDrag"
    @lostpointercapture="cancelDrag"
    @contextmenu="showContextMenu($event)"
    @keydown.shift.f10.prevent.stop="showContextMenu($event)"
    @keydown="$event.code === 'ContextMenu' && showContextMenu($event)"
  >
    <Teleport v-if="sorting" to="#page-toolbar">
      <div ref="sortToolbar" class="project-sort-toolbar" role="region" aria-label="项目排序">
        <div class="project-sort-toolbar-content">
          <div class="project-sort-instructions">
            <strong>排序项目</strong>
            <p>拖动卡片调整顺序</p>
          </div>
          <div class="project-sort-actions">
            <NDropdown
              trigger="click"
              :value="sortMode"
              :options="sortModeOptions"
              :disabled="workspace.sortingProjects || !!draggingId"
              :menu-props="() => ({ role: 'menu', 'aria-label': '排序方式选项' })"
              :node-props="() => ({ role: 'menuitem' })"
              @select="changeMode"
            >
              <NButton
                class="project-sort-mode"
                aria-label="排序方式"
                :disabled="workspace.sortingProjects || !!draggingId"
              >
                {{ sortModeLabel }}<AppIcon name="down" :size="16" />
              </NButton>
            </NDropdown>
            <NButton :disabled="workspace.sortingProjects" @click="cancelSorting">取消</NButton>
            <NButton
              type="primary"
              :loading="workspace.sortingProjects"
              :disabled="!!draggingId"
              @click="saveSorting"
              >保存排序</NButton
            >
          </div>
          <p v-if="sortError" class="form-error" role="alert">{{ sortError }}</p>
          <span class="project-sort-announcement" role="status" aria-live="polite">{{
            announcement
          }}</span>
        </div>
      </div>
    </Teleport>
    <ProjectMasonry v-if="visibleProjects.length" :projects="visibleProjects">
      <template #default="{ project, index }">
        <article
          class="panel project-card"
          :data-project-id="project.id"
          :class="{
            'is-sortable': sorting,
            'is-dragging': draggingId === project.id,
          }"
          :aria-busy="sorting && workspace.sortingProjects"
          :tabindex="sorting ? 0 : undefined"
          :aria-description="
            sorting ? `第 ${index + 1} 位，可拖动卡片或使用方向键调整顺序` : undefined
          "
          @keydown.up.self="moveWithKeyboard(project.id, -1, $event)"
          @keydown.left.self="moveWithKeyboard(project.id, -1, $event)"
          @keydown.down.self="moveWithKeyboard(project.id, 1, $event)"
          @keydown.right.self="moveWithKeyboard(project.id, 1, $event)"
          @contextmenu.stop="showContextMenu($event, project)"
          @keydown.shift.f10.prevent.stop="showContextMenu($event, project)"
          @keydown="$event.code === 'ContextMenu' && showContextMenu($event, project)"
          @pointerdown="startDrag($event, project.id)"
          @dragstart="sorting && $event.preventDefault()"
        >
          <component
            :is="sorting ? 'div' : RouterLink"
            :to="sorting ? undefined : { path: '/projects', query: { project: project.id } }"
            class="project-card-main"
          >
            <h2 :title="project.name">{{ project.name }}</h2>
            <p v-if="project.description" class="project-description">{{ project.description }}</p>
          </component>
          <IconHint v-if="!sorting" :text="project.pinned ? '取消快捷访问' : '加入快捷访问'"
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
              :style="{ color: tag.color, background: `${tag.color}18` }"
            >
              {{ tag.name }}
            </span>
          </div>
          <ProjectLinkIcons
            v-if="project.links?.length"
            :links="project.links"
            :disabled="sorting"
            :inert="sorting || undefined"
          />
        </article>
      </template>
    </ProjectMasonry>
    <section v-else class="panel">
      <EmptyState icon="folder" :title="ui.search.trim() ? '没有匹配的项目' : '暂无项目'"
        ><NButton v-if="ui.search.trim()" @click="ui.search = ''">清除搜索</NButton
        ><NButton v-else type="primary" @click="ui.newProject()">创建项目</NButton></EmptyState
      >
    </section>
    <WorkspaceFiles v-if="!sorting" />
    <NDropdown
      :key="contextVersion"
      trigger="manual"
      :show="contextVisible"
      :x="contextPosition.x"
      :y="contextPosition.y"
      :options="contextOptions"
      :menu-props="
        () => ({ role: 'menu', 'aria-label': contextProject ? '项目操作' : '项目空间操作' })
      "
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
  </section>
  <NModal
    :show="!!deleteTarget"
    preset="dialog"
    title="删除项目"
    aria-label="删除项目"
    :show-icon="false"
    :closable="false"
    :mask-closable="!deleting"
    :close-on-esc="!deleting"
    @update:show="
      (show) => {
        if (!show) deleteTarget = null
      }
    "
  >
    <p>
      确定删除“{{ deleteTarget?.name }}”？项目内的待办将移入回收站，本地项目文件夹和文件会保留。
    </p>
    <template #action>
      <NButton :disabled="deleting" @click="deleteTarget = null">取消</NButton>
      <NButton type="error" :loading="deleting" @click="confirmDeleteProject">删除项目</NButton>
    </template>
  </NModal>
</template>
