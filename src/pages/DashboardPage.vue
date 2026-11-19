<script setup lang="ts">
import { compareTaskPriority } from '../shared/task-priority'
import { computed, nextTick, ref } from 'vue'
import { NSkeleton, NTabs, NTab } from 'naive-ui'
import { useRouter } from 'vue-router'
import { useWorkspaceStore } from '../stores/workspace'
import { useUiStore } from '../stores/ui'
import { overviewTasks, projectDistribution, taskTrend } from '../shared/dashboard'
import AppIcon from '../components/AppIcon.vue'
import EmptyState from '../components/EmptyState.vue'
import TaskList from '../components/TaskList.vue'
import AnimatedNumber from '../components/AnimatedNumber.vue'
import ProjectBarChart from '../components/ProjectBarChart.vue'
import TaskTrendChart from '../components/TaskTrendChart.vue'

const workspace = useWorkspaceStore()
const ui = useUiStore()
const router = useRouter()
const taskView = ref<'pending' | 'today'>('pending')
async function taskViewKeydown(event: KeyboardEvent) {
  if (event.isComposing) return
  if (!['ArrowLeft', 'ArrowRight', 'Home', 'End', 'Enter', ' '].includes(event.key)) return
  event.preventDefault()
  if (event.key === 'Home') taskView.value = 'pending'
  else if (event.key === 'End') taskView.value = 'today'
  else if (event.key.startsWith('Arrow'))
    taskView.value = taskView.value === 'pending' ? 'today' : 'pending'
  await nextTick()
  document.getElementById(`overview-${taskView.value}-tab`)?.focus()
}
const activeProjects = computed(() => workspace.projects.filter((p) => p.status === 'active'))
const scopedTasks = computed(() => overviewTasks(workspace.tasks, workspace.projects))
const distribution = computed(() => projectDistribution(workspace.projects, workspace.tasks))
const trend = computed(() => taskTrend(scopedTasks.value, workspace.currentDay))
const initialLoading = computed(
  () => workspace.loading && !workspace.tasks.length && !workspace.projects.length,
)
const displayedTasks = computed(() =>
  [...(taskView.value === 'pending' ? workspace.pending : workspace.today)].sort(
    compareTaskPriority,
  ),
)
const metrics = computed(() => [
  {
    title: '待办事项',
    to: '/tasks',
    value: workspace.pending.length,
    icon: 'list',
  },
  {
    title: '今日待办',
    to: '/tasks?date=today',
    value: workspace.today.length,
    icon: 'calendar',
  },
  {
    title: '进行中项目',
    to: '/projects',
    value: activeProjects.value.length,
    icon: 'folder',
  },
  {
    title: '已完成待办',
    to: '/tasks?view=done',
    value: scopedTasks.value.filter((t) => t.status === 'done').length,
    icon: 'check',
  },
])
function openProject(id: string) {
  void router.push({ path: '/projects', query: { project: id } })
}
</script>

