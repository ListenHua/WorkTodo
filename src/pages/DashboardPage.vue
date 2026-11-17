<script setup lang="ts">
import { computed } from 'vue'
import { NButton, NSkeleton } from 'naive-ui'
import { useWorkspaceStore } from '../stores/workspace'
import { useUiStore } from '../stores/ui'
import { progress } from '../shared/domain'
import AppIcon from '../components/AppIcon.vue'
import EmptyState from '../components/EmptyState.vue'
import TaskList from '../components/TaskList.vue'
const workspace = useWorkspaceStore()
const ui = useUiStore()
const activeProjects = computed(() => workspace.projects.filter((p) => p.status === 'active'))
const overdue = computed(() =>
  workspace.pending.filter((t) => t.dueAt && new Date(t.dueAt).getTime() < workspace.now),
)
const metrics = computed(() => [
  {
    title: '今日计划',
    to: '/tasks?date=today',
    value: workspace.today.length,
    icon: 'calendar',
    tone: 'primary',
  },
  {
    title: '已逾期',
    to: '/tasks?view=overdue',
    value: overdue.value.length,
    icon: 'list',
    tone: 'amber',
  },
  {
    title: '进行中项目',
    to: '/projects',
    value: workspace.projects.filter((p) => p.status === 'active').length,
    icon: 'folder',
    tone: 'blue',
  },
  {
    title: '已完成待办',
    to: '/tasks?view=done',
    value: workspace.completed.filter(
      (t) => !t.projectId || activeProjects.value.some((p) => p.id === t.projectId),
    ).length,
    icon: 'check',
    tone: 'purple',
  },
])
</script>
<template>
  <section v-if="!workspace.status.rootPath" class="welcome-banner">
    <AppIcon name="folder" :size="20" />
    <span>尚未设置项目存储位置</span>
    <RouterLink to="/settings" class="text-link">
      设置存储位置 <AppIcon name="arrow" :size="17" />
    </RouterLink>
  </section>
  <div class="overview-shortcuts">
    <RouterLink to="/tasks?view=week" class="text-link"
      >未来七天<AppIcon name="arrow" :size="15"
    /></RouterLink>
  </div>
  <section class="metric-grid">
    <RouterLink v-for="metric in metrics" :key="metric.title" :to="metric.to" class="metric-card">
      <div class="metric-top">
        <span>{{ metric.title }}</span>
        <span :class="['metric-icon', metric.tone]">
          <AppIcon :name="metric.icon" :size="19" />
        </span>
      </div>
      <NSkeleton
        v-if="workspace.loading && !workspace.tasks.length"
        text
        :width="44"
        :height="40"
      />
      <strong v-else>{{ metric.value }}</strong>
    </RouterLink>
  </section>
  <section v-if="overdue.length" class="panel dashboard-overdue">
    <div class="panel-heading">
      <h2>需要关注</h2>
      <RouterLink to="/tasks?view=overdue" class="text-link">查看逾期</RouterLink>
    </div>
    <TaskList :tasks="overdue.slice(0, 3)" />
  </section>
  <div class="dashboard-columns">
    <section class="panel">
      <div class="panel-heading">
        <h2>今日待办</h2>
        <RouterLink to="/tasks?date=today" class="text-link subtle">
          查看全部 <AppIcon name="arrow" :size="15" />
        </RouterLink>
      </div>
      <TaskList v-if="workspace.today.length" :tasks="workspace.today.slice(0, 6)" />
      <EmptyState v-else title="今日暂无待办"
        ><NButton @click="ui.newTask()">新建待办</NButton></EmptyState
      >
    </section>
    <section class="panel project-overview">
      <div class="panel-heading">
        <h2>项目</h2>
        <button class="text-link" @click="ui.newProject()">
          <AppIcon name="plus" :size="15" />新建项目
        </button>
      </div>
      <div v-if="activeProjects.length" class="project-mini-grid">
        <RouterLink
          v-for="project in activeProjects.slice(0, 3)"
          :key="project.id"
          :to="{ path: '/projects', query: { project: project.id } }"
          class="project-mini"
        >
          <span
            class="project-mark"
            :style="{ color: project.color, background: project.color + '14' }"
          >
            <AppIcon name="folder" :size="21" />
          </span>
          <div>
            <h3>{{ project.name }}</h3>
            <p>{{ project.completedCount }} / {{ project.taskCount }} 项已完成</p>
          </div>
          <strong>{{ progress(project) === null ? '—' : `${progress(project)}%` }}</strong>
        </RouterLink>
      </div>
      <EmptyState v-else icon="folder" title="暂无进行中的项目"
        ><NButton @click="ui.newProject()">创建项目</NButton></EmptyState
      >
    </section>
  </div>
</template>
