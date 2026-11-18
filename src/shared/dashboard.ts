import { localDate } from './domain'
import type { Project, Task } from './types'

export interface ProjectDistribution {
  id: string
  name: string
  color: string
  completed: number
  pending: number
  total: number
}
export interface TrendDay {
  date: string
  label: string
  created: number
  completed: number
}

// Match the overview: active projects and tasks without a project, excluding trash/cancellations.
export function overviewTasks(tasks: Task[], projects: Project[]): Task[] {
  const activeIds = new Set(projects.filter((p) => p.status === 'active').map((p) => p.id))
  return tasks.filter(
    (t) => !t.deletedAt && t.status !== 'canceled' && (!t.projectId || activeIds.has(t.projectId)),
  )
}

export function projectDistribution(projects: Project[], tasks: Task[]): ProjectDistribution[] {
  const rows = new Map<string, ProjectDistribution>(
    projects
      .filter((p) => p.status === 'active')
      .map((p) => [
        p.id,
        { id: p.id, name: p.name, color: p.color, completed: 0, pending: 0, total: 0 },
      ]),
  )
  for (const task of overviewTasks(tasks, projects)) {
    const row = task.projectId ? rows.get(task.projectId) : undefined
    if (!row) continue
    row.total++
    if (task.status === 'done') row.completed++
    else row.pending++
  }
  return [...rows.values()].sort(
    (a, b) => b.total - a.total || a.name.localeCompare(b.name, 'zh-CN'),
  )
}

export function taskTrend(tasks: Task[], today: string): TrendDay[] {
  const center = new Date(`${today}T12:00:00`)
  const result: TrendDay[] = Array.from({ length: 7 }, (_, i) => {
    const date = new Date(center)
    date.setDate(center.getDate() - 3 + i)
    return {
      date: localDate(date),
      label: `${date.getMonth() + 1}/${date.getDate()}`,
      created: 0,
      completed: 0,
    }
  })
  const byDate = new Map(result.map((day) => [day.date, day]))
  function bucket(timestamp: string | null | undefined) {
    if (!timestamp) return undefined
    const date = new Date(timestamp)
    return Number.isFinite(date.getTime()) ? byDate.get(localDate(date)) : undefined
  }
  for (const task of tasks) {
    if (task.deletedAt || task.status === 'canceled') continue
    const created = bucket(task.createdAt)
    if (created) created.created++
    // Missing legacy completion dates stay unknown; creation time is never a substitute.
    const completed = task.status === 'done' ? bucket(task.completedAt) : undefined
    if (completed) completed.completed++
  }
  return result
}

export function chartScale(maximum: number) {
  const roughStep = Math.max(1, maximum / 4)
  const magnitude = 10 ** Math.floor(Math.log10(roughStep))
  const step = [1, 2, 5, 10].map((n) => n * magnitude).find((n) => n >= roughStep) ?? magnitude * 10
  return { maximum: step * 4, ticks: [0, step, step * 2, step * 3, step * 4] }
}
