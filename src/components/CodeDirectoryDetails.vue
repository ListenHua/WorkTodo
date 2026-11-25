<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { NButton, NSelect, NSkeleton } from 'naive-ui'
import { useWorkspaceStore } from '../stores/workspace'
import { useCodeSpaceStore } from '../stores/code-space'
import { useUiStore } from '../stores/ui'
import { useAppMessage } from '../composables/useAppMessage'
import { useCodeGitRefresh } from '../composables/useCodeGitRefresh'
import { desktop } from '../services/api'
import { codeApi } from '../services/code-space'
import { openProjectLink } from '../services/project-links'
import { errorMessage } from '../shared/domain'
import { codeDate, formatCodeSize, gitChangeLabel, remoteWebUrl } from '../shared/code-space'
import AppIcon from './AppIcon.vue'
import IconHint from './IconHint.vue'
import CodeStatus from './CodeStatus.vue'
import CodeCleanupDialog from './CodeCleanupDialog.vue'
import EmptyState from './EmptyState.vue'
const props = defineProps<{ projectId: string; codeId: string; codePath: string }>()
const workspace = useWorkspaceStore(),
  code = useCodeSpaceStore(),
  ui = useUiStore(),
  message = useAppMessage()
const refreshGit = useCodeGitRefresh(props)
const item = computed(() => code.entry(props.projectId, props.codeId))
const git = computed(() => item.value.git)
const usage = computed(() => item.value.usage)
const directoryName = computed(
  () => props.codePath.split(/[\\/]/).filter(Boolean).at(-1) || props.codePath,
)
const cleanupVisible = ref(false),
  remoteName = ref(''),
  changeFilter = ref('all')
