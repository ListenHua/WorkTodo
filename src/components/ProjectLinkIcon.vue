<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { brandPaths } from '../assets/project-brands'
import type { ProjectLink } from '../shared/types'
import { validIconData } from '../shared/project-links'
const props = defineProps<{ link: Pick<ProjectLink, 'name' | 'icon' | 'iconData'> }>()
const letter = computed(() => [...props.link.name.trim()][0] || '链')
const failed = ref(false)
watch(
  () => props.link.iconData,
  () => {
    failed.value = false
  },
)
const image = computed(() =>
  props.link.icon === 'website' && validIconData(props.link.iconData) && !failed.value
    ? props.link.iconData
    : '',
)
const brand = computed(() =>
  props.link.icon && props.link.icon !== 'website' ? brandPaths[props.link.icon] : '',
)
</script>
<template>
  <span
    class="project-link-icon"
    :class="image ? 'brand-website' : brand ? `brand-${link.icon}` : 'brand-letter'"
    aria-hidden="true"
  >
    <img v-if="image" :src="image" alt="" width="22" height="22" @error="failed = true" />
    <svg
      v-else-if="brand"
      viewBox="0 0 24 24"
      width="21"
      height="21"
      fill="currentColor"
      focusable="false"
    >
      <path :d="brand" />
    </svg>
    <span v-else>{{ letter }}</span>
  </span>
</template>
