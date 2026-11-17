import { spawnSync } from 'node:child_process'
import { join } from 'node:path'
import { createRequire } from 'node:module'
import { root } from './lib/release.mjs'
const require = createRequire(import.meta.url)
const commands = [
  [join(root, 'scripts/version.mjs'), 'check'],
  [
    join(require.resolve('prettier/package.json'), '..', 'bin/prettier.cjs'),
    '--check',
    'src',
    'scripts',
    '.github',
    '*.ts',
    '*.json',
    'index.html',
    'src-tauri/*.json',
    'src-tauri/capabilities/*.json',
  ],
]
// pnpm.cmd needs a shell on Windows; invoke pnpm's Node entry point instead.
const pnpmEntry = process.env.npm_execpath
if (!pnpmEntry) {
  console.error('请通过 pnpm release:check 执行')
  process.exit(1)
}
for (const task of ['check', 'rust:fmt', 'rust:lint', 'rust:check']) {
  commands.push([pnpmEntry, task, ...(task === 'rust:fmt' ? ['--', '--check'] : [])])
}
for (const command of commands) {
  console.log(`\n执行：${command.slice(1).join(' ')}`)
  const result = spawnSync(process.execPath, command, { cwd: root, stdio: 'inherit' })
  if (result.error || result.status !== 0) {
    console.error(result.error?.message ?? '检查失败')
    process.exit(result.status || 1)
  }
}
console.log('自动检查通过；双平台人工验收仍需单独记录。')
