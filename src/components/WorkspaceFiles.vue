<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { desktopApi, type WorkspaceEntry } from '../services/desktop'
import { useWorkspaceStore } from '../stores/workspace'
import { useUiStore } from '../stores/ui'
import { useAppMessage } from '../composables/useAppMessage'
import { errorMessage } from '../shared/domain'
import AppIcon from './AppIcon.vue'

const workspace = useWorkspaceStore()
const ui = useUiStore()
const message = useAppMessage()
const entries = ref<WorkspaceEntry[]>([])
const error = ref('')
const visible = computed(() =>
  entries.value.filter((entry) =>
    entry.name.toLocaleLowerCase().includes(ui.search.trim().toLocaleLowerCase()),
  ),
)
let request = 0
let closed = false
async function load() {
  const version = ++request
  if (!workspace.status.available || workspace.status.readOnly) {
    entries.value = []
    return
  }
  try {
    const next = await desktopApi.entries()
    if (closed || version !== request) return
    entries.value = next
    error.value = ''
  } catch (cause) {
    if (!closed && version === request) error.value = errorMessage(cause)
  }
}
async function open(entry: WorkspaceEntry) {
  const binding = workspace.status.bindingId
  if (!binding) return
  try {
    await desktopApi.openEntry(entry.name, binding)
  } catch (cause) {
    message.error(errorMessage(cause))
  }
}
watch(
  () => workspace.status.bindingId,
  () => void load(),
  { immediate: true },
)
onMounted(() => window.addEventListener('focus', load))
onUnmounted(() => {
  closed = true
  window.removeEventListener('focus', load)
})
</script>

<template>
  <section v-if="visible.length || error" class="workspace-files" aria-label="目录文件">
    <h2>目录文件</h2>
    <p v-if="error" class="form-error" role="alert">{{ error }}</p>
    <ul v-if="visible.length" class="workspace-entry-list">
      <li v-for="entry in visible" :key="entry.name">
        <button
          type="button"
          :aria-label="`打开${entry.directory ? '文件夹' : '文件'} ${entry.name}`"
          @click="open(entry)"
        >
          <AppIcon :name="entry.directory ? 'folder' : 'file'" :size="21" />
          <span>{{ entry.name }}</span>
        </button>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.workspace-files {
  margin-top: 28px;
}
h2 {
  font-size: 16px;
  margin: 0 0 14px;
}
.workspace-entry-list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: 10px;
  padding: 0;
  margin: 0;
  list-style: none;
}
button {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 14px;
  border: 0;
  border-radius: 10px;
  background: var(--surface);
  color: var(--text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}
button:hover {
  background: var(--surface-subtle);
}
button:focus-visible {
  outline: 2px solid currentColor;
  outline-offset: 2px;
}
span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
