import { copyFile, mkdir, mkdtemp, readdir, rm, writeFile } from 'node:fs/promises'
import { execFileSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { appIconSvg, nativeAppIconSvg } from './lib/branding.mjs'

const root = fileURLToPath(new URL('..', import.meta.url))
const destination = join(root, 'src-tauri/icons')
await mkdir(destination, { recursive: true })
await writeFile(join(root, 'public/app-icon.svg'), appIconSvg())
await writeFile(join(root, 'public/app-icon-light.svg'), appIconSvg({ light: true }))
await writeFile(join(destination, 'icon.svg'), nativeAppIconSvg())
await writeFile(join(destination, 'icon-macos.svg'), nativeAppIconSvg({ macos: true }))
const output = await mkdtemp(join(tmpdir(), 'worktodo-icons-'))
try {
  const generate = (source, outputDirectory) =>
    execFileSync(
      process.execPath,
      [join(root, 'scripts/desktop.mjs'), 'icon', source, '--output', outputDirectory],
      { cwd: root, stdio: 'inherit' },
    )
  const desktopOutput = join(output, 'desktop')
  const macosOutput = join(output, 'macos')
  generate('src-tauri/icons/icon.svg', desktopOutput)
  generate('src-tauri/icons/icon-macos.svg', macosOutput)
  // Only desktop resources are used by this project.
  for (const entry of await readdir(desktopOutput, { withFileTypes: true })) {
    if (entry.isFile() && entry.name !== 'icon.icns') {
      await copyFile(join(desktopOutput, entry.name), join(destination, entry.name))
    }
  }
  await copyFile(join(macosOutput, 'icon.icns'), join(destination, 'icon.icns'))
  // In-app branding stays transparent independently of the native app tile.
  for (const variant of ['app-icon', 'app-icon-light']) {
    const variantOutput = join(output, variant)
    execFileSync(
      process.execPath,
      [
        join(root, 'scripts/desktop.mjs'),
        'icon',
        `public/${variant}.svg`,
        '--output',
        variantOutput,
        '--png',
        '512',
      ],
      { cwd: root, stdio: 'inherit' },
    )
    await copyFile(join(variantOutput, '512x512.png'), join(root, `public/${variant}.png`))
  }
} finally {
  await rm(output, { recursive: true, force: true })
}

await import('./generate-tray-icon.mjs')
