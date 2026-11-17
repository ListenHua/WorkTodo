import { invoke } from '@tauri-apps/api/core'
import { api, desktop } from './api'
import { filesApi } from './files'
import type { FileSource, ProjectFile } from '../shared/files'
const refs = new Map<string, ProjectFile[]>()
export const attachments = {
  async list(taskId: string): Promise<ProjectFile[]> {
    if (desktop) return invoke('attachments_list', { taskId })
    const linked = refs.get(taskId) || []
    const existing = new Set<string>()
    for (const projectId of new Set(linked.map((file) => file.projectId))) {
      for (const file of await filesApi.list(projectId)) existing.add(file.id)
    }
    const available = linked.filter((file) => existing.has(file.id))
    refs.set(taskId, available)
    return [...available]
  },
  async link(taskId: string, fileId: string, remove = false) {
    if (desktop) return invoke<void>('attachments_link', { taskId, fileId, remove })
    const task = (await api.listTasks()).find((t) => t.id === taskId && !t.deletedAt)
    if (!task) throw new Error('待办不存在')
    const list = refs.get(taskId) || []
    if (remove) {
      refs.set(
        taskId,
        list.filter((f) => f.id !== fileId),
      )
      return
    }
    const file = (await filesApi.list(task.projectId || '__inbox__')).find((f) => f.id === fileId)
    if (!file) throw new Error('附件必须属于待办所在项目')
    if (!list.some((f) => f.id === fileId)) refs.set(taskId, [...list, file])
  },
  async import(taskId: string, source: FileSource) {
    if (desktop) {
      if (source.kind !== 'native') throw new Error('请使用桌面文件选择器')
      return invoke<ProjectFile>('attachments_import', { taskId, source: source.path })
    }
    const task = (await api.listTasks()).find((t) => t.id === taskId && !t.deletedAt)
    if (!task) throw new Error('待办不存在')
    const file = await filesApi.importFile(
      task.projectId || '__inbox__',
      source,
      crypto.randomUUID(),
      () => {},
    )
    await this.link(taskId, file.id)
    return file
  },
  async movePreview(taskId: string, projectId: string | null) {
    const copied: ProjectFile[] = []
    for (const file of await this.list(taskId)) {
      copied.push(await filesApi.copy!(file.projectId, file.id, projectId || '__inbox__'))
    }
    refs.set(taskId, copied)
  },
}
