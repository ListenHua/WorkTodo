import { invoke } from '@tauri-apps/api/core'
import { previewReminders as entries } from './preview-reminders-state'
import { api, desktop } from './api'
import { globalShortcutDefaults, globalShortcutConflict } from '../shared/global-shortcuts'
export interface Reminder {
  id: string
  taskId: string
  title: string
  remindAt: string
  state: 'pending' | 'sent' | 'dismissed'
  readAt: string | null
}
export interface NotificationStatus {
  permission: 'granted' | 'denied' | 'notDetermined'
  enabled: boolean
  error: string
}
export interface DesktopSettings {
  closeToTray: boolean
  shortcut: string
  toggleShortcut: string
  middleCapture: boolean
  middleHoldMs: number
  captureTrigger: string
  notifications: boolean
}
export interface DesktopStatus {
  settings: DesktopSettings
  autostart: boolean
  shortcutError: string
  middleError: string
  middlePermission: boolean
  inputMonitoringPermission: boolean
  middleListening: boolean
  isMacos: boolean
}
const previewMacos = typeof navigator !== 'undefined' && /Mac/i.test(navigator.userAgent)
const defaults: DesktopSettings = {
  closeToTray: true,
  ...globalShortcutDefaults(previewMacos),
  middleCapture: false,
  middleHoldMs: 650,
  captureTrigger: 'MouseMiddle',
  notifications: true,
}
let previewSettings = { ...defaults }
export const reminders = {
  async list(): Promise<Reminder[]> {
    if (desktop) return invoke('reminders_list')
    const tasks = await api.listTasks()
    const projects = await api.listProjects()
    for (const entry of entries) {
      const task = tasks.find((t) => t.id === entry.taskId)
      if (task) entry.title = task.title
      if (
        !task ||
        task.deletedAt ||
        ['done', 'canceled'].includes(task.status) ||
        (task.projectId && projects.find((p) => p.id === task.projectId)?.status !== 'active')
      )
        entry.state = 'dismissed'
    }
    return entries
      .filter((r) => !tasks.find((t) => t.id === r.taskId)?.deletedAt)
      .map((r) => ({ ...r }))
  },
  async schedule(taskId: string, at: string | null) {
    if (desktop) return invoke<void>('reminders_schedule', { taskId, at })
    if (!at) return
    const task = (await api.listTasks()).find(
      (t) => t.id === taskId && !t.deletedAt && ['todo', 'doing'].includes(t.status),
    )
    if (!task) throw new Error('只有未完成待办可以设置提醒')
    if (new Date(at).getTime() <= Date.now()) throw new Error('请选择未来的提醒时间')
    if (!entries.some((r) => r.taskId === taskId && r.remindAt === at && r.state === 'pending'))
      entries.unshift({
        id: crypto.randomUUID(),
        taskId,
        title: task.title,
        remindAt: at,
        state: 'pending',
        readAt: null,
      })
  },
  async action(id: string, action: 'read' | 'dismiss' | 'snooze' | 'retry') {
    if (desktop) return invoke<void>('reminders_action', { id, action })
    const entry = entries.find((r) => r.id === id)
    if (!entry) throw new Error('提醒不存在')
    if (action === 'retry') {
      const tasks = await api.listTasks()
      const projects = await api.listProjects()
      const task = tasks.find((task) => task.id === entry.taskId)
      if (
        !task ||
        task.deletedAt ||
        !['todo', 'doing'].includes(task.status) ||
        (task.projectId &&
          projects.find((project) => project.id === task.projectId)?.status !== 'active')
      )
        throw new Error('只有未完成待办可以再次提醒')
      entry.state = 'pending'
      entry.readAt = null
      return
    }
    if (action === 'snooze')
      await this.schedule(entry.taskId, new Date(Date.now() + 600000).toISOString())
    entry.readAt = new Date().toISOString()
    if (action !== 'read') entry.state = 'dismissed'
  },
}
export interface StorageInfo {
  mode: 'legacy' | 'portable'
  root: string | null
  configPath: string | null
  database: string
  state: string
  error: string | null
  bindingId: string | null
  recoveryCount: number
  counts?: { projects: number; tasks: number; files: number } | null
}
export const desktopApi = {
  async storage(): Promise<StorageInfo | null> {
    return desktop ? invoke('storage_info') : null
  },
  async recovery(action: 'export' | 'restore' | 'diagnostic'): Promise<string | null> {
    return desktop ? invoke('storage_recovery_action', { action }) : null
  },
  async clearCache() {
    if (desktop) await invoke('storage_clear_cache')
  },
  async status(): Promise<DesktopStatus> {
    return desktop
      ? invoke('desktop_status')
      : {
          settings: { ...previewSettings },
          autostart: false,
          shortcutError: '',
          middleError: '',
          middlePermission: false,
          inputMonitoringPermission: false,
          middleListening: false,
          isMacos: previewMacos,
        }
  },
  async save(input: DesktopSettings) {
    if (
      !Number.isInteger(input.middleHoldMs) ||
      input.middleHoldMs < 300 ||
      input.middleHoldMs > 3000
    )
      throw new Error('长按时间需为 300–3000 毫秒的整数')
    if (desktop) await invoke('desktop_save', { input })
    else {
      const conflict = globalShortcutConflict(input, previewMacos)
      if (conflict) throw new Error(conflict)
      previewSettings = { ...input }
    }
  },
  async middlePermission(kind: 'accessibility' | 'input-monitoring') {
    if (desktop) await invoke('middle_permission', { kind })
  },
  async retryMiddle() {
    if (desktop) await invoke('middle_retry')
  },
  async autostart(enabled: boolean) {
    if (!desktop) throw new Error('请在桌面端设置开机启动')
    await invoke('desktop_autostart', { enabled })
  },
  async notificationStatus(): Promise<NotificationStatus> {
    return desktop
      ? invoke('notification_status')
      : { permission: 'notDetermined', enabled: false, error: '' }
  },
  async notificationRequest(): Promise<NotificationStatus['permission']> {
    if (!desktop) throw new Error('请在桌面端开启系统通知')
    return invoke('notification_request')
  },
  async notificationSettings() {
    if (!desktop) throw new Error('请在桌面端打开系统通知设置')
    await invoke('notification_settings')
  },
  async notificationTest() {
    if (!desktop) throw new Error('请在桌面端测试系统通知')
    await invoke('notification_test')
  },
  async data(action: 'import'): Promise<string | null> {
    if (!desktop) throw new Error('请在桌面端管理本地数据')
    return invoke('data_action', { action })
  },
  async pending(): Promise<{ id: string; kind: string; target: string; createdAt: string }[]> {
    return desktop ? invoke('data_pending') : []
  },
  async recover(id: string) {
    await invoke('data_recover', { id })
  },
}
