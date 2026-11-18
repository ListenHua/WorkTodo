import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { nextTick, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { desktop } from '../services/api'
import { errorMessage } from '../shared/domain'
import { useUiStore } from '../stores/ui'
import { useWorkspaceStore } from '../stores/workspace'
import { useAppMessage } from './useAppMessage'

type TrayAction = 'add-task' | 'view-tasks' | 'add-project' | 'view-projects' | 'settings'

export function useTrayActions() {
  const router = useRouter()
  const ui = useUiStore()
  const workspace = useWorkspaceStore()
  const message = useAppMessage()
  let closed = false
  let unlisten: (() => void) | undefined
  let processing = Promise.resolve()

  async function drain() {
    const actions = await invoke<TrayAction[]>('tray_take_actions')
    for (const action of actions) {
      if (closed) return
      ui.projectDialog = false
      ui.taskDialog = false
      ui.search = ''
      const path =
        action === 'settings'
          ? '/settings'
          : action === 'add-project' || action === 'view-projects'
            ? '/projects'
            : '/tasks'
      if (action === 'add-task' || action === 'add-project') await workspace.refresh()
      if (closed) return
      await router.push(path)
      // Let any workspace binding change remount the dialogs before opening one.
      await nextTick()
      if (closed) return
      if (action === 'add-task') ui.newTask()
      else if (action === 'add-project') ui.newProject()
    }
  }
  function schedule() {
    processing = processing.then(drain).catch((cause) => {
      if (!closed) message.error(errorMessage(cause))
    })
  }
  onMounted(async () => {
    if (!desktop) return
    try {
      const off = await listen('tray-actions-available', schedule)
      if (closed) return off()
      unlisten = off
      // Subscribe before draining to cover clicks made while the page was loading.
      schedule()
    } catch (cause) {
      if (!closed) message.error(errorMessage(cause))
    }
  })
  onUnmounted(() => {
    closed = true
    unlisten?.()
  })
}
