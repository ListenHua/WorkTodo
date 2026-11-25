<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { NButton, NFormItem, NInput, NModal } from 'naive-ui'
import type { CodeDirectory } from '../shared/types'
import { useWorkspaceStore } from '../stores/workspace'
import { useCodeSpaceStore } from '../stores/code-space'
import { codeApi } from '../services/code-space'
import { errorMessage } from '../shared/domain'
import { useAppMessage } from '../composables/useAppMessage'
const props = defineProps<{
  projectId: string
  target: CodeDirectory | null
  mode: 'add' | 'edit' | 'delete'
}>()
const show = defineModel<boolean>({ required: true })
const emit = defineEmits<{ saved: [id: string] }>()
const workspace = useWorkspaceStore(),
  code = useCodeSpaceStore(),
  message = useAppMessage()
const name = ref(''),
  path = ref(''),
  error = ref(''),
  busy = ref(false)
const snapshot = ref({
  projectId: '',
  binding: '',
  codeId: null as string | null,
  mode: 'add' as typeof props.mode,
})
const titles = {
  add: '关联代码目录',
  edit: '编辑代码目录',
  delete: '删除代码关联',
}
const title = computed(() => titles[snapshot.value.mode])
let revision = 0,
  mounted = true
onBeforeUnmount(() => {
  mounted = false
  revision++
})
watch(show, (open) => {
  if (!open) return
  revision++
  snapshot.value = {
    projectId: props.projectId,
    binding: workspace.status.bindingId || '',
    codeId: props.target?.id || null,
    mode: props.mode,
  }
  name.value = props.target?.name || ''
  path.value = props.target?.path || ''
  error.value = ''
})
watch([() => workspace.status.bindingId, () => props.projectId], () => {
  revision++
  show.value = false
})
function current(token: number) {
  return (
    mounted && token === revision && snapshot.value.binding === (workspace.status.bindingId || '')
  )
}
async function pick() {
  if (busy.value) return
  const token = revision
  busy.value = true
  error.value = ''
  try {
    const selected = await codeApi.pickDirectory(snapshot.value.projectId, snapshot.value.binding)
    if (current(token) && selected) {
      path.value = selected
      if (!name.value.trim()) name.value = selected.split(/[\\/]/).filter(Boolean).at(-1) || ''
    }
  } catch (cause) {
    if (current(token)) error.value = errorMessage(cause)
  } finally {
    if (mounted) busy.value = false
  }
}
async function save() {
  if (busy.value || workspace.status.readOnly || !workspace.status.available) return
  const token = revision,
    data = { ...snapshot.value }
  if (data.mode !== 'delete' && (!name.value.trim() || !path.value)) return
  busy.value = true
  error.value = ''
  try {
    if (data.mode === 'delete') {
      await code.removeDirectory(data.projectId, data.codeId!)
    } else {
      const records = await code.saveDirectory(
        data.projectId,
        data.codeId,
        name.value.trim(),
        path.value,
      )
      if (current(token)) emit('saved', data.codeId || records.at(-1)!.id)
    }
    if (current(token)) {
      show.value = false
      message.success(data.mode === 'delete' ? '已删除代码关联，本地文件已保留' : '代码目录已保存')
    }
  } catch (cause) {
    if (current(token)) error.value = errorMessage(cause)
  } finally {
    if (mounted) busy.value = false
  }
}
function afterLeave() {
  if (!show.value) {
    name.value = ''
    path.value = ''
    error.value = ''
  }
}
</script>
<template>
  <NModal
    :show="show"
    preset="card"
    :title="title"
    :aria-label="title"
    :bordered="false"
    :closable="false"
    :mask-closable="!busy"
    :close-on-esc="!busy"
    style="width: min(520px, calc(100vw - 32px))"
    @update:show="!busy && (show = $event)"
    @after-leave="afterLeave"
  >
    <template v-if="snapshot.mode === 'delete'">
      <p>确定删除“{{ name }}”的代码关联？本地目录和文件会保留。</p>
      <p class="directory-path muted">{{ path }}</p>
    </template>
    <template v-else>
      <NFormItem label="名称" label-for="code-directory-name">
        <NInput
          v-model:value="name"
          :disabled="busy"
          :maxlength="80"
          placeholder="例如：前端、后端、App"
          :input-props="{ id: 'code-directory-name' }"
          @keydown.enter.prevent="save"
        />
      </NFormItem>
      <NFormItem label="代码目录">
        <div class="directory-picker">
          <span class="directory-path" :class="{ muted: !path }">{{
            path || '选择本地代码目录'
          }}</span>
          <NButton :disabled="busy" @click="pick">{{ path ? '重新选择' : '选择目录' }}</NButton>
        </div>
      </NFormItem>
    </template>
    <p v-if="error" class="form-error" role="alert">{{ error }}</p>
    <template #footer
      ><div class="dialog-footer">
        <NButton :disabled="busy" @click="show = false">取消</NButton>
        <NButton
          :type="snapshot.mode === 'delete' ? 'error' : 'primary'"
          :loading="busy"
          :disabled="
            !!workspace.status.readOnly ||
            !workspace.status.available ||
            (snapshot.mode !== 'delete' && (!name.trim() || !path))
          "
          @click="save"
        >
          {{ snapshot.mode === 'delete' ? '删除' : '保存' }}
        </NButton>
      </div></template
    >
  </NModal>
</template>
<style scoped>
.directory-picker {
  display: flex;
  align-items: center;
  gap: 16px;
  width: 100%;
}
.directory-path {
  min-width: 0;
  flex: 1;
  overflow-wrap: anywhere;
  font-size: 13px;
}
p.directory-path {
  margin-top: 12px;
}
</style>
