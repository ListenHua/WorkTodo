<script setup lang="ts">
import IconHint from './IconHint.vue'
import { computed, h, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  NButton,
  NCheckbox,
  NDropdown,
  NInput,
  NModal,
  NProgress,
  NSelect,
  NPopconfirm,
} from 'naive-ui'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit } from '@tauri-apps/api/event'
import { useAppMessage } from '../composables/useAppMessage'
import { desktop } from '../services/api'
import { filesApi, fileVersions } from '../services/files'
import { errorMessage } from '../shared/domain'
import { canPreviewFile, formatFileSize, sourceName } from '../shared/files'
import type { FileSource, ProjectFile } from '../shared/files'
import { useWorkspaceStore } from '../stores/workspace'
import AppIcon from './AppIcon.vue'
import EmptyState from './EmptyState.vue'

const props = defineProps<{ projectId: string }>()
const workspace = useWorkspaceStore()
const message = useAppMessage()
const files = ref<ProjectFile[]>([])
const changedIds = ref<string[]>([])
async function acceptChanges() {
  try {
    await fileVersions.accept(props.projectId, changedIds.value)
    await refresh()
    message.success('已确认当前文件版本')
  } catch (cause) {
    message.error(errorMessage(cause))
  }
}
const loading = ref(false)
const error = ref('')
const search = ref('')
const filter = ref('all')
const page = ref(1)
const pageSize = 50
const dropArea = ref<HTMLElement>()
const fileInput = ref<HTMLInputElement>()
const dragging = ref(false)
const picking = ref(false)
const working = ref(false)
const stopped = ref(false)
let disposed = false
let unlisten: UnlistenFn | undefined
const filters = [
  { label: '全部文件', value: 'all' },
  { label: '图片', value: 'image' },
  { label: '文档', value: 'document' },
  { label: '其他', value: 'other' },
]
const visibleFiles = computed(() =>
  files.value.filter(
    (file) =>
      (filter.value === 'all' || file.kind === filter.value) &&
      file.relativePath.toLocaleLowerCase().includes(search.value.trim().toLocaleLowerCase()),
  ),
)
const pageCount = computed(() => Math.max(1, Math.ceil(visibleFiles.value.length / pageSize)))
const pagedFiles = computed(() =>
  visibleFiles.value.slice((page.value - 1) * pageSize, page.value * pageSize),
)
const selecting = ref(false)
const selectedIds = ref(new Set<string>())
const selectedFiles = computed(() => files.value.filter((file) => selectedIds.value.has(file.id)))
const selectedOnPage = computed(
  () => pagedFiles.value.filter((file) => selectedIds.value.has(file.id)).length,
)
const pageSelected = computed(
  () => !!pagedFiles.value.length && selectedOnPage.value === pagedFiles.value.length,
)
const pageIndeterminate = computed(() => selectedOnPage.value > 0 && !pageSelected.value)
function selectFile(id: string, checked: boolean) {
  if (!selecting.value || busy.value) return
  const next = new Set(selectedIds.value)
  if (checked) next.add(id)
  else next.delete(id)
  selectedIds.value = next
}
function selectPage(checked: boolean) {
  if (!selecting.value || busy.value) return
  const next = new Set(selectedIds.value)
  for (const file of pagedFiles.value) {
    if (checked) next.add(file.id)
    else next.delete(file.id)
  }
  selectedIds.value = next
}
watch(files, (current) => {
  if (!current.length) selecting.value = false
  const ids = new Set(current.map((file) => file.id))
  selectedIds.value = new Set([...selectedIds.value].filter((id) => ids.has(id)))
})
watch([search, filter], () => {
  page.value = 1
  selectedIds.value = new Set()
})
watch(pageCount, (count) => {
  page.value = Math.min(page.value, count)
})

