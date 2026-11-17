<script setup lang="ts">
import { ref, watch } from 'vue'
import { NColorPicker } from 'naive-ui'
import IconHint from './IconHint.vue'
import { projectColors } from '../shared/types'

const color = defineModel<string>({ required: true })
withDefaults(defineProps<{ label?: string; disabled?: boolean }>(), { label: '选择颜色' })
const show = ref(false)
const pickerValue = ref(color.value)
let triggerElement: HTMLButtonElement | null = null
function togglePicker(event: MouseEvent, toggle: (() => void) | undefined) {
  triggerElement = event.currentTarget as HTMLButtonElement
  toggle?.()
}
watch(show, (visible, _, onCleanup) => {
  if (!visible) return
  const escape = (event: KeyboardEvent) => {
    if (event.key !== 'Escape') return
    event.preventDefault()
    event.stopImmediatePropagation()
    show.value = false
    triggerElement?.focus({ preventScroll: true })
  }
  document.addEventListener('keydown', escape, true)
  onCleanup(() => document.removeEventListener('keydown', escape, true))
})

function toHex(value: string): string | null {
  if (/^#[\da-f]{6}$/i.test(value)) return value.toUpperCase()
  const rgb = /^rgb\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)$/i.exec(value)
  if (!rgb) return null
  const channels = rgb.slice(1).map(Number)
  if (channels.some((channel) => channel > 255)) return null
  return `#${channels.map((channel) => channel.toString(16).padStart(2, '0')).join('')}`.toUpperCase()
}
function updateColor(value: string | null) {
  if (!value) return
  const hex = toHex(value)
  if (!hex) return
  // Keep the panel's RGB mode while persisting the existing six-digit HEX format.
  pickerValue.value = value
  color.value = hex
}
watch(color, (value) => {
  if (toHex(pickerValue.value) !== value.toUpperCase()) pickerValue.value = value
})
</script>

<template>
  <NColorPicker
    v-model:show="show"
    :value="pickerValue"
    :modes="['hex', 'rgb']"
    :actions="['confirm']"
    :show-alpha="false"
    :swatches="[...projectColors]"
    :disabled="disabled"
    @update:value="updateColor"
  >
    <template #trigger="{ onClick, ref: triggerRef }">
      <span class="color-picker-anchor">
        <IconHint :text="label" :disabled="show || disabled">
          <button
            :ref="triggerRef"
            type="button"
            class="color-picker-trigger"
            :aria-label="label"
            :aria-expanded="show"
            :disabled="disabled"
            @click="togglePicker($event, onClick)"
          >
            <span class="color-picker-swatch" :style="{ backgroundColor: color }" />
          </button>
        </IconHint>
      </span>
    </template>
  </NColorPicker>
</template>
