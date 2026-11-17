import { invoke } from '@tauri-apps/api/core'
import { desktop } from './api'
import { preferences } from './preferences'
import { normalizeLinkUrl } from '../shared/project-links'
export async function openProjectLink(raw: string) {
  const url = normalizeLinkUrl(raw)
  if (desktop) await invoke('projects_open_link', { url })
  else window.open(url, '_blank', 'noopener,noreferrer')
}

export async function extractProjectLinkIcon(raw: string): Promise<string> {
  const url = normalizeLinkUrl(raw)
  if (desktop) return invoke<string>('projects_extract_link_icon', { url })
  if ((await preferences.get('strict_offline')) === 'true')
    throw new Error('严格离线已开启，请使用品牌图标或名称首字')
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), 12000)
  async function read(address: string, limit: number) {
    const response = await fetch(address, {
      signal: controller.signal,
      credentials: 'omit',
      referrerPolicy: 'no-referrer',
    })
    if (!response.ok || Number(response.headers.get('content-length')) > limit)
      throw new Error('无法读取网站图标')
    const reader = response.body?.getReader()
    if (!reader) throw new Error('无法读取网站图标')
    const chunks: Uint8Array[] = []
    let size = 0
    try {
      while (true) {
        const { done, value } = await reader.read()
        if (done) break
        size += value.length
        if (size > limit) throw new Error('网站图标过大')
        chunks.push(value)
      }
    } finally {
      await reader.cancel()
    }
    const bytes = new Uint8Array(size)
    let offset = 0
    for (const chunk of chunks) {
      bytes.set(chunk, offset)
      offset += chunk.length
    }
    return {
      bytes,
      url: response.url || address,
      type: response.headers.get('content-type')?.split(';')[0] || '',
    }
  }
  try {
    const candidates: string[] = []
    let pageUrl = url
    try {
      const page = await read(url, 512 * 1024)
      pageUrl = page.url
      const html = new DOMParser().parseFromString(
        new TextDecoder().decode(page.bytes),
        'text/html',
      )
      const base = new URL(
        html.querySelector('base[href]')?.getAttribute('href') || pageUrl,
        pageUrl,
      )
      for (const element of html.querySelectorAll('link[rel][href]')) {
        if (
          !element
            .getAttribute('rel')
            ?.split(/\s+/)
            .some((rel) => ['icon', 'apple-touch-icon'].includes(rel.toLowerCase()))
        )
          continue
        try {
          candidates.push(normalizeLinkUrl(new URL(element.getAttribute('href')!, base).href))
        } catch {
          /* skip unsupported icon addresses */
        }
        if (candidates.length === 4) break
      }
    } catch {
      /* try conventional icon locations */
    }
    candidates.push(
      new URL('/favicon.ico', pageUrl).href,
      new URL('/apple-touch-icon.png', pageUrl).href,
    )
    for (const candidate of new Set(candidates)) {
      if (controller.signal.aborted) break
      try {
        const { bytes, type } = await read(candidate, 256 * 1024)
        const mime = type === 'image/vnd.microsoft.icon' ? 'image/x-icon' : type
        if (
          ![
            'image/png',
            'image/x-icon',
            'image/jpeg',
            'image/gif',
            'image/webp',
            'image/svg+xml',
          ].includes(mime)
        )
          continue
        const encoded = btoa(Array.from(bytes, (byte) => String.fromCharCode(byte)).join(''))
        return `data:${mime};base64,${encoded}`
      } catch {
        /* try the next icon */
      }
    }
    throw new Error('未获取到网站图标，可使用默认图标或稍后重试')
  } finally {
    clearTimeout(timeout)
  }
}