const contextVisible = ref(false)
const contextVersion = ref(0)
const contextFile = ref<ProjectFile>()
let contextRow: HTMLElement | undefined
const contextPosition = ref({ x: 0, y: 0 })
const deleteTargets = ref<ProjectFile[]>([])
const deleteMode = ref<'single' | 'batch'>('single')
const deleteFailures = ref<{ file: ProjectFile; error: string }[]>([])
const deleteProgress = ref(0)
const deleting = ref(false)
const busy = computed(() => working.value || deleting.value || loading.value)
const deleteTitle = computed(() => (deleteMode.value === 'batch' ? '批量删除文件' : '删除文件'))
const deleteFile = computed(() => deleteTargets.value[0])
function beginSelection() {
  if (busy.value || !files.value.length) return
  closeContextMenu()
  selecting.value = true
}
function endSelection() {
  if (deleting.value) return
  selecting.value = false
  selectedIds.value = new Set()
}
function requestDelete(targets: ProjectFile[], mode: 'single' | 'batch') {
  if (busy.value || !targets.length) return
  closeContextMenu()
  deleteMode.value = mode
  deleteTargets.value = [...targets]
  deleteFailures.value = []
  deleteProgress.value = 0
}
function dismissDelete() {
  if (deleting.value) return
  deleteTargets.value = []
  deleteFailures.value = []
}
const menuIcon = (name: string) => () => h(AppIcon, { name, size: 18 })
const contextOptions = computed(() => {
  const file = contextFile.value
  const unavailable = !file || file.status === 'missing'
  return [
    {
      label: '打开',
      key: 'open',
      icon: menuIcon('external'),
      disabled: !desktop || unavailable || file?.kind === 'other',
    },
    {
      label: '打开文件所在位置',
      key: 'reveal',
      icon: menuIcon('folder'),
      disabled: !desktop || unavailable,
    },
    ...(file && canPreviewFile(file)
      ? [{ label: '预览', key: 'preview', icon: menuIcon('eye') }]
      : []),
    { type: 'divider', key: 'divider' },
    {
      label: '删除',
      key: 'delete',
      icon: menuIcon('delete'),
      disabled: !file || busy.value,
    },
  ]
})
async function showContextMenu(event: MouseEvent | KeyboardEvent, file: ProjectFile) {
  if (deleting.value) return
  event.preventDefault()
  event.stopPropagation()
  const row = event.currentTarget as HTMLElement
  contextRow = row
  row.focus({ preventScroll: true })
  const rect = row.getBoundingClientRect()
  contextVisible.value = false
  // Recreate the manual popup so an earlier closing transition cannot consume
  // the mouse release that belongs to this new context-menu request.
  contextVersion.value++
  contextFile.value = file
  contextPosition.value =
    event instanceof MouseEvent && event.button === 2
      ? { x: event.clientX, y: event.clientY }
      : { x: rect.left + 24, y: rect.top + rect.height / 2 }
  await nextTick()
  if (!disposed) contextVisible.value = true
}
function selectContextAction(key: string) {
  const file = contextFile.value
  contextVisible.value = false
  if (!file) return
  if (key === 'open') void open(file)
  if (key === 'reveal') void open(file, true)
  if (key === 'preview') void preview(file)
  if (key === 'delete') requestDelete([file], 'single')
}
async function confirmDelete() {
  if (!deleteTargets.value.length || busy.value) return
  const targets = [...deleteTargets.value]
  const removed = new Set<string>()
  const failures: { file: ProjectFile; error: string }[] = []
  let recycled = 0
  deleting.value = true
  deleteProgress.value = 0
  deleteFailures.value = []
  error.value = ''
  try {
    for (const file of targets) {
      if (disposed) break
      try {
        const result = await filesApi.remove(file.projectId, file.id)
        removed.add(file.id)
        if (result === 'recycled') recycled++
      } catch (cause) {
        failures.push({ file, error: errorMessage(cause) })
      }
      deleteProgress.value++
    }
    if (removed.size && desktop) void emit('workspace-changed')
    if (disposed) return
    files.value = files.value.filter((file) => !removed.has(file.id))
    if (previewFile.value && removed.has(previewFile.value.id)) previewVisible.value = false
    deleteTargets.value = failures.map(({ file }) => file)
    deleteFailures.value = failures
    if (deleteMode.value === 'batch' && !failures.length) {
      selecting.value = false
      selectedIds.value = new Set()
    }
    if (removed.size) {
      message.success(
        deleteMode.value === 'batch'
          ? `已删除 ${removed.size} 个文件`
          : recycled
            ? '文件已移到存储目录的 _回收站'
            : '文件记录已删除',
      )
    }
    if (failures.length) {
      message.error(
        deleteMode.value === 'batch'
          ? `${failures.length} 个文件删除失败，请重试`
          : failures[0]!.error,
      )
    }
  } finally {
    deleting.value = false
    if (!disposed && jobs.value.some((job) => job.status === 'queued')) void processQueue()
  }
}
function closeContextMenu() {
  contextVisible.value = false
}
function onContextScroll(event: Event) {
  const target = event.target
  if (
    target === document ||
    (target instanceof Element && contextRow && target.contains(contextRow))
  )
    closeContextMenu()
}
watch([search, filter, page], closeContextMenu)

