<script setup lang="ts">
import tasks from '../assets/empty-states/overview-tasks.png'
import projects from '../assets/empty-states/overview-projects.png'
import activity from '../assets/empty-states/overview-trend.png'
import files from '../assets/empty-states/files.png'
import reminders from '../assets/empty-states/reminders.png'
import search from '../assets/empty-states/search.png'
import links from '../assets/empty-states/links.png'

const illustrations = { tasks, projects, activity, files, reminders, search, links }
withDefaults(
  defineProps<{
    title: string
    kind?: keyof typeof illustrations
    actionLabel?: string
    actionTo?: string
    disabled?: boolean
    compact?: boolean
  }>(),
  { kind: 'tasks' },
)
defineEmits<{ action: [] }>()
</script>

<template>
  <div class="empty-state" :class="{ 'is-compact': compact }">
    <img
      :src="illustrations[kind]"
      class="empty-illustration"
      alt=""
      aria-hidden="true"
      :draggable="false"
      width="144"
      height="144"
      decoding="async"
    />
    <div class="empty-copy">
      <h3>{{ title }}</h3>
      <template v-if="actionLabel">
        <span class="empty-separator">，</span>
        <RouterLink v-if="actionTo && !disabled" :to="actionTo" class="empty-action">{{
          actionLabel
        }}</RouterLink>
        <button
          v-else
          type="button"
          class="empty-action"
          :disabled="disabled"
          @click="$emit('action')"
        >
          {{ actionLabel }}
        </button>
      </template>
    </div>
  </div>
</template>

<style scoped>
.empty-illustration {
  display: block;
  flex-shrink: 0;
  width: var(--empty-illustration-size, 144px);
  height: var(--empty-illustration-size, 144px);
  max-width: 100%;
  margin: 0 auto 12px;
  object-fit: contain;
  pointer-events: none;
  user-select: none;
}
.empty-state.is-compact {
  --empty-illustration-size: 96px;
  padding: 16px 12px;
}
.empty-copy {
  display: flex;
  align-items: baseline;
  justify-content: center;
  max-width: 100%;
  min-width: 0;
  color: var(--muted);
  font-size: 12px;
  line-height: 1.8;
}
.empty-copy h3 {
  display: inline;
  min-width: 0;
  overflow: hidden;
  color: inherit;
  font: inherit;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.empty-separator {
  flex-shrink: 0;
}
.empty-action {
  display: inline;
  flex-shrink: 0;
  padding: 0;
  border: 0;
  border-radius: 0;
  background: transparent;
  color: #1677ff;
  font: inherit;
  line-height: inherit;
  text-decoration: none;
  white-space: nowrap;
  pointer-events: auto;
}
.dark .empty-action {
  color: #69b1ff;
}
.empty-action:hover:not(:disabled) {
  text-decoration: underline;
  text-underline-offset: 3px;
}
.empty-action:focus-visible {
  outline: 2px solid currentColor;
  outline-offset: 3px;
}
.empty-action:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}
</style>
