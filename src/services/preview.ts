import type {
  CreateProject,
  CreateTask,
  Project,
  Task,
  WorkspaceApi,
  WorkspaceStatus,
} from '../shared/types'
import { normalizePriority } from '../shared/task-priority'
import { cancelPreviewReminders } from './preview-reminders-state'
import { validateProjectName } from '../shared/domain'
import { validateProjectLinks } from '../shared/project-links'
import { validateProjectTags } from '../shared/project-tags'

// Explicit, session-only preview. No localStorage and no filesystem claims.
export function createPreviewApi(): WorkspaceApi {
  let workspace: WorkspaceStatus = { rootPath: null, available: false, pendingOperations: 0 }
  const projects: Project[] = []
  const tasks: Task[] = []
  return {
    async createFullTask(input) {
      if (
        input.reminderAt &&
        (new Date(input.reminderAt).getTime() <= Date.now() ||
          !['todo', 'doing'].includes(input.status))
      )
        throw new Error('请为未完成待办选择未来的提醒时间')
      if (input.dueAt && !Number.isFinite(new Date(input.dueAt).getTime()))
        throw new Error('截止时间无效')
      const task = await this.createTask(input)
      try {
        return await this.updateTask(task.id, input)
      } catch (error) {
        tasks.splice(
          tasks.findIndex((t) => t.id === task.id),
          1,
        )
        throw error
      }
    },
    async updateTask(id, input) {
      if (
        input.reminderAt &&
        (new Date(input.reminderAt).getTime() <= Date.now() ||
          !['todo', 'doing'].includes(input.status))
      )
        throw new Error('请为未完成待办选择未来的提醒时间')
      const task = tasks.find((t) => t.id === id && !t.deletedAt)
      if (!task) throw new Error('待办不存在或已删除')
      if (!input.title.trim() || [...input.title.trim()].length > 200)
        throw new Error('待办标题需为 1–200 字')
      if (
        input.projectId &&
        !projects.some(
          (p) => p.id === input.projectId && (p.status === 'active' || task.projectId === p.id),
        )
      )
        throw new Error('请选择进行中的项目')
      if (input.dueAt && !Number.isFinite(new Date(input.dueAt).getTime()))
        throw new Error('截止时间无效')
      if (
        [...input.description].length > 10000 ||
        (input.priority !== undefined &&
          !['none', 'low', 'medium', 'high'].includes(input.priority)) ||
        !['todo', 'doing', 'done', 'canceled'].includes(input.status)
      )
        throw new Error('待办内容或状态无效')
      if (
        input.planDate &&
        (!/^\d{4}-\d{2}-\d{2}$/.test(input.planDate) ||
          !Number.isFinite(Date.parse(input.planDate)) ||
          new Date(input.planDate).toISOString().slice(0, 10) !== input.planDate)
      )
        throw new Error('计划日期无效')
      const completedAt =
        input.status === 'done'
          ? task.status === 'done'
            ? (task.completedAt ?? null)
            : new Date().toISOString()
          : null
      Object.assign(task, input, {
        title: input.title.trim(),
        completedAt,
        priority: normalizePriority(input.priority),
      })
      if (['done', 'canceled'].includes(task.status)) cancelPreviewReminders(task.id)
      return { ...task }
    },
    async trashTask(id, restore, toInbox = false) {
      const task = tasks.find((t) => t.id === id)
      if (!task) throw new Error('待办不存在')
      if (
        restore &&
        task.projectId &&
        projects.find((p) => p.id === task.projectId)?.status !== 'active' &&
        ['todo', 'doing'].includes(task.status) &&
        !toInbox
      )
        throw new Error('原项目已归档，请恢复为无项目')
      task.deletedAt = restore ? null : new Date().toISOString()
      if (!restore) cancelPreviewReminders(task.id)
      if (restore && toInbox) task.projectId = null
    },
    async updateProject(id, input) {
      const project = projects.find((p) => p.id === id)
      if (!project) throw new Error('项目不存在')
      const invalid = validateProjectName(input.name)
      if (invalid) throw new Error(invalid)
      const name = input.name.trim()
      if (
        projects.some(
          (p) =>
            p.id !== id &&
            p.name.normalize('NFC').toLowerCase() === name.normalize('NFC').toLowerCase(),
        )
      )
        throw new Error('已存在同名项目')
      const links = validateProjectLinks(input.links ?? project.links ?? [])
      const tags = validateProjectTags(input.tags ?? project.tags ?? [])
      const status = input.status ?? project.status
      if (
        !['active', 'completed', 'archived'].includes(status) ||
        !['inbox', 'complete', 'cancel'].includes(input.pendingPolicy ?? 'inbox')
      )
        throw new Error('项目状态或未完成待办处理方式无效')
      if (status !== project.status) await this.setProjectStatus(id, status, input.pendingPolicy)
      Object.assign(project, {
        name,
        description: input.description,
        color: input.color,
        links,
        tags,
        directoryRelpath: name,
      })
      return { ...project }
    },
    async deleteProject(id) {
      const index = projects.findIndex((project) => project.id === id)
      if (index < 0) throw new Error('项目不存在')
      const now = new Date().toISOString()
      for (const task of tasks.filter((task) => task.projectId === id)) {
        task.projectId = null
        task.deletedAt = task.deletedAt || now
        cancelPreviewReminders(task.id)
      }
      projects.splice(index, 1)
    },
    async setProjectStatus(id, status, policy = 'inbox') {
      const project = projects.find((p) => p.id === id)
      if (!project) throw new Error('项目不存在')
      if (status !== 'active')
        for (const task of tasks.filter((t) => t.projectId === id)) cancelPreviewReminders(task.id)
      if (status !== 'active')
        for (const task of tasks.filter(
          (t) => t.projectId === id && !t.deletedAt && ['todo', 'doing'].includes(t.status),
        )) {
          if (policy === 'inbox') task.projectId = null
          else {
            task.status = policy === 'complete' ? 'done' : 'canceled'
            task.completedAt = policy === 'complete' ? new Date().toISOString() : null
          }
        }
      project.status = status
    },
    async setProjectPinned(id, pinned) {
      const project = projects.find((p) => p.id === id)
      if (!project) throw new Error('项目不存在')
      project.pinned = pinned
    },
    async getStatus() {
      return { ...workspace }
    },
    async selectRoot() {
      if (projects.length) throw new Error('已有项目时需要目录迁移；框架阶段暂不支持直接切换')
      workspace = { rootPath: '/preview/projects', available: true, pendingOperations: 0 }
      return { ...workspace }
    },
    async listProjects() {
      return projects.map((p) => {
        const related = tasks.filter(
          (t) => t.projectId === p.id && t.status !== 'canceled' && !t.deletedAt,
        )
        return {
          ...p,
          taskCount: related.length,
          completedCount: related.filter((t) => t.status === 'done').length,
        }
      })
    },
    async createProject(input: CreateProject) {
      if (input.directoryMerge) throw new Error('请在桌面端关联已有文件夹')
      if (!workspace.rootPath) throw new Error('请先选择存储位置')
      const invalid = validateProjectName(input.name)
      if (invalid) throw new Error(invalid)
      const name = input.name.trim()
      if (
        projects.some(
          (p) => p.name.normalize('NFC').toLowerCase() === name.normalize('NFC').toLowerCase(),
        )
      )
        throw new Error('已存在同名项目')
      const links = validateProjectLinks(input.links ?? [])
      const tags = validateProjectTags(input.tags ?? [])
      const project: Project = {
        ...input,
        links,
        tags,
        name,
        directoryRelpath: name,
        id: crypto.randomUUID(),
        status: 'active',
        pinned: false,
        createdAt: new Date().toISOString(),
        taskCount: 0,
        completedCount: 0,
      }
      projects.unshift(project)
      return { ...project }
    },
    async listTasks() {
      return tasks.map((t) => ({ ...t }))
    },
    async createTask(input: CreateTask) {
      const title = input.title.trim()
      if (!title || [...title].length > 200) throw new Error('待办标题需为 1–200 字')
      if (
        input.projectId &&
        !projects.some((p) => p.id === input.projectId && p.status === 'active')
      )
        throw new Error('项目不存在')
      if (
        input.priority !== undefined &&
        !['none', 'low', 'medium', 'high'].includes(input.priority)
      )
        throw new Error('优先级无效')
      const task: Task = {
        ...input,
        priority: normalizePriority(input.priority),
        title,
        id: crypto.randomUUID(),
        status: 'todo',
        completedAt: null,
        createdAt: new Date().toISOString(),
      }
      tasks.unshift(task)
      return { ...task }
    },
    async setTaskCompleted(id, completed) {
      const task = tasks.find((t) => t.id === id)
      if (!task) throw new Error('待办不存在')
      task.completedAt = completed
        ? task.status === 'done'
          ? (task.completedAt ?? null)
          : new Date().toISOString()
        : null
      task.status = completed ? 'done' : 'todo'
      if (completed) cancelPreviewReminders(task.id)
    },
    async openProjectFolder() {
      throw new Error('浏览器预览不会访问本地文件；请在 Tauri 桌面端打开项目文件夹')
    },
  }
}
