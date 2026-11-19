<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'

import { NConfigProvider, NGlobalStyle, NMessageProvider, zhCN, dateZhCN } from 'naive-ui'
import { useThemeStore } from './stores/theme'
import AppShell from './components/AppShell.vue'
import ProjectDialog from './components/ProjectDialog.vue'
import FolderProjectsDialog from './components/FolderProjectsDialog.vue'
import TaskDialog from './components/TaskDialog.vue'
import { useWorkspaceStore } from './stores/workspace'
import { useReleaseStore } from './stores/release'
const route = useRoute()
const toolbarHeight = ref(0)
const quick = computed(() => route.path === '/capture')
const workspace = useWorkspaceStore()
const theme = useThemeStore()
onMounted(() => {
  void workspace.refresh()
  if (!quick.value) void useReleaseStore().initialize()
})
</script>

<template>
  <NConfigProvider
    :theme="theme.naiveTheme"
    :theme-overrides="theme.overrides"
    :locale="zhCN"
    :date-locale="dateZhCN"
  >
    <NGlobalStyle />
    <NMessageProvider
      placement="top-right"
      :container-style="{
        top: quick
          ? '12px'
          : route.path === '/settings'
            ? 'calc(var(--settings-topbar-height) + 8px)'
            : `calc(var(--topbar-height) + ${toolbarHeight}px + 8px)`,
      }"
    >
      <RouterView v-if="quick" /><AppShell v-else @toolbar-resize="toolbarHeight = $event"
        ><RouterView
          :key="route.path === '/settings' ? 'settings' : workspace.status.bindingId || 'unbound'"
      /></AppShell>
      <ProjectDialog v-if="!quick" :key="`project-${workspace.status.bindingId}`" />
      <FolderProjectsDialog v-if="!quick" :key="`folders-${workspace.status.bindingId}`" />
      <TaskDialog v-if="!quick" :key="`task-${workspace.status.bindingId}`" />
    </NMessageProvider>
  </NConfigProvider>
</template>
