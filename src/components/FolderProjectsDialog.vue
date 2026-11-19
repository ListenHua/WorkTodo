<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { NButton, NCheckbox, NInput, NModal, NSpin } from 'naive-ui'
import { desktopApi, type WorkspaceEntry } from '../services/desktop'
import { useWorkspaceStore } from '../stores/workspace'
import { useUiStore } from '../stores/ui'
import { useAppMessage } from '../composables/useAppMessage'
import { errorMessage, validateProjectName } from '../shared/domain'
import AppIcon from './AppIcon.vue'
import EmptyState from './EmptyState.vue'

const workspace = useWorkspaceStore()
const ui = useUiStore()
const message = useAppMessage()
const folders = ref<WorkspaceEntry[]>([])
const selected = ref<string[]>([])
const search = ref('')
const loading = ref(false)
const busy = ref(false)
const error = ref('')
const failures = ref<Record<string, string>>({})
const visible = computed(() =>
  folders.value.filter((folder) =>
    folder.name.toLocaleLowerCase().includes(search.value.trim().toLocaleLowerCase()),
  ),
)
const available = computed(() =>
  visible.value.filter((folder) => !validateProjectName(folder.name)),
)
const allSelected = computed(
  () =>
    available.value.length > 0 &&
    available.value.every((folder) => selected.value.includes(folder.name)),
)
let request = 0
let disposed = false

watch(
  () => ui.folderProjectsDialog,
  async (open) => {
    const version = ++request
    if (!open) return
    selected.value = []
    search.value = ''
    error.value = ''
    failures.value = {}
    folders.value = []
    loading.value = true
    const binding = workspace.status.bindingId
    try {
      const entries = await desktopApi.entries()
      if (disposed || version !== request || binding !== workspace.status.bindingId) return
      folders.value = entries.filter(
        (entry) => entry.directory && entry.name.toLocaleLowerCase() !== 'files',
      )
    } catch (cause) {
      if (!disposed && version === request) error.value = errorMessage(cause)
    } finally {
      if (!disposed && version === request) loading.value = false
    }
  },
)
onBeforeUnmount(() => {
  disposed = true
  request++
})
function newProject() {
  ui.folderProjectsDialog = false
  ui.newProject()
}
function toggle(name: string, checked: boolean) {
  selected.value = checked
    ? [...selected.value, name]
    : selected.value.filter((item) => item !== name)
}
function toggleAll(checked: boolean) {
  const names = available.value.map((folder) => folder.name)
  selected.value = checked
    ? [...new Set([...selected.value, ...names])]
    : selected.value.filter((name) => !names.includes(name))
}
async function create() {
  if (busy.value || !selected.value.length || workspace.status.readOnly) return
  const binding = workspace.status.bindingId
  busy.value = true
  failures.value = {}
  let created = 0
  try {
    for (const name of [...selected.value]) {
      if (disposed || binding !== workspace.status.bindingId) return
      try {
        const folder = folders.value.find((folder) => folder.name === name)
        if (!folder?.directoryMerge) throw new Error('文件夹信息已失效，请关闭后重新选择')
        await workspace.createProject({
          name,
          description: '',
          color: '#000000',
          directoryMerge: folder.directoryMerge,
        })
        if (disposed || binding !== workspace.status.bindingId) return
        created++
        folders.value = folders.value.filter((folder) => folder.name !== name)
        selected.value = selected.value.filter((item) => item !== name)
      } catch (cause) {
        if (disposed || binding !== workspace.status.bindingId) return
        failures.value[name] = errorMessage(cause)
      }
    }
    if (created) message.success(`已创建 ${created} 个项目`)
    if (!selected.value.length) ui.folderProjectsDialog = false
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <NModal
    v-model:show="ui.folderProjectsDialog"
    preset="card"
    title="从文件夹创建项目"
    aria-label="从文件夹创建项目"
    class="folder-projects-dialog"
    :style="{ width: '480px', maxWidth: 'calc(100vw - 32px)' }"
    :closable="!busy"
    :mask-closable="!busy"
    :close-on-esc="!busy"
  >
    <p class="folder-description">选中文件夹创建项目，原有内容将归入项目的 Files 文件夹。</p>
    <NInput
      v-model:value="search"
      placeholder="搜索文件夹…"
      aria-label="搜索文件夹"
      :disabled="busy || loading"
      clearable
    />
    <p v-if="error" class="form-error" role="alert">{{ error }}</p>
    <div v-if="loading" class="folder-loading" role="status" aria-label="正在读取文件夹">
      <NSpin size="small" />
    </div>
    <template v-else-if="folders.length">
      <NCheckbox
        :checked="allSelected"
        :indeterminate="!allSelected && available.some((folder) => selected.includes(folder.name))"
        :disabled="busy || !available.length"
        class="folder-select-all"
        @update:checked="toggleAll"
        >全选</NCheckbox
      >
      <ul class="folder-options" aria-label="可创建项目的文件夹">
        <li v-for="folder in visible" :key="folder.name">
          <NCheckbox
            :checked="selected.includes(folder.name)"
            :disabled="busy || !!validateProjectName(folder.name)"
            @update:checked="toggle(folder.name, $event)"
          >
            <span class="folder-option"
              ><AppIcon name="folder" :size="20" /><span>{{ folder.name }}</span></span
            >
          </NCheckbox>
          <p
            v-if="failures[folder.name] || validateProjectName(folder.name)"
            class="form-error"
            role="alert"
          >
            {{ failures[folder.name] || validateProjectName(folder.name) }}
          </p>
        </li>
      </ul>
      <EmptyState
        v-if="!visible.length"
        compact
        kind="search"
        title="没有匹配的文件夹"
        action-label="点击清除搜索"
        :disabled="busy"
        @action="search = ''"
      />
    </template>
    <EmptyState
      v-else-if="!error"
      kind="projects"
      title="暂无可创建项目的文件夹"
      action-label="点击新建项目"
      :disabled="busy || !!workspace.status.readOnly"
      @action="newProject"
    />
    <template #footer>
      <div class="folder-actions">
        <NButton :disabled="busy" @click="ui.folderProjectsDialog = false">取消</NButton>
        <NButton
          type="primary"
          :disabled="loading || !selected.length || !!workspace.status.readOnly"
          :loading="busy"
          @click="create"
          >创建项目{{ selected.length ? `（${selected.length}）` : '' }}</NButton
        >
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.folder-description {
  margin: 0 0 16px;
  color: var(--text-muted);
  font-size: 13px;
}
.folder-loading {
  display: grid;
  place-items: center;
  min-height: 160px;
}
.folder-select-all {
  margin: 18px 0 10px;
}
.folder-options {
  list-style: none;
  margin: 0;
  padding: 0;
  max-height: min(360px, 45vh);
  overflow-y: auto;
}
.folder-options li {
  padding: 12px 0;
}
.folder-options :deep(.n-checkbox) {
  display: flex;
  align-items: center;
}
.folder-options :deep(.n-checkbox__label) {
  min-width: 0;
}
.folder-option {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}
.folder-option span {
  overflow-wrap: anywhere;
}
.folder-options .form-error {
  margin: 6px 0 0 24px;
}
.folder-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}
</style>
