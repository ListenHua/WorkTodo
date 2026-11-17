<script setup lang="ts">
import IconHint from './IconHint.vue'
import AppIcon from './AppIcon.vue'
import { useAppMessage } from '../composables/useAppMessage'
import { ref, watch } from 'vue'
import { NButton, NModal, NInput, NSelect } from 'naive-ui'
import { useUiStore } from '../stores/ui'
import { useWorkspaceStore } from '../stores/workspace'
import { errorMessage, validateProjectName } from '../shared/domain'
import ProjectTagsEditor from './ProjectTagsEditor.vue'
import AppColorPicker from './AppColorPicker.vue'
import { validateProjectTags } from '../shared/project-tags'
import { projectDirectoryConflict } from '../shared/project-directory'
import type { CreateProject, Project, ProjectTag, ProjectDirectoryConflict } from '../shared/types'
import { projectColors } from '../shared/types'
const ui = useUiStore()
const workspace = useWorkspaceStore()
const message = useAppMessage()
const name = ref('')
const description = ref('')
const color = ref<string>(projectColors[0])
const tags = ref<ProjectTag[]>([])
const status = ref<Project['status']>('active')
const pendingPolicy = ref<'inbox' | 'complete' | 'cancel'>('inbox')
const busy = ref(false)
const error = ref('')
const pendingMerge = ref<{ directory: ProjectDirectoryConflict; input: CreateProject } | null>(null)
watch(
  () => [ui.projectDialog, ui.editingProject?.id] as const,
  ([open]) => {
    pendingMerge.value = null
    if (open) {
      name.value = ui.editingProject?.name || ''
      description.value = ui.editingProject?.description || ''
      color.value = ui.editingProject?.color || projectColors[0]
      tags.value = (ui.editingProject?.tags ?? []).map((tag) => ({ ...tag }))
      status.value = ui.editingProject?.status || 'active'
      pendingPolicy.value = 'inbox'
      error.value = ''
    }
  },
)
async function save() {
  error.value = validateProjectName(name.value) || ''
  if (error.value || busy.value) return
  try {
    const input: CreateProject = {
      name: name.value,
      description: description.value,
      color: color.value,
      tags: validateProjectTags(tags.value),
      ...(ui.editingProject ? { status: status.value, pendingPolicy: pendingPolicy.value } : {}),
    }
    await persist(input)
  } catch (cause) {
    error.value = errorMessage(cause)
  }
}
async function persist(input: CreateProject) {
  if (busy.value) return
  busy.value = true
  error.value = ''
  try {
    if (ui.editingProject) await workspace.updateProject(ui.editingProject.id, input)
    else await workspace.createProject(input)
    const text = ui.editingProject
      ? '项目已保存'
      : input.directoryMerge
        ? '项目已合并创建'
        : '项目已创建'
    pendingMerge.value = null
    ui.projectDialog = false
    message.success(text)
  } catch (cause) {
    const directory = !ui.editingProject && projectDirectoryConflict(cause)
    if (directory) {
      pendingMerge.value = { directory, input }
    } else {
      pendingMerge.value = null
      error.value = errorMessage(cause)
    }
  } finally {
    busy.value = false
  }
}
async function confirmMerge() {
  const pending = pendingMerge.value
  if (!pending || busy.value) return
  await persist({
    ...pending.input,
    name: pending.directory.name,
    directoryMerge: { path: pending.directory.path, fingerprint: pending.directory.fingerprint },
  })
}
</script>
<template>
  <NModal
    v-model:show="ui.projectDialog"
    :title="ui.editingProject ? '编辑项目' : '新建项目'"
    preset="card"
    class="form-modal project-form-modal"
    :aria-label="ui.editingProject ? '编辑项目' : '新建项目'"
    :bordered="false"
    :mask-closable="false"
    :close-on-esc="!busy && !pendingMerge"
    :closable="false"
  >
    <template #header-extra>
      <button
        type="button"
        class="icon-button"
        aria-label="关闭弹窗"
        :disabled="busy"
        @click="ui.projectDialog = false"
      >
        <AppIcon name="close" :size="20" />
      </button>
    </template>
    <form class="dialog-form" @submit.prevent="save">
      <label for="project-name">项目名称</label
      ><NInput
        :input-props="{ id: 'project-name' }"
        v-model:value="name"
        placeholder="例如：农田管理"
        maxlength="100"
        show-count
        autofocus
      /><label for="project-description">项目描述 <small>可选</small></label
      ><NInput
        :input-props="{ id: 'project-description' }"
        v-model:value="description"
        type="textarea"
        :autosize="{ minRows: 3, maxRows: 5 }"
        maxlength="2000"
        placeholder="输入项目描述"
      />
      <template v-if="ui.editingProject">
        <label>项目状态</label>
        <NSelect
          v-model:value="status"
          aria-label="项目状态"
          :options="[
            { label: '进行中', value: 'active' },
            { label: '已完成', value: 'completed' },
            { label: '已归档', value: 'archived' },
          ]"
        >
          <template #arrow><AppIcon name="down" :size="16" /></template>
        </NSelect>
        <template v-if="status !== 'active' && status !== ui.editingProject.status">
          <label>未完成待办处理</label>
          <NSelect
            v-model:value="pendingPolicy"
            aria-label="未完成待办处理"
            :options="[
              { label: '设为无项目（复制附件，保留原文件）', value: 'inbox' },
              { label: '全部标记已完成', value: 'complete' },
              { label: '全部取消', value: 'cancel' },
            ]"
          >
            <template #arrow><AppIcon name="down" :size="16" /></template>
          </NSelect>
          <small class="muted">保存后取消该项目未发送的提醒，项目文件保留。</small>
        </template>
      </template>
      <label>项目颜色</label>
      <div class="color-options">
        <IconHint v-for="option in projectColors" :key="option" text="选择颜色"
          ><button
            type="button"
            :style="{ background: option }"
            :class="{ selected: color.toUpperCase() === option.toUpperCase() }"
            :aria-label="`选择颜色 ${option}`"
            :aria-pressed="color.toUpperCase() === option.toUpperCase()"
            @click="color = option"
        /></IconHint>
        <AppColorPicker v-model="color" label="自定义项目颜色" :disabled="busy" />
      </div>
      <ProjectTagsEditor v-model="tags" :disabled="busy" />
      <p class="folder-hint">
        {{ workspace.desktop ? '项目目录' : '预览目录' }}：<code
          >{{ workspace.status.rootPath || '请先设置存储位置' }}/{{
            name.trim() || '项目名称'
          }}</code
        >
      </p>
      <p v-if="error" class="form-error" role="alert">{{ error }}</p>
      <div v-if="!workspace.status.rootPath" class="form-error">
        请先前往设置，选择项目存储位置。<RouterLink to="/settings" @click="ui.projectDialog = false"
          >前往设置 <AppIcon name="arrow" :size="14"
        /></RouterLink>
      </div>
      <div class="dialog-footer">
        <NButton :disabled="busy" @click="ui.projectDialog = false">取消</NButton
        ><NButton
          type="primary"
          attr-type="submit"
          :loading="busy"
          :disabled="!workspace.status.available"
          >{{ ui.editingProject ? '保存项目' : '创建项目' }}</NButton
        >
      </div>
    </form></NModal
  >
  <NModal
    :show="!!pendingMerge"
    preset="card"
    title="同名文件夹已存在"
    aria-label="合并已有文件夹"
    class="form-modal"
    :bordered="false"
    :closable="false"
    :mask-closable="false"
    :close-on-esc="!busy"
    @update:show="
      (show) => {
        if (!show && !busy) pendingMerge = null
      }
    "
  >
    <template #header-extra>
      <button
        type="button"
        class="icon-button"
        aria-label="关闭合并确认"
        :disabled="busy"
        @click="pendingMerge = null"
      >
        <AppIcon name="close" :size="20" />
      </button>
    </template>
    <template v-if="pendingMerge">
      <p class="project-merge-description">
        合并后沿用此文件夹和当前项目配置，保留原有文件及子文件夹，进入文件页时自动读取。
      </p>
      <p class="folder-hint project-merge-path">
        <code>{{ pendingMerge.directory.path }}</code>
      </p>
      <p
        v-if="pendingMerge.input.name.trim() !== pendingMerge.directory.name"
        class="muted project-merge-name"
      >
        项目名称使用文件夹名称：{{ pendingMerge.directory.name }}
      </p>
      <div class="dialog-footer">
        <NButton :disabled="busy" @click="pendingMerge = null">返回修改</NButton>
        <NButton type="primary" :loading="busy" @click="confirmMerge">合并并创建</NButton>
      </div>
    </template>
  </NModal>
</template>
