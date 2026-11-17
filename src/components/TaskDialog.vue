<script setup lang="ts">
import DateField from './DateField.vue'
import TaskAttachments from './TaskAttachments.vue'
import AppIcon from './AppIcon.vue'
import { useAppMessage } from '../composables/useAppMessage'
import { computed, ref, watch } from 'vue'
import { NButton, NModal, NInput, NSelect } from 'naive-ui'
import { useUiStore } from '../stores/ui'
import { useWorkspaceStore } from '../stores/workspace'
import { errorMessage } from '../shared/domain'
import { priorityLabels, statusLabels } from '../shared/types'
import type { Priority, TaskStatus } from '../shared/types'
const ui = useUiStore()
const workspace = useWorkspaceStore()
const message = useAppMessage()
const title = ref('')
const description = ref('')
const dueAt = ref('')
const reminderAt = ref('')
const taskStatus = ref<TaskStatus>('todo')
const savedId = ref<string | null>(null)
const reminderTouched = ref(false)
function localTime(value?: string | null) {
  if (!value) return ''
  const date = new Date(value)
  if (!Number.isFinite(date.getTime())) return ''
  return new Date(date.getTime() - date.getTimezoneOffset() * 60000).toISOString().slice(0, 16)
}
const projectId = ref('')
const priority = ref<Priority>('none')
const planDate = ref('')
const busy = ref(false)
const error = ref('')
const advanced = ref(false)
const projectOptions = computed(() => [
  { label: '无', value: '' },
  ...workspace.projects
    .filter((p) => p.status === 'active' || p.id === ui.editingTask?.projectId)
    .map((project) => ({ label: project.name, value: project.id })),
])
const priorityOptions = Object.entries(priorityLabels).map(([value, label]) => ({ value, label }))
watch(
  () => [ui.taskDialog, ui.editingTask?.id] as const,
  ([open]) => {
    if (open) {
      const task = ui.editingTask
      advanced.value = !!task
      savedId.value = task?.id || null
      title.value = task?.title || ui.captureText
      description.value = task?.description || ''
      dueAt.value = localTime(task?.dueAt)
      taskStatus.value = task?.status || 'todo'
      reminderAt.value = ''
      reminderTouched.value = false
      projectId.value = task ? task.projectId || '' : ui.defaultProjectId || ''
      priority.value = task?.priority || 'none'
      planDate.value = task?.planDate || ''
      error.value = ''
    }
  },
)
async function save() {
  if (busy.value) return
  if (!title.value.trim() || [...title.value.trim()].length > 200) {
    error.value = '请输入 1–200 字的待办标题'
    return
  }
  busy.value = true
  error.value = ''
  try {
    const input = {
      title: title.value,
      projectId: projectId.value || null,
      priority: priority.value,
      planDate: planDate.value || null,
      description: description.value,
      dueAt: dueAt.value ? new Date(dueAt.value).toISOString() : null,
      status: taskStatus.value,
      reminderAt:
        reminderTouched.value && reminderAt.value ? new Date(reminderAt.value).toISOString() : null,
    }
    if (savedId.value) await workspace.updateTask(savedId.value, input)
    else await workspace.createFullTask(input)
    ui.taskDialog = false
    message.success('待办已保存')
  } catch (cause) {
    error.value = errorMessage(cause)
  } finally {
    busy.value = false
  }
}
</script>
<template>
  <NModal
    v-model:show="ui.taskDialog"
    :title="ui.editingTask ? '编辑待办' : '新建待办'"
    preset="card"
    class="form-modal task-modal"
    :aria-label="ui.editingTask ? '编辑待办' : '新建待办'"
    :bordered="false"
    :mask-closable="false"
    :close-on-esc="!busy"
    :closable="false"
  >
    <template #header-extra>
      <button
        type="button"
        class="icon-button"
        aria-label="关闭弹窗"
        :disabled="busy"
        @click="ui.taskDialog = false"
      >
        <AppIcon name="close" :size="20" />
      </button>
    </template>
    <form class="dialog-form" @submit.prevent="save">
      <label for="task-title">待办内容</label
      ><NInput
        :input-props="{ id: 'task-title' }"
        v-model:value="title"
        placeholder="输入待办内容"
        maxlength="200"
        show-count
        autofocus
      /><label for="task-project">所属项目</label
      ><NSelect
        v-model:value="projectId"
        :options="projectOptions"
        :input-props="{ id: 'task-project', 'aria-label': '所属项目' }"
        aria-label="所属项目"
        ><template #arrow><AppIcon name="down" :size="16" /></template
      ></NSelect>
      <DateField v-model="planDate" label="计划日期" />
      <button
        type="button"
        class="advanced-toggle"
        :aria-expanded="advanced"
        @click="advanced = !advanced"
      >
        更多设置<AppIcon
          name="down"
          :size="16"
          :style="{ transform: advanced ? 'rotate(180deg)' : undefined }"
        />
      </button>
      <div v-show="advanced" class="advanced-fields">
        <label for="task-priority">优先级</label>
        <NSelect v-model:value="priority" :options="priorityOptions" aria-label="优先级">
          <template #arrow><AppIcon name="down" :size="16" /></template>
        </NSelect>
        <label for="task-description">描述</label
        ><NInput
          v-model:value="description"
          type="textarea"
          :input-props="{ id: 'task-description' }"
          maxlength="10000"
          :autosize="{ minRows: 2, maxRows: 5 }"
          placeholder="输入描述"
        />
        <div class="form-columns">
          <div>
            <label for="task-status">状态</label
            ><NSelect
              v-model:value="taskStatus"
              :options="Object.entries(statusLabels).map(([value, label]) => ({ value, label }))"
              :input-props="{ id: 'task-status', 'aria-label': '状态' }"
              aria-label="状态"
              ><template #arrow><AppIcon name="down" :size="16" /></template
            ></NSelect>
          </div>
          <div>
            <DateField v-model="dueAt" label="截止时间" datetime />
          </div>
        </div>
        <DateField
          v-model="reminderAt"
          label="提醒时间"
          datetime
          @update:model-value="reminderTouched = true"
        />
        <small v-if="ui.editingTask" class="muted">已有提醒在提醒中心管理；此处可新增提醒。</small>
        <TaskAttachments
          v-if="ui.editingTask && ui.editingTask.projectId === (projectId || null)"
          :key="ui.editingTask.id"
          :task-id="ui.editingTask.id"
          :project-id="ui.editingTask.projectId"
        />
        <p v-else-if="ui.editingTask" class="muted">保存后，附件将复制到目标项目；原文件保留。</p>
      </div>
      <p v-if="error" class="form-error" role="alert">{{ error }}</p>
      <div class="dialog-footer task-dialog-footer">
        <NButton :disabled="busy" @click="ui.taskDialog = false">取消</NButton
        ><NButton type="primary" attr-type="submit" :loading="busy">保存待办</NButton>
      </div>
    </form></NModal
  >
</template>
