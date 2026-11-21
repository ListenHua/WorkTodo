import type { Task, Project, Priority, TaskStatus } from './types'
import { isPending, localDate } from './domain'
import { compareTaskPriority, normalizePriority } from './task-priority'

export type TaskDateField = 'plan' | 'due' | 'completed'
export type TaskDateRange = [string, string]
export interface TaskFilters {
  search: string
  status: 'pending' | 'all' | 'overdue' | TaskStatus
  project: string
  priorities: Priority[]
  date: 'all' | 'range'
  dateField: TaskDateField
  range: TaskDateRange | null
  sort: 'created' | 'plan' | 'due' | 'priority' | 'title'
}
export function defaultTaskFilters(): TaskFilters {
  return {
    search: '',
    status: 'pending',
    project: '',
    priorities: ['high', 'medium', 'low'],
    date: 'all',
    dateField: 'plan',
    range: null,
    sort: 'priority',
  }
}

export const taskDateFieldLabels: Record<TaskDateField, string> = {
  plan: '计划日期',
  due: '截止日期',
  completed: '完成时间',
}

function offsetDay(day: string, offset: number) {
  const date = new Date(`${day}T12:00:00`)
  date.setDate(date.getDate() + offset)
  return localDate(date)
}
export function taskDateShortcuts(today: string): Record<string, [number, number]> {
  const date = new Date(`${today}T12:00:00`)
  const monday = offsetDay(today, -((date.getDay() + 6) % 7))
  const first = new Date(date.getFullYear(), date.getMonth(), 1)
  const last = new Date(date.getFullYear(), date.getMonth() + 1, 0)
  const timestamp = (day: string) => new Date(`${day}T00:00:00`).getTime()
  const range = (start: string, end = start): [number, number] => [timestamp(start), timestamp(end)]
  return {
    今天: range(today),
    明天: range(offsetDay(today, 1)),
    本周: range(monday, offsetDay(monday, 6)),
    未来七天: range(today, offsetDay(today, 6)),
    本月: range(localDate(first), localDate(last)),
  }
}

// Explicit navigation from overview takes precedence over the remembered filters.
export function taskFilterEntry(query: Record<string, unknown>, today: string): TaskFilters | null {
  if (!['view', 'date', 'project'].some((key) => typeof query[key] === 'string')) return null
  const filters = defaultTaskFilters()
  if (['pending', 'all', 'doing', 'done', 'canceled', 'overdue'].includes(String(query.view)))
    filters.status = query.view as TaskFilters['status']
  const date = query.date || query.view
  if (date === 'today' || date === 'week') {
    filters.date = 'range'
    filters.dateField = 'plan'
    filters.range = [today, date === 'week' ? offsetDay(today, 6) : today]
  } else if (date === 'overdue') {
    filters.status = 'overdue'
  }
  if (typeof query.project === 'string')
    filters.project = query.project === 'inbox' ? 'none' : query.project
  return filters
}

function timestampDate(value: string | null | undefined) {
  if (!value) return null
  const date = new Date(value)
  return Number.isFinite(date.getTime()) ? localDate(date) : null
}
export function filterTasks(
  tasks: Task[],
  projects: Project[],
  filters: TaskFilters,
  scope: string,
  now = Date.now(),
) {
  const active = new Set(
    projects.filter((project) => project.status === 'active').map((project) => project.id),
  )
  const search = filters.search.trim().toLocaleLowerCase()
  return tasks
    .filter((task) => {
      if (scope === 'trash') {
        if (!task.deletedAt) return false
      } else {
        if (task.deletedAt || (task.projectId && !active.has(task.projectId))) return false
        if (
          filters.status === 'overdue'
            ? !(isPending(task) && task.dueAt && new Date(task.dueAt).getTime() < now)
            : filters.status === 'pending'
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
      if (!filters.priorities.includes(normalizePriority(task.priority))) return false
      if (filters.date === 'range' && filters.range) {
        const [start, end] = filters.range
        const inside = (date: string | null) => !!date && date >= start && date <= end
        if (
          filters.dateField === 'plan'
            ? !inside(task.planDate)
            : filters.dateField === 'due'
              ? !inside(timestampDate(task.dueAt))
              : task.status !== 'done' || !inside(timestampDate(task.completedAt))
        )
          return false
      }
      return `${task.title} ${task.description || ''}`.toLocaleLowerCase().includes(search)
    })
    .sort((a, b) => {
      const newest = () => b.createdAt.localeCompare(a.createdAt) || a.id.localeCompare(b.id)
      if (filters.sort === 'due' || filters.sort === 'plan') {
        const time = (task: Task) => {
          const value = filters.sort === 'plan' ? task.planDate : task.dueAt
          const timestamp = value ? new Date(value).getTime() : NaN
          return Number.isFinite(timestamp) ? timestamp : Infinity
        }
        return time(a) - time(b) || newest()
      }
      if (filters.sort === 'priority') return compareTaskPriority(a, b)
      if (filters.sort === 'title') return a.title.localeCompare(b.title, 'zh-CN') || newest()
      return newest()
    })
}
