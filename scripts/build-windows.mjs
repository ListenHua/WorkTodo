import { spawn, spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { homedir } from 'node:os'
import { delimiter, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../', import.meta.url))
const llvmPaths = ['/opt/homebrew/opt/llvm/bin', '/usr/local/opt/llvm/bin'].filter(existsSync)
const env = {
  ...process.env,
  PATH: [join(homedir(), '.cargo', 'bin'), ...llvmPaths, process.env.PATH]
    .filter(Boolean)
    .join(delimiter),
}
if (process.platform !== 'win32') {
  const requirements = [
    ['cargo-xwin', ['--version'], 'cargo-xwin'],
    ['makensis', ['-VERSION'], 'NSIS（makensis）'],
    ['clang-cl', ['--version'], 'LLVM（clang-cl）'],
    ['llvm-rc', ['--version'], 'LLVM（llvm-rc）'],
  ]
  const missing = requirements
    .filter(([command, args]) => {
      const result = spawnSync(command, args, { env, stdio: 'ignore' })
      // llvm-rc may return a nonzero status for --version; finding the executable is sufficient here.
      return result.error?.code === 'ENOENT' || (command !== 'llvm-rc' && result.status !== 0)
    })
    .map(([, , label]) => label)
  const targets = spawnSync('rustup', ['target', 'list', '--installed'], { env, encoding: 'utf8' })
  if (!targets.stdout?.includes('x86_64-pc-windows-msvc'))
    missing.push('Rust target: x86_64-pc-windows-msvc')
  if (missing.length) {
    console.error(
      `Windows 交叉构建缺少：\n${missing.map((item) => `  - ${item}`).join('\n')}\n\n请安装上述交叉构建工具，或在安装了 MSVC Build Tools 的 Windows 电脑上构建。当前没有生成 Windows 安装包。`,
    )
    process.exit(1)
  }
}
const args =
  process.platform === 'win32'
    ? ['--bundles', 'nsis', ...process.argv.slice(2)]
    : [
        '--runner',
        'cargo-xwin',
        '--target',
        'x86_64-pc-windows-msvc',
        '--bundles',
        'nsis',
        ...process.argv.slice(2),
      ]
const child = spawn(process.execPath, [join(root, 'scripts', 'build-package.mjs'), ...args], {
  cwd: root,
  env,
  stdio: 'inherit',
})
child.on('error', (error) => {
  console.error(error.message)
  process.exit(1)
})
child.on('exit', (code) => process.exit(code ?? 1))
