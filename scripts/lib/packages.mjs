import { createHash } from 'node:crypto'
import { createReadStream } from 'node:fs'
import { cp, lstat, mkdir, mkdtemp, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises'
import { basename, join, resolve } from 'node:path'
import { tmpdir } from 'node:os'
import { execFileSync } from 'node:child_process'
import { gitInfo, readRepository, releaseBody, root } from './release.mjs'

export function targetInfo(triple) {
  const targets = {
    'aarch64-apple-darwin': ['macos', 'aarch64'],
    'x86_64-apple-darwin': ['macos', 'x86_64'],
    'universal-apple-darwin': ['macos', 'universal'],
    'x86_64-pc-windows-msvc': ['windows', 'x86_64'],
    'aarch64-pc-windows-msvc': ['windows', 'aarch64'],
  }
  if (!Object.hasOwn(targets, triple)) throw new Error(`尚未支持的目标：${triple}`)
  const [os, arch] = targets[triple]
  return { triple, os, arch }
}
export async function checksum(file) {
  const hash = createHash('sha256')
  for await (const bytes of createReadStream(file)) hash.update(bytes)
  return hash.digest('hex')
}
export async function installerFiles(directory) {
  const files = []
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const file = join(directory, entry.name)
    if (entry.isDirectory() && entry.name.endsWith('.app')) files.push(file)
    else if (entry.isDirectory()) files.push(...(await installerFiles(file)))
    else if (entry.isFile() && /\.(dmg|exe|msi)$/.test(entry.name)) files.push(file)
  }
  return files.sort()
}
export async function reportPackages(bundleRoot, directory = root) {
  const context = JSON.parse(
    await readFile(join(bundleRoot, '.build-context.json'), 'utf8').catch(() => {
      throw new Error(
        '缺少本次构建上下文，请使用 pnpm package:build 或 pnpm build:mac 构建；不会直接扫描旧包生成报告',
      )
    }),
  )
  const repository = await readRepository(directory)
  const source = gitInfo(directory)
  if (
    repository.version !== context.version ||
    source.revision !== context.revision ||
    source.dirty !== context.dirty
  )
    throw new Error('源代码状态或版本已变化，请重新构建')
  const target = targetInfo(context.target)
  if (!['internal', 'stable'].includes(context.releaseType)) throw new Error('构建类型无效')
  if (!Number.isFinite(Date.parse(context.startedAt)) || !/^[a-zA-Z0-9-]+$/.test(context.buildId))
    throw new Error('构建上下文无效')
  if (
    context.releaseType === 'stable' &&
    (!source.revision || source.dirty || context.signature !== 'verified')
  )
    throw new Error('正式包需要干净的已提交源码及已验证签名')
  const files = await installerFiles(bundleRoot)
  if (!files.length) throw new Error('本次没有生成应用或安装包')
  const destinations = new Set()
  for (const file of files) {
    const info = await stat(file)
    if (info.mtimeMs < Date.parse(context.startedAt) - 2000)
      throw new Error(`发现旧产物：${basename(file)}`)
    const app = file.endsWith('.app')
    if (
      (target.os === 'windows' && !/\.(exe|msi)$/.test(file)) ||
      (target.os === 'macos' && !app && !file.endsWith('.dmg'))
    )
      throw new Error('安装包类型与目标系统不符')
    if (app) {
      const version = execFileSync(
        '/usr/libexec/PlistBuddy',
        ['-c', 'Print :CFBundleShortVersionString', join(file, 'Contents/Info.plist')],
        { encoding: 'utf8' },
      ).trim()
      if (version !== repository.version) throw new Error('应用包内版本与当前版本不符')
    } else if (
      !basename(file).includes(`_${repository.version}_`) &&
      !basename(file).includes(`_${repository.version}.`)
    )
      throw new Error(`安装包名称缺少当前版本：${basename(file)}`)
    const suffix = app
      ? '.app.zip'
      : file.endsWith('.exe')
        ? '-setup.exe'
        : file.endsWith('.msi')
          ? '.msi'
          : '.dmg'
    const name = `WorkTodo_${context.version}_${target.os}_${target.arch}${suffix}`
    if (destinations.has(name)) throw new Error('发现重复安装包，请检查输出目录')
    destinations.add(name)
  }
  const destination = resolve(
    directory,
    'artifacts',
    context.version,
    context.target,
    context.buildId,
  )
  // A fresh export folder prevents older installers from being included in a new report.
  await mkdir(resolve(destination, '..'), { recursive: true })
  try {
    await mkdir(destination)
  } catch (error) {
    if (error.code !== 'EEXIST') throw error
    const existing = JSON.parse(await readFile(join(destination, 'build-metadata.json'), 'utf8'))
    if (existing.buildId !== context.buildId) throw new Error('产物目录冲突')
    for (const item of existing.packages)
      if ((await checksum(join(destination, item.file))) !== item.sha256)
        throw new Error('已导出安装包校验失败，请重新构建')
    return destination
  }
  try {
    const packages = []
    let index = 0
    for (const file of files) {
      const name = [...destinations][index++]
      const output = join(destination, name)
      if (file.endsWith('.app'))
        execFileSync('/usr/bin/ditto', [
          '-c',
          '-k',
          '--sequesterRsrc',
          '--keepParent',
          file,
          output,
        ])
      else await cp(file, output, { errorOnExist: true, force: false })
      packages.push({
        file: name,
        bytes: (await stat(output)).size,
        sha256: await checksum(output),
      })
    }
    if (target.os === 'windows') {
      const executable = resolve(bundleRoot, '..', 'worktodo.exe')
      if (!(await readFile(executable)).subarray(0, 2).equals(Buffer.from('MZ')))
        throw new Error('Windows 应用程序不存在或不是有效的 EXE')
      const temporary = await mkdtemp(join(tmpdir(), 'worktodo-portable-'))
      try {
        const application = join(temporary, 'WorkTodo.exe')
        await cp(executable, application, { errorOnExist: true, force: false })
        const name = `WorkTodo_${context.version}_${target.os}_${target.arch}.app.zip`
        const output = join(destination, name)
        if (process.platform === 'win32')
          execFileSync(
            'powershell.exe',
            [
              '-NoProfile',
              '-NonInteractive',
              '-Command',
              "$ErrorActionPreference = 'Stop'; Compress-Archive -LiteralPath $env:WORKTODO_PORTABLE_SOURCE -DestinationPath $env:WORKTODO_PORTABLE_OUTPUT -CompressionLevel Optimal",
            ],
            {
              env: {
                ...process.env,
                WORKTODO_PORTABLE_SOURCE: application,
                WORKTODO_PORTABLE_OUTPUT: output,
              },
              stdio: 'pipe',
            },
          )
        else execFileSync('/usr/bin/ditto', ['-c', '-k', application, output])
        packages.push({
          file: name,
          bytes: (await stat(output)).size,
          sha256: await checksum(output),
        })
      } finally {
        await rm(temporary, { recursive: true, force: true })
      }
    }
    const schema = Number(
      (await readFile(join(directory, 'src-tauri/src/db.rs'), 'utf8')).match(
        /SCHEMA_VERSION:\s*i64\s*=\s*(\d+)/,
      )?.[1],
    )
    const backupFormat = Number(
      (await readFile(join(directory, 'src-tauri/src/portable/transfer.rs'), 'utf8')).match(
        /"format":\s*(\d+)/,
      )?.[1],
    )
    if (!schema || !backupFormat) throw new Error('无法读取数据库或备份版本')
    const metadata = {
      version: context.version,
      changelogVersion: repository.current.version,
      releaseType: context.releaseType,
      buildId: context.buildId,
      createdAt: new Date().toISOString(),
      startedAt: context.startedAt,
      host: { platform: process.platform, arch: process.arch },
      target,
      ...source,
      schemaVersion: schema,
      backupFormat,
      signature: context.signature,
      ci: { runId: context.runId ?? null, attempt: context.attempt ?? null },
      packages,
      acceptance: '构建产物校验；系统通知、全局采集及安装卸载仍需按发布验收文档真机验证',
    }
    await writeFile(
      join(destination, 'build-metadata.json'),
      JSON.stringify(metadata, null, 2) + '\n',
    )
    await writeFile(
      join(destination, 'SHA256SUMS.txt'),
      packages.map((item) => `${item.sha256}  ${item.file}`).join('\n') + '\n',
    )
    await writeFile(
      join(destination, 'release-notes.md'),
      `# v${context.version}\n\n${releaseBody(repository.current, { initial: repository.current === repository.log.releases.at(-1) })}\n`,
    )
    return destination
  } catch (error) {
    // Leave a failed export clearly identifiable instead of reporting partial success.
    await writeFile(join(destination, 'FAILED.txt'), error.message).catch(() => {})
    throw error
  }
}
export async function stageDraft(downloadRoot, output, directory = root) {
  const repository = await readRepository(directory)
  const source = gitInfo(directory)
  if (!source.revision || source.dirty) throw new Error('发布草稿需要干净的已提交源码')
  async function findReports(folder) {
    const reports = []
    for (const entry of await readdir(folder, { withFileTypes: true })) {
      const file = join(folder, entry.name)
      if (entry.isDirectory()) reports.push(...(await findReports(file)))
      else if (entry.isFile() && entry.name === 'build-metadata.json') reports.push(file)
    }
    return reports
  }
  const reports = await findReports(downloadRoot)
  if (reports.length !== 2) throw new Error('草稿需要且仅接受一份 macOS 和一份 Windows 构建报告')
  const targets = new Set(),
    assets = []
  let releaseType
  for (const report of reports) {
    const metadata = JSON.parse(await readFile(report, 'utf8'))
    if (
      metadata.version !== repository.version ||
      metadata.changelogVersion !== repository.version ||
      metadata.revision !== source.revision ||
      metadata.dirty !== false
    )
      throw new Error('包版本、日志或源提交不一致')
    const expected = targetInfo(metadata.target?.triple)
    if (
      !['aarch64-apple-darwin', 'x86_64-pc-windows-msvc'].includes(expected.triple) ||
      targets.has(expected.triple) ||
      expected.os !== metadata.target.os ||
      expected.arch !== metadata.target.arch
    )
      throw new Error('目标平台不符合首发双平台要求或存在重复')
    if (
      !['internal', 'stable'].includes(metadata.releaseType) ||
      (releaseType && releaseType !== metadata.releaseType)
    )
      throw new Error('两平台的发布类型不一致')
    if (expected.os === 'macos' && !['adhoc', 'verified'].includes(metadata.signature))
      throw new Error('Mac 构建未通过完整应用签名验证')
    if (metadata.releaseType === 'stable' && metadata.signature !== 'verified')
      throw new Error('正式构建未通过签名验证')
    releaseType = metadata.releaseType
    targets.add(expected.triple)
    if (!Array.isArray(metadata.packages) || !metadata.packages.length)
      throw new Error('报告缺少安装包')
    for (const item of metadata.packages) {
      if (
        typeof item.file !== 'string' ||
        basename(item.file) !== item.file ||
        item.file.includes('\\') ||
        !item.file.startsWith(`WorkTodo_${repository.version}_${expected.os}_${expected.arch}`) ||
        !(expected.os === 'macos'
          ? /\.(dmg|app\.zip)$/.test(item.file)
          : /\.(exe|msi|app\.zip)$/.test(item.file)) ||
        !Number.isSafeInteger(item.bytes) ||
        item.bytes <= 0 ||
        !/^[a-f0-9]{64}$/.test(item.sha256)
      )
        throw new Error('安装包记录无效或路径越界')
      const file = resolve(report, '..', item.file)
      if (
        !(await lstat(file)).isFile() ||
        (await stat(file)).size !== item.bytes ||
        (await checksum(file)) !== item.sha256
      )
        throw new Error('安装包内容与构建报告不一致')
      if (assets.some((asset) => asset.name === item.file)) throw new Error('安装包名称冲突')
      assets.push({ file, name: item.file })
    }
  }
  const required = [
    `WorkTodo_${repository.version}_macos_aarch64.dmg`,
    `WorkTodo_${repository.version}_macos_aarch64.app.zip`,
    `WorkTodo_${repository.version}_windows_x86_64-setup.exe`,
    `WorkTodo_${repository.version}_windows_x86_64.app.zip`,
  ]
  if (
    assets.length !== required.length ||
    required.some((name) => !assets.some((asset) => asset.name === name))
  )
    throw new Error('发布资源必须且仅包含 macOS、Windows 的安装包与各自的应用压缩包')
  await mkdir(output, { recursive: false })
  for (const asset of assets)
    await cp(asset.file, join(output, asset.name), { errorOnExist: true, force: false })
  await writeFile(
    join(output, 'release-notes.md'),
    `# v${repository.version}\n\n${releaseBody(repository.current, { initial: repository.current === repository.log.releases.at(-1) })}\n\n支持目标：macOS Apple Silicon、Windows x64。\n`,
  )
  return { version: repository.version, releaseType }
}
