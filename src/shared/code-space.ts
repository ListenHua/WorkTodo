export interface GitChange {
  path: string
  originalPath: string | null
  index: string
  worktree: string
  conflict: boolean
}
export interface GitRemote {
  name: string
  fetchUrl: string
  pushUrl: string
}
export interface GitInfo {
  state: 'ready' | 'none' | 'unavailable' | 'preview' | 'missing'
  message: string
  branch: string
  scope: string
  upstream: string
  ahead: number
  behind: number
  changes: GitChange[]
  totalChanges: number
  conflicts: number
  truncated: boolean
  remotes: GitRemote[]
  commit: { hash: string; subject: string; time: string } | null
}
export interface Dependency {
  path: string
  bytes: number
  fingerprint: string
}
export interface SpaceUsage {
  bytes: number
  dependencyBytes: number
  files: number
  dependencies: Dependency[]
  directories: { name: string; bytes: number }[]
  warnings: string[]
  scannedAt: string
}
export interface CodeProgress {
  files: number
  bytes: number
  current: string
}
export interface CleanResult {
  succeeded: string[]
  failed: { path: string; message: string }[]
}
export function formatCodeSize(bytes: number) {
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let index = 0
  while (bytes >= 1024 && index < units.length - 1) {
    bytes /= 1024
    index++
  }
  return `${index ? bytes.toFixed(1) : bytes} ${units[index]}`
}
export function codeDate(value: string | undefined) {
  if (!value || !Number.isFinite(new Date(value).getTime())) return '—'
  return new Date(value).toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
}
export function gitLabel(info: GitInfo | null) {
  if (!info) return '待读取'
  if (info.state === 'missing') return '目录不可用'
  if (info.state === 'none') return '非 Git 项目'
  if (info.state === 'unavailable') return 'Git 不可用'
  if (info.state === 'preview') return '桌面端可用'
  if (info.conflicts) return `存在 ${info.conflicts} 项冲突`
  return info.totalChanges ? `${info.totalChanges} 项变动` : '无变动'
}
export function gitChangeLabel(change: GitChange) {
  if (change.conflict) return '冲突'
  const codes = change.index + change.worktree
  if (codes.includes('?')) return '未跟踪'
  if (codes.includes('R')) return '重命名'
  if (codes.includes('D')) return '删除'
  if (codes.includes('A')) return '新增'
  if (codes.includes('C')) return '复制'
  return '修改'
}
export function remoteWebUrl(raw: string): string | null {
  let value = raw.trim()
  if (!value || /[\s\\]/u.test(value)) return null
  if (/^ssh:\/\//i.test(value)) {
    try {
      const url = new URL(value)
      if (url.port && url.port !== '22') return null
      value = `https://${url.hostname}${url.pathname}`
    } catch {
      return null
    }
  } else if (!/^https?:\/\//i.test(value)) {
    const match = /^(?:[^/@:]+@)?([^/:]+):(.+)$/.exec(value)
    if (!match || !match[1]!.includes('.')) return null
    value = `https://${match[1]}/${match[2]}`
  }
  try {
    const url = new URL(value)
    if (!['https:', 'http:'].includes(url.protocol) || url.username || url.password) return null
    url.pathname = url.pathname.replace(/\.git\/?$/, '')
    url.search = ''
    url.hash = ''
    return url.href
  } catch {
    return null
  }
}
