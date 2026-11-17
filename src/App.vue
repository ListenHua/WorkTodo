<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useRoute } from 'vue-router'

import { NConfigProvider, NGlobalStyle, NMessageProvider, zhCN, dateZhCN } from 'naive-ui'
import { useThemeStore } from './stores/theme'
import AppShell from './components/AppShell.vue'
import ProjectDialog from './components/ProjectDialog.vue'
import TaskDialog from './components/TaskDialog.vue'
import { useWorkspaceStore } from './stores/workspace'
import { useReleaseStore } from './stores/release'
const route = useRoute()
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
            : 'calc(var(--topbar-height) + 8px)',
      }"
    >
      <RouterView v-if="quick" /><AppShell v-else
        ><RouterView
          :key="route.path === '/settings' ? 'settings' : workspace.status.bindingId || 'unbound'"
      /></AppShell>
      <ProjectDialog v-if="!quick" :key="`project-${workspace.status.bindingId}`" />
      <TaskDialog v-if="!quick" :key="`task-${workspace.status.bindingId}`" />
    </NMessageProvider>
  </NConfigProvider>
</template>
