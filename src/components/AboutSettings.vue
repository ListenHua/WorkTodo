<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { NButton, NDrawer, NDrawerContent } from 'naive-ui'
import { useReleaseStore } from '../stores/release'
import { useThemeStore } from '../stores/theme'
import { compareReleaseVersions, releases, releaseCategoriesFor } from '../shared/releases'
import { useAppMessage } from '../composables/useAppMessage'
import AppIcon from './AppIcon.vue'
import EmptyState from './EmptyState.vue'

const release = useReleaseStore()
const theme = useThemeStore()
const message = useAppMessage()
const showLogs = ref(false)
const copying = ref(false)
const history = computed(() =>
  releases.filter((entry) => compareReleaseVersions(entry.version, release.info.version) === -1),
)
watch(showLogs, (show) => {
  if (show) void release.markRead()
})
async function copyInfo() {
  copying.value = true
  try {
    await navigator.clipboard.writeText(release.diagnostic)
    message.success('版本信息已复制')
  } catch {
    message.error('无法复制版本信息，请检查剪贴板权限后重试')
  } finally {
    copying.value = false
  }
}
</script>

<template>
  <section class="about-settings" aria-label="关于WorkTodo">
    <div class="about-identity">
      <img
        :src="theme.isDark ? '/app-icon.svg' : '/app-icon-light.svg'"
        width="64"
        height="64"
        alt="WorkTodo"
      />
      <div>
        <h2>WorkTodo</h2>
        <p class="about-version">
          v{{ release.info.version }}<span v-if="release.internal">内部测试版</span>
        </p>
      </div>
    </div>
    <div class="about-actions">
      <NButton @click="showLogs = true">
        更新日志<span
          v-if="release.unread"
          class="release-unread"
          aria-label="有未读更新日志"
        ></span>
      </NButton>
      <NButton :loading="copying" :disabled="!release.ready" @click="copyInfo"
        >复制版本信息</NButton
      >
    </div>
    <p v-if="release.error" class="form-error" role="alert">
      {{ release.error }} <NButton text @click="release.initialize()">重试</NButton>
    </p>
  </section>
  <NDrawer
    v-model:show="showLogs"
    :width="520"
    style="max-width: 100vw"
    placement="right"
    aria-label="更新日志"
  >
    <NDrawerContent
      title="更新日志"
      :closable="false"
      :header-style="{ borderBottom: 'none' }"
      :native-scrollbar="false"
    >
      <template #header
        ><div class="release-drawer-header">
          <span>更新日志</span>
          <NButton text circle aria-label="关闭更新日志" @click="showLogs = false"
            ><AppIcon name="close"
          /></NButton></div
      ></template>
      <EmptyState
        v-if="!release.current"
        compact
        kind="files"
        title="暂无该版本更新日志"
        action-label="点击刷新"
        @action="release.initialize()"
      />
      <article v-if="release.current" class="release-entry" aria-label="当前版本更新日志">
        <div class="release-heading">
          <h3>v{{ release.current.version }}</h3>
          <span>当前版本</span><time>{{ release.current.date }}</time>
        </div>
        <template v-for="[key, label] in releaseCategoriesFor(release.current.version)" :key="key">
          <section v-if="release.current[key].length" class="release-section">
            <h4>{{ label }}</h4>
            <ul>
              <li v-for="line in release.current[key]" :key="line">{{ line }}</li>
            </ul>
          </section>
        </template>
        <section v-if="release.current.notes?.length" class="release-section">
          <h4>说明</h4>
          <ul>
            <li v-for="line in release.current.notes" :key="line">{{ line }}</li>
          </ul>
        </section>
      </article>
      <details v-for="entry in history" :key="entry.version" class="release-history">
        <summary>
          <span class="release-history-heading">
            <span>v{{ entry.version }}</span
            ><time>{{ entry.date }}</time>
          </span>
        </summary>
        <article class="release-entry">
          <template v-for="[key, label] in releaseCategoriesFor(entry.version)" :key="key"
            ><section v-if="entry[key].length" class="release-section">
              <h4>{{ label }}</h4>
              <ul>
                <li v-for="line in entry[key]" :key="line">{{ line }}</li>
              </ul>
            </section></template
          >
          <section v-if="entry.notes?.length" class="release-section">
            <h4>说明</h4>
            <ul>
              <li v-for="line in entry.notes" :key="line">{{ line }}</li>
            </ul>
          </section>
        </article>
      </details>
    </NDrawerContent>
  </NDrawer>
</template>
