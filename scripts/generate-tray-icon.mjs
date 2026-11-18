import { copyFile, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { execFileSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createSSRApp, h } from 'vue'
import { renderToString } from 'vue/server-renderer'
import FolderCheckIcon from '@mingcute/vue/core-regular/folder-check'

// A separate transparent, monochrome MingCute glyph for the system tray.
// Keep the application and installer icons unchanged.
const root = fileURLToPath(new URL('..', import.meta.url))
const svg = await renderToString(
  createSSRApp({ render: () => h(FolderCheckIcon, { size: 36, color: '#FFFFFF' }) }),
)
const source = join(root, 'src-tauri/icons/tray.svg')
await writeFile(source, `${svg.replace(/<!--.*?-->/g, '')}\n`)
const output = await mkdtemp(join(tmpdir(), 'project-manage-tray-'))
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
