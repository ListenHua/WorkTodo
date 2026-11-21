<script setup lang="ts">
import { ref, watch } from 'vue'
import { NModal } from 'naive-ui'
import { desktopApi } from '../services/desktop'
import TaskForm from './TaskForm.vue'
import TaskAttachments from './TaskAttachments.vue'
import AppIcon from './AppIcon.vue'
import { useAppMessage } from '../composables/useAppMessage'
import { useUiStore } from '../stores/ui'
import { useWorkspaceStore } from '../stores/workspace'
import { errorMessage } from '../shared/domain'
import { taskFormInput, taskFormValue } from '../shared/task-form'
const ui = useUiStore()
const workspace = useWorkspaceStore()
const message = useAppMessage()
const form = ref(taskFormValue())
const busy = ref(false)
const error = ref('')
watch(
  () => [ui.taskDialog, ui.editingTask?.id] as const,
  ([open]) => {
    if (!open) return
    form.value = taskFormValue(ui.editingTask, ui.defaultProjectId)
    if (!ui.editingTask) form.value.title = ui.captureText
    error.value = ''
  },
)
async function save() {
  if (busy.value) return
  busy.value = true
  error.value = ''
  try {
    const input = taskFormInput(form.value)
    if (ui.editingTask) await workspace.updateTask(ui.editingTask.id, input)
    else await workspace.createFullTask(input)
    ui.taskDialog = false
    message.success('待办已保存')
    if (input.reminderAt && workspace.desktop) {
      try {
        const permission = await desktopApi.notificationRequest()
        if (permission !== 'granted') message.info('待办已保存，请在提醒中心开启系统通知')
      } catch (cause) {
        message.error(errorMessage(cause))
      }
    }
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
    <TaskForm
      v-model="form"
      :busy="busy"
      :editing="!!ui.editingTask"
      :original-project-id="ui.editingTask?.projectId"
      :error="error"
      @submit="save"
      @cancel="ui.taskDialog = false"
    >
      <template #advanced>
        <TaskAttachments
          v-if="ui.editingTask && form.projectId === (ui.editingTask.projectId || '')"
          :key="ui.editingTask.id"
          :task-id="ui.editingTask.id"
          :project-id="ui.editingTask.projectId"
        />
        <p v-else-if="ui.editingTask" class="muted">附件将复制到目标项目，原文件保留。</p>
      </template>
    </TaskForm>
  </NModal>
</template>
