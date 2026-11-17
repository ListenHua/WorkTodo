import {
  fileKind,
  fileLimit,
  imageLimit,
  imageMime,
  uniqueFileName,
  validateFileName,
} from '../shared/files'
import type { ProjectFile, ProjectFilesApi } from '../shared/files'

// File objects live only in this tab. This preview never copies files to a project directory.
export function createFilesPreview(
  projectExists: (id: string) => Promise<boolean>,
): ProjectFilesApi {
  const entries = new Map<string, { record: ProjectFile; file: File }>()
  async function requireProject(id: string) {
    if (!(await projectExists(id))) throw new Error('项目不存在')
  }
  async function list(projectId: string) {
    await requireProject(projectId)
    return [...entries.values()]
      .filter(({ record }) => record.projectId === projectId)
      .map(({ record }) => ({ ...record }))
  }
  return {
    list,
    scan: list,
    async pick() {
      return []
    },
    async importFile(projectId, source, _requestId, progress) {
      await requireProject(projectId)
      if (source.kind !== 'browser') throw new Error('浏览器预览无法读取本地文件路径')
      const file = source.file
      const invalid = validateFileName(file.name)
      if (invalid) throw new Error(invalid)
      const limit = fileKind(file.name) === 'image' ? imageLimit : fileLimit
      if (file.size > limit)
        throw new Error(limit === imageLimit ? '图片不能超过 20 MB' : '文件不能超过 200 MB')
      const name = uniqueFileName(
        file.name,
        (await list(projectId)).map((entry) => entry.relativePath),
      )
      const record: ProjectFile = {
        id: crypto.randomUUID(),
        projectId,
        name,
        relativePath: name,
        size: file.size,
        modifiedAt: new Date(file.lastModified).toISOString(),
        kind: fileKind(name),
        status: 'available',
      }
      entries.set(record.id, { record, file })
      progress({ copied: file.size, total: file.size })
      return { ...record }
    },
    async copy(projectId, id, target) {
      const entry = entries.get(id)
      if (!entry || entry.record.projectId !== projectId) throw new Error('文件不存在')
      await requireProject(target)
      const name = uniqueFileName(
        entry.file.name,
        (await list(target)).map((f) => f.name),
      )
      const record = {
        ...entry.record,
        id: crypto.randomUUID(),
        projectId: target,
        name,
        relativePath: name,
      }
      entries.set(record.id, { record, file: entry.file })
      return { ...record }
    },
    async remove(projectId, id) {
      await requireProject(projectId)
      const entry = entries.get(id)
      if (!entry || entry.record.projectId !== projectId) throw new Error('文件不存在')
      entries.delete(id)
      return 'removed'
    },
    async cancel() {},
    async preview(projectId, file) {
      await requireProject(projectId)
      const entry = entries.get(file.id)
      if (!entry || entry.record.projectId !== projectId) throw new Error('文件不存在')
      const mime = imageMime(entry.record.name)
      if (!mime) throw new Error('此格式请使用系统程序打开')
      return entry.file.slice(0, entry.file.size, mime)
    },
    async open() {
      throw new Error('请在桌面端打开或定位本地项目文件')
    },
  }
}
