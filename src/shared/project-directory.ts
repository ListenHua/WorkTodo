import type { ProjectDirectoryConflict } from './types'

export function projectDirectoryConflict(error: unknown): ProjectDirectoryConflict | null {
  if (
    typeof error !== 'object' ||
    error === null ||
    !('code' in error) ||
    error.code !== 'DIRECTORY_EXISTS' ||
    !('details' in error)
  )
    return null
  const details = error.details
  if (typeof details !== 'object' || details === null) return null
  if (
    !('name' in details) ||
    typeof details.name !== 'string' ||
    !details.name ||
    !('path' in details) ||
    typeof details.path !== 'string' ||
    !details.path ||
    !('fingerprint' in details) ||
    typeof details.fingerprint !== 'string' ||
    !details.fingerprint
  )
    return null
  return { name: details.name, path: details.path, fingerprint: details.fingerprint }
}
