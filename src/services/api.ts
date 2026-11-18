import { invoke, isTauri } from '@tauri-apps/api/core'
import type { WorkspaceApi } from '../shared/types'
import { createPreviewApi } from './preview'

export const desktop = isTauri()
const nativeApi: WorkspaceApi = {
  createFullTask: (input) => invoke('tasks_create_full', { input }),
  updateTask: (id, input) => invoke('tasks_update', { id, input }),
  trashTask: (id, restore, toInbox = false) => invoke('tasks_trash', { id, restore, toInbox }),
  deleteProject: (id) => invoke('projects_delete', { id }),
  updateProject: (id, input) => invoke('projects_update', { id, input }),
  setProjectPinned: (id, pinned) => invoke('projects_set_pinned', { id, pinned }),
  setProjectStatus: (id, status, policy = 'inbox') =>
    invoke('projects_set_status', { id, status, policy }),
  getStatus: () => invoke('workspace_status'),
  selectRoot: () => invoke('workspace_select_root'),
  listProjects: () => invoke('projects_list'),
  createProject: (input) => invoke('projects_create', { input }),
  listTasks: () => invoke('tasks_list'),
  createTask: (input) => invoke('tasks_create', { input }),
  setTaskCompleted: (id, completed) => invoke('tasks_set_completed', { id, completed }),
  openProjectFolder: (id) => invoke('projects_open_folder', { id }),
}
export const api: WorkspaceApi = desktop ? nativeApi : createPreviewApi()
