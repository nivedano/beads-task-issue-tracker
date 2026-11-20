import type { ColumnConfig } from '~/types/issue'

/**
 * Merge a drag-reordered list of visible columns back into the full column
 * array. Excluded columns (e.g. an identity column) are not reorderable and
 * keep their original index. Returns null when the reordered list does not
 * cover every visible column, so a partial drag never drops columns.
 */
export function mergeReorderedColumns(
  original: ColumnConfig[],
  orderedVisible: ColumnConfig[],
  excludeIds: string[] = [],
): ColumnConfig[] | null {
  const visibleCount = original.filter(column => !excludeIds.includes(column.id)).length
  if (orderedVisible.length !== visibleCount) return null

  let cursor = 0
  return original.map((column) => {
    if (excludeIds.includes(column.id)) return column
    return orderedVisible[cursor++]!
  })
}