import { emit } from '@tauri-apps/api/event'
import type { CodeDirectory } from '../shared/types'
import { defineStore } from 'pinia'
import { reactive, watch } from 'vue'
import { codeApi } from '../services/code-space'
import { desktop } from '../services/api'
import { errorMessage } from '../shared/domain'
import type { CodeProgress, GitInfo, SpaceUsage } from '../shared/code-space'
import { useWorkspaceStore } from './workspace'
interface CodeEntry {
  projectId: string
  codeId: string
  path: string
  git: GitInfo | null
  usage: SpaceUsage | null
  gitLoading: boolean
  scanning: boolean
  canceling: boolean
  requestId: string | null
  progress: CodeProgress | null
  gitError: string
  sizeError: string
}
export const useCodeSpaceStore = defineStore('code-space', () => {
  const workspace = useWorkspaceStore()
  const selected = reactive<Record<string, string>>({})
  const entries = reactive<Record<string, CodeEntry>>({})
  const scans = new Map<string, { item: CodeEntry; promise: Promise<void> }>()
  const inspections = new Map<string, { item: CodeEntry; promise: Promise<void> }>()
  const key = (projectId: string, codeId: string) => `${projectId}:${codeId}`
  function directoryPath(projectId: string, codeId: string) {
    return workspace.projects
      .find((project) => project.id === projectId)
      ?.codeDirectories?.find((code) => code.id === codeId)?.path
  }
  function entry(projectId: string, codeId: string): CodeEntry {
    const id = key(projectId, codeId)
    const path = directoryPath(projectId, codeId) || ''
    if (entries[id] && entries[id]!.path !== path) {
      cancel(projectId, codeId)
      delete entries[id]
    }
    entries[id] ||= {
      projectId,
      codeId,
      path,
      git: null,
      usage: null,
      gitLoading: false,
      scanning: false,
      canceling: false,
      requestId: null,
      progress: null,
      gitError: '',
      sizeError: '',
    }
    return entries[id]!
  }
  const alive = (item: CodeEntry) => entries[key(item.projectId, item.codeId)] === item
  async function inspect(projectId: string, codeId: string): Promise<void> {
    const item = entry(projectId, codeId)
    const id = key(projectId, codeId)
    const previous = inspections.get(id)
    if (previous?.item === item) return previous.promise
    const promise = runInspect(item)
    inspections.set(id, { item, promise })
    try {
      await promise
    } finally {
      if (inspections.get(id)?.promise === promise) inspections.delete(id)
    }
  }
  async function runInspect(item: CodeEntry) {
    item.gitLoading = true
    item.gitError = ''
    try {
      const info = await codeApi.inspect(
        item.projectId,
        item.codeId,
        workspace.status.bindingId || '',
      )
      if (alive(item)) item.git = info
    } catch (error) {
      if (alive(item)) item.gitError = errorMessage(error)
    } finally {
      item.gitLoading = false
    }
  }
  async function scan(projectId: string, codeId: string): Promise<void> {
    const item = entry(projectId, codeId),
      id = key(projectId, codeId)
    const previous = scans.get(id)
    if (previous?.item === item) {
      if (!item.canceling) return previous.promise
      await previous.promise
      if (!alive(item)) return
      return scan(projectId, codeId)
    }
    const promise = runScan(item)
    scans.set(id, { item, promise })
    try {
      await promise
    } finally {
      if (scans.get(id)?.promise === promise) scans.delete(id)
    }
  }
  async function runScan(item: CodeEntry) {
    const request = crypto.randomUUID()
    item.requestId = request
    item.scanning = true
    item.canceling = false
    item.progress = null
    item.sizeError = ''
    try {
      const usage = await codeApi.scan(
        item.projectId,
        item.codeId,
        workspace.status.bindingId || '',
        request,
        (progress) => {
          if (alive(item) && item.requestId === request) item.progress = progress
        },
      )
      if (alive(item) && !item.canceling) item.usage = usage
    } catch (error) {
      if (alive(item) && !item.canceling) item.sizeError = errorMessage(error)
    } finally {
      item.requestId = null
      item.scanning = false
      item.canceling = false
      item.progress = null
    }
  }
  function cancel(projectId: string, codeId: string) {
    const item = entries[key(projectId, codeId)]
    if (!item?.requestId) return
    item.canceling = true
    const request = item.requestId
    void codeApi.cancelScan(request).catch(() => {
      if (item.requestId === request) item.canceling = false
    })
  }
  watch(
    [() => workspace.status.bindingId, () => workspace.status.rootPath],
    () => {
      for (const id of Object.keys(selected)) delete selected[id]
      for (const item of Object.values(entries)) {
        cancel(item.projectId, item.codeId)
        delete entries[key(item.projectId, item.codeId)]
      }
    },
    { flush: 'sync' },
  )
  watch(
    () => workspace.projects.map((project) => [project.id, project.codeDirectories]),
    () => {
      for (const item of Object.values(entries)) {
        if (directoryPath(item.projectId, item.codeId) !== item.path) {
          cancel(item.projectId, item.codeId)
          delete entries[key(item.projectId, item.codeId)]
        }
      }
    },
    { deep: true, flush: 'sync' },
  )
  async function saveDirectory(
    projectId: string,
    codeId: string | null,
    name: string,
    path: string,
  ) {
    const binding = workspace.status.bindingId || ''
    const records = await codeApi.saveDirectory(projectId, binding, codeId, name, path)
    applyDirectories(projectId, binding, records)
    if (desktop) void emit('workspace-changed')
    return records
  }
  async function removeDirectory(projectId: string, codeId: string) {
    const binding = workspace.status.bindingId || ''
    const records = await codeApi.removeDirectory(projectId, codeId, binding)
    applyDirectories(projectId, binding, records)
    if (desktop) void emit('workspace-changed')
  }
  function applyDirectories(projectId: string, binding: string, records: CodeDirectory[]) {
    if ((workspace.status.bindingId || '') !== binding) return
    const project = workspace.projects.find((p) => p.id === projectId)
    if (project) project.codeDirectories = records
  }
  return { selected, entries, entry, inspect, scan, cancel, saveDirectory, removeDirectory }
})