interface ImportJob {
  id: string
  source: FileSource
  name: string
  status: 'queued' | 'copying' | 'done' | 'error' | 'canceled'
  percent: number
  error: string
}
const jobs = ref<ImportJob[]>([])
const pendingJobs = computed(() =>
  jobs.value.filter((job) => job.status === 'queued' || job.status === 'copying'),
)
const activeJob = computed(() => jobs.value.find((job) => job.status === 'copying'))
const previewVisible = ref(false)
const previewFile = ref<ProjectFile>()
const previewUrl = ref('')
const previewLoading = ref(false)
const previewError = ref('')
let previewRequest = 0

async function refresh(scan = true) {
  if (disposed) return
  loading.value = true
  error.value = ''
  try {
    // Show the saved index even if the following scan cannot access the folder.
    const cached = await filesApi.list(props.projectId)
    if (!disposed) files.value = cached
    if (scan) {
      const latest = await filesApi.scan(props.projectId)
      if (!disposed) files.value = latest
    }
    const changed = await fileVersions.changed(props.projectId)
    if (!disposed) changedIds.value = changed
  } catch (cause) {
    if (!disposed) error.value = errorMessage(cause)
  } finally {
    if (!disposed) loading.value = false
  }
}
async function choose() {
  if (!desktop) {
    fileInput.value?.click()
    return
  }
  picking.value = true
  try {
    const paths = await filesApi.pick()
    if (!disposed) enqueue(paths.map((path) => ({ kind: 'native', path })))
  } catch (cause) {
    error.value = errorMessage(cause)
  } finally {
    picking.value = false
  }
}
function selected(event: Event) {
  const input = event.target as HTMLInputElement
  enqueue([...(input.files || [])].map((file) => ({ kind: 'browser', file })))
  input.value = ''
}
function enqueue(sources: FileSource[]) {
  if (disposed || !sources.length) return
  jobs.value.push(
    ...sources.map((source): ImportJob => ({
      id: crypto.randomUUID(),
      source,
      name: sourceName(source),
      status: 'queued',
      percent: 0,
      error: '',
    })),
  )
  void processQueue()
}
async function processQueue() {
  if (working.value || deleting.value || disposed) return
  working.value = true
  stopped.value = false
  let imported = 0
  const attempted: ImportJob[] = []
  try {
    let job: ImportJob | undefined
    while (
      !stopped.value &&
      !disposed &&
      (job = jobs.value.find((entry) => entry.status === 'queued'))
    ) {
      const current = job
      attempted.push(current)
      current.status = 'copying'
      let cancelSent = false
      try {
        await filesApi.importFile(
          props.projectId,
          current.source,
          current.id,
          ({ copied, total }) => {
            current.percent = total ? Math.min(100, Math.round((copied / total) * 100)) : 0
            // The first progress event also confirms that native cancellation is registered.
            if ((stopped.value || disposed) && !cancelSent) {
              cancelSent = true
              void filesApi.cancel(current.id).catch(() => {})
            }
          },
        )
        current.status = 'done'
        current.percent = 100
        imported++
      } catch (cause) {
        const canceled =
          typeof cause === 'object' &&
          cause !== null &&
          'code' in cause &&
          cause.code === 'IMPORT_CANCELED'
        current.status = canceled ? 'canceled' : 'error'
        current.error = canceled ? '' : errorMessage(cause)
      }
    }
    for (const entry of jobs.value)
      if (entry.status === 'queued') {
        entry.status = 'canceled'
        attempted.push(entry)
      }
    if (!disposed) {
      if (imported) message.success(`已添加 ${imported} 个文件`)
      const failed = attempted.filter((job) => job.status === 'error' || job.status === 'canceled')
      if (failed.length) {
        const notify = failed.some((job) => job.status === 'error') ? message.error : message.info
        const notice = notify(
          () =>
            h('div', { class: 'file-import-notice' }, [
              h('strong', `${failed.length} 个文件未添加`),
              ...failed
                .slice(0, 3)
                .map((job) => h('div', [h('span', job.name), h('p', job.error || '已取消')])),
              h(
                NButton,
                {
                  text: true,
                  size: 'small',
                  disabled: working.value || deleting.value,
                  onClick: () => {
                    if (working.value || deleting.value) return
                    notice.destroy()
                    if (disposed) return
                    for (const job of failed) {
                      job.id = crypto.randomUUID()
                      job.status = 'queued'
                      job.error = ''
                      job.percent = 0
                    }
                    void processQueue()
                  },
                },
                () => '重试',
              ),
            ]),
          { duration: 0, closable: true },
        )
      }
      jobs.value = jobs.value.filter((job) => job.status === 'error' || job.status === 'canceled')
      await refresh(false)
      await workspace.refresh()
    }
  } finally {
    working.value = false
    if (!disposed && jobs.value.some((job) => job.status === 'queued')) void processQueue()
  }
}
async function cancel() {
  stopped.value = true
  const current = activeJob.value
  if (current) {
    try {
      await filesApi.cancel(current.id)
    } catch (cause) {
      if (!disposed) error.value = errorMessage(cause)
    }
  }
}
function browserDrag(event: DragEvent) {
  if (!desktop && event.dataTransfer?.types.includes('Files')) dragging.value = true
}
function browserDrop(event: DragEvent) {
  dragging.value = false
  if (!desktop)
    enqueue([...(event.dataTransfer?.files || [])].map((file) => ({ kind: 'browser', file })))
}
function clearPreviewUrl() {
  if (previewUrl.value) URL.revokeObjectURL(previewUrl.value)
  previewUrl.value = ''
}
watch(previewVisible, (show) => {
  if (!show) {
    previewRequest++
    clearPreviewUrl()
  }
})
async function preview(file: ProjectFile) {
  const request = ++previewRequest
  clearPreviewUrl()
  previewFile.value = file
  previewVisible.value = true
  previewLoading.value = true
  previewError.value = ''
  try {
    const blob = await filesApi.preview(props.projectId, file)
    if (!disposed && previewVisible.value && request === previewRequest)
      previewUrl.value = URL.createObjectURL(blob)
  } catch (cause) {
    if (!disposed && request === previewRequest) previewError.value = errorMessage(cause)
  } finally {
    if (request === previewRequest) previewLoading.value = false
  }
}
async function open(file: ProjectFile, reveal = false) {
  try {
    await filesApi.open(props.projectId, file.id, reveal)
  } catch (cause) {
    error.value = errorMessage(cause)
  }
}
function modifiedAt(value: string) {
  return new Date(value).toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}
