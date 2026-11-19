<script setup lang="ts">
import IconHint from './IconHint.vue'
import { computed } from 'vue'
import { NDatePicker } from 'naive-ui'
import AppIcon from './AppIcon.vue'
const props = withDefaults(
  defineProps<{ label: string; datetime?: boolean; disabled?: boolean }>(),
  {
    datetime: false,
    disabled: false,
  },
)
const model = defineModel<string>({ default: '' })
const value = computed({
  get: () => model.value || null,
  set: (next: string | null) => (model.value = next || ''),
})
function day(offset: number) {
  const date = new Date()
  date.setDate(date.getDate() + offset)
  date.setHours(props.datetime ? 18 : 0, 0, 0, 0)
  return date.getTime()
}
const shortcuts = { 今天: () => day(0), 明天: () => day(1), 一周后: () => day(7) }
</script>
<template>
  <label class="date-field">
    <span class="field-label">{{ label }}</span>
    <NDatePicker
      v-model:formatted-value="value"
      :disabled="disabled"
      :type="datetime ? 'datetime' : 'date'"
      :format="datetime ? 'yyyy-MM-dd HH:mm' : 'yyyy-MM-dd'"
      :value-format="datetime ? `yyyy-MM-dd'T'HH:mm` : 'yyyy-MM-dd'"
      :time-picker-format="'HH:mm'"
      :shortcuts="shortcuts"
      clearable
      :placeholder="datetime ? '选择日期和时间' : '选择日期'"
    >
      <template #date-icon><AppIcon name="calendar" :size="17" /></template>
      <template #prev-month
        ><IconHint text="上个月"
          ><span
            ><AppIcon name="down" :size="16" style="transform: rotate(90deg)" /></span></IconHint
      ></template>
      <template #next-month
        ><IconHint text="下个月"
          ><span
            ><AppIcon name="down" :size="16" style="transform: rotate(-90deg)" /></span></IconHint
      ></template>
      <template #prev-year
        ><IconHint text="上一年"
          ><span><AppIcon name="back" :size="16" /></span></IconHint
      ></template>
      <template #next-year
        ><IconHint text="下一年"
          ><span><AppIcon name="arrow" :size="16" /></span></IconHint
      ></template>
    </NDatePicker>
  </label>
</template>
