import { preferences } from '../services/preferences'
import { readProjectSort, sortProjects, type ProjectSort } from '../shared/project-sort'
import { reminders } from '../services/desktop'
import { emit } from '@tauri-apps/api/event'
import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { attachments } from '../services/attachments'
import { api, desktop } from '../services/api'
import { errorMessage, isPending, isTaskForDay, localDate } from '../shared/domain'
import type {
  CreateProject,
  CreateTask,
  UpdateTask,
  Project,
  Task,
  WorkspaceStatus,
} from '../shared/types'

export const useWorkspaceStore = defineStore('workspace', () => {
  const status = ref<WorkspaceStatus>({ rootPath: null, available: false, pendingOperations: 0 })
  const projects = ref<Project[]>([])
  const projectSort = ref<ProjectSort>(readProjectSort(null))
  const orderedProjects = computed(() => sortProjects(projects.value, projectSort.value))
  const sortingProjects = ref(false)
  const tasks = ref<Task[]>([])
  const loading = ref(true)
  const error = ref('')
  const currentDay = ref(localDate())
  const now = ref(Date.now())
  function syncClock() {
    now.value = Date.now()
    currentDay.value = localDate()
  }
  const pending = computed(() =>
    tasks.value.filter(
      (t) =>
        !t.deletedAt &&
        isPending(t) &&
        (!t.projectId || projects.value.some((p) => p.id === t.projectId && p.status === 'active')),
    ),
  )
  const today = computed(() => pending.value.filter((t) => isTaskForDay(t, currentDay.value)))
  const completed = computed(() => tasks.value.filter((t) => !t.deletedAt && t.status === 'done'))

  let refreshVersion = 0
  async function refresh() {
    const version = ++refreshVersion
    loading.value = true
    error.value = ''
    syncClock()
    try {
      const [nextStatus, nextProjects, nextTasks, nextSort] = await Promise.all([
        api.getStatus(),
        api.listProjects(),
        api.listTasks(),
        preferences.get('project_sort'),
      ])
      if (version !== refreshVersion) return
      status.value = nextStatus
      projects.value = nextProjects
      projectSort.value = readProjectSort(nextSort)
      tasks.value = nextTasks
    } catch (cause) {
      if (version === refreshVersion) error.value = errorMessage(cause)
    } finally {
      if (version === refreshVersion) loading.value = false
    }
  }
  async function selectRoot() {
    status.value = await api.selectRoot()
  }
  async function createProject(input: CreateProject) {
    await api.createProject(input)
    await refresh()
    if (desktop) void emit('workspace-changed')
  }
  async function createTask(input: CreateTask) {
    const task = await api.createTask(input)
    await refresh()
    if (desktop) void emit('workspace-changed')
    return task
  }
  async function createFullTask(input: UpdateTask) {
    const task = await api.createFullTask(input)
    if (!desktop && input.reminderAt) await reminders.schedule(task.id, input.reminderAt)
    await refresh()
    if (desktop) void emit('workspace-changed')
    return task
  }
  async function updateTask(id: string, input: UpdateTask) {
    const old = tasks.value.find((t) => t.id === id)
    await api.updateTask(id, input)
    try {
      if (!desktop && old && old.projectId !== input.projectId)
        await attachments.movePreview(id, input.projectId)
      if (!desktop && input.reminderAt) await reminders.schedule(id, input.reminderAt)
    } catch (error) {
      if (!desktop && old)
        await api.updateTask(id, {
          title: old.title,
          projectId: old.projectId,
          priority: old.priority,
          planDate: old.planDate,
          status: old.status,
          description: old.description || '',
          dueAt: old.dueAt || null,
        })
      throw error
    }
    await refresh()
    if (desktop) void emit('workspace-changed')
  }
  async function trashTask(id: string, restore = false, toInbox = false) {
    if (!desktop && restore && toInbox) await attachments.movePreview(id, null)
    await api.trashTask(id, restore, toInbox)
    await refresh()
    if (desktop) void emit('workspace-changed')
  }
  async function updateProject(id: string, input: CreateProject) {
    const old = projects.value.find((p) => p.id === id)
    if (
      !desktop &&
      old &&
      input.status &&
      input.status !== old.status &&
      input.status !== 'active' &&
      (input.pendingPolicy ?? 'inbox') === 'inbox'
    ) {
      for (const task of tasks.value.filter(
        (t) => t.projectId === id && !t.deletedAt && isPending(t),
      ))
        await attachments.movePreview(task.id, null)
    }
    await api.updateProject(id, input)
    await refresh()
    if (desktop) void emit('workspace-changed')
  }
  async function saveProjectSort(input: ProjectSort) {
    if (sortingProjects.value) throw new Error('正在保存项目顺序，请稍后再试')
    const bindingId = status.value.bindingId
    const next: ProjectSort = {
      mode: input.mode,
      order: sortProjects(projects.value, input).map((project) => project.id),
    }
    sortingProjects.value = true
    try {
      await preferences.set('project_sort', JSON.stringify(next), bindingId)
      if (status.value.bindingId === bindingId) {
        projectSort.value = next
        await refresh()
      }
      if (desktop) void emit('workspace-changed')
    } finally {
      sortingProjects.value = false
    }
  }
  async function setProjectPinned(id: string, pinned: boolean) {
    await api.setProjectPinned(id, pinned)
    await refresh()
    if (desktop) void emit('workspace-changed')
  }
  async function setProjectStatus(
    id: string,
    status: Project['status'],
    policy: 'inbox' | 'complete' | 'cancel' = 'inbox',
  ) {
    if (!desktop && status !== 'active' && policy === 'inbox')
      for (const task of tasks.value.filter(
        (t) => t.projectId === id && !t.deletedAt && isPending(t),
      ))
        await attachments.movePreview(task.id, null)
    await api.setProjectStatus(id, status, policy)
    await refresh()
    if (desktop) void emit('workspace-changed')
  }
  async function toggleTask(task: Task) {
    await api.setTaskCompleted(task.id, task.status !== 'done')
    await refresh()
    if (desktop) void emit('workspace-changed')
  }
  return {
    desktop,
    status,
    projects,
    projectSort,
    orderedProjects,
    sortingProjects,
    saveProjectSort,
    tasks,
    loading,
    error,
    currentDay,
    now,
    syncClock,
    pending,
    today,
    completed,
    refresh,
    selectRoot,
    createProject,
    createTask,
    createFullTask,
    toggleTask,
    updateTask,
    trashTask,
    updateProject,
    setProjectStatus,
    setProjectPinned,
  }
})
