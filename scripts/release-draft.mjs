import { resolve } from 'node:path'
import { stageDraft } from './lib/packages.mjs'
try {
  const [input, output, ...extra] = process.argv.slice(2)
  if (!input || !output || extra.length)
    throw new Error('用法：node scripts/release-draft.mjs 下载目录 草稿输出目录')
  const result = await stageDraft(resolve(input), resolve(output))
  console.log(`草稿资源已校验：v${result.version}（${result.releaseType}）→ ${resolve(output)}`)
} catch (error) {
  console.error(error.message)
  process.exitCode = 1
}
