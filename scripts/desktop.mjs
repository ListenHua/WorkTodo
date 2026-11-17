import { spawn, spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { homedir } from 'node:os'
import { delimiter, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../', import.meta.url))
// Resolve the standard rustup location without changing the user's shell profile.
const cargoBin = join(homedir(), '.cargo', 'bin')
const env = { ...process.env, PATH: [cargoBin, process.env.PATH].filter(Boolean).join(delimiter) }
const [command, ...args] = process.argv.slice(2)
if (!command) {
  console.error('用法：pnpm tauri dev | build | info')
  process.exit(1)
}
if (
  ['dev', 'build', 'cargo', 'info'].includes(command) &&
  spawnSync('cargo', ['--version'], { env, stdio: 'ignore' }).status !== 0
) {
  console.error('未找到可用的 Rust 工具链。请先安装 Rust stable，并确保 rustup 和 cargo 可用。')
  process.exit(1)
}
const cli = join(root, 'node_modules', '@tauri-apps', 'cli', 'tauri.js')
if (command !== 'cargo' && !existsSync(cli)) {
  console.error('缺少 Tauri CLI，请先运行 pnpm install。')
  process.exit(1)
}
const child =
  command === 'cargo'
    ? spawn('cargo', args, { cwd: root, env, stdio: 'inherit' })
    : spawn(process.execPath, [cli, command, ...args], { cwd: root, env, stdio: 'inherit' })
child.on('error', (error) => {
  console.error(error.message)
  process.exit(1)
})
child.on('exit', (code) => process.exit(code ?? 1))
