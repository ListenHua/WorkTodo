import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { desktop } from '../services/api'
import { preferences } from '../services/preferences'
import { compareReleaseVersions, hasUnreadRelease, releases } from '../shared/releases'

type ApplicationInfo = { version: string; platform: string; arch: string | null }
export const useReleaseStore = defineStore('release', () => {
  const info = ref<ApplicationInfo>({
    version: __APP_BUILD__.version,
    platform: '浏览器预览',
    arch: null,
  })
  const seen = ref<string | null>(null)
  const ready = ref(false)
  const error = ref('')
  const internal = __APP_BUILD__.channel !== 'stable'
  const current = computed(() => releases.find((entry) => entry.version === info.value.version))
  const unread = computed(() => ready.value && hasUnreadRelease(info.value.version, seen.value))
  let pending: Promise<void> | undefined
  async function load() {
    ready.value = false
    error.value = ''
    try {
      if (desktop) info.value = await invoke<ApplicationInfo>('app_version_info')
      if (info.value.version !== __APP_BUILD__.version)
        throw new Error('应用与页面版本不一致，请重新安装完整应用')
      const saved = await preferences.get('changelog_seen')
      if (saved === null) {
        // First use establishes a baseline, including upgrades from builds without this feature.
        await preferences.set('changelog_seen', info.value.version)
        seen.value = info.value.version
      } else seen.value = saved
      ready.value = true
    } catch (failure) {
      error.value = failure instanceof Error ? failure.message : '无法读取本地版本信息，请重试'
    }
  }
  function initialize() {
    if (!pending)
      pending = load().finally(() => {
        pending = undefined
      })
    return pending
  }
  async function markRead() {
    await (pending ?? (ready.value ? Promise.resolve() : initialize()))
    if (!ready.value || !current.value) return
    if (seen.value && compareReleaseVersions(info.value.version, seen.value) === -1) return
    try {
      await preferences.set('changelog_seen', info.value.version)
      seen.value = info.value.version
      error.value = ''
    } catch {
      error.value = '日志已打开，但未能保存已读状态，请重试'
    }
  }
  const diagnostic = computed(() =>
    [
      'WorkTodo',
      `版本：${info.value.version}${internal ? '（内部测试版）' : ''}${desktop ? '' : '（浏览器预览）'}`,
      `系统：${info.value.platform}`,
      `架构：${info.value.arch ?? '不适用'}`,
      `提交：${__APP_BUILD__.revision ?? '未提交'}${__APP_BUILD__.dirty ? '（含本地修改）' : ''}`,
    ].join('\n'),
  )
  return { info, ready, error, internal, current, unread, diagnostic, initialize, markRead }
})
