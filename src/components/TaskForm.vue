<script setup lang="ts">
import { computed, ref } from 'vue'
import { NButton, NInput, NRadio, NRadioGroup, NSelect, NSpace } from 'naive-ui'
import AppIcon from './AppIcon.vue'
import DateField from './DateField.vue'
import { useWorkspaceStore } from '../stores/workspace'
import { priorityLabels, statusLabels } from '../shared/types'
import type { TaskFormValue } from '../shared/task-form'

const props = withDefaults(
  defineProps<{
    busy?: boolean
    disabled?: boolean
    editing?: boolean
    originalProjectId?: string | null
    error?: string
  }>(),
  { busy: false, disabled: false, editing: false, error: '' },
)
const form = defineModel<TaskFormValue>({ required: true })
defineEmits<{ submit: []; cancel: [] }>()
const workspace = useWorkspaceStore()
const advanced = ref(props.editing)
const projectOptions = computed(() => [
  { label: '无', value: '' },
  ...workspace.projects
    .filter((project) => project.status === 'active' || project.id === props.originalProjectId)
    .map((project) => ({ label: project.name, value: project.id })),
])
</script>
<template>
  <form class="dialog-form" @submit.prevent="$emit('submit')">
    <label for="task-title">待办内容</label>
    <NInput
      v-model:value="form.title"
      :input-props="{ id: 'task-title' }"
      :disabled="busy || disabled"
      placeholder="输入待办内容"
      maxlength="200"
      show-count
      autofocus
    />
    <label for="task-project">所属项目</label>
    <NSelect
      v-model:value="form.projectId"
      :options="projectOptions"
      :disabled="busy || disabled"
      :input-props="{ id: 'task-project', 'aria-label': '所属项目' }"
      aria-label="所属项目"
    >
      <template #arrow><AppIcon name="down" :size="16" /></template>
    </NSelect>
    <label for="task-description">描述</label>
    <NInput
      v-model:value="form.description"
      type="textarea"
      :disabled="busy || disabled"
      :input-props="{ id: 'task-description' }"
      maxlength="10000"
      :autosize="{ minRows: 2, maxRows: 5 }"
      placeholder="输入描述"
    />
    <DateField v-model="form.planDate" label="计划日期" :disabled="busy || disabled" />
    <button
      type="button"
      class="advanced-toggle"
      :aria-expanded="advanced"
      :disabled="busy || disabled"
      @click="advanced = !advanced"
    >
      更多设置<AppIcon
        name="down"
        :size="16"
        :style="{ transform: advanced ? 'rotate(180deg)' : undefined }"
      />
    </button>
    <div v-show="advanced" class="advanced-fields">
      <span id="task-priority-label" class="field-label">优先级</span>
      <NRadioGroup
        v-model:value="form.priority"
        name="task-priority"
        role="radiogroup"
        aria-labelledby="task-priority-label"
        :disabled="busy || disabled"
      >
        <NSpace
          ><NRadio v-for="(label, value) in priorityLabels" :key="value" :value="value">{{
            label
          }}</NRadio></NSpace
        >
      </NRadioGroup>
      <span id="task-status-label" class="field-label">状态</span>
      <NRadioGroup
        v-model:value="form.status"
        name="task-status"
        role="radiogroup"
        aria-labelledby="task-status-label"
        :disabled="busy || disabled"
      >
        <NSpace
          ><NRadio v-for="(label, value) in statusLabels" :key="value" :value="value">{{
            label
          }}</NRadio></NSpace
        >
      </NRadioGroup>
      <DateField v-model="form.dueAt" label="截止时间" datetime :disabled="busy || disabled" />
      <DateField v-model="form.reminderAt" label="提醒时间" datetime :disabled="busy || disabled" />
      <small v-if="editing" class="muted">已有提醒在提醒中心管理；此处可新增提醒。</small>
      <slot name="advanced" />
    </div>
    <slot />
    <p v-if="error" class="form-error" role="alert">{{ error }}</p>
    <div class="dialog-footer task-dialog-footer">
      <NButton attr-type="button" :disabled="busy" @click="$emit('cancel')">取消</NButton>
      <NButton type="primary" attr-type="submit" :disabled="disabled" :loading="busy"
        >保存待办</NButton
      >
    </div>
  </form>
</template>

<style scoped>
.advanced-fields > .field-label {
  margin-top: 17px;
}
.dialog-form :deep(.n-radio) {
  display: inline-flex;
  margin: 0;
  align-items: center;
}
</style>