onMounted(async () => {
  void refresh()
  window.addEventListener('scroll', onContextScroll, true)
  window.addEventListener('resize', closeContextMenu)
  window.addEventListener('blur', closeContextMenu)
  if (!desktop) return
  try {
    const { getCurrentWebview } = await import('@tauri-apps/api/webview')
    const stop = await getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (disposed) return
      if (payload.type === 'leave') {
        dragging.value = false
        return
      }
      const area = dropArea.value?.getBoundingClientRect()
      const x = payload.position.x / window.devicePixelRatio
      const y = payload.position.y / window.devicePixelRatio
      const inside =
        !!area && x >= area.left && x <= area.right && y >= area.top && y <= area.bottom
      dragging.value = inside
      if (payload.type === 'drop') {
        dragging.value = false
        if (inside) enqueue(payload.paths.map((path) => ({ kind: 'native', path })))
      }
    })
    if (disposed) stop()
    else unlisten = stop
  } catch {
    if (!disposed) error.value = '暂时无法拖入文件，请使用“添加文件”'
  }
})
onBeforeUnmount(() => {
  disposed = true
  window.removeEventListener('scroll', onContextScroll, true)
  window.removeEventListener('resize', closeContextMenu)
  window.removeEventListener('blur', closeContextMenu)
  void cancel()
  unlisten?.()
  previewRequest++
  clearPreviewUrl()
})
</script>

