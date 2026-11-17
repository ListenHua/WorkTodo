import type { ProjectTag } from './types'

export function projectTagKey(name: string): string {
  return name.trim().normalize('NFC').toLowerCase()
}

export function validateProjectTags(tags: ProjectTag[]): ProjectTag[] {
  if (tags.length > 20) throw new Error('每个项目最多添加 20 个标签')
  const names = new Set<string>()
  return tags.map((tag) => {
    const name = tag.name.trim().normalize('NFC')
    const key = projectTagKey(name)
    if (!name || [...name].length > 30 || /\p{Cc}/u.test(name))
      throw new Error('标签名称需为 1–30 字，且不能包含控制字符')
    if (names.has(key)) throw new Error('项目标签名称不能重复')
    if (!/^#[a-f\d]{6}$/i.test(tag.color)) throw new Error('请选择有效的标签颜色')
    names.add(key)
    return { name, color: tag.color.toUpperCase() }
  })
}
