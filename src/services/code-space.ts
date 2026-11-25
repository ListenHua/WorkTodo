import type { CodeDirectory } from '../shared/types'
import { Channel, invoke } from '@tauri-apps/api/core'
import { desktop } from './api'
import type {
  CleanResult,
  CodeProgress,
  Dependency,
  GitInfo,
  SpaceUsage,
} from '../shared/code-space'
function channel(onProgress: (value: CodeProgress) => void) {
  const progress = new Channel<CodeProgress>()
  progress.onmessage = onProgress
  return progress
}
export const codeApi = {
  async pickDirectory(projectId: string, bindingId: string): Promise<string | null> {
    if (!desktop) throw new Error('请在桌面端选择本地代码目录')
    return invoke('code_pick_directory', { projectId, bindingId })
  },
  async saveDirectory(
    projectId: string,
    bindingId: string,
    codeId: string | null,
    name: string,
    path: string,
  ): Promise<CodeDirectory[]> {
    if (!desktop) throw new Error('请在桌面端关联本地代码目录')
    return invoke('code_save_directory', { projectId, bindingId, codeId, name, path })
  },
  async removeDirectory(
    projectId: string,
    codeId: string,
    bindingId: string,
  ): Promise<CodeDirectory[]> {
    if (!desktop) throw new Error('请在桌面端管理本地代码目录')
    return invoke('code_remove_directory', { projectId, codeId, bindingId })
  },
  async openDirectory(projectId: string, codeId: string, bindingId: string): Promise<void> {
    if (desktop) await invoke('code_open_directory', { projectId, codeId, bindingId })
  },
  async openTerminal(projectId: string, codeId: string, bindingId: string): Promise<void> {
    if (desktop) await invoke('code_open_terminal', { projectId, codeId, bindingId })
  },
  async inspect(projectId: string, codeId: string, bindingId: string): Promise<GitInfo> {
    if (!desktop)
      return {
        state: 'preview',
        message: '请在桌面端查看 Git 状态和本地目录体积',
        branch: '',
        scope: '',
        upstream: '',
        ahead: 0,
        behind: 0,
        changes: [],
        totalChanges: 0,
        conflicts: 0,
        truncated: false,
        remotes: [],
        commit: null,
      }
    return invoke('code_inspect', { projectId, codeId, bindingId })
  },
  async scan(
    projectId: string,
    codeId: string,
    bindingId: string,
    requestId: string,
    onProgress: (value: CodeProgress) => void,
  ): Promise<SpaceUsage> {
    if (!desktop) throw new Error('请在桌面端统计本地目录体积')
    return invoke('code_scan', {
      projectId,
      codeId,
      bindingId,
      requestId,
      onProgress: channel(onProgress),
    })
  },
  async reveal(projectId: string, codeId: string, bindingId: string, path: string): Promise<void> {
    if (desktop) await invoke('code_reveal', { projectId, codeId, bindingId, path })
  },
  async cancelScan(requestId: string): Promise<void> {
    if (desktop) await invoke('code_cancel_scan', { requestId })
  },
  async clean(
    projectId: string,
    codeId: string,
    bindingId: string,
    targets: Dependency[],
    onProgress: (value: CodeProgress) => void,
  ): Promise<CleanResult> {
    if (!desktop) throw new Error('请在桌面端清理本地依赖')
    return invoke('code_clean', {
      projectId,
      codeId,
      bindingId,
      targets,
      onProgress: channel(onProgress),
    })
  },
}
