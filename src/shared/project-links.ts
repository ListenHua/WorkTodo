import type { ProjectLink } from './types'
export const linkIcons = [
  { label: '默认', value: '' },
  { label: 'GitHub', value: 'github' },
  { label: 'GitLab', value: 'gitlab' },
  { label: 'Gitee', value: 'gitee' },
] as const
export function normalizeLinkUrl(raw: string): string {
  let value = raw.trim()
  if (!value || /[\s\\]/u.test(value)) throw new Error('请输入有效的网站地址')
  const bare = !/^https?:\/\//i.test(value)
  if (value.startsWith('//')) value = `https:${value}`
  else if (bare) {
    const hasScheme = /^[a-z][a-z\d+.-]*:/i.test(value)
    const hostWithPort = /^[^/?#:\s]+:\d+(?:[/?#]|$)/.test(value)
    if (hasScheme && !hostWithPort) throw new Error('链接仅支持 http 或 https')
    value = `https://${value}`
  }
  let url: URL
  try {
    url = new URL(value)
  } catch {
    throw new Error('请输入有效的网站地址')
  }
  if (
    value.length > 2048 ||
    !['http:', 'https:'].includes(url.protocol) ||
    !url.hostname ||
    url.username ||
    url.password
  )
    throw new Error('链接仅支持 http 或 https，且不能包含账号密码')
  if (bare && !raw.trim().startsWith('//') && isLocalHost(url.hostname)) url.protocol = 'http:'
  return url.href
}
function isLocalHost(host: string) {
  return (
    host === 'localhost' ||
    host.endsWith('.localhost') ||
    host.endsWith('.local') ||
    host === '[::1]' ||
    /^(127\.|10\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.)/.test(host)
  )
}
export function validIconData(value: string | undefined): value is string {
  return (
    !!value &&
    value.length <= 350000 &&
    /^data:image\/(png|x-icon|jpeg|gif|webp|svg\+xml);base64,[a-z\d+/]+={0,2}$/i.test(value)
  )
}
export function validateProjectLinks(links: ProjectLink[]): ProjectLink[] {
  if (links.length > 50) throw new Error('每个项目最多保存 50 个链接')
  const ids = new Set<string>()
  return links.map((link) => {
    const name = link.name.trim()
    if (
      !link.id ||
      link.id.length > 100 ||
      ids.has(link.id) ||
      !name ||
      [...name].length > 80 ||
      !(link.icon === 'website' || linkIcons.some((icon) => icon.value === link.icon))
    )
      throw new Error('链接名称、图标或标识无效')
    if (link.icon === 'website' && !validIconData(link.iconData))
      throw new Error('请重新提取网站图标')
    ids.add(link.id)
    const { iconData, ...rest } = link
    return {
      ...rest,
      name,
      url: normalizeLinkUrl(link.url),
      ...(link.icon === 'website' ? { iconData } : {}),
    }
  })
}

export function defaultLinkName(icon: ProjectLink['icon'], url: string): string {
  const preset = linkIcons.find((option) => option.value === icon && option.value)
  if (preset) return preset.label
  try {
    return new URL(normalizeLinkUrl(url)).hostname.replace(/^www\./i, '')
  } catch {
    return ''
  }
}
