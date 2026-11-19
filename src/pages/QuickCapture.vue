<script setup lang="ts">
import { useAppMessage } from '../composables/useAppMessage'
import TaskForm from '../components/TaskForm.vue'
import { taskFormInput, taskFormValue, type TaskFormValue } from '../shared/task-form'
import { desktopApi } from '../services/desktop'
import { ref, onMounted, onUnmounted, watch, computed, nextTick } from 'vue'
import { NButton } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { emit, emitTo, listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useWorkspaceStore } from '../stores/workspace'
import { desktop } from '../services/api'
import { errorMessage } from '../shared/domain'
import { attachments } from '../services/attachments'
import { preferences } from '../services/preferences'
const workspace = useWorkspaceStore()
const form = ref(taskFormValue())
const formInstance = ref(0)
const projectId = computed({
  get: () => form.value.projectId,
  set: (value) => {
    form.value.projectId = value
  },
})
const text = computed({
  get: () =>
    form.value.description ? `${form.value.title}\n${form.value.description}` : form.value.title,
  set: (value: string) => {
    const [first = '', ...rest] = value.split('\n')
    const characters = [...first]
    form.value.title = characters.slice(0, 200).join('')
    form.value.description = [
      ...(characters.length > 200 ? [characters.slice(200).join('')] : []),
      ...rest,
    ].join('\n')
  },
})
function draftKey() {
  return `worktodo:capture-form:${bindingId || 'browser'}`
}
function restoreDraft(value: string, project: string) {
  form.value = taskFormValue(null, project)
  formInstance.value++
  text.value = value
  try {
    const saved = JSON.parse(localStorage.getItem(draftKey()) || 'null')
    if (saved?.text === value && saved.form) {
      for (const key of [
        'title',
        'description',
        'priority',
        'status',
        'planDate',
        'dueAt',
        'reminderAt',
      ] as const) {
        if (typeof saved.form[key] === 'string')
          Object.assign(form.value, { [key]: saved.form[key] })
      }
    }
  } catch {}
}
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
let pendingDraft: {
  value: string
  bindingId: string | null
  projectId: string
  form: TaskFormValue
} | null = null
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
          // Keep the portable draft readable; retain the additional form settings locally per workspace.
          let settingsError = ''
          try {
            localStorage.setItem(
              `worktodo:capture-form:${draft.bindingId || 'browser'}`,
              JSON.stringify({ text: draft.value, form: draft.form }),
            )
          } catch {
            settingsError = '无法保存待办设置草稿，请检查本地数据访问权限'
          }
          await preferences.set('capture_draft', draft.value, draft.bindingId, draft.projectId)
          if (!desktop) await preferences.set('capture_project', draft.projectId)
          draftError.value = settingsError
        } catch (cause) {
          draftError.value = errorMessage(cause)
        }
      })
  return draftQueue
}
watch(
  form,
  () => {
    if (!draftReady.value || suppressDraft) return
    pendingDraft = {
      value: text.value,
      bindingId,
      projectId: projectId.value,
      form: { ...form.value },
    }
    clearTimeout(draftTimer)
    if (desktop) draftTimer = setTimeout(flushDraft, 300)
    else void flushDraft()
  },
  { deep: true },
)
async function reloadDraft() {
  const previous = bindingId
  await workspace.refresh()
  const next = workspace.status.bindingId || (desktop ? 'unbound' : null)
  if (next !== previous) {
    await flushDraft()
    bindingId = next
    suppressDraft = true
    restoreDraft(
      (await preferences.get('capture_draft')) || '',
      (await preferences.get('capture_project')) || '',
    )
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
    const input = taskFormInput(form.value, hasImage.value)
    if (desktop) {
      await invoke('capture_save', {
        input,
        includeImage: hasImage.value,
        requestId,
        bindingId,
      })
    } else {
      const task = await workspace.createFullTask(input)
      if (hasImage.value)
        await attachments.import(task.id, {
          kind: 'browser',
          file: new File([await (await fetch(preview.value)).blob()], '采集图片.png', {
            type: 'image/png',
          }),
        })
    }
    requestId = crypto.randomUUID()
    form.value = taskFormValue(null, projectId.value)
    formInstance.value++
    clearImage()
    if (desktop) await emitTo('main', 'capture-saved').catch(() => {})
    else message.success('已添加待办')
    await nextTick()
    await flushDraft()
    if (input.reminderAt && desktop) {
      try {
        if ((await desktopApi.notificationRequest()) !== 'granted')
          message.info('待办已保存，请在提醒中心开启系统通知')
      } catch (cause) {
        message.error(errorMessage(cause))
      }
    }
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
    form.value = taskFormValue(null, projectId.value)
    formInstance.value++
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
    if (text.value === before)
      restoreDraft(draft || '', (await preferences.get('capture_project')) || '')
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
    const prepareStorage = await listen<string>(
      'capture-prepare-storage-change',
      async ({ payload }) => {
        await nextTick()
        await flushDraft()
        await emit('capture-storage-ready', {
          requestId: payload,
          saved: !draftError.value.includes('本机暂存也失败'),
        })
      },
    )
    if (gone) prepareStorage()
    else cleanups.push(prepareStorage)
    const changes = await listen('workspace-changed', () => {
      void reloadDraft().catch((cause) => {
        error.value = errorMessage(cause)
      })
    })
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
  <main class="quick-capture" @keydown="keyboard">
    <h2>新建待办</h2>
    <TaskForm
      :key="formInstance"
      v-model="form"
      :busy="busy"
      :disabled="!draftReady"
      :error="error"
      @submit="save"
      @cancel="desktop ? hide() : discard()"
    >
      <img v-if="preview" :src="preview" alt="采集图片" class="capture-preview" />
      <div class="dialog-footer capture-actions">
        <NButton :disabled="busy || !draftReady" @click="read">读取剪贴板</NButton>
        <NButton v-if="hasImage" :disabled="busy" @click="removeImage">移除图片</NButton>
        <NButton :disabled="busy || !draftReady" @click="discard">放弃草稿</NButton>
      </div>
      <p v-if="draftError" class="form-error" role="alert">{{ draftError }}</p>
    </TaskForm>
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
.capture-preview {
  max-height: 160px;
  max-width: 100%;
  object-fit: contain;
}
.capture-actions {
  justify-content: flex-start;
  flex-wrap: wrap;
}
</style>
