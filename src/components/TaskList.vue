<script setup lang="ts">
import IconHint from './IconHint.vue'
import { h, ref } from 'vue'
import { NButton, NDropdown, NPopconfirm } from 'naive-ui'
import type { Task } from '../shared/types'
import { priorityLabels, statusLabels } from '../shared/types'
import { errorMessage } from '../shared/domain'
import { useWorkspaceStore } from '../stores/workspace'
import { useUiStore } from '../stores/ui'
import { useAppMessage } from '../composables/useAppMessage'
import AppIcon from './AppIcon.vue'

defineProps<{ tasks: Task[] }>()
const ui = useUiStore()
const workspace = useWorkspaceStore()
const message = useAppMessage()
const pendingIds = ref(new Set<string>())
async function toggle(task: Task) {
  if (pendingIds.value.has(task.id)) return
  pendingIds.value.add(task.id)
  try {
    await workspace.toggleTask(task)
  } catch (cause) {
    message.error(errorMessage(cause))
  } finally {
    pendingIds.value.delete(task.id)
  }
}
async function action(task: Task, key: string) {
  if (key === 'edit') return ui.editTask(task)
  if (pendingIds.value.has(task.id)) return
  pendingIds.value.add(task.id)
  try {
    await workspace.trashTask(task.id)
    let restoring = false
    const toast = message.success(
      () =>
        h('span', { class: 'undo-message' }, [
          '已移入回收站',
          h(
            'button',
            {
              class: 'text-link',
              onClick: async () => {
                if (restoring) return
                restoring = true
                try {
                  const inactive =
                    !!task.projectId &&
                    workspace.projects.find((p) => p.id === task.projectId)?.status !== 'active'
                  await workspace.trashTask(task.id, true, inactive)
                  toast.destroy()
                } catch (e) {
                  message.error(errorMessage(e))
                  restoring = false
                }
              },
            },
            '撤销',
          ),
        ]),
      { duration: 6000 },
    )
  } catch (cause) {
    message.error(errorMessage(cause))
  } finally {
    pendingIds.value.delete(task.id)
  }
}
function projectName(task: Task) {
  return workspace.projects.find((p) => p.id === task.projectId)?.name || '无'
}
function dueLabel(task: Task) {
  if (!task.dueAt) return ''
  return new Date(task.dueAt).toLocaleString('zh-CN', {
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
}
function overdue(task: Task) {
  return (
    !!task.dueAt &&
    new Date(task.dueAt).getTime() < workspace.now &&
    ['todo', 'doing'].includes(task.status)
  )
}
</script>
<template>
  <ul class="task-list">
    <li
      v-for="task in tasks"
      :key="task.id"
      class="task-row"
      :class="{ 'is-pending': pendingIds.has(task.id) }"
    >
      <IconHint :text="task.status === 'done' ? '重新打开' : '完成'" v-if="!task.deletedAt"
        ><button
          class="task-checkbox"
          :class="{ checked: task.status === 'done' }"
          :disabled="pendingIds.has(task.id) || task.status === 'canceled'"
          :aria-label="task.status === 'done' ? `重新打开 ${task.title}` : `完成 ${task.title}`"
          @click="toggle(task)"
        >
          <AppIcon v-if="task.status === 'done'" name="check" :size="13" /></button
      ></IconHint>
      <div class="task-text">
        <button
          class="text-button task-title"
          :class="{ 'task-done': task.status === 'done' }"
          :disabled="!!task.deletedAt"
          :aria-label="`编辑 ${task.title}`"
          :title="task.title"
          @click="ui.editTask(task)"
        >
          {{ task.title }}
        </button>
        <div class="task-metadata">
          <span class="task-project-label" :title="projectName(task)">{{ projectName(task) }}</span>
          <span v-if="task.planDate" class="task-date"
            ><AppIcon name="calendar" :size="13" />{{ task.planDate }}</span
          >
          <span v-if="task.dueAt" class="task-date" :class="{ 'is-overdue': overdue(task) }"
            ><AppIcon name="clock" :size="13" />{{ overdue(task) ? '已逾期' : '截止' }}
            {{ dueLabel(task) }}</span
          >
        </div>
      </div>
      <div class="task-indicators">
        <span v-if="task.status === 'doing' || task.status === 'canceled'" class="tag">{{
          statusLabels[task.status]
        }}</span>
        <span v-if="task.priority !== 'none'" :class="['priority-tag', task.priority]"
          >{{ priorityLabels[task.priority] }}优先级</span
        >
      </div>
      <NPopconfirm
        v-if="
          task.deletedAt &&
          task.projectId &&
          workspace.projects.find((p) => p.id === task.projectId)?.status !== 'active' &&
          ['todo', 'doing'].includes(task.status)
        "
        :show-icon="false"
        @positive-click="
          workspace.trashTask(task.id, true, true).catch((e) => message.error(errorMessage(e)))
        "
      >
        <template #trigger><NButton size="small">恢复</NButton></template
        >原项目已归档，恢复为无项目？附件将复制，原文件保留。
      </NPopconfirm>
      <NButton
        v-else-if="task.deletedAt"
        size="small"
        @click="workspace.trashTask(task.id, true).catch((e) => message.error(errorMessage(e)))"
        >恢复</NButton
      >
      <NDropdown
        v-else
        trigger="click"
        :options="[
          { label: '编辑待办', key: 'edit' },
          { label: '移入回收站', key: 'delete' },
        ]"
        @select="(key) => action(task, key)"
      >
        <span class="icon-hint-anchor"
          ><IconHint text="更多操作"
            ><button
              class="icon-button task-more"
              :aria-label="`更多操作 ${task.title}`"
              :disabled="pendingIds.has(task.id)"
            >
              <AppIcon name="more" :size="20" /></button></IconHint
        ></span>
      </NDropdown>
    </li>
  </ul>
</template>