const remote = computed(
  () => git.value?.remotes.find((r) => r.name === remoteName.value) || git.value?.remotes[0],
)
const webUrl = computed(() => remoteWebUrl(remote.value?.fetchUrl || remote.value?.pushUrl || ''))
const remoteOptions = computed(
  () => git.value?.remotes.map((r) => ({ label: r.name, value: r.name })) || [],
)
const filters = [
  { label: '全部变动', value: 'all' },
  { label: '已暂存', value: 'staged' },
  { label: '未暂存', value: 'worktree' },
  { label: '存在冲突', value: 'conflict' },
]
const changes = computed(
  () =>
    git.value?.changes.filter((change) => {
      if (
        !`${change.path} ${change.originalPath || ''}`
          .toLocaleLowerCase()
          .includes(ui.search.trim().toLocaleLowerCase())
      )
        return false
      if (changeFilter.value === 'staged') return change.index !== '.' && change.index !== '?'
      if (changeFilter.value === 'worktree') return change.worktree !== '.'
      if (changeFilter.value === 'conflict') return change.conflict
      return true
    }) || [],
)
const dependencyPercent = computed(() =>
  usage.value?.bytes ? Math.min(100, (usage.value.dependencyBytes / usage.value.bytes) * 100) : 0,
)
watch(
  [
    () => props.projectId,
    () => props.codeId,
    () => props.codePath,
    () => workspace.status.bindingId,
    () => workspace.status.rootPath,
    () => workspace.status.available,
  ],
  () => {
    if (!workspace.status.available) return
    void refreshGit()
    if (desktop && !item.value.usage) void code.scan(props.projectId, props.codeId)
  },
  { immediate: true },
)
watch(remoteOptions, (options) => {
  if (!options.some((option) => option.value === remoteName.value))
    remoteName.value = options[0]?.value || ''
})
async function perform(action: () => Promise<unknown>) {
  try {
    await action()
  } catch (error) {
    message.error(errorMessage(error))
  }
}
async function copyAddress(address: string) {
  await perform(async () => {
    await navigator.clipboard.writeText(address)
    message.success('Git 地址已复制')
  })
}
function resetSearch() {
  ui.search = ''
  changeFilter.value = 'all'
}
onBeforeUnmount(() => code.cancel(props.projectId, props.codeId))
</script>
<template>
  <section class="project-code" aria-label="项目代码空间">
    <section v-if="git?.state !== 'missing'" class="code-section" aria-label="代码空间占用">
      <div class="code-section-heading">
        <h3>{{ directoryName }}</h3>
        <div class="project-code-actions">
          <IconHint text="打开文件夹">
            <NButton
              quaternary
              circle
              size="small"
              aria-label="打开文件夹"
              :disabled="!desktop || !workspace.status.available"
              @click="
                perform(() =>
                  codeApi.openDirectory(projectId, codeId, workspace.status.bindingId || ''),
                )
              "
            >
              <template #icon><AppIcon name="folder" :size="18" /></template>
            </NButton>
          </IconHint>
          <NButton
            v-if="item.scanning"
            :disabled="item.canceling"
            @click="code.cancel(projectId, codeId)"
            >{{ item.canceling ? '正在取消…' : '取消统计' }}</NButton
          >
          <IconHint v-else :text="usage ? '重新统计' : '统计体积'">
            <NButton
              quaternary
              circle
              size="small"
              :aria-label="usage ? '重新统计' : '统计体积'"
              :disabled="!desktop || !workspace.status.available"
              @click="code.scan(projectId, codeId)"
            >
              <template #icon><AppIcon name="refresh" :size="18" /></template>
            </NButton>
          </IconHint>
          <NButton
            :disabled="
              !desktop ||
              !workspace.status.available ||
              !!workspace.status.readOnly ||
              item.scanning ||
              !usage?.dependencies.length
            "
            @click="cleanupVisible = true"
            >清理依赖</NButton
          >
        </div>
      </div>
      <div v-if="item.scanning" class="code-scan-progress" role="status" aria-live="polite">
        正在统计
        {{
          item.progress
            ? `${item.progress.files} 个文件 · ${formatCodeSize(item.progress.bytes)}`
            : '代码目录…'
        }}<span v-if="item.progress?.current" class="muted">{{ item.progress.current }}</span>
      </div>
      <p v-if="item.sizeError" class="form-error" role="alert">{{ item.sizeError }}</p>
      <template v-if="usage">
        <div class="code-size-summary">
          <div>
            <span>代码总计</span><strong>{{ formatCodeSize(usage.bytes) }}</strong>
          </div>
          <div>
            <span>依赖目录</span><strong>{{ formatCodeSize(usage.dependencyBytes) }}</strong>
          </div>
          <div>
            <span>其他文件</span
            ><strong>{{ formatCodeSize(Math.max(0, usage.bytes - usage.dependencyBytes)) }}</strong>
          </div>
        </div>
        <div
          class="code-usage-bar"
          role="img"
          :aria-label="`依赖占比 ${Math.round(dependencyPercent)}%`"
        >
          <span :style="{ width: `${dependencyPercent}%` }"></span>
        </div>
        <p class="muted code-note">
          {{ usage.files }} 个文件 · {{ codeDate(usage.scannedAt) }} 统计 ·
          体积为估算值，不跟随符号链接
        </p>
        <div v-if="usage.directories.length" class="code-directory-list">
          <div v-for="directory in usage.directories" :key="directory.name">
            <span>{{ directory.name }}</span><span class="muted">{{
              formatCodeSize(directory.bytes)
            }}</span>
          </div>
        </div>
        <div v-if="usage.warnings.length" class="code-scan-warnings" role="status">
          <p>以下内容未能完整统计，当前显示已读取的体积：</p>
          <p v-for="warning in usage.warnings" :key="warning">{{ warning }}</p>
        </div>
        <p v-if="!usage.dependencies.length" class="muted code-note">未检测到依赖目录。</p>
      </template>
      <NSkeleton v-else-if="item.scanning" text :height="70" />
      <p v-else class="muted code-note">
        {{ desktop ? '尚未统计代码体积，点击统计体积开始。' : '请在桌面端统计代码目录体积。' }}
      </p>
    </section>
    <section v-if="git?.state !== 'missing'" class="code-section" aria-label="Git 变动文件">
      <div class="code-section-heading">
        <div class="code-section-title">
          <h3>
            Git 变动 <span v-if="git?.state === 'ready'" class="muted">{{ git.totalChanges }}</span>
          </h3>
          <NButton
            quaternary
            circle
            size="small"
            aria-label="刷新 Git 状态"
            :aria-busy="item.gitLoading"
            :loading="item.gitLoading"
            :disabled="!workspace.status.available"
            @click="code.inspect(projectId, codeId)"
          >
            <template #icon><AppIcon name="refresh" :size="18" /></template>
          </NButton>
          <NButton
            quaternary
            circle
            size="small"
            aria-label="在终端中打开代码目录"
            :disabled="!desktop || !workspace.status.available"
            @click="
              perform(() =>
                codeApi.openTerminal(projectId, codeId, workspace.status.bindingId || ''),
              )
            "
          >
            <template #icon><AppIcon name="terminal" :size="18" /></template>
          </NButton>
        </div>
        <NSelect
          v-if="git?.state === 'ready'"
          v-model:value="changeFilter"
          :options="filters"
          style="width: 150px"
          :input-props="{ 'aria-label': 'Git 变动类型筛选' }"
        />
      </div>
      <p v-if="item.gitError" class="form-error" role="alert">{{ item.gitError }}</p>
      <p v-if="git?.message" class="muted code-note">{{ git.message }}</p>
      <template v-if="git?.state === 'ready'">
        <p v-if="git.truncated" class="muted code-note">
          当前显示前 500 项变动，完整内容请在 Git 客户端查看。
        </p>
        <div v-if="changes.length" class="code-change-list">
          <div v-for="change in changes" :key="change.path" class="code-change-row">
            <span class="code-change-kind" :class="{ conflict: change.conflict }">{{
              gitChangeLabel(change)
            }}</span>
            <div class="code-change-path">
              <span>{{ change.path }}</span><small v-if="change.originalPath" class="muted"
                >原路径：{{ change.originalPath }}</small
              ><small class="muted"
                >{{ change.index !== '.' && change.index !== '?' ? '已暂存' : ''
                }}{{
                  change.index !== '.' && change.index !== '?' && change.worktree !== '.'
                    ? ' · '
                    : ''
                }}{{ change.worktree !== '.' ? '未暂存' : '' }}</small
              >
            </div>
            <button
              type="button"
              class="icon-button"
              :aria-label="`打开 ${change.path} 所在文件夹`"
              @click="
                perform(() =>
                  codeApi.reveal(projectId, codeId, workspace.status.bindingId || '', change.path),
                )
              "
            >
              <AppIcon name="folder" :size="18" />
            </button>
          </div>
        </div>
        <EmptyState
          v-else
          compact
          :kind="git.totalChanges ? 'search' : 'files'"
          :title="git.totalChanges ? '没有匹配的变动' : '当前代码目录没有 Git 变动'"
          :action-label="git.totalChanges ? '点击重置' : '点击刷新'"
          @action="git.totalChanges ? resetSearch() : code.inspect(projectId, codeId)"
        />
      </template>
      <CodeStatus
        v-else-if="!item.gitError && !git?.message"
        :info="git"
        :loading="item.gitLoading"
      />
    </section>
    <section
      v-if="git?.state === 'ready'"
      class="code-section code-repository"
      aria-label="仓库信息"
    >
      <dl class="code-facts">
        <div>
          <dt>最近提交</dt>
          <dd v-if="git.commit">
            <span class="muted code-commit-hash">{{ git.commit.hash }}</span>{{ git.commit.subject
            }}<span class="muted code-commit-time">{{ codeDate(git.commit.time) }}</span>
          </dd>
          <dd v-else class="muted">尚无提交</dd>
        </div>
        <div>
          <dt>跟踪分支</dt>
          <dd>
            {{ git.upstream || '未设置'
            }}<span v-if="git.upstream" class="muted code-sync">{{
              git.ahead || git.behind
                ? `领先 ${git.ahead} 次提交 · 落后 ${git.behind} 次提交`
                : '与本地远程记录一致'
            }}</span>
          </dd>
        </div>
      </dl>
      <p v-if="git.scope" class="muted code-note">
        当前代码目录位于父级仓库，变动列表仅统计本代码目录。
      </p>
      <p v-if="git.upstream" class="muted code-note">同步情况依据本地记录，未自动连接远程仓库。</p>
      <div class="code-remote-heading">
        <h3>Git 地址</h3>
        <NSelect
          v-if="remoteOptions.length > 1"
          v-model:value="remoteName"
          :options="remoteOptions"
          style="width: 150px"
          :input-props="{ 'aria-label': '选择 Git 远程仓库' }"
        /><span v-else-if="remote" class="muted">{{ remote.name }}</span>
      </div>
      <template v-if="remote">
        <div
          v-for="address in [
            { label: '拉取', value: remote.fetchUrl },
            { label: '推送', value: remote.pushUrl },
          ].filter((a, index, list) => !!a.value && (index === 0 || a.value !== list[0]!.value))"
          :key="address.label"
          class="code-remote-row"
        >
          <span class="muted">{{ address.label }}</span><code>{{ address.value }}</code
          ><NButton text @click="copyAddress(address.value)">复制地址</NButton>
        </div>
        <NButton
          v-if="webUrl"
          text
          class="code-open-remote"
          @click="perform(() => openProjectLink(webUrl!))"
          >打开仓库网页<AppIcon name="external" :size="16"
        /></NButton>
      </template>
      <p v-else class="muted code-note">尚未配置远程仓库地址。</p>
    </section>
    <CodeCleanupDialog v-model="cleanupVisible" :project-id="projectId" :code-id="codeId" />
  </section>