<template>
  <section
    ref="dropArea"
    class="panel project-files"
    :class="{ 'files-dragging': dragging }"
    aria-label="项目文件"
    @dragover.prevent="browserDrag"
    @dragleave.self="dragging = false"
    @drop.prevent.stop="browserDrop"
  >
    <div class="files-toolbar" :class="{ 'files-toolbar-selecting': selecting }">
      <div class="file-filters">
        <span class="file-count" aria-label="文件数量" role="status"
          >{{ visibleFiles.length }} 个文件</span
        >
        <NInput
          v-model:value="search"
          placeholder="搜索文件…"
          :disabled="deleting"
          clearable
          class="file-search"
        >
          <template #prefix><AppIcon name="search" :size="16" /></template>
          <template #clear-icon><AppIcon name="clear" :size="16" /></template>
        </NInput>
        <NSelect
          v-model:value="filter"
          :disabled="deleting"
          :options="filters"
          aria-label="文件类型"
          class="file-filter"
        >
          <template #arrow><AppIcon name="down" :size="16" /></template>
        </NSelect>
      </div>
      <div class="file-actions">
        <NPopconfirm v-if="changedIds.length" :show-icon="false" @positive-click="acceptChanges">
          <template #trigger
            ><NButton :disabled="busy">确认变更（{{ changedIds.length }}）</NButton></template
          >
          将当前磁盘上的文件确认为新版本，更新备份校验摘要。请先确认文件已完整同步。
        </NPopconfirm>
        <NButton
          v-if="!selecting"
          type="error"
          secondary
          :disabled="busy || !files.length"
          @click="beginSelection"
          ><AppIcon name="delete" :size="17" />批量删除</NButton
        >
        <NButton
          v-else
          type="error"
          secondary
          :disabled="busy || !selectedFiles.length"
          @click="requestDelete(selectedFiles, 'batch')"
          ><AppIcon name="delete" :size="17" />删除所选</NButton
        >
        <NButton type="primary" :disabled="loading || deleting" :loading="picking" @click="choose"
          ><AppIcon name="plus" :size="17" />添加文件</NButton
        >
        <NButton
          v-if="selecting"
          text
          :disabled="deleting"
          aria-label="取消批量删除"
          @click="endSelection"
          >取消</NButton
        >
      </div>
      <div v-if="selecting && files.length" class="file-selection-bar">
        <div class="file-selection-actions">
          <NCheckbox
            :checked="pageSelected"
            :indeterminate="pageIndeterminate"
            :disabled="busy || !pagedFiles.length"
            aria-label="全选当前页"
            @update:checked="selectPage"
            >全选当前页</NCheckbox
          >
          <span
            v-if="selectedFiles.length"
            class="file-selection-count"
            role="status"
            aria-label="已选文件数量"
            >已选 {{ selectedFiles.length }} 项</span
          >
        </div>
      </div>
    </div>
    <input
      ref="fileInput"
      type="file"
      multiple
      hidden
      aria-label="选择项目文件"
      @change="selected"
    />
    <p v-if="error" class="form-error files-error" role="alert">{{ error }}</p>
    <p v-if="workspace.status.pendingOperations" class="form-error files-error">
      有未完成的文件操作，请检查项目目录；未完成的副本不会进入文件列表。
    </p>
    <div v-if="working && pendingJobs.length" class="import-queue" aria-label="文件添加进度">
      <div class="import-queue-heading">
        <span>正在添加</span
        ><NButton size="small" :disabled="stopped" @click="cancel">取消添加</NButton>
      </div>
      <div v-for="job in pendingJobs" :key="job.id" class="import-job">
        <div class="import-job-line">
          <span class="import-job-name">{{ job.name }}</span><span>{{
            job.status === 'queued' ? '等待中' : `${job.percent}%`
          }}</span>
        </div>
        <NProgress
          v-if="job.status === 'copying'"
          type="line"
          :percentage="job.percent"
          :show-indicator="false"
        />
      </div>
    </div>
    <div v-if="!files.length && !loading && !error" class="file-drop-empty">
      <span class="empty-icon"><AppIcon name="upload" :size="32" /></span>
      <p>拖入文件，或点击添加文件</p>
    </div>
    <template v-else-if="visibleFiles.length">
      <div class="file-list" role="list" aria-label="文件列表">
        <article
          v-for="file in pagedFiles"
          :key="file.id"
          class="file-row"
          :class="{
            'file-missing': file.status === 'missing',
            'file-selected': selectedIds.has(file.id),
          }"
          role="listitem"
          tabindex="0"
          aria-haspopup="menu"
          :aria-label="file.name"
          @contextmenu="showContextMenu($event, file)"
          @keydown.shift.f10.prevent.stop="showContextMenu($event, file)"
          @keydown="$event.code === 'ContextMenu' && showContextMenu($event, file)"
        >
          <NCheckbox
            v-if="selecting"
            class="file-checkbox"
            :checked="selectedIds.has(file.id)"
            :disabled="busy"
            :aria-label="`选择 ${file.name}`"
            @update:checked="(checked) => selectFile(file.id, checked)"
            @click.stop
            @keydown.stop
          />
          <span class="file-type-icon"
            ><AppIcon :name="file.kind === 'image' ? 'image' : 'file'" :size="25"
          /></span>
          <div class="file-info">
            <button
              v-if="canPreviewFile(file)"
              class="file-name"
              :title="file.relativePath"
              @click="preview(file)"
            >
              {{ file.name }}
            </button>
            <span v-else class="file-name" :title="file.relativePath">{{ file.name }}</span>
            <p class="file-meta">
              <span>{{ formatFileSize(file.size) }}</span><span>{{
                modifiedAt(file.modifiedAt)
              }}</span
              ><span v-if="file.relativePath !== file.name">{{ file.relativePath }}</span>
            </p>
          </div>
          <span v-if="file.status === 'missing'" class="file-missing-label">文件缺失</span>
          <span v-else-if="changedIds.includes(file.id)" class="file-missing-label"
            >文件已变化</span
          >
          <div v-else class="file-row-actions">
            <IconHint text="预览" v-if="canPreviewFile(file)"
              ><button class="icon-button" :aria-label="`预览 ${file.name}`" @click="preview(file)">
                <AppIcon name="eye" :size="18" /></button
            ></IconHint>
            <template v-if="desktop">
              <IconHint text="使用系统程序打开" v-if="file.kind !== 'other'"
                ><button class="icon-button" :aria-label="`打开 ${file.name}`" @click="open(file)">
                  <AppIcon name="external" :size="18" /></button
              ></IconHint>
              <IconHint text="在文件夹中显示"
                ><button
                  class="icon-button"
                  :aria-label="`定位 ${file.name}`"
                  @click="open(file, true)"
                >
                  <AppIcon name="folder" :size="18" /></button
              ></IconHint>
            </template>
          </div>
        </article>
      </div>
      <div v-if="pageCount > 1" class="files-pagination">
        <div class="file-actions">
          <NPopconfirm v-if="changedIds.length" :show-icon="false" @positive-click="acceptChanges">
            <template #trigger
              ><NButton :disabled="busy">确认变更（{{ changedIds.length }}）</NButton></template
            >
            将当前磁盘上的文件确认为新版本，更新备份校验摘要。请先确认文件已完整同步。
          </NPopconfirm>
          <NButton size="small" :disabled="deleting || page === 1" @click="page--">上一页</NButton>
          <span class="file-meta">{{ page }} / {{ pageCount }}</span>
          <NButton size="small" :disabled="deleting || page === pageCount" @click="page++"
            >下一页</NButton
          >
        </div>
      </div>
    </template>
    <EmptyState v-else-if="files.length && !loading" icon="search" title="没有找到匹配的文件" />
    <p v-if="dragging" class="file-drop-overlay">
      <AppIcon name="upload" :size="30" />松开即可添加
    </p>
  </section>
  <NDropdown
    :key="contextVersion"
    trigger="manual"
    :show="contextVisible"
    :x="contextPosition.x"
    :y="contextPosition.y"
    :options="contextOptions"
    :menu-props="() => ({ role: 'menu', 'aria-label': '文件操作' })"
    :node-props="
      (option) => ({
        role: 'menuitem',
        class: option.key === 'delete' ? 'file-menu-delete' : '',
        'aria-disabled': option.disabled ? ('true' as const) : ('false' as const),
      })
    "
    placement="bottom-start"
    @select="selectContextAction"
    @clickoutside="closeContextMenu"
    @update:show="contextVisible = $event"
  />
  <NModal
    :show="!!deleteTargets.length"
    preset="dialog"
    :title="deleteTitle"
    :aria-label="deleteTitle"
    :show-icon="false"
    :closable="false"
    :mask-closable="!deleting"
    :close-on-esc="!deleting"
    @update:show="
      (show) => {
        if (!show) dismissDelete()
      }
    "
  >
    <p v-if="deleteMode === 'batch'">
      删除选中的 {{ deleteTargets.length }} 个文件及其待办附件关联？{{
        desktop ? '实际文件将移到存储目录的 _回收站，缺失文件仅清理记录。' : ''
      }}
    </p>
    <ul
      v-if="deleteMode === 'batch' && !deleteFailures.length"
      class="file-delete-list"
      aria-label="待删除文件"
    >
      <li v-for="file in deleteTargets" :key="file.id">
        {{ file.name }}{{ file.status === 'missing' ? '（文件缺失）' : '' }}
      </li>
    </ul>
    <p v-if="deleteMode === 'single'">
      {{
        desktop
          ? deleteFile?.status === 'missing'
            ? `删除“${deleteFile.name}”的缺失记录及所有待办附件关联？如果文件已重新出现，将移到存储目录的 _回收站。`
            : `将“${deleteFile?.name}”移到存储目录的 _回收站，并解除所有待办附件关联。可在文件管理器中找回，项目目录之外的原文件不受影响。`
          : `删除“${deleteFile?.name}”及其待办附件关联？`
      }}
    </p>
    <ul
      v-if="deleteFailures.length"
      class="file-delete-list form-error"
      aria-label="删除失败的文件"
    >
      <li v-for="failure in deleteFailures" :key="failure.file.id">
        {{ failure.file.name }}：{{ failure.error }}
      </li>
    </ul>
    <p v-if="deleting && deleteMode === 'batch'" class="muted" role="status">
      已处理 {{ deleteProgress }} / {{ deleteTargets.length }} 项
    </p>
    <template #action>
      <NButton :disabled="deleting" @click="dismissDelete">取消</NButton>
      <NButton
        type="error"
        :loading="deleting"
        :disabled="working || loading"
        @click="confirmDelete"
        >{{ deleteMode === 'batch' && deleteFailures.length ? '重试删除' : '删除' }}</NButton
      >
    </template>
  </NModal>
  <NModal
    v-model:show="previewVisible"
    preset="card"
    :title="previewFile?.name"
    :bordered="false"
    :closable="false"
    class="image-preview-modal"
    aria-label="图片预览"
  >
    <template #header-extra
      ><button class="icon-button" aria-label="关闭图片预览" @click="previewVisible = false">
        <AppIcon name="close" :size="20" /></button
    ></template>
    <div class="image-preview-body">
      <p v-if="previewLoading" role="status">正在加载…</p>
      <p v-else-if="previewError" class="form-error" role="alert">{{ previewError }}</p>
      <img
        v-else-if="previewUrl"
        :src="previewUrl"
        :alt="previewFile?.name"
        @error="previewError = '图片无法解码，请使用系统程序打开'"
      />
    </div>
  </NModal>
</template>
