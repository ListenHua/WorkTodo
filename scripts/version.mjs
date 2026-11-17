import { execFileSync } from 'node:child_process'
import { writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import {
  prepareVersionChanges,
  readRepository,
  renderChangelog,
  replaceFiles,
  root,
} from './lib/release.mjs'

try {
  const [command, target, ...extra] = process.argv.slice(2)
  if (extra.length || (command !== 'prepare' && target)) throw new Error('存在未知参数')
  if (command === 'check') {
    const { version } = await readRepository()
    console.log(`版本与更新日志校验通过：${version}`)
  } else if (command === 'generate') {
    const { log } = await readRepository(root, { checkGenerated: false })
    await writeFile(join(root, 'CHANGELOG.md'), renderChangelog(log))
    console.log('已生成 CHANGELOG.md')
  } else if (command === 'prepare') {
    const repository = await readRepository(root, {
      requireCurrent: false,
      checkGenerated: false,
    })
    const { version } = repository
    const changes = prepareVersionChanges(repository, target)
    execFileSync(process.execPath, [join(root, 'scripts/desktop.mjs'), 'cargo', '--version'], {
      cwd: root,
      stdio: 'pipe',
    })
    await replaceFiles(root, changes, async () => {
      // Let Cargo validate the synchronized package record without resolving new dependencies.
      execFileSync(
        process.execPath,
        [
          join(root, 'scripts/desktop.mjs'),
          'cargo',
          'metadata',
          '--manifest-path',
          'src-tauri/Cargo.toml',
          '--no-deps',
          '--format-version',
          '1',
          '--locked',
          '--offline',
        ],
        { cwd: root, stdio: 'pipe' },
      )
      await readRepository()
    })
    console.log(
      `已准备 ${version} → ${target}：\n${Object.keys(changes)
        .map((name) => `  ${name}`)
        .join('\n')}\n未提交、创建标签或发布。请审阅日志日期与内容。`,
    )
  } else
    throw new Error(
      '用法：pnpm version:check | pnpm changelog:generate | pnpm version:prepare 1.0.1',
    )
} catch (error) {
  console.error(error.message)
  process.exitCode = 1
}
