import { Channel, invoke } from '@tauri-apps/api/core'
import { api, desktop } from './api'
import { createFilesPreview } from './files-preview'
import { imageMime } from '../shared/files'
import type { ImportProgress, ProjectFilesApi } from '../shared/files'

const nativeApi: ProjectFilesApi = {
  list: (projectId) => invoke('files_list', { projectId }),
  scan: (projectId) => invoke('files_scan', { projectId }),
  pick: () => invoke('files_pick'),
  async importFile(projectId, source, requestId, onProgress) {
    if (source.kind !== 'native') throw new Error('请通过桌面文件选择器或拖入文件添加')
    const channel = new Channel<ImportProgress>()
    channel.onmessage = onProgress
    return invoke('files_import', {
      projectId,
      source: source.path,
      requestId,
      onProgress: channel,
    })
  },
  remove: (projectId, id) => invoke('files_delete', { projectId, id }),
  cancel: (requestId) => invoke('files_cancel_import', { requestId }),
  async preview(projectId, file) {
    const buffer = await invoke<ArrayBuffer>('files_preview', { projectId, id: file.id })
    return new Blob([buffer], { type: imageMime(file.name) })
  },
  open: (projectId, id, reveal) => invoke('files_open', { projectId, id, reveal }),
}
export const filesApi = desktop
  ? nativeApi
  : createFilesPreview(
      async (id) =>
        id === '__inbox__' || (await api.listProjects()).some((project) => project.id === id),
    )

export const fileVersions = {
  async changed(projectId: string): Promise<string[]> {
    return desktop ? invoke('files_changed', { projectId }) : []
  },
  async accept(projectId: string, ids: string[]) {
    if (desktop) await invoke('files_accept_changes', { projectId, ids })
  },
}
