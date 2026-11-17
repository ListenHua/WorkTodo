import { copyFile, mkdir, mkdtemp, readdir, rm, writeFile } from 'node:fs/promises'
import { execFileSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createSSRApp, h } from 'vue'
import { renderToString } from 'vue/server-renderer'
import FolderCheckIcon from '@mingcute/vue/core-regular/folder-check'

// MingCute artwork is Apache-2.0 licensed; see public/licenses/mingcute.txt.
const root = fileURLToPath(new URL('..', import.meta.url))
const icon = await renderToString(
  createSSRApp({
    render: () => h(FolderCheckIcon, { size: 320, color: '#FFFFFF', x: 96, y: 96 }),
  }),
)
const artwork = `<rect width="512" height="512" rx="116" fill="#1677FF"/>
  ${icon.replace(/<!--.*?-->/g, '')}`
const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 512 512">
  <!-- MingCute folder-check, Apache-2.0; background and layout adapted for this app. -->
  ${artwork}
</svg>
`
// Keep the background and glyph together, with 50 px of transparent space on
// every side. Only the macOS ICNS uses this inset; the tray, favicon and Windows
// resources retain their existing size.
const macosSvg = svg.replace(
  artwork,
  `<g transform="translate(50 50) scale(0.8046875)">${artwork}</g>`,
)
const destination = join(root, 'src-tauri/icons')
await mkdir(destination, { recursive: true })
await writeFile(join(root, 'public/app-icon.svg'), svg)
await writeFile(join(destination, 'icon-macos.svg'), macosSvg)
const output = await mkdtemp(join(tmpdir(), 'project-manage-icons-'))
try {
  const generate = (source, outputDirectory) =>
    execFileSync(
      process.execPath,
      [join(root, 'scripts/desktop.mjs'), 'icon', source, '--output', outputDirectory],
      { cwd: root, stdio: 'inherit' },
    )
  const desktopOutput = join(output, 'desktop')
  const macosOutput = join(output, 'macos')
  generate('public/app-icon.svg', desktopOutput)
  generate('src-tauri/icons/icon-macos.svg', macosOutput)
  // Only desktop resources are used by this project.
  for (const entry of await readdir(desktopOutput, { withFileTypes: true })) {
    if (entry.isFile() && entry.name !== 'icon.icns') {
      await copyFile(join(desktopOutput, entry.name), join(destination, entry.name))
    }
  }
  await copyFile(join(macosOutput, 'icon.icns'), join(destination, 'icon.icns'))
} finally {
  await rm(output, { recursive: true, force: true })
}
