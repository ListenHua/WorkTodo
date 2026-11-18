import { execFileSync } from 'node:child_process'
import { readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

export const root = fileURLToPath(new URL('../../', import.meta.url))
export const categories = {
  added: '新增',
  improved: '优化',
  fixed: '修复',
  breaking: '重要变更',
  security: '安全',
}
export const emptyChanges = () =>
  Object.fromEntries(Object.keys(categories).map((key) => [key, []]))
export function parseVersion(version) {
  if (typeof version !== 'string' || !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version))
    throw new Error(`无效版本 ${version}：首期仅支持三段数字正式版本，如 1.0.1`)
  const parts = version.split('.').map(Number)
  if (
    parts.some((value) => !Number.isSafeInteger(value)) ||
    parts[0] > 255 ||
    parts[1] > 255 ||
    parts[2] > 65535
  )
    throw new Error('版本超出安装器范围：主/次版本不超过 255，修订版本不超过 65535')
  return parts
}
export function compareVersions(a, b) {
  const left = parseVersion(a),
    right = parseVersion(b)
  for (let index = 0; index < 3; index++) {
    if (left[index] !== right[index]) return Math.sign(left[index] - right[index])
  }
  return 0
}
export function today() {
  return new Intl.DateTimeFormat('en-CA', {
    timeZone: 'Asia/Shanghai',
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  }).format(new Date())
}
function validateDate(value) {
  if (
    typeof value !== 'string' ||
    !/^\d{4}-\d{2}-\d{2}$/.test(value) ||
    !Number.isFinite(Date.parse(value)) ||
    new Date(value).toISOString().slice(0, 10) !== value ||
    value > today()
  )
    throw new Error(`发布日期无效或晚于今天：${value}`)
}
function validateChanges(entry, released) {
  if (!entry || typeof entry !== 'object' || Array.isArray(entry))
    throw new Error('日志条目必须为对象')
  const allowed = [...Object.keys(categories), ...(released ? ['version', 'date', 'notes'] : [])]
  if (Object.keys(entry).some((key) => !allowed.includes(key))) throw new Error('日志包含未知字段')
  for (const key of [
    ...Object.keys(categories),
    ...(released && entry.notes !== undefined ? ['notes'] : []),
  ]) {
    if (
      !Array.isArray(entry[key]) ||
      entry[key].some((line) => typeof line !== 'string' || !line.trim() || /[\r\n]/.test(line))
    )
      throw new Error(`日志 ${key} 必须为非空单行文字数组`)
  }
  if (released && !Object.keys(categories).some((key) => entry[key].length))
    throw new Error('版本日志不能为空')
}
export function validateChangelog(log) {
  if (
    !log ||
    Object.keys(log).some((key) => !['unreleased', 'releases'].includes(key)) ||
    !Array.isArray(log.releases)
  )
    throw new Error('更新日志结构无效')
  validateChanges(log.unreleased, false)
  let previous
  for (const entry of log.releases) {
    validateChanges(entry, true)
    parseVersion(entry.version)
    validateDate(entry.date)
    if (previous && compareVersions(previous, entry.version) <= 0)
      throw new Error('日志版本必须唯一并按版本从新到旧排列')
    previous = entry.version
  }
  return log
}
const plain = (text) => text.replace(/[\\`*_{}\[\]<>]/g, '\\$&')
export function releaseBody(entry, { initial = false } = {}) {
  const sections = Object.entries(categories)
    .filter(([key]) => entry[key].length)
    .map(
      ([key, label]) =>
        `### ${initial && key === 'added' ? '主要功能' : label}\n\n${entry[key].map((line) => `- ${plain(line)}`).join('\n')}`,
    )
  if (entry.notes?.length)
    sections.push(`### 说明\n\n${entry.notes.map((line) => `- ${plain(line)}`).join('\n')}`)
  return sections.join('\n\n')
}
export function renderChangelog(log) {
  const pending = Object.keys(categories).some((key) => log.unreleased[key].length)
  return `# 更新日志\n\n此文件由 changelog.json 生成，请修改数据后运行 pnpm changelog:generate。条目记录对应构建的变化，正式发布状态以发布页面为准。\n\n## 未发布\n\n${pending ? releaseBody(log.unreleased) : '暂无待发布变更。'}\n${log.releases.map((entry) => `\n## ${entry.version} · ${entry.date}\n\n${releaseBody(entry, { initial: entry === log.releases.at(-1) })}\n`).join('')}`
}
export async function readRepository(
  directory = root,
  { requireCurrent = true, checkGenerated = true } = {},
) {
  const names = [
    'package.json',
    'src-tauri/tauri.conf.json',
    'src-tauri/Cargo.toml',
    'src-tauri/Cargo.lock',
    'changelog.json',
  ]
  const texts = Object.fromEntries(
    await Promise.all(
      names.map(async (name) => [name, await readFile(join(directory, name), 'utf8')]),
    ),
  )
  const pkg = JSON.parse(texts['package.json'])
  const version = pkg.version
  parseVersion(version)
  const cargo = texts['src-tauri/Cargo.toml']
    .match(/\[package\]([\s\S]*?)(?=\n\[|$)/)?.[1]
    .match(/^version\s*=\s*"([^"]+)"/m)?.[1]
  const locked = texts['src-tauri/Cargo.lock'].match(
    /\[\[package\]\]\r?\nname = "worktodo"\r?\nversion = "([^"]+)"/,
  )?.[1]
  if (
    [JSON.parse(texts['src-tauri/tauri.conf.json']).version, cargo, locked].some(
      (value) => value !== version,
    )
  )
    throw new Error('package.json、Tauri、Cargo 和 Cargo.lock 的应用版本不一致')
  const log = validateChangelog(JSON.parse(texts['changelog.json']))
  const current = log.releases.find((entry) => entry.version === version)
  if (requireCurrent && (!current || log.releases[0] !== current))
    throw new Error(`缺少当前版本 ${version} 的最新更新日志`)
  if (
    checkGenerated &&
    (await readFile(join(directory, 'CHANGELOG.md'), 'utf8')).replace(/\r\n/g, '\n') !==
      renderChangelog(log)
  )
    throw new Error('CHANGELOG.md 已过期，请运行 pnpm changelog:generate')
  return { version, log, current, texts }
}
export function gitInfo(directory = root) {
  try {
    const revision = execFileSync('git', ['rev-parse', 'HEAD'], {
      cwd: directory,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim()
    const dirty = !!execFileSync('git', ['status', '--porcelain', '--untracked-files=normal'], {
      cwd: directory,
      encoding: 'utf8',
    }).trim()
    return { revision, dirty }
  } catch {
    return { revision: null, dirty: true }
  }
}
// Restore all original files if any write or Cargo validation fails.
export async function replaceFiles(directory, changes, verify = async () => {}) {
  const originals = new Map()
  for (const name of Object.keys(changes)) {
    try {
      originals.set(name, await readFile(join(directory, name)))
    } catch (error) {
      if (error.code !== 'ENOENT') throw error
      originals.set(name, null)
    }
  }
  try {
    for (const [name, text] of Object.entries(changes)) await writeFile(join(directory, name), text)
    await verify()
  } catch (error) {
    const { unlink } = await import('node:fs/promises')
    const failures = await Promise.allSettled(
      [...originals].map(([name, bytes]) =>
        bytes === null
          ? unlink(join(directory, name)).catch((failure) => {
              if (failure.code !== 'ENOENT') throw failure
            })
          : writeFile(join(directory, name), bytes),
      ),
    )
    if (failures.some((result) => result.status === 'rejected'))
      throw new Error(`版本写入失败且部分文件无法回退，请检查文件权限：${error.message}`)
    throw error
  }
}

export function prepareVersionChanges(repository, target) {
  const { version, texts } = repository
  const log = structuredClone(repository.log)
  if (compareVersions(target, version) <= 0) throw new Error('新版本必须高于当前版本')
  if (log.releases.some((entry) => compareVersions(target, entry.version) <= 0))
    throw new Error('目标版本必须高于所有已有日志版本')
  if (!Object.values(log.unreleased).some((items) => items.length))
    throw new Error('请先在 changelog.json 的 unreleased 中填写本次变更')
  log.releases.unshift({ version: target, date: today(), ...log.unreleased })
  log.unreleased = emptyChanges()
  validateChangelog(log)
  const pkg = JSON.parse(texts['package.json']),
    config = JSON.parse(texts['src-tauri/tauri.conf.json'])
  pkg.version = config.version = target
  return {
    'package.json': JSON.stringify(pkg, null, 2) + '\n',
    'src-tauri/tauri.conf.json': JSON.stringify(config, null, 2) + '\n',
    'src-tauri/Cargo.toml': texts['src-tauri/Cargo.toml'].replace(
      /(\[package\][\s\S]*?^version\s*=\s*")[^"]+(".*$)/m,
      (_, prefix, suffix) => `${prefix}${target}${suffix}`,
    ),
    'src-tauri/Cargo.lock': texts['src-tauri/Cargo.lock'].replace(
      /(\[\[package\]\]\r?\nname = "worktodo"\r?\nversion = ")[^"]+(".*$)/m,
      (_, prefix, suffix) => `${prefix}${target}${suffix}`,
    ),
    'changelog.json': JSON.stringify(log, null, 2) + '\n',
    'CHANGELOG.md': renderChangelog(log),
  }
}
