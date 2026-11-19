import { copyFile, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { execFileSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { trayIconSvg } from './lib/branding.mjs'

const root = fileURLToPath(new URL('..', import.meta.url))
const source = join(root, 'src-tauri/icons/tray.svg')
await writeFile(source, trayIconSvg())
const output = await mkdtemp(join(tmpdir(), 'worktodo-tray-'))
try {
  execFileSync(
    process.execPath,
    [join(root, 'scripts/desktop.mjs'), 'icon', source, '--output', output, '--png', '36'],
    { cwd: root, stdio: 'inherit' },
  )
  await copyFile(join(output, '36x36.png'), join(root, 'src-tauri/icons/tray.png'))
} finally {
  await rm(output, { recursive: true, force: true })
}
