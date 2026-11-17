import type { Project } from './types'

export const projectSortOptions = [
  { label: '自定义排序', value: 'manual' },
  { label: '名称升序', value: 'name-asc' },
  { label: '名称降序', value: 'name-desc' },
  { label: '创建时间从新到旧', value: 'created-desc' },
  { label: '创建时间从旧到新', value: 'created-asc' },
] as const
export type ProjectSortMode = (typeof projectSortOptions)[number]['value']
export interface ProjectSort {
  mode: ProjectSortMode
  order: string[]
}
export function readProjectSort(value: string | null): ProjectSort {
  const fallback: ProjectSort = { mode: 'created-desc', order: [] }
  if (!value || value.length > 512000) return fallback
  try {
    const parsed = JSON.parse(value) as ProjectSort
    if (
      !parsed ||
      !projectSortOptions.some((option) => option.value === parsed.mode) ||
      !Array.isArray(parsed.order) ||
      parsed.order.length > 10000 ||
      !parsed.order.every(
        (id) =>
          typeof id === 'string' &&
          /^[\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12}$/i.test(id),
      ) ||
      new Set(parsed.order).size !== parsed.order.length
    )
      return fallback
    return { mode: parsed.mode, order: [...parsed.order] }
  } catch {
    return fallback
  }
}
const collator = new Intl.Collator('zh-CN', { numeric: true, sensitivity: 'base' })
export function sortProjects(projects: Project[], sort: ProjectSort): Project[] {
  const positions = new Map(sort.order.map((id, index) => [id, index]))
  return [...projects].sort((a, b) => {
    if (sort.mode === 'manual') {
      const difference =
        (positions.get(a.id) ?? sort.order.length) - (positions.get(b.id) ?? sort.order.length)
      if (difference) return difference
    } else if (sort.mode === 'name-asc' || sort.mode === 'name-desc') {
      const difference = collator.compare(a.name, b.name)
      if (difference) return sort.mode === 'name-asc' ? difference : -difference
    }
    const difference = (Date.parse(a.createdAt) || 0) - (Date.parse(b.createdAt) || 0)
    return (sort.mode === 'created-asc' ? difference : -difference) || a.id.localeCompare(b.id)
  })
}
