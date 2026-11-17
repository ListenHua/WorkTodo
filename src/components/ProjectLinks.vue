<script setup lang="ts">
import IconHint from './IconHint.vue'
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { NButton, NInput, NModal, NPopconfirm } from 'naive-ui'
import type { Project, ProjectLink } from '../shared/types'
import {
  defaultLinkName,
  linkIcons,
  normalizeLinkUrl,
  validateProjectLinks,
} from '../shared/project-links'
import { extractProjectLinkIcon, openProjectLink } from '../services/project-links'
import { useWorkspaceStore } from '../stores/workspace'
import { useAppMessage } from '../composables/useAppMessage'
import { errorMessage } from '../shared/domain'
import ProjectLinkIcon from './ProjectLinkIcon.vue'
import AppIcon from './AppIcon.vue'
import EmptyState from './EmptyState.vue'
const props = defineProps<{ project: Project }>()
const workspace = useWorkspaceStore()
const message = useAppMessage()
const links = computed(() => props.project.links ?? [])
const show = ref(false)
const editingId = ref<string | null>(null)
const name = ref('')
const url = ref('')
const icon = ref<ProjectLink['icon']>('')
const iconData = ref<string>()
const extracting = ref(false)
const iconError = ref('')
let extraction = 0
const busy = ref(false)
const error = ref('')
watch(
  () => props.project.id,
  () => {
    show.value = false
  },
)
watch(
  url,
  (next, previous) => {
    let changed = next !== previous
    try {
      changed = normalizeLinkUrl(next) !== normalizeLinkUrl(previous)
    } catch {
      /* incomplete input */
    }
    if (changed) {
      ++extraction
      extracting.value = false
      iconData.value = undefined
      iconError.value = ''
      if (icon.value === 'website') icon.value = ''
    }
  },
  { flush: 'sync' },
)
watch(show, () => {
  ++extraction
  extracting.value = false
})
onBeforeUnmount(() => {
  ++extraction
})
async function extractIcon() {
  if (extracting.value || busy.value) return
  if (iconData.value && icon.value !== 'website') {
    icon.value = 'website'
    fillName()
    return
  }
  const request = ++extraction
  extracting.value = true
  iconError.value = ''
  try {
    const result = await extractProjectLinkIcon(url.value)
    if (request !== extraction || !show.value) return
    iconData.value = result
    icon.value = 'website'
    fillName()
  } catch (cause) {
    if (request === extraction && show.value) iconError.value = errorMessage(cause)
  } finally {
    if (request === extraction) extracting.value = false
  }
}
function chooseIcon(value: ProjectLink['icon']) {
  icon.value = value
  fillName()
}
function fillName() {
  if (!name.value.trim()) name.value = defaultLinkName(icon.value, url.value)
}
function normalizeAddress() {
  try {
    url.value = normalizeLinkUrl(url.value)
  } catch {
    /* validate on save */
  }
}
function edit(link?: ProjectLink) {
  editingId.value = link?.id ?? null
  name.value = link?.name ?? ''
  url.value = link?.url ?? ''
  icon.value = link?.icon ?? ''
  iconData.value = link?.iconData
  iconError.value = ''
  error.value = ''
  show.value = true
}
async function persist(next: ProjectLink[]) {
  const { name, description, color } = props.project
  await workspace.updateProject(props.project.id, {
    name,
    description,
    color,
    links: validateProjectLinks(next),
  })
}
async function save() {
  fillName()
  if (busy.value || extracting.value) return
  busy.value = true
  error.value = ''
  try {
    const link: ProjectLink = {
      id: editingId.value ?? crypto.randomUUID(),
      name: name.value,
      url: url.value,
      icon: icon.value,
      ...(icon.value === 'website' ? { iconData: iconData.value } : {}),
    }
    const next = editingId.value
      ? links.value.map((old) => (old.id === editingId.value ? link : old))
      : [...links.value, link]
    await persist(next)
    show.value = false
    message.success(editingId.value ? '链接已保存' : '链接已添加')
  } catch (cause) {
    error.value = errorMessage(cause)
  } finally {
    busy.value = false
  }
}
async function remove(link: ProjectLink) {
  if (busy.value) return false
  busy.value = true
  try {
    await persist(links.value.filter((item) => item.id !== link.id))
    message.success('链接已删除')
  } catch (cause) {
    message.error(errorMessage(cause))
    return false
  } finally {
    busy.value = false
  }
}
async function open(link: ProjectLink) {
  try {
    await openProjectLink(link.url)
  } catch (cause) {
    message.error(errorMessage(cause))
  }
}
</script>
<template>
  <section class="panel project-links-panel" aria-label="项目链接列表">
    <div class="panel-heading">
      <h2>项目链接</h2>
      <NButton type="primary" :disabled="busy || links.length >= 50" @click="edit()"
        ><AppIcon name="plus" :size="17" />添加链接</NButton
      >
    </div>
    <ul v-if="links.length" class="project-links-list">
      <li v-for="link in links" :key="link.id" class="project-link-row">
        <button
          class="project-link-entry"
          :aria-label="`打开链接 ${link.name}`"
          :title="link.url"
          @click="open(link)"
        >
          <ProjectLinkIcon :link="link" /><span class="project-link-info"
            ><strong>{{ link.name }}</strong
            ><small>{{ link.url }}</small></span
          ><AppIcon name="external" :size="17" />
        </button>
        <IconHint text="编辑"
          ><button
            class="icon-button"
            :disabled="busy"
            :aria-label="`编辑链接 ${link.name}`"
            @click="edit(link)"
          >
            <AppIcon name="edit" :size="18" /></button
        ></IconHint>
        <NPopconfirm
          :show-icon="false"
          :positive-button-props="{ type: 'error', disabled: busy }"
          @positive-click="remove(link)"
          ><template #trigger
            ><span class="icon-hint-anchor"
              ><IconHint text="删除"
                ><button
                  class="icon-button danger-text"
                  :disabled="busy"
                  :aria-label="`删除链接 ${link.name}`"
                >
                  <AppIcon name="delete" :size="18" /></button></IconHint></span></template
          >删除“{{ link.name }}”？</NPopconfirm
        >
      </li>
    </ul>
    <EmptyState v-else title="暂无链接" />
  </section>
  <NModal
    v-model:show="show"
    preset="card"
    :title="editingId ? '编辑链接' : '添加链接'"
    :aria-label="editingId ? '编辑链接' : '添加链接'"
    class="form-modal"
    :bordered="false"
    :closable="false"
    :mask-closable="false"
    :close-on-esc="!busy"
  >
    <template #header-extra
      ><button class="icon-button" aria-label="关闭链接弹窗" :disabled="busy" @click="show = false">
        <AppIcon name="close" /></button
    ></template>
    <form class="dialog-form" @submit.prevent="save">
      <label id="project-link-icon-label">图标</label>
      <div class="link-icon-picker" role="radiogroup" aria-labelledby="project-link-icon-label">
        <IconHint :text="option.label" v-for="option in linkIcons" :key="option.value"
          ><button
            type="button"
            role="radio"
            class="link-icon-choice"
            :class="{ selected: icon === option.value }"
            :aria-checked="icon === option.value"
            :aria-label="option.label"
            :disabled="busy || extracting"
            @click="chooseIcon(option.value)"
          >
            <span v-if="!option.value" class="link-icon-default">默认</span>
            <ProjectLinkIcon v-else :link="{ name, icon: option.value }" /></button
        ></IconHint>
        <IconHint :text="extracting ? '正在提取网站图标' : '提取网站图标'"
          ><button
            type="button"
            role="radio"
            class="link-icon-choice"
            :class="{ selected: icon === 'website', 'is-extracting': extracting }"
            :aria-checked="icon === 'website'"
            :aria-label="extracting ? '正在提取网站图标' : '提取网站图标'"
            :disabled="busy || extracting || !url.trim()"
            @click="extractIcon"
          >
            <ProjectLinkIcon
              v-if="iconData && !extracting"
              :link="{ name, icon: 'website', iconData }"
            />
            <AppIcon v-else name="refresh" :size="21" /></button
        ></IconHint>
      </div>
      <p v-if="iconError" class="form-error" role="status">{{ iconError }}</p>
      <label for="project-link-name">链接名称</label
      ><NInput
        v-model:value="name"
        :input-props="{ id: 'project-link-name' }"
        maxlength="80"
        placeholder="例如：测试环境"
        :disabled="busy"
        autofocus
      />
      <label for="project-link-url">URL</label
      ><NInput
        v-model:value="url"
        :input-props="{ id: 'project-link-url' }"
        maxlength="2048"
        placeholder="例如：example.com"
        :disabled="busy"
        @blur="normalizeAddress"
      />
      <p v-if="error" class="form-error" role="alert">{{ error }}</p>
      <div class="dialog-footer">
        <NButton :disabled="busy" @click="show = false">取消</NButton
        ><NButton type="primary" attr-type="submit" :loading="busy" :disabled="extracting"
          >保存链接</NButton
        >
      </div>
    </form>
  </NModal>
</template>