<template>
  <section v-if="!workspace.status.rootPath && !workspace.loading" class="welcome-banner">
    <AppIcon name="folder" :size="20" /><span>尚未设置项目存储位置</span>
    <RouterLink to="/settings" class="text-link"
      >设置存储位置 <AppIcon name="arrow" :size="17"
    /></RouterLink>
  </section>
  <div class="dashboard">
    <section class="metric-grid" aria-label="概览统计">
      <RouterLink v-for="metric in metrics" :key="metric.title" :to="metric.to" class="metric-card">
        <div class="metric-top">
          <span>{{ metric.title }}</span><span class="metric-icon primary"
            ><AppIcon :name="metric.icon" :size="18"
          /></span>
        </div>
        <NSkeleton v-if="initialLoading" text :width="64" :height="40" />
        <AnimatedNumber v-else :value="metric.value" />
      </RouterLink>
    </section>
    <div class="dashboard-charts">
      <ProjectBarChart
        :projects="distribution"
        :loading="initialLoading"
        @select="openProject"
        @browse="router.push('/projects')"
      />
      <TaskTrendChart
        :days="trend"
        :today="workspace.currentDay"
        :loading="initialLoading"
        :read-only="!!workspace.status.readOnly"
        @create="ui.newTask()"
      />
    </div>
    <div class="dashboard-details">
      <section class="panel dashboard-tasks" aria-label="待办概况">
        <div class="panel-heading dashboard-panel-heading">
          <NTabs
            v-model:value="taskView"
            type="segment"
            size="small"
            class="task-view-tabs"
            role="tablist"
            aria-label="概览待办视图"
          >
            <NTab
              name="pending"
              id="overview-pending-tab"
              role="tab"
              :aria-selected="taskView === 'pending'"
              :tabindex="taskView === 'pending' ? 0 : -1"
              aria-controls="overview-task-panel"
              @keydown="taskViewKeydown"
              >待办事项 <span class="task-view-count">{{ workspace.pending.length }}</span></NTab
            >
            <NTab
              name="today"
              id="overview-today-tab"
              role="tab"
              :aria-selected="taskView === 'today'"
              :tabindex="taskView === 'today' ? 0 : -1"
              aria-controls="overview-task-panel"
              @keydown="taskViewKeydown"
              >今日待办 <span class="task-view-count">{{ workspace.today.length }}</span></NTab
            >
          </NTabs>
          <div class="dashboard-list-actions">
            <RouterLink to="/tasks?view=week" class="text-link subtle future-tasks-link"
              >未来七天</RouterLink
            >
            <RouterLink
              :to="taskView === 'pending' ? '/tasks' : '/tasks?date=today'"
              class="text-link subtle"
              >查看全部 <AppIcon name="arrow" :size="15"
            /></RouterLink>
          </div>
        </div>
        <div
          class="dashboard-list-body"
          id="overview-task-panel"
          role="tabpanel"
          :aria-labelledby="`overview-${taskView}-tab`"
        >
          <div v-if="initialLoading" class="dashboard-list-loading">
            <NSkeleton v-for="i in 3" :key="i" text :height="42" />
          </div>
          <TaskList v-else-if="displayedTasks.length" :tasks="displayedTasks" layout="list" />
          <EmptyState
            v-else
            kind="tasks"
            :title="taskView === 'pending' ? '暂无待办事项' : '今日暂无待办'"
            action-label="点击新增"
            :disabled="!!workspace.status.readOnly"
            @action="ui.newTask()"
          />
        </div>
      </section>
      <section class="panel dashboard-projects" aria-labelledby="project-progress-title">
        <div class="panel-heading dashboard-panel-heading">
          <div class="dashboard-heading-inline">
            <h2 id="project-progress-title">项目进度</h2>
            <span>{{ activeProjects.length }}</span>
          </div>
          <RouterLink to="/projects" class="text-link subtle">
            查看全部 <AppIcon name="arrow" :size="15" />
          </RouterLink>
        </div>
        <div class="dashboard-list-body">
          <div v-if="initialLoading" class="dashboard-list-loading">
            <NSkeleton v-for="i in 3" :key="i" text :height="42" />
          </div>
          <div v-else-if="distribution.length" class="dashboard-project-list">
            <RouterLink
              v-for="project in distribution"
              :key="project.id"
              :to="{ path: '/projects', query: { project: project.id } }"
              class="dashboard-project-row"
            >
              <span
                class="project-mark"
                :style="{ color: project.color, background: project.color + '14' }"
                ><AppIcon name="folder" :size="19"
              /></span>
              <div class="dashboard-project-info">
                <h3>{{ project.name }}</h3>
                <p>{{ project.completed }} / {{ project.total }} 项已完成</p>
                <div class="dashboard-progress-track" aria-hidden="true">
                  <span
                    :style="{
                      width: `${project.total ? (project.completed / project.total) * 100 : 0}%`,
                    }"
                  />
                </div>
              </div>
              <strong>{{
                project.total ? `${Math.round((project.completed / project.total) * 100)}%` : '—'
              }}</strong>
            </RouterLink>
          </div>
          <EmptyState
            v-else
            kind="projects"
            title="暂无进行中的项目"
            action-label="点击查看项目"
            action-to="/projects"
          />
        </div>
      </section>
    </div>
  </div>
</template>
