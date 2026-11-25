<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { NButton, NCheckbox, NModal, NProgress } from 'naive-ui'
import { useWorkspaceStore } from '../stores/workspace'
import { useCodeSpaceStore } from '../stores/code-space'
import { codeApi } from '../services/code-space'
import { useAppMessage } from '../composables/useAppMessage'
import { errorMessage } from '../shared/domain'
import {
  formatCodeSize,
  type CleanResult,
  type CodeProgress,
  type Dependency,
} from '../shared/code-space'
const props = defineProps<{ projectId: string; codeId: string }>()
const show = defineModel<boolean>({ required: true })
const workspace = useWorkspaceStore(),
  code = useCodeSpaceStore(),
  message = useAppMessage()
const targets = ref<Dependency[]>([]),
  selected = ref<string[]>([])
const busy = ref(false),
  error = ref(''),
  result = ref<CleanResult | null>(null),
  progress = ref<CodeProgress | null>(null)
const projectName = ref('')
let mounted = true
onBeforeUnmount(() => {
  mounted = false
})
const chosen = computed(() =>
  targets.value.filter((target) => selected.value.includes(target.path)),
)
const bytes = computed(() => chosen.value.reduce((sum, target) => sum + target.bytes, 0))
watch(show, (open) => {
  if (!open) return
  targets.value = [...(code.entry(props.projectId, props.codeId).usage?.dependencies || [])]
  selected.value = targets.value.slice(0, 100).map((t) => t.path)
  const project = workspace.projects.find((p) => p.id === props.projectId)
  const directory = project?.codeDirectories?.find((c) => c.id === props.codeId)
  projectName.value = `${project?.name || '当前项目'} / ${directory?.name || '代码目录'}（${directory?.path || ''}）`
  error.value = ''
  result.value = null
  progress.value = null
})
watch(
  () => workspace.status.bindingId,
  () => {
    show.value = false
  },
)
function toggle(path: string, checked: boolean) {
  selected.value = checked ? [...selected.value, path] : selected.value.filter((p) => p !== path)
}
async function clean() {
  if (busy.value || workspace.status.readOnly || !chosen.value.length) return
  const bindingId = workspace.status.bindingId || '',
    projectId = props.projectId,
    codeId = props.codeId,
    input = chosen.value,
    path = code.entry(projectId, codeId).path
  const current = () =>
    mounted &&
    (workspace.status.bindingId || '') === bindingId &&
    workspace.projects
      .find((p) => p.id === projectId)
      ?.codeDirectories?.find((c) => c.id === codeId)?.path === path
  busy.value = true
  error.value = ''
  result.value = null
  try {
    const outcome = await codeApi.clean(projectId, codeId, bindingId, input, (value) => {
      if (current()) progress.value = value
    })
    if (!current()) return
    result.value = outcome
    await Promise.all([code.inspect(projectId, codeId), code.scan(projectId, codeId)])
    if (!current()) return
    if (!outcome.failed.length) {
      show.value = false
      message.success(`已清理 ${outcome.succeeded.length} 个依赖目录`)
    } else {
      const failed = new Set(outcome.failed.map((item) => item.path))
      targets.value = (code.entry(projectId, codeId).usage?.dependencies || targets.value).filter(
        (item) => failed.has(item.path),
      )
      selected.value = targets.value.map((item) => item.path)
    }
  } catch (cause) {
    if (current()) error.value = errorMessage(cause)
  } finally {
    if (mounted) busy.value = false
  }
}
function afterLeave() {
  if (!show.value) {
    targets.value = []
    result.value = null
    error.value = ''
    progress.value = null
  }
}
</script>
<template>
  <NModal
    :show="show"
    preset="card"
    title="清理依赖"
    aria-label="确认清理依赖目录"
    class="code-cleanup-dialog"
    :bordered="false"
    :closable="false"
    :mask-closable="!busy"
    :close-on-esc="!busy"
    style="width: min(560px, calc(100vw - 32px))"
    @update:show="!busy && (show = $event)"
    @after-leave="afterLeave"
  >
    <p class="cleanup-summary">
      {{ projectName }} · 已选 {{ chosen.length }} 个目录，预计清理 {{ formatCodeSize(bytes) }}
    </p>
    <p class="muted">仅删除所选依赖目录，保留源代码、锁文件和 Git 记录。清理后需要重新安装依赖。</p>
    <div class="cleanup-targets">
      <div v-for="target in targets" :key="target.path" class="cleanup-target">
        <NCheckbox
          :checked="selected.includes(target.path)"
          :disabled="busy || (!selected.includes(target.path) && selected.length >= 100)"
          :aria-label="`选择清理 ${target.path}`"
          @update:checked="toggle(target.path, $event)"
          >{{ target.path }}</NCheckbox
        >
        <span class="muted">{{ formatCodeSize(target.bytes) }}</span>
      </div>
    </div>
    <div v-if="busy" class="cleanup-progress" role="status" aria-live="polite">
      <NProgress
        type="line"
        :percentage="Math.round(((progress?.files || 0) / Math.max(1, chosen.length)) * 100)"
        :show-indicator="false"
        processing
      />
      <p>{{ progress?.current ? `正在清理 ${progress.current}` : '正在刷新项目状态…' }}</p>
    </div>
    <div v-if="result?.failed.length" class="cleanup-failures" role="alert">
      <p>已清理 {{ result.succeeded.length }} 个目录，{{ result.failed.length }} 个目录未完成：</p>
      <p v-for="failure in result.failed" :key="failure.path">
        {{ failure.path }}：{{ failure.message }}
      </p>
    </div>
    <p v-if="error" class="form-error" role="alert">{{ error }}</p>
    <template #footer
      ><div class="dialog-footer">
        <NButton :disabled="busy" @click="show = false">取消</NButton
        ><NButton
          type="error"
          :loading="busy"
          :disabled="!chosen.length || !!workspace.status.readOnly"
          @click="clean"
          >{{ result?.failed.length ? '重试失败项' : '确认清理' }}</NButton
        >
      </div></template
    >
  </NModal>
</template>
<style scoped>
.cleanup-summary {
  margin-bottom: 10px;
  font-weight: 600;
}
.cleanup-targets {
  margin-top: 20px;
  max-height: 280px;
  overflow-y: auto;
}
.cleanup-target {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 0;
}
.cleanup-target :deep(.n-checkbox) {
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
}
.cleanup-target :deep(.n-checkbox__label) {
  overflow-wrap: anywhere;
  min-width: 0;
}
.cleanup-target > span {
  white-space: nowrap;
  font-size: 12px;
}
.cleanup-progress {
  margin-top: 20px;
}
.cleanup-progress p,
.cleanup-failures p {
  margin-top: 8px;
  font-size: 12px;
  overflow-wrap: anywhere;
}
.cleanup-failures {
  color: #e24b4b;
  margin-top: 16px;
}
</style>
