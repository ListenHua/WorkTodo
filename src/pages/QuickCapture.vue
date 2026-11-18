<script setup lang="ts">
import { useAppMessage } from '../composables/useAppMessage'
import AppIcon from '../components/AppIcon.vue'
import { ref, onMounted, onUnmounted, watch, computed, nextTick } from 'vue'
import { NButton, NInput, NSelect } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { emitTo, listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useWorkspaceStore } from '../stores/workspace'
import { desktop } from '../services/api'
import { errorMessage } from '../shared/domain'
import { attachments } from '../services/attachments'
import { preferences } from '../services/preferences'
const workspace = useWorkspaceStore()
const text = ref('')
const projectId = ref('')
const hasImage = ref(false)
const preview = ref('')
const error = ref('')
const busy = ref(false)
const message = useAppMessage()
let gone = false
const cleanups: (() => void)[] = []
const draftReady = ref(false)
let draftQueue = Promise.resolve()
const draftError = ref('')
let draftTimer: ReturnType<typeof setTimeout> | undefined
let pendingDraft: { value: string; bindingId: string | null; projectId: string } | null = null
let bindingId: string | null = null
let suppressDraft = false
let requestId = crypto.randomUUID()
function flushDraft() {
  clearTimeout(draftTimer)
  const draft = pendingDraft
  pendingDraft = null
  if (draft)
    draftQueue = draftQueue
      .catch(() => {})
      .then(async () => {
        try {
          await preferences.set('capture_draft', draft.value, draft.bindingId, draft.projectId)
          draftError.value = ''
        } catch (cause) {
          draftError.value = errorMessage(cause)
        }
      })
  return draftQueue
}
watch([text, projectId], ([value]) => {
  if (!draftReady.value || suppressDraft) return
  if (!desktop) {
    void preferences.set('capture_draft', value)
    return
  }
  pendingDraft = { value, bindingId, projectId: projectId.value }
  clearTimeout(draftTimer)
  draftTimer = setTimeout(flushDraft, 300)
})
async function reloadDraft() {
  const previous = bindingId
  await workspace.refresh()
  const next = workspace.status.bindingId || (desktop ? 'unbound' : null)
  if (next !== previous) {
    await flushDraft()
    bindingId = next
    suppressDraft = true
    text.value = (await preferences.get('capture_draft')) || ''
    projectId.value = (await preferences.get('capture_project')) || ''
    await nextTick()
    suppressDraft = false
    clearImage()
    requestId = crypto.randomUUID()
    await selected()
  }
}
async function hide() {
  await nextTick()
  await flushDraft()
  await getCurrentWindow().hide()
}
async function removeImage() {
  try {
    if (desktop) await invoke('capture_discard', { bindingId })
    clearImage()
  } catch (cause) {
    error.value = errorMessage(cause)
  }
}
const options = computed(() => [
  { label: '无', value: '' },
  ...workspace.projects
    .filter((p) => p.status === 'active')
    .map((p) => ({ label: p.name, value: p.id })),
])
function clearImage() {
  if (preview.value) URL.revokeObjectURL(preview.value)
  preview.value = ''
  hasImage.value = false
}
async function apply(capture: { text: string; hasImage: boolean; error?: string | null }) {
  error.value = capture.error || ''
  if (capture.error) return
  if (capture.text) text.value = text.value ? `${text.value}\n${capture.text}` : capture.text
  clearImage()
  hasImage.value = capture.hasImage
  if (hasImage.value) {
    const bytes = await invoke<ArrayBuffer>('capture_image')
    preview.value = URL.createObjectURL(new Blob([bytes], { type: 'image/png' }))
  }
}
async function read() {
  try {
    error.value = ''
    if (desktop) {
      await apply(await invoke('capture_clipboard'))
    } else {
      const value = await navigator.clipboard.readText()
      text.value = value
    }
  } catch (e) {
    error.value = errorMessage(e)
  }
}
async function selected() {
  try {
    const capture = await invoke<{ text: string; hasImage: boolean; error?: string | null } | null>(
      'capture_selection',
    )
    if (capture) await apply(capture)
  } catch (e) {
    error.value = errorMessage(e)
  }
}
async function save() {
  if (busy.value || !draftReady.value) return
  busy.value = true
  error.value = ''
  try {
    await nextTick()
    await flushDraft()
    if (draftError.value) throw new Error(draftError.value)
    if (!text.value.trim() && !hasImage.value) throw new Error('请输入待办内容或添加图片')
    if (desktop) {
      await invoke('capture_save', {
        text: text.value,
        projectId: projectId.value || null,
        includeImage: hasImage.value,
        requestId,
        bindingId,
      })
    } else {
      const task = await workspace.createTask({
        title: text.value.split('\n')[0]!.slice(0, 200),
        projectId: projectId.value || null,
        priority: 'low',
        planDate: null,
      })
      await workspace.updateTask(task.id, {
        title: task.title,
        projectId: task.projectId,
        priority: task.priority,
        planDate: null,
        status: 'todo',
        description: text.value,
        dueAt: null,
      })
      if (hasImage.value)
        await attachments.import(task.id, {
          kind: 'browser',
          file: new File([await (await fetch(preview.value)).blob()], '采集图片.png', {
            type: 'image/png',
          }),
        })
    }
    requestId = crypto.randomUUID()
    text.value = ''
    clearImage()
    if (desktop) await emitTo('main', 'capture-saved').catch(() => {})
    else message.success('已添加待办')
    await nextTick()
    await flushDraft()
    if (desktop) await getCurrentWindow().hide()
  } catch (e) {
    error.value = errorMessage(e)
  } finally {
    busy.value = false
  }
}
function keyboard(event: KeyboardEvent) {
  if (event.isComposing || busy.value) return
  if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
    event.preventDefault()
    void save()
  }
  if (event.key === 'Escape' && desktop) {
    event.preventDefault()
    void hide()
  }
}
async function discard() {
  if (busy.value) return
  busy.value = true
  try {
    await flushDraft()
    await preferences.set('capture_draft', '', bindingId)
    text.value = ''
    clearImage()
    draftError.value = ''
    if (desktop) {
      await invoke('capture_discard', { bindingId })
      await getCurrentWindow().hide()
    }
  } catch (e) {
    error.value = errorMessage(e)
  } finally {
    busy.value = false
  }
}
onMounted(async () => {
  const before = text.value
  try {
    await workspace.refresh()
    bindingId = workspace.status.bindingId || (desktop ? 'unbound' : null)
    const draft = await preferences.get('capture_draft')
    if (text.value === before) text.value = draft || ''
    projectId.value = (await preferences.get('capture_project')) || ''
  } catch {
    draftError.value = '无法读取本地草稿，请检查本地数据访问权限'
  }
  if (gone) return
  draftReady.value = true
  await workspace.refresh()
  if (desktop) {
    const quit = await listen('capture-before-exit', async () => {
      await nextTick()
      await flushDraft()
      if (draftError.value.includes('本机暂存也失败')) {
        error.value = draftError.value
        await getCurrentWindow().show()
        return
      }
      await invoke('desktop_quit')
    })
    if (gone) quit()
    else cleanups.push(quit)
    const closing = await getCurrentWindow().onCloseRequested((event) => {
      event.preventDefault()
      void hide()
    })
    if (gone) closing()
    else cleanups.push(closing)
    const focus = await getCurrentWindow().onFocusChanged(({ payload }) => {
      if (!payload) void flushDraft()
    })
    if (gone) focus()
    else cleanups.push(focus)
    const changes = await listen('workspace-changed', () => void reloadDraft())
    if (gone) changes()
    else cleanups.push(changes)
    const off = await listen('capture-selection', selected)
    if (gone) off()
    else cleanups.push(off)
    await selected()
  }
})
onUnmounted(() => {
  gone = true
  void flushDraft()
  for (const off of cleanups) off()
  clearImage()
})
</script>
<template>
  <main class="quick-capture">
    <h2>快速添加</h2>
    <form class="dialog-form" @submit.prevent="save" @keydown="keyboard">
      <label for="capture-text">待办内容</label
      ><NInput
        v-model:value="text"
        :disabled="busy || !draftReady"
        type="textarea"
        :input-props="{ id: 'capture-text' }"
        :autosize="{ minRows: 4, maxRows: 9 }"
        maxlength="10000"
        autofocus
        placeholder="输入待办内容"
      /><label>所属项目</label
      ><NSelect :disabled="busy" v-model:value="projectId" :options="options" aria-label="所属项目"
        ><template #arrow><AppIcon name="down" :size="16" /></template></NSelect
      ><img
        v-if="preview"
        :src="preview"
        alt="采集图片"
        style="max-height: 160px; max-width: 100%; object-fit: contain"
      />
      <div class="dialog-footer">
        <NButton :disabled="busy" @click="read">读取剪贴板</NButton
        ><NButton v-if="hasImage" :disabled="busy" @click="removeImage">移除图片</NButton>
      </div>
      <p v-if="draftError" class="form-error" role="alert">{{ draftError }}</p>
      <p v-if="error" class="form-error" role="alert">{{ error }}</p>
      <div class="dialog-footer">
        <NButton :disabled="busy" @click="discard">放弃草稿</NButton
        ><NButton type="primary" attr-type="submit" :loading="busy">添加待办</NButton>
      </div>
    </form>
  </main>
</template>
<style scoped>
.quick-capture {
  padding: 24px;
  min-height: 100vh;
  background: var(--surface);
  color: var(--text);
}
h2 {
  margin: 0 0 20px;
}
</style>
