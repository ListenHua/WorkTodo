import { readFile } from 'node:fs/promises'

// All color variants and tray artwork share the approved W/check contour.
const source = await readFile(new URL('../assets/worktodo-mark.svg', import.meta.url), 'utf8')
const markPath = source.match(/<path d="([^"]+)"/)?.[1]
if (!markPath) throw new Error('WorkTodo logo contour is missing')

export function appIconSvg({ light = false } = {}) {
  const foreground = light ? '#111111' : '#ffffff'
  const artwork = `<path d="${markPath}" fill="${foreground}"/>`
  return svg(artwork)
}

export function nativeAppIconSvg({ macos = false } = {}) {
  const artwork = `<rect width="1024" height="1024" rx="232" fill="#111111"/>
    <path d="${markPath}" fill="#ffffff"/>`
  // Match the macOS optical inset used by the existing app icon.
  const content = macos
    ? `<g transform="translate(100 100) scale(0.8046875)">${artwork}</g>`
    : artwork
  return svg(content)
}

function svg(content) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  ${content}
</svg>
`
}

export function trayIconSvg() {
  // Keep only the solid white mark on transparency, centered in a 36 px
  // canvas for the system tray and an 18 pt Retina macOS menu bar.
  return `<svg xmlns="http://www.w3.org/2000/svg" width="36" height="36" viewBox="0 0 1024 1024">
  <g transform="translate(0 78)">
    <path d="${markPath}" fill="#ffffff"/>
  </g>
</svg>
`
}
