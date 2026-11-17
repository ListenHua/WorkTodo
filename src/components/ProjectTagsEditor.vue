<script setup lang="ts">
import { NButton, NColorPicker, NInput } from 'naive-ui'
import AppIcon from './AppIcon.vue'
import { projectColors, type ProjectTag } from '../shared/types'
const tags = defineModel<ProjectTag[]>({ required: true })
defineProps<{ disabled?: boolean }>()
function add() {
  tags.value = [...tags.value, { name: '', color: projectColors[0] }]
}
function remove(index: number) {
  tags.value = tags.value.filter((_, position) => position !== index)
}
</script>
<template>
  <div class="project-tag-editor" aria-label="项目标签">
    <div class="project-tag-heading">
      <span>项目标签</span>
      <NButton size="small" text :disabled="disabled || tags.length >= 20" @click="add">
        <AppIcon name="plus" :size="16" />添加标签
      </NButton>
    </div>
    <div v-if="tags.length" class="project-tag-fields">
      <div v-for="(tag, index) in tags" :key="index" class="project-tag-field">
        <NInput
          v-model:value="tag.name"
          :input-props="{ 'aria-label': `标签名称 ${index + 1}` }"
          placeholder="标签名称"
          maxlength="30"
          :disabled="disabled"
        />
        <NColorPicker
          v-model:value="tag.color"
          :aria-label="`标签颜色 ${index + 1}`"
          :modes="['hex']"
          :actions="['confirm']"
          :show-alpha="false"
          :swatches="[...projectColors]"
          :disabled="disabled"
        />
        <button
          type="button"
          class="icon-button"
          :aria-label="`移除标签 ${index + 1}`"
          :disabled="disabled"
          @click="remove(index)"
        >
          <AppIcon name="close" :size="17" />
        </button>
      </div>
    </div>
  </div>
</template>
