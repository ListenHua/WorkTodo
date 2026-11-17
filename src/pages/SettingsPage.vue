<script setup lang="ts">
import { emit } from '@tauri-apps/api/event'
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { NButton, NInput, NSwitch, NInputNumber, NPopconfirm, NSelect, NTabs, NTab } from 'naive-ui'
import { useWorkspaceStore } from '../stores/workspace'
import { useThemeStore } from '../stores/theme'
import { preferences } from '../services/preferences'
import {
  isMouseTrigger,
  mouseTriggerFromButton,
  captureTriggerLabel,
} from '../shared/capture-trigger'
import { useAppMessage } from '../composables/useAppMessage'
import { desktopApi } from '../services/desktop'
import type { DesktopStatus, StorageInfo } from '../services/desktop'
import { errorMessage } from '../shared/domain'
import AppIcon from '../components/AppIcon.vue'
import IconHint from '../components/IconHint.vue'
import AboutSettings from '../components/AboutSettings.vue'
import { useReleaseStore } from '../stores/release'
import { shortcutFromKey } from '../shared/capture-shortcut'
const workspace = useWorkspaceStore()
const release = useReleaseStore()
const message = useAppMessage()
const theme = useThemeStore()
const storageInfo = ref<StorageInfo | null>(null)
const strictOffline = ref(false)
async function refreshStorage() {
  const [info, offline] = await Promise.allSettled([
    desktopApi.storage(),
    preferences.get('strict_offline'),
  ])
  if (info.status === 'fulfilled') storageInfo.value = info.value
  if (offline.status === 'fulfilled') strictOffline.value = offline.value === 'true'
}
async function setStrictOffline(value: boolean) {
  await run(async () => {
    await preferences.set('strict_offline', String(value))
    strictOffline.value = value
  }, '已保存离线设置')
}
const savedSettings = ref('')
const dirty = computed(
  () => !!runtime.value && JSON.stringify(runtime.value.settings) !== savedSettings.value,
)
const settingsTabs = [
  { name: 'storage', label: '存储' },
  { name: 'capture', label: '快捷采集' },
  { name: 'desktop', label: '桌面与提醒' },
  { name: 'appearance', label: '外观' },
  { name: 'about', label: '关于' },
] as const
type SettingsTab = (typeof settingsTabs)[number]['name']
const settingsTab = ref<SettingsTab>('storage')
function selectSettingsTab(name: string | number) {
  if (!settingsTabs.some((tab) => tab.name === name)) return
  cancelRecording()
  settingsTab.value = name as SettingsTab
}
async function tabKeydown(event: KeyboardEvent, name: SettingsTab) {
  if (event.isComposing) return
  const index = settingsTabs.findIndex((tab) => tab.name === name)
  let target = index
  if (event.key === 'ArrowRight') target = (index + 1) % settingsTabs.length
  else if (event.key === 'ArrowLeft')
    target = (index + settingsTabs.length - 1) % settingsTabs.length
  else if (event.key === 'Home') target = 0
  else if (event.key === 'End') target = settingsTabs.length - 1
  else if (!['Enter', ' '].includes(event.key)) return
  event.preventDefault()
  const tab = settingsTabs[target]!
  selectSettingsTab(tab.name)
  await nextTick()
  document.getElementById(`settings-${tab.name}-tab`)?.focus()
}
const busy = ref(false)
const runtime = ref<DesktopStatus | null>(null)
const appliedCapture = ref({ trigger: 'MouseMiddle', holdMs: 650 })
const recording = ref(false)
const usesKeyboard = computed(
  () => !!runtime.value && !isMouseTrigger(runtime.value.settings.captureTrigger),
)
// A completed mouse press can still emit click. Consume it before allowing another recording.
let suppressTriggerClick = false
function startRecording(event: MouseEvent) {
  if (suppressTriggerClick) {
    suppressTriggerClick = false
    return
  }
  if (!busy.value) {
    // WebKit does not consistently focus buttons on a mouse click.
    const button = event.currentTarget as HTMLButtonElement
    button.focus()
    recording.value = true
  }
}
function cancelRecording() {
  recording.value = false
  suppressTriggerClick = false
}
function recordMouse(event: PointerEvent) {
  if (!recording.value) {
    suppressTriggerClick = false
    return
  }
  if (busy.value || !runtime.value || event.pointerType !== 'mouse') return
  event.preventDefault()
  event.stopPropagation()
  const trigger = mouseTriggerFromButton(event.button)
  if (!trigger) return
  if (!runtime.value.isMacos && event.button > 4) {
    message.error('当前系统仅提供五个标准鼠标按键，请使用鼠标驱动映射后录入')
    return
  }
  runtime.value.settings.captureTrigger = trigger
  recording.value = false
  suppressTriggerClick = event.button === 0
}
function recordShortcut(event: KeyboardEvent) {
  if (!recording.value || busy.value) return
  event.preventDefault()
  event.stopPropagation()
  if (event.code === 'Escape') {
    cancelRecording()
    return
  }
  const shortcut = shortcutFromKey(event)
  if (!shortcut || !runtime.value) return
  runtime.value.settings.captureTrigger = shortcut
  recording.value = false
}
const needsInputMonitoring = computed(
  () => !usesKeyboard.value && !runtime.value?.inputMonitoringPermission,
)
const middleState = computed(() => {
  if (!workspace.desktop) return '请在桌面端使用快捷采集'
  if (!runtime.value?.settings.middleCapture) return '未开启'
  if (
    runtime.value.settings.captureTrigger !== appliedCapture.value.trigger ||
    runtime.value.settings.middleHoldMs !== appliedCapture.value.holdMs
  )
    return '采集设置已修改，保存后生效'
  if (!runtime.value.middlePermission || needsInputMonitoring.value) return '等待系统授权'
  return runtime.value.middleListening
    ? usesKeyboard.value
      ? '正在监听快捷键'
      : `正在监听${captureTriggerLabel(runtime.value.settings.captureTrigger).replace('鼠标', '')}`
    : '监听尚未启动'
})
let statusTimer: ReturnType<typeof setInterval> | undefined
let disposed = false
async function refreshMiddle(retry = false) {
  if (!workspace.desktop || busy.value || disposed) return
  try {
    if (retry) await desktopApi.retryMiddle()
    const status = await desktopApi.status()
    // Preserve settings that are still being edited while updating permission/health feedback.
    if (runtime.value && !disposed) {
      appliedCapture.value = {
        trigger: status.settings.captureTrigger || 'MouseMiddle',
        holdMs: status.settings.middleHoldMs,
      }
      Object.assign(runtime.value, {
        middlePermission: status.middlePermission,
        inputMonitoringPermission: status.inputMonitoringPermission,
        middleListening: status.middleListening,
        middleError: status.middleError,
      })
    }
  } catch (e) {
    if (retry && !disposed) message.error(errorMessage(e))
  }
}
const onFocus = () => {
  void refreshMiddle(true)
  if (!busy.value) {
    void refreshStorage()
    void workspace.refresh()
  }
}
async function toggleMiddle(enabled: boolean) {
  if (!runtime.value || busy.value) return
  runtime.value.settings.middleCapture = enabled
  await run(() => desktopApi.save({ ...runtime.value!.settings }))
  await load().catch((e) => message.error(errorMessage(e)))
}
const operations = ref<Awaited<ReturnType<typeof desktopApi.pending>>>([])
async function load(preserveDraft = true) {
  const status = await desktopApi.status()
  status.settings.captureTrigger ||= 'MouseMiddle'
  appliedCapture.value = {
    trigger: status.settings.captureTrigger,
    holdMs: status.settings.middleHoldMs,
  }
  const draft = preserveDraft && dirty.value ? runtime.value?.settings : null
  runtime.value = draft ? { ...status, settings: draft } : status
  savedSettings.value = JSON.stringify(status.settings)
  operations.value = await desktopApi.pending()
}
async function run(action: () => Promise<unknown>, success?: string, reload = true) {
  if (busy.value) return
  busy.value = true
  try {
    await action()
    await workspace.refresh()
    if (workspace.desktop) void emit('workspace-changed')
    if (reload) await load()
    if (success) message.success(success)
  } catch (e) {
    message.error(errorMessage(e))
  } finally {
    await refreshStorage()
    busy.value = false
  }
}
async function recovery(action: 'export' | 'restore' | 'diagnostic') {
  await run(async () => {
    const result = await desktopApi.recovery(action)
    if (result)
      message.success(
        action === 'export'
          ? `恢复稿已导出：${result}`
          : action === 'diagnostic'
            ? `诊断副本已导出：${result}`
            : result,
      )
  })
}
async function selectStorage() {
  await run(async () => {
    // Flush the capture draft before the backend takes a snapshot of the workspace.
    await preferences.get('capture_draft')
    const previous = workspace.status.bindingId ?? workspace.status.rootPath
    const hadRoot = !!workspace.status.rootPath
    await workspace.selectRoot()
    if ((workspace.status.bindingId ?? workspace.status.rootPath) !== previous) {
      runtime.value = null
      message.success(hadRoot ? '存储目录已迁移，原目录已保留' : '存储目录已设置')
    }
  })
}
async function importProject() {
  await run(async () => {
    if (await desktopApi.data('import')) message.success('已导入项目副本')
  })
}
const storageState = computed(() => {
  if (!workspace.status.rootPath)
    return { icon: 'folder', text: '尚未选择存储位置', color: 'var(--muted)' }
  if (
    !workspace.status.available ||
    workspace.status.readOnly ||
    (storageInfo.value?.state &&
      storageInfo.value.state !== 'available' &&
      storageInfo.value.mode === 'portable')
  )
    return {
      icon: 'folder-warning',
      text: '目录不可用，请检查位置、权限或配置',
      color: 'var(--danger)',
    }
  return { icon: 'brand', text: workspace.desktop ? '目录可用' : '预览目录', color: '#18a058' }
})
onMounted(async () => {
  await load().catch((e) => message.error(errorMessage(e)))
  await refreshStorage()
  if (disposed || !workspace.desktop) return
  statusTimer = setInterval(() => {
    void refreshMiddle()
  }, 1500)
  window.addEventListener('focus', onFocus)
})
onUnmounted(() => {
  disposed = true
  if (statusTimer) clearInterval(statusTimer)
  window.removeEventListener('focus', onFocus)
})
</script>
<template>
  <NTabs
    :value="settingsTab"
    type="line"
    class="settings-tabs"
    role="tablist"
    aria-label="设置分类"
    :theme-overrides="{ tabBorderColor: 'transparent', tabGapMediumLine: '20px' }"
    @update:value="selectSettingsTab"
  >
    <NTab
      v-for="tab in settingsTabs"
      :id="`settings-${tab.name}-tab`"
      :key="tab.name"
      :name="tab.name"
      role="tab"
      :tabindex="settingsTab === tab.name ? 0 : -1"
      :aria-selected="settingsTab === tab.name"
      :aria-controls="`settings-${tab.name}-panel`"
      @keydown="tabKeydown($event, tab.name)"
      >{{ tab.label
      }}<span
        v-if="tab.name === 'about' && release.unread"
        class="release-unread"
        aria-label="有未读更新日志"
      ></span
    ></NTab>
  </NTabs>
  <div class="settings-stack">
    <div
      v-show="settingsTab === 'storage'"
      id="settings-storage-panel"
      class="settings-group"
      role="tabpanel"
      aria-labelledby="settings-storage-tab"
    >
      <section id="storage-settings" class="panel settings-panel">
        <div class="panel-heading">
          <h2>项目存储位置</h2>
          <AppIcon name="folder" :size="23" />
        </div>
        <p v-if="storageInfo?.error" class="form-error" role="alert">{{ storageInfo.error }}</p>
        <NButton v-if="storageInfo?.error" :disabled="busy" @click="recovery('diagnostic')"
          >导出诊断副本</NButton
        >
        <p v-if="storageInfo?.recoveryCount" class="form-error">
          本机有 {{ storageInfo.recoveryCount }} 个待归档的恢复文件，请先导出或归档。
        </p>
        <div class="storage-setting">
          <label>当前存储位置</label>
          <div class="storage-path">
            <IconHint :text="storageState.text">
              <span
                class="storage-status-icon"
                role="img"
                :aria-label="storageState.text"
                tabindex="0"
              >
                <AppIcon :name="storageState.icon" :size="20" :color="storageState.color" />
              </span>
            </IconHint>
            <code>{{ workspace.status.rootPath || '尚未选择存储位置' }}</code>
            <NButton :disabled="busy" :loading="busy" @click="selectStorage">{{
              workspace.status.rootPath ? '迁移存储目录' : '选择文件夹'
            }}</NButton>
          </div>
        </div>
      </section>
      <section class="panel settings-panel">
        <div class="panel-heading"><h2>数据管理</h2></div>
        <div class="dialog-footer">
          <NButton
            :disabled="
              busy ||
              !workspace.desktop ||
              storageInfo?.mode !== 'portable' ||
              !workspace.status.available ||
              !!workspace.status.readOnly
            "
            @click="importProject"
            >导入项目副本</NButton
          >
        </div>
        <div v-if="storageInfo?.recoveryCount" class="dialog-footer">
          <NButton :disabled="busy" @click="recovery('export')">导出本机恢复稿</NButton>
          <NPopconfirm :show-icon="false" @positive-click="recovery('restore')">
            <template #trigger
              ><NButton :disabled="busy || storageInfo?.state !== 'available'"
                >恢复为未分类待办</NButton
              ></template
            >
            将本机恢复稿保存为当前目录的未分类待办；恢复稿原件保留在本机归档中。
          </NPopconfirm>
        </div>
        <div class="setting-row">
          <label>严格离线</label
          ><NSwitch
            :value="strictOffline"
            :disabled="busy"
            aria-label="严格离线"
            @update:value="setStrictOffline"
          />
        </div>
      </section>
      <section v-if="operations.length" class="panel settings-panel">
        <div class="panel-heading"><h2>未完成的文件操作</h2></div>
        <p class="setting-help">
          检查目录后重新核对索引。目录改名会按数据库记录恢复；内部暂存文件和未登记目录会保留。
        </p>
        <div v-for="op in operations" :key="op.id" class="reminder-row">
          <div class="reminder-info">
            <strong>{{
              op.kind === 'rename_project'
                ? '项目改名'
                : op.kind === 'create_project'
                  ? '项目创建'
                  : op.kind === 'delete_file'
                    ? '文件删除'
                    : '文件导入'
            }}</strong>
            <p class="muted">{{ op.target }}</p>
          </div>
          <NPopconfirm
            :show-icon="false"
            @positive-click="run(() => desktopApi.recover(op.id), '已重新核对并结束该记录')"
            ><template #trigger><NButton :disabled="busy">检查并恢复</NButton></template
            >确认已核对目录，重新扫描并处理这条中断记录？</NPopconfirm
          >
        </div>
      </section>
    </div>
    <div
      v-show="settingsTab === 'capture'"
      id="settings-capture-panel"
      class="settings-group"
      role="tabpanel"
      aria-labelledby="settings-capture-tab"
    >
      <section id="capture-settings" v-if="runtime" class="panel settings-panel">
        <div class="panel-heading">
          <h2>快捷采集</h2>
          <NSwitch
            :value="runtime.settings.middleCapture"
            :disabled="!workspace.desktop || busy"
            aria-label="快捷采集"
            @update:value="toggleMiddle"
          />
        </div>
        <div class="setting-row">
          <label for="capture-trigger">触发方式</label>
          <NButton
            id="capture-trigger"
            class="capture-trigger-button"
            :disabled="busy"
            :type="recording ? 'primary' : 'default'"
            :aria-pressed="recording"
            aria-label="采集触发方式"
            :aria-describedby="recording ? 'capture-recording-help' : undefined"
            title="点击录入鼠标按键或键盘快捷键"
            @click="startRecording"
            @pointerdown="recordMouse"
            @keydown="recordShortcut"
            @blur="cancelRecording"
            @contextmenu.prevent
            @auxclick.prevent
          >
            {{
              recording
                ? '请按下鼠标键或快捷键'
                : captureTriggerLabel(runtime.settings.captureTrigger)
            }}
          </NButton>
        </div>
        <p v-if="recording" id="capture-recording-help" class="setting-help" aria-live="polite">
          在此处按下鼠标按键，或按下键盘快捷键；Esc 取消。
        </p>
        <div class="setting-row">
          <label>长按时间（毫秒）</label
          ><NInputNumber
            v-model:value="runtime.settings.middleHoldMs"
            :min="300"
            :max="3000"
            :step="50"
            :precision="0"
            :input-props="{ 'aria-label': '长按时间' }"
          />
        </div>
        <p class="setting-help" role="status">{{ middleState }}</p>
        <p class="setting-help">
          {{
            usesKeyboard
              ? '选中文字后，按住快捷键超过设定时间，松开全部按键后采集。'
              : `选中文字后，长按${captureTriggerLabel(runtime.settings.captureTrigger)}超过设定时间即可采集。`
          }}
          开关立即生效；修改按键或时间后请保存。图片需原应用支持复制图像，采集会更新剪贴板。
        </p>
        <p
          v-if="
            !usesKeyboard &&
            ['MouseLeft', 'MouseRight', 'MouseBack', 'MouseForward'].includes(
              runtime.settings.captureTrigger,
            )
          "
          class="setting-help"
        >
          保留按键原有行为；右键菜单、选区或浏览器前进后退可能影响采集，可换用其他按键。
        </p>
        <template v-if="workspace.desktop && runtime.settings.middleCapture">
          <p
            v-if="runtime.isMacos && (!runtime.middlePermission || needsInputMonitoring)"
            class="form-error"
          >
            请在系统隐私设置中允许当前运行的“项目工作台”。授权后返回此处重新检测；如果系统要求退出应用，请从托盘退出后重新打开。
          </p>
          <div class="dialog-footer">
            <NButton
              v-if="runtime.isMacos && !runtime.middlePermission"
              :disabled="busy"
              @click="run(() => desktopApi.middlePermission('accessibility'), undefined, false)"
              >允许辅助功能</NButton
            >
            <NButton
              v-if="runtime.isMacos && needsInputMonitoring"
              :disabled="busy"
              @click="run(() => desktopApi.middlePermission('input-monitoring'), undefined, false)"
              >允许输入监控</NButton
            >
            <NButton :disabled="busy" @click="refreshMiddle(true)">重新检测</NButton>
          </div>
          <p v-if="runtime.middleError" class="form-error">{{ runtime.middleError }}</p>
        </template>
      </section>
    </div>
    <div
      v-show="settingsTab === 'desktop'"
      id="settings-desktop-panel"
      class="settings-group"
      role="tabpanel"
      aria-labelledby="settings-desktop-tab"
    >
      <section id="desktop-settings" v-if="runtime" class="panel settings-panel">
        <div class="panel-heading"><h2>桌面与提醒</h2></div>
        <div class="setting-row">
          <span>关闭主窗口后常驻托盘</span
          ><NSwitch
            v-model:value="runtime.settings.closeToTray"
            aria-label="关闭主窗口后常驻托盘"
          />
        </div>
        <div class="setting-row">
          <span>后台提醒</span
          ><NSwitch v-model:value="runtime.settings.notifications" aria-label="后台提醒" />
        </div>
        <div class="setting-row">
          <span>开机启动</span
          ><NSwitch
            :value="runtime.autostart"
            :disabled="busy || !workspace.desktop"
            aria-label="开机启动"
            @update:value="(value) => run(() => desktopApi.autostart(value))"
          />
        </div>
        <label for="global-shortcut">快速添加快捷键</label
        ><NInput
          v-model:value="runtime.settings.shortcut"
          :input-props="{ id: 'global-shortcut' }"
          placeholder="留空可关闭全局快捷键"
        />
        <p v-if="runtime.shortcutError" class="form-error">{{ runtime.shortcutError }}</p>
        <p class="setting-help">
          应用运行或常驻托盘时接收提醒；选择“退出”后停止。过期未发送的提醒会在下次启动或唤醒后检查。
        </p>
        <div class="dialog-footer">
          <NButton
            :disabled="busy || !workspace.desktop"
            @click="
              run(() => desktopApi.notificationTest(), '已提交测试通知，请检查系统通知与勿扰设置')
            "
            >测试通知</NButton
          >
        </div>
      </section>
    </div>
    <div
      v-show="settingsTab === 'appearance'"
      id="settings-appearance-panel"
      class="settings-group"
      role="tabpanel"
      aria-labelledby="settings-appearance-tab"
    >
      <section id="appearance-settings" class="panel settings-panel">
        <div class="panel-heading"><h2>外观</h2></div>
        <div class="setting-row">
          <label>主题</label>
          <NSelect
            :value="theme.preference"
            :options="[
              { label: '跟随系统', value: 'system' },
              { label: '浅色', value: 'light' },
              { label: '深色', value: 'dark' },
            ]"
            aria-label="主题偏好"
            style="width: 214px"
            @update:value="theme.setTheme"
          >
            <template #arrow><AppIcon name="down" :size="16" /></template>
          </NSelect>
        </div>
        <p v-if="theme.error" class="form-error" role="alert">{{ theme.error }}</p>
      </section>
    </div>
    <div
      v-show="settingsTab === 'about'"
      id="settings-about-panel"
      class="settings-group"
      role="tabpanel"
      aria-labelledby="settings-about-tab"
    >
      <AboutSettings />
    </div>
    <div
      v-if="runtime && (dirty || settingsTab === 'capture' || settingsTab === 'desktop')"
      class="settings-savebar"
    >
      <span :class="{ 'unsaved-label': dirty }" aria-live="polite">{{
        dirty ? '有未保存的设置' : '设置已保存到本地'
      }}</span>
      <NButton v-if="dirty" :disabled="busy" @click="load(false)">还原修改</NButton>
      <NButton
        type="primary"
        :loading="busy"
        :disabled="!dirty"
        @click="run(() => desktopApi.save({ ...runtime!.settings }), '设置已保存')"
        >保存桌面设置</NButton
      >
    </div>
  </div>
</template>
