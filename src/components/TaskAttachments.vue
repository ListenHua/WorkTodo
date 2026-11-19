<script setup lang="ts">
import AppIcon from './AppIcon.vue'
import EmptyState from './EmptyState.vue'
import { computed, onUnmounted, ref, watch } from 'vue'
import { NButton, NSelect, NModal } from 'naive-ui'
import { attachments } from '../services/attachments'
import { filesApi } from '../services/files'
import { desktop } from '../services/api'
import { errorMessage } from '../shared/domain'
import { canPreviewFile, formatFileSize } from '../shared/files'
import type { ProjectFile } from '../shared/files'
import { useAppMessage } from '../composables/useAppMessage'
const props = defineProps<{ taskId: string; projectId: string | null }>()
const message = useAppMessage()
const files = ref<ProjectFile[]>([])
const available = ref<ProjectFile[]>([])
const picked = ref<string | null>(null)
const input = ref<HTMLInputElement>()
const busy = ref(false)
let gone = false
const preview = ref('')
const previewName = ref('')
const options = computed(() =>
  available.value
    .filter((f) => !files.value.some((a) => a.id === f.id) && f.status === 'available')
    .map((f) => ({ label: f.name, value: f.id })),
)
async function load() {
  files.value = await attachments.list(props.taskId)
  available.value = await filesApi.scan(props.projectId || '__inbox__')
}
watch(
  () => props.taskId,
  () => void load().catch((e) => message.error(errorMessage(e))),
  { immediate: true },
)
async function run(action: () => Promise<unknown>, success = '') {
  busy.value = true
  try {
    await action()
    await load()
    if (success) message.success(success)
  } catch (e) {
    message.error(errorMessage(e))
  } finally {
    busy.value = false
  }
}
async function pick() {
  if (!desktop) {
    input.value?.click()
    return
  }
  const paths = await filesApi.pick()
  if (!paths.length) return
  await run(async () => {
    for (const path of paths) await attachments.import(props.taskId, { kind: 'native', path })
  }, `已添加 ${paths.length} 个附件`)
}
async function upload(event: Event) {
  const el = event.target as HTMLInputElement
  const chosen = Array.from(el.files || [])
  el.value = ''
  if (!chosen.length) return
  await run(async () => {
    for (const file of chosen) await attachments.import(props.taskId, { kind: 'browser', file })
  }, `已添加 ${chosen.length} 个附件`)
}
function clearPreview() {
  if (preview.value) URL.revokeObjectURL(preview.value)
  preview.value = ''
}
async function open(file: ProjectFile) {
  try {
    if (canPreviewFile(file)) {
      clearPreview()
      const blob = await filesApi.preview(file.projectId, file)
      if (gone) return
      preview.value = URL.createObjectURL(blob)
      previewName.value = file.name
    } else {
      await filesApi.open(file.projectId, file.id, false)
    }
  } catch (e) {
    message.error(errorMessage(e))
  }
}
onUnmounted(() => {
  gone = true
  clearPreview()
})
</script>
<template>
  <div class="attachment-section">
    <div class="panel-heading">
      <h3>附件</h3>
      <NButton size="small" :loading="busy" @click="pick">添加附件</NButton>
    </div>
    <input ref="input" hidden type="file" multiple @change="upload" />
    <div v-if="options.length" class="attachment-link">
      <NSelect
        v-model:value="picked"
        :options="options"
        placeholder="关联项目已有文件"
        aria-label="关联项目已有文件"
        ><template #arrow><AppIcon name="down" :size="16" /></template></NSelect
      ><NButton
        :disabled="!picked || busy"
        @click="
          run(async () => {
            await attachments.link(taskId, picked!)
            picked = null
          })
        "
        >关联</NButton
      >
    </div>
    <ul class="attachment-list">
      <li v-for="file in files" :key="file.id">
        <button type="button" class="text-button" @click="open(file)">
          {{ file.name }}</button><small>{{ formatFileSize(file.size) }}</small
        ><NButton text :disabled="busy" @click="run(() => attachments.link(taskId, file.id, true))"
          >解除关联</NButton
        >
      </li>
    </ul>
    <EmptyState
      v-if="!files.length"
      compact
      kind="files"
      title="暂无附件"
      action-label="点击添加"
      :disabled="busy"
      @action="pick"
    />
  </div>
  <NModal
    :show="!!preview"
    preset="card"
    :title="previewName"
    :bordered="false"
    :closable="false"
    class="form-modal"
    @update:show="clearPreview"
    ><template #header-extra
      ><button type="button" class="icon-button" aria-label="关闭预览" @click="clearPreview">
        <AppIcon name="close" :size="20" /></button></template
    ><img
      :src="preview"
      :alt="previewName"
      style="max-width: 100%; max-height: 60vh; display: block; margin: auto"
  /></NModal>
</template>
