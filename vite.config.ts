import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'

const applicationVersion = JSON.parse(
  readFileSync(new URL('./package.json', import.meta.url), 'utf8'),
).version
let revision: string | null = null
let dirty = true
try {
  revision = execFileSync('git', ['rev-parse', 'HEAD'], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'ignore'],
  }).trim()
  dirty = !!execFileSync('git', ['status', '--porcelain', '--untracked-files=normal'], {
    encoding: 'utf8',
  }).trim()
} catch {}
const channel = process.env.PROJECT_MANAGE_RELEASE_TYPE === 'stable' ? 'stable' : 'internal'

export default defineConfig({
  plugins: [vue()],
  define: {
    __APP_BUILD__: JSON.stringify({ version: applicationVersion, revision, dirty, channel }),
  },
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: { target: 'es2021' },
})
