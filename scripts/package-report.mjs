import { resolve } from 'node:path'
import { root } from './lib/release.mjs'
import { reportPackages } from './lib/packages.mjs'
try {
  const args = process.argv.slice(2)
  if (args.length && (args.length !== 2 || args[0] !== '--bundle-dir'))
    throw new Error('用法：pnpm package:report [--bundle-dir 目录]')
  const directory = await reportPackages(
    resolve(root, args[1] ?? 'src-tauri/target/release/bundle'),
  )
  console.log(`安装包、SHA-256、构建元数据及版本说明：${directory}`)
} catch (error) {
  console.error(error.message)
  process.exitCode = 1
}
