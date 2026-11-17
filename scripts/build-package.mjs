import { execFileSync, spawn } from 'node:child_process'
import { mkdir, rename, rm, writeFile } from 'node:fs/promises'
import { homedir } from 'node:os'
import { delimiter, dirname, join, resolve } from 'node:path'
import { randomUUID } from 'node:crypto'
import { gitInfo, readRepository, root } from './lib/release.mjs'
import { installerFiles, reportPackages, targetInfo } from './lib/packages.mjs'
import { verifyMacApplication } from './lib/macos-signing.mjs'

const args = process.argv.slice(2)
function option(name) {
  const index = args.indexOf(name)
  if (index < 0) return undefined
  if (!args[index + 1] || args[index + 1].startsWith('--') || args.lastIndexOf(name) !== index)
    throw new Error(`参数 ${name} 无效`)
  return args[index + 1]
}
let lock,
  previous,
  bundleRoot,
  started = false
try {
  const allowed = new Map([
    ['--target', 1],
    ['--bundles', 1],
    ['--runner', 1],
    ['--release-type', 1],
    ['--no-sign', 0],
    ['--ci', 0],
  ])
  for (let index = 0; index < args.length; index++) {
    if (!allowed.has(args[index])) throw new Error(`未知构建参数：${args[index]}`)
    if (allowed.get(args[index])) option(args[index])
    index += allowed.get(args[index])
  }
  const repository = await readRepository()
  const source = gitInfo()
  const env = {
    ...process.env,
    PATH: [join(homedir(), '.cargo/bin'), process.env.PATH].filter(Boolean).join(delimiter),
  }
  const explicitTarget = option('--target')
  const target =
    explicitTarget ??
    execFileSync('rustc', ['-vV'], { env, encoding: 'utf8' }).match(/^host:\s*(\S+)/m)?.[1]
  const { os } = targetInfo(target)
  const releaseType = option('--release-type') ?? 'internal'
  if (!['internal', 'stable'].includes(releaseType))
    throw new Error('发布类型仅支持 internal 或 stable')
  if (releaseType === 'stable' && (!source.revision || source.dirty || args.includes('--no-sign')))
    throw new Error('正式构建需要已提交且干净的工作区，并启用代码签名')
  // macOS still needs a sealed app bundle when no Developer ID is configured.
  // Let Tauri sign before creating the DMG so the mounted app has the same signature.
  if (os === 'macos' && releaseType === 'internal') {
    if (args.includes('--no-sign')) env.APPLE_SIGNING_IDENTITY = '-'
    else env.APPLE_SIGNING_IDENTITY ??= '-'
  }
  env.PROJECT_MANAGE_RELEASE_TYPE = releaseType
  const targetRoot = resolve(root, process.env.CARGO_TARGET_DIR ?? 'src-tauri/target')
  const profileRoot = join(targetRoot, ...(explicitTarget ? [target] : []), 'release')
  bundleRoot = join(profileRoot, 'bundle')
  await mkdir(profileRoot, { recursive: true })
  const lockPath = join(targetRoot, '.project-manage-package.lock')
  await mkdir(lockPath).catch((error) => {
    if (error.code === 'EEXIST')
      throw new Error(`已有打包任务或上次异常退出，请先检查：${lockPath}`)
    throw error
  })
  lock = lockPath
  const buildId = `${Date.now()}-${randomUUID().slice(0, 8)}`
  const history = join(profileRoot, 'package-history', buildId)
  try {
    await mkdir(history, { recursive: true })
    await rename(bundleRoot, join(history, 'bundle'))
    previous = join(history, 'bundle')
  } catch (error) {
    if (error.code !== 'ENOENT') throw error
  }
  const startedAt = new Date().toISOString()
  const forwarded = args.filter(
    (arg, index) =>
      arg !== '--release-type' &&
      args[index - 1] !== '--release-type' &&
      !(os === 'macos' && arg === '--no-sign'),
  )
  started = true
  const status = await new Promise((accept, reject) => {
    const child = spawn(
      process.execPath,
      [join(root, 'scripts/desktop.mjs'), 'build', ...forwarded],
      { cwd: root, env, stdio: 'inherit' },
    )
    child.once('error', reject)
    child.once('exit', (code, signal) => accept(signal ? 1 : (code ?? 1)))
  })
  if (status !== 0) throw new Error(`桌面构建失败（${status}）`)
  const after = gitInfo()
  if (after.revision !== source.revision || after.dirty !== source.dirty)
    throw new Error('构建期间源码状态变化，请重新构建')
  let signature = args.includes('--no-sign') ? 'skipped' : 'unverified'
  if (os === 'macos') {
    const { readdir } = await import('node:fs/promises')
    const apps = (await readdir(join(bundleRoot, 'macos'))).filter((name) => name.endsWith('.app'))
    if (apps.length !== 1) throw new Error('Mac 构建必须包含唯一 .app')
    signature = verifyMacApplication(join(bundleRoot, 'macos', apps[0]), {
      requireDeveloperId: releaseType === 'stable',
    })
    for (const dmg of (await installerFiles(bundleRoot)).filter((file) => file.endsWith('.dmg')))
      execFileSync('hdiutil', ['verify', dmg], { stdio: 'pipe' })
  } else if (releaseType === 'stable') {
    if (process.platform === 'win32') {
      const installers = (await installerFiles(bundleRoot)).filter((file) =>
        /\.(exe|msi)$/.test(file),
      )
      if (!installers.length) throw new Error('未找到正式 Windows 安装器')
      for (const installer of installers) {
        const result = execFileSync(
          'powershell.exe',
          [
            '-NoProfile',
            '-NonInteractive',
            '-Command',
            '$s=Get-AuthenticodeSignature -LiteralPath $env:PROJECT_MANAGE_PACKAGE_FILE; if($s.Status -ne "Valid"){exit 1}',
          ],
          { env: { ...env, PROJECT_MANAGE_PACKAGE_FILE: installer }, stdio: 'pipe' },
        )
        void result
      }
      signature = 'verified'
    } else throw new Error('正式 Windows 包请在 Windows runner 验证签名')
  }
  await writeFile(
    join(bundleRoot, '.build-context.json'),
    JSON.stringify(
      {
        version: repository.version,
        ...source,
        target,
        releaseType,
        signature,
        buildId,
        startedAt,
        runId: process.env.GITHUB_RUN_ID,
        attempt: process.env.GITHUB_RUN_ATTEMPT,
      },
      null,
      2,
    ) + '\n',
  )
  console.log(`\n已导出：${await reportPackages(bundleRoot)}`)
  if (previous) console.log(`上次构建保留于：${previous}`)
} catch (error) {
  console.error(error.message)
  if (previous) {
    try {
      if (started)
        await rename(bundleRoot, join(dirname(previous), 'failed-bundle')).catch((failure) => {
          if (failure.code !== 'ENOENT') throw failure
        })
      await rename(previous, bundleRoot)
      console.error('已恢复上次构建目录；失败产物保留在 package-history。')
    } catch (failure) {
      console.error(`上次产物仍保留于 ${previous}：${failure.message}`)
    }
  }
  process.exitCode = 1
} finally {
  if (lock) await rm(lock, { recursive: true })
}
