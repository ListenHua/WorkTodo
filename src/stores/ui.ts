import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { Task, Project } from '../shared/types'
export const useUiStore = defineStore('ui', () => {
  const editingTask = ref<Task | null>(null)
  const editingProject = ref<Project | null>(null)
  const captureText = ref('')
  const search = ref('')
  const projectDialog = ref(false)
  const taskDialog = ref(false)
  const defaultProjectId = ref<string | null>(null)
  function newTask(projectId: string | null = null) {
    editingTask.value = null
    captureText.value = ''
    defaultProjectId.value = projectId
    taskDialog.value = true
  }
  function editTask(task: Task) {
    defaultProjectId.value = null
    editingTask.value = task
    taskDialog.value = true
  }
  function editProject(project: Project) {
    editingProject.value = project
    projectDialog.value = true
  }
  function newProject() {
    editingProject.value = null
    projectDialog.value = true
  }
  return {
    search,
    projectDialog,
    taskDialog,
    defaultProjectId,
    newTask,
    editingTask,
    editingProject,
    editTask,
    editProject,
    newProject,
    captureText,
  }
})
