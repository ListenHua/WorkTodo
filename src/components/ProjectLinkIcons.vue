<script setup lang="ts">
import IconHint from './IconHint.vue'
import { NPopover } from 'naive-ui'
import type { ProjectLink } from '../shared/types'
import { openProjectLink } from '../services/project-links'
import { useAppMessage } from '../composables/useAppMessage'
import { errorMessage } from '../shared/domain'
import ProjectLinkIcon from './ProjectLinkIcon.vue'
import AppIcon from './AppIcon.vue'
withDefaults(defineProps<{ links?: ProjectLink[]; disabled?: boolean }>(), {
  links: () => [],
  disabled: false,
})
const message = useAppMessage()
async function open(link: ProjectLink) {
  try {
    await openProjectLink(link.url)
  } catch (error) {
    message.error(errorMessage(error))
  }
}
</script>
<template>
  <div class="project-link-icons" aria-label="项目链接">
    <IconHint :text="link.name" v-for="link in links.slice(0, 4)" :key="link.id"
      ><button
        class="project-link-button"
        :disabled="disabled"
        :aria-label="`打开链接 ${link.name}`"
        @click.stop="open(link)"
      >
        <ProjectLinkIcon :link="link" /></button
    ></IconHint>
    <NPopover
      v-if="links.length > 4"
      :disabled="disabled"
      trigger="click"
      placement="bottom-start"
      :show-arrow="false"
    >
      <template #trigger>
        <button
          class="project-link-button"
          :disabled="disabled"
          aria-label="更多项目链接"
          @click.stop
        >
          <AppIcon name="more" />
        </button>
      </template>
      <div class="project-link-overflow" aria-label="其他项目链接">
        <button
          v-for="link in links.slice(4)"
          :key="link.id"
          class="project-link-overflow-item"
          :disabled="disabled"
          :title="link.name"
          :aria-label="`打开链接 ${link.name}`"
          @click.stop="open(link)"
        >
          <ProjectLinkIcon :link="link" /><span>{{ link.name }}</span
          ><AppIcon name="external" :size="15" />
        </button>
      </div>
    </NPopover>
  </div>
</template>
