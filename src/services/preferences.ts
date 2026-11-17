import { invoke } from '@tauri-apps/api/core'
import { desktop } from './api'

type PreferenceKey =
  | 'appearance'
  | 'capture_project'
  | 'capture_draft'
  | 'changelog_seen'
  | 'strict_offline'
  | 'project_sort'
const legacyKeys: Record<PreferenceKey, string> = {
  appearance: 'project-manage:theme',
  project_sort: 'project-manage:project-sort',
  capture_project: 'capture-project',
  strict_offline: 'project-manage:strict-offline',
  capture_draft: 'capture-draft',
  changelog_seen: 'project-manage:changelog-seen',
}
function readBrowser(key: string) {
  try {
    return localStorage.getItem(key)
  } catch {
    return null
  }
}
export const preferences = {
  async get(key: PreferenceKey): Promise<string | null> {
    if (!desktop) return readBrowser(legacyKeys[key])
    const saved = await invoke<string | null>('preference_get', { key })
    if (saved !== null) return saved
    if (key === 'changelog_seen' || key === 'project_sort') return null
    // Migrate only after a successful database read; leave the old copy on failure.
    const legacy = readBrowser(legacyKeys[key])
    if (legacy !== null) {
      await invoke('preference_set', { key, value: legacy })
      try {
        localStorage.removeItem(legacyKeys[key])
      } catch {}
    }
    return legacy
  },
  async set(
    key: PreferenceKey,
    value: string,
    bindingId?: string | null,
    draftProjectId?: string,
  ): Promise<void> {
    if (desktop)
      await invoke(
        'preference_set',
        bindingId === undefined
          ? { key, value }
          : { key, value, bindingId, ...(draftProjectId === undefined ? {} : { draftProjectId }) },
      )
    else localStorage.setItem(legacyKeys[key], value)
  },
  async paths(): Promise<{ database: string; projects: string | null } | null> {
    return desktop ? invoke('local_data_paths') : null
  },
}
