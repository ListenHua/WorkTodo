<script setup lang="ts">
import IconHint from '../components/IconHint.vue'
import AppIcon from '../components/AppIcon.vue'
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { NButton, NSelect } from 'naive-ui'
import { reminders } from '../services/desktop'
import type { Reminder } from '../services/desktop'
import { useWorkspaceStore } from '../stores/workspace'
import { useUiStore } from '../stores/ui'
import { useAppMessage } from '../composables/useAppMessage'
import { errorMessage } from '../shared/domain'
import { desktop } from '../services/api'
import { listen } from '@tauri-apps/api/event'
import EmptyState from '../components/EmptyState.vue'
const workspace = useWorkspaceStore()
const ui = useUiStore()
const message = useAppMessage()
const items = ref<Reminder[]>([])
const filter = ref('all')
const visibleItems = computed(() => {
  const query = ui.search.trim().toLocaleLowerCase()
  return items.value.filter(
    (item) =>
      (filter.value === 'all' || item.state === filter.value) &&
      item.title.toLocaleLowerCase().includes(query),
  )
})
let unlisten: (() => void) | undefined
let gone = false
async function load() {
  try {
    items.value = await reminders.list()
  } catch (e) {
    message.error(errorMessage(e))
  }
}
async function action(id: string, action: 'read' | 'dismiss' | 'snooze') {
  try {
    await reminders.action(id, action)
    await load()
  } catch (e) {
    message.error(errorMessage(e))
  }
}
async function open(item: Reminder) {
  await workspace.refresh()
  const task = workspace.tasks.find((t) => t.id === item.taskId)
  if (task) ui.editTask(task)
  await action(item.id, 'read')
}
onMounted(async () => {
  await load()
  if (desktop) {
    const off = await listen('reminders-changed', load)
    if (gone) off()
    else unlisten = off
  }
})
onUnmounted(() => {
  gone = true
  unlisten?.()
})
</script>
<template>
  <section class="panel">
    <div class="list-toolbar">
      <IconHint text="刷新提醒"
        ><NButton text aria-label="刷新提醒" @click="load"
          ><AppIcon name="refresh" :size="18" /></NButton
      ></IconHint>
      <NSelect
        v-model:value="filter"
        :options="[
          { label: '全部提醒', value: 'all' },
          { label: '待提醒', value: 'pending' },
          { label: '已发送', value: 'sent' },
          { label: '已取消', value: 'dismissed' },
        ]"
        aria-label="提醒筛选"
        style="width: 180px"
        ><template #arrow><AppIcon name="down" :size="16" /></template
      ></NSelect>
    </div>
    <p v-if="!workspace.desktop" class="muted">预览仅展示提醒记录；系统通知在桌面端运行。</p>
    <div v-for="item in visibleItems" :key="item.id" class="reminder-row">
      <div class="reminder-info">
        <button class="text-button" @click="open(item)">{{ item.title }}</button>
        <p class="muted">
          {{ new Date(item.remindAt).toLocaleString('zh-CN') }} ·
          {{
            item.state === 'pending'
              ? '待提醒'
              : item.state === 'sent'
                ? '已提交系统通知'
                : '已取消'
          }}{{ item.readAt ? ' · 已读' : '' }}
        </p>
      </div>
      <NButton v-if="item.state === 'sent'" size="small" @click="action(item.id, 'snooze')"
        >延后 10 分钟</NButton
      ><NButton v-if="item.state === 'pending'" size="small" @click="action(item.id, 'dismiss')"
        >取消提醒</NButton
      ><NButton v-if="!item.readAt" text @click="action(item.id, 'read')">标记已读</NButton>
    </div>
    <EmptyState v-if="!visibleItems.length" :title="items.length ? '没有匹配的提醒' : '暂无提醒'">
      <NButton v-if="ui.search.trim()" @click="ui.search = ''">清除搜索</NButton>
    </EmptyState>
  </section>
</template>
