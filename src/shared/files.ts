export interface ProjectFile {
  id: string
  projectId: string
  name: string
  relativePath: string
  size: number
  modifiedAt: string
  status: 'available' | 'missing'
  kind: 'image' | 'document' | 'other'
}

export type FileSource = { kind: 'native'; path: string } | { kind: 'browser'; file: File }
export interface ImportProgress {
  copied: number
  total: number
}
export interface ProjectFilesApi {
  list(projectId: string): Promise<ProjectFile[]>
  scan(projectId: string): Promise<ProjectFile[]>
  pick(): Promise<string[]>
  importFile(
    projectId: string,
    source: FileSource,
    requestId: string,
    onProgress: (progress: ImportProgress) => void,
  ): Promise<ProjectFile>
  copy?(projectId: string, id: string, target: string): Promise<ProjectFile>
  cancel(requestId: string): Promise<void>
  remove(projectId: string, id: string): Promise<'recycled' | 'removed'>
  preview(projectId: string, file: ProjectFile): Promise<Blob>
  open(projectId: string, id: string, reveal: boolean): Promise<void>
}

export const imageLimit = 20 * 1024 * 1024
export const fileLimit = 200 * 1024 * 1024
export function imageMime(name: string): string | undefined {
  const extension = name.split('.').pop()?.toLowerCase() || ''
  return (
    {
      png: 'image/png',
      jpg: 'image/jpeg',
      jpeg: 'image/jpeg',
      gif: 'image/gif',
      webp: 'image/webp',
      bmp: 'image/bmp',
    } as Record<string, string>
  )[extension]
}
export function fileKind(name: string): ProjectFile['kind'] {
  if (imageMime(name) || /\.(heic|heif|tiff?|avif)$/i.test(name)) return 'image'
  return /\.(pdf|docx?|xlsx?|pptx?|txt|md|csv|rtf|odt|ods)$/i.test(name) ? 'document' : 'other'
}
export function sourceName(source: FileSource): string {
  return source.kind === 'browser' ? source.file.name : source.path.split(/[\\/]/).pop() || '文件'
}
export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}
export function uniqueFileName(name: string, existing: string[]): string {
  const normalized = new Set(existing.map((value) => value.normalize('NFC').toLowerCase()))
  const dot = name.lastIndexOf('.')
  const stem = dot > 0 ? name.slice(0, dot) : name
  const suffix = dot > 0 ? name.slice(dot) : ''
  for (let index = 0; index < 10_000; index++) {
    const candidate = index ? `${stem} (${index})${suffix}` : name
    if (!normalized.has(candidate.normalize('NFC').toLowerCase())) return candidate
  }
  throw new Error('同名文件过多，请重命名后重试')
}

export function validateFileName(name: string): string | null {
  if (
    !name ||
    name.startsWith('.project-manage-import-') ||
    [...name].length > 255 ||
    /[<>:"/\\|?*\u0000-\u001f\u007f]/.test(name) ||
    /[. ]$/.test(name) ||
    ['.', '..'].includes(name) ||
    /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(name)
  )
    return '文件名含有不可用字符或保留名称，请重命名后重试'
  return null
}

export function canPreviewFile(file: ProjectFile): boolean {
  return file.status === 'available' && !!imageMime(file.name) && file.size <= imageLimit
}
