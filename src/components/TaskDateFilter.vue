<script setup lang="ts">
import { computed } from 'vue'
import { NDatePicker, NSelect } from 'naive-ui'
import AppIcon from './AppIcon.vue'
import {
  taskDateShortcuts,
  taskDateFieldLabels,
  type TaskDateField,
  type TaskFilters,
} from '../shared/task-filters'

const props = defineProps<{ today: string }>()
const filters = defineModel<TaskFilters>({ required: true })
const shortcuts = computed(() => taskDateShortcuts(props.today))
const fieldOptions = Object.entries(taskDateFieldLabels).map(([value, label]) => ({ value, label }))
function setField(value: TaskDateField) {
  filters.value.dateField = value
  if (value === 'completed') filters.value.status = 'done'
}
function clearDate() {
  filters.value.date = 'all'
  filters.value.range = null
}
function setRange(value: string | [string, string] | null) {
  if (!Array.isArray(value)) {
    clearDate()
    return
  }
  filters.value.range = value[0] <= value[1] ? [...value] : [value[1], value[0]]
  filters.value.date = 'range'
}
</script>

<template>
  <div class="task-date-filters" role="group" aria-label="日期筛选">
    <div class="task-date-range">
      <NSelect
        :value="filters.dateField"
        :options="fieldOptions"
        class="task-date-field"
        :input-props="{ 'aria-label': '筛选日期类型' }"
        @update:value="setField"
      >
        <template #arrow><AppIcon name="down" :size="16" /></template>
      </NSelect>
      <NDatePicker
        :formatted-value="filters.date === 'range' ? filters.range : null"
        type="daterange"
        format="yyyy/MM/dd"
        value-format="yyyy-MM-dd"
        start-placeholder="开始日期"
        end-placeholder="结束日期"
        separator="—"
        :shortcuts="shortcuts"
        :actions="null"
        :first-day-of-week="1"
        :close-on-select="true"
        :update-value-on-close="false"
        clearable
        class="task-date-picker"
        aria-label="待办日期范围"
        @update:formatted-value="setRange"
      >
        <template #date-icon><AppIcon name="calendar" :size="17" /></template>
        <template #prev-month><AppIcon name="back" :size="16" /></template>
        <template #next-month><AppIcon name="arrow" :size="16" /></template>
        <template #prev-year><AppIcon name="back" :size="16" /></template>
        <template #next-year><AppIcon name="arrow" :size="16" /></template>
      </NDatePicker>
    </div>
  </div>
</template>

<style scoped>
.task-date-filters,
.task-date-range {
  display: flex;
  align-items: center;
  gap: 8px;
}
.task-date-filters {
  flex-wrap: wrap;
  max-width: 100%;
}
.task-date-range {
  max-width: 100%;
  min-width: 0;
}
.task-date-field {
  width: 140px;
  flex-shrink: 0;
}
.task-date-picker {
  width: 270px;
  min-width: 0;
}
.task-date-picker :deep(.n-date-picker-icon) {
  color: var(--muted);
}
@media (max-width: 700px) {
  .task-date-range {
    flex-wrap: wrap;
    width: 100%;
  }
  .task-date-field {
    width: 100%;
  }
  .task-date-picker {
    width: 100%;
  }
}
</style>
