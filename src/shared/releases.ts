import changelog from '../../changelog.json'

export const releaseCategories = [
  ['added', '新增'],
  ['improved', '优化'],
  ['fixed', '修复'],
  ['breaking', '重要变更'],
  ['security', '安全'],
] as const
export function releaseCategoriesFor(version: string) {
  return releaseCategories.map(
    ([key, label]) =>
      [
        key,
        version === changelog.releases.at(-1)?.version && key === 'added' ? '主要功能' : label,
      ] as const,
  )
}
export type Release = (typeof changelog.releases)[number]
export const releases = changelog.releases
export function compareReleaseVersions(a: string, b: string): number | null {
  const pattern = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/
  if (!pattern.test(a) || !pattern.test(b)) return null
  const left = a.split('.').map(Number),
    right = b.split('.').map(Number)
  if ([...left, ...right].some((part) => !Number.isSafeInteger(part))) return null
  for (let index = 0; index < 3; index++) {
    if (left[index] !== right[index]) return Math.sign(left[index]! - right[index]!)
  }
  return 0
}
export function hasUnreadRelease(version: string, seen: string | null) {
  return (
    seen !== null &&
    releases.some((entry) => entry.version === version) &&
    compareReleaseVersions(version, seen) === 1
  )
}
