<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { onUnmounted, ref } from 'vue'
import { NButton } from 'naive-ui'
import { desktop } from '../services/api'
import { shortcutFromKey, shortcutLabel } from '../shared/global-shortcuts'
import { errorMessage } from '../shared/domain'
import { useAppMessage } from '../composables/useAppMessage'

const model = defineModel<string>({ required: true })
const props = defineProps<{
  id: string
  label: string
  defaultValue: string
  isMacos: boolean
  disabled?: boolean
}>()
const message = useAppMessage()
const recording = ref(false)
const ready = ref(false)
async function stop() {
  if (!recording.value) return
  recording.value = false
  ready.value = false
  if (desktop) {
    try {
      await invoke('shortcut_recording', { active: false })
    } catch (cause) {
      message.error(errorMessage(cause))
    }
  }
}
async function start(event: MouseEvent) {
  if (props.disabled || recording.value) return
  const button = event.currentTarget as HTMLButtonElement
  button.focus()
  recording.value = true
  try {
    if (desktop) await invoke('shortcut_recording', { active: true })
    if (recording.value) ready.value = true
    else if (desktop) await invoke('shortcut_recording', { active: false })
  } catch (cause) {
    message.error(errorMessage(cause))
    void stop()
  }
}
function record(event: KeyboardEvent) {
  if (!recording.value) return
  event.stopPropagation()
  if (event.code === 'Tab') {
    void stop()
    return
  }
  event.preventDefault()
  if (event.code === 'Escape') {
    void stop()
    return
  }
  if (!ready.value || props.disabled) return
  if (
    ['Backspace', 'Delete'].includes(event.code) &&
    !event.ctrlKey &&
    !event.altKey &&
    !event.metaKey &&
    !event.shiftKey
  ) {
    model.value = ''
    void stop()
    return
  }
  const value = shortcutFromKey(event)
  if (!value) return
  model.value = value
  void stop()
}
onUnmounted(() => {
  void stop()
})
</script>

<template>
  <div class="shortcut-setting">
    <span class="shortcut-setting-label">{{ label }}</span>
    <div class="shortcut-controls">
      <NButton
        :id="id"
        :aria-label="label"
        :disabled="disabled"
        :type="recording ? 'primary' : 'default'"
        :aria-pressed="recording"
        class="capture-trigger-button"
        @click="start"
        @keydown="record"
        @blur="stop"
      >
        {{ recording ? '请按下快捷键…' : shortcutLabel(model, isMacos) }}
      </NButton>
      <NButton :disabled="disabled || !model" :aria-label="`清除${label}`" @click="model = ''"
        >清除</NButton
      >
      <NButton
        :disabled="disabled || model === defaultValue"
        :aria-label="`恢复${label}默认设置`"
        @click="model = defaultValue"
        >恢复默认</NButton
      >
    </div>
  </div>
</template>
