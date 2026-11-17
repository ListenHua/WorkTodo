import { execFileSync, spawnSync } from 'node:child_process'

export function verifyMacApplication(app, { requireDeveloperId = false } = {}) {
  try {
    execFileSync('codesign', ['--verify', '--deep', '--strict', app], { stdio: 'pipe' })
  } catch (error) {
    throw new Error(`Mac 应用完整性校验失败：${app}\n${error.stderr?.toString() ?? error.message}`)
  }
  const inspection = spawnSync('codesign', ['-d', '--verbose=4', app], { encoding: 'utf8' })
  if (inspection.error || inspection.status !== 0) throw new Error('无法读取 Mac 应用签名')
  const signature = inspection.stderr
  if (signature.includes('Authority=Developer ID Application:')) return 'verified'
  if (requireDeveloperId) throw new Error('正式 Mac 构建必须使用 Developer ID 签名')
  if (signature.includes('Signature=adhoc') && !signature.includes('linker-signed')) return 'adhoc'
  throw new Error('Mac 应用需要完整的 ad-hoc 或 Developer ID 签名')
}
