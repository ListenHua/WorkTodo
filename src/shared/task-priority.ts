import type { Priority, Task } from './types'

// Legacy tasks without a priority share the low-priority default.
export function normalizePriority(priority: unknown): Priority {
  return priority === 'high' || priority === 'medium' ? priority : 'low'
}

const priorityRank: Record<Priority, number> = { high: 0, medium: 1, low: 2 }

export function compareTaskPriority(a: Task, b: Task): number {
  return (
    priorityRank[normalizePriority(a.priority)] - priorityRank[normalizePriority(b.priority)] ||
    b.createdAt.localeCompare(a.createdAt) ||
    a.id.localeCompare(b.id)
  )
}
