import type { Project, Task } from './types'

export function localDate(date = new Date()): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`
}
export function isPending(task: Task): boolean {
  return task.status === 'todo' || task.status === 'doing'
}
export function isTaskForDay(task: Task, day: string): boolean {
  return task.planDate === day || !!(task.dueAt && localDate(new Date(task.dueAt)) === day)
}
export function progress(project: Project): number | null {
  return project.taskCount ? Math.round((project.completedCount / project.taskCount) * 100) : null
}
export function validateProjectName(raw: string): string | null {
  const name = raw.trim()
  if (!name || [...name].length > 100) return '项目名需为 1–100 字'
  if (
    /[<>:"/\\|?*\u0000-\u001f\u007f]/.test(name) ||
    /[. ]$/.test(name) ||
    ['.', '..', '_收件箱', '_回收站'].includes(name)
  )
    return '请使用有效的文件夹名称，避免路径字符和系统保留名称'
  if (/^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(name))
    return '该名称为 Windows 保留名称，请更换'
  return null
}
export function errorMessage(error: unknown): string {
  if (
    typeof error === 'object' &&
    error !== null &&
    'message' in error &&
    typeof error.message === 'string'
  )
    return error.message
  return typeof error === 'string' ? error : '操作失败，请稍后重试'
}
