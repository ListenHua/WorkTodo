<script setup lang="ts">
import { NButton, NDropdown, NPopconfirm } from 'naive-ui'
import type { Task } from '../shared/types'
import { statusLabels } from '../shared/types'
import AppIcon from './AppIcon.vue'
import IconHint from './IconHint.vue'

defineProps<{
  task: Task
  pending: boolean
  projectName: string
  dueLabel: string
  overdue: boolean
  needsRestoreConfirmation: boolean
}>()
const emit = defineEmits<{
  toggle: []
  action: [key: string]
  restore: [toInbox?: boolean]
}>()
</script>

<template>
  <li class="task-row" :class="{ 'is-pending': pending }">
    <IconHint v-if="!task.deletedAt" :text="task.status === 'done' ? '重新打开' : '完成'">
      <button
        class="task-checkbox"
        :class="{ checked: task.status === 'done' }"
        :disabled="pending || task.status === 'canceled'"
        :aria-label="task.status === 'done' ? `重新打开 ${task.title}` : `完成 ${task.title}`"
        @click="emit('toggle')"
      >
        <AppIcon v-if="task.status === 'done'" name="check" :size="13" />
      </button>
    </IconHint>
    <div class="task-text">
      <button
        class="text-button task-title"
        :class="{ 'task-done': task.status === 'done' }"
        :disabled="!!task.deletedAt"
        :aria-label="`编辑 ${task.title}`"
        :title="task.title"
        @click="emit('action', 'edit')"
      >
        {{ task.title }}
      </button>
      <div class="task-metadata">
        <span class="task-project-label" :title="projectName">{{ projectName }}</span>
        <span v-if="task.planDate" class="task-date">
          <AppIcon name="calendar" :size="13" />{{ task.planDate }}
        </span>
        <span v-if="task.dueAt" class="task-date" :class="{ 'is-overdue': overdue }">
          <AppIcon name="clock" :size="13" />{{ overdue ? '已逾期' : '截止' }} {{ dueLabel }}
        </span>
      </div>
    </div>
    <div class="task-indicators">
      <span v-if="task.status === 'doing' || task.status === 'canceled'" class="tag">
        {{ statusLabels[task.status] }}
      </span>
    </div>
    <NPopconfirm
      v-if="task.deletedAt && needsRestoreConfirmation"
      :show-icon="false"
      @positive-click="emit('restore', true)"
    >
      <template #trigger><NButton size="small">恢复</NButton></template>
      原项目已归档，恢复为无项目？附件将复制，原文件保留。
    </NPopconfirm>
    <NButton v-else-if="task.deletedAt" size="small" @click="emit('restore')">恢复</NButton>
    <NDropdown
      v-else
      trigger="click"
      :options="[
        { label: '编辑待办', key: 'edit' },
        { label: '删除待办', key: 'delete' },
      ]"
      @select="(key) => emit('action', key)"
    >
      <button
        class="icon-button task-more"
        :aria-label="`更多操作 ${task.title}`"
        :disabled="pending"
      >
        <AppIcon name="more" :size="20" />
      </button>
    </NDropdown>
  </li>
</template>
