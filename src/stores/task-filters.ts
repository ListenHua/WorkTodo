import { defineStore } from 'pinia'
import { computed, reactive } from 'vue'
import { defaultTaskFilters, filterTasks, type TaskFilters } from '../shared/task-filters'
import { useWorkspaceStore } from './workspace'

export const useTaskFiltersStore = defineStore('task-filters', () => {
  // In-memory only: navigating away preserves filters without writing workspace data.
  const sessions = reactive(new Map<string, TaskFilters>())
  function forWorkspace(key: string): TaskFilters {
    if (!sessions.has(key)) sessions.set(key, defaultTaskFilters())
    return sessions.get(key)!
  }
  const workspace = useWorkspaceStore()
  const workspaceKey = computed(
    () => workspace.status.bindingId || workspace.status.rootPath || 'unbound',
  )
  const current = computed(() => forWorkspace(workspaceKey.value))
  const filtered = computed(() =>
    filterTasks(workspace.tasks, workspace.projects, current.value, 'all', workspace.now),
  )
  return { forWorkspace, workspaceKey, current, filtered }
})
