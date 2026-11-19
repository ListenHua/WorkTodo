import type { Task, UpdateTask } from './types'
import { normalizePriority } from './task-priority'

export interface TaskFormValue {
  title: string
  projectId: string
  description: string
  planDate: string
  priority: UpdateTask['priority']
  status: UpdateTask['status']
  dueAt: string
  reminderAt: string
}
function localTime(value?: string | null) {
  if (!value) return ''
  const date = new Date(value)
  if (!Number.isFinite(date.getTime())) return ''
  return new Date(date.getTime() - date.getTimezoneOffset() * 60000).toISOString().slice(0, 16)
}
export function taskFormValue(task?: Task | null, projectId?: string | null): TaskFormValue {
  return {
    title: task?.title || '',
    projectId: task ? task.projectId || '' : projectId || '',
    description: task?.description || '',
    planDate: task?.planDate || '',
    priority: normalizePriority(task?.priority),
    status: task?.status || 'todo',
    dueAt: localTime(task?.dueAt),
    reminderAt: '',
  }
}
export function taskFormInput(form: TaskFormValue, imageOnly = false): UpdateTask {
  const title = form.title.trim() || (imageOnly ? '图片待办' : '')
  if (!title || [...title].length > 200) throw new Error('请输入 1–200 字的待办标题')
  return {
    title,
    projectId: form.projectId || null,
    description: form.description,
    planDate: form.planDate || null,
    priority: form.priority,
    status: form.status,
    dueAt: form.dueAt ? new Date(form.dueAt).toISOString() : null,
    reminderAt: form.reminderAt ? new Date(form.reminderAt).toISOString() : null,
  }
}
