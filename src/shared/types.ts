export type TaskStatus = 'todo' | 'doing' | 'done' | 'canceled'
export type Priority = 'low' | 'medium' | 'high'
export interface WorkspaceStatus {
  rootPath: string | null
  available: boolean
  pendingOperations: number
  readOnly?: boolean
  storageError?: string | null
  bindingId?: string | null
}
export interface ProjectLink {
  id: string
  icon: '' | 'github' | 'gitlab' | 'gitee' | 'website'
  iconData?: string
  name: string
  url: string
}
export interface ProjectTag {
  name: string
  color: string
}
export interface CodeDirectory {
  id: string
  name: string
  path: string
  createdAt: string
}
export interface Project {
  id: string
  name: string
  description: string
  color: string
  status: 'active' | 'completed' | 'archived'
  pinned: boolean
  directoryRelpath: string
  createdAt: string
  taskCount: number
  completedCount: number
  tags?: ProjectTag[]
  links?: ProjectLink[]
  codeDirectories?: CodeDirectory[]
}
export interface Task {
  id: string
  projectId: string | null
  title: string
  status: TaskStatus
  priority: Priority
  planDate: string | null
  createdAt: string
  completedAt?: string | null
  description?: string
  dueAt?: string | null
  deletedAt?: string | null
}
export interface TrashRequest {
  ids: string[]
  action: 'restore' | 'purge'
  bindingId: string
  restoreToInbox: boolean
}
export interface TrashResult {
  succeededIds: string[]
  failed: { id: string; message: string }[]
}
export interface ProjectDirectoryMerge {
  path: string
  fingerprint: string
}
export interface ProjectDirectoryConflict extends ProjectDirectoryMerge {
  name: string
}
export interface CreateProject {
  directoryMerge?: ProjectDirectoryMerge
  tags?: ProjectTag[]
  links?: ProjectLink[]
  status?: Project['status']
  pendingPolicy?: 'inbox' | 'complete' | 'cancel'
  name: string
  description: string
  color: string
}
export interface CreateTask {
  title: string
  projectId: string | null
  priority: Priority
  planDate: string | null
}
export interface UpdateTask extends CreateTask {
  reminderAt?: string | null
  description: string
  dueAt: string | null
  status: TaskStatus
}
export const statusLabels: Record<TaskStatus, string> = {
  todo: '待处理',
  doing: '进行中',
  done: '已完成',
  canceled: '已取消',
}
export interface WorkspaceApi {
  getStatus(): Promise<WorkspaceStatus>
  selectRoot(): Promise<WorkspaceStatus | null>
  listProjects(): Promise<Project[]>
  createProject(input: CreateProject): Promise<Project>
  listTasks(): Promise<Task[]>
  createTask(input: CreateTask): Promise<Task>
  setTaskCompleted(id: string, completed: boolean): Promise<void>
  openProjectFolder(id: string): Promise<void>
  createFullTask(input: UpdateTask): Promise<Task>
  updateTask(id: string, input: UpdateTask): Promise<Task>
  trashTask(id: string, restore: boolean, toInbox?: boolean): Promise<void>
  manageTrash(request: TrashRequest): Promise<TrashResult>
  updateProject(id: string, input: CreateProject): Promise<Project>
  deleteProject(id: string, deleteFiles?: boolean): Promise<void>
  setProjectPinned(id: string, pinned: boolean): Promise<void>
  setProjectStatus(
    id: string,
    status: Project['status'],
    policy?: 'inbox' | 'complete' | 'cancel',
  ): Promise<void>
}
export const projectColors = [
  '#1677ff',
  '#e60033',
  '#f8e58c',
  '#f7b977',
  '#ea5506',
  '#ffd900',
  '#674598',
  '#16160e',
] as const
export const priorityLabels: Record<Priority, string> = {
  low: '低',
  medium: '中',
  high: '高',
}