</template>
<style scoped>
.project-code {
  min-width: 0;
}
.project-code-actions,
.code-section-heading,
.code-remote-heading,
.code-remote-row {
  display: flex;
  align-items: center;
  gap: 16px;
  flex-wrap: wrap;
}
.project-code-actions {
  margin-left: auto;
  gap: 10px;
}
.code-section {
  padding: 24px 0;
  border-top: 1px solid var(--border);
}
.project-code > .code-section:first-child {
  padding-top: 0;
  border-top: 0;
}
.code-section-title {
  display: flex;
  align-items: center;
  gap: 8px;
}
.code-section-heading {
  justify-content: space-between;
  margin-bottom: 24px;
}
.code-section h3 {
  font-size: 14px;
  font-weight: 600;
}
.code-section h3 .muted {
  margin-left: 7px;
  font-size: 12px;
  font-weight: 400;
}
.code-facts {
  margin: 0;
}
.code-facts > div {
  display: flex;
  gap: 20px;
  margin-bottom: 16px;
}
.code-facts dt {
  flex: 0 0 65px;
  color: var(--muted);
  font-size: 12px;
}
.code-facts dd {
  margin: 0;
  font-size: 13px;
  overflow-wrap: anywhere;
}
.code-commit-hash {
  margin-right: 10px;
  font-family: monospace;
}
.code-commit-time,
.code-sync {
  display: inline-block;
  margin-left: 14px;
  font-size: 12px;
}
.code-note {
  margin: 12px 0;
  font-size: 12px;
  line-height: 1.7;
}
.code-remote-heading {
  margin: 26px 0 14px;
  font-size: 12px;
}
.code-remote-row {
  padding: 10px 0;
  font-size: 12px;
  flex-wrap: nowrap;
}
.code-remote-row code {
  flex: 1;
  overflow-wrap: anywhere;
  min-width: 0;
  font-family: inherit;
}
.code-remote-row > span {
  flex-shrink: 0;
}
.code-open-remote {
  margin-top: 8px;
}
.code-open-remote :deep(.n-button__content) {
  gap: 8px;
}
.code-size-summary {
  display: flex;
  gap: 50px;
  flex-wrap: wrap;
  margin-bottom: 20px;
}
.code-size-summary > div {
  display: flex;
  flex-direction: column;
  gap: 9px;
}
.code-size-summary span {
  color: var(--muted);
  font-size: 12px;
}
.code-size-summary strong {
  font-size: 25px;
  line-height: 1.3;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
.code-usage-bar {
  height: 7px;
  border-radius: 4px;
  overflow: hidden;
  background: var(--border);
}
.code-usage-bar span {
  display: block;
  height: 100%;
  border-radius: 4px;
  background: var(--accent);
}
.code-directory-list {
  margin-top: 24px;
  max-width: 640px;
}
.code-directory-list > div {
  display: flex;
  justify-content: space-between;
  gap: 24px;
  padding: 9px 0;
  font-size: 12px;
}
.code-directory-list > div > span:first-child {
  overflow-wrap: anywhere;
  min-width: 0;
}
.code-directory-list > div > span:last-child {
  white-space: nowrap;
}
.code-scan-progress {
  margin-bottom: 18px;
  font-size: 12px;
}
.code-scan-progress span {
  display: block;
  margin-top: 7px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.code-scan-warnings {
  margin: 16px 0;
  font-size: 12px;
  color: #c16b15;
}
.code-change-row {
  display: flex;
  gap: 16px;
  align-items: center;
  padding: 14px 0;
}
.code-change-row + .code-change-row {
  border-top: 1px solid var(--border);
}
.code-change-kind {
  flex: 0 0 48px;
  font-size: 12px;
  color: var(--muted);
}
.code-change-kind.conflict {
  color: #e24b4b;
}
.code-change-path {
  flex: 1;
  min-width: 0;
  font-size: 13px;
  overflow-wrap: anywhere;
}
.code-change-path small {
  display: block;
  margin-top: 5px;
  font-size: 11px;
}
@media (max-width: 700px) {
  .project-code-actions {
    margin-left: 0;
    width: 100%;
  }
  .code-size-summary {
    gap: 30px;
  }
}
</style>
