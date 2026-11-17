<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { NButton, NInput, NModal } from 'naive-ui'
import AppIcon from './AppIcon.vue'
import AppColorPicker from './AppColorPicker.vue'
import IconHint from './IconHint.vue'
import { useWorkspaceStore } from '../stores/workspace'
import { errorMessage } from '../shared/domain'
import { projectTagKey, validateProjectTags } from '../shared/project-tags'
import { projectColors, type ProjectTag } from '../shared/types'

const tags = defineModel<ProjectTag[]>({ required: true })
defineProps<{ disabled?: boolean }>()
const workspace = useWorkspaceStore()
const show = ref(false)
const editingIndex = ref<number | null>(null)
const name = ref('')
const color = ref<string>(projectColors[0])
const error = ref('')
// Keep tags removed from this draft available until the project dialog closes.
const recentTags = ref<ProjectTag[]>([])
const knownTags = computed(() => {
  const known = new Map<string, ProjectTag>()
  for (const tag of [
    ...tags.value,
    ...recentTags.value,
    ...workspace.projects.flatMap((project) => project.tags ?? []),
  ]) {
    const key = projectTagKey(tag.name)
    if (key && !known.has(key)) known.set(key, { ...tag })
  }
  return [...known.values()]
})
const selectedNames = computed(() => new Set(tags.value.map((tag) => projectTagKey(tag.name))))
function selected(tag: ProjectTag) {
  return selectedNames.value.has(projectTagKey(tag.name))
}
function open(index: number | null = null) {
  editingIndex.value = index
  name.value = index === null ? '' : tags.value[index]!.name
  color.value = index === null ? projectColors[0] : tags.value[index]!.color
  error.value = ''
  show.value = true
}
function apply(tag: ProjectTag) {
  try {
    const next = tags.value.map((item) => ({ ...item }))
    if (editingIndex.value === null) next.push(tag)
    else next[editingIndex.value] = tag
    const normalized = validateProjectTags(next)
    tags.value = normalized
    const key = projectTagKey(tag.name)
    recentTags.value = [
      { ...normalized.find((item) => projectTagKey(item.name) === key)! },
      ...recentTags.value.filter((item) => projectTagKey(item.name) !== key),
    ]
    show.value = false
  } catch (cause) {
    error.value = errorMessage(cause)
  }
}
function remove(index: number) {
  const tag = tags.value[index]!
  if (!recentTags.value.some((item) => projectTagKey(item.name) === projectTagKey(tag.name))) {
    recentTags.value.push({ ...tag })
  }
  tags.value = tags.value.filter((_, position) => position !== index)
}
watch([name, color], () => {
  error.value = ''
})
</script>

<template>
  <div class="project-tag-editor" aria-label="项目标签">
    <div class="project-tag-heading">
      <span>项目标签</span>
      <NButton size="small" text :disabled="disabled || tags.length >= 20" @click="open()">
        <AppIcon name="plus" :size="16" />添加标签
      </NButton>
    </div>
    <div v-if="tags.length" class="project-tag-selection" role="list" aria-label="已选标签">
      <span
        v-for="(tag, index) in tags"
        :key="index"
        class="project-tag project-tag-selection-item"
        role="listitem"
        :style="{ '--tag-color': tag.color, background: `${tag.color}14` }"
      >
        <IconHint text="编辑标签">
          <button
            type="button"
            class="project-tag-edit"
            :aria-label="`编辑标签 ${tag.name}`"
            :disabled="disabled"
            @click="open(index)"
          >
            <span class="project-tag-dot" />
            <span class="project-tag-label">{{ tag.name }}</span>
          </button>
        </IconHint>
        <IconHint text="移除标签">
          <button
            type="button"
            class="project-tag-remove"
            :aria-label="`移除标签 ${tag.name}`"
            :disabled="disabled"
            @click="remove(index)"
          >
            <AppIcon name="close" :size="14" />
          </button>
        </IconHint>
      </span>
    </div>
  </div>
  <NModal
    v-model:show="show"
    preset="card"
    :title="editingIndex === null ? '添加标签' : '编辑标签'"
    :aria-label="editingIndex === null ? '添加标签' : '编辑标签'"
    class="form-modal tag-form-modal"
    :bordered="false"
    :mask-closable="false"
    :closable="false"
  >
    <template #header-extra>
      <button type="button" class="icon-button" aria-label="关闭标签弹窗" @click="show = false">
        <AppIcon name="close" :size="20" />
      </button>
    </template>
    <form class="dialog-form" @submit.prevent="!disabled && apply({ name, color })">
      <div class="tag-form-fields">
        <NInput
          v-model:value="name"
          :input-props="{ 'aria-label': '标签名称' }"
          placeholder="标签名称"
          maxlength="30"
          :disabled="disabled"
          autofocus
        />
        <AppColorPicker v-model="color" label="标签颜色" :disabled="disabled" />
      </div>
      <p v-if="error" class="form-error" role="alert">{{ error }}</p>
      <section v-if="editingIndex === null && knownTags.length" class="tag-reuse">
        <h3>已有标签</h3>
        <div class="tag-reuse-options">
          <button
            v-for="tag in knownTags"
            :key="projectTagKey(tag.name)"
            type="button"
            class="project-tag tag-reuse-option"
            :aria-label="`复用标签 ${tag.name}`"
            :aria-pressed="selected(tag)"
            :disabled="disabled || selected(tag) || tags.length >= 20"
            :style="{ '--tag-color': tag.color, background: `${tag.color}14` }"
            @click="apply({ ...tag })"
          >
            <span class="project-tag-dot" />
            <span class="project-tag-label">{{ tag.name }}</span>
            <AppIcon v-if="selected(tag)" name="check" :size="14" />
          </button>
        </div>
      </section>
      <div class="dialog-footer">
        <NButton @click="show = false">取消</NButton>
        <NButton type="primary" attr-type="submit" :disabled="disabled">
          {{ editingIndex === null ? '添加' : '保存' }}
        </NButton>
      </div>
    </form>
  </NModal>
</template>
