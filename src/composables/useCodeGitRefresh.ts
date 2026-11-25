import { getCurrentWindow } from '@tauri-apps/api/window'
import { onBeforeUnmount, onMounted, watch } from 'vue'
import { desktop } from '../services/api'
import { useCodeSpaceStore } from '../stores/code-space'
import { useWorkspaceStore } from '../stores/workspace'

export function useCodeGitRefresh(target: {
  readonly projectId: string
  readonly codeId: string
  readonly codePath: string
}) {
  const workspace = useWorkspaceStore()
  const code = useCodeSpaceStore()
  let closed = false
  let focused = document.hasFocus()
  let timer: ReturnType<typeof setTimeout> | undefined
  let wakeRequested = false
  let refreshing = false
  let queued: boolean | null = null
  let unlistenFocus: (() => void) | undefined

  function active(focusOnly: boolean) {
    return !closed && workspace.status.available && (!focusOnly || (focused && !document.hidden))
  }
  function cancelPending() {
    if (timer !== undefined) clearTimeout(timer)
    timer = undefined
    wakeRequested = false
    queued = null
  }
  async function refreshGit(focusOnly = false): Promise<void> {
    if (!active(focusOnly)) return
    // A normal directory load can run in the background; focus requests cannot.
    queued = queued === false ? false : focusOnly
    if (refreshing) return
    refreshing = true
    try {
      while (queued !== null) {
        const focusOnly = queued
        queued = null
        const { projectId, codeId, codePath } = target
        const binding = workspace.status.bindingId
        const root = workspace.status.rootPath
        const item = code.entry(projectId, codeId)
        const current = () =>
          active(focusOnly) &&
          target.projectId === projectId &&
          target.codeId === codeId &&
          target.codePath === codePath &&
          workspace.status.bindingId === binding &&
          workspace.status.rootPath === root &&
          item.path === codePath &&
          code.entries[`${projectId}:${codeId}`] === item
        if (!current()) continue
        if (item.gitLoading) {
          // Wait for a previous visit/manual refresh, then read fresh data once.
          await code.inspect(projectId, codeId)
          if (!current()) continue
          queued = null
        }
        await code.inspect(projectId, codeId)
      }
    } finally {
      refreshing = false
    }
  }
  function schedule() {
    if (!desktop || !active(true)) return
    wakeRequested = true
    if (timer !== undefined) clearTimeout(timer)
    timer = setTimeout(() => {
      timer = undefined
      // AppShell already reloads workspace metadata when the window wakes.
      if (workspace.loading) return
      wakeRequested = false
      void refreshGit(true)
    }, 250)
  }
  function focusChanged(value: boolean) {
    focused = value
    if (value) schedule()
    else cancelPending()
  }
  function windowFocus(event: FocusEvent) {
    if (event.target === window) focusChanged(true)
  }
  function windowBlur(event: FocusEvent) {
    if (event.target === window) focusChanged(false)
  }
  function visibilityChanged() {
    if (document.hidden) cancelPending()
    else schedule()
  }
  watch(
    () => workspace.loading,
    (loading) => {
      if (!loading && wakeRequested) schedule()
    },
  )
  watch(
    [
      () => target.projectId,
      () => target.codeId,
      () => target.codePath,
      () => workspace.status.bindingId,
      () => workspace.status.rootPath,
      () => workspace.status.available,
    ],
    cancelPending,
    { flush: 'sync' },
  )
  onMounted(() => {
    document.addEventListener('visibilitychange', visibilityChanged)
    window.addEventListener('focus', windowFocus)
    window.addEventListener('blur', windowBlur)
    if (desktop) {
      void getCurrentWindow()
        .onFocusChanged(({ payload }) => focusChanged(payload))
        .then((off) => {
          if (closed) off()
          else unlistenFocus = off
        })
        .catch(() => {
          // DOM window events remain available if native registration fails.
        })
    }
  })
  onBeforeUnmount(() => {
    closed = true
    cancelPending()
    unlistenFocus?.()
    document.removeEventListener('visibilitychange', visibilityChanged)
    window.removeEventListener('focus', windowFocus)
    window.removeEventListener('blur', windowBlur)
  })
  return refreshGit
}
