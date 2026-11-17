import type { Task, Project, Priority, TaskStatus } from './types'
import { isPending, isTaskForDay, localDate } from './domain'
export interface TaskFilters {
  search: string
  status: 'pending' | 'all' | TaskStatus
  project: string
  priority: '' | Priority
  date: 'all' | 'overdue' | 'week' | 'today' | 'unplanned'
  sort: 'created' | 'due' | 'priority' | 'title'
}
export function filterTasks(
  tasks: Task[],
  projects: Project[],
  filters: TaskFilters,
  scope: string,
  today: string,
  now = Date.now(),
) {
  const active = new Set(
    projects.filter((project) => project.status === 'active').map((project) => project.id),
  )
  const week = new Date(`${today}T12:00:00`)
  week.setDate(week.getDate() + 6)
  const weekEnd = localDate(week)
  const rank: Record<Priority, number> = { high: 0, medium: 1, low: 2, none: 3 }
  return tasks
    .filter((task) => {
      if (scope === 'trash') {
        if (!task.deletedAt) return false
      } else {
        if (task.deletedAt || (task.projectId && !active.has(task.projectId))) return false
        if (
          filters.status === 'pending'
            ? !isPending(task)
            : filters.status !== 'all' && task.status !== filters.status
        )
          return false
      }
      if (
        filters.project === 'none'
          ? !!task.projectId
          : filters.project && task.projectId !== filters.project
      )
        return false
      if (filters.priority && task.priority !== filters.priority) return false
      if (
        filters.date === 'overdue' &&
        !(isPending(task) && task.dueAt && new Date(task.dueAt).getTime() < now)
      )
        return false
      const taskDate = task.planDate || (task.dueAt ? localDate(new Date(task.dueAt)) : null)
      if (filters.date === 'today' && !isTaskForDay(task, today)) return false
      if (filters.date === 'week' && !(taskDate && taskDate >= today && taskDate <= weekEnd))
        return false
      if (filters.date === 'unplanned' && task.planDate) return false
      return `${task.title} ${task.description || ''}`
        .toLocaleLowerCase()
        .includes(filters.search.trim().toLocaleLowerCase())
    })
    .sort((a, b) => {
      if (filters.sort === 'due')
        return (
          (a.dueAt ? new Date(a.dueAt).getTime() : Infinity) -
            (b.dueAt ? new Date(b.dueAt).getTime() : Infinity) ||
          b.createdAt.localeCompare(a.createdAt)
        )
      if (filters.sort === 'priority')
        return rank[a.priority] - rank[b.priority] || b.createdAt.localeCompare(a.createdAt)
      if (filters.sort === 'title') return a.title.localeCompare(b.title, 'zh-CN')
      return b.createdAt.localeCompare(a.createdAt)
    })
}
