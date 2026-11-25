<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { NButton, NTab, NTabs } from 'naive-ui'
import type { CodeDirectory } from '../shared/types'
import { desktop } from '../services/api'
import { useWorkspaceStore } from '../stores/workspace'
import { useCodeSpaceStore } from '../stores/code-space'
import AppIcon from './AppIcon.vue'
import EmptyState from './EmptyState.vue'
import CodeDirectoryDetails from './CodeDirectoryDetails.vue'
import CodeDirectoryDialog from './CodeDirectoryDialog.vue'
const props = defineProps<{ projectId: string }>()
const workspace = useWorkspaceStore(),
  code = useCodeSpaceStore()
const directories = computed(
  () => workspace.projects.find((p) => p.id === props.projectId)?.codeDirectories || [],
)
const selectedId = computed({
  get: () => code.selected[props.projectId] || '',
  set: (id: string) => {
    code.selected[props.projectId] = id
  },
})
const selected = computed(() => directories.value.find((c) => c.id === selectedId.value))
const missing = computed(
  () => selected.value && code.entry(props.projectId, selected.value.id).git?.state === 'missing',
)
const disabled = computed(
  () => !desktop || !!workspace.status.readOnly || !workspace.status.available,
)
const dialogVisible = ref(false),
  target = ref<CodeDirectory | null>(null)
type DirectoryAction = 'add' | 'edit' | 'delete'
const mode = ref<DirectoryAction>('add')
watch(
  [directories, () => props.projectId],
  ([current, projectId], [previous, previousProjectId]) => {
    if (current.some((directory) => directory.id === selectedId.value)) return
    const previousIndex =
      projectId === previousProjectId
        ? (previous?.findIndex((directory) => directory.id === selectedId.value) ?? 0)
        : 0
    selectedId.value = current[Math.max(0, Math.min(previousIndex, current.length - 1))]?.id || ''
  },
  { immediate: true },
)
function open(action: DirectoryAction, directory = selected.value) {
  if (disabled.value || (action !== 'add' && !directory)) return
  mode.value = action
  target.value = action === 'add' ? null : { ...directory! }
  dialogVisible.value = true
}
function onSaved(id: string) {
  if (mode.value === 'add') selectedId.value = id
}
function onTabKeydown(event: KeyboardEvent, id: string) {
  const index = directories.value.findIndex((directory) => directory.id === id)
  const last = directories.value.length - 1
  let next = index
  switch (event.key) {
    case 'ArrowRight':
      next = index === last ? 0 : index + 1
      break
    case 'ArrowLeft':
      next = index === 0 ? last : index - 1
      break
    case 'Home':
      next = 0
      break
    case 'End':
      next = last
      break
    case 'Enter':
    case ' ':
      break
    default:
      return
  }
  event.preventDefault()
  selectedId.value = directories.value[next]!.id
  const tabs = (event.currentTarget as HTMLElement)
    .closest('.code-directory-tabs')
    ?.querySelectorAll<HTMLElement>('[role="tab"]')
  tabs?.[next]?.focus()
}
</script>
<template>
  <section class="project-code-space" aria-label="项目代码空间">
    <div v-if="directories.length" class="code-selector">
      <NTabs
        v-model:value="selectedId"
        type="line"
        class="code-directory-tabs"
        role="tablist"
        aria-label="代码目录"
      >
        <NTab
          v-for="directory in directories"
          :key="directory.id"
          :name="directory.id"
          role="tab"
          :aria-label="directory.name"
          :aria-selected="selectedId === directory.id"
          :tabindex="selectedId === directory.id ? 0 : -1"
          @keydown="onTabKeydown($event, directory.id)"
        >
          <span class="code-directory-label">
            <span>{{ directory.name }}</span>
            <span class="code-directory-actions" @mousedown.stop @keydown.stop>
              <button
                type="button"
                class="icon-button code-directory-action"
                :aria-label="`编辑代码目录：${directory.name}`"
                :disabled="disabled"
                @click.stop="open('edit', directory)"
              >
                <AppIcon name="edit" :size="16" />
              </button>
              <button
                type="button"
                class="icon-button code-directory-action code-directory-delete"
                :aria-label="`删除代码关联：${directory.name}`"
                :disabled="disabled"
                @click.stop="open('delete', directory)"
              >
                <AppIcon name="delete" :size="16" />
              </button>
            </span>
          </span>
        </NTab>
      </NTabs>
      <NButton :disabled="disabled" @click="open('add')">关联代码</NButton>
    </div>
    <EmptyState
      v-if="!directories.length"
      kind="files"
      title="暂无关联代码"
      action-label="点击关联"
      :disabled="disabled"
      @action="open('add')"
    />
    <template v-else-if="selected">
      <EmptyState
        v-if="missing"
        compact
        kind="files"
        title="代码目录不可用"
        action-label="点击重新选择"
        :disabled="disabled"
        @action="open('edit')"
      />
      <CodeDirectoryDetails
        :key="`${projectId}:${selected.id}:${selected.path}`"
        :project-id="projectId"
        :code-id="selected.id"
        :code-path="selected.path"
      />
    </template>
    <CodeDirectoryDialog
      v-model="dialogVisible"
      :project-id="projectId"
      :target="target"
      :mode="mode"
      @saved="onSaved"
    />
  </section>
</template>
<style scoped>
.project-code-space {
  min-width: 0;
}
.code-selector {
  display: flex;
  align-items: center;
  gap: 14px;
  margin-bottom: 16px;
}
.code-directory-tabs {
  flex: 1;
  min-width: 0;
}
.code-directory-label,
.code-directory-actions {
  display: inline-flex;
  align-items: center;
}
.code-directory-label {
  gap: 8px;
}
.code-directory-actions {
  flex: none;
  gap: 2px;
  opacity: 0;
  pointer-events: none;
  transition: opacity 120ms ease;
}
.code-directory-tabs :deep(.n-tabs-tab:hover) .code-directory-actions,
.code-directory-tabs :deep(.n-tabs-tab:focus-within) .code-directory-actions {
  opacity: 1;
  pointer-events: auto;
}
.code-directory-action {
  width: 24px;
  height: 24px;
  color: var(--muted);
}
.code-directory-action:hover {
  color: var(--text);
}
.code-directory-delete:hover {
  color: var(--danger);
}
.code-directory-action:focus-visible,
.code-directory-tabs :deep(.n-tabs-tab:focus-visible) {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
}
@media (prefers-reduced-motion: reduce) {
  .code-directory-actions {
    transition: none;
  }
}
</style>
